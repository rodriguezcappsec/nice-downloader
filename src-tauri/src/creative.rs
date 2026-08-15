use crate::{
    database::Database,
    engines::managed,
    errors::AppError,
    models::{LibraryMediaKind, LibraryStoryboard, LibraryStoryboardFrame},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tokio::{
    process::Command,
    sync::Mutex,
    time::{timeout, Duration},
};

const MIN_STORYBOARD_FRAMES: u8 = 4;
const MAX_STORYBOARD_FRAMES: u8 = 24;
const MAX_FRAME_BYTES: u64 = 1_500_000;
const STORYBOARD_TIMEOUT_SECONDS: u64 = 90;

#[derive(Clone)]
pub struct CreativeManager {
    database: Database,
    cache_directory: PathBuf,
    generation_lock: Arc<Mutex<()>>,
}

impl CreativeManager {
    pub fn new(database: Database, cache_directory: PathBuf) -> Self {
        Self {
            database,
            cache_directory,
            generation_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn storyboard(
        &self,
        item_id: &str,
        frame_count: u8,
    ) -> Result<LibraryStoryboard, AppError> {
        if !(MIN_STORYBOARD_FRAMES..=MAX_STORYBOARD_FRAMES).contains(&frame_count) {
            return Err(AppError::new(
                "invalid_storyboard_frame_count",
                "Choose between 4 and 24 storyboard frames.",
            ));
        }
        let item = self.database.get_library_item(item_id)?;
        if item.missing || item.media_kind != Some(LibraryMediaKind::Video) {
            return Err(AppError::new(
                "storyboard_video_required",
                "Storyboard previews require an available video.",
            ));
        }
        let duration = item
            .duration_seconds
            .filter(|value| value.is_finite() && *value > 0.1)
            .ok_or_else(|| {
                AppError::new(
                    "storyboard_duration_unknown",
                    "This video's duration is unavailable. Rescan its metadata and try again.",
                )
            })?;
        let source = PathBuf::from(&item.path);
        validate_storyboard_source(&source)?;
        let fingerprint = item.modified_at_ms.unwrap_or_default().unsigned_abs();
        let output_directory = self
            .cache_directory
            .join(format!("{}-{fingerprint}-{frame_count}", item.id));
        let _guard = self.generation_lock.lock().await;
        if !storyboard_cache_complete(&output_directory, frame_count) {
            generate_storyboard(&source, &output_directory, duration, frame_count).await?;
            cleanup_old_storyboards(&self.cache_directory, &item.id, &output_directory);
        }
        let frames = read_storyboard_frames(&output_directory, duration, frame_count)?;
        Ok(LibraryStoryboard {
            item_id: item.id,
            duration_seconds: duration,
            frames,
            generated_at: Utc::now(),
        })
    }
}

fn validate_storyboard_source(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        AppError::new(
            "storyboard_source_missing",
            "The video is no longer available. Reconcile the Library and try again.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::new(
            "storyboard_source_invalid",
            "Storyboard previews require a regular local video file.",
        ));
    }
    Ok(())
}

async fn generate_storyboard(
    source: &Path,
    output_directory: &Path,
    duration: f64,
    frame_count: u8,
) -> Result<(), AppError> {
    let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
        AppError::new(
            "ffmpeg_missing",
            "Install FFmpeg in Settings > Engines before generating a storyboard.",
        )
    })?;
    let parent = output_directory.parent().ok_or_else(|| {
        AppError::new(
            "storyboard_cache_unavailable",
            "The storyboard cache is unavailable.",
        )
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::internal("could not create the storyboard cache", error))?;
    let staging = parent.join(format!(".storyboard-{}.part", uuid::Uuid::new_v4()));
    std::fs::create_dir(&staging)
        .map_err(|error| AppError::internal("could not stage storyboard frames", error))?;
    let output_pattern = staging.join("frame-%03d.jpg");
    let frame_rate = f64::from(frame_count) / duration;
    let mut command = Command::new(ffmpeg);
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-i")
        .arg(source)
        .arg("-vf")
        .arg(format!("fps={frame_rate:.8},scale=320:-2:flags=lanczos"))
        .arg("-frames:v")
        .arg(frame_count.to_string())
        .arg("-q:v")
        .arg("4")
        .arg(&output_pattern);
    let result = timeout(
        Duration::from_secs(STORYBOARD_TIMEOUT_SECONDS),
        command.output(),
    )
    .await
    .map_err(|_| {
        let _ = std::fs::remove_dir_all(&staging);
        AppError::new(
            "storyboard_generation_timeout",
            "Generating this storyboard took too long. The original video is unchanged.",
        )
    })?
    .map_err(|error| {
        AppError::internal("could not start FFmpeg for storyboard generation", error)
    })?;
    if !result.status.success() || !storyboard_cache_complete(&staging, frame_count) {
        let _ = std::fs::remove_dir_all(&staging);
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        return Err(AppError::with_details(
            "storyboard_generation_failed",
            "FFmpeg could not generate a storyboard for this video.",
            diagnostic.chars().take(2_000).collect::<String>(),
        ));
    }
    if output_directory.exists() {
        std::fs::remove_dir_all(output_directory).map_err(|error| {
            AppError::internal("could not replace the cached storyboard", error)
        })?;
    }
    std::fs::rename(&staging, output_directory)
        .map_err(|error| AppError::internal("could not finalize the storyboard cache", error))?;
    Ok(())
}

fn storyboard_cache_complete(directory: &Path, frame_count: u8) -> bool {
    (1..=frame_count).all(|index| {
        let path = directory.join(format!("frame-{index:03}.jpg"));
        path.metadata().is_ok_and(|metadata| {
            metadata.is_file() && metadata.len() > 0 && metadata.len() <= MAX_FRAME_BYTES
        })
    })
}

fn read_storyboard_frames(
    directory: &Path,
    duration: f64,
    frame_count: u8,
) -> Result<Vec<LibraryStoryboardFrame>, AppError> {
    let mut frames = Vec::with_capacity(usize::from(frame_count));
    for index in 0..frame_count {
        let path = directory.join(format!("frame-{:03}.jpg", index + 1));
        let metadata = path
            .metadata()
            .map_err(|error| AppError::internal("could not inspect a storyboard frame", error))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_FRAME_BYTES {
            return Err(AppError::new(
                "storyboard_cache_invalid",
                "A cached storyboard frame was invalid. Generate the storyboard again.",
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|error| AppError::internal("could not read a storyboard frame", error))?;
        frames.push(LibraryStoryboardFrame {
            time_seconds: duration * f64::from(index) / f64::from(frame_count),
            image_data_url: format!("data:image/jpeg;base64,{}", STANDARD.encode(bytes)),
        });
    }
    Ok(frames)
}

fn cleanup_old_storyboards(root: &Path, item_id: &str, keep: &Path) {
    let prefix = format!("{item_id}-");
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path == keep {
            continue;
        }
        if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(&prefix))
        {
            if let Err(error) = std::fs::remove_dir_all(&path) {
                tracing::warn!(path = %path.display(), %error, "could not remove an old storyboard cache entry");
            }
        }
    }
}
