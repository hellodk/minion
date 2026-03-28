//! MINION Tauri Application
//!
//! Main entry point for the Tauri desktop application.

mod commands;
mod state;

#[cfg(test)]
mod tests;

use state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::RwLock;

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("minion=debug".parse().unwrap())
                .add_directive("tauri=info".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting MINION v{}", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Initialize application state
            let state = AppState::new()?;
            app.manage(Arc::new(RwLock::new(state)));

            // Open DevTools in debug builds
            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                    tracing::info!("DevTools opened for debugging");
                }
            }

            tracing::info!("MINION initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // System commands
            commands::get_system_info,
            commands::get_config,
            commands::set_config,
            // Module commands
            commands::list_modules,
            commands::get_module_status,
            // File Intelligence commands
            commands::files_add_directory,
            commands::files_start_scan,
            commands::files_start_multi_scan,
            commands::files_get_scan_progress,
            commands::files_list_duplicates,
            commands::files_get_analytics,
            commands::files_open_file,
            commands::files_bulk_delete,
            commands::files_bulk_move,
            // Book Reader commands
            commands::reader_open_book,
            commands::reader_list_books,
            // Finance commands
            commands::finance_add_account,
            commands::finance_list_accounts,
            commands::finance_add_transaction,
            commands::finance_list_transactions,
            commands::finance_get_summary,
            commands::finance_import_csv,
            commands::finance_spending_by_category,
            // Fitness commands
            commands::fitness_add_habit,
            commands::fitness_list_habits,
            commands::fitness_toggle_habit,
            commands::fitness_log_metric,
            commands::fitness_get_metrics,
            commands::fitness_get_dashboard,
            // Reader persistence commands
            commands::reader_import_book,
            commands::reader_get_library,
            commands::reader_update_progress,
            commands::reader_add_annotation,
            commands::reader_get_annotations,
            // Collection commands
            commands::reader_create_collection,
            commands::reader_list_collections,
            commands::reader_add_to_collection,
            commands::reader_remove_from_collection,
            commands::reader_get_collection_books,
            commands::reader_delete_collection,
            commands::reader_scan_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
