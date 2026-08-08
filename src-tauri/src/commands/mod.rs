use crate::{
    database::Database,
    engines::{discovery, ytdlp::YtDlpEngine},
    errors::AppError,
    models::{DownloadJob, EngineInfo, MediaAnalysis, StartDownloadRequest},
    queue::QueueManager,
};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub async fn detect_engines() -> Result<Vec<EngineInfo>, AppError> {
    Ok(discovery::detect_all().await)
}

#[tauri::command]
pub async fn analyze_url(url: String) -> Result<MediaAnalysis, AppError> {
    YtDlpEngine::discover()?.analyze(&url).await
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
    request: StartDownloadRequest,
) -> Result<DownloadJob, AppError> {
    queue.enqueue(app, request).await
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
