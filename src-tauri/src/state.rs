//! Application state management

use minion_core::{Config, EventBus, TaskScheduler};
use minion_db::Database;
use minion_files::{DuplicateGroup, FileInfo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Scan task status
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ScanStatus {
    Pending,
    Running {
        files_found: usize,
        files_processed: usize,
        bytes_processed: u64,
    },
    Completed {
        total_files: usize,
        total_size: u64,
        duplicates_found: usize,
    },
    Failed(String),
}

/// A directory being monitored
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WatchedDirectory {
    pub id: String,
    pub path: PathBuf,
    pub recursive: bool,
    pub last_scan: Option<chrono::DateTime<chrono::Utc>>,
    pub file_count: u64,
    pub total_size: u64,
}

/// Scan task information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScanTask {
    pub id: String,
    pub directory_id: Option<String>,
    pub path: PathBuf,
    pub status: ScanStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

/// File scan results cache
#[allow(dead_code)]
pub struct ScanCache {
    pub files: Vec<FileInfo>,
    pub duplicates: Vec<DuplicateGroup>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Global application state
#[allow(dead_code)]
pub struct AppState {
    /// Application configuration
    pub config: Config,

    /// Database connection pool
    pub db: Database,

    /// Event bus for inter-module communication
    pub event_bus: Arc<EventBus>,

    /// Background task scheduler
    pub task_scheduler: TaskScheduler,

    /// Data directory path
    pub data_dir: PathBuf,

    /// Watched directories
    pub watched_dirs: HashMap<String, WatchedDirectory>,

    /// Active scan tasks
    pub scan_tasks: HashMap<String, ScanTask>,

    /// Scan results cache
    pub scan_cache: Option<ScanCache>,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Load configuration
        let config = Config::load().unwrap_or_default();

        // Ensure directories exist
        std::fs::create_dir_all(&config.data_dir)?;
        std::fs::create_dir_all(&config.config_dir)?;
        std::fs::create_dir_all(&config.cache_dir)?;

        // Initialize database
        let db_path = config.data_dir.join(&config.database.path);
        let db = Database::new(&db_path, config.database.pool_size)?;
        db.migrate()?;

        // Initialize event bus
        let event_bus = Arc::new(EventBus::new());

        // Initialize task scheduler
        let task_scheduler = TaskScheduler::new(config.workers.background_workers);

        let data_dir = config.data_dir.clone();

        Ok(Self {
            config,
            db,
            event_bus,
            task_scheduler,
            data_dir,
            watched_dirs: HashMap::new(),
            scan_tasks: HashMap::new(),
            scan_cache: None,
        })
    }
}
