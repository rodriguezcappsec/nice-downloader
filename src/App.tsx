import { AlertTriangle, CheckCircle2, X } from "lucide-react";
import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { AppShell } from "./components/AppShell";
import { OnboardingDialog } from "./components/OnboardingDialog";
import { DownloadsPage } from "./features/downloads/DownloadsPage";
import { HistoryPage } from "./features/history/HistoryPage";
import { LibraryPage } from "./features/library/LibraryPage";
import { NewDownloadPage } from "./features/new-download/NewDownloadPage";
import { PresetsPage, SettingsPage } from "./features/settings/SettingsPage";
import { SubscriptionsPage } from "./features/subscriptions/SubscriptionsPage";
import { TransformationsPage } from "./features/transformations/TransformationsPage";
import { useAppStore } from "./stores/app-store";
import { api, normalizeError } from "./lib/tauri";
import type { DesktopPreferences, OnboardingStatus } from "./types";

const ComparePage = lazy(() => import("./features/library/ComparePage").then((module) => ({ default: module.ComparePage })));
const ImageComparePage = lazy(() => import("./features/library/ImageComparePage").then((module) => ({ default: module.ImageComparePage })));
const CurationPage = lazy(() => import("./features/library/CurationPage").then((module) => ({ default: module.CurationPage })));

export default function App() {
  const view = useAppStore((state) => state.view);
  const error = useAppStore((state) => state.error);
  const setError = useAppStore((state) => state.setError);
  const initialize = useAppStore((state) => state.initialize);
  const notice = useAppStore((state) => state.notice);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const errorAction = error ? getErrorAction(error.code, error.message) : null;
  const jobs = useAppStore((state) => state.jobs);
  const [desktopPreferences, setDesktopPreferences] = useState<DesktopPreferences | null>(null);
  const desktopPreferencesRef = useRef<DesktopPreferences | null>(null);
  const activeJobCountRef = useRef(0);
  const [onboarding, setOnboarding] = useState<OnboardingStatus | null>(null);
  const activeJobs = Object.values(jobs).filter((job) => ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state));
  desktopPreferencesRef.current = desktopPreferences;
  activeJobCountRef.current = activeJobs.length;

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let mounted = true;
    void initialize().then((unlisten) => {
      if (mounted) cleanup = unlisten;
      else unlisten();
    });
    return () => {
      mounted = false;
      cleanup?.();
    };
  }, [initialize]);

  useEffect(() => {
    void Promise.all([api.getDesktopPreferences(), api.getDesktopRuntimeStatus(), api.getOnboardingStatus()])
      .then(([preferences, runtime, status]) => {
        setDesktopPreferences(!runtime.trayAvailable && preferences.closeBehavior === "background"
          ? { ...preferences, closeBehavior: "quit", startMinimized: false }
          : preferences);
        if (!status.completed) setOnboarding(status);
      })
      .catch((error) => setError(normalizeError(error)));
  }, [setError]);

  useEffect(() => {
    let disposed = false;
    let unlisten = () => {};
    void getCurrentWindow().onCloseRequested(async (event) => {
      const preferences = desktopPreferencesRef.current;
      if (!preferences) {
        event.preventDefault();
        await api.quitApp();
        return;
      }
      if (preferences.closeBehavior === "quit") {
        event.preventDefault();
        await api.quitApp();
        return;
      }
      if (preferences.closeBehavior === "background") {
        event.preventDefault();
        try {
          await api.hideToTray();
          setNotice({ message: "Nice Downloader is still running in the system tray." });
        } catch (error) {
          setError(normalizeError(error));
        }
        return;
      }
      if (activeJobCountRef.current > 0) {
        event.preventDefault();
        const background = window.confirm(`${activeJobCountRef.current} download${activeJobCountRef.current === 1 ? " is" : "s are"} active. Choose OK to keep running in the background, or Cancel to quit.`);
        if (background) {
          try { await api.hideToTray(); }
          catch (error) { setError(normalizeError(error)); }
        } else {
          await api.quitApp();
        }
      } else {
        event.preventDefault();
        await api.quitApp();
      }
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => { disposed = true; unlisten(); };
  }, [setError, setNotice]);

  useEffect(() => {
    const updatePreferences = (event: Event) => {
      setDesktopPreferences((event as CustomEvent<DesktopPreferences>).detail);
    };
    window.addEventListener("nice-downloader:desktop-preferences", updatePreferences);
    return () => window.removeEventListener("nice-downloader:desktop-preferences", updatePreferences);
  }, []);

  useEffect(() => {
    if (!desktopPreferences) return;
    document.documentElement.classList.toggle("reduce-motion", desktopPreferences.reduceMotion);
    document.documentElement.classList.toggle("high-contrast", desktopPreferences.highContrast);
    void getCurrentWebview().setZoom(desktopPreferences.interfaceScalePercent / 100)
      .catch((error) => setError(normalizeError(error)));
  }, [desktopPreferences, setError]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey)) return;
      if (event.key.toLocaleLowerCase() === "n") {
        event.preventDefault();
        setView("new");
        window.setTimeout(() => document.getElementById("media-url")?.focus(), 0);
      } else if (event.key === ",") {
        event.preventDefault();
        setView("settings");
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setView]);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 8000);
    return () => window.clearTimeout(timer);
  }, [notice, setNotice]);

  return (
    <AppShell>
      {error ? (
        <div className="sticky top-0 z-20 border-b border-red-500/25 bg-[#36181c]/95 px-8 py-3.5 text-[13px] text-red-100 shadow-[0_12px_28px_rgb(0_0_0/0.16)] backdrop-blur-xl" role="alert" aria-live="assertive">
          <div className="mx-auto flex max-w-[1340px] items-start gap-3">
            <div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-red-400/10 text-red-300"><AlertTriangle size={16} /></div>
            <div className="min-w-0 flex-1">
              <p className="font-medium leading-5">{error.message}</p>
              {errorAction ? <button className="mt-1.5 text-[11px] font-semibold text-red-200 underline decoration-red-300/50 underline-offset-4 transition hover:text-white" onClick={() => { setView(errorAction.view); setError(null); }}>{errorAction.label}</button> : null}
              {error.details ? (
                <details className="mt-2 text-[11px] text-red-200/80">
                  <summary className="cursor-pointer select-none font-medium text-red-200">Technical details</summary>
                  <pre className="number-font mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded-[7px] border border-red-500/20 bg-black/20 p-3 leading-5">{error.details}</pre>
                </details>
              ) : null}
            </div>
            <button className="grid size-8 place-items-center rounded-[7px] text-red-300 transition hover:bg-white/5 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-300/70" onClick={() => setError(null)} aria-label="Dismiss error"><X size={16} /></button>
          </div>
        </div>
      ) : null}
      <div className={view === "new" ? "" : "hidden"}><NewDownloadPage /></div>
      <div className={view === "downloads" ? "" : "hidden"}><DownloadsPage /></div>
      <div className={view === "queue" ? "" : "hidden"}><DownloadsPage queueOnly /></div>
      <div className={view === "history" ? "" : "hidden"}><HistoryPage /></div>
      <div className={view === "subscriptions" ? "" : "hidden"}><SubscriptionsPage /></div>
      <div className={view === "transformations" ? "" : "hidden"}><TransformationsPage /></div>
      <LibraryPage active={view === "library"} />
      {view === "compare" ? <Suspense fallback={<div className="grid min-h-[60vh] place-items-center text-sm text-muted" role="status">Opening comparison workspace…</div>}><ComparePage /></Suspense> : null}
      {view === "imageCompare" ? <Suspense fallback={<div className="grid min-h-[60vh] place-items-center text-sm text-muted" role="status">Opening image studio…</div>}><ImageComparePage /></Suspense> : null}
      {view === "curation" ? <Suspense fallback={<div className="grid min-h-[60vh] place-items-center text-sm text-muted" role="status">Opening curation workspace…</div>}><CurationPage /></Suspense> : null}
      <div className={view === "presets" ? "" : "hidden"}><PresetsPage /></div>
      <div className={view === "settings" ? "" : "hidden"}><SettingsPage /></div>
      {notice ? (
        <div className="panel-shadow fixed bottom-6 right-6 z-40 flex min-w-80 items-center gap-3 rounded-[10px] border border-line/70 bg-panel/95 p-3.5 backdrop-blur-xl" role="status" aria-live="polite">
          <div className="grid size-8 place-items-center rounded-[7px] bg-accent/10 text-accent"><CheckCircle2 size={17} /></div>
          <span className="flex-1 text-[13px] font-medium">{notice.message}</span>
          {notice.view ? <button className="text-[11px] font-semibold text-accent transition hover:brightness-125" onClick={() => { setView(notice.view!); setNotice(null); }}>View</button> : null}
          <button className="grid size-7 place-items-center rounded-[6px] text-muted transition hover:bg-elevated hover:text-ink" onClick={() => setNotice(null)} aria-label="Dismiss notification"><X size={14} /></button>
        </div>
      ) : null}
      {onboarding ? <OnboardingDialog status={onboarding} onComplete={() => void api.completeOnboarding().then(() => setOnboarding(null)).catch((error) => setError(normalizeError(error)))} /> : null}
    </AppShell>
  );
}

function getErrorAction(code: string, message: string): { label: string; view: "settings" | "history" | "queue" } | null {
  const value = `${code} ${message}`.toLocaleLowerCase();
  if (value.includes("cookie") || value.includes("login") || value.includes("engine") || value.includes("proxy")) return { label: "Open relevant settings", view: "settings" };
  if (value.includes("duplicate")) return { label: "View previous downloads", view: "history" };
  if (value.includes("rate") || value.includes("retry")) return { label: "View queue and retries", view: "queue" };
  return null;
}
