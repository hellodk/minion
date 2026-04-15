//! High-level RAG pipeline: chunk → embed → store → search.

use crate::chunker::{chunk_markdown, Chunk, ChunkOptions};
use crate::embeddings::{normalize, EmbeddingProvider};
use crate::error::RagResult;
use crate::store::{StoredChunk, VectorStore};
use std::sync::Arc;

pub struct RagPipeline {
    store: VectorStore,
    embedder: Arc<dyn EmbeddingProvider>,
    options: ChunkOptions,
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub score: f32,
    pub chunk: StoredChunk,
}

impl RagPipeline {
    pub fn new(store: VectorStore, embedder: Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            store,
            embedder,
            options: ChunkOptions::default(),
        }
    }

    pub fn with_options(mut self, options: ChunkOptions) -> Self {
        self.options = options;
        self
    }

    /// Index (or re-index) a document. Existing chunks for `doc_id`
    /// are wiped first so this is idempotent.
    pub async fn index(
        &self,
        doc_id: &str,
        title: Option<&str>,
        source_path: Option<&str>,
        body: &str,
    ) -> RagResult<usize> {
        self.store.upsert_document(doc_id, title, source_path)?;
        self.store.clear_document(doc_id)?;
        let chunks: Vec<Chunk> = chunk_markdown(body, self.options);
        if chunks.is_empty() {
            return Ok(0);
        }
        for c in &chunks {
            let mut emb = self.embedder.embed(&c.text).await?;
            normalize(&mut emb);
            self.store.insert_chunk(
                doc_id,
                c.index as i64,
                &c.text,
                c.heading.as_deref(),
                c.start_char as i64,
                &emb,
            )?;
        }
        Ok(chunks.len())
    }

    pub async fn search(
        &self,
        query: &str,
        k: usize,
        doc_filter: Option<&str>,
    ) -> RagResult<Vec<SearchHit>> {
        let mut q = self.embedder.embed(query).await?;
        normalize(&mut q);
        let rows = self.store.top_k(&q, k, doc_filter)?;
        Ok(rows
            .into_iter()
            .map(|(score, chunk)| SearchHit { score, chunk })
            .collect())
    }

    pub fn delete_document(&self, doc_id: &str) -> RagResult<()> {
        self.store.delete_document(doc_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::normalize;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;

    /// Deterministic fake embedder for tests: hashes the text into a
    /// tiny vector so we can reason about ranking without running Ollama.
    struct FakeEmbedder {
        dim: usize,
        calls: AtomicUsize,
    }
    impl FakeEmbedder {
        fn new(dim: usize) -> Self {
            Self { dim, calls: AtomicUsize::new(0) }
        }
        fn hash_vec(&self, s: &str) -> Vec<f32> {
            let mut v = vec![0.0_f32; self.dim];
            for (i, b) in s.bytes().enumerate() {
                v[i % self.dim] += b as f32;
            }
            normalize(&mut v);
            v
        }
    }
    #[async_trait]
    impl EmbeddingProvider for FakeEmbedder {
        fn name(&self) -> &str { "fake" }
        fn dimension(&self) -> usize { self.dim }
        async fn embed(&self, text: &str) -> RagResult<Vec<f32>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.hash_vec(text))
        }
    }

    #[tokio::test]
    async fn index_and_search_end_to_end() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("r.db")).unwrap();
        let embedder = Arc::new(FakeEmbedder::new(16));
        let pipeline = RagPipeline::new(store, embedder.clone());

        let body = "# Kubernetes\n\nK8s is a container orchestration platform.\n\n\
                    # Something Else\n\nUnrelated content here.";
        let n = pipeline.index("doc1", Some("Intro"), None, body).await.unwrap();
        assert!(n >= 1);

        let hits = pipeline.search("Kubernetes container orchestration", 2, None).await.unwrap();
        assert!(!hits.is_empty());
        // Top hit should reference Kubernetes, not the unrelated section.
        assert!(
            hits[0].chunk.text.to_lowercase().contains("kubernetes")
                || hits[0]
                    .chunk
                    .heading
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains("kubernetes"),
            "top hit did not reference Kubernetes: {:?}",
            hits[0].chunk.text
        );
    }

    #[tokio::test]
    async fn re_index_replaces_prior_chunks() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("r.db")).unwrap();
        let embedder = Arc::new(FakeEmbedder::new(8));
        let pipeline = RagPipeline::new(store, embedder);
        pipeline.index("d", None, None, "# a\n\nv1").await.unwrap();
        let n1 = pipeline.store.chunk_count().unwrap();
        pipeline.index("d", None, None, "# a\n\nv2").await.unwrap();
        let n2 = pipeline.store.chunk_count().unwrap();
        assert_eq!(n1, n2, "re-indexing should not accumulate chunks");
    }

    #[tokio::test]
    async fn doc_filter_scopes_search() {
        let dir = tempdir().unwrap();
        let store = VectorStore::open(&dir.path().join("r.db")).unwrap();
        let embedder = Arc::new(FakeEmbedder::new(16));
        let pipeline = RagPipeline::new(store, embedder);
        pipeline.index("doc_a", None, None, "# A\n\nrelevant kubernetes content").await.unwrap();
        pipeline.index("doc_b", None, None, "# B\n\nalso kubernetes content").await.unwrap();
        let hits = pipeline.search("kubernetes", 10, Some("doc_a")).await.unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|h| h.chunk.doc_id == "doc_a"));
    }
}
