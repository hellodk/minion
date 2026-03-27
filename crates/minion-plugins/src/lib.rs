//! MINION Plugin SDK
//!
//! SDK for developing MINION plugins. Provides a plugin registry, builder trait,
//! plugin info, and manifest types. This crate is self-contained and does not
//! depend on minion-core.

use std::collections::HashMap;

use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Plugin error: {0}")]
    Plugin(String),

    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    #[error("Plugin not found: {0}")]
    NotFound(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

// ---------------------------------------------------------------------------
// PluginBuilder trait
// ---------------------------------------------------------------------------

/// Factory trait for creating plugin instances.
///
/// Implementations describe a plugin (id, name, version, description) and can
/// produce an instance via [`PluginBuilder::build`].
pub trait PluginBuilder: Send + Sync {
    /// Unique identifier for the plugin this builder creates.
    fn id(&self) -> &str;

    /// Human-readable display name.
    fn name(&self) -> &str;

    /// Semantic version of the plugin.
    fn version(&self) -> Version;

    /// Short description of the plugin.
    fn description(&self) -> &str;

    /// Create a new plugin instance.
    fn build(&self) -> Result<Box<dyn std::any::Any + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// PluginInfo
// ---------------------------------------------------------------------------

/// Summary information about a registered plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique identifier.
    pub id: String,

    /// Human-readable display name.
    pub name: String,

    /// Semantic version.
    pub version: Version,

    /// Short description.
    pub description: String,

    /// Whether the plugin is currently enabled.
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// PluginManifest
// ---------------------------------------------------------------------------

/// Parsed representation of a plugin manifest file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Semantic version.
    pub version: Version,

    /// Author name or email.
    pub author: String,

    /// Short description.
    pub description: String,

    /// License (SPDX identifier).
    pub license: String,

    /// Entry point (e.g. shared library path or module name).
    pub entry_point: String,
}

// ---------------------------------------------------------------------------
// PluginRegistry
// ---------------------------------------------------------------------------

/// A higher-level registry for managing plugin builders.
///
/// `PluginRegistry` stores [`PluginBuilder`] instances keyed by their id and
/// exposes query methods to list and inspect available plugins.
pub struct PluginRegistry {
    builders: HashMap<String, Box<dyn PluginBuilder>>,
}

impl PluginRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            builders: HashMap::new(),
        }
    }

    /// Register a [`PluginBuilder`] under the given `id`.
    ///
    /// Returns an error if a builder with the same id is already registered.
    pub fn register_builder(&mut self, id: &str, builder: Box<dyn PluginBuilder>) -> Result<()> {
        if self.builders.contains_key(id) {
            return Err(Error::AlreadyRegistered(id.to_string()));
        }
        self.builders.insert(id.to_string(), builder);
        Ok(())
    }

    /// List summary information for every registered builder.
    pub fn list_available(&self) -> Vec<PluginInfo> {
        self.builders
            .values()
            .map(|b| PluginInfo {
                id: b.id().to_string(),
                name: b.name().to_string(),
                version: b.version(),
                description: b.description().to_string(),
                enabled: false,
            })
            .collect()
    }

    /// Get summary information for a single plugin by id.
    pub fn get_info(&self, id: &str) -> Option<PluginInfo> {
        self.builders.get(id).map(|b| PluginInfo {
            id: b.id().to_string(),
            name: b.name().to_string(),
            version: b.version(),
            description: b.description().to_string(),
            enabled: false,
        })
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Existing tests -----------------------------------------------------

    #[test]
    fn test_error_plugin() {
        let err = Error::Plugin("test plugin error".to_string());
        assert!(err.to_string().contains("Plugin error"));
        assert!(err.to_string().contains("test plugin error"));
    }

    #[test]
    fn test_result_type() {
        let ok_result: Result<i32> = Ok(42);
        assert_eq!(ok_result.unwrap(), 42);

        let err_result: Result<i32> = Err(Error::Plugin("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_error_debug() {
        let err = Error::Plugin("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Plugin"));
    }

    // -- Helper: stub builder -----------------------------------------------

    struct StubBuilder {
        id: String,
        name: String,
        ver: Version,
        desc: String,
    }

    impl StubBuilder {
        fn new(id: &str, name: &str, ver: Version, desc: &str) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                ver,
                desc: desc.to_string(),
            }
        }
    }

    impl PluginBuilder for StubBuilder {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn version(&self) -> Version {
            self.ver.clone()
        }

        fn description(&self) -> &str {
            &self.desc
        }

        fn build(&self) -> Result<Box<dyn std::any::Any + Send + Sync>> {
            // Return a simple string as the "plugin instance" for testing
            Ok(Box::new(format!("instance-of-{}", self.id)))
        }
    }

    // -- New tests ----------------------------------------------------------

    #[test]
    fn test_registry_creation() {
        let registry = PluginRegistry::new();
        assert!(registry.list_available().is_empty());

        // Default trait should also work
        let registry2 = PluginRegistry::default();
        assert!(registry2.list_available().is_empty());
    }

    #[test]
    fn test_register_and_list_builders() {
        let mut registry = PluginRegistry::new();

        let builder_a = Box::new(StubBuilder::new(
            "plugin.alpha",
            "Alpha Plugin",
            Version::new(1, 0, 0),
            "First plugin",
        ));
        let builder_b = Box::new(StubBuilder::new(
            "plugin.beta",
            "Beta Plugin",
            Version::new(2, 3, 1),
            "Second plugin",
        ));

        registry
            .register_builder("plugin.alpha", builder_a)
            .expect("should register alpha");
        registry
            .register_builder("plugin.beta", builder_b)
            .expect("should register beta");

        let available = registry.list_available();
        assert_eq!(available.len(), 2);

        // Verify both plugins are present (order is not guaranteed by HashMap)
        let ids: Vec<&str> = available.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"plugin.alpha"));
        assert!(ids.contains(&"plugin.beta"));

        // Registering the same id again should fail
        let duplicate = Box::new(StubBuilder::new(
            "plugin.alpha",
            "Alpha Duplicate",
            Version::new(1, 0, 1),
            "Duplicate",
        ));
        let err = registry
            .register_builder("plugin.alpha", duplicate)
            .unwrap_err();
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn test_get_info() {
        let mut registry = PluginRegistry::new();

        let builder = Box::new(StubBuilder::new(
            "plugin.gamma",
            "Gamma Plugin",
            Version::new(3, 2, 1),
            "A gamma plugin",
        ));
        registry
            .register_builder("plugin.gamma", builder)
            .expect("should register gamma");

        // Existing plugin
        let info = registry.get_info("plugin.gamma");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.id, "plugin.gamma");
        assert_eq!(info.name, "Gamma Plugin");
        assert_eq!(info.version, Version::new(3, 2, 1));
        assert_eq!(info.description, "A gamma plugin");
        assert!(!info.enabled);

        // Non-existent plugin
        let missing = registry.get_info("plugin.nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_manifest_serialization() {
        let manifest = PluginManifest {
            id: "com.example.greeter".to_string(),
            name: "Greeter".to_string(),
            version: Version::new(1, 4, 0),
            author: "Jane Doe <jane@example.com>".to_string(),
            description: "A friendly greeter plugin".to_string(),
            license: "MIT".to_string(),
            entry_point: "libgreeter.so".to_string(),
        };

        let json = serde_json::to_string(&manifest).expect("serialize manifest");
        let deserialized: PluginManifest =
            serde_json::from_str(&json).expect("deserialize manifest");

        assert_eq!(deserialized.id, manifest.id);
        assert_eq!(deserialized.name, manifest.name);
        assert_eq!(deserialized.version, manifest.version);
        assert_eq!(deserialized.author, manifest.author);
        assert_eq!(deserialized.description, manifest.description);
        assert_eq!(deserialized.license, manifest.license);
        assert_eq!(deserialized.entry_point, manifest.entry_point);

        // Round-trip through serde_json::Value to verify JSON structure
        let value: serde_json::Value = serde_json::from_str(&json).expect("parse as Value");
        assert_eq!(value["id"], "com.example.greeter");
        assert_eq!(value["version"], "1.4.0");
        assert_eq!(value["entry_point"], "libgreeter.so");
    }

    #[test]
    fn test_plugin_info_creation() {
        let info = PluginInfo {
            id: "plugin.test".to_string(),
            name: "Test Plugin".to_string(),
            version: Version::new(0, 1, 0),
            description: "A test plugin".to_string(),
            enabled: true,
        };

        assert_eq!(info.id, "plugin.test");
        assert_eq!(info.name, "Test Plugin");
        assert_eq!(info.version, Version::new(0, 1, 0));
        assert_eq!(info.description, "A test plugin");
        assert!(info.enabled);

        // PluginInfo should also serialize/deserialize
        let json = serde_json::to_string(&info).expect("serialize PluginInfo");
        let deserialized: PluginInfo = serde_json::from_str(&json).expect("deserialize PluginInfo");
        assert_eq!(deserialized.id, info.id);
        assert_eq!(deserialized.enabled, info.enabled);
    }

    #[test]
    fn test_builder_build() {
        let builder = StubBuilder::new(
            "plugin.delta",
            "Delta",
            Version::new(1, 0, 0),
            "Delta plugin",
        );

        let instance = builder.build().expect("build should succeed");
        let value = instance
            .downcast_ref::<String>()
            .expect("should be a String");
        assert_eq!(value, "instance-of-plugin.delta");
    }

    #[test]
    fn test_error_variants() {
        let e1 = Error::Plugin("generic".to_string());
        assert!(e1.to_string().contains("Plugin error"));

        let e2 = Error::AlreadyRegistered("foo".to_string());
        assert!(e2.to_string().contains("already registered"));
        assert!(e2.to_string().contains("foo"));

        let e3 = Error::NotFound("bar".to_string());
        assert!(e3.to_string().contains("not found"));
        assert!(e3.to_string().contains("bar"));
    }
}
