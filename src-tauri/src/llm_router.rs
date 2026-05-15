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
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};

pub type AppStateHandle = Arc<RwLock<AppState>>;

// -------------------------------------------------------------------------
// Model-list cache  (C1/C2 fix)
// -------------------------------------------------------------------------
// Avoids an extra HTTP round-trip on every LLM call for simple features.
// TTL = 60s — model list changes only when the user installs/removes a model.

const CACHE_TTL: Duration = Duration::from_secs(60);

static MODEL_LIST_CACHE: LazyLock<Mutex<HashMap<String, (Instant, Vec<String>)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

/// Parse a provider_type string to the enum. Centralised so that Debug-format
/// matching (fragile, C5 fix) is never used — callers call this instead.
fn parse_provider_type(s: &str) -> Result<ProviderType, String> {
    match s {
        "ollama"            => Ok(ProviderType::Ollama),
        "openai_compatible" => Ok(ProviderType::OpenaiCompatible),
        "openai"            => Ok(ProviderType::Openai),
        "anthropic"         => Ok(ProviderType::Anthropic),
        "google_gemini"     => Ok(ProviderType::GoogleGemini),
        "airllm"            => Ok(ProviderType::Airllm),
        other               => Err(format!("Unknown provider type: {other}")),
    }
}

fn to_config(ep: StoredEndpoint, model_override: Option<String>) -> Result<EndpointConfig, String> {
    let pt = parse_provider_type(&ep.provider_type)?;
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
// Tier 1: Runtime capability probe
// -------------------------------------------------------------------------

/// Stored capability record for one (endpoint, model) pair.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelCapability {
    pub endpoint_id: String,
    pub model_name: String,
    pub is_thinking: bool,
    pub supports_nothink: bool,
    pub context_window: Option<i64>,
    pub probed_at: String,
    pub probe_success: bool,
}

/// Probe a single model: send a minimal test message and observe whether
/// the response includes a non-empty `thinking` field (Ollama format) or
/// `reasoning_content` (OpenAI-compatible reasoning models).
///
/// If `is_thinking`, also test the `/nothink` directive to see if the model
/// supports suppressing chain-of-thought.
///
/// Returns the capability record. Does NOT write to DB — caller persists.
pub async fn probe_model_capabilities(
    endpoint_id: &str,
    provider_type: &str,
    base_url: &str,
    api_key: Option<&str>,
    model_name: &str,
) -> ModelCapability {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(_) => return ModelCapability {
            endpoint_id: endpoint_id.to_string(),
            model_name: model_name.to_string(),
            is_thinking: false,
            supports_nothink: false,
            context_window: None,
            probed_at: chrono::Utc::now().to_rfc3339(),
            probe_success: false,
        },
    };

    let mut is_thinking = false;
    let mut supports_nothink = false;
    let mut context_window: Option<i64> = None;
    let mut probe_success = false;
    let base = base_url.trim_end_matches('/');

    match provider_type {
        "ollama" => {
            // Probe 1: simple chat — does it produce thinking tokens?
            let body = serde_json::json!({
                "model": model_name,
                "messages": [{"role": "user", "content": "1+1="}],
                "stream": false,
                "options": {"num_predict": 32}
            });
            if let Ok(resp) = client.post(format!("{base}/api/chat")).json(&body).send().await {
                if resp.status().is_success() {
                    probe_success = true;
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        let thinking = json["message"]["thinking"].as_str().unwrap_or("");
                        is_thinking = !thinking.is_empty();
                    }
                }
            }

            // Probe 2: if thinking, test /nothink suppression
            if is_thinking {
                let body2 = serde_json::json!({
                    "model": model_name,
                    "messages": [
                        {"role": "system", "content": "/nothink"},
                        {"role": "user", "content": "1+1="}
                    ],
                    "stream": false,
                    "options": {"num_predict": 32}
                });
                if let Ok(resp2) = client.post(format!("{base}/api/chat")).json(&body2).send().await {
                    if resp2.status().is_success() {
                        if let Ok(json2) = resp2.json::<serde_json::Value>().await {
                            let thinking2 = json2["message"]["thinking"].as_str().unwrap_or("");
                            supports_nothink = thinking2.is_empty();
                        }
                    }
                }
            }

            // Probe 3: fetch context window from /api/show
            let show_body = serde_json::json!({"name": model_name});
            if let Ok(resp3) = client.post(format!("{base}/api/show")).json(&show_body).send().await {
                if let Ok(json3) = resp3.json::<serde_json::Value>().await {
                    context_window = json3["model_info"]["llama.context_length"]
                        .as_i64()
                        .or_else(|| json3["parameters"].as_str()
                            .and_then(|p| {
                                p.lines().find(|l| l.starts_with("num_ctx"))
                                    .and_then(|l| l.split_whitespace().nth(1))
                                    .and_then(|v| v.parse::<i64>().ok())
                            }));
                }
            }
        }
        "openai_compatible" | "openai" => {
            let mut req_builder = client.post(format!("{base}/v1/chat/completions"));
            if let Some(k) = api_key { if !k.is_empty() { req_builder = req_builder.bearer_auth(k); } }
            let body = serde_json::json!({
                "model": model_name,
                "messages": [{"role": "user", "content": "1+1="}],
                "stream": false,
                "max_tokens": 32
            });
            if let Ok(resp) = req_builder.json(&body).send().await {
                if resp.status().is_success() {
                    probe_success = true;
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        // Some providers put reasoning in reasoning_content
                        let rc = json["choices"][0]["message"]["reasoning_content"]
                            .as_str().unwrap_or("");
                        is_thinking = !rc.is_empty();
                    }
                }
            }
        }
        _ => {}
    }

    ModelCapability {
        endpoint_id: endpoint_id.to_string(),
        model_name: model_name.to_string(),
        is_thinking,
        supports_nothink,
        context_window,
        probed_at: chrono::Utc::now().to_rfc3339(),
        probe_success,
    }
}

/// Read cached capabilities for a model from the DB.
/// Returns None if not yet probed or if the probe is stale (>7 days).
fn read_cached_capability(
    conn: &rusqlite::Connection,
    endpoint_id: &str,
    model_name: &str,
) -> Option<ModelCapability> {
    const STALE_DAYS: i64 = 7;
    conn.query_row(
        "SELECT endpoint_id, model_name, is_thinking, supports_nothink,
                context_window, probed_at, probe_success
         FROM llm_model_capabilities
         WHERE endpoint_id = ?1 AND model_name = ?2
           AND julianday('now') - julianday(probed_at) < ?3",
        rusqlite::params![endpoint_id, model_name, STALE_DAYS],
        |r| Ok(ModelCapability {
            endpoint_id: r.get(0)?,
            model_name: r.get(1)?,
            is_thinking: r.get::<_, i64>(2)? != 0,
            supports_nothink: r.get::<_, i64>(3)? != 0,
            context_window: r.get(4)?,
            probed_at: r.get(5)?,
            probe_success: r.get::<_, i64>(6)? != 0,
        }),
    ).ok()
}

/// Write a probe result into the DB.
fn write_capability(
    conn: &rusqlite::Connection,
    cap: &ModelCapability,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO llm_model_capabilities
             (endpoint_id, model_name, is_thinking, supports_nothink,
              context_window, probed_at, probe_success)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        rusqlite::params![
            cap.endpoint_id, cap.model_name,
            cap.is_thinking as i64, cap.supports_nothink as i64,
            cap.context_window, cap.probed_at,
            cap.probe_success as i64,
        ],
    )?;
    Ok(())
}

// -------------------------------------------------------------------------
// Task classification + model intelligence
// -------------------------------------------------------------------------

/// Tasks that benefit from a fast, direct response model rather than a
/// slow reasoning/thinking model. The router auto-downgrades for these.
const SIMPLE_FEATURES: &[&str] = &[
    "explorer_format_md",
    "blog_llm_grammar",
    "blog_llm_meta",
    "blog_llm_tags",
    "blog_llm_hook",
    "blog_llm_conclusion",
    "blog_llm_snippets",
    "blog_llm_tone",
];

/// Returns true if the model name pattern suggests it is a chain-of-thought
/// reasoning model that produces "thinking" tokens before content.
///
/// IMPORTANT: deepseek-v3 is NOT a reasoning model (only deepseek-r1 is).
/// qwen3 IS a reasoning model by default but supports /nothink suppression.
fn is_thinking_model(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("qwen3")
        || n.contains("deepseek-r1")   // NOT deepseek-v3 (C3 fix)
        || n.starts_with("r1-")
        || n.contains("-r1:")
        || n.contains("-r1-")
        || n.contains(":r1")
        || n.contains("thinking")
        || n.starts_with("o1-")
        || n.starts_with("o3-")
        || n.contains("qwq")
        || n.contains("marco-o1")
}

/// Returns true if this thinking model supports the `/nothink` system-prompt
/// directive to disable chain-of-thought for simple tasks (C4 fix).
fn supports_nothink_directive(name: &str) -> bool {
    name.to_lowercase().contains("qwen3")
}

/// Prepend `/nothink` to the system prompt to disable Qwen3's reasoning mode.
/// This is faster than model-switching: same model, no thinking overhead.
pub(crate) fn maybe_inject_nothink(model: &str, system: &str) -> String {
    if supports_nothink_directive(model) && !system.starts_with("/nothink") {
        format!("/nothink\n\n{system}")
    } else {
        system.to_string()
    }
}

/// Score a model for quality-vs-speed on simple text tasks (H1/H2 fix).
/// Lower score = preferred. Models below a quality floor are excluded.
/// Returns None if the model should not be used for chat (embedding models etc.)
fn simple_task_score(name: &str) -> Option<i32> {
    let n = name.to_lowercase();
    // Never use embedding or multimodal-only models for text chat
    if n.contains("embed") || n.contains("nomic") || n.contains("clip") { return None; }
    // Code-specialised models are fine for markdown with code blocks (H2 fix — no penalty)
    // Minimum quality floor: avoid tiny models (<7B) for formatting tasks (H1 fix)
    if n.contains("1b") || n.contains("1.5b") || n.contains("2b") || n.contains("3b") {
        return None; // too small for reliable markdown formatting
    }
    // Score by parameter count — prefer mid-size for simple tasks
    let size_score = if n.contains("7b") || n.contains("8b")  { 10 }
        else if n.contains("13b") || n.contains("14b") { 15 }
        else if n.contains("30b") || n.contains("32b") { 25 }
        else if n.contains("70b") || n.contains("72b") { 40 }
        else { 10 }; // unknown — treat as 7B class
    Some(size_score)
}

/// Fetch the list of models available on an Ollama or OpenAI-compatible endpoint.
/// Results are cached for CACHE_TTL to avoid a round-trip on every call (C1 fix).
async fn fetch_available_models(
    provider_type: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Vec<String> {
    // Check cache first
    {
        let cache = MODEL_LIST_CACHE.lock().await;
        if let Some((ts, models)) = cache.get(base_url) {
            if ts.elapsed() < CACHE_TTL {
                return models.clone();
            }
        }
    }

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![], // M3 fix: don't fall back to a client with no timeout
    };
    let base = base_url.trim_end_matches('/');
    let models: Vec<String> = match provider_type {
        "ollama" => {
            if let Ok(resp) = client.get(format!("{base}/api/tags")).send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    json["models"].as_array().unwrap_or(&vec![])
                        .iter()
                        .filter_map(|m| m["name"].as_str().map(str::to_string))
                        .collect()
                } else { vec![] }
            } else { vec![] }
        }
        "openai_compatible" | "openai" => {
            let mut req = client.get(format!("{base}/v1/models"));
            if let Some(k) = api_key { if !k.is_empty() { req = req.bearer_auth(k); } }
            if let Ok(resp) = req.send().await {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    json["data"].as_array().unwrap_or(&vec![])
                        .iter()
                        .filter_map(|m| m["id"].as_str().map(str::to_string))
                        .collect()
                } else { vec![] }
            } else { vec![] }
        }
        _ => vec![],
    };

    // Store in cache
    if !models.is_empty() {
        let mut cache = MODEL_LIST_CACHE.lock().await;
        cache.insert(base_url.to_string(), (Instant::now(), models.clone()));
    }
    models
}

/// Look up whether a model is a thinking model.
/// Tier 1: DB probe result (ground truth, refreshed every 7 days).
/// Tier 2: name-pattern heuristic (fallback, triggers background probe).
/// Returns (is_thinking, supports_nothink).
async fn model_thinking_status(
    state: &AppStateHandle,
    endpoint_id: &str,
    provider_type: &str,
    base_url: &str,
    api_key: Option<&str>,
    model_name: &str,
) -> (bool, bool) {
    // Tier 1: check DB
    {
        let st = state.read().await;
        if let Ok(conn) = st.db.get() {
            if let Some(cap) = read_cached_capability(&conn, endpoint_id, model_name) {
                if cap.probe_success {
                    return (cap.is_thinking, cap.supports_nothink);
                }
            }
        }
    }
    // DB miss — spawn a background probe so next call gets ground truth
    {
        let state_clone = state.clone();
        let eid = endpoint_id.to_string();
        let pt = provider_type.to_string();
        let url = base_url.to_string();
        let key = api_key.map(str::to_string);
        let model = model_name.to_string();
        tauri::async_runtime::spawn(async move {
            let cap = probe_model_capabilities(&eid, &pt, &url, key.as_deref(), &model).await;
            if let Ok(st) = state_clone.read().await.db.get() {
                let _ = write_capability(&st, &cap);
            }
            tracing::debug!(
                "llm_router: probed '{}' — thinking={} nothink={}",
                cap.model_name, cap.is_thinking, cap.supports_nothink
            );
        });
    }
    // Tier 2: name-pattern heuristic while probe runs in background
    let thinking = is_thinking_model(model_name);
    let nothink = supports_nothink_directive(model_name);
    (thinking, nothink)
}

/// For simple tasks on an endpoint whose default model is a reasoning model,
/// prefer a non-thinking alternative if available.
///
/// G4 fix: if `user_override` is true the user explicitly chose this model.
/// C4 fix: Qwen3 → inject /nothink rather than switch models.
/// Tier 1: uses DB probe results for model classification.
async fn pick_best_model(
    feature: &str,
    cfg: EndpointConfig,
    user_override: bool,
    state: &AppStateHandle,
) -> EndpointConfig {
    if user_override { return cfg; }
    if !SIMPLE_FEATURES.contains(&feature) { return cfg; }

    // Need the endpoint_id to query the capabilities DB.
    // Resolve it from the DB using base_url as a lookup key.
    let endpoint_id = {
        let st = state.read().await;
        st.db.get().ok()
            .and_then(|conn| conn.query_row(
                "SELECT id FROM llm_endpoints WHERE base_url = ?1 LIMIT 1",
                rusqlite::params![cfg.base_url],
                |r| r.get::<_, String>(0),
            ).ok())
            .unwrap_or_default()
    };

    let pt_str = match cfg.provider_type {
        ProviderType::Ollama           => "ollama",
        ProviderType::OpenaiCompatible | ProviderType::Openai | ProviderType::Airllm => "openai_compatible",
        _                              => "other",
    };

    let (is_thinking, nothink_ok) = model_thinking_status(
        state, &endpoint_id, pt_str, &cfg.base_url, cfg.api_key.as_deref(), &cfg.default_model,
    ).await;

    if !is_thinking { return cfg; }

    // Qwen3 and models supporting /nothink: keep model, inject directive at call site
    if nothink_ok {
        return cfg;
    }

    // Other thinking models: look for a capable non-thinking alternative
    let models = fetch_available_models(pt_str, &cfg.base_url, cfg.api_key.as_deref()).await;

    // Filter using Tier 1 probe results where available, name patterns as fallback
    let mut candidates: Vec<(i32, String)> = vec![];
    for m in &models {
        if m == &cfg.default_model { continue; }
        let (m_thinking, _) = model_thinking_status(
            state, &endpoint_id, pt_str, &cfg.base_url, cfg.api_key.as_deref(), m,
        ).await;
        if m_thinking { continue; }
        if let Some(score) = simple_task_score(m) {
            candidates.push((score, m.clone()));
        }
    }
    candidates.sort_by_key(|(s, _)| *s);

    match candidates.first() {
        Some((_, model)) => {
            tracing::info!(
                "llm_router: auto-selected '{}' over '{}' for simple task '{}'",
                model, cfg.default_model, feature
            );
            EndpointConfig { default_model: model.clone(), ..cfg }
        }
        None => cfg,
    }
}

// -------------------------------------------------------------------------
// Public API
// -------------------------------------------------------------------------

/// Resolve the `EndpointConfig` for `feature`. Returns `None` when no
/// enabled endpoint exists at all (callers decide how to surface this).
///
/// For simple features (formatting, grammar, etc.) the router automatically
/// prefers a fast non-thinking model over a reasoning model if both are
/// available on the endpoint.
pub async fn resolve(
    state: &AppStateHandle,
    feature: &str,
) -> Result<Option<EndpointConfig>, String> {
    let stored = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        query_endpoint(&conn, feature)
    };
    let (ep, model_override) = match stored { Some(s) => s, None => return Ok(None) };
    let user_override = model_override.as_ref().map_or(false, |m| !m.trim().is_empty());
    let cfg = to_config(ep, model_override)?;

    // Auto-select a better model for simple tasks when no explicit override is set.
    // Passes state so pick_best_model can read/write the capabilities DB.
    Ok(Some(pick_best_model(feature, cfg, user_override, state).await))
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

    // Build the initial config to run it through pick_best_model so the
    // pre-click indicator shows the actual model that will be used.
    let raw_model = model_override
        .filter(|m| !m.trim().is_empty())
        .or_else(|| ep.default_model.as_ref().filter(|m| !m.trim().is_empty()).cloned())
        .unwrap_or_default();
    drop(conn); // release DB lock before async model fetch
    drop(st);
    let user_override = !raw_model.is_empty() && ep.default_model.as_deref().map_or(true, |d| d != raw_model);
    let initial_cfg = to_config(
        StoredEndpoint {
            provider_type: ep.provider_type.clone(),
            base_url: ep.base_url.clone(),
            api_key: ep.api_key.clone(),
            default_model: Some(raw_model),
            extra_headers: ep.extra_headers.clone(),
        },
        None,
    )?;
    let resolved = pick_best_model(feature, initial_cfg, user_override, state).await;
    let model = resolved.default_model.clone();
    let ep_base_url = ep.base_url.clone();
    let ep_provider_type = ep.provider_type.clone();

    let is_cloud = matches!(
        ep_provider_type.as_str(),
        "openai" | "anthropic" | "google_gemini"
    ) || (!ep_base_url.contains("localhost") && !ep_base_url.contains("127.0.0.1") && !ep_base_url.contains("::1"));

    Ok(Some(LlmFeatureStatus {
        feature: feature.to_string(),
        endpoint_name: name,
        model,
        provider_type: ep_provider_type,
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
// Public capability management commands (called from Settings UI)
// -------------------------------------------------------------------------

/// Probe all models on a specific endpoint and store the results.
/// Returns the list of capability records. Safe to call repeatedly.
pub async fn probe_endpoint_capabilities(
    state: &AppStateHandle,
    endpoint_id: &str,
) -> Result<Vec<ModelCapability>, String> {
    let (pt, base_url, api_key) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT provider_type, base_url, api_key_encrypted FROM llm_endpoints WHERE id = ?1",
            rusqlite::params![endpoint_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)),
        ).map_err(|e| format!("Endpoint not found: {e}"))?
    };

    let models = fetch_available_models(&pt, &base_url, api_key.as_deref()).await;
    if models.is_empty() {
        return Err("No models found on endpoint — is it reachable?".to_string());
    }

    let mut results = Vec::new();
    for model in &models {
        let cap = probe_model_capabilities(endpoint_id, &pt, &base_url, api_key.as_deref(), model).await;
        {
            let st = state.read().await;
            if let Ok(conn) = st.db.get() {
                let _ = write_capability(&conn, &cap);
            }
        }
        tracing::info!(
            "llm_router: probed '{}' on '{}' — thinking={} nothink={}",
            cap.model_name, endpoint_id, cap.is_thinking, cap.supports_nothink
        );
        results.push(cap);
    }
    Ok(results)
}

/// Read all stored capability records for an endpoint.
pub async fn get_endpoint_capabilities(
    state: &AppStateHandle,
    endpoint_id: &str,
) -> Result<Vec<ModelCapability>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT endpoint_id, model_name, is_thinking, supports_nothink,
                context_window, probed_at, probe_success
         FROM llm_model_capabilities WHERE endpoint_id = ?1
         ORDER BY model_name ASC",
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(rusqlite::params![endpoint_id], |r| {
        Ok(ModelCapability {
            endpoint_id: r.get(0)?,
            model_name: r.get(1)?,
            is_thinking: r.get::<_, i64>(2)? != 0,
            supports_nothink: r.get::<_, i64>(3)? != 0,
            context_window: r.get(4)?,
            probed_at: r.get(5)?,
            probe_success: r.get::<_, i64>(6)? != 0,
        })
    }).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;
    Ok(rows)
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
                    let content_token = json["message"]["content"].as_str().unwrap_or("");
                    let thinking_token = json["message"]["thinking"].as_str().unwrap_or("");

                    if !content_token.is_empty() {
                        // Actual response content — stream to editor
                        full_content.push_str(content_token);
                        emit("chunk", Some(content_token.to_string()), None, None, elapsed());
                    } else if !thinking_token.is_empty() {
                        // Reasoning model (e.g. qwen3) is in thinking phase.
                        // Emit a "thinking" stage event so the UI shows progress
                        // without updating editor content.
                        emit("thinking", None, None, None, elapsed());
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
        // M1 fix: process lines using cursor to avoid reallocating the buffer tail
        let mut start = 0;
        while let Some(rel) = buf[start..].find('\n') {
            let end = start + rel;
            let line = buf[start..end].trim();
            start = end + 1;
            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    return Ok(full_content);
                }
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
        buf.drain(..start);
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
    // C5 fix: store the string from DB (not Debug format) for reliable matching
    // We need to extract provider_type before converting to the enum.
    // Use a helper that maps the enum back to its canonical string.
    let pt_str = match cfg.provider_type {
        ProviderType::Ollama           => "ollama",
        ProviderType::OpenaiCompatible | ProviderType::Openai | ProviderType::Airllm => "openai_compatible",
        ProviderType::Anthropic        => "anthropic",
        ProviderType::GoogleGemini     => "google_gemini",
    };
    let pt = pt_str.to_string();
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

        // C4 fix: inject /nothink for Qwen3 to suppress chain-of-thought on simple tasks
        let effective_system = maybe_inject_nothink(&model_name, &system);

        // H3 fix: hard cap on total streaming time (300s) so leaked tasks don't run forever
        let work = async {
            match pt.as_str() {
                "ollama" => {
                    stream_ollama(
                        &base_url, &effective_system, &user, &model_name,
                        temperature, max_tokens, &emit, &elapsed,
                    ).await
                }
                "openai_compatible" => {
                    stream_openai(
                        &base_url, api_key.as_deref(), &effective_system, &user, &model_name,
                        temperature, max_tokens, &emit, &elapsed,
                    ).await
                }
                _ => {
                    emit("generating", None, None, None, elapsed());
                    use minion_llm::types::{ChatMessage, ChatRequest};
                    let provider = create_provider(cfg_clone);
                    let req = ChatRequest {
                        messages: vec![ChatMessage::user(user.clone())],
                        system: Some(effective_system.clone()),
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
            }
        };

        let result = tokio::time::timeout(std::time::Duration::from_secs(300), work)
            .await
            .unwrap_or(Err("Streaming timed out after 5 minutes".to_string()));

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
