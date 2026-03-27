//! Error types for the core engine

use thiserror::Error;

/// Core engine result type
pub type Result<T> = std::result::Result<T, Error>;

/// Core engine errors
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Event bus error: {0}")]
    EventBus(String),

    #[error("Task error: {0}")]
    Task(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Crypto error: {0}")]
    Crypto(#[from] minion_crypto::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}
