//! MINION AI Layer
//!
//! Provides Ollama integration, embedding generation, and RAG capabilities.

pub mod embeddings;
pub mod ollama;
pub mod rag;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Ollama error: {0}")]
    Ollama(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("RAG error: {0}")]
    Rag(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Timeout")]
    Timeout,
}

pub type Result<T> = std::result::Result<T, Error>;

/// AI configuration
#[derive(Debug, Clone)]
pub struct AIConfig {
    pub ollama_host: String,
    pub ollama_port: u16,
    pub default_model: String,
    pub embedding_model: String,
    pub timeout_seconds: u64,
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            ollama_host: "127.0.0.1".to_string(),
            ollama_port: 11434,
            default_model: "llama3.2:3b".to_string(),
            embedding_model: "nomic-embed-text".to_string(),
            timeout_seconds: 300,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_config_default() {
        let config = AIConfig::default();

        assert_eq!(config.ollama_host, "127.0.0.1");
        assert_eq!(config.ollama_port, 11434);
        assert_eq!(config.default_model, "llama3.2:3b");
        assert_eq!(config.embedding_model, "nomic-embed-text");
        assert_eq!(config.timeout_seconds, 300);
    }

    #[test]
    fn test_ai_config_clone() {
        let config = AIConfig::default();
        let cloned = config.clone();

        assert_eq!(cloned.ollama_host, config.ollama_host);
        assert_eq!(cloned.ollama_port, config.ollama_port);
        assert_eq!(cloned.default_model, config.default_model);
    }

    #[test]
    fn test_error_variants() {
        let ollama_err = Error::Ollama("test error".to_string());
        assert!(ollama_err.to_string().contains("Ollama error"));

        let embedding_err = Error::Embedding("test error".to_string());
        assert!(embedding_err.to_string().contains("Embedding error"));

        let rag_err = Error::Rag("test error".to_string());
        assert!(rag_err.to_string().contains("RAG error"));

        let model_err = Error::ModelNotFound("llama".to_string());
        assert!(model_err.to_string().contains("Model not found"));

        let timeout_err = Error::Timeout;
        assert!(timeout_err.to_string().contains("Timeout"));
    }

    #[test]
    fn test_error_from_reqwest() {
        // We can't easily create a reqwest::Error, but we can test the From impl exists
        // by ensuring the error type compiles with the from conversion
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Timeout);
        assert!(err_result.is_err());
    }
}
