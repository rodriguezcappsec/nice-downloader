use crate::{errors::AppError, utils::validation::validate_media_url};
use serde_json::Value;
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKind {
    Hls,
    Dash,
}

impl ManifestKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hls => "HLS",
            Self::Dash => "DASH",
        }
    }

    pub fn extractor(self) -> &'static str {
        match self {
            Self::Hls => "manifest:hls",
            Self::Dash => "manifest:dash",
        }
    }
}

/// Recognizes explicit manifest paths and signed endpoints whose query values
/// declare a manifest type. It deliberately does not classify generic URLs
/// containing words such as `stream` or `manifest` on their own.
pub fn classify_manifest_url(value: &str) -> Option<ManifestKind> {
    let url = Url::parse(value).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }

    if let Some(kind) = classify_hint(url.path()) {
        return Some(kind);
    }

    url.query_pairs().find_map(|(key, value)| {
        let key = key.to_ascii_lowercase();
        let value = value.to_ascii_lowercase();
        let declares_media_type = matches!(
            key.as_str(),
            "format"
                | "ext"
                | "extension"
                | "type"
                | "mime"
                | "mime_type"
                | "content-type"
                | "content_type"
                | "manifest"
                | "manifest_url"
                | "url"
        );
        (declares_media_type || value.ends_with(".m3u8") || value.ends_with(".mpd"))
            .then(|| classify_hint(&value))
            .flatten()
    })
}

fn classify_hint(value: &str) -> Option<ManifestKind> {
    let lowercase = value.to_ascii_lowercase();
    let without_fragment = lowercase.split('#').next().unwrap_or(&lowercase);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);

    if without_query.ends_with(".m3u8")
        || matches!(
            without_query,
            "m3u8" | "hls" | "application/x-mpegurl" | "application/vnd.apple.mpegurl"
        )
    {
        Some(ManifestKind::Hls)
    } else if without_query.ends_with(".mpd")
        || matches!(without_query, "mpd" | "dash" | "application/dash+xml")
    {
        Some(ManifestKind::Dash)
    } else {
        None
    }
}

pub fn validate_manifest_metadata(value: &Value, kind: ManifestKind) -> Result<(), AppError> {
    let formats = value
        .get("formats")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    let has_drm = value.get("has_drm").and_then(Value::as_bool) == Some(true)
        || formats.clone().any(|format| {
            format.get("has_drm").and_then(Value::as_bool) == Some(true)
                || format
                    .get("drm_family")
                    .and_then(Value::as_str)
                    .is_some_and(|family| !family.trim().is_empty())
        });
    if has_drm {
        return Err(manifest_error(
            "drm_manifest",
            format!(
                "This {} manifest is DRM protected and cannot be downloaded.",
                kind.label()
            ),
            "yt-dlp reported DRM-protected manifest formats.",
        ));
    }

    let live_status = value
        .get("live_status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let is_live = value.get("is_live").and_then(Value::as_bool) == Some(true)
        || matches!(live_status, "is_live" | "post_live");
    if is_live {
        return Err(manifest_error(
            "live_manifest",
            format!(
                "This is a live {} stream. Nice Downloader currently accepts finite VOD manifests only.",
                kind.label()
            ),
            format!("yt-dlp live_status={live_status:?}"),
        ));
    }

    Ok(())
}

pub fn map_manifest_analysis_error(kind: ManifestKind, error: AppError) -> AppError {
    let details = error.details.as_deref().unwrap_or_default();
    let lowercase = details.to_ascii_lowercase();
    let (code, message) = if contains_any(
        &lowercase,
        &[
            "drm",
            "widevine",
            "playready",
            "fairplay",
            "encrypted media extensions",
        ],
    ) {
        (
            "drm_manifest",
            format!(
                "This {} manifest is DRM protected and cannot be downloaded.",
                kind.label()
            ),
        )
    } else if kind == ManifestKind::Hls
        && contains_any(
            &lowercase,
            &[
                "unsupported encryption",
                "unsupported key",
                "unable to download key",
                "failed to download key",
                "sample-aes",
                "skd://",
            ],
        )
    {
        (
            "unsupported_hls_encryption",
            "This HLS playlist uses encryption or a media key that the downloader cannot access."
                .to_owned(),
        )
    } else if contains_any(
        &lowercase,
        &[
            "is a live stream",
            "live event",
            "live_status",
            "livestream",
        ],
    ) {
        (
            "live_manifest",
            format!(
                "This is a live {} stream. Nice Downloader currently accepts finite VOD manifests only.",
                kind.label()
            ),
        )
    } else {
        (
            "manifest_analysis_failed",
            format!(
                "The {} manifest could not be parsed or its media segments were unavailable.",
                kind.label()
            ),
        )
    };

    AppError::with_details(code, message, details)
}

pub fn normalize_manifest_analysis(
    mut analysis: crate::models::MediaAnalysis,
    requested_url: &str,
    kind: ManifestKind,
) -> Result<crate::models::MediaAnalysis, AppError> {
    let validated = validate_media_url(requested_url)?;
    // Keep the complete URL, including its signed query, for the eventual
    // engine invocation. Diagnostics are sanitized separately.
    analysis.webpage_url = validated.to_string();
    analysis.download_url = validated.to_string();
    analysis.extractor = kind.extractor().to_owned();
    analysis.site_name = format!("{} manifest", kind.label());
    Ok(analysis)
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn manifest_error(code: &str, message: String, details: impl Into<String>) -> AppError {
    AppError::with_details(code, message, details.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DownloadEngineKind, MediaAnalysis};

    #[test]
    fn recognizes_explicit_and_signed_manifest_urls_without_guessing() {
        assert_eq!(
            classify_manifest_url("https://cdn.example/video/master.M3U8?token=secret"),
            Some(ManifestKind::Hls)
        );
        assert_eq!(
            classify_manifest_url(
                "https://cdn.example/signed/playback?mime=application%2Fdash%2Bxml&sig=secret"
            ),
            Some(ManifestKind::Dash)
        );
        assert_eq!(
            classify_manifest_url(
                "https://cdn.example/play?manifest_url=https%3A%2F%2Forigin.example%2Fmaster.m3u8"
            ),
            Some(ManifestKind::Hls)
        );
        assert_eq!(
            classify_manifest_url("https://example.com/watch?stream=manifest"),
            None
        );
        assert_eq!(classify_manifest_url("file:///tmp/master.m3u8"), None);
    }

    #[test]
    fn rejects_drm_and_live_metadata_but_accepts_finite_vod() {
        let drm = serde_json::json!({
            "formats": [{"format_id": "dash", "has_drm": true, "protocol": "http_dash_segments"}]
        });
        assert_eq!(
            validate_manifest_metadata(&drm, ManifestKind::Dash)
                .unwrap_err()
                .code,
            "drm_manifest"
        );

        let live = serde_json::json!({"is_live": true, "live_status": "is_live"});
        assert_eq!(
            validate_manifest_metadata(&live, ManifestKind::Hls)
                .unwrap_err()
                .code,
            "live_manifest"
        );

        let vod = serde_json::json!({
            "is_live": false,
            "live_status": "not_live",
            "formats": [{"format_id": "hls-720", "protocol": "m3u8_native"}]
        });
        validate_manifest_metadata(&vod, ManifestKind::Hls).unwrap();
    }

    #[test]
    fn preserves_signed_query_when_normalizing_analysis() {
        let requested = "https://cdn.example/master.m3u8?token=secret&expires=99";
        let analysis = MediaAnalysis {
            engine: DownloadEngineKind::YtDlp,
            title: "Master".to_owned(),
            uploader: None,
            duration_seconds: Some(60.0),
            thumbnail: None,
            webpage_url: "https://wrong.example".to_owned(),
            download_url: "https://wrong.example".to_owned(),
            extractor: "generic".to_owned(),
            site_name: "Generic".to_owned(),
            playlist_title: None,
            playlist_index: None,
            formats: Vec::new(),
            quality_options: Vec::new(),
            item_count: None,
            total_size_bytes: None,
        };
        let normalized =
            normalize_manifest_analysis(analysis, requested, ManifestKind::Hls).unwrap();
        assert_eq!(normalized.download_url, requested);
        assert_eq!(normalized.extractor, "manifest:hls");
    }

    #[test]
    fn maps_manifest_failures_to_actionable_categories() {
        let encryption = AppError::with_details(
            "analysis_failed",
            "failed",
            "Unable to download key: HTTP Error 403",
        );
        assert_eq!(
            map_manifest_analysis_error(ManifestKind::Hls, encryption).code,
            "unsupported_hls_encryption"
        );

        let malformed = AppError::with_details("analysis_failed", "failed", "Invalid MPD document");
        assert_eq!(
            map_manifest_analysis_error(ManifestKind::Dash, malformed).code,
            "manifest_analysis_failed"
        );
    }

    #[test]
    fn fixture_manifests_cover_vod_live_encryption_and_drm() {
        let hls_vod = include_str!("fixtures/hls_vod.m3u8");
        assert!(hls_vod.contains("#EXT-X-ENDLIST"));
        assert!(!hls_vod.contains("#EXT-X-KEY"));

        let hls_live = include_str!("fixtures/hls_live.m3u8");
        assert!(hls_live.contains("#EXT-X-MEDIA-SEQUENCE"));
        assert!(!hls_live.contains("#EXT-X-ENDLIST"));

        let hls_encrypted = include_str!("fixtures/hls_encrypted.m3u8");
        assert!(hls_encrypted.contains("METHOD=SAMPLE-AES"));

        let dash_drm = include_str!("fixtures/dash_drm.mpd");
        assert!(dash_drm.contains("ContentProtection"));
        assert!(dash_drm.contains("widevine"));
    }
}
