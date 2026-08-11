mod metadata;
mod url_resolver;

use crate::{
    engines::DownloadEngine,
    errors::AppError,
    models::{
        DownloadOptions, EnginePreferences, FormatStrategy, MediaAnalysis, OutputConflictPolicy,
        QualityKind, VideoCodecPreference,
    },
    utils::validation::{validate_filename_template, validate_media_url},
};
use std::collections::HashSet;
use std::{path::PathBuf, time::Duration};
use tokio::{process::Command, time::timeout};

pub use metadata::parse_metadata;

#[derive(Debug, Clone)]
pub struct YtDlpEngine {
    executable: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct YtDlpRuntimeCapabilities {
    pub preferred_impersonation_target: Option<String>,
}

impl YtDlpRuntimeCapabilities {
    pub fn impersonation_available(&self) -> bool {
        self.preferred_impersonation_target.is_some()
    }
}

impl YtDlpEngine {
    pub fn discover() -> Result<Self, AppError> {
        let executable = crate::engines::managed::resolve("yt-dlp").ok_or_else(|| {
            AppError::new(
                "engine_not_found",
                "yt-dlp is not installed or could not be found in PATH.",
            )
        })?;
        Ok(Self { executable })
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    pub async fn runtime_capabilities(&self) -> YtDlpRuntimeCapabilities {
        detect_runtime_capabilities(&self.executable).await
    }

    pub async fn analyze(
        &self,
        url: &str,
        preferences: &EnginePreferences,
    ) -> Result<MediaAnalysis, AppError> {
        let validated = url_resolver::resolve_for_analysis(validate_media_url(url)?).await?;
        let capabilities = self.runtime_capabilities().await;
        let initial_result = self
            .run_structured_analysis(validated.as_str(), preferences, &capabilities)
            .await;

        match initial_result {
            Ok(value) => parse_metadata(value, validated.as_str()),
            Err(initial_error) if is_reddit_api_ip_error(initial_error.details.as_deref()) => {
                self.analyze_reddit_embed_fallback(
                    &validated,
                    preferences,
                    &capabilities,
                    initial_error,
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    pub async fn expand_playlist(
        &self,
        url: &str,
        preferences: &EnginePreferences,
    ) -> Result<(Option<String>, Vec<String>), AppError> {
        let validated = validate_media_url(url)?;
        let capabilities = self.runtime_capabilities().await;
        let mut command = Command::new(&self.executable);
        command.kill_on_drop(true).args([
            "--ignore-config",
            "--yes-playlist",
            "--flat-playlist",
            "--playlist-end",
            "20",
            "--dump-single-json",
        ]);
        command.args(access_args(preferences, &capabilities));
        command.arg("--").arg(validated.as_str());
        let output = timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| {
                AppError::new(
                    "playlist_analysis_timeout",
                    "Playlist inspection timed out after 60 seconds.",
                )
            })?
            .map_err(|error| AppError::internal("could not start yt-dlp", error))?;
        if !output.status.success() {
            let details = sanitize_diagnostic(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::with_details(
                "playlist_analysis_failed",
                friendly_engine_error(&details),
                details,
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| AppError::internal("yt-dlp returned invalid playlist JSON", error))?;
        parse_playlist_expansion(value)
    }

    async fn run_structured_analysis(
        &self,
        url: &str,
        preferences: &EnginePreferences,
        capabilities: &YtDlpRuntimeCapabilities,
    ) -> Result<serde_json::Value, AppError> {
        let mut command = Command::new(&self.executable);
        command.kill_on_drop(true).args([
            "--ignore-config",
            "--no-playlist",
            "--skip-download",
            "--dump-single-json",
        ]);
        command.args(access_args(preferences, capabilities));
        command.arg("--").arg(url);

        tracing::info!(
            engine = "yt-dlp",
            operation = "analyze",
            "starting engine operation"
        );
        let output = timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| {
                AppError::new(
                    "analysis_timeout",
                    "Media analysis timed out after 60 seconds.",
                )
            })?
            .map_err(|error| AppError::internal("could not start yt-dlp", error))?;

        if !output.status.success() {
            let details = sanitize_diagnostic(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::with_details(
                "analysis_failed",
                friendly_engine_error(&details),
                details,
            ));
        }

        let value: serde_json::Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| AppError::internal("yt-dlp returned invalid JSON", error))?;
        Ok(value)
    }

    async fn analyze_reddit_embed_fallback(
        &self,
        reddit_url: &url::Url,
        preferences: &EnginePreferences,
        capabilities: &YtDlpRuntimeCapabilities,
        initial_error: AppError,
    ) -> Result<MediaAnalysis, AppError> {
        let fallback = match url_resolver::resolve_reddit_embed_media(reddit_url).await {
            Ok(Some(fallback)) => fallback,
            Ok(None) => return Err(initial_error),
            Err(fallback_error) => {
                return Err(with_fallback_details(initial_error, &fallback_error));
            }
        };

        tracing::info!(
            engine = "yt-dlp",
            operation = "analyze",
            fallback = "reddit_embed",
            "Reddit API was unavailable; analyzing the public embed media stream"
        );
        let value = self
            .run_structured_analysis(fallback.download_url.as_str(), preferences, capabilities)
            .await
            .map_err(|error| {
                let details = error
                    .details
                    .as_deref()
                    .unwrap_or("yt-dlp did not provide a direct-stream diagnostic");
                AppError::with_details(
                    "reddit_fallback_analysis_failed",
                    "Reddit exposed the video stream, but yt-dlp could not inspect it.",
                    details,
                )
            })?;
        let mut analysis = parse_metadata(value, fallback.download_url.as_str())?;
        analysis.title = fallback.title;
        analysis.uploader = fallback.uploader;
        analysis.thumbnail = fallback.thumbnail;
        analysis.webpage_url = fallback.webpage_url.to_string();
        analysis.download_url = fallback.download_url.to_string();
        analysis.extractor = "reddit-embed-fallback".to_owned();
        analysis.site_name = "Reddit".to_owned();
        Ok(analysis)
    }
}

fn parse_playlist_expansion(
    value: serde_json::Value,
) -> Result<(Option<String>, Vec<String>), AppError> {
    let title = value
        .get("title")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    let entries = value
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            AppError::new(
                "not_a_playlist",
                "yt-dlp did not identify this URL as a playlist or collection.",
            )
        })?;
    let mut seen = HashSet::new();
    let urls = entries
        .iter()
        .filter_map(|entry| {
            ["webpage_url", "url"]
                .into_iter()
                .filter_map(|key| entry.get(key).and_then(serde_json::Value::as_str))
                .find(|candidate| validate_media_url(candidate).is_ok())
                .map(str::to_owned)
        })
        .filter(|url| seen.insert(url.clone()))
        .take(20)
        .collect::<Vec<_>>();
    if urls.is_empty() {
        return Err(AppError::new(
            "empty_playlist",
            "No downloadable entries were found in this playlist.",
        ));
    }
    Ok((title, urls))
}

impl DownloadEngine for YtDlpEngine {
    fn executable_name(&self) -> &'static str {
        "yt-dlp"
    }

    fn build_download_args(
        &self,
        url: &str,
        options: &DownloadOptions,
        metadata_title_override: Option<&str>,
        preferences: &EnginePreferences,
        capabilities: &YtDlpRuntimeCapabilities,
    ) -> Result<Vec<String>, AppError> {
        let validated_url = validate_media_url(url)?;
        let filename_template = validate_filename_template(&options.filename_template)?;
        if options.audio_only != options.quality.kind.eq(&QualityKind::AudioOnly) {
            return Err(AppError::new(
                "invalid_download_options",
                "The selected quality and audio-only option do not match.",
            ));
        }
        if options.audio_only != options.container.is_audio() {
            return Err(AppError::new(
                "invalid_download_options",
                "Choose an audio container for audio-only downloads and a video container otherwise.",
            ));
        }

        let selector = format_selector(options)?;
        let mut args = vec![
            "--ignore-config".to_owned(),
            "--no-playlist".to_owned(),
            "--newline".to_owned(),
            "--no-color".to_owned(),
            "--progress".to_owned(),
            "--progress-template".to_owned(),
            "download:nicedl:progress:%(progress._percent_str)s|%(progress.downloaded_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s".to_owned(),
            "--progress-template".to_owned(),
            "postprocess:nicedl:postprocess:%(progress.status)s".to_owned(),
            "--print".to_owned(),
            "after_move:nicedl:output:%(filepath)s".to_owned(),
            "--format".to_owned(),
            selector,
            "--output".to_owned(),
            filename_template.to_owned(),
        ];

        if let Some(title) = metadata_title_override.filter(|title| !title.trim().is_empty()) {
            args.extend([
                "--replace-in-metadata".to_owned(),
                "title".to_owned(),
                "(?s)^.*$".to_owned(),
                literal_regex_replacement(title),
            ]);
        }

        match options.output_conflict_policy {
            OutputConflictPolicy::Skip => args.push("--no-overwrites".to_owned()),
            OutputConflictPolicy::Overwrite => args.push("--force-overwrites".to_owned()),
        }

        if options.audio_only {
            args.extend([
                "--extract-audio".to_owned(),
                "--audio-format".to_owned(),
                options.container.as_str().to_owned(),
            ]);
            if let Some(quality) = options.audio_quality.yt_dlp_value() {
                args.extend(["--audio-quality".to_owned(), quality.to_owned()]);
            }
        } else {
            args.extend([
                "--merge-output-format".to_owned(),
                options.container.as_str().to_owned(),
            ]);
        }

        if options.embed_metadata {
            args.push("--embed-metadata".to_owned());
        }
        if options.embed_thumbnail {
            args.push("--embed-thumbnail".to_owned());
        }
        if options.chapters {
            args.push("--embed-chapters".to_owned());
        }
        if options.subtitles {
            args.push("--write-subs".to_owned());
            args.push("--embed-subs".to_owned());
            if !options.subtitle_languages.is_empty() {
                args.extend([
                    "--sub-langs".to_owned(),
                    options.subtitle_languages.join(","),
                ]);
            }
        }
        if options.sponsorblock {
            args.extend(["--sponsorblock-remove".to_owned(), "all".to_owned()]);
        }
        match options.format_strategy {
            FormatStrategy::Recommended => {}
            FormatStrategy::Compatibility => args.extend([
                "--format-sort".to_owned(),
                "vcodec:h264,acodec:aac,ext:mp4:m4a,res,fps".to_owned(),
            ]),
            FormatStrategy::Quality => args.extend([
                "--format-sort".to_owned(),
                "res,fps,hdr:12,vcodec,channels,acodec".to_owned(),
            ]),
            FormatStrategy::Smallest => {
                args.extend(["--format-sort".to_owned(), "+size,+br,+res,+fps".to_owned()])
            }
        }
        args.extend(access_args(preferences, capabilities));
        args.push("--".to_owned());
        args.push(validated_url.as_str().to_owned());
        Ok(args)
    }
}

fn literal_regex_replacement(value: &str) -> String {
    value.replace('\\', "\\\\")
}

fn is_reddit_api_ip_error(details: Option<&str>) -> bool {
    details.is_some_and(|details| {
        details
            .to_ascii_lowercase()
            .contains("your ip address is unable to access the reddit api")
    })
}

fn with_fallback_details(mut initial_error: AppError, fallback_error: &str) -> AppError {
    let initial_details = initial_error.details.take().unwrap_or_default();
    initial_error.details = Some(format!(
        "{initial_details}\nReddit embed fallback: {}",
        sanitize_diagnostic(fallback_error)
    ));
    initial_error
}

pub async fn detect_runtime_capabilities(path: &std::path::Path) -> YtDlpRuntimeCapabilities {
    let output = Command::new(path)
        .args(["--ignore-config", "--list-impersonate-targets"])
        .output()
        .await;
    let Ok(output) = output else {
        return YtDlpRuntimeCapabilities::default();
    };
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    YtDlpRuntimeCapabilities {
        preferred_impersonation_target: parse_impersonation_target(&text),
    }
}

fn parse_impersonation_target(value: &str) -> Option<String> {
    value.lines().find_map(|line| {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with('[')
            || line.starts_with('-')
            || line.contains("unavailable")
        {
            return None;
        }
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 3 || columns[0].eq_ignore_ascii_case("client") {
            return None;
        }
        let client = columns[0].to_ascii_lowercase();
        let operating_system = columns[1].to_ascii_lowercase();
        Some(if operating_system == "-" {
            client
        } else {
            format!("{client}:{operating_system}")
        })
    })
}

fn access_args(
    preferences: &EnginePreferences,
    capabilities: &YtDlpRuntimeCapabilities,
) -> Vec<String> {
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
    if preferences.enable_impersonation {
        if let Some(target) = &capabilities.preferred_impersonation_target {
            args.extend(["--impersonate".to_owned(), target.clone()]);
        }
    }
    if let Some(limit) = preferences.bandwidth_limit_kib {
        args.extend(["--limit-rate".to_owned(), format!("{limit}K")]);
    }
    args
}

fn format_selector(options: &DownloadOptions) -> Result<String, AppError> {
    if options.audio_only {
        return Ok("ba/b".to_owned());
    }

    let codec = match options.video_codec_preference {
        VideoCodecPreference::Any => "",
        VideoCodecPreference::H264 => "[vcodec^=avc1]",
        VideoCodecPreference::Av1 => "[vcodec^=av01]",
        VideoCodecPreference::Vp9 => "[vcodec^=vp9]",
    };
    match options.quality.kind {
        QualityKind::Best => Ok(format!("bv*{codec}+ba/b")),
        QualityKind::Resolution => {
            let height = options.quality.max_height.ok_or_else(|| {
                AppError::new(
                    "invalid_download_options",
                    "The selected resolution is incomplete.",
                )
            })?;
            Ok(format!(
                "bv*{codec}[height<={height}]+ba/b[height<={height}]"
            ))
        }
        QualityKind::AudioOnly => Err(AppError::new(
            "invalid_download_options",
            "Audio-only quality requires the audio-only option.",
        )),
    }
}

pub fn sanitize_diagnostic(value: &str) -> String {
    let trimmed = value.trim();
    let start = trimmed.len().saturating_sub(4000);
    trimmed
        .get(start..)
        .unwrap_or(trimmed)
        .lines()
        .map(redact_sensitive_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_sensitive_line(line: &str) -> String {
    let lowercase = line.trim_start().to_ascii_lowercase();
    if lowercase.starts_with("cookie:") || lowercase.starts_with("authorization:") {
        "[sensitive request header redacted]".to_owned()
    } else {
        redact_sensitive_query_values(line)
    }
}

fn redact_sensitive_query_values(line: &str) -> String {
    let mut output = line.to_owned();
    for key in ["token=", "signature=", "authorization=", "auth=", "sig="] {
        let mut search_from = 0;
        loop {
            let lowercase = output.to_ascii_lowercase();
            let Some(relative_start) = lowercase[search_from..].find(key) else {
                break;
            };
            let value_start = search_from + relative_start + key.len();
            let value_end = output[value_start..]
                .char_indices()
                .find_map(|(offset, character)| {
                    (character == '&'
                        || character.is_whitespace()
                        || matches!(character, '\'' | '"' | '<' | '>'))
                    .then_some(value_start + offset)
                })
                .unwrap_or(output.len());
            output.replace_range(value_start..value_end, "[redacted]");
            search_from = value_start + "[redacted]".len();
        }
    }
    output
}

fn friendly_engine_error(details: &str) -> String {
    let lowercase = details.to_lowercase();
    if lowercase.contains("account authentication is required")
        || lowercase.contains("cookies-from-browser")
        || lowercase.contains("sign in")
    {
        "This site requires a signed-in browser session. Choose a browser cookie source in Settings, then try again.".to_owned()
    } else if lowercase.contains("impersonation") || lowercase.contains("http error 403") {
        "The site blocked the request. Enable browser impersonation or browser cookies in Settings, then try again.".to_owned()
    } else if lowercase.contains("unsupported url") {
        "This URL is not supported by the installed version of yt-dlp.".to_owned()
    } else if lowercase.contains("private video") || lowercase.contains("login") {
        "This media is not publicly accessible. Select cookies from a signed-in browser in Settings, then try again.".to_owned()
    } else if lowercase.contains("network") || lowercase.contains("unable to download") {
        "yt-dlp could not reach the media site. Check the network connection and try again."
            .to_owned()
    } else {
        "yt-dlp could not analyze this URL.".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AudioQuality, BrowserCookieSource, OutputContainer, QualityOption};

    fn options(kind: QualityKind, height: Option<u32>, audio_only: bool) -> DownloadOptions {
        DownloadOptions {
            quality: QualityOption {
                id: "test".to_owned(),
                label: "Test".to_owned(),
                description: "Test".to_owned(),
                kind,
                max_height: height,
            },
            container: if audio_only {
                OutputContainer::Mp3
            } else {
                OutputContainer::Mp4
            },
            video_codec_preference: VideoCodecPreference::Any,
            format_strategy: crate::models::FormatStrategy::Recommended,
            audio_quality: AudioQuality::Best,
            audio_only,
            subtitles: false,
            subtitle_languages: vec![],
            embed_thumbnail: false,
            embed_metadata: true,
            chapters: false,
            sponsorblock: false,
            destination: "/tmp".to_owned(),
            filename_template: "%(title)s.%(ext)s".to_owned(),
            output_conflict_policy: OutputConflictPolicy::Skip,
            gallery_selection: None,
        }
    }

    #[test]
    fn builds_resolution_selector() {
        assert_eq!(
            format_selector(&options(QualityKind::Resolution, Some(1080), false)).unwrap(),
            "bv*[height<=1080]+ba/b[height<=1080]"
        );
    }

    #[test]
    fn builds_audio_selector() {
        assert_eq!(
            format_selector(&options(QualityKind::AudioOnly, None, true)).unwrap(),
            "ba/b"
        );
    }

    #[test]
    fn maps_output_conflict_intent_to_safe_engine_arguments() {
        let engine = YtDlpEngine {
            executable: PathBuf::from("yt-dlp"),
        };
        let mut download_options = options(QualityKind::Best, None, false);
        let skip_args = engine
            .build_download_args(
                "https://example.com/video",
                &download_options,
                None,
                &EnginePreferences::default(),
                &YtDlpRuntimeCapabilities::default(),
            )
            .unwrap();
        assert!(skip_args
            .iter()
            .any(|argument| argument == "--no-overwrites"));

        download_options.output_conflict_policy = OutputConflictPolicy::Overwrite;
        let overwrite_args = engine
            .build_download_args(
                "https://example.com/video",
                &download_options,
                None,
                &EnginePreferences::default(),
                &YtDlpRuntimeCapabilities::default(),
            )
            .unwrap();
        assert!(overwrite_args
            .iter()
            .any(|argument| argument == "--force-overwrites"));
    }

    #[test]
    fn builds_typed_browser_access_arguments() {
        let preferences = EnginePreferences {
            browser_cookie_source: BrowserCookieSource::Firefox,
            browser_profile: Some("work".to_owned()),
            proxy_url: Some("socks5://127.0.0.1:1080".to_owned()),
            enable_impersonation: true,
            bandwidth_limit_kib: None,
        };
        let capabilities = YtDlpRuntimeCapabilities {
            preferred_impersonation_target: Some("chrome-136:windows-10".to_owned()),
        };
        assert_eq!(
            access_args(&preferences, &capabilities),
            [
                "--cookies-from-browser",
                "firefox:work",
                "--proxy",
                "socks5://127.0.0.1:1080",
                "--impersonate",
                "chrome-136:windows-10"
            ]
        );
    }

    #[test]
    fn does_not_force_unavailable_impersonation() {
        assert!(access_args(
            &EnginePreferences::default(),
            &YtDlpRuntimeCapabilities::default()
        )
        .is_empty());
    }

    #[test]
    fn parses_available_impersonation_target() {
        let table = "Client OS Source\n----------------\nChrome-136 Windows-10 curl_cffi\nSafari-18 MacOS-15 curl_cffi";
        assert_eq!(
            parse_impersonation_target(table).as_deref(),
            Some("chrome-136:windows-10")
        );
    }

    #[test]
    fn ignores_unavailable_impersonation_rows() {
        let table = "Client OS Source\n----------------\nChrome - curl_cffi (unavailable)";
        assert_eq!(parse_impersonation_target(table), None);
    }

    #[test]
    fn classifies_cookie_authentication_errors() {
        assert!(friendly_engine_error(
            "Account authentication is required. Use --cookies-from-browser"
        )
        .contains("browser session"));
    }

    #[test]
    fn redacts_sensitive_headers_from_diagnostics() {
        assert_eq!(
            sanitize_diagnostic("Cookie: session=secret\nordinary diagnostic"),
            "[sensitive request header redacted]\nordinary diagnostic"
        );
        assert_eq!(
            sanitize_diagnostic("failed https://cdn.example/file?token=secret&sig=also-secret"),
            "failed https://cdn.example/file?token=[redacted]&sig=[redacted]"
        );
    }

    #[test]
    fn identifies_reddit_api_ip_blocks_for_the_embed_fallback() {
        assert!(is_reddit_api_ip_error(Some(
            "ERROR: [Reddit] abc123: Your IP address is unable to access the Reddit API"
        )));
        assert!(!is_reddit_api_ip_error(Some("ERROR: HTTP Error 403")));
    }

    #[test]
    fn escapes_backslashes_in_metadata_title_replacements() {
        assert_eq!(literal_regex_replacement(r"one\two"), r"one\\two");
    }

    #[test]
    fn extracts_unique_http_entries_from_flat_playlist_json() {
        let (title, urls) = parse_playlist_expansion(serde_json::json!({
            "title": "Examples",
            "entries": [
                {"webpage_url": "https://example.com/one"},
                {"url": "https://example.com/two"},
                {"url": "not-a-url"},
                {"url": "https://example.com/one"}
            ]
        }))
        .unwrap();
        assert_eq!(title.as_deref(), Some("Examples"));
        assert_eq!(urls, ["https://example.com/one", "https://example.com/two"]);
    }

    #[tokio::test]
    #[ignore = "requires yt-dlp and public Reddit network access"]
    async fn analyzes_api_blocked_reddit_video_through_embed_fallback() {
        let analysis = YtDlpEngine::discover()
            .expect("discover yt-dlp")
            .analyze(
                "https://www.reddit.com/comments/1vkq6jk",
                &EnginePreferences {
                    browser_cookie_source: BrowserCookieSource::Firefox,
                    browser_profile: None,
                    proxy_url: None,
                    enable_impersonation: true,
                    bandwidth_limit_kib: None,
                },
            )
            .await
            .expect("analyze Reddit fallback");
        assert_eq!(analysis.title, "Pekeasmr");
        assert_eq!(analysis.site_name, "Reddit");
        assert!(analysis.download_url.starts_with("https://v.redd.it/"));
        assert!(analysis
            .formats
            .iter()
            .any(|format| format.height == Some(720)));
        assert!(analysis
            .formats
            .iter()
            .any(|format| format.audio_codec.is_some()));
    }
}
