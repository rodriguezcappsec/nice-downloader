use crate::DesktopRuntimeState;
use crate::{
    authentication,
    database::Database,
    engines::{
        discovery,
        gallery_dl::GalleryDlEngine,
        managed::{self, EngineUpdateCandidate, ManagedEngineStatus},
        router,
        ytdlp::YtDlpEngine,
    },
    errors::AppError,
    library,
    models::{
        AuthenticationProfile, BatchAnalysisItem, BatchAnalysisResponse, BatchDownloadFailure,
        BatchDownloadResult, BrowserProfileInfo, DesktopPreferences, DesktopRuntimeStatus,
        DiskSpaceInfo, DownloadArchiveMatch, DownloadJob, EngineId, EngineInfo, EnginePreferences,
        GalleryItemList, LibraryBulkAction, LibraryBulkMovePlan, LibraryBulkMoveResult,
        LibraryCacheResult, LibraryChange, LibraryCollection, LibraryDuplicateAnalysisJob,
        LibraryDuplicateGroup, LibraryDuplicateTrashResult, LibraryFileOperation,
        LibraryFileOperationPlan, LibraryFileOperationResult, LibraryItem, LibraryScanJob,
        LibraryScanResult, LibrarySource, MediaAnalysis, OnboardingStatus, Preset,
        PreviewLibraryBulkMoveRequest, PreviewLibraryFileOperationRequest, QueueSettings,
        QueueSnapshot, SaveAuthenticationProfileRequest, SaveLibraryCollectionRequest,
        SaveLibrarySourceRequest, SavePresetRequest, SaveSitePresetRuleRequest,
        SaveSubscriptionRequest, SitePresetRule, StartDownloadRequest, StartTransformationRequest,
        Subscription, SubscriptionCheckResult, TransformationTask, UpdateLibraryItemRequest,
    },
    queue::{disk_space, QueueManager},
    subscriptions,
    transformations::TransformationManager,
    utils::validation::{
        normalize_engine_preferences, validate_destination, validate_destination_writable,
        validate_engine_preferences, validate_media_url,
    },
};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, Manager, State};
use tokio::{sync::Semaphore, task::JoinSet};

const MAX_BATCH_URLS: usize = 20;
const BATCH_ANALYSIS_CONCURRENCY: usize = 3;

#[tauri::command]
pub fn list_transformation_tasks(
    transformations: State<'_, TransformationManager>,
) -> Result<Vec<TransformationTask>, AppError> {
    transformations.list()
}

#[tauri::command]
pub fn start_transformation(
    app: AppHandle,
    transformations: State<'_, TransformationManager>,
    request: StartTransformationRequest,
) -> Result<TransformationTask, AppError> {
    transformations.start(app, request)
}

#[tauri::command]
pub fn cancel_transformation(
    transformations: State<'_, TransformationManager>,
    id: String,
) -> Result<(), AppError> {
    transformations.cancel(&id)
}

#[tauri::command]
pub fn list_library_items(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
) -> Result<Vec<LibraryItem>, AppError> {
    library.list_items(app)
}

#[tauri::command]
pub fn import_library_files(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<Vec<LibraryItem>, AppError> {
    library.import_paths(app, paths, tags)
}

#[tauri::command]
pub fn update_library_item(
    database: State<'_, Database>,
    request: UpdateLibraryItemRequest,
) -> Result<LibraryItem, AppError> {
    database.update_library_item(request)
}

#[tauri::command]
pub fn list_library_collections(
    database: State<'_, Database>,
) -> Result<Vec<LibraryCollection>, AppError> {
    database.list_library_collections()
}

#[tauri::command]
pub fn save_library_collection(
    database: State<'_, Database>,
    request: SaveLibraryCollectionRequest,
) -> Result<LibraryCollection, AppError> {
    database.save_library_collection(request)
}

#[tauri::command]
pub fn delete_library_collection(
    database: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    database.delete_library_collection(&id)
}

#[tauri::command]
pub fn apply_library_bulk_action(
    database: State<'_, Database>,
    ids: Vec<String>,
    action: LibraryBulkAction,
) -> Result<Vec<LibraryItem>, AppError> {
    database.apply_library_bulk_action(ids, action)
}

#[tauri::command]
pub fn list_library_changes(database: State<'_, Database>) -> Result<Vec<LibraryChange>, AppError> {
    database.list_library_changes()
}

#[tauri::command]
pub fn review_library_changes(
    database: State<'_, Database>,
    ids: Vec<String>,
) -> Result<usize, AppError> {
    database.review_library_changes(ids)
}

#[tauri::command]
pub fn get_library_duplicate_analysis(
    library: State<'_, library::LibraryManager>,
) -> Result<Option<LibraryDuplicateAnalysisJob>, AppError> {
    library.duplicate_analysis_job()
}

#[tauri::command]
pub fn list_library_duplicate_groups(
    library: State<'_, library::LibraryManager>,
) -> Result<Vec<LibraryDuplicateGroup>, AppError> {
    library.duplicate_groups()
}

#[tauri::command]
pub fn start_library_duplicate_analysis(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
) -> Result<LibraryDuplicateAnalysisJob, AppError> {
    library.start_duplicate_analysis(app)
}

#[tauri::command]
pub fn cancel_library_duplicate_analysis(
    library: State<'_, library::LibraryManager>,
    job_id: String,
) -> Result<(), AppError> {
    library.cancel_duplicate_analysis(&job_id)
}

#[tauri::command]
pub async fn trash_library_duplicate_item(
    library: State<'_, library::LibraryManager>,
    group_id: String,
    item_id: String,
) -> Result<LibraryDuplicateTrashResult, AppError> {
    library.trash_duplicate_item(&group_id, &item_id).await
}

#[tauri::command]
pub fn preview_library_file_operation(
    library: State<'_, library::LibraryManager>,
    request: PreviewLibraryFileOperationRequest,
) -> Result<LibraryFileOperationPlan, AppError> {
    library.preview_file_operation(request)
}

#[tauri::command]
pub async fn execute_library_file_operation(
    library: State<'_, library::LibraryManager>,
    operation_id: String,
) -> Result<LibraryFileOperationResult, AppError> {
    library.execute_file_operation(&operation_id).await
}

#[tauri::command]
pub fn preview_library_bulk_move(
    library: State<'_, library::LibraryManager>,
    request: PreviewLibraryBulkMoveRequest,
) -> Result<LibraryBulkMovePlan, AppError> {
    library.preview_bulk_move(request)
}

#[tauri::command]
pub async fn execute_library_bulk_move(
    library: State<'_, library::LibraryManager>,
    operation_ids: Vec<String>,
) -> Result<LibraryBulkMoveResult, AppError> {
    library.execute_bulk_move(operation_ids).await
}

#[tauri::command]
pub async fn undo_library_file_operation(
    library: State<'_, library::LibraryManager>,
    operation_id: String,
) -> Result<LibraryFileOperationResult, AppError> {
    library.undo_file_operation(&operation_id).await
}

#[tauri::command]
pub fn list_library_file_operations(
    database: State<'_, Database>,
) -> Result<Vec<LibraryFileOperation>, AppError> {
    database.list_library_file_operations()
}

#[tauri::command]
pub fn remove_library_item(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    database.remove_library_item(&id)
}

#[tauri::command]
pub async fn get_library_artwork(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
    id: String,
) -> Result<Option<String>, AppError> {
    library.artwork_data_url(&app, &id).await
}

#[tauri::command]
pub async fn clear_library_artwork_cache(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
) -> Result<LibraryCacheResult, AppError> {
    library.clear_artwork_cache(&app).await
}

#[tauri::command]
pub fn open_library_item(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    let item = database.get_library_item(&id)?;
    let path = std::path::PathBuf::from(item.path);
    if !path.is_absolute() || !path.is_file() {
        return Err(AppError::new(
            "library_file_not_found",
            "This media file is no longer available at its saved location.",
        ));
    }
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    crate::utils::process::sanitize_std_command(&mut command);
    let mut child = command
        .arg(path)
        .spawn()
        .map_err(|error| AppError::internal("could not open the library item", error))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
pub fn reconcile_library(database: State<'_, Database>) -> Result<LibraryScanResult, AppError> {
    library::reconcile(&database)
}

#[tauri::command]
pub fn export_library(database: State<'_, Database>, path: String) -> Result<(), AppError> {
    library::export_backup(&database, &path)
}

#[tauri::command]
pub fn restore_library(
    database: State<'_, Database>,
    path: String,
) -> Result<Vec<LibraryItem>, AppError> {
    library::restore_backup(&database, &path)
}

#[tauri::command]
pub fn list_library_sources(
    library: State<'_, library::LibraryManager>,
) -> Result<Vec<LibrarySource>, AppError> {
    library.list_sources()
}

#[tauri::command]
pub fn save_library_source(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
    request: SaveLibrarySourceRequest,
) -> Result<LibrarySource, AppError> {
    library.save_source_and_sync_watcher(app, request)
}

#[tauri::command]
pub fn remove_library_source(
    library: State<'_, library::LibraryManager>,
    id: String,
) -> Result<(), AppError> {
    library.remove_source(&id)
}

#[tauri::command]
pub fn list_library_scan_jobs(
    library: State<'_, library::LibraryManager>,
) -> Result<Vec<LibraryScanJob>, AppError> {
    library.list_scan_jobs()
}

#[tauri::command]
pub fn start_library_scan(
    app: AppHandle,
    library: State<'_, library::LibraryManager>,
    source_id: String,
) -> Result<LibraryScanJob, AppError> {
    library.start_scan(app, &source_id)
}

#[tauri::command]
pub fn cancel_library_scan(
    library: State<'_, library::LibraryManager>,
    job_id: String,
) -> Result<(), AppError> {
    library.cancel_scan(&job_id)
}

#[tauri::command]
pub fn list_authentication_profiles(
    database: State<'_, Database>,
) -> Result<Vec<AuthenticationProfile>, AppError> {
    database.list_authentication_profiles()
}

#[tauri::command]
pub fn save_authentication_profile(
    app: AppHandle,
    database: State<'_, Database>,
    request: SaveAuthenticationProfileRequest,
) -> Result<AuthenticationProfile, AppError> {
    let data_directory = app.path().app_data_dir().map_err(|error| {
        AppError::internal("could not resolve secure application storage", error)
    })?;
    authentication::save_profile(&database, &data_directory, request)
}

#[tauri::command]
pub fn delete_authentication_profile(
    database: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    authentication::delete_profile(&database, &id)
}

#[tauri::command]
pub fn discover_browser_profiles() -> Vec<BrowserProfileInfo> {
    authentication::discover_browser_profiles()
}

#[tauri::command]
pub fn get_desktop_preferences(
    database: State<'_, Database>,
) -> Result<DesktopPreferences, AppError> {
    database.load_desktop_preferences()
}

#[tauri::command]
pub fn get_desktop_runtime_status(runtime: State<'_, DesktopRuntimeState>) -> DesktopRuntimeStatus {
    DesktopRuntimeStatus {
        tray_available: runtime.tray_available,
    }
}

#[tauri::command]
pub fn hide_to_tray(
    app: AppHandle,
    runtime: State<'_, DesktopRuntimeState>,
) -> Result<(), AppError> {
    if !runtime.tray_available {
        return Err(AppError::new(
            "tray_unavailable",
            "The system tray is unavailable. Choose Quit application in Desktop settings.",
        ));
    }
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::new("window_unavailable", "The main window is unavailable."))?;
    window
        .hide()
        .map_err(|error| AppError::internal("could not hide the main window", error))
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub async fn update_desktop_preferences(
    database: State<'_, Database>,
    queue: State<'_, QueueManager>,
    runtime: State<'_, DesktopRuntimeState>,
    mut preferences: DesktopPreferences,
) -> Result<DesktopPreferences, AppError> {
    if !matches!(preferences.interface_scale_percent, 90 | 100 | 110 | 125) {
        return Err(AppError::new(
            "invalid_interface_scale",
            "Choose a supported interface scale: 90%, 100%, 110%, or 125%.",
        ));
    }
    if !runtime.tray_available {
        preferences.start_minimized = false;
        if preferences.close_behavior == crate::models::CloseBehavior::Background {
            preferences.close_behavior = crate::models::CloseBehavior::Quit;
        }
    }
    database.save_desktop_preferences(&preferences)?;
    let mut queue_settings = queue.snapshot().await.settings;
    queue_settings.notify_on_completion = preferences.notify_when_backgrounded;
    queue.update_settings(queue_settings).await?;
    Ok(preferences)
}

#[tauri::command]
pub async fn get_onboarding_status(
    app: AppHandle,
    database: State<'_, Database>,
) -> Result<OnboardingStatus, AppError> {
    let default_destination = app
        .path()
        .download_dir()
        .map_err(|error| {
            AppError::internal("could not find the system Downloads directory", error)
        })?
        .to_string_lossy()
        .into_owned();
    Ok(OnboardingStatus {
        completed: database.onboarding_completed()?,
        default_destination,
        engines: discovery::detect_all().await,
    })
}

#[tauri::command]
pub fn complete_onboarding(database: State<'_, Database>) -> Result<(), AppError> {
    database.complete_onboarding()
}

#[tauri::command]
pub async fn detect_engines() -> Result<Vec<EngineInfo>, AppError> {
    Ok(discovery::detect_all().await)
}

#[tauri::command]
pub fn list_managed_engines() -> Result<Vec<ManagedEngineStatus>, AppError> {
    managed::list_statuses()
}

#[tauri::command]
pub async fn check_engine_update(engine: EngineId) -> Result<EngineUpdateCandidate, AppError> {
    managed::check_update(engine).await
}

#[tauri::command]
pub async fn install_engine_update(
    candidate: EngineUpdateCandidate,
) -> Result<ManagedEngineStatus, AppError> {
    managed::install(candidate).await
}

#[tauri::command]
pub fn pin_managed_engine(engine: EngineId, pinned: bool) -> Result<ManagedEngineStatus, AppError> {
    managed::set_pinned(engine, pinned)
}

#[tauri::command]
pub fn rollback_managed_engine(engine: EngineId) -> Result<ManagedEngineStatus, AppError> {
    managed::rollback(engine)
}

#[tauri::command]
pub async fn test_site_access(
    database: State<'_, Database>,
    url: String,
) -> Result<MediaAnalysis, AppError> {
    let mut preferences = database.load_engine_preferences()?;
    authentication::apply_profile(&database, &url, &mut preferences)?;
    router::analyze(&url, &preferences).await
}

#[tauri::command]
pub async fn analyze_url(
    database: State<'_, Database>,
    url: String,
) -> Result<MediaAnalysis, AppError> {
    let mut preferences = database.load_engine_preferences()?;
    authentication::apply_profile(&database, &url, &mut preferences)?;
    router::analyze(&url, &preferences).await
}

#[tauri::command]
pub async fn list_gallery_items(
    database: State<'_, Database>,
    url: String,
    offset: u32,
    limit: u32,
) -> Result<GalleryItemList, AppError> {
    let mut preferences = database.load_engine_preferences()?;
    authentication::apply_profile(&database, &url, &mut preferences)?;
    GalleryDlEngine::discover()?
        .list_items(&url, &preferences, offset, limit)
        .await
}

#[tauri::command]
pub async fn analyze_urls(
    database: State<'_, Database>,
    urls: Vec<String>,
) -> Result<BatchAnalysisResponse, AppError> {
    let urls = normalize_batch_urls(urls)?;
    let preferences = database.load_engine_preferences()?;
    Ok(analyze_many(preferences, urls, None).await)
}

#[tauri::command]
pub async fn analyze_playlist(
    database: State<'_, Database>,
    url: String,
) -> Result<BatchAnalysisResponse, AppError> {
    let mut preferences = database.load_engine_preferences()?;
    authentication::apply_profile(&database, &url, &mut preferences)?;
    let engine = YtDlpEngine::discover()?;
    let (title, urls) = engine.expand_playlist(&url, &preferences).await?;
    Ok(analyze_many(preferences, urls, title).await)
}

async fn analyze_many(
    preferences: EnginePreferences,
    urls: Vec<String>,
    collection_title: Option<String>,
) -> BatchAnalysisResponse {
    let semaphore = Arc::new(Semaphore::new(BATCH_ANALYSIS_CONCURRENCY));
    let mut workers = JoinSet::new();
    for (index, input_url) in urls.into_iter().enumerate() {
        let preferences = preferences.clone();
        let semaphore = semaphore.clone();
        workers.spawn(async move {
            let result = match semaphore.acquire_owned().await {
                Ok(_permit) => router::analyze(&input_url, &preferences).await,
                Err(_) => Err(AppError::new(
                    "batch_cancelled",
                    "Batch analysis was cancelled before it could start.",
                )),
            };
            let item = match result {
                Ok(analysis) => BatchAnalysisItem {
                    input_url,
                    analysis: Some(analysis),
                    error: None,
                },
                Err(error) => BatchAnalysisItem {
                    input_url,
                    analysis: None,
                    error: Some(error),
                },
            };
            (index, item)
        });
    }
    let mut items = Vec::new();
    while let Some(result) = workers.join_next().await {
        match result {
            Ok(item) => items.push(item),
            Err(error) => tracing::warn!(%error, "batch analysis worker stopped unexpectedly"),
        }
    }
    items.sort_by_key(|(index, _)| *index);
    BatchAnalysisResponse {
        collection_title,
        items: items.into_iter().map(|(_, item)| item).collect(),
    }
}

fn normalize_batch_urls(urls: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut seen = HashSet::new();
    let urls = urls
        .into_iter()
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty() && seen.insert(url.clone()))
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(AppError::new(
            "empty_batch",
            "Paste at least one media URL.",
        ));
    }
    if urls.len() > MAX_BATCH_URLS {
        return Err(AppError::new(
            "batch_too_large",
            format!("Analyze up to {MAX_BATCH_URLS} URLs at a time."),
        ));
    }
    Ok(urls)
}

#[tauri::command]
pub fn get_engine_preferences(
    database: State<'_, Database>,
) -> Result<EnginePreferences, AppError> {
    database.load_engine_preferences()
}

#[tauri::command]
pub fn update_engine_preferences(
    database: State<'_, Database>,
    mut preferences: EnginePreferences,
) -> Result<EnginePreferences, AppError> {
    normalize_engine_preferences(&mut preferences);
    validate_engine_preferences(&preferences)?;
    database.save_engine_preferences(&preferences)?;
    Ok(preferences)
}

#[tauri::command]
pub async fn retry_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    database: State<'_, Database>,
    job_id: String,
) -> Result<DownloadJob, AppError> {
    let preferences = database.load_engine_preferences()?;
    queue.retry(app, &job_id, preferences).await
}

#[tauri::command]
pub async fn repeat_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    database: State<'_, Database>,
    job_id: String,
) -> Result<DownloadJob, AppError> {
    let preferences = database.load_engine_preferences()?;
    queue.repeat(app, &job_id, preferences).await
}

#[tauri::command]
pub fn get_default_destination(app: AppHandle) -> Result<String, AppError> {
    let path = app.path().download_dir().map_err(|error| {
        AppError::internal("could not find the system Downloads directory", error)
    })?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn start_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    database: State<'_, Database>,
    request: StartDownloadRequest,
) -> Result<DownloadJob, AppError> {
    let mut preferences = database.load_engine_preferences()?;
    authentication::apply_profile(&database, &request.url, &mut preferences)?;
    queue.enqueue(app, request, preferences).await
}

#[tauri::command]
pub async fn start_downloads(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    database: State<'_, Database>,
    requests: Vec<StartDownloadRequest>,
) -> Result<BatchDownloadResult, AppError> {
    if requests.is_empty() || requests.len() > MAX_BATCH_URLS {
        return Err(AppError::new(
            "invalid_batch_download",
            format!("Queue between 1 and {MAX_BATCH_URLS} downloads at a time."),
        ));
    }
    let base_preferences = database.load_engine_preferences()?;
    let mut jobs = Vec::with_capacity(requests.len());
    let mut failures = Vec::new();
    for request in requests {
        let url = request.url.clone();
        let mut preferences = base_preferences.clone();
        if let Err(error) = authentication::apply_profile(&database, &request.url, &mut preferences)
        {
            failures.push(BatchDownloadFailure { url, error });
            continue;
        }
        match queue.enqueue(app.clone(), request, preferences).await {
            Ok(job) => jobs.push(job),
            Err(error) => failures.push(BatchDownloadFailure { url, error }),
        }
    }
    Ok(BatchDownloadResult { jobs, failures })
}

#[tauri::command]
pub async fn cancel_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    job_id: String,
) -> Result<(), AppError> {
    queue.cancel(&app, &job_id).await
}

#[tauri::command]
pub async fn dismiss_downloads(
    queue: State<'_, QueueManager>,
    job_ids: Vec<String>,
) -> Result<Vec<String>, AppError> {
    queue.dismiss(job_ids).await
}

#[tauri::command]
pub async fn list_jobs(queue: State<'_, QueueManager>) -> Result<Vec<DownloadJob>, AppError> {
    Ok(queue.list().await)
}

#[tauri::command]
pub async fn get_queue_snapshot(queue: State<'_, QueueManager>) -> Result<QueueSnapshot, AppError> {
    Ok(queue.snapshot().await)
}

#[tauri::command]
pub async fn update_queue_settings(
    queue: State<'_, QueueManager>,
    settings: QueueSettings,
) -> Result<QueueSnapshot, AppError> {
    queue.update_settings(settings).await
}

#[tauri::command]
pub async fn move_queued_download(
    queue: State<'_, QueueManager>,
    job_id: String,
    new_index: usize,
) -> Result<QueueSnapshot, AppError> {
    queue.move_pending(&job_id, new_index).await
}

#[tauri::command]
pub async fn pause_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    job_id: String,
) -> Result<(), AppError> {
    queue.pause(&app, &job_id).await
}

#[tauri::command]
pub async fn resume_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    job_id: String,
) -> Result<(), AppError> {
    queue.resume(&app, &job_id).await
}

#[tauri::command]
pub fn check_disk_space(
    destination: String,
    required_bytes: Option<u64>,
) -> Result<DiskSpaceInfo, AppError> {
    let destination = validate_destination(&destination)?;
    disk_space(&destination, required_bytes)
}

#[tauri::command]
pub fn list_history(database: State<'_, Database>) -> Result<Vec<DownloadJob>, AppError> {
    database.list_history()
}

#[tauri::command]
pub async fn delete_history_entry(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    id: String,
) -> Result<(), AppError> {
    queue.delete_history_entry(&app, &id).await
}

#[tauri::command]
pub fn list_presets(database: State<'_, Database>) -> Result<Vec<Preset>, AppError> {
    database.list_presets()
}

#[tauri::command]
pub fn save_preset(
    database: State<'_, Database>,
    request: SavePresetRequest,
) -> Result<Preset, AppError> {
    database.save_preset(request)
}

#[tauri::command]
pub fn delete_preset(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    database.delete_preset(&id)
}

#[tauri::command]
pub fn list_site_preset_rules(
    database: State<'_, Database>,
) -> Result<Vec<SitePresetRule>, AppError> {
    database.list_site_preset_rules()
}

#[tauri::command]
pub fn save_site_preset_rule(
    database: State<'_, Database>,
    request: SaveSitePresetRuleRequest,
) -> Result<SitePresetRule, AppError> {
    database.save_site_preset_rule(request)
}

#[tauri::command]
pub fn delete_site_preset_rule(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    database.delete_site_preset_rule(&id)
}

#[tauri::command]
pub fn match_site_preset_rule(
    database: State<'_, Database>,
    url: String,
) -> Result<Option<SitePresetRule>, AppError> {
    validate_media_url(&url)?;
    database.match_site_preset_rule(&url)
}

#[tauri::command]
pub fn find_download_archive_match(
    database: State<'_, Database>,
    url: String,
    destination: String,
) -> Result<Option<DownloadArchiveMatch>, AppError> {
    validate_media_url(&url)?;
    let destination = validate_destination(&destination)?;
    database.archive_match(&url, &destination.to_string_lossy())
}

#[tauri::command]
pub fn find_download_archive_matches(
    database: State<'_, Database>,
    urls: Vec<String>,
    destination: String,
) -> Result<Vec<String>, AppError> {
    if urls.len() > MAX_BATCH_URLS {
        return Err(AppError::new(
            "archive_batch_too_large",
            format!("Check at most {MAX_BATCH_URLS} URLs at a time."),
        ));
    }
    let destination = validate_destination(&destination)?;
    let destination = destination.to_string_lossy();
    let mut matches = Vec::new();
    for url in urls {
        let url = validate_media_url(&url)?.to_string();
        if database.archive_match(&url, &destination)?.is_some() {
            matches.push(url);
        }
    }
    Ok(matches)
}

#[tauri::command]
pub fn prepare_collection_destination(
    destination: String,
    collection_title: String,
) -> Result<String, AppError> {
    let destination = validate_destination(&destination)?;
    let directory_name = safe_collection_directory(&collection_title);
    let collection = destination.join(directory_name);
    std::fs::create_dir_all(&collection)
        .map_err(|error| AppError::internal("could not create collection folder", error))?;
    validate_destination_writable(&collection)?;
    Ok(collection.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn list_subscriptions(database: State<'_, Database>) -> Result<Vec<Subscription>, AppError> {
    database.list_subscriptions()
}

#[tauri::command]
pub fn save_subscription(
    database: State<'_, Database>,
    request: SaveSubscriptionRequest,
) -> Result<Subscription, AppError> {
    database.save_subscription(request)
}

#[tauri::command]
pub fn delete_subscription(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    database.delete_subscription(&id)
}

#[tauri::command]
pub async fn check_subscription_now(
    app: AppHandle,
    database: State<'_, Database>,
    queue: State<'_, QueueManager>,
    id: String,
) -> Result<SubscriptionCheckResult, AppError> {
    subscriptions::check_subscription(app, &database, &queue, &id).await
}

fn safe_collection_directory(value: &str) -> String {
    let clean = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, ' ' | '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .take(100)
        .collect::<String>();
    let clean = clean.trim().trim_matches('.');
    if clean.is_empty() || !clean.chars().any(char::is_alphanumeric) {
        "Collection".to_owned()
    } else {
        clean.to_owned()
    }
}

#[tauri::command]
pub fn open_download_output(
    database: State<'_, Database>,
    job_id: String,
    reveal: bool,
) -> Result<(), AppError> {
    let path = database.download_output_path(&job_id)?;
    if !path.is_absolute() {
        return Err(AppError::new(
            "invalid_output_path",
            "The saved output path is invalid.",
        ));
    }
    let target = if reveal && path.is_file() {
        #[cfg(target_os = "windows")]
        {
            path.as_path()
        }
        #[cfg(not(target_os = "windows"))]
        {
            path.parent().unwrap_or(&path)
        }
    } else {
        path.as_path()
    };
    if !target.exists() {
        return Err(AppError::new(
            "output_not_found",
            "The downloaded file or folder no longer exists at its saved location.",
        ));
    }
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer.exe");
        if reveal && path.is_file() {
            command.arg("/select,");
        }
        command
    };
    crate::utils::process::sanitize_std_command(&mut command);
    let mut child = command
        .arg(target)
        .spawn()
        .map_err(|error| AppError::internal("could not open the downloaded output", error))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
pub async fn export_diagnostics(
    app: AppHandle,
    database: State<'_, Database>,
    path: String,
) -> Result<(), AppError> {
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute()
        || path.extension().and_then(|value| value.to_str()) != Some("json")
        || !path.parent().is_some_and(std::path::Path::is_dir)
    {
        return Err(AppError::new(
            "invalid_diagnostic_path",
            "Choose an existing folder and a filename ending in .json.",
        ));
    }
    let preferences = database.load_engine_preferences()?;
    let jobs = database.list_recent_jobs()?;
    let engines = discovery::detect_all().await;
    let managed = managed::list_statuses()?;
    let jobs = jobs
        .into_iter()
        .map(|job| {
            let site = url::Url::parse(&job.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned));
            serde_json::json!({
                "id": job.id,
                "site": site,
                "state": job.state,
                "engineOutputFormat": job.output_format,
                "operation": job.current_operation,
                "error": job.error.map(|value| sanitize_export_text(&value)),
                "diagnostics": job.diagnostics.into_iter().map(|value| sanitize_export_text(&value)).collect::<Vec<_>>(),
                "attemptCount": job.attempt_count,
                "updatedAt": job.updated_at,
            })
        })
        .collect::<Vec<_>>();
    let document = serde_json::json!({
        "schemaVersion": 1,
        "generatedAt": chrono::Utc::now(),
        "application": { "name": app.package_info().name, "version": app.package_info().version.to_string() },
        "platform": { "os": std::env::consts::OS, "arch": std::env::consts::ARCH },
        "access": {
            "browserCookieSource": preferences.browser_cookie_source,
            "browserProfileConfigured": preferences.browser_profile.is_some(),
            "proxyConfigured": preferences.proxy_url.is_some(),
            "impersonationEnabled": preferences.enable_impersonation,
        },
        "engines": engines,
        "managedEngines": managed,
        "jobs": jobs,
        "redactions": ["URL paths and queries", "proxy value", "browser profile", "cookies", "request headers"],
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| AppError::internal("could not serialize diagnostics", error))?;
    let parent = path.parent().ok_or_else(|| {
        AppError::new(
            "invalid_diagnostic_path",
            "The diagnostic destination is invalid.",
        )
    })?;
    let temporary = parent.join(format!(".nice-downloader-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| AppError::internal("could not write the diagnostic bundle", error))?;
    std::fs::rename(&temporary, &path)
        .map_err(|error| AppError::internal("could not finalize the diagnostic bundle", error))?;
    Ok(())
}

fn sanitize_export_text(value: &str) -> String {
    crate::engines::ytdlp::sanitize_diagnostic(value)
        .split_whitespace()
        .map(|token| {
            if token.contains("http://") || token.contains("https://") {
                "[url redacted]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_unique_batch_urls_without_reordering() {
        assert_eq!(
            normalize_batch_urls(vec![
                " https://example.com/one ".to_owned(),
                "https://example.com/two".to_owned(),
                "https://example.com/one".to_owned(),
            ])
            .unwrap(),
            ["https://example.com/one", "https://example.com/two"]
        );
    }

    #[test]
    fn rejects_oversized_batches() {
        let urls = (0..=MAX_BATCH_URLS)
            .map(|index| format!("https://example.com/{index}"))
            .collect();
        assert!(normalize_batch_urls(urls).is_err());
    }

    #[test]
    fn collection_directories_remove_path_syntax() {
        assert_eq!(
            safe_collection_directory(" My/Collection: 2026? "),
            "My_Collection_ 2026_"
        );
        assert_eq!(safe_collection_directory("../../"), "Collection");
    }

    #[test]
    fn collection_directories_are_bounded() {
        let directory = safe_collection_directory(&"a".repeat(160));
        assert_eq!(directory.chars().count(), 100);
    }

    #[test]
    fn diagnostic_exports_remove_urls_and_sensitive_queries() {
        let sanitized = sanitize_export_text(
            "failed https://cdn.example/file.mp4?token=secret Authorization: bearer",
        );
        assert!(!sanitized.contains("cdn.example"));
        assert!(!sanitized.contains("secret"));
        assert!(sanitized.contains("[url redacted]"));
    }
}
