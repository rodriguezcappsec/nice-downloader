mod metadata;

use crate::{
    engines::DownloadEngine,
    errors::AppError,
    models::{DownloadOptions, MediaAnalysis, QualityKind, VideoCodecPreference},
    utils::validation::{validate_filename_template, validate_media_url},
};
use std::{path::PathBuf, time::Duration};
use tokio::{process::Command, time::timeout};

pub use metadata::parse_metadata;

#[derive(Debug, Clone)]
pub struct YtDlpEngine {
    executable: PathBuf,
}

impl YtDlpEngine {
    pub fn discover() -> Result<Self, AppError> {
        let executable = which::which("yt-dlp").map_err(|_| {
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

    pub async fn analyze(&self, url: &str) -> Result<MediaAnalysis, AppError> {
        let validated = validate_media_url(url)?;
        let mut command = Command::new(&self.executable);
        command
            .kill_on_drop(true)
            .args([
                "--ignore-config",
                "--no-warnings",
                "--no-playlist",
                "--skip-download",
                "--dump-single-json",
                "--",
            ])
            .arg(validated.as_str());

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
        parse_metadata(value, validated.as_str())
    }
}

impl DownloadEngine for YtDlpEngine {
    fn executable_name(&self) -> &'static str {
        "yt-dlp"
    }

    fn build_download_args(
        &self,
        url: &str,
        options: &DownloadOptions,
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
        args.push("--".to_owned());
        args.push(validated_url.as_str().to_owned());
        Ok(args)
    }
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
    trimmed.get(start..).unwrap_or(trimmed).to_owned()
}

fn friendly_engine_error(details: &str) -> String {
    let lowercase = details.to_lowercase();
    if lowercase.contains("unsupported url") {
        "This URL is not supported by the installed version of yt-dlp.".to_owned()
    } else if lowercase.contains("private video") || lowercase.contains("login") {
        "This media is not publicly accessible. Authentication is not available yet.".to_owned()
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
    use crate::models::{AudioQuality, OutputContainer, QualityOption};

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
}
