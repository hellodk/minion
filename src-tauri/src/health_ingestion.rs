//! Health Vault document ingestion pipeline (week 2).
//!
//! Scans a folder of medical records, hashes + deduplicates, copies files
//! into the app vault, extracts text (digital PDF -> `pdf_extract`, scans
//! and images -> Tesseract OCR via `pdftoppm`/`tesseract`), and persists a
//! manifest + raw-text extraction row per file.
//!
//! Classification and structured extraction (mapping raw text into lab
//! tests, medications, conditions, etc.) happen in week 3 when the LLM
//! layer is wired up.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// File hashing + vault storage
// =====================================================================

/// Stream-hash a file with SHA-256 and return the hex digest.
fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Resolve the health-vault directory under the app data dir.
fn vault_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("health").join("vault").join("documents")
}

/// Copy `src` into `vault` keyed by its SHA (`{sha}.{ext}`) and return the
/// destination path. No-op if the destination already exists.
fn copy_to_vault(src: &Path, vault: &Path, sha: &str) -> Result<PathBuf, String> {
    std::fs::create_dir_all(vault).map_err(|e| e.to_string())?;
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("bin");
    let dest = vault.join(format!("{}.{}", sha, ext));
    if !dest.exists() {
        std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
    }
    Ok(dest)
}

// =====================================================================
// Discovery
// =====================================================================

/// Recursively walk `root` collecting supported medical-document files.
fn discover_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let exts = [
        "pdf", "png", "jpg", "jpeg", "tiff", "tif", "heic", "webp", "txt",
    ];
    fn walk(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, exts, out);
                } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if exts.contains(&ext.to_lowercase().as_str()) {
                        out.push(p);
                    }
                }
            }
        }
    }
    walk(root, &exts, &mut out);
    out
}

// =====================================================================
// Text extraction
// =====================================================================

fn extract_text_pdf(path: &Path) -> Result<String, String> {
    pdf_extract::extract_text(path).map_err(|e| format!("PDF extract: {}", e))
}

/// Run `tesseract <image> - -l eng` and return stdout.
fn ocr_image(path: &Path) -> Result<String, String> {
    let output = std::process::Command::new("tesseract")
        .arg(path)
        .arg("-") // output to stdout
        .arg("-l")
        .arg("eng")
        .output()
        .map_err(|e| format!("Tesseract not found: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// For scanned PDFs: rasterize pages with `pdftoppm`, OCR each PNG, concat.
fn ocr_pdf_via_pdftoppm(path: &Path) -> Result<String, String> {
    let tmpdir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let out_prefix = tmpdir.path().join("page");
    let out_status = std::process::Command::new("pdftoppm")
        .arg("-r")
        .arg("200") // DPI
        .arg("-png")
        .arg(path)
        .arg(&out_prefix)
        .output()
        .map_err(|e| format!("pdftoppm not found: {}", e))?;
    if !out_status.status.success() {
        return Err(String::from_utf8_lossy(&out_status.stderr).to_string());
    }
    let mut entries: Vec<_> = std::fs::read_dir(tmpdir.path())
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("png"))
        .collect();
    entries.sort_by_key(|e| e.path());
    let mut full_text = String::new();
    for entry in entries {
        let text = ocr_image(&entry.path()).unwrap_or_default();
        full_text.push_str(&text);
        full_text.push_str("\n\n--- PAGE BREAK ---\n\n");
    }
    Ok(full_text)
}

/// Top-level extraction dispatcher. PDFs try the digital path first; if the
/// result is suspiciously short (< 200 chars) we assume it is a scan and
/// fall back to OCR.
pub async fn extract_text_from_file(path: &Path) -> Result<String, String> {
    let p = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "pdf" => {
                let digital = extract_text_pdf(&p).unwrap_or_default();
                if digital.trim().len() > 200 {
                    Ok(digital)
                } else {
                    ocr_pdf_via_pdftoppm(&p)
                }
            }
            "txt" => std::fs::read_to_string(&p).map_err(|e| e.to_string()),
            "png" | "jpg" | "jpeg" | "tiff" | "tif" | "webp" => ocr_image(&p),
            _ => Err(format!("Unsupported extension: {}", ext)),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

fn detect_mime(path: &Path) -> Option<String> {
    let ext = path.extension().and_then(|e| e.to_str())?.to_lowercase();
    Some(
        match ext.as_str() {
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "tiff" | "tif" => "image/tiff",
            "txt" => "text/plain",
            "webp" => "image/webp",
            _ => return None,
        }
        .to_string(),
    )
}

// =====================================================================
// Serializable types
// =====================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct DiscoveredFile {
    pub path: String,
    pub size_bytes: u64,
    pub extension: String,
    pub already_imported: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestionJob {
    pub id: String,
    pub patient_id: Option<String>,
    pub source_folder: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub total_files: i64,
    pub processed_files: i64,
    pub skipped_files: i64,
    pub failed_files: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub id: String,
    pub sha256: String,
    pub original_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractionEntry {
    pub id: String,
    pub file_id: String,
    pub document_type: Option<String>,
    pub classification_confidence: Option<f64>,
    pub raw_text: Option<String>,
    pub extracted_json: Option<String>,
    pub user_reviewed: bool,
}

// =====================================================================
// Tauri commands
// =====================================================================

/// Scan a folder, hash each supported file, and return a preview list so
/// the UI can let the user untick anything they don't want imported.
#[tauri::command]
pub async fn health_discover_folder(
    state: State<'_, AppStateHandle>,
    folder: String,
) -> Result<Vec<DiscoveredFile>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let root = PathBuf::from(&folder);
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", folder));
    }
    let files = discover_files(&root);
    let mut result = Vec::with_capacity(files.len());
    for path in files {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let sha = sha256_file(&path)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM file_manifest WHERE sha256 = ?1)",
                rusqlite::params![sha],
                |row| row.get(0),
            )
            .unwrap_or(false);
        result.push(DiscoveredFile {
            path: path.to_string_lossy().to_string(),
            size_bytes: size,
            extension: ext,
            already_imported: exists,
        });
    }
    Ok(result)
}

/// Start an ingestion job. Returns the job id immediately and processes
/// files in a background task. Emits `health-ingestion-progress` after each
/// file and `health-ingestion-complete` when finished.
#[tauri::command]
pub async fn health_start_ingestion(
    app: tauri::AppHandle,
    state: State<'_, AppStateHandle>,
    patient_id: String,
    source_folder: String,
    selected_paths: Vec<String>,
) -> Result<String, String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let total = selected_paths.len() as i64;

    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO ingestion_jobs (id, patient_id, source_folder, started_at,
             status, total_files) VALUES (?1, ?2, ?3, ?4, 'running', ?5)",
            rusqlite::params![job_id, patient_id, source_folder, now, total],
        )
        .map_err(|e| e.to_string())?;
    }

    let job_id_bg = job_id.clone();
    let state_bg = state.inner().clone();
    let app_bg = app.clone();

    // Cache a DB handle *outside* the RwLock. The r2d2 pool is already
    // Arc-based, so this clone is cheap and lets the ingestion hot path
    // do `db.get()` instead of `state.read().await.db.get()` on every
    // single file — the lock becomes invisible to concurrent reads.
    let db_bg: minion_db::Database = {
        let st = state.read().await;
        st.db.clone()
    };

    // Supervisor: catches panics from the worker so a partial failure
    // doesn't leave the row stuck at status='running'. The inner task
    // does its own status='completed' write on the happy path; only the
    // panic path is patched up here.
    let job_id_supervised = job_id.clone();
    let db_supervised = db_bg.clone();
    let worker = tokio::spawn(async move {
        let app_data_dir = app_bg
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir());
        let vault = vault_dir(&app_data_dir);
        let mut processed = 0i64;
        let mut skipped = 0i64;
        let mut failed = 0i64;

        // Helper closure: bump the counter, persist, and emit progress
        // event in one shot. Used by every loop branch (success + each
        // failure mode) so the UI never sees a stalled bar. Takes the
        // lock-free DB handle so we don't serialize on the app state
        // RwLock while a 1000-file ingestion is running.
        let emit_progress = |db: minion_db::Database,
                             app: tauri::AppHandle,
                             job_id: String,
                             processed: i64,
                             skipped: i64,
                             failed: i64,
                             total: i64,
                             current: String| async move {
            let _ = update_progress_db(&db, &job_id, processed, skipped, failed);
            let _ = app.emit(
                "health-ingestion-progress",
                serde_json::json!({
                    "job_id": job_id,
                    "processed": processed,
                    "total": total,
                    "skipped": skipped,
                    "failed": failed,
                    "current": current,
                }),
            );
        };

        // Check once up-front if an extraction endpoint is configured + healthy.
        // If it is, we auto-classify each file inline (Option A). Otherwise we
        // just persist raw_text and the user can trigger classification later.
        let auto_classify =
            crate::health_classify::is_extract_endpoint_healthy(&state_bg).await;
        if auto_classify {
            tracing::info!("health ingestion: auto-classification enabled");
        } else {
            tracing::info!(
                "health ingestion: no healthy LLM endpoint — skipping auto-classify"
            );
        }

        for path_str in &selected_paths {
            let path = PathBuf::from(path_str);

            // Hash
            let sha = match sha256_file(&path) {
                Ok(s) => s,
                Err(e) => {
                    failed += 1;
                    processed += 1;
                    tracing::warn!("hash failed for {}: {}", path_str, e);
                    emit_progress(
                        db_bg.clone(),
                        app_bg.clone(),
                        job_id_bg.clone(),
                        processed,
                        skipped,
                        failed,
                        total,
                        path_str.clone(),
                    )
                    .await;
                    continue;
                }
            };

            // Dedup check
            let exists: bool = match db_bg.get() {
                Ok(conn) => conn
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM file_manifest WHERE sha256 = ?1)",
                        rusqlite::params![sha],
                        |row| row.get(0),
                    )
                    .unwrap_or(false),
                Err(e) => {
                    failed += 1;
                    processed += 1;
                    tracing::warn!("db conn failed: {}", e);
                    emit_progress(
                        db_bg.clone(),
                        app_bg.clone(),
                        job_id_bg.clone(),
                        processed,
                        skipped,
                        failed,
                        total,
                        path_str.clone(),
                    )
                    .await;
                    continue;
                }
            };

            if exists {
                skipped += 1;
                processed += 1;
                emit_progress(
                    db_bg.clone(),
                    app_bg.clone(),
                    job_id_bg.clone(),
                    processed,
                    skipped,
                    failed,
                    total,
                    path_str.clone(),
                )
                .await;
                continue;
            }

            // Copy to vault
            let stored = match copy_to_vault(&path, &vault, &sha) {
                Ok(p) => p,
                Err(e) => {
                    failed += 1;
                    processed += 1;
                    tracing::warn!("vault copy failed: {}", e);
                    emit_progress(
                        db_bg.clone(),
                        app_bg.clone(),
                        job_id_bg.clone(),
                        processed,
                        skipped,
                        failed,
                        total,
                        path_str.clone(),
                    )
                    .await;
                    continue;
                }
            };

            // Insert manifest row
            let file_id = uuid::Uuid::new_v4().to_string();
            let size = std::fs::metadata(&path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);
            let mime = detect_mime(&path);
            let now2 = chrono::Utc::now().to_rfc3339();

            match db_bg.get() {
                Ok(conn) => {
                    let _ = conn.execute(
                        "INSERT INTO file_manifest (id, sha256, original_path, stored_path,
                         mime_type, size_bytes, status, patient_id, job_id, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'extracting', ?7, ?8, ?9)",
                        rusqlite::params![
                            file_id,
                            sha,
                            path_str,
                            stored.to_string_lossy().to_string(),
                            mime,
                            size,
                            patient_id,
                            job_id_bg,
                            now2,
                        ],
                    );
                }
                Err(e) => {
                    failed += 1;
                    processed += 1;
                    tracing::warn!("db conn failed: {}", e);
                    emit_progress(
                        db_bg.clone(),
                        app_bg.clone(),
                        job_id_bg.clone(),
                        processed,
                        skipped,
                        failed,
                        total,
                        path_str.clone(),
                    )
                    .await;
                    continue;
                }
            }

            // Extract text
            let text = match extract_text_from_file(&stored).await {
                Ok(t) => t,
                Err(e) => {
                    if let Ok(conn) = db_bg.get() {
                        let _ = conn.execute(
                            "UPDATE file_manifest SET status = 'failed', error = ?1 WHERE id = ?2",
                            rusqlite::params![e, file_id],
                        );
                    }
                    failed += 1;
                    processed += 1;
                    emit_progress(
                        db_bg.clone(),
                        app_bg.clone(),
                        job_id_bg.clone(),
                        processed,
                        skipped,
                        failed,
                        total,
                        path_str.clone(),
                    )
                    .await;
                    continue;
                }
            };

            // Persist raw text. If an LLM endpoint is configured we also
            // classify + extract right now (Option A); otherwise the user
            // can run `health_classify_pending` later.
            let extraction_id = uuid::Uuid::new_v4().to_string();
            if let Ok(conn) = db_bg.get() {
                let _ = conn.execute(
                    "INSERT INTO document_extractions (id, file_id, raw_text, extracted_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![
                        extraction_id,
                        file_id,
                        text,
                        chrono::Utc::now().to_rfc3339()
                    ],
                );
                let _ = conn.execute(
                    "UPDATE file_manifest SET status = 'extracted' WHERE id = ?1",
                    rusqlite::params![file_id],
                );
            }

            if auto_classify {
                if let Err(e) = crate::health_classify::process_document(
                    &state_bg,
                    &file_id,
                    &text,
                    Some("health_extract"),
                )
                .await
                {
                    tracing::warn!("auto-classify failed for {}: {}", file_id, e);
                }
            }

            processed += 1;
            emit_progress(
                db_bg.clone(),
                app_bg.clone(),
                job_id_bg.clone(),
                processed,
                skipped,
                failed,
                total,
                path_str.clone(),
            )
            .await;
        }

        // Mark job done. We always reach this; a panic inside the loop
        // would skip past, so the supervisor wrapper installed on the
        // spawn handle below converts a panic into a `status='failed'`
        // update so the row never sticks at `running`.
        if let Ok(conn) = db_bg.get() {
            let _ = conn.execute(
                "UPDATE ingestion_jobs SET status = 'completed',
                 completed_at = ?1, processed_files = ?2, skipped_files = ?3, failed_files = ?4
                 WHERE id = ?5",
                rusqlite::params![
                    chrono::Utc::now().to_rfc3339(),
                    processed,
                    skipped,
                    failed,
                    job_id_bg
                ],
            );
        }

        let _ = app_bg.emit(
            "health-ingestion-complete",
            serde_json::json!({
                "job_id": job_id_bg,
                "processed": processed,
                "skipped": skipped,
                "failed": failed,
            }),
        );
    });

    // Watch the worker; if it panics, mark the job as failed so it isn't
    // stuck on `running` forever.
    tokio::spawn(async move {
        if let Err(join_err) = worker.await {
            if let Ok(conn) = db_supervised.get() {
                let msg = format!("worker panicked: {}", join_err);
                let _ = conn.execute(
                    "UPDATE ingestion_jobs SET status = 'failed',
                     completed_at = ?1, error = ?2 WHERE id = ?3 AND status = 'running'",
                    rusqlite::params![
                        chrono::Utc::now().to_rfc3339(),
                        msg,
                        job_id_supervised
                    ],
                );
            }
        }
    });

    Ok(job_id)
}

/// Lock-free variant: takes the pooled Database handle directly so we
/// never touch the app-state RwLock during a hot ingestion loop.
fn update_progress_db(
    db: &minion_db::Database,
    job_id: &str,
    processed: i64,
    skipped: i64,
    failed: i64,
) -> Result<(), String> {
    let conn = db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE ingestion_jobs SET processed_files = ?1, skipped_files = ?2,
         failed_files = ?3 WHERE id = ?4",
        rusqlite::params![processed, skipped, failed, job_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn health_get_ingestion_job(
    state: State<'_, AppStateHandle>,
    job_id: String,
) -> Result<IngestionJob, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, patient_id, source_folder, started_at, completed_at, status,
         total_files, processed_files, skipped_files, failed_files
         FROM ingestion_jobs WHERE id = ?1",
        rusqlite::params![job_id],
        |row| {
            Ok(IngestionJob {
                id: row.get(0)?,
                patient_id: row.get(1)?,
                source_folder: row.get(2)?,
                started_at: row.get(3)?,
                completed_at: row.get(4)?,
                status: row.get(5)?,
                total_files: row.get(6)?,
                processed_files: row.get(7)?,
                skipped_files: row.get(8)?,
                failed_files: row.get(9)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn health_list_files(
    state: State<'_, AppStateHandle>,
    patient_id: String,
) -> Result<Vec<FileEntry>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, sha256, original_path, mime_type, size_bytes, status, created_at
             FROM file_manifest WHERE patient_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![patient_id], |row| {
            Ok(FileEntry {
                id: row.get(0)?,
                sha256: row.get(1)?,
                original_path: row.get(2)?,
                mime_type: row.get(3)?,
                size_bytes: row.get(4)?,
                status: row.get(5)?,
                created_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn health_get_extraction(
    state: State<'_, AppStateHandle>,
    file_id: String,
) -> Result<Option<ExtractionEntry>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT id, file_id, document_type, classification_confidence,
         raw_text, extracted_json, user_reviewed
         FROM document_extractions WHERE file_id = ?1",
        rusqlite::params![file_id],
        |row| {
            Ok(ExtractionEntry {
                id: row.get(0)?,
                file_id: row.get(1)?,
                document_type: row.get(2)?,
                classification_confidence: row.get(3)?,
                raw_text: row.get(4)?,
                extracted_json: row.get(5)?,
                user_reviewed: row.get::<_, i64>(6)? != 0,
            })
        },
    );
    match result {
        Ok(e) => Ok(Some(e)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn health_delete_file(
    state: State<'_, AppStateHandle>,
    file_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    // Grab stored_path first so we can also remove the vault copy.
    let stored: Option<String> = conn
        .query_row(
            "SELECT stored_path FROM file_manifest WHERE id = ?1",
            rusqlite::params![file_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    if let Some(path) = stored {
        let _ = std::fs::remove_file(&path);
    }
    conn.execute(
        "DELETE FROM file_manifest WHERE id = ?1",
        rusqlite::params![file_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
