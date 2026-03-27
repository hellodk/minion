//! Configuration management for MINION

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Application data directory
    pub data_dir: PathBuf,

    /// Configuration directory
    pub config_dir: PathBuf,

    /// Cache directory
    pub cache_dir: PathBuf,

    /// Database configuration
    pub database: DatabaseConfig,

    /// Worker configuration
    pub workers: WorkerConfig,

    /// UI configuration
    pub ui: UiConfig,

    /// AI configuration
    pub ai: AiConfig,

    /// Security configuration
    pub security: SecurityConfig,

    /// Module-specific configurations
    #[serde(default)]
    pub modules: ModulesConfig,
}

impl Config {
    /// Load configuration from the default location
    pub fn load() -> Result<Self> {
        let config_dir = Self::default_config_dir()?;
        let config_path = config_dir.join("config.toml");

        if config_path.exists() {
            Self::load_from(&config_path)
        } else {
            Ok(Self::default())
        }
    }

    /// Load configuration from a specific path
    pub fn load_from(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let config_path = self.config_dir.join("config.toml");
        let contents = toml::to_string_pretty(self).map_err(|e| Error::Config(e.to_string()))?;
        std::fs::write(config_path, contents)?;
        Ok(())
    }

    /// Get the default configuration directory
    fn default_config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Config("Could not determine home directory".into()))?;
        Ok(home.join(".minion").join("config"))
    }

    /// Get the default data directory
    fn default_data_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Config("Could not determine home directory".into()))?;
        Ok(home.join(".minion").join("data"))
    }

    /// Get the default cache directory
    fn default_cache_dir() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| Error::Config("Could not determine home directory".into()))?;
        Ok(home.join(".minion").join("cache"))
    }
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = Self::default_data_dir().unwrap_or_else(|_| PathBuf::from(".minion/data"));
        let config_dir =
            Self::default_config_dir().unwrap_or_else(|_| PathBuf::from(".minion/config"));
        let cache_dir =
            Self::default_cache_dir().unwrap_or_else(|_| PathBuf::from(".minion/cache"));

        Self {
            data_dir,
            config_dir,
            cache_dir,
            database: DatabaseConfig::default(),
            workers: WorkerConfig::default(),
            ui: UiConfig::default(),
            ai: AiConfig::default(),
            security: SecurityConfig::default(),
            modules: ModulesConfig::default(),
        }
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database file path (relative to data_dir)
    pub path: PathBuf,

    /// Connection pool size
    pub pool_size: u32,

    /// SQLite cache size (negative = KB)
    pub cache_size: i32,

    /// Enable WAL mode
    pub wal_mode: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("minion.db"),
            pool_size: 4,
            cache_size: -64000, // 64MB
            wal_mode: true,
        }
    }
}

/// Worker thread configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Number of background worker threads
    pub background_workers: usize,

    /// Number of file scanning threads
    pub scan_workers: usize,

    /// Number of hash computation threads
    pub hash_workers: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        let cpus = num_cpus::get();
        Self {
            background_workers: cpus.min(4),
            scan_workers: cpus.min(8),
            hash_workers: cpus.min(4),
        }
    }
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme (dark, light, system)
    pub theme: String,

    /// Enable animations
    pub animations: bool,

    /// Search debounce delay in milliseconds
    pub search_debounce_ms: u32,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            animations: true,
            search_debounce_ms: 300,
        }
    }
}

/// AI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Ollama server host
    pub ollama_host: String,

    /// Ollama server port
    pub ollama_port: u16,

    /// Default LLM model
    pub default_model: String,

    /// Default embedding model
    pub embedding_model: String,

    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for AiConfig {
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

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Require master password on startup
    pub require_password: bool,

    /// Session timeout in minutes (0 = no timeout)
    pub session_timeout_minutes: u32,

    /// Enable audit logging
    pub audit_logging: bool,

    /// Audit log retention days
    pub audit_retention_days: u32,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_password: true,
            session_timeout_minutes: 30,
            audit_logging: true,
            audit_retention_days: 90,
        }
    }
}

/// Module-specific configurations
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModulesConfig {
    pub media: Option<serde_json::Value>,
    pub files: Option<serde_json::Value>,
    pub blog: Option<serde_json::Value>,
    pub finance: Option<serde_json::Value>,
    pub fitness: Option<serde_json::Value>,
    pub reader: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_default() {
        let config = Config::default();

        // Verify default values
        assert_eq!(config.database.pool_size, 4);
        assert!(config.database.wal_mode);
        assert_eq!(config.ui.theme, "system");
        assert!(config.ui.animations);
        assert_eq!(config.ai.ollama_port, 11434);
        assert!(config.security.require_password);
    }

    #[test]
    fn test_database_config_default() {
        let db_config = DatabaseConfig::default();

        assert_eq!(db_config.path, PathBuf::from("minion.db"));
        assert_eq!(db_config.pool_size, 4);
        assert_eq!(db_config.cache_size, -64000);
        assert!(db_config.wal_mode);
    }

    #[test]
    fn test_worker_config_default() {
        let worker_config = WorkerConfig::default();
        let cpus = num_cpus::get();

        assert!(worker_config.background_workers <= 4);
        assert!(worker_config.background_workers >= 1);
        assert!(worker_config.scan_workers <= 8);
        assert!(worker_config.hash_workers <= 4);
    }

    #[test]
    fn test_ui_config_default() {
        let ui_config = UiConfig::default();

        assert_eq!(ui_config.theme, "system");
        assert!(ui_config.animations);
        assert_eq!(ui_config.search_debounce_ms, 300);
    }

    #[test]
    fn test_ai_config_default() {
        let ai_config = AiConfig::default();

        assert_eq!(ai_config.ollama_host, "127.0.0.1");
        assert_eq!(ai_config.ollama_port, 11434);
        assert_eq!(ai_config.default_model, "llama3.2:3b");
        assert_eq!(ai_config.embedding_model, "nomic-embed-text");
        assert_eq!(ai_config.timeout_seconds, 300);
    }

    #[test]
    fn test_security_config_default() {
        let security_config = SecurityConfig::default();

        assert!(security_config.require_password);
        assert_eq!(security_config.session_timeout_minutes, 30);
        assert!(security_config.audit_logging);
        assert_eq!(security_config.audit_retention_days, 90);
    }

    #[test]
    fn test_config_save_and_load() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config_path = dir.path().join("config.toml");

        let mut config = Config::default();
        config.config_dir = dir.path().to_path_buf();
        config.ui.theme = "dark".to_string();
        config.ui.animations = false;

        // Save config
        config.save().expect("Failed to save config");

        // Load config
        let loaded = Config::load_from(&config_path).expect("Failed to load config");

        assert_eq!(loaded.ui.theme, "dark");
        assert!(!loaded.ui.animations);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();

        // Serialize to TOML
        let toml_str = toml::to_string_pretty(&config).expect("Failed to serialize");
        assert!(toml_str.contains("[database]"));
        assert!(toml_str.contains("[workers]"));
        assert!(toml_str.contains("[ui]"));

        // Deserialize back
        let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize");
        assert_eq!(deserialized.database.pool_size, config.database.pool_size);
    }

    #[test]
    fn test_config_load_from_nonexistent() {
        let result = Config::load_from(Path::new("/nonexistent/path/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_config_load_defaults_when_missing() {
        // This tests the load() method which returns default when file doesn't exist
        // Since it uses default_config_dir(), we test with explicit path
        let dir = tempdir().expect("Failed to create temp dir");
        let config_path = dir.path().join("config.toml");

        // File doesn't exist, should fall back to default
        let result = Config::load_from(&config_path);
        assert!(result.is_err()); // load_from returns error, not default
    }

    #[test]
    fn test_modules_config_default() {
        let modules = ModulesConfig::default();

        assert!(modules.media.is_none());
        assert!(modules.files.is_none());
        assert!(modules.blog.is_none());
        assert!(modules.finance.is_none());
        assert!(modules.fitness.is_none());
        assert!(modules.reader.is_none());
    }

    #[test]
    fn test_config_with_custom_modules() {
        let mut config = Config::default();
        config.modules.files = Some(serde_json::json!({
            "scan_hidden": true,
            "max_depth": 10
        }));

        let toml_str = toml::to_string_pretty(&config).expect("Failed to serialize");
        let deserialized: Config = toml::from_str(&toml_str).expect("Failed to deserialize");

        let files_config = deserialized.modules.files.unwrap();
        assert_eq!(files_config["scan_hidden"], true);
        assert_eq!(files_config["max_depth"], 10);
    }
}
