use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub id: EngineId,
    pub display_name: String,
    pub installed: bool,
    pub executable_path: Option<String>,
    pub version: Option<String>,
    pub required: bool,
    pub status_message: String,
    pub capabilities: Vec<EngineCapability>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineCapability {
    pub id: String,
    pub display_name: String,
    pub available: bool,
    pub status_message: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EngineId {
    YtDlp,
    Ffmpeg,
    Ffprobe,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BrowserCookieSource {
    None,
    Firefox,
    Chromium,
    Chrome,
    Brave,
    Vivaldi,
    Edge,
    Opera,
}

impl BrowserCookieSource {
    pub fn yt_dlp_name(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Firefox => Some("firefox"),
            Self::Chromium => Some("chromium"),
            Self::Chrome => Some("chrome"),
            Self::Brave => Some("brave"),
            Self::Vivaldi => Some("vivaldi"),
            Self::Edge => Some("edge"),
            Self::Opera => Some("opera"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EnginePreferences {
    pub browser_cookie_source: BrowserCookieSource,
    pub enable_impersonation: bool,
}

impl Default for EnginePreferences {
    fn default() -> Self {
        Self {
            browser_cookie_source: BrowserCookieSource::None,
            enable_impersonation: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub id: String,
    pub extension: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub dynamic_range: Option<String>,
    pub approximate_file_size: Option<u64>,
    pub video_only: bool,
    pub audio_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityOption {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: QualityKind,
    pub max_height: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QualityKind {
    Best,
    Resolution,
    AudioOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaAnalysis {
    pub title: String,
    pub uploader: Option<String>,
    pub duration_seconds: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub extractor: String,
    pub site_name: String,
    pub playlist_title: Option<String>,
    pub playlist_index: Option<u32>,
    pub formats: Vec<MediaFormat>,
    pub quality_options: Vec<QualityOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub quality: QualityOption,
    pub container: OutputContainer,
    pub video_codec_preference: VideoCodecPreference,
    pub audio_quality: AudioQuality,
    pub audio_only: bool,
    pub subtitles: bool,
    pub subtitle_languages: Vec<String>,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub chapters: bool,
    pub sponsorblock: bool,
    pub destination: String,
    pub filename_template: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputContainer {
    Mp4,
    Mkv,
    Webm,
    Mp3,
    M4a,
    Opus,
}

impl OutputContainer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::Webm => "webm",
            Self::Mp3 => "mp3",
            Self::M4a => "m4a",
            Self::Opus => "opus",
        }
    }

    pub fn is_audio(self) -> bool {
        matches!(self, Self::Mp3 | Self::M4a | Self::Opus)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodecPreference {
    Any,
    H264,
    Av1,
    Vp9,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AudioQuality {
    #[serde(rename = "best")]
    Best,
    #[serde(rename = "320k")]
    K320,
    #[serde(rename = "256k")]
    K256,
    #[serde(rename = "192k")]
    K192,
    #[serde(rename = "128k")]
    K128,
}

impl AudioQuality {
    pub fn yt_dlp_value(self) -> Option<&'static str> {
        match self {
            Self::Best => None,
            Self::K320 => Some("320K"),
            Self::K256 => Some("256K"),
            Self::K192 => Some("192K"),
            Self::K128 => Some("128K"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDownloadRequest {
    pub url: String,
    pub analysis: MediaAnalysis,
    pub options: DownloadOptions,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Analyzing,
    Preparing,
    Downloading,
    PostProcessing,
    Merging,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub state: DownloadState,
    pub progress_percent: f64,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bytes_per_second: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub selected_quality: String,
    pub output_format: String,
    pub destination: String,
    pub output_path: Option<String>,
    pub current_operation: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl DownloadJob {
    pub fn from_request(id: String, request: &StartDownloadRequest) -> Self {
        let now = Utc::now();
        Self {
            id,
            url: request.url.clone(),
            title: request.analysis.title.clone(),
            thumbnail: request.analysis.thumbnail.clone(),
            state: DownloadState::Queued,
            progress_percent: 0.0,
            downloaded_bytes: None,
            total_bytes: None,
            speed_bytes_per_second: None,
            eta_seconds: None,
            selected_quality: request.options.quality.label.clone(),
            output_format: request.options.container.as_str().to_owned(),
            destination: request.options.destination.clone(),
            output_path: None,
            current_operation: "Waiting for an available download slot".to_owned(),
            error: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}
