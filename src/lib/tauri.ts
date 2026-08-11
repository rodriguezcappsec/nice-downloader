import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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
  SaveSitePresetRuleRequest,
  SitePresetRule,
  DownloadArchiveMatch,
  SaveSubscriptionRequest,
  Subscription,
  SubscriptionCheckResult,
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
    invoke<DownloadJob[]>("start_downloads", { requests }),
  cancelDownload: (jobId: string) => invoke<void>("cancel_download", { jobId }),
  retryDownload: (jobId: string) => invoke<DownloadJob>("retry_download", { jobId }),
  repeatDownload: (jobId: string) => invoke<DownloadJob>("repeat_download", { jobId }),
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
  listSubscriptions: () => invoke<Subscription[]>("list_subscriptions"),
  saveSubscription: (request: SaveSubscriptionRequest) =>
    invoke<Subscription>("save_subscription", { request }),
  deleteSubscription: (id: string) => invoke<void>("delete_subscription", { id }),
  checkSubscriptionNow: (id: string) =>
    invoke<SubscriptionCheckResult>("check_subscription_now", { id }),
  openDownloadOutput: (jobId: string, reveal: boolean) =>
    invoke<void>("open_download_output", { jobId, reveal }),
  exportDiagnostics: (path: string) => invoke<void>("export_diagnostics", { path }),
  onJobUpdated: (handler: (job: DownloadJob) => void): Promise<UnlistenFn> =>
    listen<DownloadJob>("download-job-updated", (event) => handler(event.payload)),
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
