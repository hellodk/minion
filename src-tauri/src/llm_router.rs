//! Central LLM smart router for the MINION app.
//!
//! Every module that needs an LLM (blog, health, explorer, sysmon, …) should
//! call one of the public functions here instead of querying `llm_endpoints`
//! directly. This guarantees that:
//!
//! - The user's per-feature endpoint binding (`llm_feature_bindings`) is
//!   always respected.
//! - The `llama3` hardcoded fallback is gone in exactly one place.
//! - Adding a new provider or routing strategy only requires changing this file.
//!
//! ## Resolution order
//! 1. `llm_feature_bindings.endpoint_id` for the requested `feature` name
//!    (user can pin a specific endpoint+model per task in Settings).
//! 2. First enabled endpoint in `llm_endpoints` ordered by `created_at ASC`.
//!
//! ## Feature names used across the app
//! | Module | Feature string |
//! |--------|---------------|
//! | Health extract | `"health_extract"` |
//! | Health analysis | `"health_analyze"` |
//! | Health intelligence | `"health_intelligence"` |
//! | Blog LLM titles | `"blog_llm_titles"` |
//! | Blog LLM hook | `"blog_llm_hook"` |
//! | Blog LLM conclusion | `"blog_llm_conclusion"` |
//! | Blog LLM grammar | `"blog_llm_grammar"` |
//! | Blog LLM meta desc | `"blog_llm_meta"` |
//! | Blog LLM tags | `"blog_llm_tags"` |
//! | Blog LLM snippets | `"blog_llm_snippets"` |
//! | Blog LLM adapt | `"blog_llm_adapt"` |
//! | Blog LLM tone | `"blog_llm_tone"` |
//! | System monitor analysis | `"sysmon_analyze"` |
//! | Explorer Markdown fix | `"explorer_format_md"` |

use crate::state::AppState;
use minion_llm::{create_provider, EndpointConfig, ProviderType};
use serde::Serialize;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;

pub type AppStateHandle = Arc<RwLock<AppState>>;

// -------------------------------------------------------------------------
// Internal helpers
// -------------------------------------------------------------------------

struct StoredEndpoint {
    provider_type: String,
    base_url: String,
    api_key: Option<String>,
    default_model: Option<String>,
    extra_headers: Option<String>,
}

/// Returns the endpoint row and the optional model override from the binding.
fn query_endpoint(
    conn: &rusqlite::Connection,
    feature: &str,
) -> Option<(StoredEndpoint, Option<String>)> {
    // Step 1: check per-feature binding (also fetches model_override).
    let bound: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT endpoint_id, model_override FROM llm_feature_bindings WHERE feature = ?1",
            rusqlite::params![feature],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .ok();

    let row_to_stored = |row: &rusqlite::Row<'_>| -> rusqlite::Result<StoredEndpoint> {
        Ok(StoredEndpoint {
            provider_type: row.get(0)?,
            base_url: row.get(1)?,
            api_key: row.get(2)?,
            default_model: row.get(3)?,
            extra_headers: row.get(4)?,
        })
    };

    if let Some((id, model_override)) = bound {
        let hit = conn.query_row(
            "SELECT provider_type, base_url, api_key_encrypted, default_model, extra_headers
             FROM llm_endpoints WHERE id = ?1 AND enabled = 1",
            rusqlite::params![id],
            row_to_stored,
        );
        if let Ok(ep) = hit {
            return Some((ep, model_override));
        }
    }

    // Step 2: first enabled endpoint (stable ordering by creation time).
    conn.query_row(
        "SELECT provider_type, base_url, api_key_encrypted, default_model, extra_headers
         FROM llm_endpoints WHERE enabled = 1 ORDER BY created_at ASC LIMIT 1",
        [],
        row_to_stored,
    )
    .ok()
    .map(|ep| (ep, None))
}

fn to_config(ep: StoredEndpoint, model_override: Option<String>) -> Result<EndpointConfig, String> {
    let pt = match ep.provider_type.as_str() {
        "ollama"           => ProviderType::Ollama,
        "openai_compatible" => ProviderType::OpenaiCompatible,
        "openai"           => ProviderType::Openai,
        "anthropic"        => ProviderType::Anthropic,
        "google_gemini"    => ProviderType::GoogleGemini,
        "airllm"           => ProviderType::Airllm,
        other => return Err(format!("Unknown provider type: {other}")),
    };
    let extra_headers = ep.extra_headers
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();
    // model_override from the binding takes precedence over the endpoint's default_model.
    let default_model = model_override
        .filter(|m| !m.is_empty())
        .or(ep.default_model)
        .unwrap_or_default();
    Ok(EndpointConfig {
        provider_type: pt,
        base_url: ep.base_url,
        api_key: ep.api_key,
        default_model,
        extra_headers,
    })
}

// -------------------------------------------------------------------------
// Public API
// -------------------------------------------------------------------------

/// Resolve the `EndpointConfig` for `feature`. Returns `None` when no
/// enabled endpoint exists at all (callers decide how to surface this).
pub async fn resolve(
    state: &AppStateHandle,
    feature: &str,
) -> Result<Option<EndpointConfig>, String> {
    let stored = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        query_endpoint(&conn, feature)
    };
    stored.map(|(ep, model_override)| to_config(ep, model_override)).transpose()
}

// -------------------------------------------------------------------------
// LLM status query
// -------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct LlmFeatureStatus {
    pub feature: String,
    pub endpoint_name: String,
    pub model: String,
    pub provider_type: String,
    pub is_cloud: bool,
}

/// Returns human-readable info about which endpoint/model would be used for
/// a given feature. Used by the frontend to show "Will use: llama3 on Ollama"
/// before the user clicks the AI button.
pub async fn get_feature_status(
    state: &AppStateHandle,
    feature: &str,
) -> Result<Option<LlmFeatureStatus>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let stored = query_endpoint(&conn, feature);
    let Some((ep, model_override)) = stored else { return Ok(None); };

    // Get the endpoint name from llm_endpoints
    let name: String = conn.query_row(
        "SELECT name FROM llm_endpoints WHERE base_url = ?1 LIMIT 1",
        rusqlite::params![ep.base_url],
        |r| r.get(0),
    ).unwrap_or_else(|_| ep.provider_type.clone());

    let model = model_override
        .filter(|m| !m.trim().is_empty())
        .or_else(|| ep.default_model.as_ref().filter(|m| !m.trim().is_empty()).cloned())
        .unwrap_or_else(|| "default".to_string());

    let is_cloud = matches!(
        ep.provider_type.as_str(),
        "openai" | "anthropic" | "google_gemini"
    ) || (!ep.base_url.contains("localhost") && !ep.base_url.contains("127.0.0.1") && !ep.base_url.contains("::1"));

    Ok(Some(LlmFeatureStatus {
        feature: feature.to_string(),
        endpoint_name: name,
        model,
        provider_type: ep.provider_type,
        is_cloud,
    }))
}

// -------------------------------------------------------------------------
// Streaming event type
// -------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct LlmStreamEvent {
    pub call_id: String,
    pub stage: String,        // "connecting" | "generating" | "chunk" | "done" | "error" | "warning"
    pub chunk: Option<String>, // token chunk for "chunk" events
    pub content: Option<String>, // full content for "done" events
    pub model: Option<String>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

// -------------------------------------------------------------------------
// Error translation
// -------------------------------------------------------------------------

/// Translate raw LLM error strings into human-readable messages.
pub(crate) fn translate_llm_error(raw: &str) -> String {
    if raw.contains("404") && raw.contains("model") {
        "Model not found. Go to Settings → LLM Endpoints and set a valid default model.".to_string()
    } else if raw.contains("401") || raw.contains("Unauthorized") || raw.contains("invalid_api_key") {
        "API key rejected. Check your API key in Settings → LLM Endpoints.".to_string()
    } else if raw.contains("timed out") || raw.contains("timeout") {
        "Request timed out. The model may be overloaded or the file too large. Try a faster model.".to_string()
    } else if raw.contains("connection refused") || raw.contains("Cannot reach") {
        "Cannot reach the LLM endpoint. Make sure Ollama is running or check your URL in Settings.".to_string()
    } else if raw.contains("rate limit") || raw.contains("429") {
        "Rate limit reached. Wait a moment and try again, or use a different endpoint.".to_string()
    } else if raw.contains("context") && (raw.contains("length") || raw.contains("too long")) {
        "Content too long for this model. Try a shorter file or a model with a larger context window.".to_string()
    } else {
        format!("AI request failed: {raw}")
    }
}

// -------------------------------------------------------------------------
// Streaming helpers
// -------------------------------------------------------------------------

/// Stream from Ollama's /api/chat endpoint (newline-delimited JSON).
async fn stream_ollama(
    base_url: &str,
    system: &str,
    user: &str,
    model: &str,
    temperature: Option<f32>,
    _max_tokens: Option<u32>,
    emit: &impl Fn(&str, Option<String>, Option<String>, Option<String>, u64),
    elapsed: &impl Fn() -> u64,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ],
        "stream": true
    });
    if let Some(t) = temperature {
        body["options"] = serde_json::json!({"temperature": t});
    }

    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let resp = client.post(&url).json(&body).send().await
        .map_err(|e| format!("Cannot reach Ollama: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    emit("generating", None, None, None, elapsed());

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut full_content = String::new();
    let mut byte_buf: Vec<u8> = Vec::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        byte_buf.extend_from_slice(&bytes);
        // Process complete lines
        while let Some(pos) = byte_buf.iter().position(|&b| b == b'\n') {
            let line_bytes: Vec<u8> = byte_buf.drain(..=pos).collect();
            if let Ok(line) = std::str::from_utf8(&line_bytes) {
                let line = line.trim();
                if line.is_empty() { continue; }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    let token = json["message"]["content"].as_str().unwrap_or("").to_string();
                    if !token.is_empty() {
                        full_content.push_str(&token);
                        emit("chunk", Some(token), None, None, elapsed());
                    }
                    if json["done"].as_bool().unwrap_or(false) {
                        return Ok(full_content);
                    }
                }
            }
        }
    }
    Ok(full_content)
}

/// Stream from OpenAI-compatible /v1/chat/completions (SSE).
async fn stream_openai(
    base_url: &str,
    api_key: Option<&str>,
    system: &str,
    user: &str,
    model: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    emit: &impl Fn(&str, Option<String>, Option<String>, Option<String>, u64),
    elapsed: &impl Fn() -> u64,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let mut body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": user}
        ],
        "stream": true
    });
    if let Some(t) = temperature { body["temperature"] = serde_json::json!(t); }
    if let Some(m) = max_tokens  { body["max_tokens"]  = serde_json::json!(m); }

    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let mut req = client.post(&url).json(&body);
    if let Some(k) = api_key { if !k.is_empty() { req = req.bearer_auth(k); } }

    let resp = req.send().await.map_err(|e| format!("Cannot reach endpoint: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {text}"));
    }

    emit("generating", None, None, None, elapsed());

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut full_content = String::new();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        // Process SSE lines
        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" { return Ok(full_content); }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                    let token = json["choices"][0]["delta"]["content"]
                        .as_str().unwrap_or("").to_string();
                    if !token.is_empty() {
                        full_content.push_str(&token);
                        emit("chunk", Some(token), None, None, elapsed());
                    }
                }
            }
        }
    }
    Ok(full_content)
}

// -------------------------------------------------------------------------
// Streaming public call
// -------------------------------------------------------------------------

/// Streaming call: emits `llm-stream` events to the frontend via Tauri event system.
/// Returns immediately after validation; actual work happens in a spawned task.
///
/// Event stages:
///   connecting  → request is being prepared and sent
///   generating  → model has started responding
///   chunk       → a token or chunk of text (for streaming providers)
///   done        → generation complete; content has the full result
///   error       → something failed
pub async fn stream_call(
    app: AppHandle,
    state: &AppStateHandle,
    feature: String,
    call_id: String,
    system: String,
    user: String,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
    _cancellation_key: String,
) -> Result<(), String> {
    use tauri::Emitter;

    let cfg = resolve(state, &feature).await?.ok_or_else(|| {
        "No enabled LLM endpoint. Add one in Settings → LLM Endpoints.".to_string()
    })?;

    let model_name = cfg.default_model.clone();
    let base_url = cfg.base_url.clone();
    let api_key = cfg.api_key.clone();
    let pt = format!("{:?}", cfg.provider_type);
    let cfg_clone = cfg.clone();

    let call_id_clone = call_id.clone();
    let model_name_clone = model_name.clone();

    // Emit connecting event immediately
    let _ = app.emit("llm-stream", LlmStreamEvent {
        call_id: call_id.clone(),
        stage: "connecting".to_string(),
        chunk: None,
        content: None,
        model: Some(model_name.clone()),
        elapsed_ms: 0,
        error: None,
    });

    tauri::async_runtime::spawn(async move {
        use std::time::Instant;
        let start = Instant::now();
        let elapsed = || start.elapsed().as_millis() as u64;

        let app2 = app.clone();
        let cid = call_id_clone.clone();
        let mname = model_name_clone.clone();

        let emit = move |stage: &str, chunk: Option<String>, content: Option<String>, error: Option<String>, elapsed_ms: u64| {
            let _ = app2.emit("llm-stream", LlmStreamEvent {
                call_id: cid.clone(),
                stage: stage.to_string(),
                chunk,
                content,
                model: Some(mname.clone()),
                elapsed_ms,
                error,
            });
        };

        let result = match pt.as_str() {
            "Ollama" => {
                stream_ollama(
                    &base_url, &system, &user, &model_name,
                    temperature, max_tokens, &emit, &elapsed,
                ).await
            }
            "OpenaiCompatible" | "Openai" => {
                stream_openai(
                    &base_url, api_key.as_deref(), &system, &user, &model_name,
                    temperature, max_tokens, &emit, &elapsed,
                ).await
            }
            _ => {
                // Non-streaming fallback for Anthropic/Gemini
                emit("generating", None, None, None, elapsed());
                use minion_llm::types::{ChatMessage, ChatRequest};
                let provider = create_provider(cfg_clone);
                let req = ChatRequest {
                    messages: vec![ChatMessage::user(user.clone())],
                    system: Some(system.clone()),
                    model: None,
                    temperature,
                    max_tokens,
                    json_mode: false,
                };
                match tokio::time::timeout(
                    std::time::Duration::from_secs(120),
                    provider.chat(req),
                ).await {
                    Ok(Ok(resp)) => Ok(resp.content),
                    Ok(Err(e))   => Err(e.to_string()),
                    Err(_)       => Err("LLM request timed out after 120 seconds".to_string()),
                }
            }
        };

        match result {
            Ok(content) => emit("done", None, Some(content), None, elapsed()),
            Err(e)      => emit("error", None, None, Some(translate_llm_error(&e)), elapsed()),
        }
    });

    Ok(())
}

// -------------------------------------------------------------------------
// Internal chat implementation shared by `call` and `call_with`
// -------------------------------------------------------------------------

async fn call_impl(
    state: &AppStateHandle,
    feature: &str,
    system: &str,
    user: &str,
    temperature: Option<f32>,
    max_tokens: Option<u32>,
) -> Result<String, String> {
    use minion_llm::types::{ChatMessage, ChatRequest};

    let cfg = resolve(state, feature).await?.ok_or_else(|| {
        "No enabled LLM endpoint. Add one in Settings → LLM Endpoints.".to_string()
    })?;

    let mut last_err = String::new();
    for attempt in 0..2u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
        let provider = create_provider(cfg.clone());
        let req = ChatRequest {
            messages: vec![ChatMessage::user(user.to_string())],
            system: Some(system.to_string()),
            model: None,
            temperature,
            max_tokens,
            json_mode: false,
        };
        use tokio::time::{timeout, Duration};
        let res = timeout(Duration::from_secs(60), provider.chat(req))
            .await
            .map_err(|_| "LLM request timed out after 60 seconds".to_string())
            .and_then(|r| r.map_err(|e| e.to_string()));

        match res {
            Ok(resp) => return Ok(resp.content),
            Err(e) => {
                // Don't retry on auth/config errors
                if e.contains("401") || e.contains("404") || e.contains("invalid_api_key") {
                    return Err(translate_llm_error(&e));
                }
                last_err = e;
            }
        }
    }
    Err(translate_llm_error(&last_err))
}

/// Issue a single-turn chat request routed through the smart router.
///
/// The `feature` string selects the endpoint (see module doc for the table).
/// `system` is the instruction/persona prompt; `user` is the message content.
/// Returns the model's reply text, or an error string.
pub async fn call(
    state: &AppStateHandle,
    feature: &str,
    system: &str,
    user: &str,
) -> Result<String, String> {
    call_impl(state, feature, system, user, None, None).await
}

/// Like `call` but with explicit temperature and max_tokens.
pub async fn call_with(
    state: &AppStateHandle,
    feature: &str,
    system: &str,
    user: &str,
    temperature: f32,
    max_tokens: u32,
) -> Result<String, String> {
    call_impl(state, feature, system, user, Some(temperature), Some(max_tokens)).await
}
