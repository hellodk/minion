//! Plugin system for MINION extensibility

use async_trait::async_trait;
use parking_lot::RwLock;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::config::Config;
use crate::event::{Event, EventBus, EventEnvelope};
use crate::{Error, Result};

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique plugin identifier
    pub id: String,

    /// Display name
    pub name: String,

    /// Plugin version
    pub version: Version,

    /// Author name/email
    pub author: String,

    /// Short description
    pub description: String,

    /// Homepage URL
    pub homepage: Option<String>,

    /// License (SPDX identifier)
    pub license: String,

    /// Required permissions
    pub permissions: Vec<Permission>,

    /// Plugin dependencies
    pub dependencies: Vec<Dependency>,

    /// Minimum MINION version required
    pub min_minion_version: Version,
}

/// Plugin dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub plugin_id: String,
    pub version_req: String,
    pub optional: bool,
}

/// Plugin permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub enum Permission {
    /// Read files matching patterns
    FileRead { patterns: Vec<String> },

    /// Write files matching patterns
    FileWrite { patterns: Vec<String> },

    /// HTTP access to specific hosts
    NetworkHttp { hosts: Vec<String> },

    /// Read from database tables
    DatabaseRead { tables: Vec<String> },

    /// Write to database tables
    DatabaseWrite { tables: Vec<String> },

    /// Access credential vault for services
    CredentialAccess { services: Vec<String> },

    /// Access AI models
    AIModel { models: Vec<String> },

    /// Generate embeddings
    AIEmbeddings,

    /// Access other modules
    ModuleAccess { modules: Vec<String> },

    /// Subscribe to events
    EventSubscribe { event_types: Vec<String> },

    /// Publish events
    EventPublish { event_types: Vec<String> },

    /// Register UI components
    UIRegister { locations: Vec<String> },

    /// Send notifications
    Notifications,
}

/// Plugin capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
}

impl Capability {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

/// Context provided to plugins during initialization
pub struct PluginContext {
    /// Plugin's data directory
    pub data_dir: PathBuf,

    /// Plugin's cache directory
    pub cache_dir: PathBuf,

    /// Plugin configuration
    pub config: serde_json::Value,

    /// Event bus for publishing events
    pub event_bus: Arc<EventBus>,

    /// Granted permissions
    pub permissions: Vec<Permission>,
}

/// Plugin trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata;

    /// Initialize the plugin
    async fn initialize(&mut self, ctx: PluginContext) -> Result<()>;

    /// Shutdown the plugin gracefully
    async fn shutdown(&mut self) -> Result<()>;

    /// Get capabilities provided by this plugin
    fn capabilities(&self) -> Vec<Capability>;

    /// Handle an event
    async fn handle_event(&self, event: &EventEnvelope) -> Result<Option<Event>>;

    /// Handle a command
    async fn handle_command(
        &self,
        command: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value>;
}

/// Plugin state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Unloaded,
    Loaded,
    Initialized,
    Active,
    Error,
}

/// Plugin instance wrapper
struct PluginInstance {
    metadata: PluginMetadata,
    state: PluginState,
    plugin: Option<Box<dyn Plugin>>,
    error: Option<String>,
}

/// Plugin manager handles loading, unloading, and managing plugins
pub struct PluginManager {
    plugins: RwLock<HashMap<String, PluginInstance>>,
    plugin_dir: PathBuf,
    event_bus: Arc<EventBus>,
}

impl PluginManager {
    /// Create a new plugin manager
    pub async fn new(config: &Config) -> Result<Self> {
        let plugin_dir = config.data_dir.join("plugins");
        std::fs::create_dir_all(&plugin_dir)?;

        Ok(Self {
            plugins: RwLock::new(HashMap::new()),
            plugin_dir,
            event_bus: Arc::new(EventBus::new()),
        })
    }

    /// Register a built-in plugin
    pub async fn register(&self, plugin: Box<dyn Plugin>) -> Result<()> {
        let metadata = plugin.metadata();
        let id = metadata.id.clone();

        let instance = PluginInstance {
            metadata,
            state: PluginState::Loaded,
            plugin: Some(plugin),
            error: None,
        };

        self.plugins.write().insert(id.clone(), instance);

        tracing::info!("Registered plugin: {}", id);
        Ok(())
    }

    /// Initialize a plugin
    #[allow(clippy::await_holding_lock)]
    pub async fn initialize(&self, plugin_id: &str, config: &Config) -> Result<()> {
        let ctx = {
            let plugins = self.plugins.read();
            let instance = plugins
                .get(plugin_id)
                .ok_or_else(|| Error::PluginNotFound(plugin_id.to_string()))?;

            PluginContext {
                data_dir: self.plugin_dir.join(plugin_id),
                cache_dir: config.cache_dir.join("plugins").join(plugin_id),
                config: serde_json::Value::Null,
                event_bus: self.event_bus.clone(),
                permissions: instance.metadata.permissions.clone(),
            }
        };

        // Create plugin directories
        std::fs::create_dir_all(&ctx.data_dir)?;
        std::fs::create_dir_all(&ctx.cache_dir)?;

        // Initialize the plugin
        let mut plugins = self.plugins.write();
        let instance = plugins
            .get_mut(plugin_id)
            .ok_or_else(|| Error::PluginNotFound(plugin_id.to_string()))?;

        if let Some(ref mut plugin) = instance.plugin {
            match plugin.initialize(ctx).await {
                Ok(()) => {
                    instance.state = PluginState::Initialized;
                    tracing::info!("Initialized plugin: {}", plugin_id);
                }
                Err(e) => {
                    instance.state = PluginState::Error;
                    instance.error = Some(e.to_string());
                    return Err(e);
                }
            }
        }

        Ok(())
    }

    /// Get plugin metadata
    pub fn get_metadata(&self, plugin_id: &str) -> Option<PluginMetadata> {
        self.plugins
            .read()
            .get(plugin_id)
            .map(|i| i.metadata.clone())
    }

    /// List all registered plugins
    pub fn list(&self) -> Vec<(String, PluginMetadata, PluginState)> {
        self.plugins
            .read()
            .iter()
            .map(|(id, instance)| (id.clone(), instance.metadata.clone(), instance.state))
            .collect()
    }

    /// Check if a plugin has a permission
    pub fn has_permission(&self, plugin_id: &str, permission: &Permission) -> bool {
        self.plugins
            .read()
            .get(plugin_id)
            .map(|i| i.metadata.permissions.contains(permission))
            .unwrap_or(false)
    }

    /// Unload a plugin
    #[allow(clippy::await_holding_lock)]
    pub async fn unload(&self, plugin_id: &str) -> Result<()> {
        let mut plugins = self.plugins.write();

        if let Some(instance) = plugins.get_mut(plugin_id) {
            if let Some(ref mut plugin) = instance.plugin {
                plugin.shutdown().await?;
            }
            instance.state = PluginState::Unloaded;
            instance.plugin = None;
            tracing::info!("Unloaded plugin: {}", plugin_id);
        }

        Ok(())
    }

    /// Unload all plugins
    pub async fn unload_all(&self) -> Result<()> {
        let plugin_ids: Vec<String> = self.plugins.read().keys().cloned().collect();

        for id in plugin_ids {
            if let Err(e) = self.unload(&id).await {
                tracing::error!("Error unloading plugin {}: {}", id, e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tempfile::tempdir;

    struct TestPlugin {
        initialized: Arc<AtomicBool>,
    }

    impl TestPlugin {
        fn new() -> Self {
            Self {
                initialized: Arc::new(AtomicBool::new(false)),
            }
        }

        fn is_initialized(&self) -> bool {
            self.initialized.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Plugin for TestPlugin {
        fn metadata(&self) -> PluginMetadata {
            PluginMetadata {
                id: "test.plugin".to_string(),
                name: "Test Plugin".to_string(),
                version: Version::new(1, 0, 0),
                author: "Test".to_string(),
                description: "A test plugin".to_string(),
                homepage: None,
                license: "MIT".to_string(),
                permissions: vec![Permission::AIEmbeddings],
                dependencies: vec![],
                min_minion_version: Version::new(0, 1, 0),
            }
        }

        async fn initialize(&mut self, _ctx: PluginContext) -> Result<()> {
            self.initialized.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&mut self) -> Result<()> {
            self.initialized.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::new("test", "Test capability")]
        }

        async fn handle_event(&self, _event: &EventEnvelope) -> Result<Option<Event>> {
            Ok(None)
        }

        async fn handle_command(
            &self,
            command: &str,
            _args: serde_json::Value,
        ) -> Result<serde_json::Value> {
            Ok(serde_json::json!({"status": "ok", "command": command}))
        }
    }

    #[test]
    fn test_plugin_metadata() {
        let plugin = TestPlugin::new();
        let metadata = plugin.metadata();

        assert_eq!(metadata.id, "test.plugin");
        assert_eq!(metadata.name, "Test Plugin");
        assert_eq!(metadata.version, Version::new(1, 0, 0));
        assert_eq!(metadata.license, "MIT");
    }

    #[test]
    fn test_plugin_metadata_serialization() {
        let metadata = PluginMetadata {
            id: "test.plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: Version::new(1, 2, 3),
            author: "Test Author".to_string(),
            description: "Test description".to_string(),
            homepage: Some("https://example.com".to_string()),
            license: "MIT".to_string(),
            permissions: vec![Permission::AIEmbeddings],
            dependencies: vec![Dependency {
                plugin_id: "other.plugin".to_string(),
                version_req: ">=1.0.0".to_string(),
                optional: false,
            }],
            min_minion_version: Version::new(0, 1, 0),
        };

        let serialized = serde_json::to_string(&metadata).expect("Failed to serialize");
        let deserialized: PluginMetadata =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(deserialized.id, metadata.id);
        assert_eq!(deserialized.version, metadata.version);
        assert_eq!(deserialized.dependencies.len(), 1);
    }

    #[test]
    fn test_capability_creation() {
        let cap = Capability::new("test_cap", "Test capability description");

        assert_eq!(cap.name, "test_cap");
        assert_eq!(cap.description, "Test capability description");
    }

    #[test]
    fn test_permission_serialization() {
        let permissions = vec![
            Permission::FileRead {
                patterns: vec!["*.txt".to_string()],
            },
            Permission::FileWrite {
                patterns: vec!["*.log".to_string()],
            },
            Permission::NetworkHttp {
                hosts: vec!["api.example.com".to_string()],
            },
            Permission::DatabaseRead {
                tables: vec!["users".to_string()],
            },
            Permission::DatabaseWrite {
                tables: vec!["logs".to_string()],
            },
            Permission::CredentialAccess {
                services: vec!["github".to_string()],
            },
            Permission::AIModel {
                models: vec!["llama3.2".to_string()],
            },
            Permission::AIEmbeddings,
            Permission::ModuleAccess {
                modules: vec!["files".to_string()],
            },
            Permission::EventSubscribe {
                event_types: vec!["FileCreated".to_string()],
            },
            Permission::EventPublish {
                event_types: vec!["Custom:*".to_string()],
            },
            Permission::UIRegister {
                locations: vec!["sidebar".to_string()],
            },
            Permission::Notifications,
        ];

        for perm in &permissions {
            let serialized = serde_json::to_string(perm).expect("Failed to serialize");
            let deserialized: Permission =
                serde_json::from_str(&serialized).expect("Failed to deserialize");
            assert_eq!(perm, &deserialized);
        }
    }

    #[test]
    fn test_dependency_serialization() {
        let dep = Dependency {
            plugin_id: "other.plugin".to_string(),
            version_req: ">=1.0.0, <2.0.0".to_string(),
            optional: true,
        };

        let serialized = serde_json::to_string(&dep).expect("Failed to serialize");
        let deserialized: Dependency =
            serde_json::from_str(&serialized).expect("Failed to deserialize");

        assert_eq!(deserialized.plugin_id, dep.plugin_id);
        assert_eq!(deserialized.version_req, dep.version_req);
        assert!(deserialized.optional);
    }

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");

        // Plugin directory should be created
        assert!(dir.path().join("plugins").exists());
    }

    #[tokio::test]
    async fn test_plugin_registration() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");
        let plugin = Box::new(TestPlugin::new());

        manager
            .register(plugin)
            .await
            .expect("Failed to register plugin");

        // Check plugin is listed
        let plugins = manager.list();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].0, "test.plugin");
        assert_eq!(plugins[0].2, PluginState::Loaded);
    }

    #[tokio::test]
    async fn test_plugin_get_metadata() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");
        let plugin = Box::new(TestPlugin::new());

        manager
            .register(plugin)
            .await
            .expect("Failed to register plugin");

        let metadata = manager.get_metadata("test.plugin");
        assert!(metadata.is_some());
        assert_eq!(metadata.unwrap().name, "Test Plugin");

        let no_metadata = manager.get_metadata("nonexistent.plugin");
        assert!(no_metadata.is_none());
    }

    #[tokio::test]
    async fn test_plugin_initialization() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");
        let plugin = Box::new(TestPlugin::new());

        manager
            .register(plugin)
            .await
            .expect("Failed to register plugin");
        manager
            .initialize("test.plugin", &config)
            .await
            .expect("Failed to initialize plugin");

        let plugins = manager.list();
        assert_eq!(plugins[0].2, PluginState::Initialized);
    }

    #[tokio::test]
    async fn test_plugin_unload() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");
        let plugin = Box::new(TestPlugin::new());

        manager
            .register(plugin)
            .await
            .expect("Failed to register plugin");
        manager
            .unload("test.plugin")
            .await
            .expect("Failed to unload plugin");

        let plugins = manager.list();
        assert_eq!(plugins[0].2, PluginState::Unloaded);
    }

    #[tokio::test]
    async fn test_plugin_unload_all() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");

        manager.register(Box::new(TestPlugin::new())).await.unwrap();

        manager.unload_all().await.expect("Failed to unload all");

        let plugins = manager.list();
        for (_, _, state) in plugins {
            assert_eq!(state, PluginState::Unloaded);
        }
    }

    #[tokio::test]
    async fn test_has_permission() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");
        let plugin = Box::new(TestPlugin::new());

        manager
            .register(plugin)
            .await
            .expect("Failed to register plugin");

        // TestPlugin has AIEmbeddings permission
        assert!(manager.has_permission("test.plugin", &Permission::AIEmbeddings));
        assert!(!manager.has_permission("test.plugin", &Permission::Notifications));
        assert!(!manager.has_permission("nonexistent.plugin", &Permission::AIEmbeddings));
    }

    #[test]
    fn test_plugin_state_variants() {
        let states = [
            PluginState::Unloaded,
            PluginState::Loaded,
            PluginState::Initialized,
            PluginState::Active,
            PluginState::Error,
        ];

        // Just verify all states exist and are distinct
        for (i, state1) in states.iter().enumerate() {
            for (j, state2) in states.iter().enumerate() {
                if i == j {
                    assert_eq!(state1, state2);
                } else {
                    assert_ne!(state1, state2);
                }
            }
        }
    }

    #[tokio::test]
    async fn test_initialize_nonexistent_plugin() {
        let dir = tempdir().expect("Failed to create temp dir");
        let config = Config {
            data_dir: dir.path().to_path_buf(),
            cache_dir: dir.path().join("cache"),
            ..Config::default()
        };

        let manager = PluginManager::new(&config)
            .await
            .expect("Failed to create manager");

        let result = manager.initialize("nonexistent.plugin", &config).await;
        assert!(result.is_err());
    }
}
