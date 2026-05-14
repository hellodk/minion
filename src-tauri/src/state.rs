//! Application state management

use minion_core::{Config, EventBus, TaskScheduler};
use minion_db::Database;
use minion_presentation::db::PresentationDb;
use minion_files::{DuplicateGroup, FileInfo};
use rand::RngCore;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
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

    /// Mutex flag to prevent concurrent Google Fit syncs
    pub gfit_sync_running: Arc<AtomicBool>,

    /// AES-256-GCM key for encrypting blog platform API keys at rest.
    /// Loaded from (or generated into) `data_dir/blog.key` on first run.
    pub blog_enc_key: [u8; 32],

    /// Presentation database handle.
    pub presentation_db: PresentationDb,

    /// Per-session cancel senders; insert on start, remove on interrupt/complete.
    pub cancel_senders:
        tokio::sync::Mutex<std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,

    /// Presentation generation orchestrator (shared across commands).
    pub orchestrator: Arc<minion_presentation::orchestrator::Orchestrator>,
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

        // Run presentation migrations and set up PresentationDb.
        {
            let conn = db.get().map_err(|e| e.to_string())?;
            minion_presentation::migrations::run(&conn).map_err(|e| e.to_string())?;
        }
        let presentation_db = PresentationDb::new(db.clone());

        // Orchestrator for AI-driven presentation generation.
        let presentations_dir = data_dir.join("presentations");
        let orchestrator = {
            use minion_presentation::{
                orchestrator::Orchestrator,
                router::{PresentationRouter, RouterConfig},
            };
            Arc::new(Orchestrator::new(
                presentation_db.clone(),
                PresentationRouter::new(RouterConfig::default()),
                presentations_dir,
            ))
        };

        // Load or generate the blog API-key encryption key.
        let blog_enc_key = {
            let key_path = data_dir.join("blog.key");
            if key_path.exists() {
                let bytes = std::fs::read(&key_path)?;
                if bytes.len() != 32 {
                    return Err("blog.key is corrupt (expected 32 bytes)".into());
                }
                let mut k = [0u8; 32];
                k.copy_from_slice(&bytes);
                k
            } else {
                let mut k = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut k);
                std::fs::write(&key_path, &k)?;
                k
            }
        };

        Ok(Self {
            config,
            db,
            event_bus,
            task_scheduler,
            data_dir,
            watched_dirs: HashMap::new(),
            scan_tasks: HashMap::new(),
            scan_cache: None,
            gfit_sync_running: Arc::new(AtomicBool::new(false)),
            blog_enc_key,
            presentation_db,
            cancel_senders: tokio::sync::Mutex::new(std::collections::HashMap::new()),
            orchestrator,
        })
    }
}
