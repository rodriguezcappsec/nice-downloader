import { create } from "zustand";
import { api, normalizeError } from "../lib/tauri";
import type {
  AppError,
  BatchAnalysisResponse,
  DownloadJob,
  EngineInfo,
  EnginePreferences,
  GalleryItemList,
  HistoryEntry,
  MediaAnalysis,
  QueueSettings,
  QueueSnapshot,
  StartDownloadRequest,
  ViewId,
} from "../types";

interface AppStore {
  view: ViewId;
  engines: EngineInfo[];
  enginePreferences: EnginePreferences;
  jobs: Record<string, DownloadJob>;
  history: HistoryEntry[];
  queue: QueueSnapshot;
  analysis: MediaAnalysis | null;
  galleryItems: GalleryItemList | null;
  batchAnalysis: BatchAnalysisResponse | null;
  analyzing: boolean;
  galleryItemsLoading: boolean;
  engineLoading: boolean;
  historyLoading: boolean;
  error: AppError | null;
  notice: { message: string; view?: ViewId } | null;
  defaultDestination: string;
  setView: (view: ViewId) => void;
  setError: (error: AppError | null) => void;
  setNotice: (notice: AppStore["notice"]) => void;
  setEnginePreferences: (preferences: EnginePreferences) => void;
  saveEnginePreferences: () => Promise<boolean>;
  analyze: (url: string) => Promise<MediaAnalysis | null>;
  loadGalleryItems: (url: string, reset?: boolean) => Promise<GalleryItemList | null>;
  analyzeBatch: (urls: string[]) => Promise<BatchAnalysisResponse | null>;
  analyzePlaylist: (url: string) => Promise<BatchAnalysisResponse | null>;
  clearAnalysis: () => void;
  detectEngines: () => Promise<void>;
  loadHistory: () => Promise<void>;
  initialize: () => Promise<() => void>;
  startDownload: (request: StartDownloadRequest) => Promise<DownloadJob | null>;
  startDownloads: (requests: StartDownloadRequest[]) => Promise<DownloadJob[]>;
  cancelDownload: (jobId: string) => Promise<void>;
  dismissDownloads: (jobIds: string[]) => Promise<string[]>;
  retryDownload: (jobId: string) => Promise<void>;
  repeatDownload: (jobId: string) => Promise<void>;
  pauseDownload: (jobId: string) => Promise<void>;
  resumeDownload: (jobId: string) => Promise<void>;
  updateQueueSettings: (settings: QueueSettings) => Promise<void>;
  moveQueuedDownload: (jobId: string, newIndex: number) => Promise<void>;
}

const savedView = typeof window !== "undefined" ? window.localStorage.getItem("nice-downloader.view") as ViewId | null : null;
let latestAnalysisRequest = "";

export const useAppStore = create<AppStore>((set, get) => ({
  view: savedView && ["new", "downloads", "queue", "history", "subscriptions", "transformations", "library", "presets", "settings"].includes(savedView) ? savedView : "library",
  engines: [],
  enginePreferences: {
    browserCookieSource: "none",
    browserProfile: null,
    proxyUrl: null,
    enableImpersonation: true,
    bandwidthLimitKib: null,
  },
  jobs: {},
  history: [],
  queue: {
    settings: { concurrentDownloads: 2, paused: false, notifyOnCompletion: true, completionAction: "none" },
    orderedJobIds: [],
    activeJobIds: [],
    pauseSupported: true,
    completionNotificationsSupported: true,
  },
  analysis: null,
  galleryItems: null,
  batchAnalysis: null,
  analyzing: false,
  galleryItemsLoading: false,
  engineLoading: false,
  historyLoading: false,
  error: null,
  notice: null,
  defaultDestination: "",

  setView: (view) => {
    window.localStorage.setItem("nice-downloader.view", view);
    set({ view });
  },
  setError: (error) => set({ error }),
  setNotice: (notice) => set({ notice }),
  setEnginePreferences: (enginePreferences) => set({ enginePreferences }),

  saveEnginePreferences: async () => {
    try {
      const enginePreferences = await api.updateEnginePreferences(get().enginePreferences);
      set({ enginePreferences, error: null });
      return true;
    } catch (error) {
      set({ error: normalizeError(error) });
      return false;
    }
  },

  analyze: async (url) => {
    const requestId = crypto.randomUUID();
    latestAnalysisRequest = requestId;
    set({ analyzing: true, error: null, analysis: null, batchAnalysis: null, galleryItems: null });
    try {
      const analysis = await api.analyzeUrl(url);
      if (latestAnalysisRequest !== requestId) return null;
      set({ analysis });
      return analysis;
    } catch (error) {
      if (latestAnalysisRequest !== requestId) return null;
      set({ error: normalizeError(error) });
      return null;
    } finally {
      if (latestAnalysisRequest === requestId) set({ analyzing: false });
    }
  },

  loadGalleryItems: async (url, reset = false) => {
    const current = get().galleryItems;
    const offset = reset ? 0 : (current?.nextOffset ?? 0);
    if (!reset && current && current.nextOffset == null) return current;
    set({ galleryItemsLoading: true, error: null });
    try {
      const page = await api.listGalleryItems(url, offset, 12);
      const galleryItems = reset || !current
        ? page
        : {
            ...page,
            offset: 0,
            items: [...current.items, ...page.items.filter((item) =>
              !current.items.some((existing) => existing.index === item.index),
            )],
          };
      set({ galleryItems });
      return galleryItems;
    } catch (error) {
      set({ error: normalizeError(error) });
      return null;
    } finally {
      set({ galleryItemsLoading: false });
    }
  },

  analyzeBatch: async (urls) => {
    const requestId = crypto.randomUUID();
    latestAnalysisRequest = requestId;
    set({ analyzing: true, error: null, analysis: null, batchAnalysis: null, galleryItems: null });
    try {
      const batchAnalysis = await api.analyzeUrls(urls);
      if (latestAnalysisRequest !== requestId) return null;
      set({ batchAnalysis });
      return batchAnalysis;
    } catch (error) {
      if (latestAnalysisRequest !== requestId) return null;
      set({ error: normalizeError(error) });
      return null;
    } finally {
      if (latestAnalysisRequest === requestId) set({ analyzing: false });
    }
  },

  analyzePlaylist: async (url) => {
    const requestId = crypto.randomUUID();
    latestAnalysisRequest = requestId;
    set({ analyzing: true, error: null, analysis: null, batchAnalysis: null, galleryItems: null });
    try {
      const batchAnalysis = await api.analyzePlaylist(url);
      if (latestAnalysisRequest !== requestId) return null;
      set({ batchAnalysis });
      return batchAnalysis;
    } catch (error) {
      if (latestAnalysisRequest !== requestId) return null;
      set({ error: normalizeError(error) });
      return null;
    } finally {
      if (latestAnalysisRequest === requestId) set({ analyzing: false });
    }
  },

  clearAnalysis: () => {
    latestAnalysisRequest = crypto.randomUUID();
    set({ analysis: null, batchAnalysis: null, galleryItems: null, analyzing: false, error: null });
  },

  detectEngines: async () => {
    set({ engineLoading: true });
    try {
      set({ engines: await api.detectEngines() });
    } catch (error) {
      set({ error: normalizeError(error) });
    } finally {
      set({ engineLoading: false });
    }
  },

  loadHistory: async () => {
    set({ historyLoading: true });
    try {
      set({ history: await api.listHistory() });
    } catch (error) {
      set({ error: normalizeError(error) });
    } finally {
      set({ historyLoading: false });
    }
  },

  initialize: async () => {
    let unlisten = () => {};
    let queueEventsReady = false;
    const pendingQueueEvents: QueueSnapshot[] = [];
    try {
      const unlistenJob = await api.onJobUpdated((job) => {
        set((state) => ({
          jobs: { ...state.jobs, [job.id]: job },
        }));
        if (job.state === "Completed") void get().loadHistory();
      });
      const unlistenRemoved = await api.onJobRemoved((jobId) => {
        set((state) => {
          const jobs = { ...state.jobs };
          delete jobs[jobId];
          return { jobs, history: state.history.filter((entry) => entry.id !== jobId) };
        });
      });
      const unlistenQueue = await api.onQueueUpdated((queue) => {
        if (queueEventsReady) set({ queue });
        else pendingQueueEvents.push(queue);
      });
      unlisten = () => { unlistenJob(); unlistenRemoved(); unlistenQueue(); };
    } catch (error) {
      set({ error: normalizeError(error) });
      return unlisten;
    }

    const results = await Promise.allSettled([
      api.listJobs(),
      api.getDefaultDestination(),
      api.detectEngines(),
      api.listHistory(),
      api.getEnginePreferences(),
      api.getQueueSnapshot(),
    ]);

    const [jobs, destination, engines, history, preferences, queue] = results;
    if (jobs.status === "fulfilled") {
      set({ jobs: Object.fromEntries(jobs.value.map((job) => [job.id, job])) });
    }
    if (destination.status === "fulfilled") set({ defaultDestination: destination.value });
    if (engines.status === "fulfilled") set({ engines: engines.value });
    if (history.status === "fulfilled") set({ history: history.value });
    if (preferences.status === "fulfilled") {
      set({ enginePreferences: preferences.value });
    }
    if (queue.status === "fulfilled") set({ queue: queue.value });
    queueEventsReady = true;
    const latestQueueEvent = pendingQueueEvents.at(-1);
    if (latestQueueEvent) set({ queue: latestQueueEvent });

    const failure = results.find((result) => result.status === "rejected");
    if (failure?.status === "rejected") set({ error: normalizeError(failure.reason) });
    return unlisten;
  },

  startDownload: async (request) => {
    set({ error: null });
    try {
      const job = await api.startDownload(request);
      set((state) => ({
        jobs: { ...state.jobs, [job.id]: job },
        notice: { message: "Added to the download queue.", view: "queue" },
      }));
      void api.getQueueSnapshot().then((queue) => set({ queue }));
      return job;
    } catch (error) {
      set({ error: normalizeError(error) });
      return null;
    }
  },

  startDownloads: async (requests) => {
    set({ error: null });
    try {
      const result = await api.startDownloads(requests);
      const jobs = result.jobs;
      set((state) => ({
        jobs: { ...state.jobs, ...Object.fromEntries(jobs.map((job) => [job.id, job])) },
        notice: { message: result.failures.length > 0
          ? `Added ${jobs.length}; ${result.failures.length} could not be queued.`
          : `Added ${jobs.length} downloads to the queue.`, view: "queue" },
      }));
      if (result.failures.length > 0) {
        set({ error: {
          code: "batch_download_partial_failure",
          message: `${result.failures.length} download${result.failures.length === 1 ? "" : "s"} could not be queued.`,
          details: result.failures.map((failure) => `${failure.url}: ${failure.error.message}`).join("\n"),
        } });
      }
      void api.getQueueSnapshot().then((queue) => set({ queue }));
      return jobs;
    } catch (error) {
      set({ error: normalizeError(error) });
      return [];
    }
  },

  cancelDownload: async (jobId) => {
    try {
      await api.cancelDownload(jobId);
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  dismissDownloads: async (jobIds) => {
    try {
      const dismissed = await api.dismissDownloads(jobIds);
      set((state) => {
        const jobs = { ...state.jobs };
        for (const id of dismissed) delete jobs[id];
        return { jobs, error: null };
      });
      return dismissed;
    } catch (error) {
      set({ error: normalizeError(error) });
      return [];
    }
  },

  retryDownload: async (jobId) => {
    set({ error: null });
    try {
      const job = await api.retryDownload(jobId);
      set((state) => ({
        jobs: { ...state.jobs, [job.id]: job },
        view: "downloads",
      }));
      set({ queue: await api.getQueueSnapshot() });
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  repeatDownload: async (jobId) => {
    set({ error: null });
    try {
      const job = await api.repeatDownload(jobId);
      set((state) => ({
        jobs: { ...state.jobs, [job.id]: job },
        notice: { message: "Added another download using the saved options.", view: "queue" },
      }));
      set({ queue: await api.getQueueSnapshot() });
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  pauseDownload: async (jobId) => {
    try {
      await api.pauseDownload(jobId);
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  resumeDownload: async (jobId) => {
    try {
      await api.resumeDownload(jobId);
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  updateQueueSettings: async (settings) => {
    try {
      set({ queue: await api.updateQueueSettings(settings), error: null });
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },

  moveQueuedDownload: async (jobId, newIndex) => {
    try {
      set({ queue: await api.moveQueuedDownload(jobId, newIndex), error: null });
    } catch (error) {
      set({ error: normalizeError(error) });
    }
  },
}));
