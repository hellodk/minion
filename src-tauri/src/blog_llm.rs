use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use uuid::Uuid;

type AppStateHandle = Arc<RwLock<AppState>>;
type Conn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogVariant {
    pub id: String,
    pub post_id: String,
    pub variant_type: String,
    pub label: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleSuggestion {
    pub style: String,
    pub title: String,
    pub rationale: String,
}

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model,'llama3')
         FROM llm_endpoints LIMIT 1",
        [],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
            ))
        },
    )
    .ok()
}

async fn call_llm(
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
    system: &str,
    user: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role":"system","content":system},
            {"role":"user","content":user}
        ],
        "stream": false
    });
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .ok()?;
    let mut req = client.post(&url).json(&body);
    if let Some(k) = api_key {
        if !k.is_empty() {
            req = req.bearer_auth(k);
        }
    }
    let resp = req
        .send()
        .await
        .map_err(|e| tracing::warn!("LLM call failed: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        tracing::warn!("LLM returned {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
}

fn fetch_post(conn: &Conn, post_id: &str) -> Result<(String, String), String> {
    conn.query_row(
        "SELECT title, COALESCE(draft_content, content, '') FROM blog_posts WHERE id = ?1",
        params![post_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
    )
    .map_err(|_| format!("Post {} not found", post_id))
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

fn store_variant(
    conn: &Conn,
    post_id: &str,
    variant_type: &str,
    label: &str,
    content: &str,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO blog_post_variants (id, post_id, variant_type, label, content, created_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, post_id, variant_type, label, content, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn blog_llm_titles(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<TitleSuggestion>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let tokens = estimate_tokens(&content);
    let excerpt = if tokens > 2000 {
        &content[..content.len().min(8000)]
    } else {
        &content
    };

    let system = "You are a blog title expert. Respond with exactly 5 lines, each in the format:\n\
                  STYLE: TITLE | RATIONALE\n\
                  Styles: seo, curiosity, direct, question, listicle";
    let user = format!("Current title: {}\n\nContent excerpt:\n{}", title, excerpt);

    let raw = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let suggestions: Vec<TitleSuggestion> = raw
        .lines()
        .filter_map(|line| {
            let (style_title, rationale) = line.split_once(" | ")?;
            let (style, title) = style_title.split_once(": ")?;
            Some(TitleSuggestion {
                style: style.trim().to_lowercase(),
                title: title.trim().to_string(),
                rationale: rationale.trim().to_string(),
            })
        })
        .collect();

    Ok(if suggestions.is_empty() {
        None
    } else {
        Some(suggestions)
    })
}

#[tauri::command]
pub async fn blog_llm_hook(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };

    let first_para: String = content
        .lines()
        .skip_while(|l| l.trim().starts_with('#') || l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if first_para.is_empty() {
        return Ok(None);
    }

    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let system = "You are a blog writing coach. Rewrite the opening paragraph 3 ways to maximise reader retention. \
                  Respond with exactly 3 paragraphs separated by --- (triple dash on its own line). \
                  Styles: 1) Direct/declarative, 2) Story/anecdote, 3) Question-led.";
    let user = format!("Post title: {}\n\nCurrent opening:\n{}", title, first_para);

    let raw = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let variants: Vec<String> = raw
        .split("---")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(if variants.is_empty() {
        None
    } else {
        Some(variants)
    })
}

#[tauri::command]
pub async fn blog_llm_conclusion(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<String>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let system = "You are a blog writing coach. Write a strong 2-3 sentence conclusion paragraph \
                  plus a 1-sentence call to action. Be concise, specific, and direct. \
                  Return only the conclusion text — no preamble.";
    let user = format!(
        "Post title: {}\n\nPost content (last 1000 chars):\n{}",
        title,
        &content[content.len().saturating_sub(1000)..]
    );

    Ok(call_llm(&base_url, api_key.as_deref(), &model, system, &user).await)
}

#[tauri::command]
pub async fn blog_llm_grammar(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (_, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let excerpt = &content[..content.len().min(6000)];
    let system = "You are a grammar and style editor. Find issues: passive voice, weak verbs (is/was/get/got/have), \
                  filler words (very/just/really/quite/thing/stuff), and redundant phrases. \
                  Return each issue on its own line in the format: ISSUE_TYPE: \"original text\" → \"suggested rewrite\"\n\
                  Return at most 15 issues. Return only the issue lines — no preamble or summary.";
    let user = format!("Blog content:\n{}", excerpt);

    let raw = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let issues: Vec<String> = raw
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.contains('\u{2192}'))
        .collect();

    Ok(if issues.is_empty() {
        None
    } else {
        Some(issues)
    })
}

#[tauri::command]
pub async fn blog_llm_meta_description(
    state: State<'_, AppStateHandle>,
    post_id: String,
    save_to_excerpt: bool,
) -> Result<Option<String>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let excerpt = &content[..content.len().min(3000)];
    let system =
        "You are an SEO expert. Write exactly one meta description of 150-160 characters. \
                  Include the primary keyword naturally. Write in active voice. \
                  Return only the description text — no quotes, no preamble.";
    let user = format!("Post title: {}\n\nContent:\n{}", title, excerpt);

    let desc = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(d) => d.trim().to_string(),
        None => return Ok(None),
    };

    if save_to_excerpt && !desc.is_empty() {
        let c = db.get().map_err(|e| e.to_string())?;
        c.execute(
            "UPDATE blog_posts SET excerpt = ?1, updated_at = ?2 WHERE id = ?3",
            params![desc, chrono::Utc::now().to_rfc3339(), post_id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(Some(desc))
}

#[tauri::command]
pub async fn blog_llm_tags(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };

    let existing_tags: Vec<String> = {
        let c = db.get().map_err(|e| e.to_string())?;
        let mut stmt = c
            .prepare("SELECT name FROM blog_tags ORDER BY name")
            .map_err(|e| e.to_string())?;
        let tags: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        tags
    };

    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let excerpt = &content[..content.len().min(3000)];
    let system = "You are a content tagging expert. Return a comma-separated list of 5-8 tags. \
                  Prefer tags from the existing list when relevant. Add new ones only when the post \
                  clearly covers something not in the list. Return only the comma-separated tag names — no explanation.";
    let user = format!(
        "Post title: {}\nExisting tags: {}\n\nContent:\n{}",
        title,
        existing_tags.join(", "),
        excerpt
    );

    let raw = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let tags: Vec<String> = raw
        .split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty() && t.len() < 50)
        .collect();

    Ok(if tags.is_empty() { None } else { Some(tags) })
}

#[tauri::command]
pub async fn blog_llm_snippets(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let excerpt = &content[..content.len().min(3000)];
    let system =
        "You are a social media expert. Generate 4 promotional snippets for a blog post.\n\
                  Return EXACTLY in this format (each on its own line):\n\
                  TWITTER: <270 chars max, hook + emoji>\n\
                  LINKEDIN: <800 chars max, hook + 3 bullet takeaways + hashtags>\n\
                  SUBSTACK: <500 chars max, curiosity-gap teaser>\n\
                  GENERIC: <280 chars max, balanced>\n\
                  Do not include any other text.";
    let user = format!("Post title: {}\n\nContent:\n{}", title, excerpt);

    let raw = match call_llm(&base_url, api_key.as_deref(), &model, system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let mut snippets = serde_json::json!({});
    for line in raw.lines() {
        for key in &["TWITTER", "LINKEDIN", "SUBSTACK", "GENERIC"] {
            let prefix = format!("{}: ", key);
            if line.starts_with(&prefix) {
                snippets[key.to_lowercase()] = serde_json::json!(line[prefix.len()..].trim());
            }
        }
    }

    if snippets.as_object().map(|m| m.is_empty()).unwrap_or(true) {
        return Ok(None);
    }

    let json_str = snippets.to_string();
    let c = db.get().map_err(|e| e.to_string())?;
    c.execute(
        "UPDATE blog_posts SET social_snippets_json = ?1, updated_at = ?2 WHERE id = ?3",
        params![json_str, chrono::Utc::now().to_rfc3339(), post_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(Some(snippets))
}

#[tauri::command]
pub async fn blog_llm_adapt(
    state: State<'_, AppStateHandle>,
    post_id: String,
    platform: String,
) -> Result<Option<BlogVariant>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let platform_instructions = match platform.as_str() {
        "devto" => {
            "Dev.to style: technical, conversational, add a short TL;DR at top, use informal tone, \
             add canonical URL note at the bottom as: 'Originally published at [URL]'"
        }
        "hashnode" => {
            "Hashnode style: add a subtitle below the title, use numbered lists for steps, \
             include a 'Key takeaways' section at the end"
        }
        "medium" => {
            "Medium style: narrative-driven, add scene-setting opening, \
             break up code-heavy sections with more prose explanation"
        }
        "substack" => {
            "Substack newsletter style: personal opener (e.g. 'Hey friends,'), \
             casual conversational tone, end with a personal sign-off and newsletter CTA"
        }
        "linkedin" => {
            "LinkedIn article style: compress to key points, \
             add bold headers for each major point, \
             end with a question to drive comments"
        }
        _ => return Err(format!("Unknown platform: {}", platform)),
    };

    let tokens = estimate_tokens(&content);
    let input = if tokens > 3000 {
        &content[..content.len().min(12000)]
    } else {
        &content
    };

    let system = format!(
        "You are a content adaptation expert. Rewrite the following blog post for {}. \
         Instructions: {}\n\
         Return only the adapted post content in Markdown — no preamble.",
        platform, platform_instructions
    );
    let user = format!("Title: {}\n\n{}", title, input);

    let adapted = match call_llm(&base_url, api_key.as_deref(), &model, &system, &user).await {
        Some(a) => a,
        None => return Ok(None),
    };

    let label = format!("{} adaptation", platform);
    let variant_type = format!("platform_{}", platform);
    let c = db.get().map_err(|e| e.to_string())?;
    let variant_id = store_variant(&c, &post_id, &variant_type, &label, &adapted)?;
    let now = chrono::Utc::now().to_rfc3339();

    Ok(Some(BlogVariant {
        id: variant_id,
        post_id: post_id.clone(),
        variant_type,
        label,
        content: adapted,
        created_at: now,
    }))
}

#[tauri::command]
pub async fn blog_llm_tone(
    state: State<'_, AppStateHandle>,
    post_id: String,
    target: String,
) -> Result<Option<BlogVariant>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };
    let endpoint = {
        let c = db.get().map_err(|e| e.to_string())?;
        get_endpoint(&c)
    };
    let (base_url, api_key, model) = match endpoint {
        Some(v) => v,
        None => return Ok(None),
    };

    let instruction = match target.as_str() {
        "technical" => "formal, precise, use technical terminology, no contractions",
        "balanced" => "clear and professional but approachable, use some contractions",
        "conversational" => "casual, friendly, use contractions, first-person, relatable examples",
        _ => {
            return Err(format!(
                "Unknown tone: {}. Use: technical | balanced | conversational",
                target
            ))
        }
    };

    let tokens = estimate_tokens(&content);
    let input = if tokens > 3000 {
        &content[..content.len().min(12000)]
    } else {
        &content
    };

    let system = format!(
        "Rewrite the following blog post with a {} tone ({}). \
         Preserve all headings, code blocks, and factual content. \
         Only change wording and sentence structure. Return Markdown only.",
        target, instruction
    );
    let user = format!("Title: {}\n\n{}", title, input);

    let rewritten = match call_llm(&base_url, api_key.as_deref(), &model, &system, &user).await {
        Some(r) => r,
        None => return Ok(None),
    };

    let label = format!("{} tone", target);
    let variant_type = format!("tone_{}", target);
    let c = db.get().map_err(|e| e.to_string())?;
    let variant_id = store_variant(&c, &post_id, &variant_type, &label, &rewritten)?;
    let now = chrono::Utc::now().to_rfc3339();

    Ok(Some(BlogVariant {
        id: variant_id,
        post_id,
        variant_type,
        label,
        content: rewritten,
        created_at: now,
    }))
}

#[tauri::command]
pub async fn blog_get_variants(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Vec<BlogVariant>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, post_id, variant_type, label, content, created_at
         FROM blog_post_variants WHERE post_id = ?1 ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<BlogVariant> = stmt
        .query_map(params![post_id], |r| {
            Ok(BlogVariant {
                id: r.get(0)?,
                post_id: r.get(1)?,
                variant_type: r.get(2)?,
                label: r.get(3)?,
                content: r.get(4)?,
                created_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub async fn blog_delete_variant(
    state: State<'_, AppStateHandle>,
    variant_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM blog_post_variants WHERE id = ?1",
        params![variant_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn blog_get_snippets(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let json: Option<String> = conn
        .query_row(
            "SELECT social_snippets_json FROM blog_posts WHERE id = ?1",
            params![post_id],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}
