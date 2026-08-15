import {
  BrainCircuit,
  CheckCircle2,
  Eye,
  FileText,
  LoaderCircle,
  Play,
  RefreshCw,
  ServerOff,
  Square,
  Tags,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryFacets,
  LibraryThemeAnalysis,
  LibraryThemeIndexStatus,
  LibraryThemeJob,
  LibraryThemeProviderStatus,
  LibraryThemeSettings,
} from "../../types";

const DEFAULT_SETTINGS: LibraryThemeSettings = {
  enabled: false,
  model: "qwen2.5vl:7b",
  mode: "deep",
};

export function ThemesPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const [settings, setSettings] = useState<LibraryThemeSettings>(DEFAULT_SETTINGS);
  const [savedSettings, setSavedSettings] = useState<LibraryThemeSettings>(DEFAULT_SETTINGS);
  const [provider, setProvider] = useState<LibraryThemeProviderStatus | null>(null);
  const [index, setIndex] = useState<LibraryThemeIndexStatus | null>(null);
  const [facets, setFacets] = useState<LibraryFacets | null>(null);
  const [analyses, setAnalyses] = useState<LibraryThemeAnalysis[]>([]);
  const [job, setJob] = useState<LibraryThemeJob | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const nextSettings = await api.getLibraryThemeSettings();
      const [nextProvider, nextIndex, nextFacets, nextAnalyses, nextJob] = await Promise.all([
        api.getLibraryThemeProviderStatus(),
        api.getLibraryThemeIndexStatus(),
        api.getLibraryFacets({ sourceId: null, availability: "available" }),
        api.listLibraryThemeAnalyses(50),
        api.getLibraryThemeJob(),
      ]);
      setSettings(nextSettings);
      setSavedSettings(nextSettings);
      setProvider(nextProvider);
      setIndex(nextIndex);
      setFacets(nextFacets);
      setAnalyses(nextAnalyses);
      setJob(nextJob);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    let disposed = false;
    let unlisten = () => {};
    void api.onLibraryThemeAnalysisUpdated((nextJob) => {
      if (disposed) return;
      setJob(nextJob);
      if (["completed", "failed", "cancelled"].includes(nextJob.state)) {
        void Promise.all([
          api.getLibraryThemeIndexStatus(),
          api.getLibraryFacets({ sourceId: null, availability: "available" }),
          api.listLibraryThemeAnalyses(50),
        ]).then(([nextIndex, nextFacets, nextAnalyses]) => {
          if (!disposed) {
            setIndex(nextIndex);
            setFacets(nextFacets);
            setAnalyses(nextAnalyses);
          }
        });
      }
    }).then((cleanup) => disposed ? cleanup() : unlisten = cleanup);
    return () => { disposed = true; unlisten(); };
  }, [setError]);

  const running = job?.state === "running";
  const settingsChanged = JSON.stringify(settings) !== JSON.stringify(savedSettings);
  const progress = running && job.totalItems > 0
    ? Math.min(100, job.checkedItems / job.totalItems * 100)
    : 0;
  const topThemes = useMemo(() => facets?.themes.slice(0, 30) || [], [facets]);

  async function saveSettings() {
    setBusy(true);
    try {
      const saved = await api.saveLibraryThemeSettings({
        ...settings,
        model: settings.model.trim(),
      });
      setSettings(saved);
      setSavedSettings(saved);
      const [nextProvider, nextIndex] = await Promise.all([
        api.getLibraryThemeProviderStatus(),
        api.getLibraryThemeIndexStatus(),
      ]);
      setProvider(nextProvider);
      setIndex(nextIndex);
      setNotice({ message: "Local theme-analysis settings saved.", view: "themes" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function start() {
    setBusy(true);
    try {
      setJob(await api.startLibraryThemeAnalysis());
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function cancel() {
    if (!job) return;
    setBusy(true);
    try {
      await api.cancelLibraryThemeAnalysis(job.id);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  function filterByTheme(key: string) {
    setView("library");
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:library-theme-filter", { detail: { key } })), 0);
  }

  return <div className="mx-auto w-full max-w-[1480px] px-8 pb-14 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end gap-5"><div className="min-w-0 flex-1"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Optional local AI</p><h1 className="flex items-center gap-3 text-[28px] font-semibold tracking-[-0.035em]"><BrainCircuit className="text-accent" size={27} /> Theme discovery</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">Recognize what videos are actually about across creators. Deep analysis combines metadata and transcripts with six representative frames sampled from the video itself.</p></div><Button variant="secondary" disabled={loading || busy || running} onClick={() => void load()}>{loading ? <LoaderCircle className="animate-spin" size={15} /> : <RefreshCw size={15} />} Refresh</Button></header>

    <div className="grid gap-5 xl:grid-cols-[minmax(0,1fr)_400px]">
      <section className="panel-shadow border border-line/60 bg-panel/75 p-6" aria-labelledby="theme-setup-title">
        <div className="flex items-start gap-4"><div className="grid size-11 shrink-0 place-items-center rounded-[10px] bg-accent/10 text-accent">{provider?.available ? <CheckCircle2 size={20} /> : <ServerOff size={20} />}</div><div className="min-w-0 flex-1"><h2 id="theme-setup-title" className="text-sm font-semibold">Local model setup</h2><p className={`mt-1 text-xs leading-5 ${provider?.selectedModelAvailable ? "text-accent" : "text-muted"}`}>{provider?.detail || "Checking the local Ollama service…"}</p></div></div>
        <div className="mt-5 grid gap-4 sm:grid-cols-2"><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Vision model<Input className="mt-1 normal-case" list="installed-theme-models" value={settings.model} maxLength={100} disabled={running} onChange={(event) => setSettings({ ...settings, model: event.target.value })} /><datalist id="installed-theme-models">{provider?.installedModels.map((model) => <option key={model} value={model} />)}</datalist></label><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Analysis depth<Select className="mt-1 normal-case" value={settings.mode} disabled={running} onChange={(event) => setSettings({ ...settings, mode: event.target.value as LibraryThemeSettings["mode"] })}><option value="deep">Deep · metadata + transcript + frames</option><option value="fast">Fast · metadata + transcript only</option></Select></label></div>
        <label className="mt-5 flex items-start gap-3 rounded-[9px] border border-line/60 bg-elevated/35 p-4"><input className="mt-0.5" type="checkbox" checked={settings.enabled} disabled={running} onChange={(event) => setSettings({ ...settings, enabled: event.target.checked })} /><span><strong className="block text-xs font-semibold">Enable local AI theme analysis</strong><span className="mt-1 block text-[10px] leading-4 text-muted">Nothing is uploaded by Nice Downloader. Requests go only to Ollama on 127.0.0.1, and the feature remains off until you opt in.</span></span></label>
        {!provider?.selectedModelAvailable ? <div className="mt-4 rounded-[8px] border border-amber-400/25 bg-amber-400/5 px-4 py-3 text-[11px] leading-5 text-amber-100"><p>Install and start Ollama, then download the selected model:</p><code className="number-font mt-2 block select-all rounded bg-black/20 px-3 py-2 text-[10px] text-amber-50">ollama pull {settings.model.trim() || "qwen2.5vl:7b"}</code></div> : null}
        <div className="mt-5 flex flex-wrap items-center gap-3"><Button disabled={busy || running || !settingsChanged || !settings.model.trim()} onClick={() => void saveSettings()}>{busy && !running ? <LoaderCircle className="animate-spin" size={15} /> : null} Save setup</Button><span className="text-[10px] text-muted">Qwen2.5-VL 7B is the default; another installed vision-capable Ollama model can be selected.</span></div>
      </section>

      <aside className="panel-shadow border border-line/60 bg-panel/75 p-6" aria-labelledby="theme-index-title"><h2 id="theme-index-title" className="text-sm font-semibold">Theme index</h2><div className="mt-4 grid grid-cols-3 gap-2"><Metric label="Analyzed" value={index?.indexedItems || 0} /><Metric label="Remaining" value={index?.staleItems || 0} /><Metric label="Themes" value={index?.themeCount || 0} /></div>{running ? <div className="mt-5"><div className="flex items-center justify-between text-[10px] text-muted"><span className="truncate pr-3">{job.currentTitle || "Preparing next video…"}</span><span className="number-font shrink-0">{job.checkedItems}/{job.totalItems}</span></div><div className="mt-2 h-1.5 overflow-hidden rounded-full bg-elevated"><div className="h-full bg-accent transition-[width]" style={{ width: `${progress}%` }} /></div><p className="mt-2 text-[10px] text-muted">{job.indexedItems} indexed · {job.failedItems} skipped</p></div> : <p className="mt-4 text-[10px] leading-4 text-muted">Only videos whose current fingerprint, selected model, or analysis depth is stale are processed.</p>}<Button className="mt-5 w-full" variant={running ? "secondary" : "primary"} disabled={busy || settingsChanged || !savedSettings.enabled || !provider?.selectedModelAvailable || (!running && !index?.staleItems)} onClick={() => void (running ? cancel() : start())}>{running ? <><Square size={14} /> Cancel analysis</> : <><Play size={14} /> Analyze {index?.staleItems || 0} video{index?.staleItems === 1 ? "" : "s"}</>}</Button>{job?.error ? <p className="mt-3 text-[10px] leading-4 text-red-300">{job.error}</p> : null}</aside>
    </div>

    <section className="mt-6" aria-labelledby="discovered-themes-title"><div className="flex items-end gap-4"><div className="min-w-0 flex-1"><h2 id="discovered-themes-title" className="flex items-center gap-2 text-sm font-semibold"><Tags className="text-accent" size={16} /> Discovered themes</h2><p className="mt-1 text-[10px] text-muted">Choose a theme to filter the Library across creators and sources.</p></div></div>{topThemes.length ? <div className="mt-3 flex flex-wrap gap-2">{topThemes.map((theme) => <button key={theme.key} className="rounded-[8px] border border-line/60 bg-panel/75 px-3 py-2 text-left transition hover:border-accent/35 hover:bg-accent/[0.07] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => filterByTheme(theme.key)}><span className="text-[11px] font-semibold">{theme.label}</span><span className="number-font ml-2 text-[9px] text-muted">{theme.itemCount}</span></button>)}</div> : <div className="mt-3 border border-dashed border-line/70 px-5 py-8 text-center text-xs text-muted">Analyze videos to build filterable themes.</div>}</section>

    <section className="panel-shadow mt-6 border border-line/60 bg-panel/75" aria-labelledby="theme-evidence-title"><header className="border-b border-line/60 px-5 py-4"><h2 id="theme-evidence-title" className="text-sm font-semibold">Recent analysis and evidence</h2><p className="mt-1 text-[10px] text-muted">Confidence is advisory. Evidence shows whether a theme came from sampled frames, transcript text, or metadata.</p></header>{analyses.length ? <div className="divide-y divide-line/50">{analyses.map((analysis) => <article key={analysis.itemId} className="px-5 py-4"><div className="flex flex-wrap items-start gap-4"><button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => { setView("library"); window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:open-library-item", { detail: { itemId: analysis.itemId } })), 0); }}><strong className="block truncate text-xs font-semibold hover:text-accent">{analysis.title}</strong><span className="mt-1 block text-[10px] leading-4 text-muted">{analysis.summary}</span></button><span className="flex shrink-0 items-center gap-1.5 rounded-[6px] bg-elevated px-2 py-1 text-[9px] text-muted">{analysis.mode === "deep" ? <Eye size={12} /> : <FileText size={12} />}{analysis.mode === "deep" ? "Deep" : "Fast"} · {Math.round(analysis.overallConfidence * 100)}%</span></div><div className="mt-3 flex flex-wrap gap-2">{analysis.themes.map((theme) => <details key={theme.key} className="group rounded-[6px] border border-line/55 bg-elevated/30 px-2.5 py-1.5"><summary className="cursor-pointer list-none text-[10px] font-medium"><span className="text-accent">{theme.label}</span><span className="number-font ml-1.5 text-muted">{Math.round(theme.confidence * 100)}%</span></summary>{theme.evidence.length ? <ul className="mt-2 max-w-xl space-y-1 border-t border-line/45 pt-2 text-[9px] leading-4 text-muted">{theme.evidence.map((evidence) => <li key={evidence}>• {evidence}</li>)}</ul> : <p className="mt-2 border-t border-line/45 pt-2 text-[9px] text-muted">No specific evidence was returned.</p>}</details>)}</div></article>)}</div> : <div className="px-5 py-12 text-center text-xs text-muted">No theme analyses yet.</div>}</section>
  </div>;
}

function Metric({ label, value }: { label: string; value: number }) {
  return <div className="rounded-[8px] bg-elevated/55 px-3 py-3 text-center"><strong className="number-font block text-lg font-semibold">{value.toLocaleString()}</strong><span className="mt-1 block text-[9px] uppercase tracking-[0.08em] text-muted">{label}</span></div>;
}
