use crate::{
    errors::AppError,
    models::{MediaAnalysis, MediaFormat, QualityKind, QualityOption},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawMetadata {
    title: Option<String>,
    uploader: Option<String>,
    channel: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    webpage_url: Option<String>,
    original_url: Option<String>,
    extractor: Option<String>,
    extractor_key: Option<String>,
    playlist_title: Option<String>,
    playlist_index: Option<u32>,
    #[serde(default)]
    formats: Vec<RawFormat>,
}

#[derive(Debug, Deserialize)]
struct RawFormat {
    format_id: Option<String>,
    ext: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    fps: Option<f64>,
    vcodec: Option<String>,
    acodec: Option<String>,
    dynamic_range: Option<String>,
    filesize: Option<f64>,
    filesize_approx: Option<f64>,
}

pub fn parse_metadata(
    value: serde_json::Value,
    requested_url: &str,
) -> Result<MediaAnalysis, AppError> {
    let raw: RawMetadata = serde_json::from_value(value)
        .map_err(|error| AppError::internal("could not parse yt-dlp metadata", error))?;
    let title = raw
        .title
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::new("invalid_metadata", "yt-dlp did not return a media title."))?;
    let extractor = raw.extractor.unwrap_or_else(|| "generic".to_owned());
    let site_name = raw.extractor_key.unwrap_or_else(|| extractor.clone());
    let formats: Vec<MediaFormat> = raw
        .formats
        .into_iter()
        .filter_map(normalize_format)
        .collect();
    let quality_options = quality_options(&formats);

    Ok(MediaAnalysis {
        title,
        uploader: raw.uploader.or(raw.channel),
        duration_seconds: raw
            .duration
            .filter(|value| value.is_finite() && *value >= 0.0),
        thumbnail: raw.thumbnail,
        webpage_url: raw
            .webpage_url
            .or(raw.original_url)
            .unwrap_or_else(|| requested_url.to_owned()),
        extractor,
        site_name,
        playlist_title: raw.playlist_title,
        playlist_index: raw.playlist_index,
        formats,
        quality_options,
    })
}

fn normalize_format(raw: RawFormat) -> Option<MediaFormat> {
    let id = raw.format_id?;
    let video_codec = normalize_codec(raw.vcodec);
    let audio_codec = normalize_codec(raw.acodec);
    let has_video = video_codec.is_some();
    let has_audio = audio_codec.is_some();
    Some(MediaFormat {
        id,
        extension: raw.ext,
        width: finite_u32(raw.width),
        height: finite_u32(raw.height),
        fps: raw.fps.filter(|value| value.is_finite() && *value > 0.0),
        video_codec,
        audio_codec,
        dynamic_range: raw
            .dynamic_range
            .filter(|value| !value.eq_ignore_ascii_case("SDR")),
        approximate_file_size: finite_u64(raw.filesize.or(raw.filesize_approx)),
        video_only: has_video && !has_audio,
        audio_only: has_audio && !has_video,
    })
}

fn normalize_codec(value: Option<String>) -> Option<String> {
    value.filter(|codec| !codec.is_empty() && !codec.eq_ignore_ascii_case("none"))
}

fn finite_u32(value: Option<f64>) -> Option<u32> {
    value
        .filter(|number| number.is_finite() && *number >= 0.0 && *number <= u32::MAX as f64)
        .map(|number| number.round() as u32)
}

fn finite_u64(value: Option<f64>) -> Option<u64> {
    value
        .filter(|number| number.is_finite() && *number >= 0.0 && *number <= u64::MAX as f64)
        .map(|number| number.round() as u64)
}

fn quality_options(formats: &[MediaFormat]) -> Vec<QualityOption> {
    let max_height = formats
        .iter()
        .filter_map(|format| format.height)
        .max()
        .unwrap_or(0);
    let mut options = vec![QualityOption {
        id: "best".to_owned(),
        label: "Best".to_owned(),
        description: "Highest available quality".to_owned(),
        kind: QualityKind::Best,
        max_height: None,
    }];
    for height in [2160, 1440, 1080, 720, 480, 360] {
        if max_height >= height {
            options.push(QualityOption {
                id: format!("{height}p"),
                label: format!("{height}p"),
                description: format!("Best streams up to {height}p"),
                kind: QualityKind::Resolution,
                max_height: Some(height),
            });
        }
    }
    if formats.iter().any(|format| format.audio_codec.is_some()) {
        options.push(QualityOption {
            id: "audio".to_owned(),
            label: "Audio only".to_owned(),
            description: "Extract the best available audio".to_owned(),
            kind: QualityKind::AudioOnly,
            max_height: None,
        });
    }
    options
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalizes_metadata_and_quality_caps() {
        let analysis = parse_metadata(
            json!({
                "title": "Example",
                "extractor": "youtube",
                "extractor_key": "Youtube",
                "formats": [
                    {"format_id":"137","height":1080,"vcodec":"avc1","acodec":"none"},
                    {"format_id":"140","acodec":"mp4a","vcodec":"none","filesize_approx":1000}
                ]
            }),
            "https://example.com/video",
        )
        .unwrap();
        assert_eq!(analysis.title, "Example");
        assert!(analysis
            .quality_options
            .iter()
            .any(|quality| quality.id == "1080p"));
        assert!(analysis
            .quality_options
            .iter()
            .any(|quality| quality.id == "audio"));
        assert!(analysis.formats[0].video_only);
    }
}
