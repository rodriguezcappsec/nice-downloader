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
        <div className="sticky top-0 z-20 flex items-center gap-3 border-b border-red-500/30 bg-red-950/95 px-8 py-3 text-sm text-red-100 backdrop-blur">
          <AlertTriangle size={17} className="shrink-0 text-red-400" />
          <span className="flex-1">{error}</span>
          <button className="text-red-300 hover:text-white" onClick={() => setError(null)} aria-label="Dismiss error"><X size={17} /></button>
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
