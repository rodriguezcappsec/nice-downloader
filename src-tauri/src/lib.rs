mod authentication;
mod commands;
mod creative;
mod database;
mod engines;
mod errors;
mod library;
mod local_share;
mod media_server;
mod models;
mod native_player;
mod queue;
mod subscriptions;
mod transcripts;
mod transformations;
mod utils;
mod visual_similarity;

use crate::{
    creative::CreativeManager, database::Database, engines::managed, library::LibraryManager,
    local_share::LocalShareManager, queue::QueueManager, transformations::TransformationManager,
    visual_similarity::VisualSimilarityManager,
};
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[derive(Clone, Copy)]
pub(crate) struct DesktopRuntimeState {
    pub tray_available: bool,
}

fn install_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::TrayIconBuilder,
    };
    let show = MenuItem::with_id(app, "show", "Show Nice Downloader", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Nice Downloader")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                if let Some(native_player) = app.try_state::<native_player::NativePlayerManager>() {
                    native_player.close();
                }
                app.exit(0);
            }
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

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
            let tray_available = match install_tray(app) {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(%error, "system tray is unavailable; continuing with the main window");
                    false
                }
            };
            let data_directory = app.path().app_data_dir().map_err(|error| {
                format!("could not resolve the application data directory: {error}")
            })?;
            app.manage(DesktopRuntimeState { tray_available });
            std::fs::create_dir_all(&data_directory).map_err(|error| {
                format!("could not create the application data directory: {error}")
            })?;
            let resource_directory = app.path().resource_dir().map_err(|error| {
                format!("could not resolve application resource directory: {error}")
            })?;
            managed::initialize(&data_directory, &resource_directory)?;
            let database = Database::open(&data_directory.join("nice-downloader.sqlite3"))?;
            let playback_cache_directory = app
                .path()
                .app_cache_dir()
                .map(|path| path.join("library-playback"))
                .unwrap_or_else(|_| data_directory.join("library-playback-cache"));
            let media_server = media_server::start(database.clone(), playback_cache_directory)
                .unwrap_or_else(|error| {
                tracing::warn!(%error, "embedded playback is unavailable; system-player fallback remains enabled");
                media_server::MediaServerState::unavailable(database.clone())
                });
            let native_player = native_player::NativePlayerManager::new(database.clone());
            if tray_available && database.load_desktop_preferences()?.start_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            let queue = QueueManager::new(database.clone())?;
            let transformations = TransformationManager::new(database.clone())?;
            let visual_similarity = VisualSimilarityManager::new(database.clone());
            let local_share = LocalShareManager::new(database.clone());
            let creative = CreativeManager::new(
                database.clone(),
                data_directory.join("library-storyboards"),
            );
            let library = LibraryManager::new(database.clone())?;
            library.start_source_monitor(app.handle().clone());
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
            app.manage(transformations);
            app.manage(visual_similarity);
            app.manage(local_share);
            app.manage(creative);
            app.manage(library);
            app.manage(media_server);
            app.manage(native_player);
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
            commands::dismiss_downloads,
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
            commands::list_transformation_tasks,
            commands::start_transformation,
            commands::cancel_transformation,
            commands::list_library_items,
            commands::query_library_items,
            commands::get_library_facets,
            commands::get_library_items_by_ids,
            commands::search_library_transcripts,
            commands::get_library_transcript_index_status,
            commands::rebuild_library_transcript_index,
            commands::get_library_visual_similarity_status,
            commands::get_library_visual_similarity_job,
            commands::start_library_visual_similarity_index,
            commands::cancel_library_visual_similarity_index,
            commands::find_visually_similar_library_items,
            commands::get_local_share_status,
            commands::start_local_share,
            commands::stop_local_share,
            commands::prepare_library_image_view,
            commands::inspect_library_playback,
            commands::prepare_library_playback,
            commands::inspect_library_playback_fallback,
            commands::prepare_library_playback_fallback,
            commands::get_library_playback_preparation,
            commands::cancel_library_playback_preparation,
            commands::release_library_playback,
            commands::get_library_playback_cache_status,
            commands::clear_library_playback_cache,
            commands::get_native_playback_status,
            commands::open_library_native_playback,
            commands::close_library_native_playback,
            commands::save_library_playback_progress,
            commands::list_continue_watching,
            commands::get_library_watch_state,
            commands::set_library_watch_later,
            commands::set_library_watched,
            commands::list_library_watch_later,
            commands::list_recently_watched,
            commands::import_library_files,
            commands::update_library_item,
            commands::get_library_rich_metadata,
            commands::save_library_rich_metadata,
            commands::reset_library_rich_metadata,
            commands::set_library_artwork,
            commands::get_library_search_status,
            commands::rebuild_library_search_index,
            commands::list_library_smart_views,
            commands::save_library_smart_view,
            commands::delete_library_smart_view,
            commands::get_related_library_items,
            commands::list_library_collections,
            commands::save_library_collection,
            commands::delete_library_collection,
            commands::list_library_playlists,
            commands::save_library_playlist,
            commands::delete_library_playlist,
            commands::add_library_playlist_item,
            commands::remove_library_playlist_item,
            commands::move_library_playlist_item,
            commands::list_library_playlist_items,
            commands::list_library_moments,
            commands::save_library_moment,
            commands::delete_library_moment,
            commands::list_library_montages,
            commands::save_library_montage,
            commands::delete_library_montage,
            commands::start_library_montage_export,
            commands::list_library_compare_sessions,
            commands::save_library_compare_session,
            commands::delete_library_compare_session,
            commands::list_library_media_stacks,
            commands::save_library_media_stack,
            commands::delete_library_media_stack,
            commands::list_library_curation_queue,
            commands::get_library_curation_state,
            commands::set_library_curation_decision,
            commands::export_library_moments,
            commands::generate_library_storyboard,
            commands::start_library_clip_export,
            commands::apply_library_bulk_action,
            commands::list_library_changes,
            commands::review_library_changes,
            commands::get_library_duplicate_analysis,
            commands::list_library_duplicate_groups,
            commands::start_library_duplicate_analysis,
            commands::cancel_library_duplicate_analysis,
            commands::trash_library_duplicate_item,
            commands::preview_library_file_operation,
            commands::execute_library_file_operation,
            commands::preview_library_bulk_move,
            commands::execute_library_bulk_move,
            commands::undo_library_file_operation,
            commands::list_library_file_operations,
            commands::remove_library_item,
            commands::get_library_artwork,
            commands::clear_library_artwork_cache,
            commands::get_library_artwork_cache_status,
            commands::open_library_item,
            commands::reconcile_library,
            commands::export_library,
            commands::restore_library,
            commands::list_library_sources,
            commands::save_library_source,
            commands::remove_library_source,
            commands::list_library_scan_jobs,
            commands::start_library_scan,
            commands::cancel_library_scan,
            commands::list_authentication_profiles,
            commands::save_authentication_profile,
            commands::delete_authentication_profile,
            commands::discover_browser_profiles,
            commands::get_desktop_preferences,
            commands::get_desktop_runtime_status,
            commands::update_desktop_preferences,
            commands::hide_to_tray,
            commands::quit_app,
            commands::get_onboarding_status,
            commands::complete_onboarding,
        ])
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!(%error, "application runtime exited with an error");
    }
}
