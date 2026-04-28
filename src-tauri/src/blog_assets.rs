//! Blog module v2 — image asset management.
//!
//! Tauri commands backing the "Assets" tab: list all stored images, see
//! which posts use each one, upload a standalone asset, clean up orphans,
//! and resolve `asset://` paths to concrete filesystem locations so the
//! UI can render them through `convertFileSrc`.

use crate::blog_import::asset_vault_dir;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogAsset {
    pub id: String,
    pub sha256: String,
    pub stored_path: String,
    pub original_filename: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub size_bytes: Option<i64>,
    pub created_at: String,
    pub use_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetUsage {
    pub post_id: String,
    pub post_title: String,
    pub post_slug: String,
    pub referenced_as: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteResult {
    pub deleted_db_rows: i64,
    pub deleted_files: i64,
    pub freed_bytes: i64,
    pub errors: Vec<String>,
}

// =====================================================================
// Commands
// =====================================================================

#[tauri::command]
pub async fn blog_list_assets(state: State<'_, AppStateHandle>) -> Result<Vec<BlogAsset>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.sha256, a.stored_path, a.original_filename,
                    a.mime_type, a.width, a.height, a.size_bytes, a.created_at,
                    (SELECT COUNT(*) FROM blog_post_assets pa WHERE pa.asset_id = a.id)
             FROM blog_assets a ORDER BY a.created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BlogAsset {
                id: row.get(0)?,
                sha256: row.get(1)?,
                stored_path: row.get(2)?,
                original_filename: row.get(3)?,
                mime_type: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                size_bytes: row.get(7)?,
                created_at: row.get(8)?,
                use_count: row.get(9)?,
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
pub async fn blog_get_asset_usage(
    state: State<'_, AppStateHandle>,
    asset_id: String,
) -> Result<Vec<AssetUsage>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT p.id, p.title, p.slug, pa.referenced_as
             FROM blog_post_assets pa
             JOIN blog_posts p ON p.id = pa.post_id
             WHERE pa.asset_id = ?1
             ORDER BY p.updated_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&asset_id], |row| {
            Ok(AssetUsage {
                post_id: row.get(0)?,
                post_title: row.get(1)?,
                post_slug: row.get(2)?,
                referenced_as: row.get(3)?,
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
pub async fn blog_upload_asset(
    state: State<'_, AppStateHandle>,
    app: tauri::AppHandle,
    file_path: String,
) -> Result<BlogAsset, String> {
    let src = Path::new(&file_path);
    if !src.is_file() {
        return Err(format!("not a file: {}", file_path));
    }
    let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let vault = asset_vault_dir(&app_data);
    fs::create_dir_all(&vault).map_err(|e| e.to_string())?;

    let sha = sha256_file(src).map_err(|e| e.to_string())?;
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin")
        .to_lowercase();
    let vault_name = format!("{}.{}", sha, ext);
    let vault_path = vault.join(&vault_name);
    if !vault_path.exists() {
        fs::copy(src, &vault_path).map_err(|e| e.to_string())?;
    }
    let size = fs::metadata(&vault_path).map(|m| m.len() as i64).ok();
    let mime = guess_mime(&ext);

    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM blog_assets WHERE sha256 = ?1",
            rusqlite::params![sha],
            |r| r.get(0),
        )
        .ok();
    let id = match existing {
        Some(id) => id,
        None => {
            let new_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO blog_assets (id, sha256, stored_path, original_filename,
                 mime_type, size_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    new_id,
                    sha,
                    vault_path.to_string_lossy().to_string(),
                    src.file_name().and_then(|f| f.to_str()),
                    mime,
                    size
                ],
            )
            .map_err(|e| e.to_string())?;
            new_id
        }
    };

    conn.query_row(
        "SELECT a.id, a.sha256, a.stored_path, a.original_filename,
                a.mime_type, a.width, a.height, a.size_bytes, a.created_at,
                (SELECT COUNT(*) FROM blog_post_assets pa WHERE pa.asset_id = a.id)
         FROM blog_assets a WHERE a.id = ?1",
        rusqlite::params![id],
        |row| {
            Ok(BlogAsset {
                id: row.get(0)?,
                sha256: row.get(1)?,
                stored_path: row.get(2)?,
                original_filename: row.get(3)?,
                mime_type: row.get(4)?,
                width: row.get(5)?,
                height: row.get(6)?,
                size_bytes: row.get(7)?,
                created_at: row.get(8)?,
                use_count: row.get(9)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn blog_delete_orphan_assets(
    state: State<'_, AppStateHandle>,
) -> Result<DeleteResult, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    // Anything not referenced by any post is a candidate.
    let mut stmt = conn
        .prepare(
            "SELECT id, stored_path, COALESCE(size_bytes, 0) FROM blog_assets
             WHERE id NOT IN (SELECT DISTINCT asset_id FROM blog_post_assets)",
        )
        .map_err(|e| e.to_string())?;
    let orphans: Vec<(String, String, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut result = DeleteResult {
        deleted_db_rows: 0,
        deleted_files: 0,
        freed_bytes: 0,
        errors: Vec::new(),
    };
    for (id, path, size) in orphans {
        match fs::remove_file(&path) {
            Ok(()) => {
                result.deleted_files += 1;
                result.freed_bytes += size;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                result.errors.push(format!("{}: {}", path, e));
            }
        }
        if conn
            .execute(
                "DELETE FROM blog_assets WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| e.to_string())?
            > 0
        {
            result.deleted_db_rows += 1;
        }
    }
    Ok(result)
}

/// Returns the absolute path on disk for a stored asset. The UI passes
/// this through `convertFileSrc()` to render via the Tauri asset:
/// protocol without exposing the filesystem layout.
#[tauri::command]
pub async fn blog_get_asset_path(
    state: State<'_, AppStateHandle>,
    asset_id: String,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT stored_path FROM blog_assets WHERE id = ?1",
        rusqlite::params![asset_id],
        |r| r.get::<_, String>(0),
    )
    .map_err(|e| e.to_string())
}

// =====================================================================
// Tags
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogTag {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub post_count: i64,
}

#[tauri::command]
pub async fn blog_list_tags(state: State<'_, AppStateHandle>) -> Result<Vec<BlogTag>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color, t.created_at,
                    (SELECT COUNT(*) FROM blog_post_tags pt WHERE pt.tag_id = t.id)
             FROM blog_tags t ORDER BY t.name ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BlogTag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
                post_count: row.get(4)?,
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
pub async fn blog_create_tag(
    state: State<'_, AppStateHandle>,
    name: String,
    color: Option<String>,
) -> Result<BlogTag, String> {
    let name = name.trim().to_lowercase();
    if name.is_empty() {
        return Err("tag name cannot be empty".into());
    }
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    // INSERT OR IGNORE + select, so double-create is idempotent.
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO blog_tags (id, name, color) VALUES (?1, ?2, ?3)",
        rusqlite::params![id, name, color],
    )
    .map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT t.id, t.name, t.color, t.created_at,
                (SELECT COUNT(*) FROM blog_post_tags pt WHERE pt.tag_id = t.id)
         FROM blog_tags t WHERE t.name = ?1",
        rusqlite::params![name],
        |row| {
            Ok(BlogTag {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                created_at: row.get(3)?,
                post_count: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn blog_set_post_tags(
    state: State<'_, AppStateHandle>,
    post_id: String,
    tag_names: Vec<String>,
) -> Result<(), String> {
    let st = state.read().await;
    let mut conn = st.db.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM blog_post_tags WHERE post_id = ?1",
        rusqlite::params![post_id],
    )
    .map_err(|e| e.to_string())?;
    for raw in tag_names {
        let name = raw.trim().to_lowercase();
        if name.is_empty() {
            continue;
        }
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM blog_tags WHERE name = ?1",
                rusqlite::params![name],
                |r| r.get(0),
            )
            .ok();
        let tag_id = match existing {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO blog_tags (id, name) VALUES (?1, ?2)",
                    rusqlite::params![id, name],
                )
                .map_err(|e| e.to_string())?;
                id
            }
        };
        tx.execute(
            "INSERT OR IGNORE INTO blog_post_tags (post_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![post_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn blog_get_post_tags(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Vec<String>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT t.name FROM blog_post_tags pt
             JOIN blog_tags t ON t.id = pt.tag_id
             WHERE pt.post_id = ?1
             ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&post_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// =====================================================================
// Utilities
// =====================================================================

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn guess_mime(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}
