import type { ConfigureLibraryPrivacyRequest, LibraryPrivacyRules, LibraryPrivacyStatus, PrivacyDetectionRequest, PrivacyRedactionSuggestion, SetLibraryFolderPrivacyRequest, SetLibraryItemPrivacyRequest } from "../types";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  EmbeddedMpvBounds,
  EmbeddedMpvControlRequest,
  EmbeddedMpvSession,
  EmbeddedMpvStatus,
} from "../types";
import type {
  LibraryCompareSession,
  LibraryCurationDecision,
  LibraryCurationState,
  LibraryMediaStack,
  ExportLibraryMomentsRequest,
  LibraryMoment,
  LibraryChapterSuggestion,
  LibraryMontage,
  LibraryPlaylist,
  LibraryStoryboard,
  SaveLibraryCompareSessionRequest,
  SaveLibraryMediaStackRequest,
  SaveLibraryMomentRequest,
  SaveLibraryMontageRequest,
  SaveLibraryPlaylistRequest,
  StartLibraryClipExportRequest,
  StartLibraryMontageExportRequest,
} from "../types";
import type {
  BatchAnalysisResponse,
  DownloadJob,
  DiskSpaceInfo,
  EngineInfo,
  EngineId,
  EnginePreferences,
  EngineUpdateCandidate,
  GalleryItemList,
  HistoryEntry,
  MediaAnalysis,
  ManagedEngineStatus,
  QueueSettings,
  QueueSnapshot,
  Preset,
  SavePresetRequest,
  PostDownloadRecipe,
  PostDownloadRecipeRun,
  SavePostDownloadRecipeRequest,
  SaveSitePresetRuleRequest,
  SitePresetRule,
  DownloadArchiveMatch,
  SaveSubscriptionRequest,
  Subscription,
  SubscriptionCheckResult,
  StartTransformationRequest,
  TransformationTask,
  LibraryItem,
  LibraryRichMetadata,
  LibraryHealthSummary,
  LibraryThemeAnalysis,
  LibraryThemeIndexStatus,
  LibraryThemeJob,
  LibraryThemeProviderStatus,
  LibraryThemeSettings,
  StartLibraryThemeAnalysisRequest,
  StartLibrarySemanticIndexRequest,
  StartLibraryTranscriptionRequest,
  LibrarySearchStatus,
  LibrarySmartView,
  LibraryFacetRequest,
  LibraryFacets,
  LibraryQueryRequest,
  LibraryQueryResult,
  LibraryImageSession,
  LibraryPlaybackEntry,
  LibraryPlaybackPreflight,
  LibraryPlaybackPreparationProgress,
  LibraryPlaybackCacheStatus,
  LibraryPlaybackSession,
  LibraryPlaybackState,
  LibrarySubtitleTrack,
  LibraryTranscriptIndexStatus,
  LibraryTranscriptMatch,
  LibraryLocalInferenceJob,
  LibrarySemanticIndexStatus,
  LibrarySemanticTranscriptMatch,
  LibraryTranscriptionProviderStatus,
  LibraryVisualSimilarityJob,
  LibraryVisualSimilarityMatch,
  LibraryVisualSimilarityStatus,
  LibraryNearDuplicateMatch,
  LocalShareSession,
  NativePlaybackSession,
  NativePlaybackStatus,
  LibraryWatchState,
  SaveLibraryPlaybackRequest,
  LibraryCacheResult,
  LibraryBulkAction,
  LibraryBulkMovePlan,
  LibraryBulkMoveResult,
  LibraryArtworkCacheStatus,
  LibraryStoragePlan,
  LibraryChange,
  LibraryDuplicateAnalysisJob,
  LibraryDuplicateGroup,
  LibraryDuplicateTrashResult,
  LibraryFileOperation,
  LibraryFileOperationPlan,
  LibraryFileOperationResult,
  LibraryCollection,
  LibraryScanJob,
  LibraryScanResult,
  LibrarySource,
  LibrarySourceAutomation,
  SaveLibrarySourceRequest,
  SaveLibrarySourceAutomationRequest,
  UpdateLibraryItemRequest,
  SaveLibraryRichMetadataRequest,
  SaveLibrarySmartViewRequest,
  SetLibraryArtworkRequest,
  SaveLibraryCollectionRequest,
  PreviewLibraryFileOperationRequest,
  PreviewLibraryBulkMoveRequest,
  AuthenticationProfile,
  SaveAuthenticationProfileRequest,
  BrowserProfileInfo,
  DesktopPreferences,
  DesktopRuntimeStatus,
  BatchDownloadResult,
  OnboardingStatus,
  StartDownloadRequest,
} from "../types";

export const api = {
  detectEngines: () => invoke<EngineInfo[]>("detect_engines"),
  listManagedEngines: () => invoke<ManagedEngineStatus[]>("list_managed_engines"),
  checkEngineUpdate: (engine: EngineId) =>
    invoke<EngineUpdateCandidate>("check_engine_update", { engine }),
  installEngineUpdate: (candidate: EngineUpdateCandidate) =>
    invoke<ManagedEngineStatus>("install_engine_update", { candidate }),
  pinManagedEngine: (engine: EngineId, pinned: boolean) =>
    invoke<ManagedEngineStatus>("pin_managed_engine", { engine, pinned }),
  rollbackManagedEngine: (engine: EngineId) =>
    invoke<ManagedEngineStatus>("rollback_managed_engine", { engine }),
  testSiteAccess: (url: string) => invoke<MediaAnalysis>("test_site_access", { url }),
  getEnginePreferences: () => invoke<EnginePreferences>("get_engine_preferences"),
  updateEnginePreferences: (preferences: EnginePreferences) =>
    invoke<EnginePreferences>("update_engine_preferences", { preferences }),
  analyzeUrl: (url: string) => invoke<MediaAnalysis>("analyze_url", { url }),
  listGalleryItems: (url: string, offset: number, limit: number) =>
    invoke<GalleryItemList>("list_gallery_items", { url, offset, limit }),
  analyzeUrls: (urls: string[]) => invoke<BatchAnalysisResponse>("analyze_urls", { urls }),
  analyzePlaylist: (url: string) => invoke<BatchAnalysisResponse>("analyze_playlist", { url }),
  getDefaultDestination: () => invoke<string>("get_default_destination"),
  startDownload: (request: StartDownloadRequest) =>
    invoke<DownloadJob>("start_download", { request }),
  startDownloads: (requests: StartDownloadRequest[]) =>
    invoke<BatchDownloadResult>("start_downloads", { requests }),
  cancelDownload: (jobId: string) => invoke<void>("cancel_download", { jobId }),
  dismissDownloads: (jobIds: string[]) => invoke<string[]>("dismiss_downloads", { jobIds }),
  retryDownload: (jobId: string) => invoke<DownloadJob>("retry_download", { jobId }),
  repeatDownload: (jobId: string) => invoke<DownloadJob>("repeat_download", { jobId }),
  retryFailedGalleryFiles: (jobId: string) => invoke<DownloadJob>("retry_failed_gallery_files", { jobId }),
  pauseDownload: (jobId: string) => invoke<void>("pause_download", { jobId }),
  resumeDownload: (jobId: string) => invoke<void>("resume_download", { jobId }),
  getQueueSnapshot: () => invoke<QueueSnapshot>("get_queue_snapshot"),
  updateQueueSettings: (settings: QueueSettings) =>
    invoke<QueueSnapshot>("update_queue_settings", { settings }),
  moveQueuedDownload: (jobId: string, newIndex: number) =>
    invoke<QueueSnapshot>("move_queued_download", { jobId, newIndex }),
  checkDiskSpace: (destination: string, requiredBytes?: number | null) =>
    invoke<DiskSpaceInfo>("check_disk_space", { destination, requiredBytes }),
  listJobs: () => invoke<DownloadJob[]>("list_jobs"),
  listHistory: () => invoke<HistoryEntry[]>("list_history"),
  deleteHistoryEntry: (id: string) => invoke<void>("delete_history_entry", { id }),
  listPostDownloadRecipes: () => invoke<PostDownloadRecipe[]>("list_post_download_recipes"),
  savePostDownloadRecipe: (request: SavePostDownloadRecipeRequest) =>
    invoke<PostDownloadRecipe>("save_post_download_recipe", { request }),
  deletePostDownloadRecipe: (id: string) => invoke<void>("delete_post_download_recipe", { id }),
  listPostDownloadRecipeRuns: (limit = 50) =>
    invoke<PostDownloadRecipeRun[]>("list_post_download_recipe_runs", { limit }),
  listPresets: () => invoke<Preset[]>("list_presets"),
  savePreset: (request: SavePresetRequest) => invoke<Preset>("save_preset", { request }),
  deletePreset: (id: string) => invoke<void>("delete_preset", { id }),
  listSitePresetRules: () => invoke<SitePresetRule[]>("list_site_preset_rules"),
  saveSitePresetRule: (request: SaveSitePresetRuleRequest) =>
    invoke<SitePresetRule>("save_site_preset_rule", { request }),
  deleteSitePresetRule: (id: string) => invoke<void>("delete_site_preset_rule", { id }),
  matchSitePresetRule: (url: string) =>
    invoke<SitePresetRule | null>("match_site_preset_rule", { url }),
  findDownloadArchiveMatch: (url: string, destination: string) =>
    invoke<DownloadArchiveMatch | null>("find_download_archive_match", { url, destination }),
  findDownloadArchiveMatches: (urls: string[], destination: string) =>
    invoke<string[]>("find_download_archive_matches", { urls, destination }),
  prepareCollectionDestination: (destination: string, collectionTitle: string) =>
    invoke<string>("prepare_collection_destination", { destination, collectionTitle }),
  listSubscriptions: () => invoke<Subscription[]>("list_subscriptions"),
  saveSubscription: (request: SaveSubscriptionRequest) =>
    invoke<Subscription>("save_subscription", { request }),
  deleteSubscription: (id: string) => invoke<void>("delete_subscription", { id }),
  checkSubscriptionNow: (id: string) =>
    invoke<SubscriptionCheckResult>("check_subscription_now", { id }),
  listTransformationTasks: () => invoke<TransformationTask[]>("list_transformation_tasks"),
  startTransformation: (request: StartTransformationRequest) =>
    invoke<TransformationTask>("start_transformation", { request }),
  startLibraryOptimization: (itemIds: string[], destinationDirectory: string, overwrite = false) =>
    invoke<TransformationTask[]>("start_library_optimization", { itemIds, destinationDirectory, overwrite }),
  cancelTransformation: (id: string) => invoke<void>("cancel_transformation", { id }),
  suggestPrivacyRedactions: (request: PrivacyDetectionRequest) => invoke<PrivacyRedactionSuggestion[]>("suggest_privacy_redactions", { request }),
  listLibraryItems: () => invoke<LibraryItem[]>("list_library_items"),
  queryLibraryItems: (request: LibraryQueryRequest) =>
    invoke<LibraryQueryResult>("query_library_items", { request }),
  getLibraryHealthSummary: () => invoke<LibraryHealthSummary>("get_library_health_summary"),
  getLibraryStoragePlan: () => invoke<LibraryStoragePlan>("get_library_storage_plan"),
  getLibraryFacets: (request: LibraryFacetRequest) =>
    invoke<LibraryFacets>("get_library_facets", { request }),
  getLibraryItemsByIds: (ids: string[]) =>
    invoke<LibraryItem[]>("get_library_items_by_ids", { ids }),
  searchLibraryTranscripts: (search: string, limit = 12) =>
    invoke<LibraryTranscriptMatch[]>("search_library_transcripts", { search, limit }),
  getLibraryTranscriptIndexStatus: () =>
    invoke<LibraryTranscriptIndexStatus>("get_library_transcript_index_status"),
  rebuildLibraryTranscriptIndex: () =>
    invoke<LibraryTranscriptIndexStatus>("rebuild_library_transcript_index"),
  getLibraryTranscriptionProviderStatus: () =>
    invoke<LibraryTranscriptionProviderStatus>("get_library_transcription_provider_status"),
  getLibraryLocalInferenceJob: () =>
    invoke<LibraryLocalInferenceJob | null>("get_library_local_inference_job"),
  startLibraryTranscription: (request: StartLibraryTranscriptionRequest) =>
    invoke<LibraryLocalInferenceJob>("start_library_transcription", { request }),
  startLibrarySemanticTranscriptIndex: (request: StartLibrarySemanticIndexRequest) =>
    invoke<LibraryLocalInferenceJob>("start_library_semantic_transcript_index", { request }),
  cancelLibraryLocalInferenceJob: (jobId: string) =>
    invoke<void>("cancel_library_local_inference_job", { jobId }),
  getLibrarySemanticTranscriptIndexStatus: (model: string) =>
    invoke<LibrarySemanticIndexStatus>("get_library_semantic_transcript_index_status", { model }),
  searchLibrarySemanticTranscripts: (search: string, model: string, limit = 12) =>
    invoke<LibrarySemanticTranscriptMatch[]>("search_library_semantic_transcripts", { search, model, limit }),
  getLibraryVisualSimilarityStatus: () =>
    invoke<LibraryVisualSimilarityStatus>("get_library_visual_similarity_status"),
  getLibraryVisualSimilarityJob: () =>
    invoke<LibraryVisualSimilarityJob | null>("get_library_visual_similarity_job"),
  startLibraryVisualSimilarityIndex: () =>
    invoke<LibraryVisualSimilarityJob>("start_library_visual_similarity_index"),
  cancelLibraryVisualSimilarityIndex: (jobId: string) =>
    invoke<void>("cancel_library_visual_similarity_index", { jobId }),
  findVisuallySimilarLibraryItems: (itemId: string, limit = 24, minimumSimilarityPercent = 75) =>
    invoke<LibraryVisualSimilarityMatch[]>("find_visually_similar_library_items", { itemId, limit, minimumSimilarityPercent }),
  findLibraryNearDuplicateVideos: (minimumVisualSimilarityPercent = 85, maximumDurationDeltaSeconds = 5, limit = 100) =>
    invoke<LibraryNearDuplicateMatch[]>("find_library_near_duplicate_videos", {
      minimumVisualSimilarityPercent,
      maximumDurationDeltaSeconds,
      limit,
    }),
  getLibraryThemeSettings: () => invoke<LibraryThemeSettings>("get_library_theme_settings"),
  saveLibraryThemeSettings: (settings: LibraryThemeSettings) =>
    invoke<LibraryThemeSettings>("save_library_theme_settings", { settings }),
  getLibraryThemeProviderStatus: () =>
    invoke<LibraryThemeProviderStatus>("get_library_theme_provider_status"),
  getLibraryThemeIndexStatus: () =>
    invoke<LibraryThemeIndexStatus>("get_library_theme_index_status"),
  listLibraryThemeAnalyses: (limit = 50) =>
    invoke<LibraryThemeAnalysis[]>("list_library_theme_analyses", { limit }),
  getLibraryThemeJob: () => invoke<LibraryThemeJob | null>("get_library_theme_job"),
  startLibraryThemeAnalysis: (request: StartLibraryThemeAnalysisRequest) =>
    invoke<LibraryThemeJob>("start_library_theme_analysis", { request }),
  cancelLibraryThemeAnalysis: (jobId: string) =>
    invoke<void>("cancel_library_theme_analysis", { jobId }),
  getLocalShareStatus: () => invoke<LocalShareSession | null>("get_local_share_status"),
  startLocalShare: (itemIds: string[], expiresMinutes: number) =>
    invoke<LocalShareSession>("start_local_share", { request: { itemIds, expiresMinutes } }),
  stopLocalShare: (id: string) => invoke<void>("stop_local_share", { id }),
  prepareLibraryImageView: (id: string) =>
    invoke<LibraryImageSession>("prepare_library_image_view", { id }),
  inspectLibraryPlayback: (id: string) =>
    invoke<LibraryPlaybackPreflight>("inspect_library_playback", { id }),
  inspectLibraryPlaybackFallback: (id: string) =>
    invoke<LibraryPlaybackPreflight>("inspect_library_playback_fallback", { id }),
  prepareLibraryPlayback: (id: string, userApproved = false) =>
    invoke<LibraryPlaybackSession>("prepare_library_playback", { id, userApproved }),
  prepareLibraryLivePlayback: (id: string, startSeconds = 0) =>
    invoke<LibraryPlaybackSession>("prepare_library_live_playback", { id, startSeconds }),
  prepareLibraryHlsPlayback: (id: string, startSeconds = 0) =>
    invoke<LibraryPlaybackSession>("prepare_library_hls_playback", { id, startSeconds }),
  prepareLibraryPlaybackFallback: (id: string, userApproved = false) =>
    invoke<LibraryPlaybackSession>("prepare_library_playback_fallback", { id, userApproved }),
  getLibraryPlaybackPreparation: (id: string) =>
    invoke<LibraryPlaybackPreparationProgress | null>("get_library_playback_preparation", { id }),
  cancelLibraryPlaybackPreparation: (id: string) =>
    invoke<void>("cancel_library_playback_preparation", { id }),
  releaseLibraryPlayback: (id: string) => invoke<void>("release_library_playback", { id }),
  saveLibraryPlaybackProgress: (request: SaveLibraryPlaybackRequest) =>
    invoke<LibraryPlaybackState>("save_library_playback_progress", { request }),
  listContinueWatching: (limit = 12) =>
    invoke<LibraryPlaybackEntry[]>("list_continue_watching", { limit }),
  getLibraryWatchState: (id: string) =>
    invoke<LibraryWatchState>("get_library_watch_state", { id }),
  setLibraryWatchLater: (id: string, watchLater: boolean) =>
    invoke<LibraryWatchState>("set_library_watch_later", { id, watchLater }),
  setLibraryWatched: (id: string, watched: boolean) =>
    invoke<LibraryWatchState>("set_library_watched", { id, watched }),
  listLibraryWatchLater: (limit = 12) =>
    invoke<LibraryItem[]>("list_library_watch_later", { limit }),
  listRecentlyWatched: (limit = 12) =>
    invoke<LibraryPlaybackEntry[]>("list_recently_watched", { limit }),
  importLibraryFiles: (paths: string[], tags: string[]) => invoke<LibraryItem[]>("import_library_files", { paths, tags }),
  updateLibraryItem: (request: UpdateLibraryItemRequest) => invoke<LibraryItem>("update_library_item", { request }),
  getLibraryRichMetadata: (itemId: string) => invoke<LibraryRichMetadata>("get_library_rich_metadata", { itemId }),
  saveLibraryRichMetadata: (request: SaveLibraryRichMetadataRequest) => invoke<LibraryRichMetadata>("save_library_rich_metadata", { request }),
  resetLibraryRichMetadata: (itemId: string) => invoke<LibraryRichMetadata>("reset_library_rich_metadata", { itemId }),
  setLibraryArtwork: (request: SetLibraryArtworkRequest) => invoke<LibraryItem>("set_library_artwork", { request }),
  getLibrarySearchStatus: () => invoke<LibrarySearchStatus>("get_library_search_status"),
  rebuildLibrarySearchIndex: () => invoke<LibrarySearchStatus>("rebuild_library_search_index"),
  listLibrarySmartViews: () => invoke<LibrarySmartView[]>("list_library_smart_views"),
  saveLibrarySmartView: (request: SaveLibrarySmartViewRequest) => invoke<LibrarySmartView>("save_library_smart_view", { request }),
  deleteLibrarySmartView: (id: string) => invoke<void>("delete_library_smart_view", { id }),
  getRelatedLibraryItems: (itemId: string, limit = 8) => invoke<LibraryItem[]>("get_related_library_items", { itemId, limit }),
  listLibraryCollections: () => invoke<LibraryCollection[]>("list_library_collections"),
  saveLibraryCollection: (request: SaveLibraryCollectionRequest) => invoke<LibraryCollection>("save_library_collection", { request }),
  deleteLibraryCollection: (id: string) => invoke<void>("delete_library_collection", { id }),
  listLibraryPlaylists: () => invoke<LibraryPlaylist[]>("list_library_playlists"),
  saveLibraryPlaylist: (request: SaveLibraryPlaylistRequest) =>
    invoke<LibraryPlaylist>("save_library_playlist", { request }),
  deleteLibraryPlaylist: (id: string) => invoke<void>("delete_library_playlist", { id }),
  addLibraryPlaylistItem: (playlistId: string, itemId: string) =>
    invoke<LibraryPlaylist>("add_library_playlist_item", { playlistId, itemId }),
  removeLibraryPlaylistItem: (playlistId: string, itemId: string) =>
    invoke<LibraryPlaylist>("remove_library_playlist_item", { playlistId, itemId }),
  moveLibraryPlaylistItem: (playlistId: string, itemId: string, newIndex: number) =>
    invoke<LibraryPlaylist>("move_library_playlist_item", { playlistId, itemId, newIndex }),
  listLibraryPlaylistItems: (playlistId: string) =>
    invoke<LibraryItem[]>("list_library_playlist_items", { playlistId }),
  listLibraryMoments: (itemId: string) =>
    invoke<LibraryMoment[]>("list_library_moments", { itemId }),
  suggestLibraryChapters: (itemId: string, maximumSuggestions = 16) =>
    invoke<LibraryChapterSuggestion[]>("suggest_library_chapters", { itemId, maximumSuggestions }),
  saveLibraryMoment: (request: SaveLibraryMomentRequest) =>
    invoke<LibraryMoment>("save_library_moment", { request }),
  deleteLibraryMoment: (id: string) => invoke<void>("delete_library_moment", { id }),
  listLibraryMontages: () => invoke<LibraryMontage[]>("list_library_montages"),
  saveLibraryMontage: (request: SaveLibraryMontageRequest) =>
    invoke<LibraryMontage>("save_library_montage", { request }),
  deleteLibraryMontage: (id: string) => invoke<void>("delete_library_montage", { id }),
  startLibraryMontageExport: (request: StartLibraryMontageExportRequest) =>
    invoke<TransformationTask>("start_library_montage_export", { request }),
  listLibraryCompareSessions: () =>
    invoke<LibraryCompareSession[]>("list_library_compare_sessions"),
  saveLibraryCompareSession: (request: SaveLibraryCompareSessionRequest) =>
    invoke<LibraryCompareSession>("save_library_compare_session", { request }),
  deleteLibraryCompareSession: (id: string) =>
    invoke<void>("delete_library_compare_session", { id }),
  listLibraryMediaStacks: () =>
    invoke<LibraryMediaStack[]>("list_library_media_stacks"),
  saveLibraryMediaStack: (request: SaveLibraryMediaStackRequest) =>
    invoke<LibraryMediaStack>("save_library_media_stack", { request }),
  deleteLibraryMediaStack: (id: string) =>
    invoke<void>("delete_library_media_stack", { id }),
  listLibraryCurationQueue: (reviewLater = false, limit = 100) =>
    invoke<LibraryItem[]>("list_library_curation_queue", { reviewLater, limit }),
  getLibraryCurationState: (itemId: string) =>
    invoke<LibraryCurationState>("get_library_curation_state", { itemId }),
  setLibraryCurationDecision: (itemId: string, decision: LibraryCurationDecision | null) =>
    invoke<LibraryCurationState>("set_library_curation_decision", { itemId, decision }),
  exportLibraryMoments: (request: ExportLibraryMomentsRequest) =>
    invoke<number>("export_library_moments", { request }),
  generateLibraryStoryboard: (itemId: string, frameCount = 12) =>
    invoke<LibraryStoryboard>("generate_library_storyboard", { itemId, frameCount }),
  loadLibraryEmbeddedSubtitles: (itemId: string) =>
    invoke<LibrarySubtitleTrack[]>("load_library_embedded_subtitles", { itemId }),
  startLibraryClipExport: (request: StartLibraryClipExportRequest) =>
    invoke<TransformationTask>("start_library_clip_export", { request }),
  applyLibraryBulkAction: (ids: string[], action: LibraryBulkAction) => invoke<LibraryItem[]>("apply_library_bulk_action", { ids, action }),
  listLibraryChanges: () => invoke<LibraryChange[]>("list_library_changes"),
  reviewLibraryChanges: (ids: string[]) => invoke<number>("review_library_changes", { ids }),
  getLibraryDuplicateAnalysis: () => invoke<LibraryDuplicateAnalysisJob | null>("get_library_duplicate_analysis"),
  listLibraryDuplicateGroups: () => invoke<LibraryDuplicateGroup[]>("list_library_duplicate_groups"),
  startLibraryDuplicateAnalysis: () => invoke<LibraryDuplicateAnalysisJob>("start_library_duplicate_analysis"),
  cancelLibraryDuplicateAnalysis: (jobId: string) => invoke<void>("cancel_library_duplicate_analysis", { jobId }),
  trashLibraryDuplicateItem: (groupId: string, itemId: string) => invoke<LibraryDuplicateTrashResult>("trash_library_duplicate_item", { groupId, itemId }),
  previewLibraryFileOperation: (request: PreviewLibraryFileOperationRequest) => invoke<LibraryFileOperationPlan>("preview_library_file_operation", { request }),
  executeLibraryFileOperation: (operationId: string) => invoke<LibraryFileOperationResult>("execute_library_file_operation", { operationId }),
  previewLibraryBulkMove: (request: PreviewLibraryBulkMoveRequest) => invoke<LibraryBulkMovePlan>("preview_library_bulk_move", { request }),
  executeLibraryBulkMove: (operationIds: string[]) => invoke<LibraryBulkMoveResult>("execute_library_bulk_move", { operationIds }),
  undoLibraryFileOperation: (operationId: string) => invoke<LibraryFileOperationResult>("undo_library_file_operation", { operationId }),
  listLibraryFileOperations: () => invoke<LibraryFileOperation[]>("list_library_file_operations"),
  removeLibraryItem: (id: string) => invoke<void>("remove_library_item", { id }),
  getLibraryArtwork: (id: string) => invoke<string | null>("get_library_artwork", { id }),
  clearLibraryArtworkCache: () => invoke<LibraryCacheResult>("clear_library_artwork_cache"),
  getLibraryArtworkCacheStatus: () => invoke<LibraryArtworkCacheStatus>("get_library_artwork_cache_status"),
  clearLibraryPlaybackCache: () => invoke<LibraryCacheResult>("clear_library_playback_cache"),
  getNativePlaybackStatus: () => invoke<NativePlaybackStatus>("get_native_playback_status"),
  openLibraryNativePlayback: (id: string, startAtSeconds?: number) =>
    invoke<NativePlaybackSession>("open_library_native_playback", { id, startAtSeconds }),
  closeLibraryNativePlayback: () => invoke<void>("close_library_native_playback"),
  getEmbeddedMpvStatus: () => invoke<EmbeddedMpvStatus>("get_embedded_mpv_status"),
  openLibraryEmbeddedMpv: (id: string, bounds: EmbeddedMpvBounds, startAtSeconds?: number) =>
    invoke<EmbeddedMpvSession>("open_library_embedded_mpv", { id, bounds, startAtSeconds }),
  updateEmbeddedMpvBounds: (bounds: EmbeddedMpvBounds) =>
    invoke<void>("update_embedded_mpv_bounds", { bounds }),
  controlEmbeddedMpv: (request: EmbeddedMpvControlRequest) =>
    invoke<void>("control_embedded_mpv", { request }),
  closeEmbeddedMpv: () => invoke<void>("close_embedded_mpv"),
  getLibraryPlaybackCacheStatus: () => invoke<LibraryPlaybackCacheStatus>("get_library_playback_cache_status"),
  openLibraryItem: (id: string) => invoke<void>("open_library_item", { id }),
  reconcileLibrary: () => invoke<LibraryScanResult>("reconcile_library"),
  exportLibrary: (path: string) => invoke<void>("export_library", { path }),
  restoreLibrary: (path: string) => invoke<LibraryItem[]>("restore_library", { path }),
  getLibraryPrivacyStatus: () => invoke<LibraryPrivacyStatus>("get_library_privacy_status"),
  configureLibraryPrivacy: (request: ConfigureLibraryPrivacyRequest) => invoke<LibraryPrivacyStatus>("configure_library_privacy", { request }),
  unlockLibraryPrivacy: (passphrase: string) => invoke<LibraryPrivacyStatus>("unlock_library_privacy", { passphrase }),
  lockLibraryPrivacy: () => invoke<LibraryPrivacyStatus>("lock_library_privacy"),
  touchLibraryPrivacy: () => invoke<void>("touch_library_privacy"),
  onLibraryPrivacyChanged: (callback: (status: LibraryPrivacyStatus) => void) =>
    listen<LibraryPrivacyStatus>("library-privacy-changed", (event) => callback(event.payload)),
  listLibraryPrivacyRules: () => invoke<LibraryPrivacyRules>("list_library_privacy_rules"),
  setLibraryItemsPrivate: (request: SetLibraryItemPrivacyRequest) => invoke<LibraryPrivacyRules>("set_library_items_private", { request }),
  setLibraryFolderPrivate: (request: SetLibraryFolderPrivacyRequest) => invoke<LibraryPrivacyRules>("set_library_folder_private", { request }),
  listLibrarySources: () => invoke<LibrarySource[]>("list_library_sources"),
  saveLibrarySource: (request: SaveLibrarySourceRequest) => invoke<LibrarySource>("save_library_source", { request }),
  getLibrarySourceAutomation: (sourceId: string) => invoke<LibrarySourceAutomation>("get_library_source_automation", { sourceId }),
  saveLibrarySourceAutomation: (request: SaveLibrarySourceAutomationRequest) => invoke<LibrarySourceAutomation>("save_library_source_automation", { request }),
  removeLibrarySource: (id: string) => invoke<void>("remove_library_source", { id }),
  listLibraryScanJobs: () => invoke<LibraryScanJob[]>("list_library_scan_jobs"),
  startLibraryScan: (sourceId: string) => invoke<LibraryScanJob>("start_library_scan", { sourceId }),
  cancelLibraryScan: (jobId: string) => invoke<void>("cancel_library_scan", { jobId }),
  listAuthenticationProfiles: () => invoke<AuthenticationProfile[]>("list_authentication_profiles"),
  saveAuthenticationProfile: (request: SaveAuthenticationProfileRequest) => invoke<AuthenticationProfile>("save_authentication_profile", { request }),
  deleteAuthenticationProfile: (id: string) => invoke<void>("delete_authentication_profile", { id }),
  discoverBrowserProfiles: () => invoke<BrowserProfileInfo[]>("discover_browser_profiles"),
  getDesktopPreferences: () => invoke<DesktopPreferences>("get_desktop_preferences"),
  getDesktopRuntimeStatus: () => invoke<DesktopRuntimeStatus>("get_desktop_runtime_status"),
  updateDesktopPreferences: (preferences: DesktopPreferences) => invoke<DesktopPreferences>("update_desktop_preferences", { preferences }),
  hideToTray: () => invoke<void>("hide_to_tray"),
  quitApp: () => invoke<void>("quit_app"),
  getOnboardingStatus: () => invoke<OnboardingStatus>("get_onboarding_status"),
  completeOnboarding: () => invoke<void>("complete_onboarding"),
  openDownloadOutput: (jobId: string, reveal: boolean) =>
    invoke<void>("open_download_output", { jobId, reveal }),
  exportDiagnostics: (path: string) => invoke<void>("export_diagnostics", { path }),
  onJobUpdated: (handler: (job: DownloadJob) => void): Promise<UnlistenFn> =>
    listen<DownloadJob>("download-job-updated", (event) => handler(event.payload)),
  onJobRemoved: (handler: (jobId: string) => void): Promise<UnlistenFn> =>
    listen<string>("download-job-removed", (event) => handler(event.payload)),
  onQueueUpdated: (handler: (queue: QueueSnapshot) => void): Promise<UnlistenFn> =>
    listen<QueueSnapshot>("download-queue-updated", (event) => handler(event.payload)),
  onTransformationUpdated: (handler: (task: TransformationTask) => void): Promise<UnlistenFn> =>
    listen<TransformationTask>("transformation-task-updated", (event) => handler(event.payload)),
  onLibraryVisualSimilarityUpdated: (handler: (job: LibraryVisualSimilarityJob) => void): Promise<UnlistenFn> =>
    listen<LibraryVisualSimilarityJob>("library-visual-similarity-updated", (event) => handler(event.payload)),
  onLibraryThemeAnalysisUpdated: (handler: (job: LibraryThemeJob) => void): Promise<UnlistenFn> =>
    listen<LibraryThemeJob>("library-theme-analysis-updated", (event) => handler(event.payload)),
  onLibraryLocalInferenceUpdated: (handler: (job: LibraryLocalInferenceJob) => void): Promise<UnlistenFn> =>
    listen<LibraryLocalInferenceJob>("library-local-inference-updated", (event) => handler(event.payload)),
  onPostDownloadRecipeRunUpdated: (handler: (run: PostDownloadRecipeRun) => void): Promise<UnlistenFn> =>
    listen<PostDownloadRecipeRun>("post-download-recipe-run-updated", (event) => handler(event.payload)),
  onLibraryScanUpdated: (handler: (job: LibraryScanJob) => void): Promise<UnlistenFn> =>
    listen<LibraryScanJob>("library-scan-updated", (event) => handler(event.payload)),
  onLibraryItemIndexed: (handler: (item: LibraryItem) => void): Promise<UnlistenFn> =>
    listen<LibraryItem>("library-item-indexed", (event) => handler(event.payload)),
  onLibrarySourceUpdated: (handler: (source: LibrarySource) => void): Promise<UnlistenFn> =>
    listen<LibrarySource>("library-source-updated", (event) => handler(event.payload)),
  onLibraryChangeUpdated: (handler: (change: LibraryChange) => void): Promise<UnlistenFn> =>
    listen<LibraryChange>("library-change-updated", (event) => handler(event.payload)),
  onLibraryDuplicateAnalysisUpdated: (handler: (job: LibraryDuplicateAnalysisJob) => void): Promise<UnlistenFn> =>
    listen<LibraryDuplicateAnalysisJob>("library-duplicate-analysis-updated", (event) => handler(event.payload)),
};

export function normalizeError(error: unknown): import("../types").AppError {
  if (typeof error === "string") {
    return { code: "unknown_error", message: error };
  }
  if (error && typeof error === "object") {
    const candidate = error as { code?: unknown; message?: unknown; details?: unknown };
    if (typeof candidate.message === "string") {
      return {
        code: typeof candidate.code === "string" ? candidate.code : "unknown_error",
        message: candidate.message,
        details: typeof candidate.details === "string" ? candidate.details : null,
      };
    }
  }
  return { code: "unknown_error", message: "An unexpected error occurred." };
}
