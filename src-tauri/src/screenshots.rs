use crate::{database::Database, engines::managed, errors::AppError, models::LibraryMediaKind};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{
    process::Command,
    time::{timeout, Duration},
};
use uuid::Uuid;

const SCREENSHOT_TIMEOUT: Duration = Duration::from_secs(45);

pub async fn capture_library_frame(
    database: Database,
    item_id: String,
    timestamp_seconds: f64,
    output_path: String,
) -> Result<String, AppError> {
    if !timestamp_seconds.is_finite() || !(0.0..=86_400.0).contains(&timestamp_seconds) {
        return Err(AppError::new(
            "invalid_screenshot_timestamp",
            "Choose a valid position within the video.",
        ));
    }
    let item = database.get_library_item(&item_id)?;
    if item.missing
        || !matches!(
            item.media_kind,
            Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
        )
    {
        return Err(AppError::new(
            "screenshot_source_unavailable",
            "Frame capture requires an available Library video.",
        ));
    }
    if item
        .duration_seconds
        .is_some_and(|duration| timestamp_seconds > duration + 1.0)
    {
        return Err(AppError::new(
            "invalid_screenshot_timestamp",
            "The selected frame is beyond the end of this video.",
        ));
    }
    let source = Path::new(&item.path);
    validate_regular_file(
        source,
        "screenshot_source_unavailable",
        "Frame capture requires a regular local video file.",
    )?;
    let output = validate_output_path(&output_path)?;
    let parent = output
        .parent()
        .ok_or_else(|| invalid_output("Choose a valid screenshot destination."))?;
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    let staged = parent.join(format!(
        ".nice-downloader-frame-{}.part.{extension}",
        Uuid::new_v4()
    ));
    let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
        AppError::new(
            "ffmpeg_unavailable",
            "Install FFmpeg in Settings to capture video frames.",
        )
    })?;
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-i")
        .arg(source)
        .arg("-ss")
        .arg(format!("{timestamp_seconds:.3}"))
        .arg("-map")
        .arg("0:v:0")
        .arg("-frames:v")
        .arg("1")
        .arg("-an")
        .arg("-sn")
        .arg("-y")
        .arg(&staged)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let result = timeout(SCREENSHOT_TIMEOUT, command.output())
        .await
        .map_err(|_| {
            let _ = std::fs::remove_file(&staged);
            AppError::new("screenshot_timeout", "Capturing this frame took too long.")
        })?
        .map_err(|error| AppError::internal("could not start FFmpeg frame capture", error))?;
    if !result.status.success() || !staged.is_file() {
        let _ = std::fs::remove_file(&staged);
        return Err(AppError::with_details(
            "screenshot_failed",
            "FFmpeg could not capture the selected frame.",
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(2_000)
                .collect::<String>(),
        ));
    }
    publish_staged_frame(&staged, &output)?;
    Ok(output.to_string_lossy().into_owned())
}

fn validate_output_path(value: &str) -> Result<PathBuf, AppError> {
    let output = PathBuf::from(value);
    if !output.is_absolute() {
        return Err(invalid_output("Choose an absolute screenshot destination."));
    }
    let extension = output
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg")) {
        return Err(invalid_output("Save the captured frame as PNG or JPEG."));
    }
    let parent = output
        .parent()
        .ok_or_else(|| invalid_output("Choose a valid screenshot destination."))?;
    let metadata = parent
        .symlink_metadata()
        .map_err(|_| invalid_output("The screenshot destination folder is unavailable."))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(invalid_output(
            "Choose a regular, non-symlink destination folder.",
        ));
    }
    if let Ok(metadata) = output.symlink_metadata() {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(invalid_output(
                "The screenshot destination must be a regular file.",
            ));
        }
    }
    Ok(output)
}

fn publish_staged_frame(staged: &Path, output: &Path) -> Result<(), AppError> {
    if !output.exists() {
        return std::fs::rename(staged, output)
            .map_err(|error| AppError::internal("could not publish captured frame", error));
    }
    let parent = output
        .parent()
        .ok_or_else(|| invalid_output("Choose a valid screenshot destination."))?;
    let backup = parent.join(format!(".nice-downloader-frame-{}.backup", Uuid::new_v4()));
    std::fs::rename(output, &backup)
        .map_err(|error| AppError::internal("could not replace existing screenshot", error))?;
    if let Err(error) = std::fs::rename(staged, output) {
        let _ = std::fs::rename(&backup, output);
        let _ = std::fs::remove_file(staged);
        return Err(AppError::internal(
            "could not publish captured frame",
            error,
        ));
    }
    let _ = std::fs::remove_file(backup);
    Ok(())
}

fn validate_regular_file(path: &Path, code: &str, message: &str) -> Result<(), AppError> {
    let metadata = path
        .symlink_metadata()
        .map_err(|_| AppError::new(code, message))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(code, message));
    }
    Ok(())
}

fn invalid_output(message: &str) -> AppError {
    AppError::new("invalid_screenshot_output", message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn screenshot_outputs_are_bounded_to_regular_png_or_jpeg_paths() {
        let directory = tempdir().expect("temporary directory");
        assert!(
            validate_output_path(&directory.path().join("frame.png").to_string_lossy()).is_ok()
        );
        assert!(
            validate_output_path(&directory.path().join("frame.jpeg").to_string_lossy()).is_ok()
        );
        assert_eq!(
            validate_output_path(&directory.path().join("frame.txt").to_string_lossy())
                .expect_err("unsupported extension")
                .code,
            "invalid_screenshot_output"
        );
    }
}
