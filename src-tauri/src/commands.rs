//! Tauri IPC commands

use crate::state::{AppState, ScanCache, ScanStatus, ScanTask, WatchedDirectory};
use chrono::Utc;
use minion_files::{AnalyticsCalculator, DuplicateFinder, ScanConfig, Scanner};
use minion_reader::formats::{parse_epub, parse_pdf};
use minion_reader::BookFormat;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// ============================================================================
// Response types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub data_dir: String,
}

#[derive(Debug, Serialize)]
pub struct ModuleInfo {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ScanProgress {
    pub task_id: String,
    pub status: String,
    pub files_scanned: u64,
    pub total_files: Option<u64>,
    pub progress_percent: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DuplicateGroupResponse {
    pub id: String,
    pub match_type: String,
    pub match_label: String,
    pub file_count: usize,
    pub total_size: u64,
    pub wasted_space: u64,
    pub files: Vec<FileInfoResponse>,
    pub hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfoResponse {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: String,
    pub extension: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct DuplicateFilter {
    pub match_type: Option<String>,
    pub min_size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StorageAnalytics {
    pub total_files: u64,
    pub total_size: u64,
    pub by_extension: Vec<ExtensionStats>,
    pub duplicates_found: u64,
    pub duplicate_size: u64,
}

#[derive(Debug, Serialize)]
pub struct ExtensionStats {
    pub extension: String,
    pub count: u64,
    pub size: u64,
}

// ============================================================================
// Finance response types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FinanceAccountResponse {
    pub id: String,
    pub name: String,
    pub account_type: String,
    pub institution: Option<String>,
    pub balance: f64,
    pub currency: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinanceTransactionResponse {
    pub id: String,
    pub account_id: String,
    pub transaction_type: String,
    pub amount: f64,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    pub date: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinancialSummaryResponse {
    pub net_worth: f64,
    pub total_assets: f64,
    pub total_liabilities: f64,
    pub monthly_income: f64,
    pub monthly_expenses: f64,
    pub savings_rate: f64,
    pub account_count: u64,
    pub transaction_count: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvImportResult {
    pub total_rows: usize,
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InvestmentResponse {
    pub id: String,
    pub name: String,
    pub investment_type: String,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    pub purchase_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub purchase_date: String,
    pub gain_loss: f64,
    pub gain_loss_pct: f64,
    pub current_value: f64,
}

#[derive(Debug, Serialize)]
pub struct PortfolioSummary {
    pub total_invested: f64,
    pub current_value: f64,
    pub total_gain_loss: f64,
    pub total_gain_loss_pct: f64,
    pub by_type: Vec<TypeAllocation>,
}

#[derive(Debug, Serialize)]
pub struct TypeAllocation {
    pub investment_type: String,
    pub value: f64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct CibilResponse {
    pub score: i32,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CsvMappingRequest {
    pub date_column: Option<String>,
    pub description_column: Option<String>,
    pub amount_column: Option<String>,
    pub debit_column: Option<String>,
    pub credit_column: Option<String>,
    pub balance_column: Option<String>,
    pub date_format: Option<String>,
}

// ============================================================================
// Fitness response types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct FitnessHabitResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub frequency: String,
    pub created_at: String,
    pub completed_today: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FitnessMetricResponse {
    pub id: String,
    pub date: String,
    pub weight_kg: Option<f64>,
    pub body_fat_pct: Option<f64>,
    pub steps: Option<i64>,
    pub heart_rate_avg: Option<i64>,
    pub sleep_hours: Option<f64>,
    pub sleep_quality: Option<i64>,
    pub water_ml: Option<i64>,
    pub calories_in: Option<i64>,
    pub notes: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct FitnessDashboard {
    pub total_habits: u64,
    pub habits_completed_today: u64,
    pub current_streak: u64,
    pub latest_weight_kg: Option<f64>,
    pub avg_steps_7d: Option<f64>,
    pub avg_sleep_7d: Option<f64>,
    pub total_water_today: Option<i64>,
    pub workouts_this_week: u64,
}

// ============================================================================
// Collection response types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub book_count: i64,
    pub created_at: String,
}

// ============================================================================
// Reader (enhanced) response types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ReaderBookResponse {
    pub id: String,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub file_path: String,
    pub format: Option<String>,
    pub cover_path: Option<String>,
    pub pages: Option<i64>,
    pub current_position: Option<String>,
    pub progress: f64,
    pub rating: Option<i64>,
    pub favorite: bool,
    pub tags: Option<String>,
    pub added_at: String,
    pub last_read_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReaderAnnotationResponse {
    pub id: String,
    pub book_id: String,
    pub annotation_type: String,
    pub chapter_index: Option<i64>,
    pub start_pos: Option<i64>,
    pub end_pos: Option<i64>,
    pub text: Option<String>,
    pub note: Option<String>,
    pub color: String,
    pub created_at: String,
    pub updated_at: String,
}

// ============================================================================
// System commands
// ============================================================================

#[tauri::command]
pub async fn get_system_info(state: State<'_, AppStateHandle>) -> Result<SystemInfo, String> {
    let state = state.read().await;

    Ok(SystemInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        data_dir: state.data_dir.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn get_config(
    state: State<'_, AppStateHandle>,
    key: Option<String>,
) -> Result<serde_json::Value, String> {
    let state = state.read().await;

    match key {
        Some(k) => match k.as_str() {
            "theme" => Ok(serde_json::json!(state.config.ui.theme)),
            "animations" => Ok(serde_json::json!(state.config.ui.animations)),
            _ => Ok(serde_json::Value::Null),
        },
        None => serde_json::to_value(&state.config).map_err(|e| e.to_string()),
    }
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, AppStateHandle>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    let mut state = state.write().await;

    match key.as_str() {
        "theme" => {
            if let Some(theme) = value.as_str() {
                state.config.ui.theme = theme.to_string();
            }
        }
        "animations" => {
            if let Some(enabled) = value.as_bool() {
                state.config.ui.animations = enabled;
            }
        }
        _ => return Err(format!("Unknown config key: {}", key)),
    }

    state.config.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ============================================================================
// Module commands
// ============================================================================

#[tauri::command]
pub async fn list_modules(_state: State<'_, AppStateHandle>) -> Result<Vec<ModuleInfo>, String> {
    Ok(vec![
        ModuleInfo {
            id: "files".to_string(),
            name: "File Intelligence".to_string(),
            enabled: true,
            status: "active".to_string(),
        },
        ModuleInfo {
            id: "reader".to_string(),
            name: "Book Reader".to_string(),
            enabled: true,
            status: "active".to_string(),
        },
        ModuleInfo {
            id: "finance".to_string(),
            name: "Finance Intelligence".to_string(),
            enabled: true,
            status: "inactive".to_string(),
        },
        ModuleInfo {
            id: "fitness".to_string(),
            name: "Fitness & Wellness".to_string(),
            enabled: true,
            status: "inactive".to_string(),
        },
        ModuleInfo {
            id: "media".to_string(),
            name: "Media Intelligence".to_string(),
            enabled: false,
            status: "inactive".to_string(),
        },
        ModuleInfo {
            id: "blog".to_string(),
            name: "Blog AI Engine".to_string(),
            enabled: false,
            status: "inactive".to_string(),
        },
    ])
}

#[tauri::command]
pub async fn get_module_status(
    _state: State<'_, AppStateHandle>,
    module_id: String,
) -> Result<ModuleInfo, String> {
    Ok(ModuleInfo {
        id: module_id.clone(),
        name: format!("{} Module", module_id),
        enabled: true,
        status: "active".to_string(),
    })
}

// ============================================================================
// File Intelligence commands
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AddDirectoryRequest {
    pub path: String,
    pub recursive: Option<bool>,
}

#[tauri::command]
pub async fn files_add_directory(
    state: State<'_, AppStateHandle>,
    config: AddDirectoryRequest,
) -> Result<String, String> {
    let path = std::path::Path::new(&config.path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", config.path));
    }
    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", config.path));
    }

    let dir_id = uuid::Uuid::new_v4().to_string();

    let mut state = state.write().await;
    state.watched_dirs.insert(
        dir_id.clone(),
        WatchedDirectory {
            id: dir_id.clone(),
            path: path.to_path_buf(),
            recursive: config.recursive.unwrap_or(true),
            last_scan: None,
            file_count: 0,
            total_size: 0,
        },
    );

    tracing::info!("Added scan directory: {} ({})", config.path, dir_id);
    Ok(dir_id)
}

#[tauri::command]
pub async fn files_start_scan(
    state: State<'_, AppStateHandle>,
    path: String,
    exclude_patterns: Option<Vec<String>>,
) -> Result<ScanProgress, String> {
    let task_id = uuid::Uuid::new_v4().to_string();
    let scan_path = PathBuf::from(&path);

    if !scan_path.exists() {
        return Err(format!("Path does not exist: {}", path));
    }
    if !scan_path.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    tracing::info!("Starting file scan of: {}", path);

    {
        let mut state_guard = state.write().await;
        state_guard.scan_tasks.insert(
            task_id.clone(),
            ScanTask {
                id: task_id.clone(),
                directory_id: None,
                path: scan_path.clone(),
                status: ScanStatus::Running {
                    files_found: 0,
                    files_processed: 0,
                    bytes_processed: 0,
                },
                started_at: Utc::now(),
            },
        );
    }

    let state_clone = state.inner().clone();
    let task_id_clone = task_id.clone();

    tokio::spawn(async move {
        // Phase 1: Quick scan WITHOUT hashing (fast metadata only)
        let scan_config = ScanConfig {
            root: scan_path,
            recursive: true,
            compute_hashes: false, // Skip hashing in initial scan for speed
            exclude_patterns: exclude_patterns.unwrap_or_default(),
            ..Default::default()
        };

        let scanner = Scanner::new(scan_config);

        let scanner_files_found = scanner.files_found();
        let scanner_files_processed = scanner.files_processed();
        let scanner_bytes_processed = scanner.bytes_processed();
        let scan_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let scan_done_writer = scan_done.clone();

        // Progress updater runs on tokio async runtime - NOT blocked by the scan
        let progress_state = state_clone.clone();
        let progress_task_id = task_id_clone.clone();
        let progress_files_found = scanner_files_found.clone();
        let progress_files_processed = scanner_files_processed.clone();
        let progress_bytes_processed = scanner_bytes_processed.clone();
        let progress_done = scan_done.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                if progress_done.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let found = progress_files_found.load(std::sync::atomic::Ordering::Relaxed);
                let processed = progress_files_processed.load(std::sync::atomic::Ordering::Relaxed);
                let bytes =
                    progress_bytes_processed.load(std::sync::atomic::Ordering::Relaxed) as u64;

                let mut guard = progress_state.write().await;
                if let Some(task) = guard.scan_tasks.get_mut(&progress_task_id) {
                    if matches!(task.status, ScanStatus::Running { .. }) {
                        task.status = ScanStatus::Running {
                            files_found: found,
                            files_processed: processed,
                            bytes_processed: bytes,
                        };
                    }
                }
            }
        });

        // Run the blocking scan on a dedicated thread pool - this is the key fix!
        // spawn_blocking moves the CPU-heavy work off the tokio runtime so the
        // progress updater above can keep running.
        let scan_result = tokio::task::spawn_blocking(move || scanner.scan())
            .await
            .unwrap_or_else(|e| {
                Err(minion_files::Error::Scan(format!(
                    "Scan task panicked: {}",
                    e
                )))
            });

        scan_done_writer.store(true, std::sync::atomic::Ordering::Relaxed);

        match scan_result {
            Ok(result) => {
                tracing::info!(
                    "Scan complete: {} files, {} bytes",
                    result.files.len(),
                    result.total_size
                );

                // Also run duplicate finding on blocking pool
                // Phase 2: Find size candidates, then hash only those
                let mut files_result = result.files;
                let total_size = result.total_size;

                let dupes_result = tokio::task::spawn_blocking(move || {
                    use std::collections::HashMap;

                    // Group by size to find potential duplicates
                    let mut size_groups: HashMap<u64, Vec<usize>> = HashMap::new();
                    for (i, f) in files_result.iter().enumerate() {
                        if f.size >= 1024 {
                            size_groups.entry(f.size).or_default().push(i);
                        }
                    }

                    // Only hash files that share a size with another file
                    let mut needs_hash = 0usize;
                    for indices in size_groups.values() {
                        if indices.len() > 1 {
                            needs_hash += indices.len();
                            for &idx in indices {
                                if files_result[idx].sha256.is_none() {
                                    if let Ok(hash) =
                                        minion_files::hash::compute_sha256(&files_result[idx].path)
                                    {
                                        files_result[idx].sha256 = Some(hash);
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!(
                        "Hashed {} of {} files (size-candidate optimization)",
                        needs_hash,
                        files_result.len()
                    );

                    let finder = DuplicateFinder::default();
                    let dupes = finder.find(&files_result);
                    (files_result, dupes)
                })
                .await
                .unwrap_or_else(|_| (vec![], vec![]));

                let (files_final, duplicates) = dupes_result;
                let duplicates_count = duplicates.len();

                tracing::info!("Found {} duplicate groups", duplicates_count);

                let mut state_guard = state_clone.write().await;

                if let Some(task) = state_guard.scan_tasks.get_mut(&task_id_clone) {
                    task.status = ScanStatus::Completed {
                        total_files: files_final.len(),
                        total_size,
                        duplicates_found: duplicates_count,
                    };
                }

                state_guard.scan_cache = Some(ScanCache {
                    files: files_final,
                    duplicates,
                    last_updated: Utc::now(),
                });
            }
            Err(e) => {
                tracing::error!("Scan failed: {}", e);
                let mut state_guard = state_clone.write().await;
                if let Some(task) = state_guard.scan_tasks.get_mut(&task_id_clone) {
                    task.status = ScanStatus::Failed(e.to_string());
                }
            }
        }
    });

    Ok(ScanProgress {
        task_id,
        status: "running".to_string(),
        files_scanned: 0,
        total_files: None,
        progress_percent: 0.0,
    })
}

#[tauri::command]
pub async fn files_start_multi_scan(
    state: State<'_, AppStateHandle>,
    paths: Vec<String>,
    exclude_patterns: Option<Vec<String>>,
) -> Result<ScanProgress, String> {
    if paths.is_empty() {
        return Err("No directories specified".to_string());
    }

    // Validate all paths
    for p in &paths {
        let path = std::path::Path::new(p);
        if !path.exists() {
            return Err(format!("Path does not exist: {}", p));
        }
        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", p));
        }
    }

    let task_id = uuid::Uuid::new_v4().to_string();
    let label = if paths.len() == 1 {
        paths[0].clone()
    } else {
        format!("{} directories", paths.len())
    };

    tracing::info!("Starting multi-directory scan: {:?}", paths);

    {
        let mut state_guard = state.write().await;
        state_guard.scan_tasks.insert(
            task_id.clone(),
            ScanTask {
                id: task_id.clone(),
                directory_id: None,
                path: PathBuf::from(&label),
                status: ScanStatus::Running {
                    files_found: 0,
                    files_processed: 0,
                    bytes_processed: 0,
                },
                started_at: Utc::now(),
            },
        );
    }

    let state_clone = state.inner().clone();
    let task_id_clone = task_id.clone();
    let exclude_pats = exclude_patterns.unwrap_or_default();

    tokio::spawn(async move {
        // Create shared atomic counters for all scanners
        let total_found = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let total_processed = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let total_bytes = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let scan_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Progress updater
        let progress_state = state_clone.clone();
        let progress_task_id = task_id_clone.clone();
        let pf = total_found.clone();
        let pp = total_processed.clone();
        let pb = total_bytes.clone();
        let pd = scan_done.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                if pd.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                let found = pf.load(std::sync::atomic::Ordering::Relaxed);
                let processed = pp.load(std::sync::atomic::Ordering::Relaxed);
                let bytes = pb.load(std::sync::atomic::Ordering::Relaxed) as u64;

                let mut guard = progress_state.write().await;
                if let Some(task) = guard.scan_tasks.get_mut(&progress_task_id) {
                    if matches!(task.status, ScanStatus::Running { .. }) {
                        task.status = ScanStatus::Running {
                            files_found: found,
                            files_processed: processed,
                            bytes_processed: bytes,
                        };
                    }
                }
            }
        });

        // Scan all directories sequentially, combining results
        let tf = total_found.clone();
        let tp = total_processed.clone();
        let tb = total_bytes.clone();
        let scan_paths = paths.clone();
        let exclude_patterns_for_scan = exclude_pats;

        let scan_result = tokio::task::spawn_blocking(move || {
            let mut all_files: Vec<minion_files::FileInfo> = Vec::new();
            let mut total_size: u64 = 0;
            let mut error_count: usize = 0;

            for p in &scan_paths {
                let scan_config = ScanConfig {
                    root: PathBuf::from(p),
                    recursive: true,
                    compute_hashes: false, // Skip hashing for speed, hash only candidates later
                    exclude_patterns: exclude_patterns_for_scan.clone(),
                    ..Default::default()
                };

                let scanner = Scanner::new(scan_config);

                // Wire scanner's counters to our totals
                let sf = scanner.files_found();
                let sp = scanner.files_processed();
                let sb = scanner.bytes_processed();

                // Snapshot before this scan
                let base_found = tf.load(std::sync::atomic::Ordering::Relaxed);
                let base_processed = tp.load(std::sync::atomic::Ordering::Relaxed);
                let base_bytes = tb.load(std::sync::atomic::Ordering::Relaxed);

                // Spawn a thread to relay scanner progress to totals
                let relay_tf = tf.clone();
                let relay_tp = tp.clone();
                let relay_tb = tb.clone();
                let relay_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let relay_done2 = relay_done.clone();

                let relay = std::thread::spawn(move || {
                    while !relay_done2.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        relay_tf.store(
                            base_found + sf.load(std::sync::atomic::Ordering::Relaxed),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        relay_tp.store(
                            base_processed + sp.load(std::sync::atomic::Ordering::Relaxed),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                        relay_tb.store(
                            base_bytes + sb.load(std::sync::atomic::Ordering::Relaxed),
                            std::sync::atomic::Ordering::Relaxed,
                        );
                    }
                });

                match scanner.scan() {
                    Ok(result) => {
                        total_size += result.total_size;
                        error_count += result.error_count;
                        all_files.extend(result.files);
                    }
                    Err(e) => {
                        tracing::error!("Scan of {} failed: {}", p, e);
                        error_count += 1;
                    }
                }

                relay_done.store(true, std::sync::atomic::Ordering::Relaxed);
                let _ = relay.join();

                // Update totals to exact values after this scan
                tf.store(all_files.len(), std::sync::atomic::Ordering::Relaxed);
                tp.store(all_files.len(), std::sync::atomic::Ordering::Relaxed);
            }

            Ok::<_, minion_files::Error>(minion_files::ScanResult {
                files: all_files,
                total_size,
                error_count,
            })
        })
        .await
        .unwrap_or_else(|e| Err(minion_files::Error::Scan(format!("Task panicked: {}", e))));

        scan_done.store(true, std::sync::atomic::Ordering::Relaxed);

        match scan_result {
            Ok(result) => {
                tracing::info!(
                    "Multi-scan complete: {} files, {} bytes",
                    result.files.len(),
                    result.total_size
                );

                let mut files_result = result.files;
                let total_size = result.total_size;

                let dupes_result = tokio::task::spawn_blocking(move || {
                    use std::collections::HashMap;
                    let mut size_groups: HashMap<u64, Vec<usize>> = HashMap::new();
                    for (i, f) in files_result.iter().enumerate() {
                        if f.size >= 1024 {
                            size_groups.entry(f.size).or_default().push(i);
                        }
                    }
                    let mut needs_hash = 0usize;
                    for indices in size_groups.values() {
                        if indices.len() > 1 {
                            needs_hash += indices.len();
                            for &idx in indices {
                                if files_result[idx].sha256.is_none() {
                                    if let Ok(hash) =
                                        minion_files::hash::compute_sha256(&files_result[idx].path)
                                    {
                                        files_result[idx].sha256 = Some(hash);
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!(
                        "Hashed {} of {} files (size-candidate optimization)",
                        needs_hash,
                        files_result.len()
                    );
                    let finder = DuplicateFinder::default();
                    let dupes = finder.find(&files_result);
                    (files_result, dupes)
                })
                .await
                .unwrap_or_else(|_| (vec![], vec![]));

                let (files_final, duplicates) = dupes_result;
                let duplicates_count = duplicates.len();
                tracing::info!(
                    "Found {} duplicate groups across directories",
                    duplicates_count
                );

                let mut state_guard = state_clone.write().await;
                if let Some(task) = state_guard.scan_tasks.get_mut(&task_id_clone) {
                    task.status = ScanStatus::Completed {
                        total_files: files_final.len(),
                        total_size,
                        duplicates_found: duplicates_count,
                    };
                }
                state_guard.scan_cache = Some(ScanCache {
                    files: files_final,
                    duplicates,
                    last_updated: Utc::now(),
                });
            }
            Err(e) => {
                tracing::error!("Multi-scan failed: {}", e);
                let mut state_guard = state_clone.write().await;
                if let Some(task) = state_guard.scan_tasks.get_mut(&task_id_clone) {
                    task.status = ScanStatus::Failed(e.to_string());
                }
            }
        }
    });

    Ok(ScanProgress {
        task_id,
        status: "running".to_string(),
        files_scanned: 0,
        total_files: None,
        progress_percent: 0.0,
    })
}

#[tauri::command]
pub async fn files_get_scan_progress(
    state: State<'_, AppStateHandle>,
    task_id: String,
) -> Result<ScanProgress, String> {
    let state = state.read().await;

    if let Some(task) = state.scan_tasks.get(&task_id) {
        match &task.status {
            ScanStatus::Pending => Ok(ScanProgress {
                task_id,
                status: "pending".to_string(),
                files_scanned: 0,
                total_files: None,
                progress_percent: 0.0,
            }),
            ScanStatus::Running {
                files_found,
                files_processed,
                ..
            } => Ok(ScanProgress {
                task_id,
                status: "running".to_string(),
                files_scanned: *files_processed as u64,
                total_files: Some(*files_found as u64),
                progress_percent: if *files_found > 0 {
                    (*files_processed as f32 / *files_found as f32) * 100.0
                } else {
                    0.0
                },
            }),
            ScanStatus::Completed {
                total_files,
                total_size: _,
                duplicates_found: _,
            } => Ok(ScanProgress {
                task_id,
                status: "completed".to_string(),
                files_scanned: *total_files as u64,
                total_files: Some(*total_files as u64),
                progress_percent: 100.0,
            }),
            ScanStatus::Failed(err) => Ok(ScanProgress {
                task_id,
                status: format!("failed: {}", err),
                files_scanned: 0,
                total_files: None,
                progress_percent: 0.0,
            }),
        }
    } else {
        Err(format!("Task not found: {}", task_id))
    }
}

#[tauri::command]
pub async fn files_cancel_scan(
    state: State<'_, AppStateHandle>,
    task_id: String,
) -> Result<(), String> {
    let mut state_guard = state.write().await;
    if let Some(task) = state_guard.scan_tasks.get_mut(&task_id) {
        if matches!(task.status, ScanStatus::Running { .. }) {
            task.status = ScanStatus::Failed("Cancelled by user".to_string());
            tracing::info!("Scan {} cancelled by user", task_id);
            Ok(())
        } else {
            Err(format!("Task {} is not currently running", task_id))
        }
    } else {
        Err(format!("Task not found: {}", task_id))
    }
}

#[tauri::command]
pub async fn files_list_duplicates(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<DuplicateGroupResponse>, String> {
    let state = state.read().await;

    if let Some(cache) = &state.scan_cache {
        let groups: Vec<DuplicateGroupResponse> = cache
            .duplicates
            .iter()
            .map(|d| {
                let match_type_str = format!("{:?}", d.match_type);
                let match_label = match &match_type_str[..] {
                    "Exact" => "Identical content (SHA-256 hash match)".to_string(),
                    "Perceptual" => "Visually similar images".to_string(),
                    "Near" => "Nearly identical content".to_string(),
                    _ => format!("{} match", match_type_str),
                };
                let hash = d.files.first().and_then(|f| f.sha256.clone());
                DuplicateGroupResponse {
                    id: d.id.clone(),
                    match_type: match_type_str,
                    match_label,
                    file_count: d.files.len(),
                    total_size: d.files.iter().map(|f| f.size).sum(),
                    wasted_space: d.wasted_bytes,
                    hash,
                    files: d
                        .files
                        .iter()
                        .map(|f| FileInfoResponse {
                            path: f.path.to_string_lossy().to_string(),
                            name: f.name.clone(),
                            size: f.size,
                            modified: f.modified.to_rfc3339(),
                            extension: f.extension.clone(),
                        })
                        .collect(),
                }
            })
            .collect();

        Ok(groups)
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub async fn files_get_analytics(
    state: State<'_, AppStateHandle>,
) -> Result<StorageAnalytics, String> {
    let state = state.read().await;

    if let Some(cache) = &state.scan_cache {
        let calc = AnalyticsCalculator::new(10);
        let analytics = calc.calculate(&cache.files);

        let by_extension: Vec<ExtensionStats> = analytics
            .by_extension
            .iter()
            .map(|(ext, stats)| ExtensionStats {
                extension: ext.clone(),
                count: stats.count,
                size: stats.total_size,
            })
            .collect();

        let duplicate_size: u64 = cache.duplicates.iter().map(|d| d.wasted_bytes).sum();

        Ok(StorageAnalytics {
            total_files: analytics.total_files,
            total_size: analytics.total_size,
            by_extension,
            duplicates_found: cache.duplicates.len() as u64,
            duplicate_size,
        })
    } else {
        Ok(StorageAnalytics {
            total_files: 0,
            total_size: 0,
            by_extension: vec![],
            duplicates_found: 0,
            duplicate_size: 0,
        })
    }
}

// ============================================================================
// File operations
// ============================================================================

#[tauri::command]
pub async fn files_open_file(path: String) -> Result<(), String> {
    let file_path = std::path::Path::new(&path);
    if !file_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }
    // Use xdg-open on Linux to open with default app
    std::process::Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("Failed to open file: {}", e))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct BulkDeleteRequest {
    pub paths: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BulkOperationResult {
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<String>,
    pub freed_bytes: u64,
}

#[tauri::command]
pub async fn files_bulk_delete(
    state: State<'_, AppStateHandle>,
    request: BulkDeleteRequest,
) -> Result<BulkOperationResult, String> {
    let mut succeeded = 0;
    let mut failed = 0;
    let mut errors = Vec::new();
    let mut freed_bytes: u64 = 0;

    for path_str in &request.paths {
        let path = std::path::Path::new(path_str);
        if !path.exists() {
            failed += 1;
            errors.push(format!("Not found: {}", path_str));
            continue;
        }

        match std::fs::metadata(path) {
            Ok(meta) => {
                let size = meta.len();
                match std::fs::remove_file(path) {
                    Ok(()) => {
                        succeeded += 1;
                        freed_bytes += size;
                        tracing::info!("Deleted: {}", path_str);
                    }
                    Err(e) => {
                        failed += 1;
                        errors.push(format!("{}: {}", path_str, e));
                    }
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", path_str, e));
            }
        }
    }

    // Clear scan cache since files changed
    if succeeded > 0 {
        let mut state_guard = state.write().await;
        state_guard.scan_cache = None;
    }

    Ok(BulkOperationResult {
        succeeded,
        failed,
        errors,
        freed_bytes,
    })
}

#[derive(Debug, Deserialize)]
pub struct BulkMoveRequest {
    pub paths: Vec<String>,
    pub destination: String,
}

#[tauri::command]
pub async fn files_bulk_move(
    state: State<'_, AppStateHandle>,
    request: BulkMoveRequest,
) -> Result<BulkOperationResult, String> {
    let dest = std::path::Path::new(&request.destination);
    if !dest.exists() {
        std::fs::create_dir_all(dest)
            .map_err(|e| format!("Failed to create destination: {}", e))?;
    }
    if !dest.is_dir() {
        return Err(format!(
            "Destination is not a directory: {}",
            request.destination
        ));
    }

    let mut succeeded = 0;
    let mut failed = 0;
    let mut errors = Vec::new();
    let mut freed_bytes: u64 = 0;

    for path_str in &request.paths {
        let src = std::path::Path::new(path_str);
        if !src.exists() {
            failed += 1;
            errors.push(format!("Not found: {}", path_str));
            continue;
        }

        let filename = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let mut dest_path = dest.join(&filename);

        // Handle name conflicts by appending a number
        if dest_path.exists() {
            let stem = src
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = src
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let mut counter = 1;
            loop {
                dest_path = dest.join(format!("{}_{}{}", stem, counter, ext));
                if !dest_path.exists() {
                    break;
                }
                counter += 1;
            }
        }

        match std::fs::metadata(src) {
            Ok(meta) => {
                let size = meta.len();
                match std::fs::rename(src, &dest_path) {
                    Ok(()) => {
                        succeeded += 1;
                        freed_bytes += size;
                        tracing::info!("Moved: {} -> {}", path_str, dest_path.display());
                    }
                    Err(_) => {
                        // rename fails across filesystems, try copy+delete
                        match std::fs::copy(src, &dest_path) {
                            Ok(_) => {
                                let _ = std::fs::remove_file(src);
                                succeeded += 1;
                                freed_bytes += size;
                            }
                            Err(e) => {
                                failed += 1;
                                errors.push(format!("{}: {}", path_str, e));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", path_str, e));
            }
        }
    }

    if succeeded > 0 {
        let mut state_guard = state.write().await;
        state_guard.scan_cache = None;
    }

    Ok(BulkOperationResult {
        succeeded,
        failed,
        errors,
        freed_bytes,
    })
}

// ============================================================================
// Book Reader commands
// ============================================================================

#[derive(Debug, Serialize)]
pub struct BookInfo {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub path: String,
    pub format: String,
    pub cover_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChapterInfo {
    pub index: usize,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct TocEntryInfo {
    pub title: String,
    pub href: String,
}

#[derive(Debug, Serialize)]
pub struct BookContentResponse {
    pub metadata: BookMetadataInfo,
    pub chapters: Vec<ChapterInfo>,
    pub toc: Vec<TocEntryInfo>,
    pub file_path: Option<String>,
    pub format: String,
    pub cover_base64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BookMetadataInfo {
    pub title: String,
    pub authors: Vec<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn reader_open_book(path: String) -> Result<BookContentResponse, String> {
    let book_path = PathBuf::from(&path);

    if !book_path.exists() {
        return Err(format!("File does not exist: {}", path));
    }

    let ext = book_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let format =
        BookFormat::from_extension(ext).ok_or_else(|| format!("Unsupported format: {}", ext))?;

    match format {
        BookFormat::Epub => {
            let content = parse_epub(&book_path).map_err(|e| e.to_string())?;

            // Get metadata
            let doc = epub::doc::EpubDoc::new(&book_path).map_err(|e| e.to_string())?;
            let get_str =
                |name: &str| -> Option<String> { doc.mdata(name).map(|m| m.value.clone()) };

            let metadata = BookMetadataInfo {
                title: get_str("title").unwrap_or_else(|| "Unknown".to_string()),
                authors: get_str("creator").map(|a| vec![a]).unwrap_or_default(),
                publisher: get_str("publisher"),
                language: get_str("language"),
                description: get_str("description"),
            };

            let chapters: Vec<ChapterInfo> = content
                .chapters
                .iter()
                .map(|c| ChapterInfo {
                    index: c.index,
                    title: c.title.clone(),
                    content: c.content.clone(),
                })
                .collect();

            let toc: Vec<TocEntryInfo> = content
                .toc
                .iter()
                .map(|t| TocEntryInfo {
                    title: t.title.clone(),
                    href: t.href.clone(),
                })
                .collect();

            Ok(BookContentResponse {
                metadata,
                chapters,
                toc,
                file_path: Some(path.clone()),
                format: "epub".to_string(),
                cover_base64: content.cover_base64,
            })
        }
        BookFormat::Pdf => {
            let content = parse_pdf(&book_path).map_err(|e| e.to_string())?;
            let filename = book_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            let chapters: Vec<ChapterInfo> = content
                .chapters
                .iter()
                .map(|c| ChapterInfo {
                    index: c.index,
                    title: c.title.clone(),
                    content: c.content.clone(),
                })
                .collect();

            Ok(BookContentResponse {
                metadata: BookMetadataInfo {
                    title: filename,
                    authors: vec![],
                    publisher: None,
                    language: None,
                    description: None,
                },
                chapters,
                toc: vec![],
                file_path: Some(path.clone()),
                format: "pdf".to_string(),
                cover_base64: None,
            })
        }
        BookFormat::Txt => {
            let content = std::fs::read_to_string(&book_path).map_err(|e| e.to_string())?;
            let filename = book_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            Ok(BookContentResponse {
                metadata: BookMetadataInfo {
                    title: filename.clone(),
                    authors: vec![],
                    publisher: None,
                    language: None,
                    description: None,
                },
                chapters: vec![ChapterInfo {
                    index: 0,
                    title: filename,
                    content: format!(
                        "<pre style=\"white-space: pre-wrap; font-family: inherit;\">{}</pre>",
                        html_escape::encode_text(&content)
                    ),
                }],
                toc: vec![],
                file_path: Some(path.clone()),
                format: "txt".to_string(),
                cover_base64: None,
            })
        }
        BookFormat::Markdown => {
            let content = std::fs::read_to_string(&book_path).map_err(|e| e.to_string())?;
            let filename = book_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            // Basic markdown to HTML (simplified)
            let html_content = markdown_to_html(&content);

            Ok(BookContentResponse {
                metadata: BookMetadataInfo {
                    title: filename.clone(),
                    authors: vec![],
                    publisher: None,
                    language: None,
                    description: None,
                },
                chapters: vec![ChapterInfo {
                    index: 0,
                    title: filename,
                    content: html_content,
                }],
                toc: vec![],
                file_path: Some(path.clone()),
                format: "md".to_string(),
                cover_base64: None,
            })
        }
        BookFormat::Html => {
            let content = std::fs::read_to_string(&book_path).map_err(|e| e.to_string())?;
            let filename = book_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();

            Ok(BookContentResponse {
                metadata: BookMetadataInfo {
                    title: filename.clone(),
                    authors: vec![],
                    publisher: None,
                    language: None,
                    description: None,
                },
                chapters: vec![ChapterInfo {
                    index: 0,
                    title: filename,
                    content,
                }],
                toc: vec![],
                file_path: Some(path.clone()),
                format: "html".to_string(),
                cover_base64: None,
            })
        }
        _ => Err(format!("Format {:?} not yet supported", format)),
    }
}

/// Simple markdown to HTML conversion
fn markdown_to_html(md: &str) -> String {
    let mut html = String::new();

    for line in md.lines() {
        let trimmed = line.trim();

        if let Some(h1) = trimmed.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", h1));
        } else if let Some(h2) = trimmed.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", h2));
        } else if let Some(h3) = trimmed.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", h3));
        } else if let Some(li) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            html.push_str(&format!("<li>{}</li>\n", li));
        } else if trimmed.is_empty() {
            html.push_str("<br/>\n");
        } else {
            html.push_str(&format!("<p>{}</p>\n", trimmed));
        }
    }

    html
}

#[tauri::command]
pub async fn reader_list_books(directory: String) -> Result<Vec<BookInfo>, String> {
    let dir_path = PathBuf::from(&directory);

    if !dir_path.exists() || !dir_path.is_dir() {
        return Err(format!("Invalid directory: {}", directory));
    }

    let mut books = Vec::new();
    let supported_extensions = ["epub", "pdf", "txt", "md", "markdown", "html", "htm"];

    if let Ok(entries) = std::fs::read_dir(&dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                        let filename = path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string();

                        books.push(BookInfo {
                            id: uuid::Uuid::new_v4().to_string(),
                            title: filename,
                            authors: vec![],
                            path: path.to_string_lossy().to_string(),
                            format: ext.to_uppercase(),
                            cover_url: None,
                        });
                    }
                }
            }
        }
    }

    Ok(books)
}

// ============================================================================
// Finance commands
// ============================================================================

#[tauri::command]
pub async fn finance_add_account(
    state: State<'_, AppStateHandle>,
    name: String,
    account_type: String,
    currency: Option<String>,
    institution: Option<String>,
    balance: Option<f64>,
) -> Result<FinanceAccountResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let currency = currency.unwrap_or_else(|| "INR".to_string());
    let initial_balance = balance.unwrap_or(0.0);
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO finance_accounts (id, name, account_type, institution, balance, currency, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        rusqlite::params![id, name, account_type, institution, initial_balance, currency, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(FinanceAccountResponse {
        id,
        name,
        account_type,
        institution: institution.clone(),
        balance: initial_balance,
        currency,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn finance_list_accounts(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<FinanceAccountResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, account_type, institution, balance, currency, created_at, updated_at
             FROM finance_accounts ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(FinanceAccountResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                account_type: row.get(2)?,
                institution: row.get(3)?,
                balance: row.get(4)?,
                currency: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row.map_err(|e| e.to_string())?);
    }
    Ok(accounts)
}

#[tauri::command]
pub async fn finance_add_transaction(
    state: State<'_, AppStateHandle>,
    account_id: String,
    transaction_type: String,
    amount: f64,
    description: Option<String>,
    category: Option<String>,
    date: Option<String>,
) -> Result<FinanceTransactionResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let date = date.unwrap_or_else(|| now.clone());

    conn.execute(
        "INSERT INTO finance_transactions (id, account_id, type, amount, description, category, date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![id, account_id, transaction_type, amount, description, category, date, now],
    )
    .map_err(|e| e.to_string())?;

    // Update account balance
    let balance_delta = if transaction_type == "credit" {
        amount
    } else {
        -amount
    };
    conn.execute(
        "UPDATE finance_accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![balance_delta, now, account_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(FinanceTransactionResponse {
        id,
        account_id,
        transaction_type,
        amount,
        description,
        category,
        tags: None,
        date,
        created_at: now,
    })
}

#[tauri::command]
pub async fn finance_list_transactions(
    state: State<'_, AppStateHandle>,
    account_id: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<FinanceTransactionResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let limit = limit.unwrap_or(100);

    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &account_id {
        Some(aid) => (
            "SELECT id, account_id, type, amount, description, category, tags, date, created_at
             FROM finance_transactions WHERE account_id = ?1 ORDER BY date DESC LIMIT ?2"
                .to_string(),
            vec![
                Box::new(aid.clone()) as Box<dyn rusqlite::types::ToSql>,
                Box::new(limit),
            ],
        ),
        None => (
            "SELECT id, account_id, type, amount, description, category, tags, date, created_at
             FROM finance_transactions ORDER BY date DESC LIMIT ?1"
                .to_string(),
            vec![Box::new(limit) as Box<dyn rusqlite::types::ToSql>],
        ),
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok(FinanceTransactionResponse {
                id: row.get(0)?,
                account_id: row.get(1)?,
                transaction_type: row.get(2)?,
                amount: row.get(3)?,
                description: row.get(4)?,
                category: row.get(5)?,
                tags: row.get(6)?,
                date: row.get(7)?,
                created_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut transactions = Vec::new();
    for row in rows {
        transactions.push(row.map_err(|e| e.to_string())?);
    }
    Ok(transactions)
}

#[tauri::command]
pub async fn finance_get_summary(
    state: State<'_, AppStateHandle>,
) -> Result<FinancialSummaryResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    // Total assets (positive-balance accounts: bank, investment, wallet)
    let total_assets: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(balance), 0) FROM finance_accounts
             WHERE account_type IN ('bank', 'investment', 'wallet')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Total liabilities (credit_card, loan balances)
    let total_liabilities: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(ABS(balance)), 0) FROM finance_accounts
             WHERE account_type IN ('credit_card', 'loan')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Monthly income (credits in the last 30 days)
    let monthly_income: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM finance_transactions
             WHERE type = 'credit' AND date >= date('now', '-30 days')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Monthly expenses (debits in the last 30 days)
    let monthly_expenses: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(amount), 0) FROM finance_transactions
             WHERE type = 'debit' AND date >= date('now', '-30 days')",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let savings_rate = if monthly_income > 0.0 {
        ((monthly_income - monthly_expenses) / monthly_income) * 100.0
    } else {
        0.0
    };

    let account_count: u64 = conn
        .query_row("SELECT COUNT(*) FROM finance_accounts", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    let transaction_count: u64 = conn
        .query_row("SELECT COUNT(*) FROM finance_transactions", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    Ok(FinancialSummaryResponse {
        net_worth: total_assets - total_liabilities,
        total_assets,
        total_liabilities,
        monthly_income,
        monthly_expenses,
        savings_rate,
        account_count,
        transaction_count,
    })
}

#[tauri::command]
pub async fn finance_import_csv(
    state: State<'_, AppStateHandle>,
    path: String,
    account_id: String,
    mapping: Option<CsvMappingRequest>,
) -> Result<CsvImportResult, String> {
    let csv_path = PathBuf::from(&path);
    if !csv_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    // Build column mapping
    let col_mapping = match mapping {
        Some(m) => minion_finance::import::CsvColumnMapping {
            date_column: m.date_column.unwrap_or_else(|| "Date".to_string()),
            description_column: m
                .description_column
                .unwrap_or_else(|| "Description".to_string()),
            amount_column: m.amount_column.unwrap_or_else(|| "Amount".to_string()),
            debit_column: m.debit_column,
            credit_column: m.credit_column,
            balance_column: m.balance_column,
            date_format: m.date_format.unwrap_or_else(|| "%d/%m/%Y".to_string()),
        },
        None => {
            // Try auto-detect from CSV headers
            let mut reader = csv::ReaderBuilder::new()
                .flexible(true)
                .trim(csv::Trim::All)
                .from_path(&csv_path)
                .map_err(|e| format!("Failed to open CSV: {}", e))?;
            let headers: Vec<String> = reader
                .headers()
                .map_err(|e| format!("Failed to read headers: {}", e))?
                .iter()
                .map(|h: &str| h.to_string())
                .collect();
            minion_finance::import::auto_detect_columns(&headers)
        }
    };

    let import_result =
        minion_finance::import::import_csv(&csv_path, &col_mapping).map_err(|e| e.to_string())?;

    // Persist imported transactions to database
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    let mut insert_stmt = conn
        .prepare(
            "INSERT INTO finance_transactions
             (id, account_id, type, amount, description, category, date, imported_from, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .map_err(|e| e.to_string())?;

    for tx in &import_result.transactions {
        let id = uuid::Uuid::new_v4().to_string();
        insert_stmt
            .execute(rusqlite::params![
                id,
                account_id,
                tx.transaction_type,
                tx.amount,
                tx.description,
                tx.category,
                tx.date,
                path,
                now,
            ])
            .map_err(|e| e.to_string())?;
    }

    // Recalculate account balance from all transactions
    let new_balance: f64 = conn
        .query_row(
            "SELECT COALESCE(
                SUM(CASE WHEN type = 'credit' THEN amount ELSE -amount END), 0
             ) FROM finance_transactions WHERE account_id = ?1",
            rusqlite::params![account_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    conn.execute(
        "UPDATE finance_accounts SET balance = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![new_balance, now, account_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(CsvImportResult {
        total_rows: import_result.total_rows,
        imported: import_result.imported,
        skipped: import_result.skipped,
        errors: import_result.errors,
    })
}

#[tauri::command]
pub async fn finance_spending_by_category(
    state: State<'_, AppStateHandle>,
    month: Option<String>,
) -> Result<HashMap<String, f64>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    // month is "YYYY-MM" e.g. "2026-03". If None, return all-time.
    let (sql, params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = match &month {
        Some(m) => {
            let start = format!("{}-01", m);
            // Calculate end of month by adding 1 month
            let end = format!("{}-01", {
                let parts: Vec<&str> = m.split('-').collect();
                if parts.len() == 2 {
                    let y: i32 = parts[0].parse().unwrap_or(2026);
                    let mo: i32 = parts[1].parse().unwrap_or(1);
                    if mo >= 12 {
                        format!("{:04}-{:02}", y + 1, 1)
                    } else {
                        format!("{:04}-{:02}", y, mo + 1)
                    }
                } else {
                    m.clone()
                }
            });
            (
                "SELECT COALESCE(category, 'Uncategorized'), SUM(amount)
                 FROM finance_transactions WHERE type = 'debit'
                 AND date >= ?1 AND date < ?2
                 GROUP BY category ORDER BY SUM(amount) DESC"
                    .to_string(),
                vec![
                    Box::new(start) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(end),
                ],
            )
        }
        None => (
            "SELECT COALESCE(category, 'Uncategorized'), SUM(amount)
             FROM finance_transactions WHERE type = 'debit'
             GROUP BY category ORDER BY SUM(amount) DESC"
                .to_string(),
            vec![],
        ),
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut result = HashMap::new();
    for row in rows {
        let (cat, amount) = row.map_err(|e| e.to_string())?;
        result.insert(cat, amount);
    }
    Ok(result)
}

// ============================================================================
// Investment portfolio commands
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AddInvestmentRequest {
    pub name: String,
    pub investment_type: String,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    pub purchase_price: f64,
    pub current_price: f64,
    pub quantity: f64,
    pub purchase_date: String,
}

#[tauri::command]
pub async fn finance_add_investment(
    state: State<'_, AppStateHandle>,
    req: AddInvestmentRequest,
) -> Result<InvestmentResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO finance_investments
         (id, name, type, symbol, exchange, purchase_price, current_price, quantity,
          purchase_date, last_price_update, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        rusqlite::params![
            id,
            req.name,
            req.investment_type,
            req.symbol,
            req.exchange,
            req.purchase_price,
            req.current_price,
            req.quantity,
            req.purchase_date,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    let invested = req.purchase_price * req.quantity;
    let current_value = req.current_price * req.quantity;
    let gain_loss = current_value - invested;
    let gain_loss_pct = if invested > 0.0 {
        (gain_loss / invested) * 100.0
    } else {
        0.0
    };

    Ok(InvestmentResponse {
        id,
        name: req.name,
        investment_type: req.investment_type,
        symbol: req.symbol,
        exchange: req.exchange,
        purchase_price: req.purchase_price,
        current_price: req.current_price,
        quantity: req.quantity,
        purchase_date: req.purchase_date,
        gain_loss,
        gain_loss_pct,
        current_value,
    })
}

#[tauri::command]
pub async fn finance_list_investments(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<InvestmentResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, type, symbol, exchange, purchase_price, current_price,
                    quantity, purchase_date
             FROM finance_investments ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let purchase_price: f64 = row.get(5)?;
            let current_price: f64 = row.get(6)?;
            let quantity: f64 = row.get(7)?;
            let invested = purchase_price * quantity;
            let current_value = current_price * quantity;
            let gain_loss = current_value - invested;
            let gain_loss_pct = if invested > 0.0 {
                (gain_loss / invested) * 100.0
            } else {
                0.0
            };

            Ok(InvestmentResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                investment_type: row.get(2)?,
                symbol: row.get(3)?,
                exchange: row.get(4)?,
                purchase_price,
                current_price,
                quantity,
                purchase_date: row.get(8)?,
                gain_loss,
                gain_loss_pct,
                current_value,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut investments = Vec::new();
    for row in rows {
        investments.push(row.map_err(|e| e.to_string())?);
    }
    Ok(investments)
}

#[tauri::command]
pub async fn finance_portfolio_summary(
    state: State<'_, AppStateHandle>,
) -> Result<PortfolioSummary, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT type, purchase_price, current_price, quantity
             FROM finance_investments",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut total_invested = 0.0;
    let mut current_value = 0.0;
    let mut type_values: HashMap<String, f64> = HashMap::new();

    for row in rows {
        let (inv_type, pp, cp, qty) = row.map_err(|e| e.to_string())?;
        let invested = pp * qty;
        let value = cp * qty;
        total_invested += invested;
        current_value += value;
        *type_values.entry(inv_type).or_insert(0.0) += value;
    }

    let total_gain_loss = current_value - total_invested;
    let total_gain_loss_pct = if total_invested > 0.0 {
        (total_gain_loss / total_invested) * 100.0
    } else {
        0.0
    };

    let by_type: Vec<TypeAllocation> = type_values
        .into_iter()
        .map(|(investment_type, value)| {
            let percentage = if current_value > 0.0 {
                (value / current_value) * 100.0
            } else {
                0.0
            };
            TypeAllocation {
                investment_type,
                value,
                percentage,
            }
        })
        .collect();

    Ok(PortfolioSummary {
        total_invested,
        current_value,
        total_gain_loss,
        total_gain_loss_pct,
        by_type,
    })
}

#[tauri::command]
pub async fn finance_update_price(
    state: State<'_, AppStateHandle>,
    investment_id: String,
    new_price: f64,
) -> Result<(), String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();

    let rows_changed = conn
        .execute(
            "UPDATE finance_investments SET current_price = ?1, last_price_update = ?2
             WHERE id = ?3",
            rusqlite::params![new_price, now, investment_id],
        )
        .map_err(|e| e.to_string())?;

    if rows_changed == 0 {
        return Err(format!("Investment not found: {}", investment_id));
    }

    Ok(())
}

#[tauri::command]
pub async fn finance_delete_investment(
    state: State<'_, AppStateHandle>,
    investment_id: String,
) -> Result<(), String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let rows_changed = conn
        .execute(
            "DELETE FROM finance_investments WHERE id = ?1",
            rusqlite::params![investment_id],
        )
        .map_err(|e| e.to_string())?;

    if rows_changed == 0 {
        return Err(format!("Investment not found: {}", investment_id));
    }

    Ok(())
}

#[tauri::command]
pub async fn finance_fetch_mf_nav(scheme_code: String) -> Result<f64, String> {
    let url = format!("https://api.mfapi.in/mf/{}/latest", scheme_code);

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch MF NAV: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse MF API response: {}", e))?;

    let nav_str = body
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.get("nav"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "NAV data not found in API response".to_string())?;

    nav_str
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse NAV value '{}': {}", nav_str, e))
}

#[tauri::command]
pub async fn finance_calc_cagr(initial: f64, current: f64, years: f64) -> Result<f64, String> {
    if initial <= 0.0 {
        return Err("Initial value must be greater than zero".to_string());
    }
    if years <= 0.0 {
        return Err("Years must be greater than zero".to_string());
    }

    let cagr = ((current / initial).powf(1.0 / years) - 1.0) * 100.0;
    Ok(cagr)
}

// ============================================================================
// Zerodha Kite Connect types & helpers
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ZerodhaHolding {
    pub tradingsymbol: String,
    pub exchange: String,
    pub quantity: i64,
    pub average_price: f64,
    pub last_price: f64,
    pub pnl: f64,
    pub day_change_percentage: f64,
}

/// Fetch holdings from Kite Connect API (non-command helper).
async fn fetch_zerodha_holdings(
    api_key: &str,
    access_token: &str,
) -> Result<Vec<ZerodhaHolding>, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.kite.trade/portfolio/holdings")
        .header("X-Kite-Version", "3")
        .header(
            "Authorization",
            format!("token {}:{}", api_key, access_token),
        )
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Kite API error: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Kite API returned error: {}", body));
    }

    // Kite returns { "status": "success", "data": [...] }
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let holdings = body["data"]
        .as_array()
        .ok_or("Invalid Kite API response")?
        .iter()
        .map(|h| ZerodhaHolding {
            tradingsymbol: h["tradingsymbol"].as_str().unwrap_or("").to_string(),
            exchange: h["exchange"].as_str().unwrap_or("").to_string(),
            quantity: h["quantity"].as_i64().unwrap_or(0),
            average_price: h["average_price"].as_f64().unwrap_or(0.0),
            last_price: h["last_price"].as_f64().unwrap_or(0.0),
            pnl: h["pnl"].as_f64().unwrap_or(0.0),
            day_change_percentage: h["day_change_percentage"].as_f64().unwrap_or(0.0),
        })
        .collect();

    Ok(holdings)
}

/// Read Zerodha credentials from the config table.
fn read_zerodha_creds(conn: &rusqlite::Connection) -> Result<(String, String), String> {
    let api_key: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'zerodha_api_key'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "Zerodha API key not configured".to_string())?;
    let access_token: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'zerodha_access_token'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| "Not logged into Zerodha. Please login first.".to_string())?;
    Ok((api_key, access_token))
}

// ============================================================================
// Zerodha Kite Connect commands
// ============================================================================

#[tauri::command]
pub async fn zerodha_save_config(
    state: State<'_, AppStateHandle>,
    api_key: String,
    api_secret: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('zerodha_api_key', ?1)",
        rusqlite::params![api_key],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('zerodha_api_secret', ?1)",
        rusqlite::params![api_secret],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn zerodha_open_login(
    app: tauri::AppHandle,
    state: State<'_, AppStateHandle>,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let api_key: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'zerodha_api_key'",
            [],
            |row| row.get(0),
        )
        .map_err(|_| {
            "Zerodha API key not configured. Go to Settings > Zerodha to add it.".to_string()
        })?;
    drop(st); // release lock before opening window

    let login_url = format!(
        "https://kite.zerodha.com/connect/login?v=3&api_key={}",
        api_key
    );

    use tauri::{WebviewUrl, WebviewWindowBuilder};
    WebviewWindowBuilder::new(
        &app,
        "zerodha-login",
        WebviewUrl::External(login_url.parse().unwrap()),
    )
    .title("Zerodha Kite - Login")
    .inner_size(600.0, 700.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn zerodha_save_token(
    state: State<'_, AppStateHandle>,
    access_token: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('zerodha_access_token', ?1)",
        rusqlite::params![access_token],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn zerodha_fetch_holdings(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<ZerodhaHolding>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let (api_key, access_token) = read_zerodha_creds(&conn)?;
    drop(st);

    fetch_zerodha_holdings(&api_key, &access_token).await
}

#[tauri::command]
pub async fn zerodha_sync_to_portfolio(state: State<'_, AppStateHandle>) -> Result<String, String> {
    // Read creds and fetch holdings
    let (api_key, access_token) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        read_zerodha_creds(&conn)?
    };
    let holdings = fetch_zerodha_holdings(&api_key, &access_token).await?;

    // Upsert into finance_investments
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    let mut synced = 0u32;
    for h in &holdings {
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM finance_investments WHERE symbol = ?1 AND exchange = ?2",
                rusqlite::params![h.tradingsymbol, h.exchange],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = existing {
            conn.execute(
                "UPDATE finance_investments SET current_price = ?1, \
                 last_price_update = ?2 WHERE id = ?3",
                rusqlite::params![h.last_price, now, id],
            )
            .map_err(|e| e.to_string())?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO finance_investments \
                 (id, name, type, symbol, exchange, purchase_price, current_price, \
                  quantity, last_price_update, created_at) \
                 VALUES (?1, ?2, 'stock', ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                rusqlite::params![
                    id,
                    h.tradingsymbol,
                    h.tradingsymbol,
                    h.exchange,
                    h.average_price,
                    h.last_price,
                    h.quantity as f64,
                    now
                ],
            )
            .map_err(|e| e.to_string())?;
        }
        synced += 1;
    }

    Ok(format!("Synced {} holdings from Zerodha", synced))
}

// ============================================================================
// CIBIL score commands
// ============================================================================

#[tauri::command]
pub async fn finance_save_cibil(
    state: State<'_, AppStateHandle>,
    score: i32,
) -> Result<(), String> {
    if !(300..=900).contains(&score) {
        return Err("CIBIL score must be between 300 and 900".to_string());
    }

    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();
    let value = format!("{}|{}", score, now);

    conn.execute(
        "INSERT INTO config (key, value, updated_at)
         VALUES ('cibil_score', ?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?1, updated_at = ?2",
        rusqlite::params![value, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn finance_get_cibil(
    state: State<'_, AppStateHandle>,
) -> Result<Option<CibilResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let result: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'cibil_score'",
            [],
            |row| row.get(0),
        )
        .ok();

    match result {
        Some(val) => {
            let parts: Vec<&str> = val.splitn(2, '|').collect();
            if parts.len() == 2 {
                let score: i32 = parts[0]
                    .parse()
                    .map_err(|_| "Invalid CIBIL score in database".to_string())?;
                Ok(Some(CibilResponse {
                    score,
                    updated_at: parts[1].to_string(),
                }))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
    }
}

// ============================================================================
// Fitness commands
// ============================================================================

#[tauri::command]
pub async fn fitness_add_habit(
    state: State<'_, AppStateHandle>,
    name: String,
    frequency: Option<String>,
    description: Option<String>,
) -> Result<FitnessHabitResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let frequency = frequency.unwrap_or_else(|| "daily".to_string());
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO fitness_habits (id, name, description, frequency, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, name, description, frequency, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(FitnessHabitResponse {
        id,
        name,
        description,
        frequency,
        created_at: now,
        completed_today: false,
    })
}

#[tauri::command]
pub async fn fitness_list_habits(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<FitnessHabitResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let mut stmt = conn
        .prepare(
            "SELECT h.id, h.name, h.description, h.frequency, h.created_at,
                    EXISTS(
                        SELECT 1 FROM fitness_habit_completions c
                        WHERE c.habit_id = h.id AND date(c.completed_at) = date(?1)
                    ) as completed_today
             FROM fitness_habits h ORDER BY h.created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![today], |row| {
            Ok(FitnessHabitResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                frequency: row.get(3)?,
                created_at: row.get(4)?,
                completed_today: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut habits = Vec::new();
    for row in rows {
        habits.push(row.map_err(|e| e.to_string())?);
    }
    Ok(habits)
}

#[tauri::command]
pub async fn fitness_toggle_habit(
    state: State<'_, AppStateHandle>,
    habit_id: String,
) -> Result<bool, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Check if already completed today
    let already_completed: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM fitness_habit_completions
                WHERE habit_id = ?1 AND date(completed_at) = date(?2)
            )",
            rusqlite::params![habit_id, today],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if already_completed {
        // Un-complete: remove today's completion
        conn.execute(
            "DELETE FROM fitness_habit_completions
             WHERE habit_id = ?1 AND date(completed_at) = date(?2)",
            rusqlite::params![habit_id, today],
        )
        .map_err(|e| e.to_string())?;
        Ok(false) // now uncompleted
    } else {
        // Complete: add a completion record
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO fitness_habit_completions (id, habit_id, completed_at)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![id, habit_id, now],
        )
        .map_err(|e| e.to_string())?;
        Ok(true) // now completed
    }
}

#[derive(Debug, Deserialize)]
pub struct LogMetricRequest {
    pub weight_kg: Option<f64>,
    pub body_fat_pct: Option<f64>,
    pub steps: Option<i64>,
    pub heart_rate_avg: Option<i64>,
    pub sleep_hours: Option<f64>,
    pub sleep_quality: Option<i64>,
    pub water_ml: Option<i64>,
    pub calories_in: Option<i64>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn fitness_log_metric(
    state: State<'_, AppStateHandle>,
    metric: LogMetricRequest,
) -> Result<FitnessMetricResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO fitness_metrics
         (id, date, weight_kg, body_fat_pct, steps, heart_rate_avg, sleep_hours,
          sleep_quality, water_ml, calories_in, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            id,
            today,
            metric.weight_kg,
            metric.body_fat_pct,
            metric.steps,
            metric.heart_rate_avg,
            metric.sleep_hours,
            metric.sleep_quality,
            metric.water_ml,
            metric.calories_in,
            metric.notes,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(FitnessMetricResponse {
        id,
        date: today,
        weight_kg: metric.weight_kg,
        body_fat_pct: metric.body_fat_pct,
        steps: metric.steps,
        heart_rate_avg: metric.heart_rate_avg,
        sleep_hours: metric.sleep_hours,
        sleep_quality: metric.sleep_quality,
        water_ml: metric.water_ml,
        calories_in: metric.calories_in,
        notes: metric.notes,
        created_at: now,
    })
}

#[tauri::command]
pub async fn fitness_get_metrics(
    state: State<'_, AppStateHandle>,
    days: Option<u32>,
) -> Result<Vec<FitnessMetricResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let days = days.unwrap_or(30);
    let since = format!("-{} days", days);

    let mut stmt = conn
        .prepare(
            "SELECT id, date, weight_kg, body_fat_pct, steps, heart_rate_avg, sleep_hours,
                    sleep_quality, water_ml, calories_in, notes, created_at
             FROM fitness_metrics WHERE date >= date('now', ?1)
             ORDER BY date DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![since], |row| {
            Ok(FitnessMetricResponse {
                id: row.get(0)?,
                date: row.get(1)?,
                weight_kg: row.get(2)?,
                body_fat_pct: row.get(3)?,
                steps: row.get(4)?,
                heart_rate_avg: row.get(5)?,
                sleep_hours: row.get(6)?,
                sleep_quality: row.get(7)?,
                water_ml: row.get(8)?,
                calories_in: row.get(9)?,
                notes: row.get(10)?,
                created_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut metrics = Vec::new();
    for row in rows {
        metrics.push(row.map_err(|e| e.to_string())?);
    }
    Ok(metrics)
}

#[tauri::command]
pub async fn fitness_get_dashboard(
    state: State<'_, AppStateHandle>,
) -> Result<FitnessDashboard, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let total_habits: u64 = conn
        .query_row("SELECT COUNT(*) FROM fitness_habits", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let habits_completed_today: u64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT habit_id) FROM fitness_habit_completions
             WHERE date(completed_at) = date(?1)",
            rusqlite::params![today],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    // Simple streak: count consecutive days with at least one habit completed
    // going backwards from today
    let current_streak: u64 = conn
        .query_row(
            "WITH RECURSIVE dates(d, streak) AS (
                SELECT date('now'), 0
                UNION ALL
                SELECT date(d, '-1 day'), streak + 1 FROM dates
                WHERE EXISTS(
                    SELECT 1 FROM fitness_habit_completions
                    WHERE date(completed_at) = dates.d
                )
                AND streak < 365
             )
             SELECT MAX(streak) FROM dates",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let latest_weight_kg: Option<f64> = conn
        .query_row(
            "SELECT weight_kg FROM fitness_metrics WHERE weight_kg IS NOT NULL
             ORDER BY date DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    let avg_steps_7d: Option<f64> = conn
        .query_row(
            "SELECT AVG(steps) FROM fitness_metrics
             WHERE steps IS NOT NULL AND date >= date('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let avg_sleep_7d: Option<f64> = conn
        .query_row(
            "SELECT AVG(sleep_hours) FROM fitness_metrics
             WHERE sleep_hours IS NOT NULL AND date >= date('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let total_water_today: Option<i64> = conn
        .query_row(
            "SELECT SUM(water_ml) FROM fitness_metrics WHERE date = ?1 AND water_ml IS NOT NULL",
            rusqlite::params![today],
            |row| row.get(0),
        )
        .ok()
        .flatten();

    let workouts_this_week: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM fitness_workouts WHERE date >= date('now', '-7 days')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(FitnessDashboard {
        total_habits,
        habits_completed_today,
        current_streak,
        latest_weight_kg,
        avg_steps_7d,
        avg_sleep_7d,
        total_water_today,
        workouts_this_week,
    })
}

// ============================================================================
// Reader persistence commands (DB-backed)
// ============================================================================

#[tauri::command]
pub async fn reader_import_book(
    state: State<'_, AppStateHandle>,
    path: String,
) -> Result<ReaderBookResponse, String> {
    let book_path = PathBuf::from(&path);
    if !book_path.exists() {
        return Err(format!("File not found: {}", path));
    }

    let ext = book_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Extract metadata for epub files
    let (title, authors) = if ext == "epub" {
        match epub::doc::EpubDoc::new(&book_path) {
            Ok(doc) => {
                let title = doc
                    .mdata("title")
                    .map(|m| m.value.clone())
                    .unwrap_or_else(|| {
                        book_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    });
                let authors = doc
                    .mdata("creator")
                    .map(|m| m.value.clone())
                    .unwrap_or_default();
                (title, authors)
            }
            Err(_) => (
                book_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                String::new(),
            ),
        }
    } else {
        (
            book_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string(),
            String::new(),
        )
    };

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    // Check if book with this path already exists
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM reader_books WHERE file_path = ?1",
            rusqlite::params![path],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_id) = existing {
        // Return existing book
        return conn
            .query_row(
                "SELECT id, title, authors, file_path, format, cover_path, pages,
                        current_position, progress, rating, favorite, tags, added_at, last_read_at
                 FROM reader_books WHERE id = ?1",
                rusqlite::params![existing_id],
                |row| {
                    Ok(ReaderBookResponse {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        authors: row.get(2)?,
                        file_path: row.get(3)?,
                        format: row.get(4)?,
                        cover_path: row.get(5)?,
                        pages: row.get(6)?,
                        current_position: row.get(7)?,
                        progress: row.get::<_, f64>(8).unwrap_or(0.0),
                        rating: row.get(9)?,
                        favorite: row.get::<_, bool>(10).unwrap_or(false),
                        tags: row.get(11)?,
                        added_at: row.get(12)?,
                        last_read_at: row.get(13)?,
                    })
                },
            )
            .map_err(|e| e.to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Extract cover image for EPUB files and save to data dir
    let cover_path: Option<String> = if ext == "epub" {
        if let Ok(mut doc) = epub::doc::EpubDoc::new(&book_path) {
            if let Some((cover_data, _mime)) = doc.get_cover() {
                let covers_dir = st.data_dir.join("covers");
                let _ = std::fs::create_dir_all(&covers_dir);
                let cover_file = covers_dir.join(format!("{}.jpg", id));
                if std::fs::write(&cover_file, &cover_data).is_ok() {
                    Some(cover_file.to_string_lossy().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    conn.execute(
        "INSERT INTO reader_books (id, title, authors, file_path, format, cover_path, progress, added_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
        rusqlite::params![id, title, authors, path, ext, cover_path, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(ReaderBookResponse {
        id,
        title: Some(title),
        authors: if authors.is_empty() {
            None
        } else {
            Some(authors)
        },
        file_path: path,
        format: Some(ext),
        cover_path,
        pages: None,
        current_position: None,
        progress: 0.0,
        rating: None,
        favorite: false,
        tags: None,
        added_at: now,
        last_read_at: None,
    })
}

#[tauri::command]
pub async fn reader_get_library(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<ReaderBookResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, title, authors, file_path, format, cover_path, pages,
                    current_position, progress, rating, favorite, tags, added_at, last_read_at
             FROM reader_books ORDER BY added_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(ReaderBookResponse {
                id: row.get(0)?,
                title: row.get(1)?,
                authors: row.get(2)?,
                file_path: row.get(3)?,
                format: row.get(4)?,
                cover_path: row.get(5)?,
                pages: row.get(6)?,
                current_position: row.get(7)?,
                progress: row.get::<_, f64>(8).unwrap_or(0.0),
                rating: row.get(9)?,
                favorite: row.get::<_, bool>(10).unwrap_or(false),
                tags: row.get(11)?,
                added_at: row.get(12)?,
                last_read_at: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut books = Vec::new();
    for row in rows {
        books.push(row.map_err(|e| e.to_string())?);
    }
    Ok(books)
}

#[tauri::command]
pub async fn reader_update_progress(
    state: State<'_, AppStateHandle>,
    book_id: String,
    progress: f64,
    position: Option<String>,
) -> Result<(), String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE reader_books SET progress = ?1, current_position = ?2, last_read_at = ?3
         WHERE id = ?4",
        rusqlite::params![progress, position, now, book_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn reader_add_annotation(
    state: State<'_, AppStateHandle>,
    book_id: String,
    annotation_type: String,
    chapter_index: Option<i64>,
    text: Option<String>,
    note: Option<String>,
    color: Option<String>,
) -> Result<ReaderAnnotationResponse, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let color = color.unwrap_or_else(|| "yellow".to_string());

    conn.execute(
        "INSERT INTO reader_annotations
         (id, book_id, type, chapter_index, text, note, color, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        rusqlite::params![
            id,
            book_id,
            annotation_type,
            chapter_index,
            text,
            note,
            color,
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(ReaderAnnotationResponse {
        id,
        book_id,
        annotation_type,
        chapter_index,
        start_pos: None,
        end_pos: None,
        text,
        note,
        color,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub async fn reader_get_annotations(
    state: State<'_, AppStateHandle>,
    book_id: String,
) -> Result<Vec<ReaderAnnotationResponse>, String> {
    let state = state.read().await;
    let conn = state.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, book_id, type, chapter_index, start_pos, end_pos,
                    text, note, color, created_at, updated_at
             FROM reader_annotations WHERE book_id = ?1
             ORDER BY chapter_index ASC, created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![book_id], |row| {
            Ok(ReaderAnnotationResponse {
                id: row.get(0)?,
                book_id: row.get(1)?,
                annotation_type: row.get(2)?,
                chapter_index: row.get(3)?,
                start_pos: row.get(4)?,
                end_pos: row.get(5)?,
                text: row.get(6)?,
                note: row.get(7)?,
                color: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut annotations = Vec::new();
    for row in rows {
        annotations.push(row.map_err(|e| e.to_string())?);
    }
    Ok(annotations)
}

// ============================================================================
// Collection commands
// ============================================================================

#[tauri::command]
pub async fn reader_create_collection(
    state: State<'_, AppStateHandle>,
    name: String,
    description: Option<String>,
    color: Option<String>,
) -> Result<CollectionResponse, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let color = color.unwrap_or_else(|| "#0ea5e9".to_string());

    conn.execute(
        "INSERT INTO reader_collections (id, name, description, color, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, name, description, color, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(CollectionResponse {
        id,
        name,
        description,
        color,
        book_count: 0,
        created_at: now,
    })
}

#[tauri::command]
pub async fn reader_list_collections(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<CollectionResponse>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.name, c.description, c.color, c.created_at,
                    COUNT(cb.book_id) as book_count
             FROM reader_collections c
             LEFT JOIN reader_collection_books cb ON c.id = cb.collection_id
             GROUP BY c.id
             ORDER BY c.sort_order ASC, c.created_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(CollectionResponse {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                color: row.get(3)?,
                created_at: row.get(4)?,
                book_count: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut collections = Vec::new();
    for row in rows {
        collections.push(row.map_err(|e| e.to_string())?);
    }
    Ok(collections)
}

#[tauri::command]
pub async fn reader_add_to_collection(
    state: State<'_, AppStateHandle>,
    collection_id: String,
    book_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO reader_collection_books (collection_id, book_id)
         VALUES (?1, ?2)",
        rusqlite::params![collection_id, book_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn reader_remove_from_collection(
    state: State<'_, AppStateHandle>,
    collection_id: String,
    book_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM reader_collection_books
         WHERE collection_id = ?1 AND book_id = ?2",
        rusqlite::params![collection_id, book_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn reader_get_collection_books(
    state: State<'_, AppStateHandle>,
    collection_id: String,
) -> Result<Vec<ReaderBookResponse>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT b.id, b.title, b.authors, b.file_path, b.format, b.cover_path, b.pages,
                    b.current_position, b.progress, b.rating, b.favorite, b.tags,
                    b.added_at, b.last_read_at
             FROM reader_books b
             INNER JOIN reader_collection_books cb ON b.id = cb.book_id
             WHERE cb.collection_id = ?1
             ORDER BY cb.added_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(rusqlite::params![collection_id], |row| {
            Ok(ReaderBookResponse {
                id: row.get(0)?,
                title: row.get(1)?,
                authors: row.get(2)?,
                file_path: row.get(3)?,
                format: row.get(4)?,
                cover_path: row.get(5)?,
                pages: row.get(6)?,
                current_position: row.get(7)?,
                progress: row.get::<_, f64>(8).unwrap_or(0.0),
                rating: row.get(9)?,
                favorite: row.get::<_, bool>(10).unwrap_or(false),
                tags: row.get(11)?,
                added_at: row.get(12)?,
                last_read_at: row.get(13)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut books = Vec::new();
    for row in rows {
        books.push(row.map_err(|e| e.to_string())?);
    }
    Ok(books)
}

#[tauri::command]
pub async fn reader_delete_collection(
    state: State<'_, AppStateHandle>,
    collection_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    // Delete association rows first (in case FK cascade isn't enforced)
    conn.execute(
        "DELETE FROM reader_collection_books WHERE collection_id = ?1",
        rusqlite::params![collection_id],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM reader_collections WHERE id = ?1",
        rusqlite::params![collection_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn reader_scan_directory(
    state: State<'_, AppStateHandle>,
    path: String,
) -> Result<Vec<ReaderBookResponse>, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let book_extensions = [
        "epub", "pdf", "mobi", "azw3", "fb2", "djvu", "cbz", "cbr", "txt", "md", "markdown",
        "html", "htm",
    ];
    let mut book_paths = Vec::new();

    // Collect book files from the directory (non-recursive for now, then recurse)
    fn collect_books(dir: &PathBuf, exts: &[&str], out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    collect_books(&path, exts, out);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if exts.contains(&ext.to_lowercase().as_str()) {
                        out.push(path);
                    }
                }
            }
        }
    }

    collect_books(&dir, &book_extensions, &mut book_paths);

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let mut imported = Vec::new();
    for book_path in book_paths {
        let path_str = book_path.to_string_lossy().to_string();
        let ext = book_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Check if already imported
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM reader_books WHERE file_path = ?1",
                rusqlite::params![path_str],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing_id) = existing {
            if let Ok(book) = conn.query_row(
                "SELECT id, title, authors, file_path, format, cover_path, pages,
                        current_position, progress, rating, favorite, tags, added_at, last_read_at
                 FROM reader_books WHERE id = ?1",
                rusqlite::params![existing_id],
                |row| {
                    Ok(ReaderBookResponse {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        authors: row.get(2)?,
                        file_path: row.get(3)?,
                        format: row.get(4)?,
                        cover_path: row.get(5)?,
                        pages: row.get(6)?,
                        current_position: row.get(7)?,
                        progress: row.get::<_, f64>(8).unwrap_or(0.0),
                        rating: row.get(9)?,
                        favorite: row.get::<_, bool>(10).unwrap_or(false),
                        tags: row.get(11)?,
                        added_at: row.get(12)?,
                        last_read_at: row.get(13)?,
                    })
                },
            ) {
                imported.push(book);
            }
            continue;
        }

        // Extract metadata for epub
        let (title, authors) = if ext == "epub" {
            match epub::doc::EpubDoc::new(&book_path) {
                Ok(doc) => {
                    let title = doc
                        .mdata("title")
                        .map(|m| m.value.clone())
                        .unwrap_or_else(|| {
                            book_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Unknown")
                                .to_string()
                        });
                    let authors = doc
                        .mdata("creator")
                        .map(|m| m.value.clone())
                        .unwrap_or_default();
                    (title, authors)
                }
                Err(_) => (
                    book_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                    String::new(),
                ),
            }
        } else {
            (
                book_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                String::new(),
            )
        };

        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();

        if conn
            .execute(
                "INSERT INTO reader_books (id, title, authors, file_path, format, progress, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
                rusqlite::params![id, title, authors, path_str, ext, now],
            )
            .is_ok()
        {
            imported.push(ReaderBookResponse {
                id,
                title: Some(title),
                authors: if authors.is_empty() {
                    None
                } else {
                    Some(authors)
                },
                file_path: path_str,
                format: Some(ext),
                cover_path: None,
                pages: None,
                current_position: None,
                progress: 0.0,
                rating: None,
                favorite: false,
                tags: None,
                added_at: now,
                last_read_at: None,
            });
        }
    }

    Ok(imported)
}

// ============================================================================
// O'Reilly connection commands
// ============================================================================

#[derive(Debug, Serialize)]
pub struct OreillyConnectResult {
    pub success: bool,
    pub message: String,
}

#[tauri::command]
pub async fn oreilly_connect_chrome(
    state: State<'_, AppStateHandle>,
) -> Result<OreillyConnectResult, String> {
    // Try to find Chrome cookies for oreilly.com
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());

    // Chrome cookie paths (Linux)
    let cookie_paths = vec![
        format!("{}/.config/google-chrome/Default/Cookies", home),
        format!("{}/.config/google-chrome/Profile 1/Cookies", home),
        format!("{}/.config/chromium/Default/Cookies", home),
        format!("{}/.config/brave/Default/Cookies", home),
    ];

    let mut found_path: Option<String> = None;
    for p in &cookie_paths {
        if std::path::Path::new(p).exists() {
            found_path = Some(p.clone());
            break;
        }
    }

    let cookie_db_path = match found_path {
        Some(p) => p,
        None => {
            return Ok(OreillyConnectResult {
                success: false,
                message: "Chrome cookie database not found. Make sure Chrome/Chromium is installed and you've logged into O'Reilly.".to_string(),
            });
        }
    };

    tracing::info!("Reading Chrome cookies from: {}", cookie_db_path);

    // Copy the cookie DB to a temp file (Chrome locks it)
    let temp_dir = std::env::temp_dir();
    let temp_cookie_path = temp_dir.join("minion_chrome_cookies_tmp");
    std::fs::copy(&cookie_db_path, &temp_cookie_path).map_err(|e| {
        format!(
            "Failed to copy Chrome cookie DB: {}. Try closing Chrome first.",
            e
        )
    })?;

    // Open the copied cookie DB
    let conn = rusqlite::Connection::open(&temp_cookie_path).map_err(|e| e.to_string())?;

    // Check for oreilly.com cookies
    let cookie_count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM cookies WHERE host_key LIKE '%oreilly.com%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_cookie_path);

    if cookie_count > 0 {
        // Store the cookie DB path in our config for future use
        let st = state.read().await;
        let db_conn = st.db.get().map_err(|e| e.to_string())?;
        db_conn
            .execute(
                "INSERT OR REPLACE INTO config (key, value) VALUES ('oreilly_cookie_source', ?1)",
                rusqlite::params![cookie_db_path],
            )
            .map_err(|e| e.to_string())?;

        tracing::info!("Found {} O'Reilly cookies in Chrome", cookie_count);

        Ok(OreillyConnectResult {
            success: true,
            message: format!(
                "Found active O'Reilly session in Chrome ({} cookies). You can now search and download books.",
                cookie_count
            ),
        })
    } else {
        Ok(OreillyConnectResult {
            success: false,
            message: "No O'Reilly cookies found in Chrome. Please log into learning.oreilly.com in Chrome first, then try again.".to_string(),
        })
    }
}

#[tauri::command]
pub async fn oreilly_connect_sso() -> Result<OreillyConnectResult, String> {
    // Open O'Reilly login page in the default browser
    let login_url = "https://www.oreilly.com/member/login/";

    std::process::Command::new("xdg-open")
        .arg(login_url)
        .spawn()
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    Ok(OreillyConnectResult {
        success: false,
        message: "Opened O'Reilly login in your browser. Complete the SSO login (ACM/institutional), then click 'Use Chrome Session' to import the session.".to_string(),
    })
}

#[tauri::command]
pub async fn oreilly_open_browser(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    WebviewWindowBuilder::new(
        &app,
        "oreilly",
        WebviewUrl::External("https://learning.oreilly.com".parse().unwrap()),
    )
    .title("O'Reilly Learning - MINION")
    .inner_size(1100.0, 800.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn oreilly_connect_manual(
    email: String,
    password: String,
) -> Result<OreillyConnectResult, String> {
    // For manual login, we'd need to POST to O'Reilly's auth endpoint
    // This doesn't work with SSO accounts but works for direct O'Reilly accounts
    tracing::info!("Manual O'Reilly login attempt for: {}", email);

    Ok(OreillyConnectResult {
        success: false,
        message: "Manual login requires direct O'Reilly credentials (not SSO). For ACM/institutional access, use 'Sign in with SSO' then 'Use Chrome Session'.".to_string(),
    })
}

#[tauri::command]
pub async fn oreilly_logout(state: State<'_, AppStateHandle>) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM config WHERE key LIKE 'oreilly_%'", [])
        .map_err(|e| e.to_string())?;
    tracing::info!("O'Reilly session cleared");
    Ok(())
}

// ============================================================================
// AI / LLM commands
// ============================================================================

#[tauri::command]
pub async fn ai_test_connection(url: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/tags", url))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;
    let body = resp.text().await.map_err(|e| e.to_string())?;
    Ok(body)
}

#[tauri::command]
pub async fn ai_analyze_health(
    state: State<'_, AppStateHandle>,
    metrics_json: String,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    // Read LLM config from DB
    let ollama_url: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'ai_ollama_url'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "http://192.168.1.10:11434".to_string());

    let model: String = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'ai_model'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "llama3.2:3b".to_string());

    drop(conn);
    drop(st);

    let prompt = format!(
        "You are a health and fitness AI assistant. Analyze the following health metrics \
         and provide personalized insights, recommendations, and areas of concern. \
         Be concise but thorough. Format your response with clear sections.\n\n\
         Health Metrics:\n{}\n\n\
         Please provide:\n\
         1. Overall health assessment\n\
         2. Key observations\n\
         3. Actionable recommendations\n\
         4. Areas that need attention",
        metrics_json
    );

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/generate", ollama_url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {}", e))?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let response = body
        .get("response")
        .and_then(|v| v.as_str())
        .unwrap_or("No response from model")
        .to_string();

    Ok(response)
}

// ============================================================================
// Google Fit integration
// ============================================================================

#[tauri::command]
pub async fn gfit_open_auth(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};

    let auth_url = "https://accounts.google.com/o/oauth2/v2/auth?\
        scope=https://www.googleapis.com/auth/fitness.activity.read+\
        https://www.googleapis.com/auth/fitness.body.read+\
        https://www.googleapis.com/auth/fitness.sleep.read+\
        https://www.googleapis.com/auth/fitness.heart_rate.read&\
        response_type=code&\
        access_type=offline&\
        redirect_uri=urn:ietf:wg:oauth:2.0:oob";

    WebviewWindowBuilder::new(
        &app,
        "google-fit-auth",
        WebviewUrl::External(auth_url.parse().unwrap()),
    )
    .title("Google Fit - Sign In")
    .inner_size(500.0, 700.0)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn gfit_sync(state: State<'_, AppStateHandle>) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let token: Option<String> = conn
        .query_row(
            "SELECT value FROM config WHERE key = 'gfit_access_token'",
            [],
            |row| row.get(0),
        )
        .ok();

    let token = token.ok_or("Not connected to Google Fit. Please connect first in Settings.")?;

    let client = reqwest::Client::new();
    let now_ms = chrono::Utc::now().timestamp_millis();
    let day_ago_ms = now_ms - 86_400_000;

    let steps_body = serde_json::json!({
        "aggregateBy": [{"dataTypeName": "com.google.step_count.delta"}],
        "bucketByTime": {"durationMillis": 86400000},
        "startTimeMillis": day_ago_ms,
        "endTimeMillis": now_ms
    });

    let resp = client
        .post("https://www.googleapis.com/fitness/v1/users/me/dataset:aggregate")
        .bearer_auth(&token)
        .json(&steps_body)
        .send()
        .await
        .map_err(|e| format!("Google Fit API error: {}", e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;

    if !status.is_success() {
        return Err(format!("Google Fit API returned {}: {}", status, body));
    }

    Ok(format!(
        "Synced successfully. Raw: {}",
        &body[..body.len().min(500)]
    ))
}

#[tauri::command]
pub async fn gfit_save_token(
    state: State<'_, AppStateHandle>,
    token: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (key, value) VALUES ('gfit_access_token', ?1)",
        rusqlite::params![token],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn gfit_check_connected(state: State<'_, AppStateHandle>) -> Result<bool, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM config WHERE key = 'gfit_access_token')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);
    Ok(exists)
}

#[tauri::command]
pub async fn gfit_disconnect(state: State<'_, AppStateHandle>) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM config WHERE key LIKE 'gfit_%'", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
