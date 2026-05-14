//! VS Code-style file explorer backend.
//!
//! Workspace folders are persisted in `file_viewer_workspaces`.
//! The directory tree is loaded lazily: `fv_read_dir` is called per directory
//! when the user expands a node in the frontend.

use crate::state::AppState;
use base64::Engine as _;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct FvWorkspace {
    pub path: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub struct FvEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub extension: Option<String>,
    pub size: u64,
    pub modified: String,
}

#[derive(Debug, Serialize)]
pub struct FvFileContent {
    pub text: String,
    pub size: u64,
    pub is_binary: bool,
    pub language: String,
    pub line_count: usize,
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn path_label(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn ext_to_language(ext: &str) -> &'static str {
    match ext.to_lowercase().as_str() {
        "rs"               => "rust",
        "js"|"mjs"|"cjs"   => "javascript",
        "ts"|"mts"|"cts"   => "typescript",
        "jsx"              => "javascript",
        "tsx"              => "typescript",
        "py"|"pyi"         => "python",
        "go"               => "go",
        "java"             => "java",
        "kt"|"kts"         => "kotlin",
        "rb"               => "ruby",
        "php"              => "php",
        "cs"               => "csharp",
        "c"|"h"            => "c",
        "cpp"|"cc"|"cxx"|"hpp" => "cpp",
        "swift"            => "swift",
        "html"|"htm"       => "html",
        "css"              => "css",
        "scss"|"sass"      => "scss",
        "less"             => "less",
        "json"|"jsonc"     => "json",
        "yaml"|"yml"       => "yaml",
        "toml"             => "toml",
        "xml"|"svg"        => "xml",
        "sql"              => "sql",
        "sh"|"bash"|"zsh"|"fish" => "bash",
        "ps1"|"psm1"       => "powershell",
        "md"|"markdown"|"mdx" => "markdown",
        "dockerfile"       => "dockerfile",
        "lua"              => "lua",
        "r"                => "r",
        "scala"            => "scala",
        "ex"|"exs"         => "elixir",
        "hs"               => "haskell",
        "vim"              => "vim",
        _                  => "plaintext",
    }
}

/// Returns true when the byte slice looks like binary data.
fn is_binary(data: &[u8]) -> bool {
    let sample = &data[..data.len().min(8192)];
    // Null byte → almost certainly binary.
    if sample.contains(&0u8) {
        return true;
    }
    // >30 % high bytes AND not valid UTF-8 → treat as binary.
    let high = sample.iter().filter(|&&b| b > 127).count();
    high as f32 / sample.len() as f32 > 0.30 && std::str::from_utf8(data).is_err()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Return all saved workspace folders in insertion order.
#[tauri::command]
pub async fn fv_list_workspaces(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<FvWorkspace>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT path, label FROM file_viewer_workspaces ORDER BY added_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<FvWorkspace> = stmt
        .query_map([], |r| {
            Ok(FvWorkspace { path: r.get(0)?, label: r.get(1)? })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Add a folder to the workspace.  The path is canonicalised before saving.
#[tauri::command]
pub async fn fv_add_workspace(
    state: State<'_, AppStateHandle>,
    path: String,
) -> Result<FvWorkspace, String> {
    let canonical = std::fs::canonicalize(&path)
        .map_err(|_| format!("Path does not exist: {path}"))?;
    if !canonical.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    let canonical_str = canonical.to_string_lossy().into_owned();
    let label = path_label(&canonical_str);
    let now = chrono::Utc::now().to_rfc3339();

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO file_viewer_workspaces (path, label, added_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![canonical_str, label, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(FvWorkspace { path: canonical_str, label })
}

/// Remove a workspace folder (does not delete anything on disk).
#[tauri::command]
pub async fn fv_remove_workspace(
    state: State<'_, AppStateHandle>,
    path: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM file_viewer_workspaces WHERE path = ?1",
        rusqlite::params![path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// List the immediate children of a directory.
/// Hidden entries (starting with `.`) are excluded unless `show_hidden` is true.
/// Result is sorted: directories first (a-z), then files (a-z).
///
/// Uses spawn_blocking so std::fs::read_dir does not block the tokio executor.
#[tauri::command]
pub async fn fv_read_dir(path: String, show_hidden: Option<bool>) -> Result<Vec<FvEntry>, String> {
    let show_hidden = show_hidden.unwrap_or(false);
    tokio::task::spawn_blocking(move || {
        let mut entries: Vec<FvEntry> = std::fs::read_dir(&path)
            .map_err(|e| format!("Cannot read directory: {e}"))?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if !show_hidden && name.starts_with('.') {
                    return None;
                }
                let meta = e.metadata().ok()?;
                let is_dir = meta.is_dir();
                let ext = if is_dir {
                    None
                } else {
                    std::path::Path::new(&name)
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_lowercase())
                };
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| {
                        let secs = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_secs() as i64;
                        chrono::DateTime::from_timestamp(secs, 0)
                            .map(|dt| dt.format("%Y-%m-%d").to_string())
                    })
                    .unwrap_or_default();
                Some(FvEntry {
                    path: e.path().to_string_lossy().into_owned(),
                    name,
                    is_dir,
                    size: if is_dir { 0 } else { meta.len() },
                    extension: ext,
                    modified,
                })
            })
            .collect();

        entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))
    .and_then(|r| r)
}

/// Read an image file and return it as a base64 data URI.
/// Works for PNG, JPEG, GIF, WebP, SVG, ICO, BMP, AVIF.
/// Files larger than 10 MB are rejected.
#[tauri::command]
pub async fn fv_read_image_base64(path: String) -> Result<String, String> {
    const MAX: u64 = 10 * 1024 * 1024;
    let size = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access '{}': {}", path, e))?
        .len();
    if size > MAX {
        return Err(format!("Image too large ({:.1} MB)", size as f64 / 1_048_576.0));
    }
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let mime = match ext.as_str() {
        "svg"          => "image/svg+xml",
        "png"          => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif"          => "image/gif",
        "webp"         => "image/webp",
        "ico"          => "image/x-icon",
        "bmp"          => "image/bmp",
        "avif"         => "image/avif",
        _              => "application/octet-stream",
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
    Ok(format!("data:{mime};base64,{b64}"))
}

/// Format / fix a Markdown document using the first enabled LLM endpoint.
#[tauri::command]
pub async fn fv_format_md(
    state: State<'_, AppStateHandle>,
    markdown: String,
) -> Result<String, String> {
    use minion_llm::{create_provider, EndpointConfig, ProviderType};
    use minion_llm::types::{ChatMessage, ChatRequest};

    let (pt_str, base_url, api_key, model): (String, String, Option<String>, String) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT provider_type, base_url, api_key_encrypted, \
             COALESCE(default_model, 'llama3') FROM llm_endpoints \
             WHERE enabled = 1 ORDER BY created_at DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| "No enabled LLM endpoint. Add one in Settings → LLM Endpoints.".to_string())?
    };

    let pt = match pt_str.as_str() {
        "ollama"        => ProviderType::Ollama,
        "anthropic"     => ProviderType::Anthropic,
        "google_gemini" => ProviderType::GoogleGemini,
        _               => ProviderType::OpenaiCompatible,
    };
    let cfg = EndpointConfig {
        provider_type: pt, base_url, api_key,
        default_model: model,
        extra_headers: Default::default(),
    };

    let provider = create_provider(cfg);
    let req = ChatRequest {
        messages: vec![
            ChatMessage::user(format!("Fix and improve this Markdown:\n\n{markdown}")),
        ],
        system: Some(
            "You are a Markdown expert. Fix and improve the formatting of the \
             provided Markdown document. Return ONLY the corrected Markdown — \
             no explanations, no surrounding code fences.".to_string(),
        ),
        model: None,
        temperature: Some(0.2),
        max_tokens: Some(8192),
        json_mode: false,
    };

    let resp = provider.chat(req).await.map_err(|e| e.to_string())?;
    Ok(resp.content)
}

/// Read a text file and return its content with metadata.
/// Files larger than 5 MB are rejected.
/// Binary files are detected and flagged — `text` will be empty.
///
/// I/O uses tokio::fs (async). CPU-bound work (binary detection, UTF-8
/// conversion, line counting) runs in spawn_blocking so the tokio async
/// thread pool is never stalled by synchronous processing.
#[tauri::command]
pub async fn fv_read_file(path: String) -> Result<FvFileContent, String> {
    const MAX: u64 = 5 * 1024 * 1024;

    let meta = tokio::fs::metadata(&path)
        .await
        .map_err(|e| format!("Cannot access '{}': {}", path, e))?;
    let size = meta.len();

    if size > MAX {
        return Err(format!(
            "File too large to preview ({:.1} MB). Limit is 5 MB.",
            size as f64 / 1_048_576.0
        ));
    }

    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| format!("Cannot read '{}': {}", path, e))?;

    // All remaining work is CPU-bound: move it off the tokio async thread.
    tokio::task::spawn_blocking(move || {
        if is_binary(&data) {
            return Ok(FvFileContent {
                text: String::new(),
                size,
                is_binary: true,
                language: "plaintext".to_owned(),
                line_count: 0,
            });
        }

        let text = String::from_utf8_lossy(&data).into_owned();
        let line_count = text.lines().count();
        let ext = std::path::Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let language = ext_to_language(&ext).to_owned();

        Ok(FvFileContent { text, size, is_binary: false, language, line_count })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))
    .and_then(|r| r)
}
