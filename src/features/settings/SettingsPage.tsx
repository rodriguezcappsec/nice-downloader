import { CheckCircle2, CircleX, RefreshCw, Settings2, Wrench } from "lucide-react";
import { Page } from "../../components/Page";
import { Button } from "../../components/ui";
import { useAppStore } from "../../stores/app-store";

export function SettingsPage() {
  const engines = useAppStore((state) => state.engines);
  const loading = useAppStore((state) => state.engineLoading);
  const detect = useAppStore((state) => state.detectEngines);
  return (
    <Page
      title="Settings"
      description="Application and engine configuration."
      actions={<Button variant="secondary" disabled={loading} onClick={() => void detect()}><RefreshCw className={loading ? "animate-spin" : ""} size={15} /> Detect again</Button>}
    >
      <section>
        <div className="mb-3 flex items-center gap-2"><Wrench size={17} className="text-muted" /><h2 className="text-sm font-semibold">Engines</h2></div>
        <div className="divide-y divide-line border border-line bg-panel">
          {engines.map((engine) => (
            <article key={engine.id} className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-4 px-4 py-4">
              <div className={`grid size-9 place-items-center rounded-md ${engine.installed ? "bg-emerald-500/10 text-emerald-400" : "bg-red-500/10 text-red-400"}`}>
                {engine.installed ? <CheckCircle2 size={18} /> : <CircleX size={18} />}
              </div>
              <div className="min-w-0">
                <div className="flex items-center gap-2"><h3 className="text-sm font-semibold">{engine.displayName}</h3>{engine.required ? <span className="bg-blue-500/10 px-1.5 py-0.5 text-[10px] font-medium uppercase text-blue-400">Required</span> : null}</div>
                <p className="mt-1 truncate font-mono text-xs text-muted">{engine.executablePath || engine.statusMessage}</p>
              </div>
              <div className="text-right"><div className={`text-xs font-medium ${engine.installed ? "text-emerald-400" : "text-red-400"}`}>{engine.installed ? "Installed" : "Not found"}</div><div className="mt-1 max-w-64 truncate text-xs text-muted">{engine.version || "—"}</div></div>
            </article>
          ))}
          {!loading && engines.length === 0 ? <div className="p-5 text-sm text-muted">No engine status is available.</div> : null}
        </div>
        <p className="mt-3 text-xs leading-5 text-muted">Nice Downloader currently discovers executables from PATH. Managed sidecars and automatic updates are planned after the core workflow is stable.</p>
      </section>
    </Page>
  );
}

export function PresetsPage() {
  return <Page title="Presets" description="Reusable download intentions will arrive in a later milestone."><div className="grid min-h-64 place-items-center border border-dashed border-line text-center"><div><Settings2 className="mx-auto text-muted" size={24} /><h2 className="mt-3 text-sm font-semibold">Presets are coming next</h2><p className="mt-1 text-sm text-muted">The intent model is already designed for saved presets.</p></div></div></Page>;
}
