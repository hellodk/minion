//! Vector store backed by SQLite BLOBs.
//!
//! Brute-force cosine similarity by scanning every row is O(N·D), which
//! on a modern laptop handles ~500k 768-dim rows in well under a second.
//! When we outgrow that (we won't soon) we can swap the query to
//! `sqlite-vec` without touching the caller — the row shape stays the
//! same.

use crate::error::{RagError, RagResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

type PooledConn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub const CURRENT_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChunk {
    pub id: String,
    pub doc_id: String,
    pub chunk_index: i64,
    pub text: String,
    pub heading: Option<String>,
    pub start_char: i64,
    pub dimension: i64,
    pub source_path: Option<String>,
    pub created_at: String,
}

/// Wraps a r2d2-pooled Database or a bare Connection.
pub struct VectorStore {
    db: minion_db::Database,
}

impl VectorStore {
    pub fn new(db: minion_db::Database) -> RagResult<Self> {
        let conn = db.get().map_err(|e| RagError::Embedding(e.to_string()))?;
        init_schema(&conn)?;
        Ok(Self { db })
    }

    pub fn open(path: &Path) -> RagResult<Self> {
        // Use open_bare so we don't pollute the RAG database with the
        // full minion health/blog/etc. schema. The only tables we want
        // are our own rag_* pair.
        let db = minion_db::open_bare(path, 2).map_err(|e| RagError::Embedding(e.to_string()))?;
        Self::new(db)
    }

    fn conn(&self) -> RagResult<PooledConn> {
        self.db
            .get()
            .map_err(|e| RagError::Embedding(e.to_string()))
    }

    pub fn upsert_document(
        &self,
        doc_id: &str,
        title: Option<&str>,
        source_path: Option<&str>,
    ) -> RagResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO rag_documents (id, title, source_path, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title,
               source_path = excluded.source_path,
               updated_at = CURRENT_TIMESTAMP",
            params![doc_id, title, source_path],
        )?;
        Ok(())
    }

    /// Wipe all chunks for `doc_id`. Called before re-indexing a document.
    pub fn clear_document(&self, doc_id: &str) -> RagResult<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM rag_chunks WHERE doc_id = ?1", params![doc_id])?;
        Ok(())
    }

    pub fn insert_chunk(
        &self,
        doc_id: &str,
        chunk_index: i64,
        text: &str,
        heading: Option<&str>,
        start_char: i64,
        embedding: &[f32],
    ) -> RagResult<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO rag_chunks
             (id, doc_id, chunk_index, text, heading, start_char, dimension, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                doc_id,
                chunk_index,
                text,
                heading,
                start_char,
                embedding.len() as i64,
                embedding_to_bytes(embedding),
            ],
        )?;
        Ok(id)
    }

    pub fn chunk_count(&self) -> RagResult<i64> {
        let conn = self.conn()?;
        Ok(conn.query_row("SELECT COUNT(*) FROM rag_chunks", [], |r| r.get(0))?)
    }

    /// Brute-force top-k cosine similarity.
    ///
    /// Assumes both stored vectors and the query are unit-normalized, so
    /// cosine reduces to a dot product. `normalize()` in `embeddings.rs`
    /// is the canonical way to make this so.
    pub fn top_k(
        &self,
        query: &[f32],
        k: usize,
        doc_filter: Option<&str>,
    ) -> RagResult<Vec<(f32, StoredChunk)>> {
        if k == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn()?;
        let sql = if doc_filter.is_some() {
            "SELECT id, doc_id, chunk_index, text, heading, start_char, dimension,
                    created_at, embedding,
                    (SELECT source_path FROM rag_documents d WHERE d.id = c.doc_id)
             FROM rag_chunks c WHERE doc_id = ?1"
        } else {
            "SELECT id, doc_id, chunk_index, text, heading, start_char, dimension,
                    created_at, embedding,
                    (SELECT source_path FROM rag_documents d WHERE d.id = c.doc_id)
             FROM rag_chunks c"
        };
        let mut stmt = conn.prepare(sql)?;
        type Row = (StoredChunk, Vec<f32>);
        let rows: Vec<Row> = if let Some(filter) = doc_filter {
            stmt.query_map(params![filter], row_mapper)?
                .filter_map(|r| r.ok())
                .collect()
        } else {
            stmt.query_map([], row_mapper)?
                .filter_map(|r| r.ok())
                .collect()
        };
        // Rank by dot product; smaller-than-full set is fine.
        let mut scored: Vec<(f32, StoredChunk)> = rows
            .into_iter()
            .filter(|(_, v)| v.len() == query.len())
            .map(|(c, v)| (dot(query, &v), c))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        Ok(scored)
    }

    pub fn delete_document(&self, doc_id: &str) -> RagResult<()> {
        let conn = self.conn()?;
        conn.execute("DELETE FROM rag_chunks WHERE doc_id = ?1", params![doc_id])?;
        conn.execute("DELETE FROM rag_documents WHERE id = ?1", params![doc_id])?;
        Ok(())
    }

    pub fn document_exists(&self, doc_id: &str) -> RagResult<bool> {
        let conn = self.conn()?;
        Ok(conn
            .query_row(
                "SELECT 1 FROM rag_documents WHERE id = ?1",
                params![doc_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }
}

fn row_mapper(row: &rusqlite::Row) -> rusqlite::Result<(StoredChunk, Vec<f32>)> {
    let chunk = StoredChunk {
        id: row.get(0)?,
        doc_id: row.get(1)?,
        chunk_index: row.get(2)?,
        text: row.get(3)?,
        heading: row.get(4)?,
        start_char: row.get(5)?,
        dimension: row.get(6)?,
        created_at: row.get(7)?,
        source_path: row.get(9)?,
    };
    let bytes: Vec<u8> = row.get(8)?;
    let vec = bytes_to_embedding(&bytes);
    Ok((chunk, vec))
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn embedding_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn bytes_to_embedding(b: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(b.len() / 4);
    for chunk in b.chunks_exact(4) {
        let arr: [u8; 4] = chunk.try_into().unwrap();
        out.push(f32::from_le_bytes(arr));
    }
    out
}

// =====================================================================
// Schema
// =====================================================================

fn init_schema(conn: &Connection) -> RagResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS rag_documents (
            id TEXT PRIMARY KEY,
            title TEXT,
            source_path TEXT,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS rag_chunks (
            id TEXT PRIMARY KEY,
            doc_id TEXT NOT NULL REFERENCES rag_documents(id) ON DELETE CASCADE,
            chunk_index INTEGER NOT NULL,
            text TEXT NOT NULL,
            heading TEXT,
            start_char INTEGER NOT NULL,
            dimension INTEGER NOT NULL,
            embedding BLOB NOT NULL,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_rag_chunks_doc ON rag_chunks(doc_id);
        ",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fake_embedding(seed: f32) -> Vec<f32> {
        let mut v = vec![seed, seed * 0.5, -seed, 1.0 - seed];
        crate::embeddings::normalize(&mut v);
        v
    }

    #[test]
    fn insert_and_top_k_ranks_by_similarity() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("rag.db")).unwrap();
        store
            .upsert_document("d1", Some("Test"), Some("/tmp/t.md"))
            .unwrap();

        let base = fake_embedding(1.0);
        let close = fake_embedding(0.95);
        let far = fake_embedding(-0.9);
        store
            .insert_chunk("d1", 0, "close chunk", Some("h"), 0, &close)
            .unwrap();
        store
            .insert_chunk("d1", 1, "far chunk", Some("h"), 100, &far)
            .unwrap();

        let hits = store.top_k(&base, 2, None).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].1.text, "close chunk");
        assert!(hits[0].0 > hits[1].0, "close should outrank far");
    }

    #[test]
    fn clear_document_removes_chunks() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("rag.db")).unwrap();
        store.upsert_document("d1", None, None).unwrap();
        store
            .insert_chunk("d1", 0, "hi", None, 0, &fake_embedding(1.0))
            .unwrap();
        assert_eq!(store.chunk_count().unwrap(), 1);
        store.clear_document("d1").unwrap();
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn cascade_on_document_delete() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("rag.db")).unwrap();
        store.upsert_document("d1", None, None).unwrap();
        store
            .insert_chunk("d1", 0, "hi", None, 0, &fake_embedding(1.0))
            .unwrap();
        store.delete_document("d1").unwrap();
        assert_eq!(store.chunk_count().unwrap(), 0);
    }

    #[test]
    fn roundtrip_embedding_bytes() {
        let v = vec![1.0_f32, -2.0, 0.5, std::f32::consts::PI];
        let bytes = embedding_to_bytes(&v);
        let back = bytes_to_embedding(&bytes);
        assert_eq!(v, back);
    }
}
