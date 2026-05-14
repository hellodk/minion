// src-tauri/src/presentation_commands.rs
use minion_presentation::schema::types::{DeckPatch, DeckSummary, GenerationConfig};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::RwLock;

use crate::state::AppState;

type AppStateHandle<'a> = State<'a, Arc<RwLock<AppState>>>;

/// Start AI generation. Stubbed — real implementation in AI Pipeline sub-plan.
#[tauri::command]
pub async fn start_presentation_generation(
    _inputs: serde_json::Value,
    _config: GenerationConfig,
    state: AppStateHandle<'_>,
    _app: AppHandle,
) -> Result<String, String> {
    let _guard = state.read().await;
    Ok(uuid::Uuid::new_v4().to_string())
}

/// Interrupt a running generation. Stubbed.
#[tauri::command]
pub async fn interrupt_generation(
    _session_id: String,
    _after_agent: String,
    _instruction: String,
    state: AppStateHandle<'_>,
) -> Result<(), String> {
    let _guard = state.read().await;
    Ok(())
}

/// Load a deck by ID. Stubbed.
#[tauri::command]
pub async fn get_deck(
    _id: String,
    state: AppStateHandle<'_>,
) -> Result<serde_json::Value, String> {
    let _guard = state.read().await;
    Err("not yet implemented — filled in AI Pipeline sub-plan".into())
}

/// Apply patches to a deck. Stubbed.
#[tauri::command]
pub async fn save_deck_patch(
    _id: String,
    _patches: Vec<DeckPatch>,
    state: AppStateHandle<'_>,
) -> Result<(), String> {
    let _guard = state.read().await;
    Ok(())
}

/// List all presentations for the library view.
#[tauri::command]
pub async fn list_presentations(
    state: AppStateHandle<'_>,
) -> Result<Vec<DeckSummary>, String> {
    let guard = state.read().await;
    guard
        .presentation_db
        .list_presentations()
        .map_err(|e| e.to_string())
}

/// Export a deck. Stubbed.
#[tauri::command]
pub async fn export_presentation(
    _id: String,
    _format: String,
    _output_path: String,
    state: AppStateHandle<'_>,
) -> Result<serde_json::Value, String> {
    let _guard = state.read().await;
    Err("export not yet implemented — filled in Export sub-plan".into())
}
