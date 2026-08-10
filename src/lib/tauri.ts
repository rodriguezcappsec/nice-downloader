import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DownloadJob,
  EngineInfo,
  EnginePreferences,
  HistoryEntry,
  MediaAnalysis,
  StartDownloadRequest,
} from "../types";

export const api = {
  detectEngines: () => invoke<EngineInfo[]>("detect_engines"),
  getEnginePreferences: () => invoke<EnginePreferences>("get_engine_preferences"),
  updateEnginePreferences: (preferences: EnginePreferences) =>
    invoke<EnginePreferences>("update_engine_preferences", { preferences }),
  analyzeUrl: (url: string) => invoke<MediaAnalysis>("analyze_url", { url }),
  getDefaultDestination: () => invoke<string>("get_default_destination"),
  startDownload: (request: StartDownloadRequest) =>
    invoke<DownloadJob>("start_download", { request }),
  cancelDownload: (jobId: string) => invoke<void>("cancel_download", { jobId }),
  listJobs: () => invoke<DownloadJob[]>("list_jobs"),
  listHistory: () => invoke<HistoryEntry[]>("list_history"),
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
