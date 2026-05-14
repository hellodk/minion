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
//! | Blog LLM | `"blog_llm"` |
//! | System monitor analysis | `"sysmon_analyze"` |
//! | Explorer Markdown fix | `"explorer_format_md"` |

use crate::state::AppState;
use minion_llm::{create_provider, EndpointConfig, ProviderType};
use std::sync::Arc;
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
    let provider = create_provider(cfg);
    let req = ChatRequest {
        messages: vec![ChatMessage::user(user.to_string())],
        system: Some(system.to_string()),
        model: None,
        temperature,
        max_tokens,
        json_mode: false,
    };
    // CR6: 60-second timeout to prevent indefinite hangs
    use tokio::time::{timeout, Duration};
    let resp = timeout(Duration::from_secs(60), provider.chat(req))
        .await
        .map_err(|_| "LLM request timed out after 60 seconds".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(resp.content)
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
