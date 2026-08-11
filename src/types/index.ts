export type ViewId =
  | "new"
  | "downloads"
  | "queue"
  | "history"
  | "subscriptions"
  | "presets"
  | "settings";

export interface AppError {
  code: string;
  message: string;
  details?: string | null;
}

export interface EngineInfo {
  id: "ytDlp" | "ffmpeg" | "ffprobe" | "galleryDl" | "aria2" | "curl";
  displayName: string;
  installed: boolean;
  executablePath?: string | null;
  version?: string | null;
  required: boolean;
  statusMessage: string;
  capabilities: EngineCapability[];
}

export type EngineId = EngineInfo["id"];

export interface ManagedEngineStatus {
  engine: EngineId;
  supported: boolean;
  activeVersion?: string | null;
  previousVersion?: string | null;
  pinned: boolean;
  installedVersions: string[];
}

export interface EngineUpdateCandidate {
  engine: EngineId;
  version: string;
  downloadUrl: string;
  sha256: string;
}

export interface EngineCapability {
  id: string;
  displayName: string;
  available: boolean;
  statusMessage: string;
}

export type BrowserCookieSource =
  | "none"
  | "firefox"
  | "chromium"
  | "chrome"
  | "brave"
  | "vivaldi"
  | "edge"
  | "opera";

export interface EnginePreferences {
  browserCookieSource: BrowserCookieSource;
  browserProfile?: string | null;
  proxyUrl?: string | null;
  enableImpersonation: boolean;
  bandwidthLimitKib?: number | null;
}

export interface MediaFormat {
  id: string;
  extension?: string | null;
  width?: number | null;
  height?: number | null;
  fps?: number | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
  dynamicRange?: string | null;
  approximateFileSize?: number | null;
  videoOnly: boolean;
  audioOnly: boolean;
}

export type QualityKind = "best" | "resolution" | "audioOnly";

export interface QualityOption {
  id: string;
  label: string;
  description: string;
  kind: QualityKind;
  maxHeight?: number | null;
}

export interface MediaAnalysis {
  engine: "ytDlp" | "galleryDl" | "directHttp";
  title: string;
  uploader?: string | null;
  durationSeconds?: number | null;
  thumbnail?: string | null;
  webpageUrl: string;
  downloadUrl: string;
  extractor: string;
  siteName: string;
  playlistTitle?: string | null;
  playlistIndex?: number | null;
  formats: MediaFormat[];
  qualityOptions: QualityOption[];
  itemCount?: number | null;
  totalSizeBytes?: number | null;
}

export interface GalleryItem {
  index: number;
  id: string;
  filename: string;
  extension?: string | null;
  sizeBytes?: number | null;
  thumbnail?: string | null;
  isVideo: boolean;
}

export interface GalleryItemList {
  items: GalleryItem[];
  totalCount: number;
  truncated: boolean;
  offset: number;
  nextOffset?: number | null;
}

export interface GalleryItemSelection {
  index: number;
  sizeBytes?: number | null;
}

export interface BatchAnalysisItem {
  inputUrl: string;
  analysis?: MediaAnalysis | null;
  error?: AppError | null;
}

export interface BatchAnalysisResponse {
  collectionTitle?: string | null;
  items: BatchAnalysisItem[];
}

export interface DownloadOptions {
  quality: QualityOption;
  container: "mp4" | "mkv" | "webm" | "mp3" | "m4a" | "opus";
  videoCodecPreference: "any" | "h264" | "av1" | "vp9";
  formatStrategy: "recommended" | "compatibility" | "quality" | "smallest";
  audioQuality: "best" | "320k" | "256k" | "192k" | "128k";
  audioOnly: boolean;
  subtitles: boolean;
  subtitleLanguages: string[];
  embedThumbnail: boolean;
  embedMetadata: boolean;
  chapters: boolean;
  sponsorblock: boolean;
  sponsorblockCategories: SponsorBlockCategory[];
  destination: string;
  filenameTemplate: string;
  outputConflictPolicy: "skip" | "overwrite";
  gallerySelection?: GalleryItemSelection[] | null;
}

export type SponsorBlockCategory =
  | "sponsor"
  | "intro"
  | "outro"
  | "selfPromotion"
  | "interaction"
  | "preview"
  | "filler"
  | "musicOfftopic";

export type DownloadState =
  | "Queued"
  | "Paused"
  | "Analyzing"
  | "Preparing"
  | "Downloading"
  | "PostProcessing"
  | "Merging"
  | "Verifying"
  | "Completed"
  | "Failed"
  | "Cancelled";

export interface DownloadJob {
  id: string;
  url: string;
  title: string;
  thumbnail?: string | null;
  state: DownloadState;
  progressPercent: number;
  downloadedBytes?: number | null;
  totalBytes?: number | null;
  speedBytesPerSecond?: number | null;
  etaSeconds?: number | null;
  selectedQuality: string;
  outputFormat: string;
  destination: string;
  outputPath?: string | null;
  currentOperation: string;
  error?: string | null;
  diagnostics: string[];
  attemptCount: number;
  retryOfId?: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface StartDownloadRequest {
  url: string;
  analysis: MediaAnalysis;
  options: DownloadOptions;
  scheduledFor?: string | null;
}

export interface QueueSettings {
  concurrentDownloads: number;
  paused: boolean;
  notifyOnCompletion: boolean;
  completionAction: "none" | "openFolder";
}

export interface QueueSnapshot {
  settings: QueueSettings;
  orderedJobIds: string[];
  activeJobIds: string[];
}

export interface DiskSpaceInfo {
  availableBytes: number;
  requiredBytes?: number | null;
  sufficient: boolean;
}

export interface PresetIntent {
  quality: "best" | "2160" | "1440" | "1080" | "720" | "480" | "audio";
  container: DownloadOptions["container"];
  videoCodecPreference: DownloadOptions["videoCodecPreference"];
  audioQuality: DownloadOptions["audioQuality"];
  embedThumbnail: boolean;
  embedMetadata: boolean;
  outputConflictPolicy: DownloadOptions["outputConflictPolicy"];
  sponsorblockCategories: SponsorBlockCategory[];
  filenameTemplate?: string | null;
}

export interface Preset {
  id: string;
  name: string;
  intent: PresetIntent;
  createdAt: string;
  updatedAt: string;
}

export interface SavePresetRequest {
  id?: string | null;
  name: string;
  intent: PresetIntent;
}

export interface SitePresetRule {
  id: string;
  name: string;
  hostSuffix: string;
  pathPrefix?: string | null;
  presetId: string;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SaveSitePresetRuleRequest {
  id?: string | null;
  name: string;
  hostSuffix: string;
  pathPrefix?: string | null;
  presetId: string;
  enabled: boolean;
}

export interface DownloadArchiveMatch {
  url: string;
  destination: string;
  outputPath?: string | null;
  completedAt: string;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  presetId: string;
  destination: string;
  checkIntervalMinutes: number;
  maxEntries: number;
  enabled: boolean;
  lastCheckedAt?: string | null;
  nextCheckAt: string;
  lastError?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface SaveSubscriptionRequest {
  id?: string | null;
  name: string;
  url: string;
  presetId: string;
  destination: string;
  checkIntervalMinutes: number;
  maxEntries: number;
  enabled: boolean;
}

export interface SubscriptionCheckResult {
  subscription: Subscription;
  discoveredCount: number;
  queuedCount: number;
  skippedCount: number;
}

export type HistoryEntry = DownloadJob;
