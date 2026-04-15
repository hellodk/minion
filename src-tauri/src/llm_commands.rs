//! LLM endpoint management (week 2 foundation, week 3 will wire it into
//! the ingestion pipeline for classification + structured extraction).

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmEndpoint {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    /// Masked representation ("•••••••••") when a key is stored. Never the
    /// real key; the raw value stays inside the database.
    pub api_key: Option<String>,
    pub default_model: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateLlmEndpointRequest {
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: Option<String>,
}

#[tauri::command]
pub async fn llm_list_endpoints(
    state: State<'_, AppStateHandle>,
) -> Result<Vec<LlmEndpoint>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, provider_type, base_url, api_key_encrypted, default_model, enabled
             FROM llm_endpoints ORDER BY created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(LlmEndpoint {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_type: row.get(2)?,
                base_url: row.get(3)?,
                api_key: row
                    .get::<_, Option<String>>(4)?
                    .map(|_| "•••••••••".to_string()),
                default_model: row.get(5)?,
                enabled: row.get::<_, i64>(6)? != 0,
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
pub async fn llm_create_endpoint(
    state: State<'_, AppStateHandle>,
    request: CreateLlmEndpointRequest,
) -> Result<LlmEndpoint, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO llm_endpoints (id, name, provider_type, base_url,
         api_key_encrypted, default_model)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            id,
            request.name,
            request.provider_type,
            request.base_url,
            request.api_key, // TODO: encrypt at rest once vault key is available
            request.default_model,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(LlmEndpoint {
        id,
        name: request.name,
        provider_type: request.provider_type,
        base_url: request.base_url,
        api_key: request.api_key.map(|_| "•••••••••".to_string()),
        default_model: request.default_model,
        enabled: true,
    })
}

#[tauri::command]
pub async fn llm_delete_endpoint(
    state: State<'_, AppStateHandle>,
    endpoint_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM llm_endpoints WHERE id = ?1",
        rusqlite::params![endpoint_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn llm_test_endpoint(
    state: State<'_, AppStateHandle>,
    endpoint_id: String,
) -> Result<bool, String> {
    use minion_llm::{create_provider, EndpointConfig, ProviderType};

    let (provider_type, base_url, api_key, default_model): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = {
        let st = state.read().await;
        let conn = st.db.get().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT provider_type, base_url, api_key_encrypted, default_model
             FROM llm_endpoints WHERE id = ?1",
            rusqlite::params![endpoint_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?
    };

    let pt = match provider_type.as_str() {
        "ollama" => ProviderType::Ollama,
        "openai_compatible" => ProviderType::OpenaiCompatible,
        "openai" => ProviderType::Openai,
        "anthropic" => ProviderType::Anthropic,
        "google_gemini" => ProviderType::GoogleGemini,
        "airllm" => ProviderType::Airllm,
        other => return Err(format!("Unknown provider type: {}", other)),
    };
    let cfg = EndpointConfig {
        provider_type: pt,
        base_url,
        api_key,
        default_model: default_model.unwrap_or_default(),
        extra_headers: Default::default(),
    };
    let provider = create_provider(cfg);
    provider.health_check().await.map_err(|e| e.to_string())
}
