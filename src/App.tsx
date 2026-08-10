import { AlertTriangle, X } from "lucide-react";
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

  return (
    <AppShell>
      {error ? (
        <div className="sticky top-0 z-20 border-b border-red-500/30 bg-red-950/95 px-8 py-3 text-sm text-red-100 backdrop-blur">
          <div className="flex items-start gap-3">
            <AlertTriangle size={17} className="mt-0.5 shrink-0 text-red-400" />
            <div className="min-w-0 flex-1">
              <p>{error.message}</p>
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
      {view === "new" ? <NewDownloadPage /> : null}
      {view === "downloads" ? <DownloadsPage /> : null}
      {view === "queue" ? <DownloadsPage queueOnly /> : null}
      {view === "history" ? <HistoryPage /> : null}
      {view === "presets" ? <PresetsPage /> : null}
      {view === "settings" ? <SettingsPage /> : null}
    </AppShell>
  );
}
