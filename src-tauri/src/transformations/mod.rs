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
        let duration = operation_duration(&request).await.unwrap_or(None);
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
        append_operation_arguments(&mut command, &request).await?;
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
            let split_times = chapter_split_times(&request.input_path).await?;
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

async fn operation_duration(request: &StartTransformationRequest) -> Result<Option<f64>, AppError> {
    if let TransformationOperation::Trim {
        start_seconds,
        end_seconds,
    } = request.operation
    {
        return Ok(Some(end_seconds - start_seconds));
    }
    probe_duration(&request.input_path).await
}

async fn probe_duration(input: &str) -> Result<Option<f64>, AppError> {
    let ffprobe = managed::resolve("ffprobe").ok_or_else(|| {
        AppError::new(
            "ffprobe_missing",
            "Install ffprobe to calculate transformation progress.",
        )
    })?;
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    let output = command
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
            input,
        ])
        .output()
        .await
        .map_err(|error| AppError::internal("could not start ffprobe", error))?;
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

async fn chapter_split_times(input: &str) -> Result<Vec<String>, AppError> {
    let ffprobe = managed::resolve("ffprobe")
        .ok_or_else(|| AppError::new("ffprobe_missing", "Install ffprobe to read chapters."))?;
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    let output = command
        .args(["-v", "error", "-show_chapters", "-of", "json", input])
        .output()
        .await
        .map_err(|error| AppError::internal("could not start ffprobe", error))?;
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
}
