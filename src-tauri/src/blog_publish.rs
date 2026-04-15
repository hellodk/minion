//! Blog module v2 — multi-platform publishing.
//!
//! Two flavors:
//!
//! * **Auto-publish** (WordPress, Dev.to, Hashnode) — we call the
//!   platform's REST/GraphQL API from Rust and record `remote_url` +
//!   `remote_id` in `blog_platform_publications`.
//! * **Manual export** (Medium, Substack, LinkedIn, X/Twitter) — the
//!   backend produces a platform-tailored payload (markdown/HTML/text)
//!   and the UI copies it to clipboard + opens the target editor.
//!
//! Account credentials live in `blog_platform_accounts`. API keys are
//! stored as written today; the same at-rest encryption work queued for
//! Health Vault will cover this table on the next pass.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

// =====================================================================
// Accounts
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformAccount {
    pub id: String,
    pub platform: String,
    pub account_label: Option<String>,
    pub base_url: Option<String>,
    pub publication_id: Option<String>,
    pub default_tags: Option<Vec<String>>,
    pub enabled: bool,
    pub created_at: String,
    /// Never returned (masked) for safety. Only `has_key: bool` is
    /// exposed.
    pub has_key: bool,
}

fn row_to_account(row: &rusqlite::Row) -> rusqlite::Result<PlatformAccount> {
    let default_tags_raw: Option<String> = row.get(5)?;
    let default_tags: Option<Vec<String>> = default_tags_raw.and_then(|s| {
        serde_json::from_str(&s).ok()
    });
    let api_key: Option<String> = row.get(7)?;
    Ok(PlatformAccount {
        id: row.get(0)?,
        platform: row.get(1)?,
        account_label: row.get(2)?,
        base_url: row.get(3)?,
        publication_id: row.get(4)?,
        default_tags,
        enabled: row.get::<_, i64>(6)? != 0,
        created_at: row.get(8)?,
        has_key: api_key.as_deref().is_some_and(|s| !s.is_empty()),
    })
}

const ACCOUNT_COLUMNS: &str =
    "id, platform, account_label, base_url, publication_id, default_tags, \
     enabled, api_key_encrypted, created_at";

#[tauri::command]
pub async fn blog_list_platform_accounts(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<PlatformAccount>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_accounts ORDER BY platform, account_label",
        ACCOUNT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_account)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
pub struct CreateAccount {
    pub platform: String,
    #[serde(default)]
    pub account_label: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub publication_id: Option<String>,
    #[serde(default)]
    pub default_tags: Option<Vec<String>>,
}

#[tauri::command]
pub async fn blog_create_platform_account(
    state: State<'_, AppStateHandle>,
    account: CreateAccount,
) -> Result<PlatformAccount, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let default_tags_json = account
        .default_tags
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO blog_platform_accounts
         (id, platform, account_label, base_url, api_key_encrypted,
          publication_id, default_tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![
            id,
            account.platform,
            account.account_label,
            account.base_url,
            account.api_key,
            account.publication_id,
            default_tags_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_accounts WHERE id = ?1",
        ACCOUNT_COLUMNS
    );
    conn.query_row(&sql, rusqlite::params![id], row_to_account)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn blog_delete_platform_account(
    state: State<'_, AppStateHandle>,
    id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM blog_platform_accounts WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

async fn load_account(
    state: &AppStateHandle,
    account_id: &str,
) -> Result<(PlatformAccount, Option<String>), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_accounts WHERE id = ?1",
        ACCOUNT_COLUMNS
    );
    let account = conn
        .query_row(&sql, rusqlite::params![account_id], row_to_account)
        .map_err(|e| format!("account not found: {}", e))?;
    let key: Option<String> = conn
        .query_row(
            "SELECT api_key_encrypted FROM blog_platform_accounts WHERE id = ?1",
            rusqlite::params![account_id],
            |r| r.get(0),
        )
        .ok();
    Ok((account, key))
}

// =====================================================================
// Publications
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    pub id: String,
    pub post_id: String,
    pub platform: String,
    pub account_id: Option<String>,
    pub status: Option<String>,
    pub remote_id: Option<String>,
    pub remote_url: Option<String>,
    pub canonical_url: Option<String>,
    pub published_at: Option<String>,
    pub last_synced_at: Option<String>,
    pub error: Option<String>,
}

fn row_to_publication(row: &rusqlite::Row) -> rusqlite::Result<Publication> {
    Ok(Publication {
        id: row.get(0)?,
        post_id: row.get(1)?,
        platform: row.get(2)?,
        account_id: row.get(3)?,
        status: row.get(4)?,
        remote_id: row.get(5)?,
        remote_url: row.get(6)?,
        canonical_url: row.get(7)?,
        published_at: row.get(8)?,
        last_synced_at: row.get(9)?,
        error: row.get(10)?,
    })
}

const PUB_COLUMNS: &str =
    "id, post_id, platform, account_id, status, remote_id, remote_url, \
     canonical_url, published_at, last_synced_at, error";

#[tauri::command]
pub async fn blog_list_publications(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Vec<Publication>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_publications WHERE post_id = ?1 ORDER BY platform",
        PUB_COLUMNS
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&post_id], row_to_publication)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn blog_unpublish(
    state: State<'_, AppStateHandle>,
    publication_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM blog_platform_publications WHERE id = ?1",
        rusqlite::params![publication_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
// Post loader
// =====================================================================

#[derive(Debug, Clone)]
struct LoadedPost {
    id: String,
    title: String,
    slug: String,
    content: String,
    excerpt: Option<String>,
    canonical_url: Option<String>,
    tags: Vec<String>,
}

async fn load_post(
    state: &AppStateHandle,
    post_id: &str,
) -> Result<LoadedPost, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let (id, title, slug, content, excerpt): (String, String, String, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT id, title, slug, content, excerpt FROM blog_posts WHERE id = ?1",
            rusqlite::params![post_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|e| format!("post not found: {}", e))?;

    let canonical_url: Option<String> = conn
        .query_row(
            "SELECT canonical_url FROM blog_platform_publications
             WHERE post_id = ?1 AND canonical_url IS NOT NULL LIMIT 1",
            rusqlite::params![post_id],
            |r| r.get(0),
        )
        .ok();

    let mut stmt = conn
        .prepare(
            "SELECT t.name FROM blog_post_tags pt
             JOIN blog_tags t ON t.id = pt.tag_id
             WHERE pt.post_id = ?1 ORDER BY t.name",
        )
        .map_err(|e| e.to_string())?;
    let tags: Vec<String> = stmt
        .query_map([&post_id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(LoadedPost {
        id,
        title,
        slug,
        content: content.unwrap_or_default(),
        excerpt,
        canonical_url,
        tags,
    })
}

// =====================================================================
// Platform dispatch
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    pub publication_id: String,
    pub status: String,
    pub remote_url: Option<String>,
    pub remote_id: Option<String>,
}

/// Schedule a publish for later. The ticker spawned at app start flips
/// `scheduled` → `published` when the wall clock catches up.
#[tauri::command]
pub async fn blog_schedule_publish(
    state: State<'_, AppStateHandle>,
    post_id: String,
    account_id: String,
    scheduled_at: String,
) -> Result<Publication, String> {
    // Normalize to UTC so the ticker's string comparison stays correct
    // regardless of the caller's timezone. Without this, a job sent as
    // `...-05:00` would sort-compare wrong against the ticker's `Z` clock.
    let scheduled_at = chrono::DateTime::parse_from_rfc3339(&scheduled_at)
        .map_err(|e| format!("scheduled_at must be RFC3339: {e}"))?
        .with_timezone(&chrono::Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let (account, _key) = load_account(state.inner(), &account_id).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO blog_platform_publications
         (id, post_id, platform, account_id, status, scheduled_at, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, 'scheduled', ?5, ?6)
         ON CONFLICT(post_id, platform, account_id) DO UPDATE SET
           status = 'scheduled',
           scheduled_at = excluded.scheduled_at,
           last_synced_at = excluded.last_synced_at,
           error = NULL",
        rusqlite::params![
            id,
            post_id,
            account.platform,
            account_id,
            scheduled_at,
            chrono::Utc::now().to_rfc3339(),
        ],
    )
    .map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_publications
         WHERE post_id = ?1 AND platform = ?2 AND account_id = ?3",
        PUB_COLUMNS
    );
    conn.query_row(
        &sql,
        rusqlite::params![post_id, account.platform, account_id],
        row_to_publication,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn blog_cancel_scheduled(
    state: State<'_, AppStateHandle>,
    publication_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM blog_platform_publications
         WHERE id = ?1 AND status = 'scheduled'",
        rusqlite::params![publication_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Run once at app start. Spawns a low-frequency ticker that every
/// minute picks up any `status='scheduled'` row whose `scheduled_at` is
/// in the past and runs the platform publish for it. `db` is passed in
/// directly so we don't need to synchronously block on the state
/// RwLock from inside Tauri's `.setup()` (which ran the older version
/// on the main thread before the async runtime had fully started).
pub fn spawn_scheduled_publisher(state: AppStateHandle, db: minion_db::Database) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        // Skip the immediate first tick so app startup isn't tagged with
        // unrelated publish noise.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let due = match due_scheduled(&db) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("scheduled publisher: due lookup failed: {e}");
                    continue;
                }
            };
            for (post_id, account_id) in due {
                if let Err(e) = run_scheduled_publish(&state, &post_id, &account_id).await {
                    tracing::warn!(
                        "scheduled publish {}/{}: {}",
                        post_id,
                        account_id,
                        e
                    );
                }
            }
        }
    });
}

fn due_scheduled(db: &minion_db::Database) -> Result<Vec<(String, String)>, String> {
    let conn = db.get().map_err(|e| e.to_string())?;
    // Match the exact format used on insert (`Z` suffix, seconds
    // precision) so lexicographic comparison is a valid time comparison.
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut stmt = conn
        .prepare(
            "SELECT post_id, account_id FROM blog_platform_publications
             WHERE status = 'scheduled' AND scheduled_at <= ?1 AND account_id IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([&now], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

async fn run_scheduled_publish(
    state: &AppStateHandle,
    post_id: &str,
    account_id: &str,
) -> Result<(), String> {
    // Mirror blog_publish_to_platform but without the State wrapper.
    let (account, api_key) = load_account(state, account_id).await?;
    let post = load_post(state, post_id).await?;
    let key = api_key.filter(|k| !k.is_empty());
    let result = match account.platform.as_str() {
        "wordpress" => publish_wordpress(&post, &account, key.as_deref()).await,
        "devto" | "dev_to" | "forem" => publish_devto(&post, key.as_deref()).await,
        "hashnode" => publish_hashnode(&post, &account, key.as_deref()).await,
        "ghost" => publish_ghost(&post, &account, key.as_deref()).await,
        "webhook" => publish_webhook(&post, &account, key.as_deref()).await,
        _ => return Ok(()), // manual platforms can't be scheduled
    };
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    match result {
        Ok((remote_url, remote_id, status)) => {
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "UPDATE blog_platform_publications
                 SET status = ?1, remote_id = COALESCE(?2, remote_id),
                     remote_url = COALESCE(?3, remote_url),
                     published_at = COALESCE(published_at, ?4),
                     last_synced_at = ?4, error = NULL
                 WHERE post_id = ?5 AND platform = ?6 AND account_id = ?7",
                rusqlite::params![
                    status,
                    remote_id,
                    remote_url,
                    now,
                    post_id,
                    account.platform,
                    account_id
                ],
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        Err(e) => {
            conn.execute(
                "UPDATE blog_platform_publications
                 SET status = 'failed', error = ?1, last_synced_at = ?2
                 WHERE post_id = ?3 AND platform = ?4 AND account_id = ?5",
                rusqlite::params![
                    e,
                    chrono::Utc::now().to_rfc3339(),
                    post_id,
                    account.platform,
                    account_id
                ],
            )
            .map_err(|e2| e2.to_string())?;
            Err(e)
        }
    }
}

// Auto social snippet ================================================

/// Derive a ~280-char social teaser from markdown content. Strips code
/// fences and HTML tags, squeezes whitespace, and trims at a word
/// boundary with an ellipsis.
pub fn social_snippet(markdown: &str) -> String {
    const LIMIT: usize = 280;
    let mut plain = String::with_capacity(markdown.len());
    let mut in_fence = false;
    let mut in_tag = false;
    for ch in markdown.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            continue;
        }
        if in_tag {
            continue;
        }
        plain.push(ch);
    }
    let mut out = String::with_capacity(LIMIT);
    for line in plain.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || t.is_empty() {
            continue;
        }
        // Strip markdown heading/list markers + links.
        let cleaned = t
            .trim_start_matches('#')
            .trim_start()
            .trim_start_matches("- ")
            .trim_start_matches("* ");
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(cleaned);
        if out.len() >= LIMIT {
            break;
        }
    }
    // Collapse whitespace.
    let mut collapsed = String::with_capacity(out.len());
    let mut prev_ws = false;
    for ch in out.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                collapsed.push(' ');
            }
            prev_ws = true;
        } else {
            collapsed.push(ch);
            prev_ws = false;
        }
    }
    if collapsed.len() <= LIMIT {
        return collapsed.trim().to_string();
    }
    // Reserve 3 bytes for the UTF-8 ellipsis so the returned string
    // stays within LIMIT.
    const ELLIPSIS: &str = "…";
    let budget = LIMIT.saturating_sub(ELLIPSIS.len());
    let mut end = budget;
    while end > 0 && !collapsed.is_char_boundary(end) {
        end -= 1;
    }
    let truncated = &collapsed[..end];
    // Only back up to the last space if it's *close* to the truncation
    // point — otherwise a pathological early-space input (or CJK with
    // no spaces at all) degenerates to a single-word snippet.
    let last_space = truncated.rfind(' ');
    let keep_until = match last_space {
        Some(s) if end.saturating_sub(s) < 60 => s,
        _ => end,
    };
    let base = &truncated[..keep_until];
    format!(
        "{}{}",
        base.trim_end_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace()),
        ELLIPSIS
    )
}

/// Regenerate the `social_snippet` column for a single post. Called
/// from the UI "Refresh snippet" button and on save.
#[tauri::command]
pub async fn blog_regenerate_social_snippet(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<String, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let content: String = conn
        .query_row(
            "SELECT COALESCE(content, '') FROM blog_posts WHERE id = ?1",
            rusqlite::params![post_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let snippet = social_snippet(&content);
    conn.execute(
        "UPDATE blog_posts SET social_snippet = ?1 WHERE id = ?2",
        rusqlite::params![snippet, post_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(snippet)
}

#[tauri::command]
pub async fn blog_get_social_snippet(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<String>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT social_snippet FROM blog_posts WHERE id = ?1",
        rusqlite::params![post_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn blog_publish_to_platform(
    state: State<'_, AppStateHandle>,
    post_id: String,
    account_id: String,
) -> Result<PublishResult, String> {
    let (account, api_key) = load_account(state.inner(), &account_id).await?;
    let post = load_post(state.inner(), &post_id).await?;
    let key = api_key.filter(|k| !k.is_empty());

    let (remote_url, remote_id, status) = match account.platform.as_str() {
        "wordpress" => publish_wordpress(&post, &account, key.as_deref()).await?,
        "devto" | "dev_to" | "forem" => publish_devto(&post, key.as_deref()).await?,
        "hashnode" => publish_hashnode(&post, &account, key.as_deref()).await?,
        "ghost" => publish_ghost(&post, &account, key.as_deref()).await?,
        "webhook" => publish_webhook(&post, &account, key.as_deref()).await?,
        other => {
            return Err(format!(
                "platform {} does not support auto-publish; use blog_export_for_platform",
                other
            ))
        }
    };

    // Upsert publication row.
    let pub_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO blog_platform_publications
         (id, post_id, platform, account_id, status, remote_id, remote_url,
          published_at, last_synced_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(post_id, platform, account_id) DO UPDATE SET
           status = excluded.status,
           remote_id = COALESCE(excluded.remote_id, remote_id),
           remote_url = COALESCE(excluded.remote_url, remote_url),
           published_at = COALESCE(excluded.published_at, published_at),
           last_synced_at = excluded.last_synced_at,
           error = NULL",
        rusqlite::params![
            pub_id,
            post.id,
            account.platform,
            account_id,
            status,
            remote_id,
            remote_url,
            now,
        ],
    )
    .map_err(|e| e.to_string())?;

    // Return whichever id now owns the row (may differ from pub_id on conflict).
    let final_id: String = conn
        .query_row(
            "SELECT id FROM blog_platform_publications
             WHERE post_id = ?1 AND platform = ?2 AND COALESCE(account_id, '') = COALESCE(?3, '')",
            rusqlite::params![post.id, account.platform, Some(&account_id)],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(PublishResult {
        publication_id: final_id,
        status,
        remote_url,
        remote_id,
    })
}

#[tauri::command]
pub async fn blog_test_platform_connection(
    state: State<'_, AppStateHandle>,
    account_id: String,
) -> Result<bool, String> {
    let (account, api_key) = load_account(state.inner(), &account_id).await?;
    let key = api_key.unwrap_or_default();
    match account.platform.as_str() {
        "wordpress" => test_wordpress(&account, &key).await,
        "devto" | "dev_to" | "forem" => test_devto(&key).await,
        "hashnode" => test_hashnode(&account, &key).await,
        "ghost" => test_ghost(&account, &key).await,
        "webhook" => test_webhook(&account, &key).await,
        _ => Ok(true), // manual-export platforms have no connection to test
    }
}

// =====================================================================
// WordPress (REST v2)
// =====================================================================

async fn publish_wordpress(
    post: &LoadedPost,
    account: &PlatformAccount,
    api_key: Option<&str>,
) -> Result<(Option<String>, Option<String>, String), String> {
    let base = account.base_url.as_deref().ok_or("WordPress base_url missing")?;
    let user = account
        .account_label
        .as_deref()
        .ok_or("WordPress needs the username in account_label (used for basic auth)")?;
    let key = api_key.ok_or("WordPress application password missing")?;

    let url = format!("{}/wp-json/wp/v2/posts", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "title": post.title,
        "slug": post.slug,
        "content": post.content,
        "excerpt": post.excerpt,
        "status": "publish",
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .basic_auth(user, Some(key))
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("WordPress POST failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let remote_id = v.get("id").and_then(|n| n.as_i64()).map(|n| n.to_string());
    let remote_url = v.get("link").and_then(|s| s.as_str()).map(|s| s.to_string());
    Ok((remote_url, remote_id, "published".to_string()))
}

async fn test_wordpress(account: &PlatformAccount, key: &str) -> Result<bool, String> {
    let base = account.base_url.as_deref().ok_or("missing base_url")?;
    let user = account
        .account_label
        .as_deref()
        .ok_or("missing account_label (username)")?;
    let url = format!("{}/wp-json/wp/v2/users/me", base.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .basic_auth(user, Some(key))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.status().is_success())
}

// =====================================================================
// Dev.to (REST, /api/articles)
// =====================================================================

async fn publish_devto(
    post: &LoadedPost,
    api_key: Option<&str>,
) -> Result<(Option<String>, Option<String>, String), String> {
    let key = api_key.ok_or("Dev.to API key missing")?;
    // Dev.to caps tags at 4.
    let tags: Vec<String> = post.tags.iter().take(4).cloned().collect();
    let body = serde_json::json!({
        "article": {
            "title": post.title,
            "body_markdown": with_canonical_footer(&post.content, post.canonical_url.as_deref()),
            "published": false, // always draft; lets the user review on dev.to
            "tags": tags,
            "canonical_url": post.canonical_url,
        }
    });
    let resp = reqwest::Client::new()
        .post("https://dev.to/api/articles")
        .header("api-key", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("Dev.to POST failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let remote_id = v.get("id").and_then(|n| n.as_i64()).map(|n| n.to_string());
    let remote_url = v.get("url").and_then(|s| s.as_str()).map(|s| s.to_string());
    Ok((remote_url, remote_id, "draft".to_string()))
}

async fn test_devto(api_key: &str) -> Result<bool, String> {
    let resp = reqwest::Client::new()
        .get("https://dev.to/api/users/me")
        .header("api-key", api_key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.status().is_success())
}

// =====================================================================
// Hashnode (GraphQL)
// =====================================================================

async fn publish_hashnode(
    post: &LoadedPost,
    account: &PlatformAccount,
    api_key: Option<&str>,
) -> Result<(Option<String>, Option<String>, String), String> {
    let key = api_key.ok_or("Hashnode API key missing")?;
    let pub_id = account
        .publication_id
        .as_deref()
        .ok_or("Hashnode publication_id missing")?;

    let tags_obj: Vec<serde_json::Value> = post
        .tags
        .iter()
        .take(5)
        .map(|t| serde_json::json!({ "slug": t, "name": t }))
        .collect();

    let mutation = r#"
      mutation PublishPost($input: PublishPostInput!) {
        publishPost(input: $input) {
          post { id slug url }
        }
      }
    "#;
    let body = serde_json::json!({
        "query": mutation,
        "variables": {
            "input": {
                "title": post.title,
                "contentMarkdown": post.content,
                "publicationId": pub_id,
                "tags": tags_obj,
                "originalArticleURL": post.canonical_url,
                "slug": post.slug,
            }
        }
    });
    let resp = reqwest::Client::new()
        .post("https://gql.hashnode.com/")
        .header("Authorization", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("Hashnode call failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(errs) = v.get("errors").and_then(|e| e.as_array()).filter(|a| !a.is_empty()) {
        return Err(format!("Hashnode returned GraphQL errors: {:?}", errs));
    }
    let post_obj = v
        .pointer("/data/publishPost/post")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let remote_id = post_obj.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let remote_url = post_obj.get("url").and_then(|x| x.as_str()).map(|s| s.to_string());
    Ok((remote_url, remote_id, "published".to_string()))
}

async fn test_hashnode(account: &PlatformAccount, key: &str) -> Result<bool, String> {
    let _ = account;
    let body = serde_json::json!({
        "query": "query { me { id username } }"
    });
    let resp = reqwest::Client::new()
        .post("https://gql.hashnode.com/")
        .header("Authorization", key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Ok(false);
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok(v.pointer("/data/me/id").is_some())
}

// =====================================================================
// Ghost (Admin API, HS256 JWT)
// =====================================================================

/// Ghost admin API keys look like `<hex24>:<hex64>`. The first half is
/// the key id (→ JWT `kid`) and the second is the HS256 secret encoded
/// as hex.
fn parse_ghost_key(raw: &str) -> Result<(String, Vec<u8>), String> {
    let (id, secret_hex) = raw
        .split_once(':')
        .ok_or("Ghost key must look like '<id>:<hex-secret>'")?;
    let secret = (0..secret_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&secret_hex[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
        .map_err(|e| format!("Ghost key secret is not valid hex: {e}"))?;
    Ok((id.to_string(), secret))
}

fn b64url(data: &[u8]) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
    B64.encode(data)
}

fn ghost_jwt(id: &str, secret: &[u8]) -> Result<String, String> {
    use hmac::{Hmac, Mac};
    type HmacSha256 = Hmac<sha2::Sha256>;

    let header = serde_json::json!({"alg":"HS256","typ":"JWT","kid": id});
    let now = chrono::Utc::now().timestamp();
    // Back-date `iat` by 10s so mild clock skew between this machine
    // and the Ghost server doesn't cause "Invalid token" rejections.
    let payload = serde_json::json!({
        "iat": now - 10,
        "exp": now + 5 * 60,
        "aud": "/admin/",
    });
    let enc_header = b64url(serde_json::to_string(&header).map_err(|e| e.to_string())?.as_bytes());
    let enc_payload = b64url(serde_json::to_string(&payload).map_err(|e| e.to_string())?.as_bytes());
    let signing_input = format!("{}.{}", enc_header, enc_payload);
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|e| e.to_string())?;
    mac.update(signing_input.as_bytes());
    let sig = b64url(&mac.finalize().into_bytes());
    Ok(format!("{}.{}", signing_input, sig))
}

async fn publish_ghost(
    post: &LoadedPost,
    account: &PlatformAccount,
    api_key: Option<&str>,
) -> Result<(Option<String>, Option<String>, String), String> {
    let base = account
        .base_url
        .as_deref()
        .ok_or("Ghost base_url missing (e.g. https://yourblog.com)")?;
    let key = api_key.ok_or("Ghost admin API key missing")?;
    let (id, secret) = parse_ghost_key(key)?;
    let jwt = ghost_jwt(&id, &secret)?;

    // Ghost's Admin API does not accept a `markdown` field. The post
    // body must be `mobiledoc`/`lexical`/`html`. We send HTML + the
    // `?source=html` query flag so Ghost converts once at publish.
    let html = markdown_to_html(&post.content);
    let body = serde_json::json!({
        "posts": [{
            "title": post.title,
            "slug": post.slug,
            "status": "published",
            "html": html,
            "custom_excerpt": post.excerpt,
            "canonical_url": post.canonical_url,
            "tags": post.tags.iter().map(|t| serde_json::json!({"name": t})).collect::<Vec<_>>(),
        }]
    });
    let url = format!(
        "{}/ghost/api/admin/posts/?source=html",
        base.trim_end_matches('/')
    );
    let resp = reqwest::Client::new()
        .post(&url)
        .header("Authorization", format!("Ghost {}", jwt))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("Ghost POST failed ({}): {}", s, b));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let first = v
        .pointer("/posts/0")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let remote_id = first.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let remote_url = first.get("url").and_then(|x| x.as_str()).map(|s| s.to_string());
    Ok((remote_url, remote_id, "published".to_string()))
}

async fn test_ghost(account: &PlatformAccount, key: &str) -> Result<bool, String> {
    let base = account.base_url.as_deref().ok_or("missing base_url")?;
    let (id, secret) = parse_ghost_key(key)?;
    let jwt = ghost_jwt(&id, &secret)?;
    let url = format!("{}/ghost/api/admin/site/", base.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .header("Authorization", format!("Ghost {}", jwt))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.status().is_success())
}

/// Minimal markdown → HTML converter for the Ghost publish path. Handles
/// headings, fenced code, paragraphs, inline code, bold/italic, and
/// links. Not a full CommonMark renderer — intentionally no table or
/// list support yet, since most blog posts are prose + code.
pub(crate) fn markdown_to_html(md: &str) -> String {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }
    let mut out = String::with_capacity(md.len() + md.len() / 4);
    let mut in_fence = false;
    let mut fence_lang = String::new();
    let mut fence_buf = String::new();
    let mut para = String::new();

    fn flush_para(out: &mut String, para: &mut String) {
        let text = para.trim();
        if text.is_empty() {
            para.clear();
            return;
        }
        out.push_str("<p>");
        out.push_str(&inline_md(text));
        out.push_str("</p>\n");
        para.clear();
    }

    for line in md.lines() {
        if in_fence {
            if line.trim_start().starts_with("```") {
                out.push_str(&format!(
                    "<pre><code class=\"language-{}\">{}</code></pre>\n",
                    escape(fence_lang.trim()),
                    escape(fence_buf.trim_end())
                ));
                in_fence = false;
                fence_lang.clear();
                fence_buf.clear();
            } else {
                fence_buf.push_str(line);
                fence_buf.push('\n');
            }
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            flush_para(&mut out, &mut para);
            in_fence = true;
            fence_lang = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_para(&mut out, &mut para);
            out.push_str(&format!("<h1>{}</h1>\n", inline_md(rest.trim())));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_para(&mut out, &mut para);
            out.push_str(&format!("<h2>{}</h2>\n", inline_md(rest.trim())));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_para(&mut out, &mut para);
            out.push_str(&format!("<h3>{}</h3>\n", inline_md(rest.trim())));
            continue;
        }
        if line.trim().is_empty() {
            flush_para(&mut out, &mut para);
            continue;
        }
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(line.trim_end());
    }
    flush_para(&mut out, &mut para);
    if in_fence {
        // Unclosed fence: emit what we captured so we don't drop content.
        out.push_str(&format!(
            "<pre><code class=\"language-{}\">{}</code></pre>\n",
            escape(fence_lang.trim()),
            escape(fence_buf.trim_end())
        ));
    }
    out
}

fn inline_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'&' => out.push_str("&amp;"),
            b'<' => out.push_str("&lt;"),
            b'>' => out.push_str("&gt;"),
            b'`' => {
                if let Some(end) = s[i + 1..].find('`') {
                    let code = &s[i + 1..i + 1 + end];
                    out.push_str("<code>");
                    out.push_str(
                        &code
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;"),
                    );
                    out.push_str("</code>");
                    i += end + 2;
                    continue;
                } else {
                    out.push('`');
                }
            }
            b'*' if bytes.get(i + 1) == Some(&b'*') => {
                if let Some(end) = s[i + 2..].find("**") {
                    let inner = &s[i + 2..i + 2 + end];
                    out.push_str(&format!("<strong>{}</strong>", inline_md(inner)));
                    i += end + 4;
                    continue;
                } else {
                    out.push('*');
                }
            }
            b'*' | b'_' => {
                let c = bytes[i] as char;
                if let Some(end) = s[i + 1..].find(c) {
                    let inner = &s[i + 1..i + 1 + end];
                    out.push_str(&format!("<em>{}</em>", inline_md(inner)));
                    i += end + 2;
                    continue;
                } else {
                    out.push(c);
                }
            }
            b'[' => {
                if let Some(close_bracket) = s[i..].find("](") {
                    let link_text = &s[i + 1..i + close_bracket];
                    let after = &s[i + close_bracket + 2..];
                    if let Some(close_paren) = after.find(')') {
                        let url = &after[..close_paren];
                        out.push_str(&format!(
                            "<a href=\"{}\">{}</a>",
                            url.replace('"', "&quot;"),
                            inline_md(link_text)
                        ));
                        i += close_bracket + 2 + close_paren + 1;
                        continue;
                    }
                }
                out.push('[');
            }
            c => out.push(c as char),
        }
        i += 1;
    }
    out
}

// =====================================================================
// Custom Webhook
// =====================================================================
//
// The webhook contract is deliberately boring: we POST a JSON body
// describing the post and include a `Bearer` token from the account's
// api_key (optional). This is the escape hatch for self-hosted CMSs,
// n8n, Zapier, etc.

async fn publish_webhook(
    post: &LoadedPost,
    account: &PlatformAccount,
    api_key: Option<&str>,
) -> Result<(Option<String>, Option<String>, String), String> {
    let url = account
        .base_url
        .as_deref()
        .ok_or("Webhook base_url (target URL) missing")?;
    let body = serde_json::json!({
        "id": post.id,
        "title": post.title,
        "slug": post.slug,
        "markdown": post.content,
        "excerpt": post.excerpt,
        "canonical_url": post.canonical_url,
        "tags": post.tags,
    });
    let mut req = reqwest::Client::new().post(url).json(&body);
    if let Some(key) = api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        let s = resp.status();
        let b = resp.text().await.unwrap_or_default();
        return Err(format!("Webhook POST failed ({}): {}", s, b));
    }
    // The webhook MAY return a JSON body with {url,id}; if it does,
    // adopt those values. If not, fall back to marking the call as
    // accepted without a remote URL.
    let remote_url: Option<String>;
    let remote_id: Option<String>;
    if let Ok(v) = resp.json::<serde_json::Value>().await {
        remote_url = v.get("url").and_then(|x| x.as_str()).map(|s| s.to_string());
        remote_id = v.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
    } else {
        remote_url = None;
        remote_id = None;
    }
    Ok((remote_url, remote_id, "published".to_string()))
}

async fn test_webhook(account: &PlatformAccount, key: &str) -> Result<bool, String> {
    let url = account.base_url.as_deref().ok_or("missing base_url")?;
    // Most webhooks won't expose an OPTIONS/GET endpoint, so a "test"
    // here is just: does the URL parse + resolve + accept HEAD without
    // TLS errors? We deliberately do NOT send the key on a test.
    let _ = key;
    let resp = reqwest::Client::new()
        .head(url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    // Accept any non-5xx as "reachable".
    Ok(!resp.status().is_server_error())
}

// =====================================================================
// Manual export transforms
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPayload {
    pub platform: String,
    /// "markdown" | "html" | "text"
    pub format: String,
    pub copy_text: String,
    /// URL to open in the user's browser for paste-to-editor flows.
    pub open_url: Option<String>,
}

#[tauri::command]
pub async fn blog_export_for_platform(
    state: State<'_, AppStateHandle>,
    post_id: String,
    platform: String,
) -> Result<ExportPayload, String> {
    let post = load_post(state.inner(), &post_id).await?;
    Ok(match platform.as_str() {
        "linkedin" => export_linkedin(&post),
        "medium" => export_medium(&post),
        "substack" => export_substack(&post),
        "twitter" | "x" => export_twitter(&post),
        other => ExportPayload {
            platform: other.to_string(),
            format: "markdown".into(),
            copy_text: post.content.clone(),
            open_url: None,
        },
    })
}

/// Mark an export as done in the publications table so the UI matrix
/// shows a status chip. The user gets a `remote_url` blank until they
/// come back and paste the actual URL.
#[tauri::command]
pub async fn blog_mark_exported(
    state: State<'_, AppStateHandle>,
    post_id: String,
    platform: String,
    remote_url: Option<String>,
) -> Result<Publication, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    // Manual-export rows have no account; SQLite UNIQUE treats every
    // NULL as distinct, so we'd never hit the ON CONFLICT path and end
    // up with duplicate rows on repeated clicks. Store a sentinel empty
    // string instead so UNIQUE actually catches duplicates.
    conn.execute(
        "INSERT INTO blog_platform_publications
         (id, post_id, platform, account_id, status, remote_url, published_at, last_synced_at)
         VALUES (?1, ?2, ?3, '', 'exported', ?4, ?5, ?5)
         ON CONFLICT(post_id, platform, account_id) DO UPDATE SET
           status = 'exported',
           remote_url = COALESCE(excluded.remote_url, remote_url),
           last_synced_at = excluded.last_synced_at",
        rusqlite::params![id, post_id, platform, remote_url, now],
    )
    .map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {} FROM blog_platform_publications
         WHERE post_id = ?1 AND platform = ?2 AND account_id = ''",
        PUB_COLUMNS
    );
    conn.query_row(
        &sql,
        rusqlite::params![post_id, platform],
        row_to_publication,
    )
    .map_err(|e| e.to_string())
}

fn export_linkedin(post: &LoadedPost) -> ExportPayload {
    // LinkedIn renders plain text; strip code fences into plain blocks,
    // preserve bullets, cap at 110k characters.
    let mut body = strip_markdown_code_blocks(&post.content);
    body = body.replace("> ", "\u{201C}");
    body.truncate(110_000.min(body.len()));
    let copy = format!("{}\n\n{}", post.title, body);
    ExportPayload {
        platform: "linkedin".into(),
        format: "text".into(),
        copy_text: copy,
        open_url: Some("https://www.linkedin.com/post/new/".into()),
    }
}

fn export_medium(post: &LoadedPost) -> ExportPayload {
    let mut body = post.content.clone();
    if let Some(url) = &post.canonical_url {
        body.push_str(&format!(
            "\n\n---\n*Originally published at [{}]({})*.",
            url, url
        ));
    }
    ExportPayload {
        platform: "medium".into(),
        format: "markdown".into(),
        copy_text: format!("# {}\n\n{}", post.title, body),
        open_url: Some("https://medium.com/new-story".into()),
    }
}

fn export_substack(post: &LoadedPost) -> ExportPayload {
    ExportPayload {
        platform: "substack".into(),
        format: "markdown".into(),
        copy_text: format!("# {}\n\n{}", post.title, post.content),
        open_url: Some("https://substack.com/publish".into()),
    }
}

fn export_twitter(post: &LoadedPost) -> ExportPayload {
    // Naive but useful: break at paragraph boundaries, number 1/n.
    let chunks = chunk_into_tweets(&post.title, &post.content, 270);
    let total = chunks.len();
    let numbered: Vec<String> = chunks
        .into_iter()
        .enumerate()
        .map(|(i, c)| format!("{}/{}  {}", i + 1, total, c))
        .collect();
    ExportPayload {
        platform: "twitter".into(),
        format: "text".into(),
        copy_text: numbered.join("\n\n---\n\n"),
        open_url: Some("https://twitter.com/compose/tweet".into()),
    }
}

fn with_canonical_footer(content: &str, canonical: Option<&str>) -> String {
    match canonical {
        Some(url) => format!(
            "{}\n\n---\n*Originally published at [{}]({}).*",
            content, url, url
        ),
        None => content.to_string(),
    }
}

fn strip_markdown_code_blocks(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let mut in_code = false;
    for line in md.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            out.push_str("— code sample —\n");
            continue;
        }
        if in_code {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        } else {
            // Inline code → strip backticks.
            out.push_str(&line.replace('`', ""));
            out.push('\n');
        }
    }
    out
}

fn chunk_into_tweets(title: &str, body: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let combined = if title.is_empty() {
        body.to_string()
    } else {
        format!("{}\n\n{}", title, body)
    };
    let mut current = String::new();
    for paragraph in combined.split("\n\n") {
        let p = paragraph.trim();
        if p.is_empty() {
            continue;
        }
        if current.is_empty() {
            if p.len() <= limit {
                current = p.to_string();
            } else {
                // Hard-split a too-long paragraph.
                let mut remaining = p;
                while remaining.len() > limit {
                    let mut cut = limit;
                    while !remaining.is_char_boundary(cut) {
                        cut -= 1;
                    }
                    out.push(remaining[..cut].to_string());
                    remaining = &remaining[cut..];
                }
                current = remaining.to_string();
            }
        } else if current.len() + 2 + p.len() <= limit {
            current.push_str("\n\n");
            current.push_str(p);
        } else {
            out.push(std::mem::take(&mut current));
            current = p.to_string();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_post(content: &str) -> LoadedPost {
        LoadedPost {
            id: "1".into(),
            title: "Hello World".into(),
            slug: "hello-world".into(),
            content: content.into(),
            excerpt: None,
            canonical_url: None,
            tags: vec!["k8s".into(), "nfs".into()],
        }
    }

    #[test]
    fn linkedin_strips_code_fences() {
        let post = mk_post("Intro.\n\n```rust\nfn main() {}\n```\n\nOutro.");
        let p = export_linkedin(&post);
        assert_eq!(p.format, "text");
        assert!(p.copy_text.contains("— code sample —"));
        assert!(p.copy_text.contains("Outro."));
    }

    #[test]
    fn medium_appends_canonical_footer() {
        let mut post = mk_post("Body.");
        post.canonical_url = Some("https://blog.foo.com/p/hello".into());
        let p = export_medium(&post);
        assert!(p.copy_text.contains("Originally published at"));
        assert!(p.copy_text.contains("blog.foo.com"));
    }

    #[test]
    fn twitter_chunks_and_numbers() {
        let long = "p1".repeat(150);
        let post = mk_post(&format!("{}\n\n{}\n\n{}", long, long, long));
        let p = export_twitter(&post);
        assert!(p.copy_text.contains("1/"));
        assert!(p.copy_text.contains("---"));
    }

    #[test]
    fn chunk_respects_limit() {
        let chunks = chunk_into_tweets("t", "p1\n\np2", 100);
        for c in &chunks {
            assert!(c.len() <= 100, "chunk too long: {} chars", c.len());
        }
    }

    #[test]
    fn canonical_footer_noop_when_missing() {
        assert_eq!(with_canonical_footer("body", None), "body");
    }

    #[test]
    fn social_snippet_respects_limit_and_skips_fences() {
        let md = "# Title\n\n```rust\nfn main() {}\n```\n\n\
                  This is a **paragraph** about kubernetes storage in \
                  production environments.";
        let s = social_snippet(md);
        assert!(!s.contains("fn main"));
        assert!(s.len() <= 280);
        assert!(s.contains("kubernetes"));
    }

    #[test]
    fn social_snippet_truncates_long_content() {
        let long: String = "word ".repeat(200);
        let s = social_snippet(&long);
        assert!(s.len() <= 280);
        assert!(s.ends_with('…'), "expected ellipsis, got: {s:?}");
    }

    #[test]
    fn ghost_jwt_parses_id_and_signs() {
        // 24-hex + ":" + 64-hex. The secret half must decode; we don't
        // assert exact JWT output (depends on iat), just that parse +
        // sign succeed.
        let id = "606e7a3a5f2e2c0123456789";
        let secret = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let full = format!("{}:{}", id, secret);
        let (parsed_id, parsed_secret) = parse_ghost_key(&full).unwrap();
        assert_eq!(parsed_id, id);
        assert_eq!(parsed_secret.len(), 32);
        let token = ghost_jwt(&parsed_id, &parsed_secret).unwrap();
        // Three dot-separated segments.
        assert_eq!(token.matches('.').count(), 2);
    }

    #[test]
    fn ghost_jwt_rejects_malformed_key() {
        assert!(parse_ghost_key("no-colon-here").is_err());
        assert!(parse_ghost_key("id:not-hex!!!").is_err());
    }
}
