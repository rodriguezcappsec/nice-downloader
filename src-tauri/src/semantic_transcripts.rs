use crate::{
    database::{Database, SemanticTranscriptDocument},
    engines::managed,
    errors::AppError,
    models::{
        LibraryLocalInferenceJob, LibraryLocalInferenceJobState, LibraryMediaKind,
        LibrarySemanticIndexStatus, LibrarySemanticTranscriptMatch,
        LibraryTranscriptionProviderStatus, StartLibrarySemanticIndexRequest,
        StartLibraryTranscriptionRequest,
    },
    transcripts,
    utils::process::sanitize_tokio_command,
};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Command,
    time::{timeout, Duration},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const LIBRARY_LOCAL_INFERENCE_EVENT: &str = "library-local-inference-updated";
const OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
#[cfg(test)]
const DEFAULT_SEMANTIC_MODEL: &str = "nomic-embed-text";

#[derive(Clone)]
pub struct SemanticTranscriptManager {
    database: Database,
    active: Arc<Mutex<Option<(String, CancellationToken)>>>,
    job: Arc<Mutex<Option<LibraryLocalInferenceJob>>>,
}

impl SemanticTranscriptManager {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            active: Arc::new(Mutex::new(None)),
            job: Arc::new(Mutex::new(None)),
        }
    }

    pub fn job(&self) -> Result<Option<LibraryLocalInferenceJob>, AppError> {
        self.job
            .lock()
            .map(|job| job.clone())
            .map_err(|_| unavailable())
    }

    pub fn transcription_provider_status(&self) -> LibraryTranscriptionProviderStatus {
        match whisper_executable() {
            Some(path) => LibraryTranscriptionProviderStatus {
                available: true,
                executable: path.file_name().map(|name| name.to_string_lossy().into_owned()),
                detail: "A local whisper.cpp command is ready. Audio and transcripts stay on this device."
                    .to_owned(),
            },
            None => LibraryTranscriptionProviderStatus {
                available: false,
                executable: None,
                detail: "Install whisper-cli (or whisper) from whisper.cpp to transcribe locally."
                    .to_owned(),
            },
        }
    }

    pub fn semantic_index_status(
        &self,
        model: &str,
    ) -> Result<LibrarySemanticIndexStatus, AppError> {
        let model = validate_model(model)?;
        self.database.semantic_transcript_index_status(&model)
    }

    pub async fn start_transcription(
        &self,
        app: AppHandle,
        request: StartLibraryTranscriptionRequest,
    ) -> Result<LibraryLocalInferenceJob, AppError> {
        if request.item_ids.is_empty() || request.item_ids.len() > 100 {
            return Err(AppError::new(
                "invalid_transcription_selection",
                "Choose between 1 and 100 Library videos to transcribe.",
            ));
        }
        let model_path = validate_model_path(&request.model_path)?;
        let language = validate_language(request.language.as_deref())?;
        let whisper = whisper_executable().ok_or_else(|| {
            AppError::new(
                "whisper_missing",
                "Install whisper-cli (or whisper) from whisper.cpp before transcribing.",
            )
        })?;
        let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
            AppError::new(
                "ffmpeg_missing",
                "Install FFmpeg before transcribing videos.",
            )
        })?;
        let mut seen = HashSet::new();
        if request.item_ids.iter().any(|id| !seen.insert(id.clone())) {
            return Err(AppError::new(
                "duplicate_transcription_item",
                "The transcription selection contains the same video more than once.",
            ));
        }
        let items = self
            .database
            .list_library_items_by_ids(request.item_ids.clone())?;
        if items.len() != request.item_ids.len() {
            return Err(AppError::new(
                "transcription_item_unavailable",
                "One or more selected videos are missing, private while locked, or no longer in the Library.",
            ));
        }
        let by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut targets = Vec::with_capacity(request.item_ids.len());
        for item_id in request.item_ids {
            let item = by_id.get(&item_id).ok_or_else(|| {
                AppError::new(
                    "transcription_item_unavailable",
                    "A selected video is unavailable.",
                )
            })?;
            if item.missing
                || !matches!(
                    item.media_kind,
                    Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
                )
            {
                return Err(AppError::new(
                    "invalid_transcription_item",
                    "Local transcription accepts available Library videos only.",
                ));
            }
            validate_media_path(Path::new(&item.path))?;
            targets.push((
                item.id.clone(),
                item.title.clone(),
                PathBuf::from(&item.path),
            ));
        }

        let mut active = self.active.lock().map_err(|_| unavailable())?;
        if active.is_some() {
            return Err(AppError::new(
                "local_inference_busy",
                "Another local transcript job is already running.",
            ));
        }
        let now = Utc::now();
        let mut job = LibraryLocalInferenceJob {
            id: Uuid::new_v4().to_string(),
            kind: "transcription".to_owned(),
            state: LibraryLocalInferenceJobState::Running,
            checked_items: 0,
            total_items: targets.len(),
            completed_items: 0,
            failed_items: 0,
            current_title: None,
            model: model_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "whisper.cpp model".to_owned()),
            error: None,
            started_at: now,
            updated_at: now,
        };
        let token = CancellationToken::new();
        *active = Some((job.id.clone(), token.clone()));
        drop(active);
        self.store_and_emit(&app, &job)?;
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            for (item_id, title, media_path) in targets {
                if token.is_cancelled() {
                    break;
                }
                job.current_title = Some(title);
                job.updated_at = Utc::now();
                let _ = manager.store_and_emit(&app, &job);
                match transcribe_video(
                    &ffmpeg,
                    &whisper,
                    &model_path,
                    language.as_deref(),
                    &media_path,
                    &token,
                )
                .await
                {
                    Ok(cues) => {
                        if manager
                            .database
                            .replace_library_local_transcript_cues(&item_id, &cues)
                            .is_ok()
                        {
                            job.completed_items += 1;
                        } else {
                            job.failed_items += 1;
                        }
                    }
                    Err(error) if error.code == "local_inference_cancelled" => break,
                    Err(_) => job.failed_items += 1,
                }
                job.checked_items += 1;
                job.updated_at = Utc::now();
                let _ = manager.store_and_emit(&app, &job);
            }
            manager.finish(&app, &mut job, token.is_cancelled(), None);
        });
        self.job()?.ok_or_else(unavailable)
    }

    pub async fn start_semantic_index(
        &self,
        app: AppHandle,
        request: StartLibrarySemanticIndexRequest,
    ) -> Result<LibraryLocalInferenceJob, AppError> {
        if !(1..=500).contains(&request.max_videos) || request.item_ids.len() > 500 {
            return Err(AppError::new(
                "invalid_semantic_transcript_batch",
                "Choose between 1 and 500 videos for a semantic-index run.",
            ));
        }
        let model = validate_model(&request.model)?;
        let client = local_client(Duration::from_secs(30))?;
        ensure_ollama_model(&client, &model).await?;
        let documents = self.database.list_semantic_transcript_documents(
            &request.item_ids,
            usize::try_from(request.max_videos).unwrap_or(500),
        )?;
        let mut pending = Vec::new();
        for document in documents {
            if !self
                .database
                .semantic_transcript_embedding_is_current(&document, &model)?
            {
                pending.push(document);
            }
        }
        let mut active = self.active.lock().map_err(|_| unavailable())?;
        if active.is_some() {
            return Err(AppError::new(
                "local_inference_busy",
                "Another local transcript job is already running.",
            ));
        }
        let now = Utc::now();
        let mut job = LibraryLocalInferenceJob {
            id: Uuid::new_v4().to_string(),
            kind: "semanticIndex".to_owned(),
            state: if pending.is_empty() {
                LibraryLocalInferenceJobState::Completed
            } else {
                LibraryLocalInferenceJobState::Running
            },
            checked_items: 0,
            total_items: pending.len(),
            completed_items: 0,
            failed_items: 0,
            current_title: None,
            model: model.clone(),
            error: None,
            started_at: now,
            updated_at: now,
        };
        if pending.is_empty() {
            self.store_and_emit(&app, &job)?;
            return Ok(job);
        }
        let token = CancellationToken::new();
        *active = Some((job.id.clone(), token.clone()));
        drop(active);
        self.store_and_emit(&app, &job)?;
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let client = match local_client(Duration::from_secs(600)) {
                Ok(client) => client,
                Err(error) => {
                    manager.finish(&app, &mut job, false, Some(error.message));
                    return;
                }
            };
            for document in pending {
                if token.is_cancelled() {
                    break;
                }
                job.current_title = Some(document.title.clone());
                job.updated_at = Utc::now();
                let _ = manager.store_and_emit(&app, &job);
                match embed_document(&client, &model, &document, &token).await {
                    Ok(vector) => match serde_json::to_string(&vector) {
                        Ok(vector_json) => {
                            if manager
                                .database
                                .save_semantic_transcript_embedding(&document, &model, &vector_json)
                                .is_ok()
                            {
                                job.completed_items += 1;
                            } else {
                                job.failed_items += 1;
                            }
                        }
                        Err(_) => job.failed_items += 1,
                    },
                    Err(error) if error.code == "local_inference_cancelled" => break,
                    Err(error) if error.code == "ollama_model_failed" => {
                        manager.finish(&app, &mut job, false, Some(error.message));
                        return;
                    }
                    Err(_) => job.failed_items += 1,
                }
                job.checked_items += 1;
                job.updated_at = Utc::now();
                let _ = manager.store_and_emit(&app, &job);
            }
            manager.finish(&app, &mut job, token.is_cancelled(), None);
        });
        self.job()?.ok_or_else(unavailable)
    }

    pub async fn search(
        &self,
        query: &str,
        model: &str,
        limit: usize,
    ) -> Result<Vec<LibrarySemanticTranscriptMatch>, AppError> {
        let query = query.trim();
        if !(2..=500).contains(&query.chars().count()) || query.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_semantic_transcript_query",
                "Semantic searches contain between 2 and 500 visible characters.",
            ));
        }
        if !(1..=50).contains(&limit) {
            return Err(AppError::new(
                "invalid_semantic_transcript_limit",
                "Semantic search returns between 1 and 50 matches.",
            ));
        }
        let model = validate_model(model)?;
        let client = local_client(Duration::from_secs(120))?;
        let query_vector = embed_text(&client, &model, query, &CancellationToken::new()).await?;
        let stored = self
            .database
            .list_semantic_transcript_embeddings(&model, 50_000)?;
        let mut scored = stored
            .into_iter()
            .filter_map(|embedding| {
                let vector = serde_json::from_str::<Vec<f32>>(&embedding.vector_json).ok()?;
                let score = cosine_similarity(&query_vector, &vector)?;
                Some((embedding.item_id, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.1.total_cmp(&left.1).then(left.0.cmp(&right.0)));
        scored.truncate(limit);
        let items = self.database.list_library_items_by_ids(
            scored.iter().map(|(item_id, _)| item_id.clone()).collect(),
        )?;
        let item_by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut matches = Vec::new();
        for (item_id, score) in scored {
            let Some(item) = item_by_id.get(&item_id).cloned() else {
                continue;
            };
            let Some(excerpt) = self.database.library_transcript_excerpt(&item_id, query)? else {
                continue;
            };
            matches.push(LibrarySemanticTranscriptMatch {
                item,
                score_percent: (score * 100.0).clamp(0.0, 100.0),
                start_seconds: excerpt.start_seconds,
                end_seconds: excerpt.end_seconds,
                excerpt: excerpt.text,
                language: excerpt.language,
            });
        }
        Ok(matches)
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), AppError> {
        let active = self.active.lock().map_err(|_| unavailable())?;
        let Some((active_id, token)) = active.as_ref() else {
            return Err(AppError::new(
                "local_inference_not_running",
                "No local transcript job is running.",
            ));
        };
        if active_id != job_id {
            return Err(AppError::new(
                "local_inference_job_mismatch",
                "That local transcript job is no longer active.",
            ));
        }
        token.cancel();
        Ok(())
    }

    fn store_and_emit(
        &self,
        app: &AppHandle,
        job: &LibraryLocalInferenceJob,
    ) -> Result<(), AppError> {
        *self.job.lock().map_err(|_| unavailable())? = Some(job.clone());
        let _ = app.emit(LIBRARY_LOCAL_INFERENCE_EVENT, job);
        Ok(())
    }

    fn finish(
        &self,
        app: &AppHandle,
        job: &mut LibraryLocalInferenceJob,
        cancelled: bool,
        error: Option<String>,
    ) {
        job.state = if cancelled {
            LibraryLocalInferenceJobState::Cancelled
        } else if error.is_some() {
            LibraryLocalInferenceJobState::Failed
        } else {
            LibraryLocalInferenceJobState::Completed
        };
        job.error = error;
        job.current_title = None;
        job.updated_at = Utc::now();
        let _ = self.store_and_emit(app, job);
        if let Ok(mut active) = self.active.lock() {
            active.take();
        }
    }
}

async fn transcribe_video(
    ffmpeg: &Path,
    whisper: &Path,
    model_path: &Path,
    language: Option<&str>,
    media_path: &Path,
    token: &CancellationToken,
) -> Result<Vec<crate::models::LibraryTranscriptCue>, AppError> {
    let directory = tempfile::tempdir()
        .map_err(|error| AppError::internal("could not create transcription workspace", error))?;
    let audio_path = directory.path().join("audio.wav");
    let output_prefix = directory.path().join("transcript");
    let output_path = directory.path().join("transcript.vtt");

    let mut ffmpeg_command = Command::new(ffmpeg);
    ffmpeg_command
        .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
        .arg(media_path)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(&audio_path);
    run_command(
        ffmpeg_command,
        token,
        Duration::from_secs(3_600),
        "FFmpeg audio extraction",
    )
    .await?;

    let mut whisper_command = Command::new(whisper);
    whisper_command
        .arg("-m")
        .arg(model_path)
        .arg("-f")
        .arg(&audio_path)
        .arg("-ovtt")
        .arg("-of")
        .arg(&output_prefix);
    if let Some(language) = language {
        whisper_command.arg("-l").arg(language);
    }
    run_command(
        whisper_command,
        token,
        Duration::from_secs(12 * 3_600),
        "whisper.cpp transcription",
    )
    .await?;
    let contents = tokio::fs::read_to_string(&output_path)
        .await
        .map_err(|error| AppError::internal("could not read generated WebVTT transcript", error))?;
    let cues = transcripts::parse_webvtt_cues(&contents, "Local Whisper.vtt", language);
    if cues.is_empty() {
        return Err(AppError::new(
            "transcription_empty",
            "Local transcription completed without producing readable WebVTT cues.",
        ));
    }
    Ok(cues)
}

async fn run_command(
    mut command: Command,
    token: &CancellationToken,
    duration: Duration,
    operation: &str,
) -> Result<(), AppError> {
    sanitize_tokio_command(&mut command);
    command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal("could not start local inference process", error))?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::new(
            "local_inference_io",
            format!("{operation} diagnostics are unavailable."),
        )
    })?;
    let stderr_task = tokio::spawn(read_bounded(stderr, 8_192));
    let status = tokio::select! {
        _ = token.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stderr_task.abort();
            return Err(AppError::new("local_inference_cancelled", "Local transcript work was cancelled."));
        }
        result = timeout(duration, child.wait()) => match result {
            Ok(Ok(status)) => status,
            Ok(Err(error)) => return Err(AppError::internal("could not wait for local inference process", error)),
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                stderr_task.abort();
                return Err(AppError::new("local_inference_timeout", format!("{operation} exceeded its time limit.")));
            }
        }
    };
    let diagnostic = stderr_task
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    if !status.success() {
        return Err(AppError::with_details(
            "local_inference_failed",
            format!("{operation} failed."),
            diagnostic.chars().take(2_000).collect::<String>(),
        ));
    }
    Ok(())
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    limit: usize,
) -> Result<String, AppError> {
    let mut bytes = Vec::new();
    BufReader::new(reader)
        .take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| AppError::internal("could not read local inference diagnostics", error))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

async fn embed_document(
    client: &Client,
    model: &str,
    document: &SemanticTranscriptDocument,
    token: &CancellationToken,
) -> Result<Vec<f32>, AppError> {
    let bounded = document.text.chars().take(48_000).collect::<String>();
    let mut chunks = Vec::new();
    let characters = bounded.chars().collect::<Vec<_>>();
    for chunk in characters.chunks(6_000).take(8) {
        chunks.push(chunk.iter().collect::<String>());
    }
    let mut vectors = Vec::new();
    for chunk in chunks {
        vectors.push(embed_text(client, model, &chunk, token).await?);
    }
    average_vectors(&vectors).ok_or_else(|| {
        AppError::new(
            "ollama_embedding_invalid",
            "The local embedding model returned incompatible vectors.",
        )
    })
}

async fn embed_text(
    client: &Client,
    model: &str,
    text: &str,
    token: &CancellationToken,
) -> Result<Vec<f32>, AppError> {
    let request = client
        .post(format!("{OLLAMA_BASE_URL}/api/embed"))
        .json(&json!({ "model": model, "input": text }));
    let response = tokio::select! {
        _ = token.cancelled() => return Err(AppError::new("local_inference_cancelled", "Local transcript work was cancelled.")),
        response = request.send() => response.map_err(|_| AppError::new("ollama_unavailable", "The local Ollama service could not be reached."))?,
    };
    if !response.status().is_success() {
        return Err(AppError::new(
            "ollama_model_failed",
            format!("Ollama could not create an embedding with {model}."),
        ));
    }
    let payload = response.json::<OllamaEmbedResponse>().await.map_err(|_| {
        AppError::new(
            "ollama_embedding_invalid",
            "Ollama returned an unreadable embedding.",
        )
    })?;
    let vector = payload.embeddings.into_iter().next().ok_or_else(|| {
        AppError::new(
            "ollama_embedding_invalid",
            "Ollama returned an empty embedding.",
        )
    })?;
    validate_vector(vector)
}

fn validate_vector(vector: Vec<f32>) -> Result<Vec<f32>, AppError> {
    if !(8..=65_536).contains(&vector.len()) || vector.iter().any(|value| !value.is_finite()) {
        return Err(AppError::new(
            "ollama_embedding_invalid",
            "The local embedding model returned an invalid vector.",
        ));
    }
    Ok(vector)
}

fn average_vectors(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let dimensions = vectors.first()?.len();
    if vectors.iter().any(|vector| vector.len() != dimensions) {
        return None;
    }
    let mut average = vec![0.0_f32; dimensions];
    for vector in vectors {
        for (target, value) in average.iter_mut().zip(vector) {
            *target += *value;
        }
    }
    let divisor = vectors.len() as f32;
    for value in &mut average {
        *value /= divisor;
    }
    validate_vector(average).ok()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        let left = f64::from(*left);
        let right = f64::from(*right);
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    let denominator = left_norm.sqrt() * right_norm.sqrt();
    (denominator > f64::EPSILON).then_some(dot / denominator)
}

async fn ensure_ollama_model(client: &Client, model: &str) -> Result<(), AppError> {
    let response = client
        .get(format!("{OLLAMA_BASE_URL}/api/tags"))
        .send()
        .await
        .map_err(|_| {
            AppError::new(
                "ollama_unavailable",
                "Start Ollama locally before building semantic search.",
            )
        })?;
    let tags = response.json::<OllamaTags>().await.map_err(|_| {
        AppError::new(
            "ollama_unavailable",
            "Ollama returned an unreadable model list.",
        )
    })?;
    if tags
        .models
        .iter()
        .any(|entry| same_model(&entry.name, model))
    {
        Ok(())
    } else {
        Err(AppError::new(
            "semantic_model_missing",
            format!("Download {model} in Ollama before building semantic search."),
        ))
    }
}

fn local_client(request_timeout: Duration) -> Result<Client, AppError> {
    Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .timeout(request_timeout)
        .build()
        .map_err(|error| AppError::internal("could not configure the local Ollama client", error))
}

fn validate_model(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > 100
        || value.chars().any(char::is_whitespace)
        || value.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "invalid_semantic_model",
            "The Ollama embedding model name is invalid.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_model_path(value: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(value.trim());
    let metadata = path.symlink_metadata().map_err(|_| {
        AppError::new(
            "whisper_model_missing",
            "Choose an existing local whisper.cpp model file.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::new(
            "invalid_whisper_model",
            "The whisper.cpp model must be a regular, non-symlink file.",
        ));
    }
    Ok(path)
}

fn validate_media_path(path: &Path) -> Result<(), AppError> {
    let metadata = path.symlink_metadata().map_err(|_| {
        AppError::new(
            "transcription_media_missing",
            "A selected video file is no longer available.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::new(
            "invalid_transcription_media",
            "Local transcription accepts regular, non-symlink video files only.",
        ));
    }
    Ok(())
}

fn validate_language(value: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > 24
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(AppError::new(
            "invalid_transcription_language",
            "Use a short whisper.cpp language code such as en, es, or auto.",
        ));
    }
    Ok((value != "auto").then(|| value.to_owned()))
}

fn whisper_executable() -> Option<PathBuf> {
    ["whisper-cli", "whisper"]
        .into_iter()
        .find_map(|name| which::which(name).ok())
}

fn same_model(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
        || left
            .strip_suffix(":latest")
            .is_some_and(|value| value.eq_ignore_ascii_case(right))
        || right
            .strip_suffix(":latest")
            .is_some_and(|value| value.eq_ignore_ascii_case(left))
}

fn unavailable() -> AppError {
    AppError::new(
        "local_inference_unavailable",
        "Local transcript job state is unavailable.",
    )
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Deserialize)]
struct OllamaTags {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_ranks_matching_vectors() {
        let matching = cosine_similarity(&[1.0, 0.0, 0.0], &[0.9, 0.1, 0.0]).unwrap();
        let unrelated = cosine_similarity(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]).unwrap();
        assert!(matching > unrelated);
    }

    #[test]
    fn rejects_unsafe_language_and_model_names() {
        assert!(validate_language(Some("en-US")).is_ok());
        assert!(validate_language(Some("../en")).is_err());
        assert!(validate_model(DEFAULT_SEMANTIC_MODEL).is_ok());
        assert!(validate_model("model with spaces").is_err());
    }

    #[test]
    fn averages_equal_dimension_embeddings() {
        assert_eq!(
            average_vectors(&[vec![1.0; 8], vec![3.0; 8]]),
            Some(vec![2.0; 8])
        );
        assert!(average_vectors(&[vec![1.0; 8], vec![1.0; 9]]).is_none());
    }
}
