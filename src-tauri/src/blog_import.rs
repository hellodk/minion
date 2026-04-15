//! Blog module v2 — bulk import pipeline.
//!
//! Ingests markdown/HTML/plaintext files (one file or a whole folder),
//! parses YAML frontmatter, infers tags from the parent folder chain,
//! and stages each candidate as an `ImportPreview` that the UI can edit
//! before the user confirms. On confirm we:
//!
//! 1. Create the `blog_posts` row
//! 2. Find every `![alt](path)` and `<img src="…">` in the body
//! 3. Copy referenced local images into the asset vault, deduped by
//!    SHA-256, and rewrite the body to point at `assets/{sha}.{ext}`
//! 4. Wire up tags in `blog_post_tags`
//!
//! Remote (http/https) image references are left alone — those stay on
//! the origin server. The vault lives at
//! `{app_data}/blog/assets/{sha}.{ext}`.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

const IMPORTABLE_EXT: &[&str] = &["md", "markdown", "txt", "html", "htm"];
const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "bmp"];

// =====================================================================
// Types
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    /// UUID used to correlate confirm() with the preview.
    pub preview_id: String,
    pub source_path: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: Option<String>,
    pub date: Option<String>,
    pub status: String,
    pub canonical_url: Option<String>,
    /// Tags we suggest (folder names, frontmatter tags). The UI can edit.
    pub suggested_tags: Vec<String>,
    /// Number of `![...](path)` / `<img src="...">` references we detected.
    pub image_references: Vec<String>,
    pub had_frontmatter: bool,
    pub already_exists: bool,
    /// Size in characters of the parsed body (post-frontmatter).
    pub body_char_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: i64,
    pub skipped: i64,
    pub failed: i64,
    pub post_ids: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConfirmImportEntry {
    #[allow(dead_code)]
    pub preview_id: String,
    pub source_path: String,
    pub title: String,
    pub slug: String,
    pub content: String,
    pub excerpt: Option<String>,
    pub date: Option<String>,
    pub status: String,
    #[allow(dead_code)]
    pub canonical_url: Option<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
}

// =====================================================================
// Tauri commands: discover
// =====================================================================

#[tauri::command]
pub async fn blog_import_files(
    state: State<'_, AppStateHandle>,
    paths: Vec<String>,
) -> Result<Vec<ImportPreview>, String> {
    let mut out = Vec::new();
    for p in &paths {
        let path = Path::new(p);
        if !path.is_file() {
            continue;
        }
        if !is_importable(path) {
            continue;
        }
        match build_preview(&state, path).await {
            Ok(p) => out.push(p),
            Err(e) => tracing::warn!("build_preview({}): {}", p, e),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn blog_import_folder(
    state: State<'_, AppStateHandle>,
    path: String,
) -> Result<Vec<ImportPreview>, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {}", path));
    }
    let mut files = Vec::new();
    collect_importable(&root, &mut files);
    let mut out = Vec::with_capacity(files.len());
    for file in files {
        match build_preview(&state, &file).await {
            Ok(p) => out.push(p),
            Err(e) => tracing::warn!("build_preview({}): {}", file.display(), e),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn blog_confirm_import(
    state: State<'_, AppStateHandle>,
    app: tauri::AppHandle,
    entries: Vec<ConfirmImportEntry>,
    author: Option<String>,
) -> Result<ImportResult, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let asset_root = asset_vault_dir(&app_data_dir);
    fs::create_dir_all(&asset_root).map_err(|e| e.to_string())?;

    let mut imported = 0i64;
    let mut skipped = 0i64;
    let mut failed = 0i64;
    let mut ids = Vec::new();
    let mut errors = Vec::new();

    for entry in entries {
        match insert_one_post(&state, &entry, author.as_deref(), &asset_root).await {
            Ok(Some(id)) => {
                imported += 1;
                ids.push(id);
            }
            Ok(None) => {
                skipped += 1;
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", entry.source_path, e));
            }
        }
    }
    Ok(ImportResult {
        imported,
        skipped,
        failed,
        post_ids: ids,
        errors,
    })
}

// =====================================================================
// Preview construction
// =====================================================================

async fn build_preview(
    state: &State<'_, AppStateHandle>,
    path: &Path,
) -> Result<ImportPreview, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (fm, body) = split_frontmatter(&raw);
    let had_frontmatter = fm.is_some();

    let title = fm
        .as_ref()
        .and_then(|m| m.get("title").cloned())
        .or_else(|| first_h1(body))
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.replace(['_', '-'], " "))
        })
        .unwrap_or_else(|| "Untitled".to_string());

    let slug = minion_blog::posts::slugify(&title);

    let date = fm
        .as_ref()
        .and_then(|m| m.get("date").cloned())
        .or_else(|| {
            fs::metadata(path).ok().and_then(|meta| {
                meta.modified().ok().map(|m| {
                    let dt: chrono::DateTime<chrono::Utc> = m.into();
                    dt.format("%Y-%m-%d").to_string()
                })
            })
        });

    let status = fm
        .as_ref()
        .and_then(|m| m.get("status").cloned())
        .unwrap_or_else(|| "draft".to_string());

    let canonical_url = fm.as_ref().and_then(|m| m.get("canonical_url").cloned());

    let mut tags: BTreeSet<String> = BTreeSet::new();
    if let Some(m) = &fm {
        if let Some(tag_list) = m.get("tags") {
            for t in parse_yaml_list(tag_list) {
                tags.insert(t);
            }
        }
    }
    // Suggest the immediate parent folder as a tag (normalized).
    if let Some(parent_name) = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
    {
        let norm = parent_name
            .trim()
            .to_lowercase()
            .replace([' ', '_'], "-");
        if !norm.is_empty() && norm != "posts" && norm != "content" {
            tags.insert(norm);
        }
    }

    let image_refs = extract_image_refs(body);

    // Check whether we've already imported this slug.
    let already_exists = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT 1 FROM blog_posts WHERE slug = ?1 LIMIT 1",
            rusqlite::params![slug],
            |_| Ok(()),
        )
        .is_ok()
    };

    let excerpt = body
        .trim()
        .lines()
        .find(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let cleaned = l.trim().to_string();
            cleaned.chars().take(160).collect::<String>()
        });

    Ok(ImportPreview {
        preview_id: uuid::Uuid::new_v4().to_string(),
        source_path: path.to_string_lossy().into_owned(),
        title,
        slug,
        content: body.to_string(),
        excerpt,
        date,
        status,
        canonical_url,
        suggested_tags: tags.into_iter().collect(),
        image_references: image_refs,
        had_frontmatter,
        already_exists,
        body_char_count: body.chars().count(),
    })
}

// =====================================================================
// Insert
// =====================================================================

async fn insert_one_post(
    state: &State<'_, AppStateHandle>,
    entry: &ConfirmImportEntry,
    fallback_author: Option<&str>,
    asset_root: &Path,
) -> Result<Option<String>, String> {
    // Dedup by slug — skip quietly.
    {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM blog_posts WHERE slug = ?1)",
                rusqlite::params![entry.slug],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if exists {
            return Ok(None);
        }
    }

    let post_id = uuid::Uuid::new_v4().to_string();
    let source_dir = Path::new(&entry.source_path)
        .parent()
        .map(|p| p.to_path_buf());

    // Process images first so we can rewrite the body to point at the vault.
    let (rewritten, asset_refs) = copy_and_rewrite_assets(
        state,
        asset_root,
        source_dir.as_deref(),
        &entry.content,
    )
    .await?;

    let wc = minion_blog::posts::word_count(&rewritten) as i64;
    let rt = minion_blog::posts::calculate_reading_time(&rewritten) as i64;
    let now = chrono::Utc::now().to_rfc3339();
    let created_at = entry
        .date
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| now.clone());
    let author = entry
        .author
        .as_deref()
        .or(fallback_author)
        .unwrap_or("");

    let st = state.read().await;
    let mut conn = st.db.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO blog_posts (id, title, slug, content, excerpt, status,
         author, word_count, reading_time, created_at, updated_at, published_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        rusqlite::params![
            post_id,
            entry.title,
            entry.slug,
            rewritten,
            entry.excerpt,
            entry.status,
            author,
            wc,
            rt,
            created_at,
            now,
            if entry.status == "published" { Some(&created_at) } else { None },
        ],
    )
    .map_err(|e| e.to_string())?;

    // Tags: normalize, upsert into blog_tags, link via blog_post_tags.
    for raw in &entry.tags {
        let name = raw.trim().to_lowercase();
        if name.is_empty() {
            continue;
        }
        let tag_id = upsert_tag(&tx, &name)?;
        tx.execute(
            "INSERT OR IGNORE INTO blog_post_tags (post_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![post_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Asset rows: insert link between post and each asset_id we touched.
    for (asset_id, referenced_as) in asset_refs {
        tx.execute(
            "INSERT OR IGNORE INTO blog_post_assets (post_id, asset_id, referenced_as)
             VALUES (?1, ?2, ?3)",
            rusqlite::params![post_id, asset_id, referenced_as],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(Some(post_id))
}

fn upsert_tag(tx: &rusqlite::Transaction, name: &str) -> Result<String, String> {
    let existing: Option<String> = tx
        .query_row(
            "SELECT id FROM blog_tags WHERE name = ?1",
            rusqlite::params![name],
            |r| r.get(0),
        )
        .ok();
    if let Some(id) = existing {
        return Ok(id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO blog_tags (id, name) VALUES (?1, ?2)",
        rusqlite::params![id, name],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

// =====================================================================
// Asset handling
// =====================================================================

async fn copy_and_rewrite_assets(
    state: &State<'_, AppStateHandle>,
    asset_root: &Path,
    source_dir: Option<&Path>,
    body: &str,
) -> Result<(String, Vec<(String, String)>), String> {
    // Collect all local image refs to replace. Build replacement plan
    // first, then apply string substitutions so overlapping matches stay
    // stable.
    let refs = extract_image_refs(body);
    let mut plan: Vec<(String, String)> = Vec::new();
    let mut asset_refs: Vec<(String, String)> = Vec::new();

    for r in refs {
        if is_remote(&r) {
            continue;
        }
        let original = r.clone();
        let abs = resolve_image(source_dir, &r);
        let Some(abs) = abs else {
            continue;
        };
        if !abs.exists() {
            continue;
        }
        let sha = match sha256_file(&abs) {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("sha256 {}: {}", abs.display(), e);
                continue;
            }
        };
        let ext = abs
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_lowercase();
        let vault_name = format!("{}.{}", sha, ext);
        let vault_path = asset_root.join(&vault_name);
        if !vault_path.exists() {
            if let Err(e) = fs::copy(&abs, &vault_path) {
                tracing::warn!("copy asset {}: {}", abs.display(), e);
                continue;
            }
        }

        // Upsert asset row keyed by SHA.
        let asset_id = {
            let st = state.read().await;
            let conn = st.db.get().map_err(|e| e.to_string())?;
            let existing: Option<String> = conn
                .query_row(
                    "SELECT id FROM blog_assets WHERE sha256 = ?1",
                    rusqlite::params![sha],
                    |r| r.get(0),
                )
                .ok();
            match existing {
                Some(id) => id,
                None => {
                    let id = uuid::Uuid::new_v4().to_string();
                    let size = fs::metadata(&abs).map(|m| m.len() as i64).unwrap_or(0);
                    let mime = guess_mime(&ext);
                    conn.execute(
                        "INSERT INTO blog_assets (id, sha256, stored_path,
                         original_filename, mime_type, size_bytes)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        rusqlite::params![
                            id,
                            sha,
                            vault_path.to_string_lossy().to_string(),
                            abs.file_name().and_then(|f| f.to_str()),
                            mime,
                            size
                        ],
                    )
                    .map_err(|e| e.to_string())?;
                    id
                }
            }
        };

        let replacement = format!("assets/{}", vault_name);
        plan.push((original.clone(), replacement));
        asset_refs.push((asset_id, original));
    }

    // Apply substitutions. This is purely textual; we treat the body as
    // opaque so malformed Markdown doesn't break import.
    let mut rewritten = body.to_string();
    for (from, to) in plan {
        rewritten = rewritten.replace(&from, &to);
    }
    Ok((rewritten, asset_refs))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
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

// =====================================================================
// Parsing helpers
// =====================================================================

/// Split YAML-like frontmatter from the body. Only the subset of YAML we
/// actually use: `key: value` and `tags: [a, b]` / `tags:\n  - a`.
fn split_frontmatter(raw: &str) -> (Option<std::collections::HashMap<String, String>>, &str) {
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    if !raw.starts_with("---") {
        return (None, raw);
    }
    let after = &raw[3..];
    let after = after.strip_prefix('\n').unwrap_or(after);
    let Some(end_idx) = find_closing_fence(after) else {
        return (None, raw);
    };
    let yaml = &after[..end_idx];
    let body = &after[end_idx..];
    // Step past the closing `---\n`.
    let body = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))
        .or_else(|| body.strip_prefix("---"))
        .unwrap_or(body);

    let mut map = std::collections::HashMap::new();
    let mut lines = yaml.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_string();
            let value = v.trim();
            if value.is_empty() {
                // Multiline list: peek indented `- foo` items.
                let mut items = Vec::new();
                while let Some(peeked) = lines.peek() {
                    let t = peeked.trim_start();
                    if t.starts_with("- ") || t.starts_with('-') {
                        let item = t.trim_start_matches('-').trim().trim_matches('"');
                        items.push(item.to_string());
                        lines.next();
                    } else {
                        break;
                    }
                }
                if !items.is_empty() {
                    map.insert(key, format!("[{}]", items.join(", ")));
                }
            } else {
                map.insert(key, value.trim_matches('"').to_string());
            }
        }
    }
    (Some(map), body)
}

fn find_closing_fence(after_open: &str) -> Option<usize> {
    // The closing --- must be on its own line.
    let bytes = after_open.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Find start-of-line.
        let line_start = i;
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        let line = &after_open[line_start..i];
        if line.trim_end() == "---" {
            return Some(line_start);
        }
        i += 1;
    }
    None
}

fn parse_yaml_list(raw: &str) -> Vec<String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(trimmed);
    inner
        .split(',')
        .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn first_h1(body: &str) -> Option<String> {
    for line in body.lines().take(40) {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn extract_image_refs(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();

    // Markdown: ![alt](path). We intentionally keep this simple and only
    // accept a parenthesized URL with no nested parens.
    let bytes = body.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'!' && bytes.get(i + 1) == Some(&b'[') {
            let alt_start = i + 2;
            let Some(alt_end) = memchr_single(bytes, alt_start, b']') else {
                i += 1;
                continue;
            };
            if bytes.get(alt_end + 1) != Some(&b'(') {
                i = alt_end + 1;
                continue;
            }
            let url_start = alt_end + 2;
            let Some(url_end) = memchr_single(bytes, url_start, b')') else {
                i = url_start;
                continue;
            };
            let url = body[url_start..url_end].trim();
            // Strip "URL TITLE" shape: take up to first whitespace.
            let first = url.split_whitespace().next().unwrap_or(url);
            if !first.is_empty() && seen.insert(first.to_string()) {
                out.push(first.to_string());
            }
            i = url_end + 1;
            continue;
        }
        i += 1;
    }

    // HTML: <img src="...">. Lowercase-ish scan.
    let lower = body.to_lowercase();
    let lb = lower.as_bytes();
    let mut j = 0;
    while let Some(k) = find_sub(lb, j, b"<img") {
        let rest_start = k + 4;
        // Look for src= within this tag.
        let tag_end = find_byte(lb, rest_start, b'>').unwrap_or(lb.len());
        if let Some(src_idx) = find_sub(&lb[rest_start..tag_end], 0, b"src=") {
            let s = rest_start + src_idx + 4;
            if s < tag_end {
                let quote = bytes[s];
                if quote == b'"' || quote == b'\'' {
                    if let Some(end) = find_byte(bytes, s + 1, quote) {
                        if end <= tag_end {
                            let url = &body[s + 1..end];
                            if !url.is_empty() && seen.insert(url.to_string()) {
                                out.push(url.to_string());
                            }
                        }
                    }
                }
            }
        }
        j = tag_end;
    }

    out
}

fn memchr_single(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    bytes[start..].iter().position(|b| *b == target).map(|p| start + p)
}
fn find_byte(bytes: &[u8], start: usize, target: u8) -> Option<usize> {
    memchr_single(bytes, start, target)
}
fn find_sub(hay: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || start >= hay.len() {
        return None;
    }
    hay[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| start + p)
}

fn is_remote(s: &str) -> bool {
    let l = s.trim().to_lowercase();
    l.starts_with("http://")
        || l.starts_with("https://")
        || l.starts_with("data:")
        || l.starts_with("//")
}

fn resolve_image(source_dir: Option<&Path>, rel: &str) -> Option<PathBuf> {
    let clean = rel.trim().trim_matches(&['"', '\''][..]);
    if clean.is_empty() {
        return None;
    }
    let p = Path::new(clean);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    source_dir.map(|dir| dir.join(clean))
}

fn is_importable(p: &Path) -> bool {
    match p.extension().and_then(|e| e.to_str()) {
        Some(ext) => IMPORTABLE_EXT.contains(&ext.to_lowercase().as_str()),
        None => false,
    }
}

fn collect_importable(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_importable(&path, out);
        } else if is_importable(&path) {
            out.push(path);
        }
    }
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

pub fn asset_vault_dir(app_data: &Path) -> PathBuf {
    app_data.join("blog").join("assets")
}

#[allow(dead_code)]
fn _image_ext_check() {
    // Keep IMAGE_EXT referenced so future use isn't linted away.
    let _ = IMAGE_EXT;
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parses_basic_fields() {
        let raw = "---\ntitle: Hello\ndate: 2024-01-02\nstatus: published\n---\nBody text";
        let (fm, body) = split_frontmatter(raw);
        let fm = fm.expect("frontmatter");
        assert_eq!(fm.get("title").unwrap(), "Hello");
        assert_eq!(fm.get("date").unwrap(), "2024-01-02");
        assert_eq!(fm.get("status").unwrap(), "published");
        assert_eq!(body.trim(), "Body text");
    }

    #[test]
    fn frontmatter_parses_tag_list() {
        let raw = "---\ntitle: x\ntags: [k8s, nfs, storage]\n---\nX";
        let (fm, _) = split_frontmatter(raw);
        let tags = parse_yaml_list(fm.unwrap().get("tags").unwrap());
        assert_eq!(tags, vec!["k8s", "nfs", "storage"]);
    }

    #[test]
    fn frontmatter_parses_multiline_list() {
        let raw = "---\ntitle: x\ntags:\n  - foo\n  - bar\n---\nhi";
        let (fm, _) = split_frontmatter(raw);
        let fm = fm.unwrap();
        let tags = parse_yaml_list(fm.get("tags").unwrap());
        assert_eq!(tags, vec!["foo", "bar"]);
    }

    #[test]
    fn no_frontmatter_returns_none() {
        let raw = "# Hello\n\nBody.";
        let (fm, body) = split_frontmatter(raw);
        assert!(fm.is_none());
        assert_eq!(body, raw);
    }

    #[test]
    fn extract_markdown_and_html_images() {
        let body = r#"
Here is ![alt](./img/local.png) and ![remote](https://cdn.com/x.png).
Also <img src="abs.jpg" alt="x"/> and <img src='single.gif'/>.
"#;
        let refs = extract_image_refs(body);
        assert!(refs.contains(&"./img/local.png".to_string()));
        assert!(refs.contains(&"https://cdn.com/x.png".to_string()));
        assert!(refs.contains(&"abs.jpg".to_string()));
        assert!(refs.contains(&"single.gif".to_string()));
    }

    #[test]
    fn is_remote_detects_http_and_data() {
        assert!(is_remote("https://foo"));
        assert!(is_remote("HTTP://foo"));
        assert!(is_remote("data:image/png;base64,abc"));
        assert!(is_remote("//cdn/foo"));
        assert!(!is_remote("./local.png"));
        assert!(!is_remote("img/local.png"));
    }

    #[test]
    fn first_h1_found() {
        let body = "Intro paragraph.\n\n# The Title\n\nMore text.";
        assert_eq!(first_h1(body).as_deref(), Some("The Title"));
    }
}
