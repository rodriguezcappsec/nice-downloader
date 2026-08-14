export type ViewId =
  | "new"
  | "downloads"
  | "queue"
  | "history"
  | "subscriptions"
  | "transformations"
  | "library"
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

export interface AuthenticationProfile {
  id: string;
  name: string;
  hostSuffix: string;
  browserCookieSource: BrowserCookieSource;
  browserProfile?: string | null;
  proxyUrl?: string | null;
  cookieFileConfigured: boolean;
  enabled: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SaveAuthenticationProfileRequest {
  id?: string | null;
  name: string;
  hostSuffix: string;
  browserCookieSource: BrowserCookieSource;
  browserProfile?: string | null;
  proxyUrl?: string | null;
  cookieFilePath?: string | null;
  clearCookieFile: boolean;
  enabled: boolean;
}

export interface BrowserProfileInfo {
  browser: BrowserCookieSource;
  name: string;
  path: string;
}

export interface DesktopPreferences {
  closeBehavior: "ask" | "background" | "quit";
  startMinimized: boolean;
  notifyWhenBackgrounded: boolean;
  reduceMotion: boolean;
  highContrast: boolean;
  interfaceScalePercent: 90 | 100 | 110 | 125;
}
export interface DesktopRuntimeStatus { trayAvailable: boolean; }

export interface OnboardingStatus {
  completed: boolean;
  defaultDestination: string;
  engines: EngineInfo[];
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
export interface BatchDownloadFailure { url: string; error: AppError; }
export interface BatchDownloadResult { jobs: DownloadJob[]; failures: BatchDownloadFailure[]; }

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
  pauseSupported: boolean;
  completionNotificationsSupported: boolean;
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

export type TransformationOperation =
  | { type: "trim"; startSeconds: number; endSeconds: number }
  | { type: "extractAudio"; codec: "mp3" | "aac" | "opus" }
  | { type: "transcode"; container: string; videoCodec: "copy" | "h264" | "hevc" | "vp9" | "av1"; audioCodec: "copy" | "aac" | "opus" }
  | { type: "splitChapters" };

export interface StartTransformationRequest {
  inputPath: string;
  outputPath: string;
  operation: TransformationOperation;
  overwrite: boolean;
}

export interface TransformationTask {
  id: string;
  inputPath: string;
  outputPath: string;
  operation: TransformationOperation;
  state: "Queued" | "Running" | "Completed" | "Failed" | "Cancelled";
  progressPercent: number;
  currentOperation: string;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface LibraryItem {
  id: string;
  path: string;
  sourceId?: string | null;
  relativePath?: string | null;
  mediaKind?: LibraryMediaKind | null;
  title: string;
  sourceUrl?: string | null;
  thumbnail?: string | null;
  durationSeconds?: number | null;
  site?: string | null;
  tags: string[];
  metadata: Record<string, unknown>;
  fileSizeBytes?: number | null;
  modifiedAtMs?: number | null;
  width?: number | null;
  height?: number | null;
  fps?: number | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
  container?: string | null;
  dynamicRange?: string | null;
  metadataState: "pending" | "ready" | "failed" | "unavailable";
  metadataError?: string | null;
  favorite: boolean;
  rating?: number | null;
  missing: boolean;
  addedAt: string;
  updatedAt: string;
}

export interface LibraryCacheResult {
  removedFiles: number;
  freedBytes: number;
}

export interface UpdateLibraryItemRequest {
  id: string;
  title: string;
  tags: string[];
  favorite: boolean;
  rating?: number | null;
}

export interface LibraryCollection {
  id: string;
  name: string;
  itemIds: string[];
  createdAt: string;
  updatedAt: string;
}

export type LibraryChangeKind = "added" | "modified" | "renamed" | "missing" | "restored";

export interface LibraryChange {
  id: string;
  itemId: string;
  sourceId?: string | null;
  kind: LibraryChangeKind;
  previousPath?: string | null;
  detectedAt: string;
  reviewedAt?: string | null;
}

export type LibraryDuplicateAnalysisState = "running" | "completed" | "cancelled" | "failed";

export interface LibraryDuplicateAnalysisJob {
  id: string;
  state: LibraryDuplicateAnalysisState;
  examinedItems: number;
  candidateItems: number;
  hashedItems: number;
  skippedItems: number;
  totalCandidateBytes: number;
  hashedBytes: number;
  groupsFound: number;
  currentTitle?: string | null;
  error?: string | null;
  startedAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface LibraryDuplicateGroup {
  id: string;
  itemIds: string[];
  fileSizeBytes: number;
  potentialReclaimBytes: number;
}

export interface LibraryDuplicateTrashResult {
  itemId: string;
  groupId: string;
  remainingItemIds: string[];
  movedBytes: number;
}

export type LibraryFileOperationKind = "move" | "copy" | "rename";
export type LibraryFileOperationState = "planned" | "executing" | "completed" | "failed" | "undone";

export interface PreviewLibraryFileOperationRequest {
  itemId: string;
  kind: LibraryFileOperationKind;
  destinationDirectory?: string | null;
  newFileName?: string | null;
}

export interface LibraryFileOperation {
  id: string;
  itemId: string;
  outputItemId?: string | null;
  kind: LibraryFileOperationKind;
  state: LibraryFileOperationState;
  sourcePath: string;
  destinationPath: string;
  sourceSizeBytes: number;
  sourceModifiedAtMs: number;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export interface LibraryFileOperationPlan {
  operation: LibraryFileOperation;
  sourceDisplayName: string;
  destinationDisplayName: string;
  warnings: string[];
}

export interface LibraryFileOperationResult {
  operation: LibraryFileOperation;
  items: LibraryItem[];
}

export interface PreviewLibraryBulkMoveRequest {
  itemIds: string[];
  destinationDirectory: string;
}

export interface LibraryBulkMovePlan {
  operations: LibraryFileOperationPlan[];
  destinationDirectory: string;
  totalSizeBytes: number;
  warnings: string[];
}

export interface LibraryBulkMoveFailure {
  operationId: string;
  itemId: string;
  code: string;
  message: string;
  details?: string | null;
}

export interface LibraryBulkMoveResult {
  results: LibraryFileOperationResult[];
  failures: LibraryBulkMoveFailure[];
}

export interface SaveLibraryCollectionRequest {
  id?: string | null;
  name: string;
}

export type LibraryBulkAction =
  | { type: "setFavorite"; favorite: boolean }
  | { type: "setRating"; rating?: number | null }
  | { type: "addTags"; tags: string[] }
  | { type: "addToCollection"; collectionId: string }
  | { type: "removeFromCollection"; collectionId: string };

export interface LibraryScanResult {
  checkedCount: number;
  missingCount: number;
}

export type LibraryMediaKind = "video" | "image" | "both";
export type LibrarySourceStatus = "online" | "offline" | "scanning" | "error";

export interface LibrarySource {
  id: string;
  name: string;
  path: string;
  mediaKind: LibraryMediaKind;
  recursive: boolean;
  watchEnabled: boolean;
  private: boolean;
  includePatterns: string[];
  excludePatterns: string[];
  status: LibrarySourceStatus;
  lastError?: string | null;
  lastScannedAt?: string | null;
  itemCount: number;
  createdAt: string;
  updatedAt: string;
}

export interface SaveLibrarySourceRequest {
  id?: string | null;
  name: string;
  path: string;
  mediaKind: LibraryMediaKind;
  recursive: boolean;
  watchEnabled: boolean;
  private: boolean;
  includePatterns: string[];
  excludePatterns: string[];
}

export type LibraryScanState = "running" | "completed" | "cancelled" | "failed";

export interface LibraryScanJob {
  id: string;
  sourceId: string;
  state: LibraryScanState;
  discoveredCount: number;
  indexedCount: number;
  skippedCount: number;
  warningCount: number;
  currentPath?: string | null;
  warnings: string[];
  startedAt: string;
  updatedAt: string;
  completedAt?: string | null;
}

export type HistoryEntry = DownloadJob;
