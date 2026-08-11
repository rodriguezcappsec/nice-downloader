use crate::{
    errors::AppError,
    models::{
        DownloadEngineKind, EnginePreferences, MediaAnalysis, OutputConflictPolicy, QualityKind,
        QualityOption, StartDownloadRequest,
    },
    utils::validation::validate_media_url,
};
use std::path::PathBuf;
use url::Url;

const DIRECT_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "mp3", "m4a", "aac", "ogg", "opus", "flac", "wav",
    "zip", "7z", "tar", "gz", "jpg", "jpeg", "png", "gif", "webp",
];

#[derive(Debug, Clone)]
pub struct DirectHttpDownload {
    pub executable: PathBuf,
    pub engine_name: &'static str,
    pub args: Vec<String>,
    pub filename: String,
}

pub fn is_direct_download_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    url.path_segments()
        .and_then(Iterator::last)
        .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
        .is_some_and(|extension| {
            DIRECT_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

pub async fn analyze_direct(url: &str) -> Result<MediaAnalysis, AppError> {
    validate_media_url(url)?;
    let parsed = Url::parse(url)
        .map_err(|error| AppError::internal("could not parse the direct download URL", error))?;
    let filename = safe_filename(
        parsed
            .path_segments()
            .and_then(Iterator::last)
            .unwrap_or("download"),
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|error| AppError::internal("could not initialize HTTP analysis", error))?;
    let size = match client.head(parsed.clone()).send().await {
        Ok(response) if response.status().is_success() => response.content_length(),
        _ => None,
    };
    let host = parsed.host_str().unwrap_or("Direct HTTP").to_owned();
    Ok(MediaAnalysis {
        engine: DownloadEngineKind::DirectHttp,
        title: filename.clone(),
        uploader: None,
        duration_seconds: None,
        thumbnail: None,
        webpage_url: parsed.to_string(),
        download_url: parsed.to_string(),
        extractor: "direct-http".to_owned(),
        site_name: host,
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original file".to_owned(),
            description: "No conversion".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count: Some(1),
        total_size_bytes: size,
    })
}

pub fn prepare_direct(
    request: &StartDownloadRequest,
    preferences: &EnginePreferences,
) -> Result<DirectHttpDownload, AppError> {
    validate_media_url(&request.url)?;
    let parsed = Url::parse(&request.url)
        .map_err(|error| AppError::internal("could not parse the direct download URL", error))?;
    let filename = safe_filename(
        parsed
            .path_segments()
            .and_then(Iterator::last)
            .unwrap_or("download"),
    );
    let output_path = std::path::Path::new(&request.options.destination).join(&filename);
    if output_path.exists() && request.options.output_conflict_policy == OutputConflictPolicy::Skip
    {
        return Err(AppError::new(
            "output_exists",
            "The direct-download output already exists. Choose overwrite or a different destination.",
        ));
    }
    if let Ok(executable) = which::which("aria2c") {
        let mut args = vec![
            "--continue=true".to_owned(),
            "--max-tries=12".to_owned(),
            "--retry-wait=3".to_owned(),
            "--connect-timeout=20".to_owned(),
            "--timeout=60".to_owned(),
            "--file-allocation=none".to_owned(),
            "--auto-file-renaming=false".to_owned(),
            "--allow-overwrite=true".to_owned(),
            "--console-log-level=warn".to_owned(),
            "--summary-interval=1".to_owned(),
            format!("--out={filename}"),
        ];
        if request.options.output_conflict_policy == OutputConflictPolicy::Skip {
            args.push("--conditional-get=true".to_owned());
        }
        if let Some(proxy) = &preferences.proxy_url {
            args.push(format!("--all-proxy={proxy}"));
        }
        if let Some(limit) = preferences.bandwidth_limit_kib {
            args.push(format!("--max-download-limit={limit}K"));
        }
        args.push("--".to_owned());
        args.push(parsed.to_string());
        return Ok(DirectHttpDownload {
            executable,
            engine_name: "aria2c",
            args,
            filename,
        });
    }

    let executable = which::which("curl").map_err(|_| {
        AppError::new(
            "transfer_engine_not_found",
            "A direct download requires aria2c or curl, but neither executable was found in PATH.",
        )
    })?;
    let mut args = vec![
        "--location".to_owned(),
        "--fail".to_owned(),
        "--continue-at".to_owned(),
        "-".to_owned(),
        "--retry".to_owned(),
        "12".to_owned(),
        "--retry-all-errors".to_owned(),
        "--retry-delay".to_owned(),
        "3".to_owned(),
        "--connect-timeout".to_owned(),
        "20".to_owned(),
        "--output".to_owned(),
        filename.clone(),
    ];
    if let Some(proxy) = &preferences.proxy_url {
        args.extend(["--proxy".to_owned(), proxy.clone()]);
    }
    if let Some(limit) = preferences.bandwidth_limit_kib {
        args.extend(["--limit-rate".to_owned(), format!("{limit}K")]);
    }
    args.push("--".to_owned());
    args.push(parsed.to_string());
    Ok(DirectHttpDownload {
        executable,
        engine_name: "curl",
        args,
        filename,
    })
}

fn safe_filename(value: &str) -> String {
    let decoded = value.replace(['/', '\\'], "_");
    let cleaned = decoded
        .chars()
        .filter(|character| !character.is_control())
        .take(180)
        .collect::<String>();
    if cleaned.is_empty() || matches!(cleaned.as_str(), "." | "..") {
        "download.bin".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_only_known_direct_file_extensions() {
        assert!(is_direct_download_url(
            "https://cdn.example/video.mp4?token=secret"
        ));
        assert!(!is_direct_download_url("https://example.com/watch?id=one"));
        assert!(!is_direct_download_url("file:///tmp/video.mp4"));
    }

    #[test]
    fn removes_path_separators_from_output_names() {
        assert_eq!(safe_filename("../video.mp4"), ".._video.mp4");
    }
}
