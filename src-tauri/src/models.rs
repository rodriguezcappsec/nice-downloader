use crate::errors::AppError;
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EngineId {
    YtDlp,
    Ffmpeg,
    Ffprobe,
    GalleryDl,
    Aria2,
    Curl,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DownloadEngineKind {
    #[default]
    YtDlp,
    GalleryDl,
    DirectHttp,
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
    #[serde(default)]
    pub browser_profile: Option<String>,
    #[serde(default)]
    pub proxy_url: Option<String>,
    pub enable_impersonation: bool,
    #[serde(default)]
    pub bandwidth_limit_kib: Option<u64>,
    #[serde(default, skip_serializing)]
    pub managed_cookie_file: Option<String>,
}

impl Default for EnginePreferences {
    fn default() -> Self {
        Self {
            browser_cookie_source: BrowserCookieSource::None,
            browser_profile: None,
            proxy_url: None,
            enable_impersonation: true,
            bandwidth_limit_kib: None,
            managed_cookie_file: None,
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
    #[serde(default)]
    pub engine: DownloadEngineKind,
    pub title: String,
    pub uploader: Option<String>,
    pub duration_seconds: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub download_url: String,
    pub extractor: String,
    pub site_name: String,
    pub playlist_title: Option<String>,
    pub playlist_index: Option<u32>,
    pub formats: Vec<MediaFormat>,
    pub quality_options: Vec<QualityOption>,
    #[serde(default)]
    pub item_count: Option<u32>,
    #[serde(default)]
    pub total_size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItem {
    pub index: u32,
    pub id: String,
    pub filename: String,
    pub extension: Option<String>,
    pub size_bytes: Option<u64>,
    pub thumbnail: Option<String>,
    pub is_video: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItemList {
    pub items: Vec<GalleryItem>,
    pub total_count: u32,
    pub truncated: bool,
    pub offset: u32,
    pub next_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GalleryItemSelection {
    pub index: u32,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAnalysisItem {
    pub input_url: String,
    pub analysis: Option<MediaAnalysis>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAnalysisResponse {
    pub collection_title: Option<String>,
    pub items: Vec<BatchAnalysisItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    pub quality: QualityOption,
    pub container: OutputContainer,
    pub video_codec_preference: VideoCodecPreference,
    #[serde(default)]
    pub format_strategy: FormatStrategy,
    pub audio_quality: AudioQuality,
    pub audio_only: bool,
    pub subtitles: bool,
    pub subtitle_languages: Vec<String>,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub chapters: bool,
    pub sponsorblock: bool,
    #[serde(default)]
    pub sponsorblock_categories: Vec<SponsorBlockCategory>,
    pub destination: String,
    pub filename_template: String,
    #[serde(default)]
    pub output_conflict_policy: OutputConflictPolicy,
    #[serde(default)]
    pub gallery_selection: Option<Vec<GalleryItemSelection>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SponsorBlockCategory {
    Sponsor,
    Intro,
    Outro,
    SelfPromotion,
    Interaction,
    Preview,
    Filler,
    MusicOfftopic,
}

impl SponsorBlockCategory {
    pub fn as_ytdlp(self) -> &'static str {
        match self {
            Self::Sponsor => "sponsor",
            Self::Intro => "intro",
            Self::Outro => "outro",
            Self::SelfPromotion => "selfpromo",
            Self::Interaction => "interaction",
            Self::Preview => "preview",
            Self::Filler => "filler",
            Self::MusicOfftopic => "music_offtopic",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FormatStrategy {
    #[default]
    Recommended,
    Compatibility,
    Quality,
    Smallest,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OutputConflictPolicy {
    #[default]
    Skip,
    Overwrite,
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
    #[serde(default)]
    pub scheduled_for: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueueSettings {
    pub concurrent_downloads: usize,
    pub paused: bool,
    #[serde(default = "default_true")]
    pub notify_on_completion: bool,
    #[serde(default)]
    pub completion_action: CompletionAction,
}

impl Default for QueueSettings {
    fn default() -> Self {
        Self {
            concurrent_downloads: 2,
            paused: false,
            notify_on_completion: true,
            completion_action: CompletionAction::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompletionAction {
    #[default]
    None,
    OpenFolder,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetIntent {
    pub quality: String,
    pub container: OutputContainer,
    pub video_codec_preference: VideoCodecPreference,
    pub audio_quality: AudioQuality,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub output_conflict_policy: OutputConflictPolicy,
    #[serde(default)]
    pub sponsorblock_categories: Vec<SponsorBlockCategory>,
    #[serde(default)]
    pub filename_template: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub intent: PresetIntent,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePresetRequest {
    pub id: Option<String>,
    pub name: String,
    pub intent: PresetIntent,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SitePresetRule {
    pub id: String,
    pub name: String,
    pub host_suffix: String,
    pub path_prefix: Option<String>,
    pub preset_id: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSitePresetRuleRequest {
    pub id: Option<String>,
    pub name: String,
    pub host_suffix: String,
    pub path_prefix: Option<String>,
    pub preset_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadArchiveMatch {
    pub url: String,
    pub destination: String,
    pub output_path: Option<String>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub preset_id: String,
    pub destination: String,
    pub check_interval_minutes: u32,
    pub max_entries: u32,
    pub enabled: bool,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub next_check_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSubscriptionRequest {
    pub id: Option<String>,
    pub name: String,
    pub url: String,
    pub preset_id: String,
    pub destination: String,
    pub check_interval_minutes: u32,
    pub max_entries: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionCheckResult {
    pub subscription: Subscription,
    pub discovered_count: usize,
    pub queued_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransformationOperation {
    Trim {
        start_seconds: f64,
        end_seconds: f64,
    },
    ExtractAudio {
        codec: AudioTransformCodec,
    },
    Transcode {
        container: String,
        video_codec: TransformVideoCodec,
        audio_codec: TransformAudioCodec,
    },
    SplitChapters,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AudioTransformCodec {
    Mp3,
    Aac,
    Opus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransformVideoCodec {
    Copy,
    H264,
    Hevc,
    Vp9,
    Av1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TransformAudioCodec {
    Copy,
    Aac,
    Opus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransformationState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTransformationRequest {
    pub input_path: String,
    pub output_path: String,
    pub operation: TransformationOperation,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformationTask {
    pub id: String,
    pub input_path: String,
    pub output_path: String,
    pub operation: TransformationOperation,
    pub state: TransformationState,
    pub progress_percent: f64,
    pub current_operation: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub path: String,
    pub title: String,
    pub source_url: Option<String>,
    pub thumbnail: Option<String>,
    pub duration_seconds: Option<f64>,
    pub site: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub missing: bool,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryItemRequest {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanResult {
    pub checked_count: usize,
    pub missing_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProfile {
    pub id: String,
    pub name: String,
    pub host_suffix: String,
    pub browser_cookie_source: BrowserCookieSource,
    pub browser_profile: Option<String>,
    pub proxy_url: Option<String>,
    pub cookie_file_configured: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing)]
    pub managed_cookie_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAuthenticationProfileRequest {
    pub id: Option<String>,
    pub name: String,
    pub host_suffix: String,
    pub browser_cookie_source: BrowserCookieSource,
    pub browser_profile: Option<String>,
    pub proxy_url: Option<String>,
    pub cookie_file_path: Option<String>,
    pub clear_cookie_file: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileInfo {
    pub browser: BrowserCookieSource,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub settings: QueueSettings,
    pub ordered_job_ids: Vec<String>,
    pub active_job_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceInfo {
    pub available_bytes: u64,
    pub required_bytes: Option<u64>,
    pub sufficient: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadState {
    Queued,
    Paused,
    Analyzing,
    Preparing,
    Downloading,
    PostProcessing,
    Merging,
    Verifying,
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
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default = "default_attempt_count")]
    pub attempt_count: u32,
    pub retry_of_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl DownloadJob {
    pub fn from_request(id: String, request: &StartDownloadRequest) -> Self {
        let now = Utc::now();
        let gallery_download = request.analysis.engine == DownloadEngineKind::GalleryDl;
        let selected_gallery_total = request
            .options
            .gallery_selection
            .as_ref()
            .and_then(|items| {
                items
                    .iter()
                    .try_fold(0_u64, |total, item| total.checked_add(item.size_bytes?))
            });
        let selected_gallery_count = gallery_download
            .then(|| request.options.gallery_selection.as_ref().map(Vec::len))
            .flatten();
        Self {
            id,
            url: request.url.clone(),
            title: request.analysis.title.clone(),
            thumbnail: request.analysis.thumbnail.clone(),
            state: DownloadState::Queued,
            progress_percent: 0.0,
            downloaded_bytes: None,
            total_bytes: if gallery_download && request.options.gallery_selection.is_some() {
                selected_gallery_total
            } else {
                request.analysis.total_size_bytes
            },
            speed_bytes_per_second: None,
            eta_seconds: None,
            selected_quality: selected_gallery_count
                .map(|count| format!("{count} selected files"))
                .unwrap_or_else(|| request.options.quality.label.clone()),
            output_format: if gallery_download {
                "album".to_owned()
            } else {
                request.options.container.as_str().to_owned()
            },
            destination: request.options.destination.clone(),
            output_path: None,
            current_operation: "Waiting for an available download slot".to_owned(),
            error: None,
            diagnostics: Vec::new(),
            attempt_count: 1,
            retry_of_id: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }
}

fn default_attempt_count() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn older_persisted_analysis_defaults_to_ytdlp() {
        let analysis: MediaAnalysis = serde_json::from_value(json!({
            "title": "Legacy",
            "uploader": null,
            "durationSeconds": null,
            "thumbnail": null,
            "webpageUrl": "https://example.com/video",
            "downloadUrl": "https://example.com/video",
            "extractor": "generic",
            "siteName": "Generic",
            "playlistTitle": null,
            "playlistIndex": null,
            "formats": [],
            "qualityOptions": []
        }))
        .unwrap();
        assert_eq!(analysis.engine, DownloadEngineKind::YtDlp);
        assert_eq!(analysis.item_count, None);
        assert_eq!(analysis.total_size_bytes, None);
    }

    #[test]
    fn older_download_options_default_to_a_full_gallery() {
        let options: DownloadOptions = serde_json::from_value(json!({
            "quality": {"id":"best","label":"Best","description":"","kind":"best","maxHeight":null},
            "container": "mp4",
            "videoCodecPreference": "any",
            "audioQuality": "best",
            "audioOnly": false,
            "subtitles": false,
            "subtitleLanguages": [],
            "embedThumbnail": false,
            "embedMetadata": true,
            "chapters": false,
            "sponsorblock": false,
            "destination": "/tmp",
            "filenameTemplate": "%(title)s.%(ext)s"
        }))
        .unwrap();
        assert_eq!(options.gallery_selection, None);
        assert_eq!(options.output_conflict_policy, OutputConflictPolicy::Skip);
    }
}
