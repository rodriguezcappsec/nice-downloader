use crate::{
    database::Database,
    engines::{
        discovery,
        gallery_dl::GalleryDlEngine,
        managed::{self, EngineUpdateCandidate, ManagedEngineStatus},
        router,
        ytdlp::YtDlpEngine,
    },
    errors::AppError,
    models::{
        BatchAnalysisItem, BatchAnalysisResponse, DiskSpaceInfo, DownloadJob, EngineId, EngineInfo,
        EnginePreferences, GalleryItemList, MediaAnalysis, Preset, QueueSettings, QueueSnapshot,
        SavePresetRequest, StartDownloadRequest,
    },
    queue::{disk_space, QueueManager},
    utils::validation::{
        normalize_engine_preferences, validate_destination, validate_engine_preferences,
    },
};
use std::{collections::HashSet, sync::Arc};
use tauri::{AppHandle, Manager, State};
use tokio::{sync::Semaphore, task::JoinSet};

const MAX_BATCH_URLS: usize = 20;
const BATCH_ANALYSIS_CONCURRENCY: usize = 3;

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
    let preferences = database.load_engine_preferences()?;
    router::analyze(&url, &preferences).await
}

#[tauri::command]
pub async fn analyze_url(
    database: State<'_, Database>,
    url: String,
) -> Result<MediaAnalysis, AppError> {
    let preferences = database.load_engine_preferences()?;
    router::analyze(&url, &preferences).await
}

#[tauri::command]
pub async fn list_gallery_items(
    database: State<'_, Database>,
    url: String,
    offset: u32,
    limit: u32,
) -> Result<GalleryItemList, AppError> {
    let preferences = database.load_engine_preferences()?;
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
    let preferences = database.load_engine_preferences()?;
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
    let preferences = database.load_engine_preferences()?;
    queue.enqueue(app, request, preferences).await
}

#[tauri::command]
pub async fn start_downloads(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    database: State<'_, Database>,
    requests: Vec<StartDownloadRequest>,
) -> Result<Vec<DownloadJob>, AppError> {
    if requests.is_empty() || requests.len() > MAX_BATCH_URLS {
        return Err(AppError::new(
            "invalid_batch_download",
            format!("Queue between 1 and {MAX_BATCH_URLS} downloads at a time."),
        ));
    }
    let preferences = database.load_engine_preferences()?;
    let mut jobs = Vec::with_capacity(requests.len());
    for request in requests {
        jobs.push(
            queue
                .enqueue(app.clone(), request, preferences.clone())
                .await?,
        );
    }
    Ok(jobs)
}

#[tauri::command]
pub async fn cancel_download(
    queue: State<'_, QueueManager>,
    job_id: String,
) -> Result<(), AppError> {
    queue.cancel(&job_id).await
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
pub fn delete_history_entry(database: State<'_, Database>, id: String) -> Result<(), AppError> {
    database.delete_history_entry(&id)
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
        path.parent().unwrap_or(&path)
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
    fn diagnostic_exports_remove_urls_and_sensitive_queries() {
        let sanitized = sanitize_export_text(
            "failed https://cdn.example/file.mp4?token=secret Authorization: bearer",
        );
        assert!(!sanitized.contains("cdn.example"));
        assert!(!sanitized.contains("secret"));
        assert!(sanitized.contains("[url redacted]"));
    }
}
