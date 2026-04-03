//! MINION Tauri Application
//!
//! Main entry point for the Tauri desktop application.

mod calendar_integration;
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
            commands::files_cancel_scan,
            commands::files_list_duplicates,
            commands::files_get_analytics,
            commands::files_open_file,
            commands::files_bulk_delete,
            commands::files_bulk_move,
            commands::files_get_video_metadata,
            // Book Reader commands
            commands::reader_open_book,
            commands::reader_get_chapter,
            commands::reader_prefetch_epub_chapters,
            commands::reader_get_pdf_path,
            commands::reader_get_pdf_bytes,
            commands::reader_list_books,
            // Finance commands
            commands::finance_add_account,
            commands::finance_list_accounts,
            commands::finance_add_transaction,
            commands::finance_list_transactions,
            commands::finance_get_summary,
            commands::finance_import_csv,
            commands::finance_spending_by_category,
            // Investment portfolio commands
            commands::finance_add_investment,
            commands::finance_list_investments,
            commands::finance_portfolio_summary,
            commands::finance_update_price,
            commands::finance_delete_investment,
            commands::finance_fetch_mf_nav,
            commands::finance_calc_cagr,
            // CIBIL score commands
            commands::finance_save_cibil,
            commands::finance_get_cibil,
            // Zerodha Kite Connect commands
            commands::zerodha_save_config,
            commands::zerodha_open_login,
            commands::zerodha_save_token,
            commands::zerodha_fetch_holdings,
            commands::zerodha_sync_to_portfolio,
            // Fitness commands
            commands::fitness_add_habit,
            commands::fitness_list_habits,
            commands::fitness_toggle_habit,
            commands::fitness_log_metric,
            commands::fitness_get_metrics,
            commands::fitness_get_dashboard,
            // Fitness workout & nutrition commands
            commands::fitness_log_workout,
            commands::fitness_list_workouts,
            commands::fitness_delete_workout,
            commands::fitness_log_food,
            commands::fitness_list_nutrition,
            commands::fitness_nutrition_summary,
            commands::fitness_delete_nutrition,
            // Reader persistence commands
            commands::reader_import_book,
            commands::reader_get_library,
            commands::reader_update_progress,
            commands::reader_add_annotation,
            commands::reader_get_annotations,
            // O'Reilly commands
            commands::oreilly_connect_chrome,
            commands::oreilly_connect_sso,
            commands::oreilly_open_browser,
            commands::oreilly_connect_manual,
            commands::oreilly_logout,
            // Collection commands
            commands::reader_create_collection,
            commands::reader_list_collections,
            commands::reader_add_to_collection,
            commands::reader_remove_from_collection,
            commands::reader_get_collection_books,
            commands::reader_delete_collection,
            commands::reader_scan_directory,
            // AI commands
            commands::ai_test_connection,
            commands::ai_analyze_health,
            // Google Fit commands
            commands::gfit_open_auth,
            commands::gfit_sync,
            commands::gfit_save_token,
            commands::gfit_save_client_id,
            commands::gfit_check_connected,
            commands::gfit_disconnect,
            commands::gfit_exchange_auth_code,
            commands::gfit_get_client_id,
            // Calendar commands
            commands::calendar_add_event,
            commands::calendar_list_events,
            commands::calendar_delete_event,
            commands::calendar_list_accounts,
            commands::calendar_google_open_auth,
            commands::calendar_outlook_open_auth,
            commands::calendar_save_outlook_client_id,
            commands::calendar_get_outlook_client_id,
            commands::calendar_remove_account,
            commands::calendar_sync_google,
            commands::calendar_sync_outlook,
            // Media Intelligence commands
            commands::media_import_video,
            commands::media_list_projects,
            commands::media_get_project,
            commands::media_update_project,
            commands::media_delete_project,
            commands::media_open_video,
            commands::media_get_metadata,
            // Blog Engine commands
            commands::blog_create_post,
            commands::blog_list_posts,
            commands::blog_get_post,
            commands::blog_update_post,
            commands::blog_delete_post,
            commands::blog_analyze_seo,
            commands::blog_generate_slug,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
