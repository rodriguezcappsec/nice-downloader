import { AlertTriangle, CheckCircle2, X } from "lucide-react";
import { useEffect } from "react";
import { AppShell } from "./components/AppShell";
import { DownloadsPage } from "./features/downloads/DownloadsPage";
import { HistoryPage } from "./features/history/HistoryPage";
import { NewDownloadPage } from "./features/new-download/NewDownloadPage";
import { PresetsPage, SettingsPage } from "./features/settings/SettingsPage";
import { useAppStore } from "./stores/app-store";

export default function App() {
  const view = useAppStore((state) => state.view);
  const error = useAppStore((state) => state.error);
  const setError = useAppStore((state) => state.setError);
  const initialize = useAppStore((state) => state.initialize);
  const notice = useAppStore((state) => state.notice);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const errorAction = error ? getErrorAction(error.code, error.message) : null;

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
        <div className="sticky top-0 z-20 border-b border-red-500/30 bg-red-950/95 px-8 py-3 text-sm text-red-100 backdrop-blur">
          <div className="flex items-start gap-3">
            <AlertTriangle size={17} className="mt-0.5 shrink-0 text-red-400" />
            <div className="min-w-0 flex-1">
              <p>{error.message}</p>
              {errorAction ? <button className="mt-2 text-xs font-semibold text-red-200 underline underline-offset-2" onClick={() => { setView(errorAction.view); setError(null); }}>{errorAction.label}</button> : null}
              {error.details ? (
                <details className="mt-2 text-xs text-red-200/80">
                  <summary className="cursor-pointer select-none font-medium text-red-200">Technical details</summary>
                  <pre className="mt-2 max-h-40 overflow-auto whitespace-pre-wrap border border-red-500/20 bg-black/20 p-3 font-mono leading-5">{error.details}</pre>
                </details>
              ) : null}
            </div>
            <button className="text-red-300 hover:text-white" onClick={() => setError(null)} aria-label="Dismiss error"><X size={17} /></button>
          </div>
        </div>
      ) : null}
      <div className={view === "new" ? "" : "hidden"}><NewDownloadPage /></div>
      <div className={view === "downloads" ? "" : "hidden"}><DownloadsPage /></div>
      <div className={view === "queue" ? "" : "hidden"}><DownloadsPage queueOnly /></div>
      <div className={view === "history" ? "" : "hidden"}><HistoryPage /></div>
      <div className={view === "presets" ? "" : "hidden"}><PresetsPage /></div>
      <div className={view === "settings" ? "" : "hidden"}><SettingsPage /></div>
      {notice ? (
        <div className="fixed bottom-5 right-5 z-40 flex min-w-80 items-center gap-3 border border-line bg-panel p-3 shadow-xl">
          <CheckCircle2 size={18} className="text-emerald-400" />
          <span className="flex-1 text-sm">{notice.message}</span>
          {notice.view ? <button className="text-xs font-semibold text-blue-400 hover:text-blue-300" onClick={() => { setView(notice.view!); setNotice(null); }}>View</button> : null}
          <button className="text-muted hover:text-ink" onClick={() => setNotice(null)} aria-label="Dismiss notification"><X size={15} /></button>
        </div>
      ) : null}
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
