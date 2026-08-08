import { create } from "zustand";
import { api, errorMessage } from "../lib/tauri";
import type {
  DownloadJob,
  EngineInfo,
  HistoryEntry,
  MediaAnalysis,
  StartDownloadRequest,
  ViewId,
} from "../types";

interface AppStore {
  view: ViewId;
  engines: EngineInfo[];
  jobs: Record<string, DownloadJob>;
  history: HistoryEntry[];
  analysis: MediaAnalysis | null;
  analyzing: boolean;
  engineLoading: boolean;
  historyLoading: boolean;
  error: string | null;
  defaultDestination: string;
  setView: (view: ViewId) => void;
  setError: (error: string | null) => void;
  analyze: (url: string) => Promise<MediaAnalysis | null>;
  clearAnalysis: () => void;
  detectEngines: () => Promise<void>;
  loadHistory: () => Promise<void>;
  initialize: () => Promise<() => void>;
  startDownload: (request: StartDownloadRequest) => Promise<DownloadJob | null>;
  cancelDownload: (jobId: string) => Promise<void>;
}

export const useAppStore = create<AppStore>((set, get) => ({
  view: "new",
  engines: [],
  jobs: {},
  history: [],
  analysis: null,
  analyzing: false,
  engineLoading: false,
  historyLoading: false,
  error: null,
  defaultDestination: "",

  setView: (view) => set({ view }),
  setError: (error) => set({ error }),

  analyze: async (url) => {
    set({ analyzing: true, error: null, analysis: null });
    try {
      const analysis = await api.analyzeUrl(url);
      set({ analysis });
      return analysis;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    } finally {
      set({ analyzing: false });
    }
  },

  clearAnalysis: () => set({ analysis: null, error: null }),

  detectEngines: async () => {
    set({ engineLoading: true });
    try {
      set({ engines: await api.detectEngines() });
    } catch (error) {
      set({ error: errorMessage(error) });
    } finally {
      set({ engineLoading: false });
    }
  },

  loadHistory: async () => {
    set({ historyLoading: true });
    try {
      set({ history: await api.listHistory() });
    } catch (error) {
      set({ error: errorMessage(error) });
    } finally {
      set({ historyLoading: false });
    }
  },

  initialize: async () => {
    let unlisten = () => {};
    try {
      unlisten = await api.onJobUpdated((job) => {
        set((state) => ({ jobs: { ...state.jobs, [job.id]: job } }));
        if (job.state === "Completed") void get().loadHistory();
      });
    } catch (error) {
      set({ error: errorMessage(error) });
      return unlisten;
    }

    const results = await Promise.allSettled([
      api.listJobs(),
      api.getDefaultDestination(),
      api.detectEngines(),
      api.listHistory(),
    ]);

    const [jobs, destination, engines, history] = results;
    if (jobs.status === "fulfilled") {
      set({ jobs: Object.fromEntries(jobs.value.map((job) => [job.id, job])) });
    }
    if (destination.status === "fulfilled") set({ defaultDestination: destination.value });
    if (engines.status === "fulfilled") set({ engines: engines.value });
    if (history.status === "fulfilled") set({ history: history.value });

    const failure = results.find((result) => result.status === "rejected");
    if (failure?.status === "rejected") set({ error: errorMessage(failure.reason) });
    return unlisten;
  },

  startDownload: async (request) => {
    set({ error: null });
    try {
      const job = await api.startDownload(request);
      set((state) => ({
        jobs: { ...state.jobs, [job.id]: job },
        view: "downloads",
      }));
      return job;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  cancelDownload: async (jobId) => {
    try {
      await api.cancelDownload(jobId);
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },
}));
