mod commands;
mod database;
mod engines;
mod errors;
mod models;
mod queue;
mod subscriptions;
mod utils;

use crate::{database::Database, engines::managed, queue::QueueManager};
use tauri::Manager;
use tracing_subscriber::EnvFilter;

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("nice_downloader=info")),
        )
        .try_init();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_directory = app.path().app_data_dir().map_err(|error| {
                format!("could not resolve the application data directory: {error}")
            })?;
            std::fs::create_dir_all(&data_directory).map_err(|error| {
                format!("could not create the application data directory: {error}")
            })?;
            managed::initialize(&data_directory)?;
            let database = Database::open(&data_directory.join("nice-downloader.sqlite3"))?;
            let queue = QueueManager::new(database.clone())?;
            let recovery_queue = queue.clone();
            let recovery_app = app.handle().clone();
            let recovery_preferences = database.load_engine_preferences()?;
            tauri::async_runtime::spawn(async move {
                recovery_queue
                    .recover_pending(recovery_app, recovery_preferences)
                    .await;
            });
            subscriptions::spawn_scheduler(app.handle().clone(), database.clone(), queue.clone());
            app.manage(database);
            app.manage(queue);
            tracing::info!(path = %data_directory.display(), "application backend initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_engines,
            commands::list_managed_engines,
            commands::check_engine_update,
            commands::install_engine_update,
            commands::pin_managed_engine,
            commands::rollback_managed_engine,
            commands::test_site_access,
            commands::analyze_url,
            commands::list_gallery_items,
            commands::analyze_urls,
            commands::analyze_playlist,
            commands::get_engine_preferences,
            commands::update_engine_preferences,
            commands::get_default_destination,
            commands::start_download,
            commands::start_downloads,
            commands::cancel_download,
            commands::retry_download,
            commands::repeat_download,
            commands::list_jobs,
            commands::get_queue_snapshot,
            commands::update_queue_settings,
            commands::move_queued_download,
            commands::pause_download,
            commands::resume_download,
            commands::check_disk_space,
            commands::list_history,
            commands::delete_history_entry,
            commands::list_presets,
            commands::save_preset,
            commands::delete_preset,
            commands::list_site_preset_rules,
            commands::save_site_preset_rule,
            commands::delete_site_preset_rule,
            commands::match_site_preset_rule,
            commands::find_download_archive_match,
            commands::find_download_archive_matches,
            commands::prepare_collection_destination,
            commands::list_subscriptions,
            commands::save_subscription,
            commands::delete_subscription,
            commands::check_subscription_now,
            commands::open_download_output,
            commands::export_diagnostics,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!(%error, "application runtime exited with an error");
    }
}
