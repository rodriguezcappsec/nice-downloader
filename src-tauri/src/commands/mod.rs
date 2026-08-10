use crate::{
    database::Database,
    engines::{discovery, ytdlp::YtDlpEngine},
    errors::AppError,
    models::{DownloadJob, EngineInfo, EnginePreferences, MediaAnalysis, StartDownloadRequest},
    queue::QueueManager,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn detect_engines() -> Result<Vec<EngineInfo>, AppError> {
    Ok(discovery::detect_all().await)
}

#[tauri::command]
pub async fn analyze_url(
    database: State<'_, Database>,
    url: String,
) -> Result<MediaAnalysis, AppError> {
    let preferences = database.load_engine_preferences()?;
    YtDlpEngine::discover()?.analyze(&url, &preferences).await
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
    preferences: EnginePreferences,
) -> Result<EnginePreferences, AppError> {
    database.save_engine_preferences(&preferences)?;
    Ok(preferences)
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
pub fn list_history(database: State<'_, Database>) -> Result<Vec<DownloadJob>, AppError> {
    database.list_history()
}
