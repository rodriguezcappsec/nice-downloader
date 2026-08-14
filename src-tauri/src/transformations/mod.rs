use crate::{
    database::Database,
    engines::managed,
    errors::AppError,
    models::{
        AudioTransformCodec, StartTransformationRequest, TransformAudioCodec, TransformVideoCodec,
        TransformationOperation, TransformationState, TransformationTask,
    },
};
use chrono::Utc;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    time::{timeout, Duration},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const TASK_EVENT: &str = "transformation-task-updated";

#[derive(Clone)]
pub struct TransformationManager {
    database: Database,
    tasks: Arc<Mutex<HashMap<String, TransformationTask>>>,
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl TransformationManager {
    pub fn new(database: Database) -> Result<Self, AppError> {
        let mut tasks = HashMap::new();
        for mut task in database.list_transformation_tasks()? {
            if matches!(
                task.state,
                TransformationState::Queued | TransformationState::Running
            ) {
                cleanup_staged_output(&task.output_path, &task.operation, &task.id);
                task.state = TransformationState::Failed;
                task.current_operation = "Interrupted when the application closed".to_owned();
                task.error =
                    Some("The transformation was interrupted. Start it again to retry.".to_owned());
                task.updated_at = Utc::now();
                database.save_transformation_task(&task)?;
            }
            tasks.insert(task.id.clone(), task);
        }
        Ok(Self {
            database,
            tasks: Arc::new(Mutex::new(tasks)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn list(&self) -> Result<Vec<TransformationTask>, AppError> {
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|_| unavailable())?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
        Ok(tasks)
    }

    pub fn start(
        &self,
        app: AppHandle,
        request: StartTransformationRequest,
    ) -> Result<TransformationTask, AppError> {
        let request = validate_request(request)?;
        let now = Utc::now();
        let task = TransformationTask {
            id: Uuid::new_v4().to_string(),
            input_path: request.input_path.clone(),
            output_path: request.output_path.clone(),
            operation: request.operation.clone(),
            state: TransformationState::Queued,
            progress_percent: 0.0,
            current_operation: "Waiting for FFmpeg".to_owned(),
            error: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        self.database.save_transformation_task(&task)?;
        self.tasks
            .lock()
            .map_err(|_| unavailable())?
            .insert(task.id.clone(), task.clone());
        let token = CancellationToken::new();
        self.cancellations
            .lock()
            .map_err(|_| unavailable())?
            .insert(task.id.clone(), token.clone());
        let manager = self.clone();
        let task_id = task.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = manager.run(app.clone(), &task_id, request, token).await {
                let _ = manager.finish(
                    &app,
                    &task_id,
                    TransformationState::Failed,
                    "Transformation failed",
                    Some(error.message),
                );
            }
            if let Ok(mut cancellations) = manager.cancellations.lock() {
                cancellations.remove(&task_id);
            }
        });
        Ok(task)
    }

    pub fn cancel(&self, id: &str) -> Result<(), AppError> {
        let token = self
            .cancellations
            .lock()
            .map_err(|_| unavailable())?
            .get(id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "transformation_not_running",
                    "This transformation is not running.",
                )
            })?;
        token.cancel();
        Ok(())
    }

    async fn run(
        &self,
        app: AppHandle,
        id: &str,
        request: StartTransformationRequest,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
            AppError::new(
                "ffmpeg_missing",
                "Install FFmpeg before transforming media.",
            )
        })?;
        let duration = operation_duration(&request, &token).await.unwrap_or(None);
        if token.is_cancelled() {
            self.finish(&app, id, TransformationState::Cancelled, "Cancelled", None)?;
            return Ok(());
        }
        let mut staged = StagedTransformationOutput::new(&request, id)?;
        let staged_request = staged.request().clone();
        self.update(&app, id, |task| {
            task.state = TransformationState::Running;
            task.current_operation = operation_label(&task.operation).to_owned();
        })?;

        let mut command = Command::new(ffmpeg);
        crate::utils::process::sanitize_tokio_command(&mut command);
        command.arg("-hide_banner").arg("-loglevel").arg("error");
        if request.overwrite {
            command.arg("-y");
        } else {
            command.arg("-n");
        }
        if let Err(error) = append_operation_arguments(&mut command, &staged_request, &token).await
        {
            if token.is_cancelled() {
                self.finish(&app, id, TransformationState::Cancelled, "Cancelled", None)?;
                return Ok(());
            }
            return Err(error);
        }
        command
            .arg("-progress")
            .arg("pipe:1")
            .arg("-nostats")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::internal("could not start FFmpeg", error))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::new("ffmpeg_io", "FFmpeg progress output was unavailable."))?;
        let mut stderr = child.stderr.take().ok_or_else(|| {
            AppError::new("ffmpeg_io", "FFmpeg diagnostic output was unavailable.")
        })?;
        let stderr_task = tokio::spawn(async move {
            let mut value = String::new();
            let _ = stderr.read_to_string(&mut value).await;
            value
        });
        let mut lines = BufReader::new(stdout).lines();
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    self.finish(&app, id, TransformationState::Cancelled, "Cancelled", None)?;
                    return Ok(());
                }
                line = lines.next_line() => {
                    match line.map_err(|error| AppError::internal("could not read FFmpeg progress", error))? {
                        Some(line) => {
                            if let Some(value) = line.strip_prefix("out_time_us=").and_then(|value| value.parse::<f64>().ok()) {
                                if let Some(duration) = duration.filter(|value| *value > 0.0) {
                                    let percent = ((value / 1_000_000.0) / duration * 100.0).clamp(0.0, 99.5);
                                    self.update(&app, id, |task| task.progress_percent = percent)?;
                                }
                            }
                        }
                        None => break,
                    }
                }
            }
        }
        let status = child
            .wait()
            .await
            .map_err(|error| AppError::internal("could not wait for FFmpeg", error))?;
        let diagnostic = stderr_task.await.unwrap_or_default();
        if !status.success() {
            return Err(AppError::with_details(
                "ffmpeg_failed",
                "FFmpeg could not complete this transformation.",
                diagnostic.chars().take(4000).collect::<String>(),
            ));
        }
        if token.is_cancelled() {
            self.finish(&app, id, TransformationState::Cancelled, "Cancelled", None)?;
            return Ok(());
        }
        staged.publish()?;
        self.finish(
            &app,
            id,
            TransformationState::Completed,
            "Transformation completed",
            None,
        )?;
        Ok(())
    }

    fn update(
        &self,
        app: &AppHandle,
        id: &str,
        update: impl FnOnce(&mut TransformationTask),
    ) -> Result<(), AppError> {
        let task = {
            let mut tasks = self.tasks.lock().map_err(|_| unavailable())?;
            let task = tasks.get_mut(id).ok_or_else(|| {
                AppError::new("transformation_not_found", "Transformation task not found.")
            })?;
            update(task);
            task.updated_at = Utc::now();
            self.database.save_transformation_task(task)?;
            task.clone()
        };
        let _ = app.emit(TASK_EVENT, task);
        Ok(())
    }

    fn finish(
        &self,
        app: &AppHandle,
        id: &str,
        state: TransformationState,
        operation: &str,
        error: Option<String>,
    ) -> Result<(), AppError> {
        self.update(app, id, |task| {
            task.state = state;
            task.current_operation = operation.to_owned();
            task.error = error;
            if state == TransformationState::Completed {
                task.progress_percent = 100.0;
            }
            task.completed_at = Some(Utc::now());
        })
    }
}

struct StagedTransformationOutput {
    original: StartTransformationRequest,
    staged: StartTransformationRequest,
    published: bool,
}

impl StagedTransformationOutput {
    fn new(request: &StartTransformationRequest, task_id: &str) -> Result<Self, AppError> {
        let output = PathBuf::from(&request.output_path);
        output
            .parent()
            .filter(|path| path.is_dir())
            .ok_or_else(|| {
                AppError::new(
                    "invalid_output_path",
                    "Choose an output inside an existing folder.",
                )
            })?;
        let split_chapters = matches!(request.operation, TransformationOperation::SplitChapters);
        if output.exists() && !request.overwrite && !split_chapters {
            return Err(AppError::new(
                "output_exists",
                "The transformation output already exists. Enable replacement or choose another destination.",
            ));
        }
        if split_chapters && output.exists() && !output.is_dir() {
            return Err(AppError::new(
                "invalid_output_path",
                "Choose a folder for chapter outputs.",
            ));
        }
        let staged_path = staged_output_path(&output, &request.operation, task_id);
        let mut staged = request.clone();
        staged.output_path = staged_path.to_string_lossy().into_owned();
        Ok(Self {
            original: request.clone(),
            staged,
            published: false,
        })
    }

    fn request(&self) -> &StartTransformationRequest {
        &self.staged
    }

    fn publish(&mut self) -> Result<(), AppError> {
        let staged = PathBuf::from(&self.staged.output_path);
        let output = PathBuf::from(&self.original.output_path);
        match &self.original.operation {
            TransformationOperation::SplitChapters => {
                std::fs::create_dir_all(&output).map_err(|error| {
                    AppError::internal("could not create chapter output folder", error)
                })?;
                let entries = std::fs::read_dir(&staged)
                    .map_err(|error| {
                        AppError::internal("could not read staged chapter outputs", error)
                    })?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        AppError::internal("could not read staged chapter output", error)
                    })?;
                if !self.original.overwrite
                    && entries
                        .iter()
                        .any(|entry| output.join(entry.file_name()).exists())
                {
                    return Err(AppError::new(
                        "output_exists",
                        "A chapter output already exists. Enable replacement or choose another folder.",
                    ));
                }
                publish_staged_chapters(&entries, &output, self.original.overwrite)?;
                std::fs::remove_dir(&staged).map_err(|error| {
                    AppError::internal("could not finalize chapter outputs", error)
                })?;
            }
            _ => publish_staged_file(&staged, &output, self.original.overwrite)?,
        }
        self.published = true;
        Ok(())
    }
}

impl Drop for StagedTransformationOutput {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let staged = PathBuf::from(&self.staged.output_path);
        if matches!(
            self.original.operation,
            TransformationOperation::SplitChapters
        ) {
            let _ = std::fs::remove_dir_all(staged);
        } else {
            let _ = std::fs::remove_file(staged);
        }
    }
}

fn staged_output_path(
    output: &Path,
    operation: &TransformationOperation,
    task_id: &str,
) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    match operation {
        TransformationOperation::SplitChapters => {
            parent.join(format!(".nice-downloader-chapters-{task_id}.part"))
        }
        _ => {
            let stem = output
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("output");
            let extension = output
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| format!(".{value}"))
                .unwrap_or_default();
            parent.join(format!(".{stem}-{task_id}.part{extension}"))
        }
    }
}

fn cleanup_staged_output(output: &str, operation: &TransformationOperation, task_id: &str) {
    let staged = staged_output_path(Path::new(output), operation, task_id);
    if matches!(operation, TransformationOperation::SplitChapters) {
        let _ = std::fs::remove_dir_all(staged);
    } else {
        let _ = std::fs::remove_file(staged);
    }
}

struct PublishedChapter {
    staged: PathBuf,
    destination: PathBuf,
    backup: Option<PathBuf>,
}

fn publish_staged_chapters(
    entries: &[std::fs::DirEntry],
    output: &Path,
    overwrite: bool,
) -> Result<(), AppError> {
    let mut ordered = entries.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|entry| entry.file_name());
    let mut published = Vec::<PublishedChapter>::new();

    for entry in ordered {
        let staged = entry.path();
        let destination = output.join(entry.file_name());
        let backup = if destination.exists() {
            if !overwrite {
                rollback_published_chapters(&mut published);
                return Err(AppError::new(
                    "output_exists",
                    "A chapter output already exists. Enable replacement or choose another folder.",
                ));
            }
            let backup = destination.with_file_name(format!(
                ".{}-{}.backup",
                destination
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("chapter"),
                Uuid::new_v4()
            ));
            if let Err(error) = std::fs::rename(&destination, &backup) {
                rollback_published_chapters(&mut published);
                return Err(AppError::internal(
                    "could not back up existing chapter output",
                    error,
                ));
            }
            Some(backup)
        } else {
            None
        };

        if let Err(error) = std::fs::hard_link(&staged, &destination) {
            if let Some(backup) = &backup {
                let _ = std::fs::rename(backup, &destination);
            }
            rollback_published_chapters(&mut published);
            return Err(AppError::internal(
                "could not publish chapter output without overwriting",
                error,
            ));
        }
        if let Err(error) = std::fs::remove_file(&staged) {
            let _ = std::fs::remove_file(&destination);
            if let Some(backup) = &backup {
                let _ = std::fs::rename(backup, &destination);
            }
            rollback_published_chapters(&mut published);
            return Err(AppError::internal(
                "could not finalize staged chapter output",
                error,
            ));
        }
        published.push(PublishedChapter {
            staged,
            destination,
            backup,
        });
    }

    for chapter in published {
        if let Some(backup) = chapter.backup {
            if let Err(error) = std::fs::remove_file(&backup) {
                tracing::warn!(
                    %error,
                    path = %backup.display(),
                    "could not remove replaced chapter backup"
                );
            }
        }
    }
    Ok(())
}

fn rollback_published_chapters(published: &mut Vec<PublishedChapter>) {
    for chapter in published.drain(..).rev() {
        if std::fs::hard_link(&chapter.destination, &chapter.staged).is_ok() {
            let _ = std::fs::remove_file(&chapter.destination);
        }
        if let Some(backup) = chapter.backup {
            let _ = std::fs::rename(backup, chapter.destination);
        }
    }
}

fn publish_staged_file(staged: &Path, output: &Path, overwrite: bool) -> Result<(), AppError> {
    if overwrite {
        if !output.exists() {
            return std::fs::rename(staged, output).map_err(|error| {
                AppError::internal("could not publish transformation output", error)
            });
        }
        let backup = output.with_file_name(format!(
            ".{}-{}.backup",
            output
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("output"),
            Uuid::new_v4()
        ));
        std::fs::rename(output, &backup).map_err(|error| {
            AppError::internal("could not preserve existing transformation output", error)
        })?;
        if let Err(error) = std::fs::rename(staged, output) {
            let _ = std::fs::rename(&backup, output);
            return Err(AppError::internal(
                "could not publish transformation output",
                error,
            ));
        }
        if let Err(error) = std::fs::remove_file(&backup) {
            tracing::warn!(path = %backup.display(), %error, "could not remove transformation output backup");
        }
        Ok(())
    } else {
        std::fs::hard_link(staged, output).map_err(|error| {
            AppError::internal(
                "could not publish transformation output without replacing it",
                error,
            )
        })?;
        std::fs::remove_file(staged).map_err(|error| {
            AppError::internal("could not remove staged transformation output", error)
        })
    }
}

fn unavailable() -> AppError {
    AppError::new(
        "transformation_unavailable",
        "Transformation state is temporarily unavailable.",
    )
}

fn validate_request(
    mut request: StartTransformationRequest,
) -> Result<StartTransformationRequest, AppError> {
    let input = PathBuf::from(&request.input_path)
        .canonicalize()
        .map_err(|_| AppError::new("invalid_input_path", "Choose an existing local media file."))?;
    if !input.is_file() {
        return Err(AppError::new(
            "invalid_input_path",
            "Choose a local media file, not a folder.",
        ));
    }
    let output = PathBuf::from(&request.output_path);
    if !output.is_absolute() {
        return Err(AppError::new(
            "invalid_output_path",
            "Choose an absolute output path.",
        ));
    }
    if input == output {
        return Err(AppError::new(
            "invalid_output_path",
            "Input and output paths must be different.",
        ));
    }
    if let TransformationOperation::Trim {
        start_seconds,
        end_seconds,
    } = request.operation
    {
        if !start_seconds.is_finite()
            || !end_seconds.is_finite()
            || start_seconds < 0.0
            || end_seconds <= start_seconds
        {
            return Err(AppError::new(
                "invalid_trim_range",
                "Trim end time must be greater than the start time.",
            ));
        }
    }
    request.input_path = input.to_string_lossy().into_owned();
    Ok(request)
}

fn operation_label(operation: &TransformationOperation) -> &'static str {
    match operation {
        TransformationOperation::Trim { .. } => "Trimming media",
        TransformationOperation::ExtractAudio { .. } => "Extracting audio",
        TransformationOperation::Transcode { .. } => "Converting media",
        TransformationOperation::SplitChapters => "Splitting chapters",
    }
}

async fn append_operation_arguments(
    command: &mut Command,
    request: &StartTransformationRequest,
    token: &CancellationToken,
) -> Result<(), AppError> {
    match &request.operation {
        TransformationOperation::Trim {
            start_seconds,
            end_seconds,
        } => {
            command
                .arg("-ss")
                .arg(start_seconds.to_string())
                .arg("-to")
                .arg(end_seconds.to_string())
                .arg("-i")
                .arg(&request.input_path)
                .arg("-map")
                .arg("0")
                .arg("-c")
                .arg("copy")
                .arg(&request.output_path);
        }
        TransformationOperation::ExtractAudio { codec } => {
            let encoder = match codec {
                AudioTransformCodec::Mp3 => "libmp3lame",
                AudioTransformCodec::Aac => "aac",
                AudioTransformCodec::Opus => "libopus",
            };
            command
                .arg("-i")
                .arg(&request.input_path)
                .arg("-vn")
                .arg("-c:a")
                .arg(encoder)
                .arg(&request.output_path);
        }
        TransformationOperation::Transcode {
            video_codec,
            audio_codec,
            ..
        } => {
            let video = match video_codec {
                TransformVideoCodec::Copy => "copy",
                TransformVideoCodec::H264 => "libx264",
                TransformVideoCodec::Hevc => "libx265",
                TransformVideoCodec::Vp9 => "libvpx-vp9",
                TransformVideoCodec::Av1 => "libsvtav1",
            };
            let audio = match audio_codec {
                TransformAudioCodec::Copy => "copy",
                TransformAudioCodec::Aac => "aac",
                TransformAudioCodec::Opus => "libopus",
            };
            command
                .arg("-i")
                .arg(&request.input_path)
                .arg("-map")
                .arg("0")
                .arg("-c:v")
                .arg(video)
                .arg("-c:a")
                .arg(audio)
                .arg(&request.output_path);
        }
        TransformationOperation::SplitChapters => {
            let output = Path::new(&request.output_path);
            std::fs::create_dir_all(output).map_err(|error| {
                AppError::internal("could not create the chapter output folder", error)
            })?;
            let split_times = chapter_split_times(&request.input_path, token).await?;
            if split_times.is_empty() {
                return Err(AppError::new(
                    "chapters_missing",
                    "This media file does not contain multiple chapters.",
                ));
            }
            command
                .arg("-i")
                .arg(&request.input_path)
                .arg("-map")
                .arg("0")
                .arg("-c")
                .arg("copy")
                .arg("-f")
                .arg("segment")
                .arg("-segment_times")
                .arg(split_times.join(","))
                .arg("-reset_timestamps")
                .arg("1")
                .arg(output.join("chapter-%03d.mkv"));
        }
    }
    Ok(())
}

async fn run_ffprobe(
    command: &mut Command,
    token: &CancellationToken,
) -> Result<ProbeOutput, AppError> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal("could not start ffprobe", error))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::new("ffprobe_io", "ffprobe metadata output was unavailable."))?;
    let stdout_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).await.map(|_| bytes)
    });

    let status = tokio::select! {
        _ = token.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            return Err(AppError::new(
                "transformation_cancelled",
                "The transformation was cancelled.",
            ));
        },
        result = timeout(Duration::from_secs(30), child.wait()) => {
            match result {
                Ok(Ok(status)) => status,
                Ok(Err(error)) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    stdout_task.abort();
                    return Err(AppError::internal("could not wait for ffprobe", error));
                }
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    stdout_task.abort();
                    return Err(AppError::new(
                        "ffprobe_timeout",
                        "ffprobe took too long to inspect this media file.",
                    ));
                }
            }
        }
    };
    let stdout = stdout_task
        .await
        .map_err(|error| AppError::internal("ffprobe output task stopped", error))?
        .map_err(|error| AppError::internal("could not read ffprobe output", error))?;
    Ok(ProbeOutput { status, stdout })
}

struct ProbeOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
}

async fn operation_duration(
    request: &StartTransformationRequest,
    token: &CancellationToken,
) -> Result<Option<f64>, AppError> {
    if let TransformationOperation::Trim {
        start_seconds,
        end_seconds,
    } = request.operation
    {
        return Ok(Some(end_seconds - start_seconds));
    }
    probe_duration(&request.input_path, token).await
}

async fn probe_duration(input: &str, token: &CancellationToken) -> Result<Option<f64>, AppError> {
    let ffprobe = managed::resolve("ffprobe").ok_or_else(|| {
        AppError::new(
            "ffprobe_missing",
            "Install ffprobe to calculate transformation progress.",
        )
    })?;
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "json",
        input,
    ]);
    let output = run_ffprobe(&mut command, token).await?;
    if !output.status.success() {
        return Ok(None);
    }
    #[derive(Deserialize)]
    struct Probe {
        format: ProbeFormat,
    }
    #[derive(Deserialize)]
    struct ProbeFormat {
        duration: Option<String>,
    }
    Ok(serde_json::from_slice::<Probe>(&output.stdout)
        .ok()
        .and_then(|probe| probe.format.duration?.parse().ok()))
}

async fn chapter_split_times(
    input: &str,
    token: &CancellationToken,
) -> Result<Vec<String>, AppError> {
    let ffprobe = managed::resolve("ffprobe")
        .ok_or_else(|| AppError::new("ffprobe_missing", "Install ffprobe to read chapters."))?;
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command.args(["-v", "error", "-show_chapters", "-of", "json", input]);
    let output = run_ffprobe(&mut command, token).await?;
    if !output.status.success() {
        return Err(AppError::new(
            "ffprobe_failed",
            "ffprobe could not read chapter metadata.",
        ));
    }
    #[derive(Deserialize)]
    struct Chapters {
        chapters: Vec<Chapter>,
    }
    #[derive(Deserialize)]
    struct Chapter {
        start_time: String,
    }
    let chapters: Chapters = serde_json::from_slice(&output.stdout)
        .map_err(|error| AppError::internal("could not parse chapter metadata", error))?;
    Ok(chapters
        .chapters
        .into_iter()
        .skip(1)
        .map(|chapter| chapter.start_time)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_invalid_trim_ranges() {
        let request = StartTransformationRequest {
            input_path: "/missing".to_owned(),
            output_path: "/tmp/out.mp4".to_owned(),
            operation: TransformationOperation::Trim {
                start_seconds: 8.0,
                end_seconds: 2.0,
            },
            overwrite: false,
        };
        assert!(validate_request(request).is_err());
    }

    #[test]
    fn staged_output_preserves_existing_file_when_work_is_abandoned() {
        let directory = tempdir().expect("temporary directory");
        let output = directory.path().join("converted.mp4");
        std::fs::write(&output, b"original").expect("original output");
        let request = StartTransformationRequest {
            input_path: directory
                .path()
                .join("input.mkv")
                .to_string_lossy()
                .into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            operation: TransformationOperation::Transcode {
                container: "mp4".to_owned(),
                video_codec: TransformVideoCodec::H264,
                audio_codec: TransformAudioCodec::Aac,
            },
            overwrite: true,
        };
        let staged = StagedTransformationOutput::new(&request, "task-one").expect("staging output");
        std::fs::write(&staged.request().output_path, b"partial").expect("partial output");
        let staged_path = PathBuf::from(&staged.request().output_path);
        drop(staged);
        assert_eq!(
            std::fs::read(&output).expect("original remains"),
            b"original"
        );
        assert!(!staged_path.exists());
    }

    #[test]
    fn successful_staged_output_replaces_existing_file() {
        let directory = tempdir().expect("temporary directory");
        let output = directory.path().join("converted.mp4");
        std::fs::write(&output, b"original").expect("original output");
        let request = StartTransformationRequest {
            input_path: directory
                .path()
                .join("input.mkv")
                .to_string_lossy()
                .into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            operation: TransformationOperation::Transcode {
                container: "mp4".to_owned(),
                video_codec: TransformVideoCodec::H264,
                audio_codec: TransformAudioCodec::Aac,
            },
            overwrite: true,
        };
        let mut staged =
            StagedTransformationOutput::new(&request, "task-two").expect("staging output");
        std::fs::write(&staged.request().output_path, b"complete").expect("complete output");
        staged.publish().expect("publish output");
        assert_eq!(
            std::fs::read(output).expect("published output"),
            b"complete"
        );
    }

    #[test]
    fn chapter_publication_rolls_back_if_a_later_output_fails() {
        let directory = tempdir().expect("temporary directory");
        let output = directory.path().join("chapters");
        let request = StartTransformationRequest {
            input_path: directory
                .path()
                .join("input.mkv")
                .to_string_lossy()
                .into_owned(),
            output_path: output.to_string_lossy().into_owned(),
            operation: TransformationOperation::SplitChapters,
            overwrite: false,
        };
        let mut staged =
            StagedTransformationOutput::new(&request, "task-chapters").expect("staging output");
        let staged_directory = PathBuf::from(&staged.request().output_path);
        std::fs::create_dir_all(&staged_directory).expect("staged directory");
        std::fs::write(staged_directory.join("chapter-001.mkv"), b"first").expect("first chapter");
        std::fs::create_dir(staged_directory.join("chapter-002.mkv"))
            .expect("invalid second chapter");

        assert!(staged.publish().is_err());
        assert!(!output.join("chapter-001.mkv").exists());
        assert_eq!(
            std::fs::read(staged_directory.join("chapter-001.mkv"))
                .expect("first chapter restored"),
            b"first"
        );
    }
}
