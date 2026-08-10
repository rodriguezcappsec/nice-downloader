use crate::{
    database::Database,
    engines::{discovery, ytdlp::YtDlpEngine},
    errors::AppError,
    models::{
        BatchAnalysisItem, BatchAnalysisResponse, DownloadJob, EngineInfo, EnginePreferences,
        MediaAnalysis, StartDownloadRequest,
    },
    queue::QueueManager,
    utils::validation::{normalize_engine_preferences, validate_engine_preferences},
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
pub async fn analyze_url(
    database: State<'_, Database>,
    url: String,
) -> Result<MediaAnalysis, AppError> {
    let preferences = database.load_engine_preferences()?;
    YtDlpEngine::discover()?.analyze(&url, &preferences).await
}

#[tauri::command]
pub async fn analyze_urls(
    database: State<'_, Database>,
    urls: Vec<String>,
) -> Result<BatchAnalysisResponse, AppError> {
    let urls = normalize_batch_urls(urls)?;
    let preferences = database.load_engine_preferences()?;
    let engine = YtDlpEngine::discover()?;
    Ok(analyze_many(engine, preferences, urls, None).await)
}

#[tauri::command]
pub async fn analyze_playlist(
    database: State<'_, Database>,
    url: String,
) -> Result<BatchAnalysisResponse, AppError> {
    let preferences = database.load_engine_preferences()?;
    let engine = YtDlpEngine::discover()?;
    let (title, urls) = engine.expand_playlist(&url, &preferences).await?;
    Ok(analyze_many(engine, preferences, urls, title).await)
}

async fn analyze_many(
    engine: YtDlpEngine,
    preferences: EnginePreferences,
    urls: Vec<String>,
    collection_title: Option<String>,
) -> BatchAnalysisResponse {
    let semaphore = Arc::new(Semaphore::new(BATCH_ANALYSIS_CONCURRENCY));
    let mut workers = JoinSet::new();
    for (index, input_url) in urls.into_iter().enumerate() {
        let engine = engine.clone();
        let preferences = preferences.clone();
        let semaphore = semaphore.clone();
        workers.spawn(async move {
            let result = match semaphore.acquire_owned().await {
                Ok(_permit) => engine.analyze(&input_url, &preferences).await,
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
pub fn list_history(database: State<'_, Database>) -> Result<Vec<DownloadJob>, AppError> {
    database.list_history()
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
}
