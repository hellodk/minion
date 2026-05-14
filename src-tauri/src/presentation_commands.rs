// src-tauri/src/presentation_commands.rs
use minion_presentation::{
    bundle,
    input::InputSource,
    schema::types::{DeckId, DeckPatch, DeckSummary, GenerationConfig},
};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::RwLock;

use crate::state::AppState;

type AppStateHandle<'a> = State<'a, Arc<RwLock<AppState>>>;

/// Start AI generation for a new presentation.
/// Returns a session ID that the caller can use to track / interrupt generation.
#[tauri::command]
pub async fn start_presentation_generation(
    inputs: serde_json::Value,
    config: GenerationConfig,
    state: AppStateHandle<'_>,
    _app: AppHandle,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let (event_tx, _) = tokio::sync::broadcast::channel(256);

    let sources: Vec<InputSource> =
        serde_json::from_value(inputs).map_err(|e| e.to_string())?;

    let guard = state.read().await;
    guard
        .cancel_senders
        .lock()
        .await
        .insert(session_id.clone(), cancel_tx);
    let orchestrator = Arc::clone(&guard.orchestrator);
    let sid = session_id.clone();

    tokio::spawn(async move {
        if let Err(e) = orchestrator
            .generate(&sid, sources, config, event_tx, cancel_rx)
            .await
        {
            tracing::error!("generation failed session={sid}: {e:#}");
        }
    });

    Ok(session_id)
}

/// Send a cancellation signal to a running generation session.
#[tauri::command]
pub async fn interrupt_generation(
    session_id: String,
    _after_agent: String,
    _instruction: String,
    state: AppStateHandle<'_>,
) -> Result<(), String> {
    let guard = state.read().await;
    if let Some(tx) = guard
        .cancel_senders
        .lock()
        .await
        .remove(&session_id)
    {
        let _ = tx.send(true);
    }
    Ok(())
}

/// Load a deck bundle by its ID.
#[tauri::command]
pub async fn get_deck(
    id: String,
    state: AppStateHandle<'_>,
) -> Result<serde_json::Value, String> {
    let guard = state.read().await;
    let deck_id = DeckId(uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?);
    let path_str = guard
        .presentation_db
        .get_bundle_path(&deck_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("deck {id} not found"))?;
    let deck =
        bundle::load_bundle(std::path::Path::new(&path_str)).map_err(|e| e.to_string())?;
    serde_json::to_value(deck).map_err(|e| e.to_string())
}

/// Apply a list of patches to an existing deck and persist it.
#[tauri::command]
pub async fn save_deck_patch(
    id: String,
    patches: Vec<DeckPatch>,
    state: AppStateHandle<'_>,
) -> Result<(), String> {
    let guard = state.read().await;
    let deck_id = DeckId(uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?);
    let path_str = guard
        .presentation_db
        .get_bundle_path(&deck_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("deck {id} not found"))?;
    let path = std::path::Path::new(&path_str);
    let mut deck = bundle::load_bundle(path).map_err(|e| e.to_string())?;
    for patch in patches {
        bundle::apply_patch(&mut deck, patch);
    }
    bundle::save_bundle(&deck, path).map_err(|e| e.to_string())?;
    guard
        .presentation_db
        .update_slide_count(&deck_id, deck.slide_count())
        .map_err(|e| e.to_string())
}

/// Return summary rows for the presentations library view.
#[tauri::command]
pub async fn list_presentations(
    state: AppStateHandle<'_>,
) -> Result<Vec<DeckSummary>, String> {
    state
        .read()
        .await
        .presentation_db
        .list_presentations()
        .map_err(|e| e.to_string())
}

/// Export a presentation to the requested format. Not yet implemented.
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
