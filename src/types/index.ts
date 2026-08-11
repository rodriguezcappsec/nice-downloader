export type ViewId =
  | "new"
  | "downloads"
  | "queue"
  | "history"
  | "presets"
  | "settings";

export interface AppError {
  code: string;
  message: string;
  details?: string | null;
}

export interface EngineInfo {
  id: "ytDlp" | "ffmpeg" | "ffprobe" | "galleryDl" | "aria2";
  displayName: string;
  installed: boolean;
  executablePath?: string | null;
  version?: string | null;
  required: boolean;
  statusMessage: string;
  capabilities: EngineCapability[];
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
  engine: "ytDlp" | "galleryDl";
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
  audioQuality: "best" | "320k" | "256k" | "192k" | "128k";
  audioOnly: boolean;
  subtitles: boolean;
  subtitleLanguages: string[];
  embedThumbnail: boolean;
  embedMetadata: boolean;
  chapters: boolean;
  sponsorblock: boolean;
  destination: string;
  filenameTemplate: string;
  outputConflictPolicy: "skip" | "overwrite";
  gallerySelection?: GalleryItemSelection[] | null;
}

export type DownloadState =
  | "Queued"
  | "Paused"
  | "Analyzing"
  | "Preparing"
  | "Downloading"
  | "PostProcessing"
  | "Merging"
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

export type HistoryEntry = DownloadJob;
