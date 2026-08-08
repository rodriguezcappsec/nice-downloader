import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DownloadJob,
  EngineInfo,
  HistoryEntry,
  MediaAnalysis,
  StartDownloadRequest,
} from "../types";

export const api = {
  detectEngines: () => invoke<EngineInfo[]>("detect_engines"),
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

export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    const candidate = error as { message?: unknown; details?: unknown };
    if (typeof candidate.message === "string") return candidate.message;
  }
  return "An unexpected error occurred.";
}
