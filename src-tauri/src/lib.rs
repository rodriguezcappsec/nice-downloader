mod commands;
mod database;
mod engines;
mod errors;
mod models;
mod queue;
mod utils;

use crate::{database::Database, queue::QueueManager};
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
            let database = Database::open(&data_directory.join("nice-downloader.sqlite3"))?;
            let queue = QueueManager::new(database.clone(), 2);
            app.manage(database);
            app.manage(queue);
            tracing::info!(path = %data_directory.display(), "application backend initialized");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::detect_engines,
            commands::analyze_url,
            commands::get_engine_preferences,
            commands::update_engine_preferences,
            commands::get_default_destination,
            commands::start_download,
            commands::cancel_download,
            commands::list_jobs,
            commands::list_history,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!(%error, "application runtime exited with an error");
    }
}
