// crates/minion-presentation/src/db.rs
use crate::schema::types::{DeckId, DeckSummary};
use chrono::Utc;
use minion_db::{Database, Result};

#[derive(Clone)]
pub struct PresentationDb {
    db: Database,
}

impl PresentationDb {
    pub fn new(db: Database) -> Self { Self { db } }

    pub fn insert_presentation(
        &self,
        id: &DeckId,
        title: &str,
        bundle_path: &str,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO presentations (id, title, created_at, updated_at, bundle_path, thumbnail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id.0.to_string(), title, now, now, bundle_path, thumbnail
            ],
        )?;
        Ok(())
    }

    pub fn update_presentation_title(&self, id: &DeckId, title: &str) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presentations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![title, now, id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn update_thumbnail(&self, id: &DeckId, thumbnail: Vec<u8>) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presentations SET thumbnail = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![thumbnail, now, id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn delete_presentation(&self, id: &DeckId) -> Result<()> {
        let conn = self.db.get()?;
        conn.execute(
            "DELETE FROM presentations WHERE id = ?1",
            rusqlite::params![id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn list_presentations(&self) -> Result<Vec<DeckSummary>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, thumbnail, slide_count
             FROM presentations ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let title: String = row.get(1)?;
            let created_ts: i64 = row.get(2)?;
            let updated_ts: i64 = row.get(3)?;
            let thumbnail: Option<Vec<u8>> = row.get(4)?;
            let slide_count: i64 = row.get(5)?;
            Ok((id_str, title, created_ts, updated_ts, thumbnail, slide_count))
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            let (id_str, title, created_ts, updated_ts, thumb, slide_count) = row?;
            let id = uuid::Uuid::parse_str(&id_str)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            summaries.push(DeckSummary {
                id: DeckId(id),
                title,
                slide_count: slide_count as usize,
                created_at: chrono::DateTime::from_timestamp(created_ts, 0)
                    .unwrap_or_else(chrono::Utc::now),
                updated_at: chrono::DateTime::from_timestamp(updated_ts, 0)
                    .unwrap_or_else(chrono::Utc::now),
                thumbnail_data_url: thumb.map(|b| {
                    use base64ct::{Base64, Encoding};
                    format!("data:image/png;base64,{}", Base64::encode_string(&b))
                }),
            });
        }
        Ok(summaries)
    }

    pub fn get_bundle_path(&self, id: &DeckId) -> Result<Option<String>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT bundle_path FROM presentations WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![id.0.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        Ok(rows.next().transpose()?)
    }

    /// Look up a single presentation by ID. Returns `None` if not found.
    /// `slide_count` is always 0 here — callers that need the real count must
    /// load the deck bundle and call `Deck::slide_count()`.
    pub fn get_presentation(&self, id: &DeckId) -> Result<Option<DeckSummary>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, thumbnail
             FROM presentations WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![id.0.to_string()],
            |row| {
                let id_str: String = row.get(0)?;
                let title: String = row.get(1)?;
                let created_ts: i64 = row.get(2)?;
                let updated_ts: i64 = row.get(3)?;
                let thumbnail: Option<Vec<u8>> = row.get(4)?;
                Ok((id_str, title, created_ts, updated_ts, thumbnail))
            },
        )?;
        match rows.next() {
            None => Ok(None),
            Some(row) => {
                let (id_str, title, created_ts, updated_ts, thumb) = row?;
                let uid = uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
                Ok(Some(DeckSummary {
                    id: DeckId(uid),
                    title,
                    slide_count: 0,
                    created_at: chrono::DateTime::from_timestamp(created_ts, 0)
                        .unwrap_or_else(chrono::Utc::now),
                    updated_at: chrono::DateTime::from_timestamp(updated_ts, 0)
                        .unwrap_or_else(chrono::Utc::now),
                    thumbnail_data_url: thumb.map(|b| {
                        use base64ct::{Base64, Encoding};
                        format!("data:image/png;base64,{}", Base64::encode_string(&b))
                    }),
                }))
            }
        }
    }

    /// Update the stored slide count for a presentation (call after saving the bundle).
    pub fn update_slide_count(&self, id: &DeckId, count: usize) -> Result<()> {
        let conn = self.db.get()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE presentations SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id.0.to_string()],
        )?;
        // Store count in DB so list_presentations can return it without loading bundles.
        // This requires a schema migration (see migrations.rs — add slide_count column).
        let _ = conn.execute(
            "UPDATE presentations SET slide_count = ?1 WHERE id = ?2",
            rusqlite::params![count as i64, id.0.to_string()],
        );
        Ok(())
    }
}
