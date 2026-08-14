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
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub relative_path: Option<String>,
    #[serde(default)]
    pub media_kind: Option<LibraryMediaKind>,
    pub title: String,
    pub source_url: Option<String>,
    pub thumbnail: Option<String>,
    pub duration_seconds: Option<f64>,
    pub site: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    #[serde(default)]
    pub file_size_bytes: Option<u64>,
    #[serde(default)]
    pub modified_at_ms: Option<i64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub video_codec: Option<String>,
    #[serde(default)]
    pub audio_codec: Option<String>,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub dynamic_range: Option<String>,
    #[serde(default)]
    pub metadata_state: LibraryMetadataState,
    #[serde(default)]
    pub metadata_error: Option<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub rating: Option<u8>,
    pub missing: bool,
    pub added_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryAvailabilityFilter {
    All,
    #[default]
    Available,
    Missing,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryFolderScope {
    #[default]
    Tree,
    Direct,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySort {
    #[default]
    Recent,
    Oldest,
    TitleAsc,
    TitleDesc,
    DurationDesc,
    DurationAsc,
    SizeDesc,
    SizeAsc,
    FileDateDesc,
    FileDateAsc,
    PublishedDesc,
    PublishedAsc,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQueryRequest {
    pub page: u32,
    pub page_size: u32,
    pub search: Option<String>,
    pub source_id: Option<String>,
    pub folder_source_id: Option<String>,
    pub folder_path: Option<String>,
    pub folder_scope: LibraryFolderScope,
    pub media_kind: Option<LibraryMediaKind>,
    pub resolution: Option<String>,
    pub container: Option<String>,
    pub duration: Option<String>,
    pub date_added: Option<String>,
    #[serde(default)]
    pub date_published: Option<String>,
    pub availability: LibraryAvailabilityFilter,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creators: Vec<String>,
    #[serde(default)]
    pub series: Vec<String>,
    #[serde(default)]
    pub sites: Vec<String>,
    pub rating: Option<String>,
    pub collection_id: Option<String>,
    pub favorite_only: bool,
    #[serde(default)]
    pub sort: LibrarySort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySmartViewQuery {
    pub search: Option<String>,
    pub source_id: Option<String>,
    pub folder_source_id: Option<String>,
    pub folder_path: Option<String>,
    pub folder_scope: LibraryFolderScope,
    pub media_kind: Option<LibraryMediaKind>,
    pub resolution: Option<String>,
    pub container: Option<String>,
    pub duration: Option<String>,
    pub date_added: Option<String>,
    pub date_published: Option<String>,
    pub availability: LibraryAvailabilityFilter,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub creators: Vec<String>,
    #[serde(default)]
    pub series: Vec<String>,
    #[serde(default)]
    pub sites: Vec<String>,
    pub rating: Option<String>,
    pub collection_id: Option<String>,
    pub favorite_only: bool,
    pub sort: LibrarySort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySmartView {
    pub id: String,
    pub name: String,
    pub query: LibrarySmartViewQuery,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibrarySmartViewRequest {
    pub id: Option<String>,
    pub name: String,
    pub query: LibrarySmartViewQuery,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySearchStatus {
    pub catalog_items: usize,
    pub indexed_items: usize,
    pub consistent: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryQueryResult {
    pub items: Vec<LibraryItem>,
    pub total_items: usize,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFacetRequest {
    pub source_id: Option<String>,
    pub availability: LibraryAvailabilityFilter,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFolderFacet {
    pub source_id: String,
    pub path: String,
    pub direct_count: usize,
    pub tree_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFacets {
    pub total_items: usize,
    pub available_items: usize,
    pub missing_items: usize,
    pub containers: Vec<String>,
    pub tags: Vec<String>,
    pub creators: Vec<String>,
    pub series: Vec<String>,
    pub sites: Vec<String>,
    pub folders: Vec<LibraryFolderFacet>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMetadataFields {
    #[serde(default)]
    pub creator: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub upload_date: Option<String>,
    #[serde(default)]
    pub series: Option<String>,
    #[serde(default)]
    pub episode: Option<String>,
    #[serde(default)]
    pub original_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRichMetadata {
    pub item_id: String,
    pub extracted: LibraryMetadataFields,
    pub overrides: LibraryMetadataFields,
    pub effective: LibraryMetadataFields,
    pub overridden_fields: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibraryRichMetadataRequest {
    pub item_id: String,
    pub creator: Option<String>,
    pub description: Option<String>,
    pub upload_date: Option<String>,
    pub series: Option<String>,
    pub episode: Option<String>,
    pub original_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLibraryArtworkRequest {
    pub item_id: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaybackState {
    pub item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub completed: bool,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibraryPlaybackRequest {
    pub item_id: String,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub completed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaybackEntry {
    pub item: LibraryItem,
    pub playback: LibraryPlaybackState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryWatchState {
    pub item_id: String,
    pub watch_later: bool,
    pub playback: Option<LibraryPlaybackState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySubtitleTrack {
    pub label: String,
    pub language: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaybackSession {
    pub item: LibraryItem,
    pub url: String,
    pub playback: Option<LibraryPlaybackState>,
    pub subtitles: Vec<LibrarySubtitleTrack>,
    pub compatibility_mode: bool,
    pub compatibility_transcoded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryImageSession {
    pub item_id: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaybackPreflight {
    pub compatibility_required: bool,
    pub compatibility_cached: bool,
    pub unusually_large: bool,
    pub reencode_required: bool,
    pub estimated_copy_bytes: u64,
    pub available_space_bytes: Option<u64>,
    pub reason: Option<String>,
    pub preference: PlaybackCompatibilityPreference,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaybackCacheStatus {
    pub file_count: usize,
    pub total_bytes: u64,
    pub max_files: usize,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryMetadataState {
    #[default]
    Pending,
    Ready,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct LibraryProbeResult {
    pub file_size_bytes: u64,
    pub modified_at_ms: i64,
    pub duration_seconds: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub dynamic_range: Option<String>,
    pub state: LibraryMetadataState,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCacheResult {
    pub removed_files: usize,
    pub freed_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtworkCacheStatus {
    pub file_count: usize,
    pub total_bytes: u64,
    pub max_files: usize,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLibraryItemRequest {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub rating: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCollection {
    pub id: String,
    pub name: String,
    pub item_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPlaylist {
    pub id: String,
    pub name: String,
    pub description: String,
    pub item_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryChangeKind {
    Added,
    Modified,
    Renamed,
    Missing,
    Restored,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryChange {
    pub id: String,
    pub item_id: String,
    pub source_id: Option<String>,
    pub kind: LibraryChangeKind,
    pub previous_path: Option<String>,
    pub detected_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryFileIdentity {
    pub device_id: u64,
    pub file_id: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryDuplicateAnalysisState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDuplicateAnalysisJob {
    pub id: String,
    pub state: LibraryDuplicateAnalysisState,
    pub examined_items: usize,
    pub candidate_items: usize,
    pub hashed_items: usize,
    pub skipped_items: usize,
    pub total_candidate_bytes: u64,
    pub hashed_bytes: u64,
    pub groups_found: usize,
    pub current_title: Option<String>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDuplicateGroup {
    pub id: String,
    pub item_ids: Vec<String>,
    pub file_size_bytes: u64,
    pub potential_reclaim_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDuplicateTrashResult {
    pub item_id: String,
    pub group_id: String,
    pub remaining_item_ids: Vec<String>,
    pub moved_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryFileOperationKind {
    Move,
    Copy,
    Rename,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryFileOperationState {
    Planned,
    Executing,
    Completed,
    Failed,
    Undone,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLibraryFileOperationRequest {
    pub item_id: String,
    pub kind: LibraryFileOperationKind,
    pub destination_directory: Option<String>,
    pub new_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFileOperation {
    pub id: String,
    pub item_id: String,
    pub output_item_id: Option<String>,
    pub kind: LibraryFileOperationKind,
    pub state: LibraryFileOperationState,
    pub source_path: String,
    pub destination_path: String,
    pub source_size_bytes: u64,
    pub source_modified_at_ms: i64,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFileOperationPlan {
    pub operation: LibraryFileOperation,
    pub source_display_name: String,
    pub destination_display_name: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFileOperationResult {
    pub operation: LibraryFileOperation,
    pub items: Vec<LibraryItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewLibraryBulkMoveRequest {
    pub item_ids: Vec<String>,
    pub destination_directory: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBulkMovePlan {
    pub operations: Vec<LibraryFileOperationPlan>,
    pub destination_directory: String,
    pub total_size_bytes: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBulkMoveFailure {
    pub operation_id: String,
    pub item_id: String,
    pub code: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBulkMoveResult {
    pub results: Vec<LibraryFileOperationResult>,
    pub failures: Vec<LibraryBulkMoveFailure>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibraryCollectionRequest {
    pub id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibraryPlaylistRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LibraryBulkAction {
    SetFavorite { favorite: bool },
    SetRating { rating: Option<u8> },
    AddTags { tags: Vec<String> },
    AddToCollection { collection_id: String },
    RemoveFromCollection { collection_id: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanResult {
    pub checked_count: usize,
    pub missing_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryMediaKind {
    Video,
    Image,
    Both,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibrarySourceStatus {
    Online,
    Offline,
    Scanning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySource {
    pub id: String,
    pub name: String,
    pub path: String,
    pub media_kind: LibraryMediaKind,
    pub recursive: bool,
    pub watch_enabled: bool,
    pub private: bool,
    pub include_patterns: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub status: LibrarySourceStatus,
    pub last_error: Option<String>,
    pub last_scanned_at: Option<DateTime<Utc>>,
    pub item_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveLibrarySourceRequest {
    pub id: Option<String>,
    pub name: String,
    pub path: String,
    pub media_kind: LibraryMediaKind,
    pub recursive: bool,
    pub watch_enabled: bool,
    pub private: bool,
    #[serde(default)]
    pub include_patterns: Vec<String>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LibraryScanState {
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanJob {
    pub id: String,
    pub source_id: String,
    pub state: LibraryScanState,
    pub discovered_count: usize,
    pub indexed_count: usize,
    pub skipped_count: usize,
    pub warning_count: usize,
    pub current_path: Option<String>,
    pub warnings: Vec<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
    #[default]
    Ask,
    Background,
    Quit,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackCompatibilityPreference {
    #[default]
    Ask,
    Automatic,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct DesktopPreferences {
    pub close_behavior: CloseBehavior,
    pub start_minimized: bool,
    pub notify_when_backgrounded: bool,
    pub reduce_motion: bool,
    pub high_contrast: bool,
    pub interface_scale_percent: u16,
    pub playback_compatibility: PlaybackCompatibilityPreference,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRuntimeStatus {
    pub tray_available: bool,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            close_behavior: CloseBehavior::Ask,
            start_minimized: false,
            notify_when_backgrounded: true,
            reduce_motion: false,
            high_contrast: false,
            interface_scale_percent: 100,
            playback_compatibility: PlaybackCompatibilityPreference::Ask,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub completed: bool,
    pub default_destination: String,
    pub engines: Vec<EngineInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub settings: QueueSettings,
    pub ordered_job_ids: Vec<String>,
    pub active_job_ids: Vec<String>,
    pub pause_supported: bool,
    pub completion_notifications_supported: bool,
}

impl QueueSnapshot {
    pub fn new(
        settings: QueueSettings,
        ordered_job_ids: Vec<String>,
        active_job_ids: Vec<String>,
    ) -> Self {
        Self {
            settings,
            ordered_job_ids,
            active_job_ids,
            pause_supported: cfg!(unix),
            completion_notifications_supported: cfg!(target_os = "linux"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDownloadFailure {
    pub url: String,
    pub error: crate::errors::AppError,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDownloadResult {
    pub jobs: Vec<DownloadJob>,
    pub failures: Vec<BatchDownloadFailure>,
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

    #[test]
    fn older_desktop_preferences_receive_accessibility_defaults() {
        let preferences: DesktopPreferences = serde_json::from_value(json!({
            "closeBehavior": "ask",
            "startMinimized": false,
            "notifyWhenBackgrounded": true
        }))
        .expect("legacy desktop preferences");
        assert!(!preferences.reduce_motion);
        assert!(!preferences.high_contrast);
        assert_eq!(preferences.interface_scale_percent, 100);
        assert_eq!(
            preferences.playback_compatibility,
            PlaybackCompatibilityPreference::Ask
        );
    }
}
