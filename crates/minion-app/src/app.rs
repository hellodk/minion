//! Application lifecycle management.

use crate::{Error, Result};

/// Represents the current phase of the application lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppPhase {
    Starting,
    Initializing,
    Ready,
    ShuttingDown,
    Stopped,
}

/// Static information about the application.
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

impl AppInfo {
    pub fn new() -> Self {
        Self {
            name: "MINION",
            version: env!("CARGO_PKG_VERSION"),
            description: "Your personal AI-powered digital assistant",
        }
    }
}

impl Default for AppInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Core application struct managing lifecycle and metadata.
pub struct Application {
    phase: AppPhase,
    info: AppInfo,
}

impl Application {
    /// Create a new application instance in the `Starting` phase.
    pub fn new() -> Self {
        Self {
            phase: AppPhase::Starting,
            info: AppInfo::new(),
        }
    }

    /// Return a reference to the application info.
    pub fn info(&self) -> &AppInfo {
        &self.info
    }

    /// Return the current lifecycle phase.
    pub fn phase(&self) -> AppPhase {
        self.phase
    }

    /// Start the application, transitioning from `Starting` through
    /// `Initializing` to `Ready`.
    pub fn start(&mut self) -> Result<()> {
        if self.phase != AppPhase::Starting {
            return Err(Error::App(format!(
                "Cannot start application from {:?} phase",
                self.phase
            )));
        }

        tracing::info!("Application starting");
        self.phase = AppPhase::Initializing;

        tracing::info!("Initializing subsystems");
        // Subsystem initialization would happen here in a full implementation.

        self.phase = AppPhase::Ready;
        tracing::info!("Application ready");
        Ok(())
    }

    /// Shut down the application, transitioning through `ShuttingDown` to
    /// `Stopped`.
    pub fn shutdown(&mut self) -> Result<()> {
        if self.phase == AppPhase::Stopped {
            return Err(Error::App("Application is already stopped".to_string()));
        }
        if self.phase == AppPhase::ShuttingDown {
            return Err(Error::App(
                "Application is already shutting down".to_string(),
            ));
        }

        tracing::info!("Application shutting down");
        self.phase = AppPhase::ShuttingDown;

        // Cleanup would happen here in a full implementation.

        self.phase = AppPhase::Stopped;
        tracing::info!("Application stopped");
        Ok(())
    }

    /// Returns `true` when the application is in the `Ready` phase.
    pub fn is_ready(&self) -> bool {
        self.phase == AppPhase::Ready
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_info_defaults() {
        let info = AppInfo::new();
        assert_eq!(info.name, "MINION");
        assert!(!info.version.is_empty());
        assert_eq!(
            info.description,
            "Your personal AI-powered digital assistant"
        );
    }

    #[test]
    fn test_app_info_default_trait() {
        let info = AppInfo::default();
        assert_eq!(info.name, "MINION");
    }

    #[test]
    fn test_new_application_starts_in_starting_phase() {
        let app = Application::new();
        assert_eq!(app.phase(), AppPhase::Starting);
        assert!(!app.is_ready());
    }

    #[test]
    fn test_application_default_trait() {
        let app = Application::default();
        assert_eq!(app.phase(), AppPhase::Starting);
    }

    #[test]
    fn test_start_transitions_to_ready() {
        let mut app = Application::new();
        assert!(app.start().is_ok());
        assert_eq!(app.phase(), AppPhase::Ready);
        assert!(app.is_ready());
    }

    #[test]
    fn test_cannot_start_twice() {
        let mut app = Application::new();
        app.start().unwrap();
        let err = app.start().unwrap_err();
        assert!(err.to_string().contains("Cannot start application"));
    }

    #[test]
    fn test_shutdown_from_ready() {
        let mut app = Application::new();
        app.start().unwrap();
        assert!(app.shutdown().is_ok());
        assert_eq!(app.phase(), AppPhase::Stopped);
        assert!(!app.is_ready());
    }

    #[test]
    fn test_shutdown_from_starting() {
        let mut app = Application::new();
        assert!(app.shutdown().is_ok());
        assert_eq!(app.phase(), AppPhase::Stopped);
    }

    #[test]
    fn test_cannot_shutdown_when_stopped() {
        let mut app = Application::new();
        app.shutdown().unwrap();
        let err = app.shutdown().unwrap_err();
        assert!(err.to_string().contains("already stopped"));
    }

    #[test]
    fn test_cannot_start_after_shutdown() {
        let mut app = Application::new();
        app.start().unwrap();
        app.shutdown().unwrap();
        let err = app.start().unwrap_err();
        assert!(err.to_string().contains("Cannot start application"));
    }

    #[test]
    fn test_full_lifecycle() {
        let mut app = Application::new();
        assert_eq!(app.phase(), AppPhase::Starting);

        app.start().unwrap();
        assert_eq!(app.phase(), AppPhase::Ready);
        assert!(app.is_ready());

        assert_eq!(app.info().name, "MINION");

        app.shutdown().unwrap();
        assert_eq!(app.phase(), AppPhase::Stopped);
        assert!(!app.is_ready());
    }

    #[test]
    fn test_app_phase_clone_and_copy() {
        let phase = AppPhase::Ready;
        let cloned = phase.clone();
        let copied = phase;
        assert_eq!(phase, cloned);
        assert_eq!(phase, copied);
    }

    #[test]
    fn test_app_phase_debug() {
        assert_eq!(format!("{:?}", AppPhase::Starting), "Starting");
        assert_eq!(format!("{:?}", AppPhase::Initializing), "Initializing");
        assert_eq!(format!("{:?}", AppPhase::Ready), "Ready");
        assert_eq!(format!("{:?}", AppPhase::ShuttingDown), "ShuttingDown");
        assert_eq!(format!("{:?}", AppPhase::Stopped), "Stopped");
    }
}
