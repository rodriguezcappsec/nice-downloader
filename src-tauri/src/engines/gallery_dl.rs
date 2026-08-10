use crate::{
    engines::ytdlp::sanitize_diagnostic,
    errors::AppError,
    models::{
        DownloadEngineKind, EnginePreferences, MediaAnalysis, OutputConflictPolicy, QualityKind,
        QualityOption, StartDownloadRequest,
    },
    utils::validation::validate_media_url,
};
use serde_json::Value;
use std::{path::PathBuf, time::Duration};
use tokio::{process::Command, time::timeout};
use url::Url;

const MAX_ANALYSIS_BYTES: usize = 1_000_000;

#[derive(Debug, Clone)]
pub struct GalleryDlEngine {
    executable: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GalleryDownload {
    pub args: Vec<String>,
    pub output_directory: String,
}

impl GalleryDlEngine {
    pub fn discover() -> Result<Self, AppError> {
        let executable = which::which("gallery-dl").map_err(|_| {
            AppError::new(
                "engine_not_found",
                "gallery-dl is required for Bunkr albums but was not found in PATH. Install gallery-dl, then use Settings > Engines to detect it.",
            )
        })?;
        Ok(Self { executable })
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    pub async fn analyze(
        &self,
        url: &str,
        preferences: &EnginePreferences,
    ) -> Result<MediaAnalysis, AppError> {
        let validated = validate_bunkr_album_url(url)?;
        let mut command = Command::new(&self.executable);
        command.kill_on_drop(true).args([
            "--config-ignore",
            "--no-colors",
            "--no-input",
            "--range",
            "1",
            "--dump-json",
        ]);
        command.args(access_args(preferences));
        command.arg("--").arg(validated.as_str());

        tracing::info!(
            engine = "gallery-dl",
            operation = "analyze",
            site = "bunkr",
            "starting engine operation"
        );
        let output = timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| {
                AppError::new(
                    "analysis_timeout",
                    "Bunkr album analysis timed out after 60 seconds.",
                )
            })?
            .map_err(|error| AppError::internal("could not start gallery-dl", error))?;

        if !output.status.success() {
            let details = sanitize_diagnostic(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::with_details(
                "analysis_failed",
                friendly_error(&details),
                details,
            ));
        }
        if output.stdout.len() > MAX_ANALYSIS_BYTES {
            return Err(AppError::new(
                "analysis_too_large",
                "gallery-dl returned more album metadata than the application can safely process.",
            ));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| AppError::internal("gallery-dl returned invalid JSON", error))?;
        parse_album_metadata(value, validated.as_str())
    }

    pub fn build_download(
        &self,
        request: &StartDownloadRequest,
        preferences: &EnginePreferences,
    ) -> Result<GalleryDownload, AppError> {
        let validated = validate_bunkr_album_url(&request.url)?;
        let album_id = bunkr_album_id(&validated).ok_or_else(|| {
            AppError::new(
                "unsupported_gallery_url",
                "This is not a supported Bunkr album URL.",
            )
        })?;
        let output_directory = safe_album_directory(&request.analysis.title, album_id);
        let mut args = vec![
            "--config-ignore".to_owned(),
            "--no-colors".to_owned(),
            "--no-input".to_owned(),
            "--restrict-filenames".to_owned(),
            "unix".to_owned(),
            "--directory".to_owned(),
            output_directory.clone(),
            "--Print".to_owned(),
            "prepare:nicedl:gallery:item:{num}|{count}|{size}|{filename}.{extension}".to_owned(),
            "--Print".to_owned(),
            "after:nicedl:gallery:complete:{num}|{count}|{size}|{_path}".to_owned(),
            "--Print".to_owned(),
            "skip:nicedl:gallery:skip:{num}|{count}|{size}|{_path}".to_owned(),
            "--option".to_owned(),
            r#"output.mode={"start":"nicedl:gallery:output:{}\n","skip":"nicedl:gallery:output:{}\n","success":"nicedl:gallery:output:{}\n","progress":"\nnicedl:gallery:progress:NA|{0}|{1}|NA","progress-total":"\nnicedl:gallery:progress:{3}|{0}|{1}|{2}"}"#.to_owned(),
            "--option".to_owned(),
            "output.shorten=false".to_owned(),
        ];
        if request.options.output_conflict_policy == OutputConflictPolicy::Overwrite {
            args.push("--no-skip".to_owned());
        }
        args.extend(access_args(preferences));
        args.push("--".to_owned());
        args.push(validated.into());
        Ok(GalleryDownload {
            args,
            output_directory,
        })
    }
}

pub fn is_bunkr_album_url(value: &str) -> bool {
    validate_media_url(value)
        .ok()
        .is_some_and(|url| bunkr_album_id(&url).is_some())
}

fn validate_bunkr_album_url(value: &str) -> Result<Url, AppError> {
    let url = validate_media_url(value)?;
    if bunkr_album_id(&url).is_none() {
        return Err(AppError::new(
            "unsupported_gallery_url",
            "Only Bunkr album URLs in the form https://bunkr.cr/a/album-id are supported by this gallery adapter.",
        ));
    }
    Ok(url)
}

fn bunkr_album_id(url: &Url) -> Option<&str> {
    let host = url.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    if host != "bunkr.cr" && !host.ends_with(".bunkr.cr") {
        return None;
    }
    let segments = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 2 || segments[0] != "a" {
        return None;
    }
    let id = segments[1];
    (!id.is_empty()
        && id.len() <= 80
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_')))
    .then_some(id)
}

fn parse_album_metadata(value: Value, requested_url: &str) -> Result<MediaAnalysis, AppError> {
    let events = value.as_array().ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not return structured Bunkr album metadata.",
        )
    })?;
    let mut album = None;
    let mut first_item = None;
    for event in events {
        let Some(parts) = event.as_array() else {
            continue;
        };
        match parts.first().and_then(Value::as_u64) {
            Some(2) => album = parts.get(1).and_then(Value::as_object),
            Some(3) => first_item = parts.get(2).and_then(Value::as_object),
            _ => {}
        }
    }
    let metadata = album.or(first_item).ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not identify a Bunkr album.",
        )
    })?;
    let title = metadata
        .get("album_name")
        .and_then(Value::as_str)
        .map(clean_title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Bunkr album".to_owned());
    let item_count = metadata
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok());
    let total_size_bytes = metadata
        .get("album_size")
        .and_then(Value::as_str)
        .and_then(parse_human_bytes);
    let thumbnail = first_item
        .and_then(|item| item.get("thumbnail"))
        .and_then(Value::as_str)
        .filter(|url| validate_media_url(url).is_ok())
        .map(str::to_owned);

    Ok(MediaAnalysis {
        engine: DownloadEngineKind::GalleryDl,
        title,
        uploader: None,
        duration_seconds: None,
        thumbnail,
        webpage_url: requested_url.to_owned(),
        download_url: requested_url.to_owned(),
        extractor: "gallery-dl:bunkr".to_owned(),
        site_name: "Bunkr".to_owned(),
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original files".to_owned(),
            description: "Preserve every file in the album".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count,
        total_size_bytes,
    })
}

pub(crate) fn parse_human_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    let number_end = value
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_digit() && character != '.').then_some(index)
        })
        .unwrap_or(value.len());
    let amount = value[..number_end].trim().parse::<f64>().ok()?;
    let unit = value[number_end..]
        .trim()
        .trim_end_matches("/s")
        .to_ascii_uppercase();
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let multiplier = match unit.as_str() {
        "" | "B" => 1.0,
        "KB" | "K" => 1_000.0,
        "MB" | "M" => 1_000_000.0,
        "GB" | "G" => 1_000_000_000.0,
        "TB" | "T" => 1_000_000_000_000.0,
        "KIB" => 1_024.0,
        "MIB" => 1_048_576.0,
        "GIB" => 1_073_741_824.0,
        _ => return None,
    };
    let bytes = amount * multiplier;
    (bytes <= u64::MAX as f64).then_some(bytes.round() as u64)
}

fn safe_album_directory(title: &str, album_id: &str) -> String {
    let mut clean = title
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(120)
        .collect::<String>();
    clean = clean.trim().trim_matches('.').to_owned();
    if clean.is_empty() {
        clean = "Bunkr album".to_owned();
    }
    format!("{clean} [{album_id}]")
}

fn clean_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(200)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn access_args(preferences: &EnginePreferences) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(browser) = preferences.browser_cookie_source.yt_dlp_name() {
        let browser = preferences
            .browser_profile
            .as_deref()
            .map(|profile| format!("{browser}:{profile}"))
            .unwrap_or_else(|| browser.to_owned());
        args.extend(["--cookies-from-browser".to_owned(), browser]);
    }
    if let Some(proxy) = &preferences.proxy_url {
        args.extend(["--proxy".to_owned(), proxy.clone()]);
    }
    args
}

fn friendly_error(details: &str) -> String {
    let lowercase = details.to_ascii_lowercase();
    if lowercase.contains("cookies") || lowercase.contains("login") {
        "Bunkr requires a browser session. Choose a browser cookie source in Settings, then try again.".to_owned()
    } else if lowercase.contains("403") || lowercase.contains("429") {
        "Bunkr blocked or rate-limited the request. Try again later or configure a proxy in Settings.".to_owned()
    } else {
        "gallery-dl could not analyze this Bunkr album.".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AudioQuality, DownloadOptions, OutputContainer, VideoCodecPreference};
    use serde_json::json;

    #[test]
    fn routes_only_exact_bunkr_album_urls() {
        assert!(is_bunkr_album_url("https://bunkr.cr/a/ul3n82nu"));
        assert!(is_bunkr_album_url("https://app.bunkr.cr/a/album_1/"));
        assert!(!is_bunkr_album_url("https://bunkr.cr.example/a/ul3n82nu"));
        assert!(!is_bunkr_album_url("https://bunkr.cr/v/ul3n82nu"));
        assert!(!is_bunkr_album_url("https://bunkr.cr/a/one/extra"));
    }

    #[test]
    fn parses_bounded_album_summary_without_preserving_media_url() {
        let signed = "https://cdn.example/file.mkv?token=secret";
        let analysis = parse_album_metadata(
            json!([
                [2, {"album_id":"ul3n82nu","album_name":"Album","album_size":"12.59 GB","count":67}],
                [3, signed, {"thumbnail":"https://i.example/thumb.png","size":215914671}]
            ]),
            "https://bunkr.cr/a/ul3n82nu",
        )
        .unwrap();
        assert_eq!(analysis.engine, DownloadEngineKind::GalleryDl);
        assert_eq!(analysis.item_count, Some(67));
        assert_eq!(analysis.total_size_bytes, Some(12_590_000_000));
        assert_eq!(analysis.download_url, "https://bunkr.cr/a/ul3n82nu");
        assert!(!serde_json::to_string(&analysis).unwrap().contains("secret"));
    }

    #[test]
    fn builds_safe_gallery_arguments() {
        let engine = GalleryDlEngine {
            executable: PathBuf::from("gallery-dl"),
        };
        let analysis = parse_album_metadata(
            json!([[2, {"album_id":"safe","album_name":"../Album","count":1}]]),
            "https://bunkr.cr/a/safe",
        )
        .unwrap();
        let request = StartDownloadRequest {
            url: analysis.download_url.clone(),
            analysis,
            options: DownloadOptions {
                quality: QualityOption {
                    id: "original".into(),
                    label: "Original files".into(),
                    description: String::new(),
                    kind: QualityKind::Best,
                    max_height: None,
                },
                container: OutputContainer::Mp4,
                video_codec_preference: VideoCodecPreference::Any,
                audio_quality: AudioQuality::Best,
                audio_only: false,
                subtitles: false,
                subtitle_languages: vec![],
                embed_thumbnail: false,
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                destination: "/tmp".into(),
                filename_template: "ignored".into(),
                output_conflict_policy: OutputConflictPolicy::Skip,
            },
        };
        let prepared = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap();
        assert_eq!(prepared.output_directory, "_Album [safe]");
        assert_eq!(prepared.args.last().unwrap(), "https://bunkr.cr/a/safe");
        assert!(!prepared.args.iter().any(|arg| arg == "--no-skip"));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg.starts_with("output.mode={") && arg.contains("progress-total")));
    }

    #[tokio::test]
    #[ignore = "requires gallery-dl and public Bunkr network access"]
    async fn analyzes_public_bunkr_album() {
        let analysis = GalleryDlEngine::discover()
            .unwrap()
            .analyze("https://bunkr.cr/a/ul3n82nu", &EnginePreferences::default())
            .await
            .unwrap();
        assert_eq!(analysis.engine, DownloadEngineKind::GalleryDl);
        assert!(analysis.item_count.is_some_and(|count| count > 1));
        assert!(analysis.total_size_bytes.is_some());
    }

    #[tokio::test]
    #[ignore = "requires gallery-dl and public Bunkr network access"]
    async fn validates_public_bunkr_download_templates_without_downloading() {
        let engine = GalleryDlEngine::discover().unwrap();
        let analysis = engine
            .analyze("https://bunkr.cr/a/ul3n82nu", &EnginePreferences::default())
            .await
            .unwrap();
        let request = StartDownloadRequest {
            url: analysis.download_url.clone(),
            options: DownloadOptions {
                quality: analysis.quality_options[0].clone(),
                container: OutputContainer::Mp4,
                video_codec_preference: VideoCodecPreference::Any,
                audio_quality: AudioQuality::Best,
                audio_only: false,
                subtitles: false,
                subtitle_languages: vec![],
                embed_thumbnail: false,
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                destination: "/tmp".into(),
                filename_template: "ignored".into(),
                output_conflict_policy: OutputConflictPolicy::Skip,
            },
            analysis,
        };
        let mut prepared = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap();
        let separator = prepared.args.iter().position(|arg| arg == "--").unwrap();
        prepared.args.splice(
            separator..separator,
            [
                "--no-download".to_owned(),
                "--range".to_owned(),
                "1".to_owned(),
            ],
        );
        let output = Command::new(engine.executable())
            .args(prepared.args)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("nicedl:gallery:item:1|"));
    }
}
