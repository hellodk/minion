//! Publishing platform integrations

use serde::{Deserialize, Serialize};

use crate::{Error, Platform, Result};

/// Serializable version of `Platform` for configuration storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlatformType {
    WordPress,
    Medium,
    Hashnode,
    DevTo,
    Custom,
}

impl From<Platform> for PlatformType {
    fn from(p: Platform) -> Self {
        match p {
            Platform::WordPress => Self::WordPress,
            Platform::Medium => Self::Medium,
            Platform::Hashnode => Self::Hashnode,
            Platform::DevTo => Self::DevTo,
            Platform::Custom => Self::Custom,
        }
    }
}

impl From<PlatformType> for Platform {
    fn from(pt: PlatformType) -> Self {
        match pt {
            PlatformType::WordPress => Self::WordPress,
            PlatformType::Medium => Self::Medium,
            PlatformType::Hashnode => Self::Hashnode,
            PlatformType::DevTo => Self::DevTo,
            PlatformType::Custom => Self::Custom,
        }
    }
}

/// Configuration for a single publishing platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformConfig {
    pub platform: PlatformType,
    pub api_url: String,
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub enabled: bool,
}

/// Manages platform configurations.
pub struct PlatformManager {
    configs: Vec<PlatformConfig>,
}

impl PlatformManager {
    /// Create a new empty `PlatformManager`.
    pub fn new() -> Self {
        Self {
            configs: Vec::new(),
        }
    }

    /// Add a platform configuration.
    ///
    /// Returns an error if the platform is already configured.
    pub fn add(&mut self, config: PlatformConfig) -> Result<()> {
        if self.is_configured(config.platform) {
            return Err(Error::Platform(format!(
                "platform already configured: {:?}",
                config.platform
            )));
        }
        self.configs.push(config);
        Ok(())
    }

    /// Retrieve the configuration for a platform, if present.
    pub fn get(&self, platform: PlatformType) -> Option<&PlatformConfig> {
        self.configs.iter().find(|c| c.platform == platform)
    }

    /// Return a slice of all configurations.
    pub fn list(&self) -> &[PlatformConfig] {
        &self.configs
    }

    /// Remove the configuration for a platform.
    ///
    /// Returns an error if the platform is not configured.
    pub fn remove(&mut self, platform: PlatformType) -> Result<()> {
        let idx = self
            .configs
            .iter()
            .position(|c| c.platform == platform)
            .ok_or_else(|| Error::Platform(format!("platform not configured: {platform:?}")))?;

        self.configs.remove(idx);
        Ok(())
    }

    /// Check whether a given platform has been configured.
    pub fn is_configured(&self, platform: PlatformType) -> bool {
        self.configs.iter().any(|c| c.platform == platform)
    }
}

impl Default for PlatformManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wordpress_config() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::WordPress,
            api_url: "https://example.com/wp-json/wp/v2".to_string(),
            api_key: Some("key123".to_string()),
            username: Some("admin".to_string()),
            enabled: true,
        }
    }

    fn medium_config() -> PlatformConfig {
        PlatformConfig {
            platform: PlatformType::Medium,
            api_url: "https://api.medium.com/v1".to_string(),
            api_key: Some("medium-token".to_string()),
            username: None,
            enabled: true,
        }
    }

    // ---- From conversions ----

    #[test]
    fn test_platform_type_from_platform() {
        assert_eq!(
            PlatformType::from(Platform::WordPress),
            PlatformType::WordPress
        );
        assert_eq!(PlatformType::from(Platform::Medium), PlatformType::Medium);
        assert_eq!(
            PlatformType::from(Platform::Hashnode),
            PlatformType::Hashnode
        );
        assert_eq!(PlatformType::from(Platform::DevTo), PlatformType::DevTo);
        assert_eq!(PlatformType::from(Platform::Custom), PlatformType::Custom);
    }

    #[test]
    fn test_platform_from_platform_type() {
        assert_eq!(Platform::from(PlatformType::WordPress), Platform::WordPress);
        assert_eq!(Platform::from(PlatformType::Medium), Platform::Medium);
        assert_eq!(Platform::from(PlatformType::Hashnode), Platform::Hashnode);
        assert_eq!(Platform::from(PlatformType::DevTo), Platform::DevTo);
        assert_eq!(Platform::from(PlatformType::Custom), Platform::Custom);
    }

    #[test]
    fn test_roundtrip_platform_type() {
        let original = Platform::Hashnode;
        let pt: PlatformType = original.into();
        let back: Platform = pt.into();
        assert_eq!(original, back);
    }

    // ---- PlatformManager tests ----

    #[test]
    fn test_new_empty() {
        let mgr = PlatformManager::new();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_add_and_get() {
        let mut mgr = PlatformManager::new();
        mgr.add(wordpress_config()).unwrap();

        let cfg = mgr.get(PlatformType::WordPress).unwrap();
        assert_eq!(cfg.api_url, "https://example.com/wp-json/wp/v2");
        assert_eq!(cfg.api_key.as_deref(), Some("key123"));
        assert!(cfg.enabled);
    }

    #[test]
    fn test_add_duplicate_error() {
        let mut mgr = PlatformManager::new();
        mgr.add(wordpress_config()).unwrap();
        let err = mgr.add(wordpress_config()).unwrap_err();
        assert!(err.to_string().contains("already configured"));
    }

    #[test]
    fn test_get_missing() {
        let mgr = PlatformManager::new();
        assert!(mgr.get(PlatformType::DevTo).is_none());
    }

    #[test]
    fn test_list() {
        let mut mgr = PlatformManager::new();
        mgr.add(wordpress_config()).unwrap();
        mgr.add(medium_config()).unwrap();
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn test_remove() {
        let mut mgr = PlatformManager::new();
        mgr.add(wordpress_config()).unwrap();
        mgr.remove(PlatformType::WordPress).unwrap();
        assert!(!mgr.is_configured(PlatformType::WordPress));
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_remove_missing_error() {
        let mut mgr = PlatformManager::new();
        let err = mgr.remove(PlatformType::Medium).unwrap_err();
        assert!(err.to_string().contains("not configured"));
    }

    #[test]
    fn test_is_configured() {
        let mut mgr = PlatformManager::new();
        assert!(!mgr.is_configured(PlatformType::WordPress));
        mgr.add(wordpress_config()).unwrap();
        assert!(mgr.is_configured(PlatformType::WordPress));
        assert!(!mgr.is_configured(PlatformType::Medium));
    }

    #[test]
    fn test_default_impl() {
        let mgr = PlatformManager::default();
        assert!(mgr.list().is_empty());
    }

    #[test]
    fn test_platform_type_serde() {
        let json = serde_json::to_string(&PlatformType::DevTo).unwrap();
        let deserialized: PlatformType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, PlatformType::DevTo);
    }

    #[test]
    fn test_platform_config_serde() {
        let cfg = wordpress_config();
        let json = serde_json::to_string(&cfg).unwrap();
        let deserialized: PlatformConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.platform, PlatformType::WordPress);
        assert_eq!(deserialized.api_url, cfg.api_url);
        assert_eq!(deserialized.api_key, cfg.api_key);
        assert_eq!(deserialized.username, cfg.username);
        assert_eq!(deserialized.enabled, cfg.enabled);
    }
}
