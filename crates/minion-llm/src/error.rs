//! Error types for the LLM provider abstraction.

use thiserror::Error;

/// Result type alias used across this crate.
pub type LlmResult<T> = Result<T, LlmError>;

/// Errors returned by LLM providers.
#[derive(Debug, Error)]
pub enum LlmError {
    /// HTTP/network-level failure (connection refused, timeout, TLS, etc.).
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// The endpoint returned a non-success status code.
    #[error("Provider returned HTTP {status}: {body}")]
    ProviderHttp { status: u16, body: String },

    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The provider response could not be understood (missing fields, etc.).
    #[error("Invalid response from provider: {0}")]
    InvalidResponse(String),

    /// Required configuration was missing (e.g. API key for a cloud provider).
    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    /// The request itself was malformed.
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Catch-all for other unexpected conditions.
    #[error("LLM error: {0}")]
    Other(String),
}

impl LlmError {
    /// Create an `InvalidResponse` from any displayable value.
    pub fn invalid_response<S: Into<String>>(msg: S) -> Self {
        Self::InvalidResponse(msg.into())
    }

    /// Create a `MissingConfig` from any displayable value.
    pub fn missing_config<S: Into<String>>(msg: S) -> Self {
        Self::MissingConfig(msg.into())
    }

    /// Create an `InvalidRequest` from any displayable value.
    pub fn invalid_request<S: Into<String>>(msg: S) -> Self {
        Self::InvalidRequest(msg.into())
    }
}
