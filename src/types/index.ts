export type ViewId =
  | "home"
  | "new"
  | "downloads"
  | "queue"
  | "history"
  | "subscriptions"
  | "transformations"
  | "library"
  | "compare"
  | "imageCompare"
  | "curation"
  | "montages"
  | "similarity"
  | "themes"
  | "sharing"
  | "health"
  | "recipes"
  | "presets"
  | "tools"
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
  playbackCompatibility: "ask" | "automatic" | "never";
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

export type GalleryFileState = "queued" | "downloading" | "completed" | "skipped" | "failed";

export interface GalleryFileProgress {
  index: number;
  filename: string;
  sizeBytes?: number | null;
  state: GalleryFileState;
  progressPercent: number;
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
  galleryFiles: GalleryFileProgress[];
  attemptCount: number;
  retryOfId?: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt?: string | null;
  scheduledFor?: string | null;
}

export interface StartDownloadRequest {
  url: string;
  analysis: MediaAnalysis;
  options: DownloadOptions;
  scheduledFor?: string | null;
  postDownloadRecipeIds?: string[];
}

export type PostDownloadRecipeStep =
  | { type: "importLibrary" }
  | { type: "addTags"; tags: string[] }
  | { type: "addToCollection"; collectionId: string }
  | { type: "transcodeMp4" }
  | { type: "extractAudio"; codec: "mp3" | "aac" | "opus" }
  | { type: "privacySafeCopy" }
  | { type: "transcribe"; modelPath: string; language: string | null }
  | { type: "semanticIndex"; model: string };

export interface PostDownloadRecipe {
  id: string;
  name: string;
  enabled: boolean;
  steps: PostDownloadRecipeStep[];
  createdAt: string;
  updatedAt: string;
}

export interface SavePostDownloadRecipeRequest {
  id?: string | null;
  name: string;
  enabled: boolean;
  steps: PostDownloadRecipeStep[];
}

export interface PostDownloadRecipeRun {
  id: string;
  recipeId: string;
  recipeName: string;
  downloadId: string;
  state: "running" | "completed" | "failed";
  currentStep: number;
  stepCount: number;
  error: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
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
  | { type: "splitChapters" }
  | { type: "montage"; segmentCount: number; totalDurationSeconds: number; fadeSeconds: number }
  | { type: "privacyExport"; stripMetadata: boolean; blurRegions: PrivacyBlurRegion[]; muteRanges: PrivacyMuteRange[] };

export interface PrivacyBlurRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  startSeconds: number;
  endSeconds: number;
}

export interface PrivacyMuteRange {
  startSeconds: number;
  endSeconds: number;
}

export type PrivacyDetectionKind = "face" | "licensePlate" | "screen";

export interface PrivacyDetectionRequest {
  inputPath: string;
  model: string;
  kinds: PrivacyDetectionKind[];
}

export interface PrivacyRedactionSuggestion {
  kind: PrivacyDetectionKind;
  confidence: number;
  frameTimeSeconds: number;
  region: PrivacyBlurRegion;
}

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

export type LibraryAvailabilityFilter = "all" | "available" | "missing";
export type LibraryFolderScope = "tree" | "direct";
export type LibrarySort = "recent" | "oldest" | "titleAsc" | "titleDesc" | "durationDesc" | "durationAsc" | "sizeDesc" | "sizeAsc" | "fileDateDesc" | "fileDateAsc" | "publishedDesc" | "publishedAsc";

export interface LibraryQueryRequest {
  page: number;
  pageSize: number;
  search?: string | null;
  sourceId?: string | null;
  folderSourceId?: string | null;
  folderPath?: string | null;
  folderScope: LibraryFolderScope;
  mediaKind?: Exclude<LibraryMediaKind, "both"> | null;
  resolution?: string | null;
  container?: string | null;
  duration?: string | null;
  dateAdded?: string | null;
  datePublished?: string | null;
  availability: LibraryAvailabilityFilter;
  tags: string[];
  creators: string[];
  series: string[];
  sites: string[];
  theme?: string | null;
  rating?: string | null;
  collectionId?: string | null;
  favoriteOnly: boolean;
  privateOnly?: boolean;
  sort: LibrarySort;
}

export type LibrarySmartViewQuery = Omit<LibraryQueryRequest, "page" | "pageSize">;

export interface LibrarySmartView {
  id: string;
  name: string;
  query: LibrarySmartViewQuery;
  createdAt: string;
  updatedAt: string;
}

export interface SaveLibrarySmartViewRequest {
  id?: string | null;
  name: string;
  query: LibrarySmartViewQuery;
}

export interface LibrarySearchStatus {
  catalogItems: number;
  indexedItems: number;
  consistent: boolean;
}

export type LibraryHealthIssueKind = "missing" | "changed" | "metadataPending" | "metadataFailed" | "compatibilityRisk";

export interface LibraryHealthIssue {
  itemId: string;
  title: string;
  mediaKind?: LibraryMediaKind | null;
  kinds: LibraryHealthIssueKind[];
}

export interface LibraryHealthSummary {
  totalItems: number;
  availableItems: number;
  missingItems: number;
  changedItems: number;
  metadataPendingItems: number;
  metadataFailedItems: number;
  compatibilityRiskItems: number;
  issues: LibraryHealthIssue[];
}

export interface LibraryQueryResult {
  items: LibraryItem[];
  totalItems: number;
  page: number;
  pageSize: number;
  totalPages: number;
}

export interface LibraryFacetRequest {
  sourceId?: string | null;
  availability: LibraryAvailabilityFilter;
}

export interface LibraryFolderFacet {
  sourceId: string;
  path: string;
  directCount: number;
  treeCount: number;
}

export interface LibraryThemeFacet {
  key: string;
  label: string;
  itemCount: number;
}

export interface LibraryFacets {
  totalItems: number;
  availableItems: number;
  missingItems: number;
  containers: string[];
  tags: string[];
  creators: string[];
  series: string[];
  sites: string[];
  themes: LibraryThemeFacet[];
  folders: LibraryFolderFacet[];
}

export type LibraryThemeAnalysisMode = "fast" | "deep";
export type LibraryThemeAnalysisScope = "all" | "folder" | "items";

export interface LibraryThemeSettings {
  enabled: boolean;
  model: string;
  mode: LibraryThemeAnalysisMode;
}

export interface StartLibraryThemeAnalysisRequest {
  scope: LibraryThemeAnalysisScope;
  maxVideos: number;
  sourceId?: string | null;
  folderPath?: string | null;
  folderScope: LibraryFolderScope;
  itemIds: string[];
}

export interface LibraryThemeProviderStatus {
  available: boolean;
  installedModels: string[];
  selectedModelAvailable: boolean;
  detail: string;
}

export interface LibraryThemeAssignment {
  key: string;
  label: string;
  confidence: number;
  evidence: string[];
}

export interface LibraryThemeAnalysis {
  itemId: string;
  title: string;
  model: string;
  mode: LibraryThemeAnalysisMode;
  summary: string;
  overallConfidence: number;
  themes: LibraryThemeAssignment[];
  analyzedAt: string;
}

export interface LibraryThemeIndexStatus {
  eligibleItems: number;
  indexedItems: number;
  staleItems: number;
  themeCount: number;
}

export type LibraryThemeJobState = "running" | "completed" | "failed" | "cancelled";

export interface LibraryThemeJob {
  id: string;
  state: LibraryThemeJobState;
  checkedItems: number;
  totalItems: number;
  indexedItems: number;
  failedItems: number;
  currentTitle?: string | null;
  error?: string | null;
  startedAt: string;
  updatedAt: string;
}

export interface LibraryMetadataFields {
  creator?: string | null;
  description?: string | null;
  uploadDate?: string | null;
  series?: string | null;
  episode?: string | null;
  originalUrl?: string | null;
}

export interface LibraryRichMetadata {
  itemId: string;
  extracted: LibraryMetadataFields;
  overrides: LibraryMetadataFields;
  effective: LibraryMetadataFields;
  overriddenFields: string[];
  updatedAt: string;
}

export interface SaveLibraryRichMetadataRequest extends LibraryMetadataFields {
  itemId: string;
}

export interface SetLibraryArtworkRequest {
  itemId: string;
  path?: string | null;
}

export interface LibraryPlaybackState {
  itemId: string;
  positionSeconds: number;
  durationSeconds: number;
  completed: boolean;
  updatedAt: string;
}

export interface SaveLibraryPlaybackRequest {
  itemId: string;
  positionSeconds: number;
  durationSeconds: number;
  completed: boolean;
}

export interface LibraryPlaybackEntry {
  item: LibraryItem;
  playback: LibraryPlaybackState;
}

export interface LibraryWatchState {
  itemId: string;
  watchLater: boolean;
  playback?: LibraryPlaybackState | null;
}

export interface LibrarySubtitleTrack {
  label: string;
  language?: string | null;
  url: string;
}

export interface LibraryPlaybackSession {
  item: LibraryItem;
  url: string;
  playback?: LibraryPlaybackState | null;
  subtitles: LibrarySubtitleTrack[];
  compatibilityMode: boolean;
  compatibilityTranscoded: boolean;
  liveTranscode: boolean;
  segmentedStream: boolean;
  streamStartSeconds: number;
}

export interface LibraryImageSession {
  itemId: string;
  url: string;
}

export interface LibraryPlaybackPreflight {
  compatibilityRequired: boolean;
  compatibilityCached: boolean;
  unusuallyLarge: boolean;
  reencodeRequired: boolean;
  estimatedCopyBytes: number;
  availableSpaceBytes?: number | null;
  reason?: string | null;
  preference: DesktopPreferences["playbackCompatibility"];
  requiresConfirmation: boolean;
}

export interface LibraryTranscriptMatch {
  itemId: string;
  startSeconds: number;
  endSeconds: number;
  text: string;
  language?: string | null;
}

export interface LibraryTranscriptIndexStatus {
  indexedItems: number;
  cueCount: number;
}

export interface StartLibraryTranscriptionRequest {
  itemIds: string[];
  modelPath: string;
  language: string | null;
}

export interface StartLibrarySemanticIndexRequest {
  itemIds: string[];
  maxVideos: number;
  model: string;
}

export type LibraryLocalInferenceJobState = "running" | "completed" | "failed" | "cancelled";

export interface LibraryLocalInferenceJob {
  id: string;
  kind: "transcription" | "semanticIndex";
  state: LibraryLocalInferenceJobState;
  checkedItems: number;
  totalItems: number;
  completedItems: number;
  failedItems: number;
  currentTitle: string | null;
  model: string;
  error: string | null;
  startedAt: string;
  updatedAt: string;
}

export interface LibraryTranscriptionProviderStatus {
  available: boolean;
  executable: string | null;
  detail: string;
}

export interface LibrarySemanticIndexStatus {
  model: string;
  indexedItems: number;
  staleItems: number;
}

export interface LibrarySemanticTranscriptMatch {
  item: LibraryItem;
  scorePercent: number;
  startSeconds: number;
  endSeconds: number;
  excerpt: string;
  language: string | null;
}

export type LibraryVisualSimilarityState = "running" | "completed" | "failed" | "cancelled";

export interface LibraryVisualSimilarityJob {
  id: string;
  state: LibraryVisualSimilarityState;
  checkedItems: number;
  totalItems: number;
  indexedItems: number;
  failedItems: number;
  currentTitle?: string | null;
  error?: string | null;
  startedAt: string;
  updatedAt: string;
}

export interface LibraryVisualSimilarityStatus {
  eligibleItems: number;
  indexedItems: number;
  staleItems: number;
}

export interface LibraryVisualSimilarityMatch {
  item: LibraryItem;
  similarityPercent: number;
  hammingDistance: number;
}

export interface LibraryNearDuplicateMatch {
  left: LibraryItem;
  right: LibraryItem;
  visualSimilarityPercent: number;
  durationDeltaSeconds: number;
  sizeDeltaBytes: number | null;
  sameResolution: boolean;
  sameContainer: boolean;
  sameVideoCodec: boolean;
  sameAudioCodec: boolean;
  exactHashCompared: boolean;
}

export interface StartLocalShareRequest {
  itemIds: string[];
  expiresMinutes: number;
}

export interface LocalShareSession {
  id: string;
  url: string;
  itemCount: number;
  expiresAt: string;
  reachableOnLan: boolean;
}

export interface LibraryPlaybackPreparationProgress {
  itemId: string;
  phase: "queued" | "copying" | "encoding" | "finalizing";
  progressPercent?: number | null;
  processedSeconds?: number | null;
  durationSeconds?: number | null;
  reencodeRequired: boolean;
}

export interface LibraryPlaybackCacheStatus {
  fileCount: number;
  totalBytes: number;
  maxFiles: number;
  maxBytes: number;
}

export interface NativePlaybackSession {
  itemId: string;
  title: string;
  resumedAtSeconds: number;
  durationSeconds: number;
}

export interface NativePlaybackStatus {
  available: boolean;
  version?: string | null;
  active?: NativePlaybackSession | null;
  detail: string;
}

export interface EmbeddedMpvBounds {
  x: number;
  y: number;
  width: number;
  height: number;
  devicePixelRatio: number;
  visible: boolean;
}

export interface EmbeddedMpvSession {
  itemId: string;
  title: string;
  resumedAtSeconds: number;
  positionSeconds: number;
  durationSeconds: number;
  paused: boolean;
}

export interface EmbeddedMpvStatus {
  available: boolean;
  active?: EmbeddedMpvSession | null;
  detail: string;
  lastError?: string | null;
}

export type EmbeddedMpvControl =
  | "play"
  | "pause"
  | "seekRelative"
  | "seekAbsolute"
  | "setRate"
  | "setMuted";

export interface EmbeddedMpvControlRequest {
  action: EmbeddedMpvControl;
  value?: number | null;
}

export interface LibraryCacheResult {
  removedFiles: number;
  freedBytes: number;
}

export interface LibraryArtworkCacheStatus {
  fileCount: number;
  totalBytes: number;
  maxFiles: number;
  maxBytes: number;
}

export interface LibraryStorageBucket {
  key: string;
  label: string;
  bytes: number;
  itemCount: number;
}

export interface LibraryStorageSource {
  sourceId: string;
  name: string;
  path: string;
  catalogBytes: number;
  itemCount: number;
  availableBytes: number | null;
}

export interface LibraryStoragePlan {
  catalogBytes: number;
  sources: LibraryStorageSource[];
  folders: LibraryStorageBucket[];
  containers: LibraryStorageBucket[];
  exactDuplicateReclaimableBytes: number;
  playbackCacheBytes: number;
  artworkCacheBytes: number;
  incomingKnownBytes: number;
  incomingUnknownItems: number;
  activeTransformationCount: number;
  warnings: string[];
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

export interface LibraryPlaylist {
  id: string;
  name: string;
  description: string;
  itemIds: string[];
  createdAt: string;
  updatedAt: string;
}

export type LibraryMomentColor = "accent" | "blue" | "amber" | "green" | "rose";

export interface LibraryMoment {
  id: string;
  itemId: string;
  label: string;
  note: string;
  startSeconds: number;
  endSeconds?: number | null;
  color: LibraryMomentColor;
  createdAt: string;
  updatedAt: string;
}

export type LibraryChapterSuggestionKind = "chapter" | "highlight";

export interface LibraryChapterSuggestion {
  kind: LibraryChapterSuggestionKind;
  label: string;
  startSeconds: number;
  endSeconds: number | null;
  confidence: number;
  reason: string;
}

export interface SaveLibraryMomentRequest {
  id?: string | null;
  itemId: string;
  label: string;
  note: string;
  startSeconds: number;
  endSeconds?: number | null;
  color: LibraryMomentColor;
}

export interface LibraryMontageSegment {
  itemId: string;
  label: string;
  startSeconds: number;
  endSeconds: number;
}

export interface LibraryMontage {
  id: string;
  name: string;
  notes: string;
  fadeSeconds: number;
  segments: LibraryMontageSegment[];
  expectedSegmentCount: number;
  incomplete: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SaveLibraryMontageRequest {
  id?: string | null;
  name: string;
  notes: string;
  fadeSeconds: number;
  segments: LibraryMontageSegment[];
}

export interface StartLibraryMontageExportRequest {
  montageId: string;
  outputPath: string;
  overwrite: boolean;
}

export interface StartLibraryClipExportRequest {
  itemId: string;
  outputPath: string;
  startSeconds: number;
  endSeconds: number;
  overwrite: boolean;
}

export interface LibraryStoryboardFrame {
  timeSeconds: number;
  imageDataUrl: string;
}

export interface LibraryStoryboard {
  itemId: string;
  durationSeconds: number;
  frames: LibraryStoryboardFrame[];
  generatedAt: string;
}

export interface LibraryCompareSessionItem {
  itemId: string;
  position: number;
  playbackPositionSeconds: number;
  playbackRate: number;
}

export interface LibraryCompareSession {
  id: string;
  name: string;
  notes: string;
  items: LibraryCompareSessionItem[];
  activeAudioItemId?: string | null;
  multipleAudio: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface SaveLibraryCompareSessionRequest {
  id?: string | null;
  name: string;
  notes: string;
  items: LibraryCompareSessionItem[];
  activeAudioItemId?: string | null;
  multipleAudio: boolean;
}

export type LibraryMediaStackRole = "original" | "alternate" | "edit" | "generated" | "other";

export interface LibraryMediaStackMember {
  itemId: string;
  position: number;
  role: LibraryMediaStackRole;
}

export interface LibraryMediaStack {
  id: string;
  name: string;
  notes: string;
  coverItemId?: string | null;
  members: LibraryMediaStackMember[];
  createdAt: string;
  updatedAt: string;
}

export interface SaveLibraryMediaStackRequest {
  id?: string | null;
  name: string;
  notes: string;
  coverItemId?: string | null;
  members: LibraryMediaStackMember[];
}

export type LibraryCurationDecision = "keep" | "reviewLater";

export interface LibraryCurationState {
  itemId: string;
  decision?: LibraryCurationDecision | null;
  reviewedAt?: string | null;
}

export interface ExportLibraryMomentsRequest {
  itemId: string;
  outputPath: string;
  format: "markdown" | "csv";
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

export interface SaveLibraryPlaylistRequest {
  id?: string | null;
  name: string;
  description: string;
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

export interface LibraryPrivacyStatus {
  configured: boolean;
  locked: boolean;
  autoLockMinutes: number;
  privateSourceCount: number;
  privateFolderCount: number;
  privateItemCount: number;
}

export interface ConfigureLibraryPrivacyRequest {
  passphrase: string;
  autoLockMinutes: number;
}

export interface LibraryPrivateFolder {
  sourceId: string;
  relativePath: string;
}

export interface LibraryPrivacyRules {
  itemIds: string[];
  folders: LibraryPrivateFolder[];
}

export interface SetLibraryItemPrivacyRequest {
  itemIds: string[];
  private: boolean;
}

export interface SetLibraryFolderPrivacyRequest {
  sourceId: string;
  relativePath: string;
  private: boolean;
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
