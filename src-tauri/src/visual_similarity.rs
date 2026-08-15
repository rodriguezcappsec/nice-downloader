use crate::{
    database::{Database, VisualSignatureTarget},
    engines::managed,
    errors::AppError,
    models::{LibraryVisualSimilarityJob, LibraryVisualSimilarityState},
};
use chrono::Utc;
use std::{
    path::Path,
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

const VISUAL_SIMILARITY_EVENT: &str = "library-visual-similarity-updated";
const BATCH_SIZE: usize = 100;

#[derive(Clone)]
pub struct VisualSimilarityManager {
    database: Database,
    active: Arc<Mutex<Option<(String, CancellationToken)>>>,
    job: Arc<Mutex<Option<LibraryVisualSimilarityJob>>>,
}

impl VisualSimilarityManager {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            active: Arc::new(Mutex::new(None)),
            job: Arc::new(Mutex::new(None)),
        }
    }

    pub fn job(&self) -> Result<Option<LibraryVisualSimilarityJob>, AppError> {
        self.job
            .lock()
            .map(|job| job.clone())
            .map_err(|_| unavailable())
    }

    pub fn start(&self, app: AppHandle) -> Result<LibraryVisualSimilarityJob, AppError> {
        let mut active = self.active.lock().map_err(|_| unavailable())?;
        if active.is_some() {
            return Err(AppError::new(
                "visual_similarity_running",
                "The visual similarity index is already being refreshed.",
            ));
        }
        let status = self.database.visual_similarity_status()?;
        let now = Utc::now();
        let mut job = LibraryVisualSimilarityJob {
            id: Uuid::new_v4().to_string(),
            state: if status.stale_items == 0 {
                LibraryVisualSimilarityState::Completed
            } else {
                LibraryVisualSimilarityState::Running
            },
            checked_items: 0,
            total_items: status.stale_items,
            indexed_items: 0,
            failed_items: 0,
            current_title: None,
            error: None,
            started_at: now,
            updated_at: now,
        };
        if status.stale_items == 0 {
            let _ = app.emit(VISUAL_SIMILARITY_EVENT, &job);
            *self.job.lock().map_err(|_| unavailable())? = Some(job.clone());
            return Ok(job);
        }
        let token = CancellationToken::new();
        *active = Some((job.id.clone(), token.clone()));
        drop(active);
        *self.job.lock().map_err(|_| unavailable())? = Some(job.clone());
        let _ = app.emit(VISUAL_SIMILARITY_EVENT, &job);

        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let result = manager.run(&app, &mut job, token.clone()).await;
            if let Err(error) = result {
                job.state = if token.is_cancelled() {
                    LibraryVisualSimilarityState::Cancelled
                } else {
                    LibraryVisualSimilarityState::Failed
                };
                job.error = (!token.is_cancelled()).then_some(error.message);
            } else {
                job.state = if token.is_cancelled() {
                    LibraryVisualSimilarityState::Cancelled
                } else {
                    LibraryVisualSimilarityState::Completed
                };
            }
            job.current_title = None;
            job.updated_at = Utc::now();
            if let Ok(mut stored) = manager.job.lock() {
                *stored = Some(job.clone());
            }
            if let Ok(mut active) = manager.active.lock() {
                active.take();
            }
            let _ = app.emit(VISUAL_SIMILARITY_EVENT, &job);
        });
        self.job()?.ok_or_else(|| {
            AppError::new(
                "visual_similarity_unavailable",
                "The index job did not start.",
            )
        })
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), AppError> {
        let active = self.active.lock().map_err(|_| unavailable())?;
        let Some((active_id, token)) = active.as_ref() else {
            return Err(AppError::new(
                "visual_similarity_not_running",
                "No visual similarity refresh is running.",
            ));
        };
        if active_id != job_id {
            return Err(AppError::new(
                "visual_similarity_job_mismatch",
                "That visual similarity refresh is no longer active.",
            ));
        }
        token.cancel();
        Ok(())
    }

    async fn run(
        &self,
        app: &AppHandle,
        job: &mut LibraryVisualSimilarityJob,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
            AppError::new(
                "ffmpeg_missing",
                "Install FFmpeg before building the visual similarity index.",
            )
        })?;
        let mut after_id = None;
        loop {
            if token.is_cancelled() {
                return Ok(());
            }
            let targets = self
                .database
                .list_visual_signature_targets_after(after_id.as_deref(), BATCH_SIZE)?;
            if targets.is_empty() {
                break;
            }
            for target in &targets {
                if token.is_cancelled() {
                    return Ok(());
                }
                job.current_title = Some(target.title.clone());
                job.updated_at = Utc::now();
                self.store_and_emit(app, job)?;
                match generate_signature(&ffmpeg, target, &token).await {
                    Ok(signature) => {
                        self.database.save_visual_signature(target, signature)?;
                        job.indexed_items += 1;
                    }
                    Err(error) if error.code == "visual_similarity_cancelled" => return Ok(()),
                    Err(_) => {
                        self.database.remove_visual_signature(&target.id)?;
                        job.failed_items += 1;
                    }
                }
                job.checked_items += 1;
                job.updated_at = Utc::now();
                self.store_and_emit(app, job)?;
            }
            after_id = targets.last().map(|target| target.id.clone());
        }
        Ok(())
    }

    fn store_and_emit(
        &self,
        app: &AppHandle,
        job: &LibraryVisualSimilarityJob,
    ) -> Result<(), AppError> {
        *self.job.lock().map_err(|_| unavailable())? = Some(job.clone());
        let _ = app.emit(VISUAL_SIMILARITY_EVENT, job);
        Ok(())
    }
}

async fn generate_signature(
    ffmpeg: &Path,
    target: &VisualSignatureTarget,
    token: &CancellationToken,
) -> Result<u64, AppError> {
    let path = Path::new(&target.path);
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        AppError::new(
            "visual_similarity_source_missing",
            "A visual similarity source is no longer available.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::new(
            "visual_similarity_source_invalid",
            "Visual similarity uses regular, non-symlink media files only.",
        ));
    }
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command.args(["-hide_banner", "-loglevel", "error"]);
    if matches!(target.media_kind.as_str(), "video" | "both") {
        let seek = target
            .duration_seconds
            .filter(|duration| duration.is_finite() && *duration > 0.5)
            .map(|duration| (duration * 0.1).min(duration - 0.25).max(0.0))
            .unwrap_or(0.0);
        command.arg("-ss").arg(format!("{seek:.6}"));
    }
    command
        .arg("-i")
        .arg(path)
        .args([
            "-frames:v",
            "1",
            "-vf",
            "scale=9:8:flags=area,format=gray",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal("could not start FFmpeg", error))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::new(
            "visual_similarity_io",
            "FFmpeg visual fingerprint output was unavailable.",
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::new(
            "visual_similarity_io",
            "FFmpeg visual fingerprint diagnostics were unavailable.",
        )
    })?;
    let stdout_task = tokio::spawn(read_bounded(stdout, 73));
    let stderr_task = tokio::spawn(read_bounded(stderr, 4_096));
    let status = tokio::select! {
        _ = token.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(AppError::new("visual_similarity_cancelled", "Visual indexing was cancelled."));
        }
        result = timeout(Duration::from_secs(30), child.wait()) => {
            match result {
                Ok(Ok(status)) => status,
                Ok(Err(error)) => return Err(AppError::internal("could not wait for FFmpeg", error)),
                Err(_) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    stdout_task.abort();
                    stderr_task.abort();
                    return Err(AppError::new("visual_similarity_timeout", "FFmpeg took too long to fingerprint media."));
                }
            }
        }
    };
    let pixels = stdout_task
        .await
        .map_err(|error| AppError::internal("visual fingerprint task stopped", error))??;
    let diagnostic = stderr_task
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    if !status.success() || pixels.len() != 72 {
        return Err(AppError::with_details(
            "visual_similarity_failed",
            "FFmpeg could not create a visual fingerprint for this item.",
            String::from_utf8_lossy(&diagnostic)
                .chars()
                .take(2_000)
                .collect::<String>(),
        ));
    }
    Ok(difference_hash(&pixels))
}

async fn read_bounded<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    limit: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    BufReader::new(reader)
        .take(u64::try_from(limit).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .await?;
    Ok(bytes)
}

fn difference_hash(pixels: &[u8]) -> u64 {
    let mut signature = 0_u64;
    for row in 0..8 {
        for column in 0..8 {
            let left = pixels[row * 9 + column];
            let right = pixels[row * 9 + column + 1];
            if left > right {
                signature |= 1_u64 << (row * 8 + column);
            }
        }
    }
    signature
}

fn unavailable() -> AppError {
    AppError::new(
        "visual_similarity_unavailable",
        "Visual similarity state is temporarily unavailable.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn difference_hash_is_deterministic_and_direction_sensitive() {
        let ascending = (0_u8..72).collect::<Vec<_>>();
        let descending = ascending.iter().rev().copied().collect::<Vec<_>>();
        assert_eq!(difference_hash(&ascending), 0);
        assert_eq!(difference_hash(&descending), u64::MAX);
    }
}
