# MINION Plugin Interface Specification

## Overview

MINION's plugin system allows third-party developers to extend functionality while maintaining security and stability. Plugins are sandboxed Rust dynamic libraries (`.so`/`.dll`/`.dylib`) or WASM modules.

---

## Plugin Types

### 1. Native Plugins (Rust)

High-performance plugins compiled to native code.

```rust
// plugin_sdk/src/lib.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin trait that all native plugins must implement
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Returns plugin metadata
    fn metadata(&self) -> PluginMetadata;
    
    /// Initialize the plugin with context
    async fn initialize(&mut self, ctx: PluginContext) -> Result<(), PluginError>;
    
    /// Shutdown gracefully
    async fn shutdown(&mut self) -> Result<(), PluginError>;
    
    /// List capabilities this plugin provides
    fn capabilities(&self) -> Vec<Capability>;
    
    /// Handle events from the event bus
    async fn handle_event(&self, event: &Event) -> Result<Option<Event>, PluginError>;
    
    /// Handle commands directed at this plugin
    async fn handle_command(&self, cmd: Command) -> Result<Value, PluginError>;
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Unique identifier (reverse domain notation recommended)
    pub id: String,
    
    /// Display name
    pub name: String,
    
    /// Semantic version
    pub version: semver::Version,
    
    /// Author or organization
    pub author: String,
    
    /// Short description
    pub description: String,
    
    /// Plugin homepage URL
    pub homepage: Option<String>,
    
    /// License identifier (SPDX)
    pub license: String,
    
    /// Required permissions
    pub permissions: Vec<Permission>,
    
    /// Plugin dependencies
    pub dependencies: Vec<Dependency>,
    
    /// Minimum MINION version required
    pub min_minion_version: semver::Version,
    
    /// Supported platforms
    pub platforms: Vec<Platform>,
}

/// Plugin dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub plugin_id: String,
    pub version_req: semver::VersionReq,
    pub optional: bool,
}

/// Supported platforms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Platform {
    Linux,
    Windows,
    MacOS,
    All,
}
```

### 2. WASM Plugins

Sandboxed WebAssembly plugins for maximum isolation.

```rust
/// WASM plugin interface (via wit-bindgen)
// plugin.wit

interface plugin {
    record metadata {
        id: string,
        name: string,
        version: string,
        author: string,
        permissions: list<string>,
    }
    
    // Required exports
    get-metadata: func() -> metadata
    initialize: func(config: string) -> result<_, string>
    shutdown: func() -> result<_, string>
    handle-event: func(event-type: string, payload: string) -> result<option<string>, string>
    handle-command: func(command: string, args: string) -> result<string, string>
}
```

---

## Permission Model

```rust
/// Permissions that plugins can request
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // ================================
    // FILE SYSTEM
    // ================================
    
    /// Read files matching pattern
    FileRead {
        patterns: Vec<String>,  // Glob patterns
    },
    
    /// Write files matching pattern
    FileWrite {
        patterns: Vec<String>,
    },
    
    /// Watch file system for changes
    FileWatch {
        patterns: Vec<String>,
    },
    
    // ================================
    // NETWORK
    // ================================
    
    /// HTTP(S) access to specific hosts
    NetworkHttp {
        hosts: Vec<String>,  // ["api.example.com", "*.google.com"]
    },
    
    /// WebSocket connections
    NetworkWebSocket {
        hosts: Vec<String>,
    },
    
    // ================================
    // SYSTEM
    // ================================
    
    /// Spawn child processes
    ProcessSpawn {
        allowed_commands: Vec<String>,  // ["ffmpeg", "convert"]
    },
    
    /// Access clipboard
    Clipboard {
        read: bool,
        write: bool,
    },
    
    /// Send system notifications
    Notifications,
    
    /// Access system information
    SystemInfo,
    
    // ================================
    // DATA
    // ================================
    
    /// Read from database tables
    DatabaseRead {
        tables: Vec<String>,
    },
    
    /// Write to database tables
    DatabaseWrite {
        tables: Vec<String>,
    },
    
    /// Access credential vault
    CredentialAccess {
        services: Vec<String>,
    },
    
    /// Store plugin-specific data
    PluginStorage {
        max_size_bytes: u64,
    },
    
    // ================================
    // AI
    // ================================
    
    /// Access AI model inference
    AIModel {
        models: Vec<String>,  // ["llama3", "gpt-4", "*"]
    },
    
    /// Generate embeddings
    AIEmbeddings,
    
    /// Access vector store
    VectorStore {
        namespaces: Vec<String>,
    },
    
    // ================================
    // MINION MODULES
    // ================================
    
    /// Access other modules' APIs
    ModuleAccess {
        modules: Vec<String>,
        commands: Vec<String>,
    },
    
    /// Subscribe to events
    EventSubscribe {
        event_types: Vec<String>,
    },
    
    /// Publish events
    EventPublish {
        event_types: Vec<String>,
    },
    
    // ================================
    // UI
    // ================================
    
    /// Register UI components
    UIRegister {
        locations: Vec<UILocation>,
    },
    
    /// Register keyboard shortcuts
    KeyboardShortcuts {
        shortcuts: Vec<String>,
    },
    
    /// Add to command palette
    CommandPalette,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UILocation {
    Sidebar,
    Toolbar,
    StatusBar,
    SettingsPage,
    Dashboard,
    ModuleTab,
    ContextMenu,
}
```

---

## Plugin Context

```rust
/// Context provided to plugins during initialization
pub struct PluginContext {
    /// Plugin's unique data directory
    pub data_dir: PathBuf,
    
    /// Plugin's cache directory
    pub cache_dir: PathBuf,
    
    /// Plugin configuration
    pub config: PluginConfig,
    
    /// Logger instance
    pub logger: Logger,
    
    /// Event bus sender
    pub event_sender: mpsc::Sender<Event>,
    
    /// API client for core services
    pub api: PluginAPI,
}

/// API available to plugins
pub struct PluginAPI {
    file_api: FileAPI,
    database_api: DatabaseAPI,
    ai_api: AIAPI,
    credential_api: CredentialAPI,
    module_api: ModuleAPI,
    ui_api: UIAPI,
}

impl PluginAPI {
    // File operations (permission checked)
    pub async fn read_file(&self, path: &Path) -> Result<Vec<u8>, PluginError>;
    pub async fn write_file(&self, path: &Path, data: &[u8]) -> Result<(), PluginError>;
    pub async fn list_directory(&self, path: &Path) -> Result<Vec<DirEntry>, PluginError>;
    
    // Database operations (permission checked)
    pub async fn db_query(&self, query: &str, params: &[Value]) -> Result<Vec<Row>, PluginError>;
    pub async fn db_execute(&self, query: &str, params: &[Value]) -> Result<u64, PluginError>;
    
    // AI operations (permission checked)
    pub async fn ai_complete(&self, prompt: &str, options: &AIOptions) -> Result<String, PluginError>;
    pub async fn ai_embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, PluginError>;
    
    // Credential operations (permission checked)
    pub async fn get_credential(&self, service: &str) -> Result<Credential, PluginError>;
    pub async fn store_credential(&self, service: &str, cred: Credential) -> Result<(), PluginError>;
    
    // Module communication (permission checked)
    pub async fn call_module(&self, module: &str, command: &str, args: Value) -> Result<Value, PluginError>;
    
    // UI registration (permission checked)
    pub fn register_ui_component(&self, location: UILocation, component: UIComponent) -> Result<(), PluginError>;
}
```

---

## Event System

```rust
/// Events that plugins can subscribe to and emit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event type identifier
    pub event_type: String,
    
    /// Source plugin/module ID
    pub source: String,
    
    /// Event payload
    pub payload: Value,
    
    /// Event timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Correlation ID for tracking
    pub correlation_id: Option<String>,
}

/// Standard event types
pub mod event_types {
    // System events
    pub const SYSTEM_STARTUP: &str = "system.startup";
    pub const SYSTEM_SHUTDOWN: &str = "system.shutdown";
    pub const MODULE_LOADED: &str = "module.loaded";
    pub const MODULE_UNLOADED: &str = "module.unloaded";
    
    // File events
    pub const FILE_CREATED: &str = "file.created";
    pub const FILE_MODIFIED: &str = "file.modified";
    pub const FILE_DELETED: &str = "file.deleted";
    
    // Content events
    pub const CONTENT_CREATED: &str = "content.created";
    pub const CONTENT_UPDATED: &str = "content.updated";
    pub const CONTENT_PUBLISHED: &str = "content.published";
    
    // AI events
    pub const AI_REQUEST_STARTED: &str = "ai.request.started";
    pub const AI_REQUEST_COMPLETED: &str = "ai.request.completed";
    
    // User events
    pub const USER_ACTION: &str = "user.action";
}
```

---

## Plugin Lifecycle

```
                    ┌──────────────┐
                    │   INSTALL    │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
          ┌────────│   VALIDATE   │────────┐
          │        └──────┬───────┘        │
          │ Invalid       │ Valid          │
          ▼               ▼                │
   ┌──────────────┐ ┌──────────────┐       │
   │    ERROR     │ │    LOAD      │       │
   └──────────────┘ └──────┬───────┘       │
                           │               │
                           ▼               │
                    ┌──────────────┐       │
                    │  INITIALIZE  │       │
                    └──────┬───────┘       │
                           │               │
           ┌───────────────┼───────────────┤
           │               │               │
           ▼               ▼               ▼
    ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
    │    ACTIVE    │ │   DISABLED   │ │    ERROR     │
    └──────┬───────┘ └──────────────┘ └──────────────┘
           │
           │ User disables or uninstalls
           ▼
    ┌──────────────┐
    │   SHUTDOWN   │
    └──────┬───────┘
           │
           ▼
    ┌──────────────┐
    │   UNLOAD     │
    └──────────────┘
```

---

## Plugin Manifest

Each plugin must include a `plugin.toml` manifest:

```toml
[plugin]
id = "com.example.my-plugin"
name = "My Awesome Plugin"
version = "1.0.0"
author = "Your Name <your@email.com>"
description = "A plugin that does awesome things"
homepage = "https://github.com/yourname/my-plugin"
license = "MIT"
min_minion_version = "0.1.0"

[plugin.platforms]
linux = true
windows = true
macos = true

[permissions]
# File access
[[permissions.file_read]]
patterns = ["*.md", "*.txt"]

[[permissions.file_write]]
patterns = ["~/.minion/plugins/com.example.my-plugin/*"]

# Network access
[[permissions.network_http]]
hosts = ["api.example.com"]

# Database access
[[permissions.database_read]]
tables = ["blog_posts", "book_annotations"]

# AI access
[[permissions.ai_model]]
models = ["*"]

# Module access
[[permissions.module_access]]
modules = ["reader", "blog"]
commands = ["*"]

# Event access
[[permissions.event_subscribe]]
event_types = ["content.*", "file.*"]

[[permissions.event_publish]]
event_types = ["com.example.my-plugin.*"]

# UI registration
[[permissions.ui_register]]
locations = ["sidebar", "settings_page"]

[dependencies]
# Optional dependencies on other plugins
# "com.other.plugin" = { version = ">=1.0.0", optional = true }

[build]
# Build configuration
type = "native"  # or "wasm"
entry = "src/lib.rs"
```

---

## Plugin SDK Usage Example

```rust
// my_plugin/src/lib.rs

use minion_plugin_sdk::prelude::*;

pub struct MyPlugin {
    ctx: Option<PluginContext>,
    config: MyPluginConfig,
}

#[derive(Debug, Deserialize)]
struct MyPluginConfig {
    enabled: bool,
    custom_setting: String,
}

impl Default for MyPlugin {
    fn default() -> Self {
        Self {
            ctx: None,
            config: MyPluginConfig {
                enabled: true,
                custom_setting: String::new(),
            },
        }
    }
}

#[async_trait]
impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "com.example.my-plugin".to_string(),
            name: "My Plugin".to_string(),
            version: semver::Version::new(1, 0, 0),
            author: "Your Name".to_string(),
            description: "Does awesome things".to_string(),
            homepage: Some("https://github.com/yourname/my-plugin".to_string()),
            license: "MIT".to_string(),
            permissions: vec![
                Permission::DatabaseRead { tables: vec!["blog_posts".to_string()] },
                Permission::AIModel { models: vec!["*".to_string()] },
            ],
            dependencies: vec![],
            min_minion_version: semver::Version::new(0, 1, 0),
            platforms: vec![Platform::All],
        }
    }
    
    async fn initialize(&mut self, ctx: PluginContext) -> Result<(), PluginError> {
        ctx.logger.info("Initializing My Plugin");
        
        // Load configuration
        self.config = ctx.config.parse()?;
        
        // Store context for later use
        self.ctx = Some(ctx);
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        if let Some(ctx) = &self.ctx {
            ctx.logger.info("Shutting down My Plugin");
        }
        Ok(())
    }
    
    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::new("custom_action", "Performs custom action"),
        ]
    }
    
    async fn handle_event(&self, event: &Event) -> Result<Option<Event>, PluginError> {
        match event.event_type.as_str() {
            "content.created" => {
                // React to content creation
                let ctx = self.ctx.as_ref().ok_or(PluginError::NotInitialized)?;
                ctx.logger.debug(&format!("Content created: {:?}", event.payload));
                Ok(None)
            }
            _ => Ok(None),
        }
    }
    
    async fn handle_command(&self, cmd: Command) -> Result<Value, PluginError> {
        let ctx = self.ctx.as_ref().ok_or(PluginError::NotInitialized)?;
        
        match cmd.name.as_str() {
            "custom_action" => {
                // Use the AI API
                let result = ctx.api.ai_complete(
                    "Generate a creative title",
                    &AIOptions::default(),
                ).await?;
                
                Ok(serde_json::json!({ "result": result }))
            }
            _ => Err(PluginError::UnknownCommand(cmd.name)),
        }
    }
}

// Required export for native plugins
minion_plugin_sdk::export_plugin!(MyPlugin);
```

---

## Security Sandbox

### Native Plugin Isolation

```rust
/// Sandbox configuration for native plugins
pub struct SandboxConfig {
    /// Memory limit in bytes
    pub memory_limit: u64,
    
    /// CPU time limit per operation
    pub cpu_time_limit: Duration,
    
    /// Maximum open file descriptors
    pub max_fds: u32,
    
    /// Allowed system calls (seccomp filter)
    pub allowed_syscalls: Vec<String>,
    
    /// Chroot directory (if any)
    pub chroot: Option<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            memory_limit: 256 * 1024 * 1024,  // 256MB
            cpu_time_limit: Duration::from_secs(30),
            max_fds: 64,
            allowed_syscalls: default_allowed_syscalls(),
            chroot: None,
        }
    }
}
```

### WASM Plugin Isolation

- Memory-isolated WASM runtime (wasmtime)
- No direct file system access
- No network access (except through API)
- Fuel-based execution limits
- Capability-based security

---

## UI Component Registration

```typescript
// Plugin can register UI components

interface UIComponent {
    type: 'sidebar' | 'toolbar' | 'settings' | 'dashboard-widget' | 'context-menu';
    
    // For sidebar/toolbar
    icon?: string;          // Icon name or SVG
    label: string;          // Display label
    
    // For settings page
    settingsSchema?: JSONSchema;
    
    // For dashboard widget
    widget?: {
        minWidth: number;
        minHeight: number;
        defaultWidth: number;
        defaultHeight: number;
        component: string;   // Web component name
    };
    
    // For context menu
    contextMenu?: {
        fileTypes?: string[];  // ["image/*", "video/*"]
        modules?: string[];    // ["media", "files"]
        label: string;
        action: string;
    };
}

// Example: Register sidebar item
ctx.api.register_ui_component(UILocation::Sidebar, UIComponent {
    type: "sidebar",
    icon: "puzzle",
    label: "My Plugin",
    ..Default::default()
});
```

---

## Plugin Distribution

### Plugin Package Structure

```
my-plugin-1.0.0.mpkg/
├── plugin.toml           # Manifest
├── lib/
│   ├── linux-x64/
│   │   └── libmy_plugin.so
│   ├── windows-x64/
│   │   └── my_plugin.dll
│   └── macos-x64/
│       └── libmy_plugin.dylib
├── assets/
│   ├── icon.svg
│   └── styles.css
├── ui/
│   └── components.js     # Optional web components
├── LICENSE
└── README.md
```

### Plugin Signing

```bash
# Sign plugin package
minion-cli plugin sign my-plugin-1.0.0.mpkg --key ~/.minion/developer.key

# Verify signature
minion-cli plugin verify my-plugin-1.0.0.mpkg
```

### Plugin Repository

```toml
# ~/.minion/config/repositories.toml

[[repositories]]
name = "official"
url = "https://plugins.minion.dev"
trusted = true

[[repositories]]
name = "community"
url = "https://community-plugins.minion.dev"
trusted = false  # User must approve each install
```
