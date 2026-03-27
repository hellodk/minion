//! Tauri IPC commands

use crate::state::{AppState, ScanCache, ScanStatus, ScanTask, WatchedDirectory};
use chrono::Utc;
use minion_files::{AnalyticsCalculator, DuplicateFinder, ScanConfig, Scanner};
use minion_reader::formats::{parse_epub, parse_pdf};
use minion_reader::BookFormat;
use serde::{Deserialize, Serialize};
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
                let bytes = progress_bytes_processed.load(std::sync::atomic::Ordering::Relaxed) as u64;

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
        let scan_result = tokio::task::spawn_blocking(move || {
            scanner.scan()
        })
        .await
        .unwrap_or_else(|e| Err(minion_files::Error::Scan(format!("Scan task panicked: {}", e))));

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
                    for (_, indices) in &size_groups {
                        if indices.len() > 1 {
                            needs_hash += indices.len();
                            for &idx in indices {
                                if files_result[idx].sha256.is_none() {
                                    if let Ok(hash) = minion_files::hash::compute_sha256(&files_result[idx].path) {
                                        files_result[idx].sha256 = Some(hash);
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!("Hashed {} of {} files (size-candidate optimization)", needs_hash, files_result.len());

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

        let scan_result = tokio::task::spawn_blocking(move || {
            let mut all_files: Vec<minion_files::FileInfo> = Vec::new();
            let mut total_size: u64 = 0;
            let mut error_count: usize = 0;

            for p in &scan_paths {
                let scan_config = ScanConfig {
                    root: PathBuf::from(p),
                    recursive: true,
                    compute_hashes: false, // Skip hashing for speed, hash only candidates later
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
                    for (_, indices) in &size_groups {
                        if indices.len() > 1 {
                            needs_hash += indices.len();
                            for &idx in indices {
                                if files_result[idx].sha256.is_none() {
                                    if let Ok(hash) = minion_files::hash::compute_sha256(&files_result[idx].path) {
                                        files_result[idx].sha256 = Some(hash);
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!("Hashed {} of {} files (size-candidate optimization)", needs_hash, files_result.len());
                    let finder = DuplicateFinder::default();
                    let dupes = finder.find(&files_result);
                    (files_result, dupes)
                })
                .await
                .unwrap_or_else(|_| (vec![], vec![]));

                let (files_final, duplicates) = dupes_result;
                let duplicates_count = duplicates.len();
                tracing::info!("Found {} duplicate groups across directories", duplicates_count);

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
        return Err(format!("Destination is not a directory: {}", request.destination));
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
            let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let ext = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
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
    let supported_extensions = ["epub", "pdf", "txt", "md", "markdown"];

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
