//! MINION Core Engine
//!
//! Provides the foundational infrastructure for the MINION application:
//! - Event bus for inter-module communication
//! - Plugin system for extensibility
//! - Configuration management
//! - Background task scheduling

pub mod config;
pub mod error;
pub mod event;
pub mod plugin;
pub mod task;

pub use config::Config;
pub use error::{Error, Result};
pub use event::{Event, EventBus};
pub use plugin::{Plugin, PluginContext, PluginManager, PluginMetadata};
pub use task::{Task, TaskPriority, TaskScheduler};

/// Application version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the core engine with the given configuration
pub async fn init(config: Config) -> Result<CoreEngine> {
    tracing::info!("Initializing MINION Core Engine v{}", VERSION);

    let event_bus = EventBus::new();
    let plugin_manager = PluginManager::new(&config).await?;
    let task_scheduler = TaskScheduler::new(config.workers.background_workers);

    Ok(CoreEngine {
        config,
        event_bus,
        plugin_manager,
        task_scheduler,
    })
}

/// The core engine instance
pub struct CoreEngine {
    pub config: Config,
    pub event_bus: EventBus,
    pub plugin_manager: PluginManager,
    pub task_scheduler: TaskScheduler,
}

impl CoreEngine {
    /// Shutdown the core engine gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        tracing::info!("Shutting down MINION Core Engine");

        // Stop task scheduler
        self.task_scheduler.shutdown().await?;

        // Unload plugins
        self.plugin_manager.unload_all().await?;

        // Shutdown event bus
        self.event_bus.shutdown();

        Ok(())
    }
}
