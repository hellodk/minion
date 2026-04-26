//! MINION Tauri Application
//!
//! Main entry point for the Tauri desktop application.

mod blog_assets;
mod blog_import;
mod blog_publish;
mod calendar_integration;
mod commands;
mod health_classify;
mod health_commands;
mod health_entities;
mod health_analysis;
mod health_drive_sync;
mod health_ingestion;
mod health_timeline;
mod llm_commands;
pub mod sysmon_collect;
pub mod sysmon_analysis;
mod sysmon_commands;
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
            let db_handle = state.db.clone();
            let sysmon_db = state.db.clone();
            let state_arc = Arc::new(RwLock::new(state));
            app.manage(state_arc.clone());

            // Background worker: flips blog_platform_publications from
            // scheduled → published when their scheduled_at fires.
            blog_publish::spawn_scheduled_publisher(state_arc, db_handle);

            sysmon_commands::spawn_sysmon_poller(app.handle().clone(), sysmon_db);

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
            commands::reader_list_folder_files,
            commands::reader_import_paths,
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
            // Blog v2 — import + tags + assets
            blog_import::blog_import_files,
            blog_import::blog_import_folder,
            blog_import::blog_confirm_import,
            blog_assets::blog_list_assets,
            blog_assets::blog_get_asset_usage,
            blog_assets::blog_upload_asset,
            blog_assets::blog_delete_orphan_assets,
            blog_assets::blog_get_asset_path,
            blog_assets::blog_list_tags,
            blog_assets::blog_create_tag,
            blog_assets::blog_set_post_tags,
            blog_assets::blog_get_post_tags,
            // Blog v2 — publishing
            blog_publish::blog_list_platform_accounts,
            blog_publish::blog_create_platform_account,
            blog_publish::blog_delete_platform_account,
            blog_publish::blog_list_publications,
            blog_publish::blog_unpublish,
            blog_publish::blog_publish_to_platform,
            blog_publish::blog_test_platform_connection,
            blog_publish::blog_export_for_platform,
            blog_publish::blog_mark_exported,
            blog_publish::blog_schedule_publish,
            blog_publish::blog_cancel_scheduled,
            blog_publish::blog_regenerate_social_snippet,
            blog_publish::blog_get_social_snippet,
            // Health Vault commands (week 1)
            health_commands::health_get_consent,
            health_commands::health_accept_consent,
            health_commands::health_create_patient,
            health_commands::health_list_patients,
            health_commands::health_get_primary_patient,
            health_commands::health_delete_patient,
            health_commands::health_create_record,
            health_commands::health_list_records,
            health_commands::health_delete_record,
            health_commands::health_create_lab_test,
            health_commands::health_list_lab_tests,
            health_commands::health_list_test_names,
            health_commands::health_delete_lab_test,
            health_commands::health_create_medication,
            health_commands::health_list_medications,
            health_commands::health_delete_medication,
            health_commands::health_create_condition,
            health_commands::health_list_conditions,
            health_commands::health_delete_condition,
            health_commands::health_create_vital,
            health_commands::health_list_vitals,
            health_commands::health_delete_vital,
            health_commands::health_create_family_history,
            health_commands::health_list_family_history,
            health_commands::health_delete_family_history,
            health_commands::health_create_life_event,
            health_commands::health_list_life_events,
            health_commands::health_delete_life_event,
            health_commands::health_create_symptom,
            health_commands::health_list_symptoms,
            health_commands::health_resolve_symptom,
            health_commands::health_delete_symptom,
            // Health Vault ingestion (week 2)
            health_ingestion::health_discover_folder,
            health_ingestion::health_start_ingestion,
            health_ingestion::health_get_ingestion_job,
            health_ingestion::health_list_files,
            health_ingestion::health_get_extraction,
            health_ingestion::health_delete_file,
            // LLM endpoint management
            llm_commands::llm_list_endpoints,
            llm_commands::llm_create_endpoint,
            llm_commands::llm_delete_endpoint,
            llm_commands::llm_test_endpoint,
            // Health Vault classification + extraction (week 3)
            health_classify::health_classify_pending,
            health_classify::health_reclassify_file,
            health_classify::health_list_pending_review,
            health_classify::health_save_review,
            // Health Vault entity resolution (week 3)
            health_entities::health_list_entities,
            health_entities::health_merge_entities,
            // Health Vault timeline + episodes + correlations (week 4)
            health_timeline::health_timeline_get,
            health_timeline::health_episode_list,
            health_timeline::health_episode_create,
            health_timeline::health_episode_update,
            health_timeline::health_episode_delete,
            health_timeline::health_episode_attach,
            health_timeline::health_episode_autolink,
            health_timeline::health_classify_symptom,
            health_timeline::health_apply_symptom_classification,
            health_timeline::health_correlate,
            health_timeline::health_list_correlations,
            // Health Vault AI analysis (week 5)
            health_analysis::health_run_analysis,
            health_analysis::health_list_analyses,
            health_analysis::health_delete_analysis,
            health_analysis::health_preview_brief,
            health_analysis::health_analysis_endpoint_status,
            // Health Vault Google Drive sync (week 5)
            health_drive_sync::health_drive_status,
            health_drive_sync::health_drive_save_client_id,
            health_drive_sync::health_drive_set_passphrase,
            health_drive_sync::health_drive_connect,
            health_drive_sync::health_drive_disconnect,
            health_drive_sync::health_drive_backup_now,
            health_drive_sync::health_drive_restore_now,
            health_drive_sync::health_drive_export_local,
            health_drive_sync::health_drive_import_local,
            // System Monitor commands
            sysmon_commands::sysmon_get_current,
            sysmon_commands::sysmon_get_history,
            sysmon_commands::sysmon_list_alerts,
            sysmon_commands::sysmon_resolve_alert,
            sysmon_commands::sysmon_list_processes,
            sysmon_commands::sysmon_kill_process,
            sysmon_commands::sysmon_get_disk_breakdown,
            sysmon_commands::sysmon_run_analysis,
            sysmon_commands::sysmon_list_analyses,
            sysmon_commands::sysmon_get_settings,
            sysmon_commands::sysmon_save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
