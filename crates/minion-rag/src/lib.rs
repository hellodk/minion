//! MINION RAG — Retrieval-Augmented Generation primitives.
//!
//! Three concerns live here:
//!
//! * [`chunker`] — split documents into overlapping, size-bounded chunks
//!   that respect markdown structure (no splitting mid-code-fence).
//! * [`embeddings`] — pluggable embedding provider trait; ships with an
//!   [`OllamaEmbedder`] that hits `/api/embeddings`.
//! * [`store`] — a SQLite-backed vector store that does brute-force
//!   cosine similarity against stored BLOB embeddings. Fine up to ~1M
//!   rows on modest hardware; we can swap in `sqlite-vec` later without
//!   touching callers.
//!
//! The high-level [`RagPipeline`] ties the three together so callers
//! just say `pipeline.index(doc)` and `pipeline.search(query, k)`.

pub mod chunker;
pub mod embeddings;
pub mod error;
pub mod pipeline;
pub mod store;

pub use chunker::{chunk_markdown, ChunkOptions, Chunk};
pub use embeddings::{EmbeddingProvider, OllamaEmbedder};
pub use error::{RagError, RagResult};
pub use pipeline::{RagPipeline, SearchHit};
pub use store::{StoredChunk, VectorStore};
