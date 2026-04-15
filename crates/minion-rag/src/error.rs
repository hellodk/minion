use thiserror::Error;

#[derive(Debug, Error)]
pub enum RagError {
    #[error("embedding request failed: {0}")]
    Embedding(String),

    #[error("vector dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("not found: {0}")]
    NotFound(String),
}

pub type RagResult<T> = Result<T, RagError>;
