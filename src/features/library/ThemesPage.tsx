import {
  BrainCircuit,
  CheckCircle2,
  Eye,
  FileText,
  FolderTree,
  ListChecks,
  LoaderCircle,
  Play,
  RefreshCw,
  Search,
  ServerOff,
  Square,
  Tags,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryFacets,
  LibraryItem,
  LibrarySource,
  LibraryThemeAnalysis,
  LibraryThemeAnalysisScope,
  LibraryThemeIndexStatus,
  LibraryThemeJob,
  LibraryThemeProviderStatus,
  LibraryThemeSettings,
  StartLibraryThemeAnalysisRequest,
} from "../../types";

const DEFAULT_SETTINGS: LibraryThemeSettings = {
  enabled: false,
  model: "qwen2.5vl:7b",
  mode: "deep",
};

const DEFAULT_MAX_VIDEOS = 25;
const MAX_THEME_VIDEOS = 500;

interface FolderChoice {
  sourceId: string;
  path: string;
  label: string;
}

export function ThemesPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const [settings, setSettings] = useState<LibraryThemeSettings>(DEFAULT_SETTINGS);
  const [savedSettings, setSavedSettings] = useState<LibraryThemeSettings>(DEFAULT_SETTINGS);
  const [provider, setProvider] = useState<LibraryThemeProviderStatus | null>(null);
  const [index, setIndex] = useState<LibraryThemeIndexStatus | null>(null);
  const [facets, setFacets] = useState<LibraryFacets | null>(null);
  const [sources, setSources] = useState<LibrarySource[]>([]);
  const [analyses, setAnalyses] = useState<LibraryThemeAnalysis[]>([]);
  const [job, setJob] = useState<LibraryThemeJob | null>(null);
  const [scope, setScope] = useState<LibraryThemeAnalysisScope>("all");
  const [maxVideos, setMaxVideos] = useState(DEFAULT_MAX_VIDEOS);
  const [folderKey, setFolderKey] = useState("");
  const [includeNested, setIncludeNested] = useState(true);
  const [videoSourceId, setVideoSourceId] = useState("");
  const [videoSearch, setVideoSearch] = useState("");
  const [videoResults, setVideoResults] = useState<LibraryItem[]>([]);
  const [selectedItems, setSelectedItems] = useState<LibraryItem[]>([]);
  const [searchingVideos, setSearchingVideos] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  async function load() {
    setLoading(true);
    try {
      const nextSettings = await api.getLibraryThemeSettings();
      const [nextProvider, nextIndex, nextFacets, nextSources, nextAnalyses, nextJob] = await Promise.all([
        api.getLibraryThemeProviderStatus(),
        api.getLibraryThemeIndexStatus(),
        api.getLibraryFacets({ sourceId: null, availability: "available" }),
        api.listLibrarySources(),
        api.listLibraryThemeAnalyses(50),
        api.getLibraryThemeJob(),
      ]);
      setSettings(nextSettings);
      setSavedSettings(nextSettings);
      setProvider(nextProvider);
      setIndex(nextIndex);
      setFacets(nextFacets);
      setSources(nextSources);
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

  useEffect(() => {
    if (scope !== "items") return;
    if (!videoSourceId) {
      setVideoResults([]);
      return;
    }
    let disposed = false;
    const timer = window.setTimeout(() => {
      setSearchingVideos(true);
      void api.queryLibraryItems({
        page: 1,
        pageSize: 60,
        search: videoSearch.trim() || null,
        sourceId: videoSourceId,
        folderSourceId: null,
        folderPath: null,
        folderScope: "tree",
        mediaKind: "video",
        resolution: null,
        container: null,
        duration: null,
        dateAdded: null,
        datePublished: null,
        availability: "available",
        tags: [],
        creators: [],
        series: [],
        sites: [],
        theme: null,
        rating: null,
        collectionId: null,
        favoriteOnly: false,
        sort: "titleAsc",
      }).then((result) => {
        if (!disposed) setVideoResults(result.items);
      }).catch((error) => {
        if (!disposed) setError(normalizeError(error));
      }).finally(() => {
        if (!disposed) setSearchingVideos(false);
      });
    }, 200);
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [scope, setError, videoSearch, videoSourceId]);

  const running = job?.state === "running";
  const settingsChanged = JSON.stringify(settings) !== JSON.stringify(savedSettings);
  const progress = running && job.totalItems > 0
    ? Math.min(100, job.checkedItems / job.totalItems * 100)
    : 0;
  const topThemes = useMemo(() => facets?.themes.slice(0, 30) || [], [facets]);
  const sourceNames = useMemo(
    () => new Map(sources.map((source) => [source.id, source.name])),
    [sources],
  );
  const folderChoices = useMemo<FolderChoice[]>(() => {
    const choices: FolderChoice[] = [];
    for (const source of sources) {
      if (source.mediaKind === "image") continue;
      choices.push({ sourceId: source.id, path: "", label: `${source.name} /` });
      const paths = new Set(
        (facets?.folders || [])
          .filter((folder) => folder.sourceId === source.id && folder.path)
          .map((folder) => folder.path),
      );
      for (const path of [...paths].sort((left, right) => left.localeCompare(right))) {
        choices.push({ sourceId: source.id, path, label: `${source.name} / ${path}` });
      }
    }
    return choices;
  }, [facets, sources]);
  const videoSources = useMemo(
    () => sources.filter((source) => source.mediaKind !== "image"),
    [sources],
  );
  const selectedIds = useMemo(
    () => new Set(selectedItems.map((item) => item.id)),
    [selectedItems],
  );
  const chosenFolder = useMemo(() => {
    if (!folderKey) return null;
    return folderChoices.find((choice) => folderValue(choice) === folderKey) || null;
  }, [folderChoices, folderKey]);
  const validLimit = Number.isInteger(maxVideos) && maxVideos >= 1 && maxVideos <= MAX_THEME_VIDEOS;
  const validTarget = scope === "all"
    || (scope === "folder" && chosenFolder !== null)
    || (scope === "items" && selectedItems.length > 0 && selectedItems.length <= MAX_THEME_VIDEOS);

  useEffect(() => {
    if (scope !== "folder") return;
    if (!folderChoices.some((choice) => folderValue(choice) === folderKey)) {
      setFolderKey(folderChoices[0] ? folderValue(folderChoices[0]) : "");
    }
  }, [folderChoices, folderKey, scope]);

  useEffect(() => {
    if (!videoSources.some((source) => source.id === videoSourceId)) {
      setVideoSourceId(videoSources[0]?.id || "");
    }
  }, [videoSourceId, videoSources]);

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
    if (!validLimit || !validTarget) return;
    const request: StartLibraryThemeAnalysisRequest = {
      scope,
      maxVideos,
      sourceId: scope === "folder" ? chosenFolder?.sourceId : null,
      folderPath: scope === "folder" ? chosenFolder?.path : null,
      folderScope: scope === "folder" && !includeNested ? "direct" : "tree",
      itemIds: scope === "items" ? selectedItems.map((item) => item.id) : [],
    };
    setBusy(true);
    try {
      const nextJob = await api.startLibraryThemeAnalysis(request);
      setJob(nextJob);
      if (nextJob.totalItems === 0) {
        setNotice({ message: "No stale, analysis-ready videos matched that selection.", view: "themes" });
      }
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

  function toggleSelected(item: LibraryItem) {
    setSelectedItems((current) => {
      if (current.some((selected) => selected.id === item.id)) {
        return current.filter((selected) => selected.id !== item.id);
      }
      return current.length >= MAX_THEME_VIDEOS ? current : [...current, item];
    });
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

      <aside className="panel-shadow border border-line/60 bg-panel/75 p-6" aria-labelledby="theme-index-title"><h2 id="theme-index-title" className="text-sm font-semibold">Theme index</h2><div className="mt-4 grid grid-cols-3 gap-2"><Metric label="Analyzed" value={index?.indexedItems || 0} /><Metric label="Remaining" value={index?.staleItems || 0} /><Metric label="Themes" value={index?.themeCount || 0} /></div>{running ? <div className="mt-5"><div className="flex items-center justify-between text-[10px] text-muted"><span className="truncate pr-3">{job.currentTitle || "Preparing next video…"}</span><span className="number-font shrink-0">{job.checkedItems}/{job.totalItems}</span></div><div className="mt-2 h-1.5 overflow-hidden rounded-full bg-elevated"><div className="h-full bg-accent transition-[width]" style={{ width: `${progress}%` }} /></div><p className="mt-2 text-[10px] text-muted">{job.indexedItems} indexed · {job.failedItems} skipped</p></div> : <p className="mt-4 text-[10px] leading-4 text-muted">Only videos whose current fingerprint, selected model, or analysis depth is stale are processed. Each run uses the exact bounded selection below.</p>}{job?.error ? <p className="mt-3 text-[10px] leading-4 text-red-300">{job.error}</p> : null}</aside>
    </div>

    <section className="panel-shadow mt-5 border border-line/60 bg-panel/75 p-6" aria-labelledby="theme-target-title">
      <div className="flex flex-wrap items-start gap-5"><div className="min-w-0 flex-1"><h2 id="theme-target-title" className="flex items-center gap-2 text-sm font-semibold"><ListChecks className="text-accent" size={17} /> Choose videos to analyze</h2><p className="mt-1 max-w-3xl text-[10px] leading-4 text-muted">Keep large libraries manageable. Current analyses, missing files, non-videos, and videos still waiting for metadata are skipped automatically.</p></div><label className="w-44 text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Maximum this run<Input className="number-font mt-1 normal-case" type="number" min={1} max={MAX_THEME_VIDEOS} value={maxVideos} disabled={running} onChange={(event) => setMaxVideos(Number(event.target.value))} onBlur={() => setMaxVideos((value) => Math.min(MAX_THEME_VIDEOS, Math.max(1, Math.trunc(value) || 1)))} /></label></div>
      <div className="mt-5 grid gap-3 sm:grid-cols-3">
        <ScopeButton active={scope === "all"} disabled={running} title="Next stale videos" detail={`Process the next ${validLimit ? maxVideos : "bounded"} across the Library.`} onClick={() => setScope("all")} />
        <ScopeButton active={scope === "folder"} disabled={running} title="Imported folder" detail="Choose one source root or indexed subfolder." onClick={() => setScope("folder")} />
        <ScopeButton active={scope === "items"} disabled={running} title="Selected videos" detail="Search and keep an explicit catalog selection." onClick={() => setScope("items")} />
      </div>

      {scope === "folder" ? <div className="mt-5 grid gap-4 md:grid-cols-[minmax(0,1fr)_240px]"><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Imported folder<Select className="mt-1 normal-case" value={folderKey} disabled={running || folderChoices.length === 0} onChange={(event) => setFolderKey(event.target.value)}>{folderChoices.length ? folderChoices.map((choice) => <option key={folderValue(choice)} value={folderValue(choice)}>{choice.label}</option>) : <option value="">No imported video folders</option>}</Select></label><label className="flex items-center gap-3 self-end rounded-[8px] border border-line/60 bg-elevated/35 px-4 py-3 text-xs"><input type="checkbox" checked={includeNested} disabled={running} onChange={(event) => setIncludeNested(event.target.checked)} /><span><strong className="block text-[11px] font-semibold">Include nested folders</strong><span className="mt-0.5 block text-[9px] text-muted">Turn off for this folder only.</span></span></label></div> : null}

      {scope === "items" ? <div className="mt-5 grid gap-4 lg:grid-cols-[minmax(0,1.15fr)_minmax(320px,0.85fr)]">
        <div><div className="grid gap-2 sm:grid-cols-[220px_minmax(0,1fr)]"><Select value={videoSourceId} disabled={running || videoSources.length === 0} aria-label="Imported video source" onChange={(event) => setVideoSourceId(event.target.value)}>{videoSources.length ? videoSources.map((source) => <option key={source.id} value={source.id}>{source.name}</option>) : <option value="">No imported video sources</option>}</Select><label className="relative block"><Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" size={14} /><Input className="pl-9" value={videoSearch} placeholder="Search imported videos…" disabled={running || !videoSourceId} onChange={(event) => setVideoSearch(event.target.value)} /></label></div><div className="mt-2 max-h-64 overflow-y-auto border border-line/60 bg-elevated/20">{searchingVideos ? <div className="flex items-center justify-center gap-2 px-4 py-10 text-xs text-muted"><LoaderCircle className="animate-spin" size={15} /> Searching videos…</div> : videoResults.length ? videoResults.map((item) => { const selected = selectedIds.has(item.id); const selectionFull = selectedItems.length >= MAX_THEME_VIDEOS && !selected; return <label key={item.id} className="flex cursor-pointer items-start gap-3 border-b border-line/45 px-3 py-2.5 last:border-b-0 hover:bg-accent/[0.05]"><input className="mt-0.5" type="checkbox" checked={selected} disabled={running || selectionFull} onChange={() => toggleSelected(item)} /><span className="min-w-0"><strong className="block truncate text-[11px] font-medium">{item.title}</strong><span className="mt-0.5 block truncate text-[9px] text-muted">{catalogLocation(item, sourceNames)}</span></span></label>; }) : <div className="px-4 py-10 text-center text-xs text-muted">No available videos match this search.</div>}</div><p className="mt-2 text-[9px] leading-4 text-muted">Switch imported sources or change the search without losing selected catalog IDs. Each result page is bounded to 60.</p></div>
        <div className="border border-line/60 bg-elevated/20"><header className="flex items-center justify-between border-b border-line/55 px-3 py-2.5"><strong className="text-[11px] font-semibold">Selected videos</strong><span className="number-font text-[9px] text-muted">{selectedItems.length}/{MAX_THEME_VIDEOS}</span></header><div className="max-h-64 overflow-y-auto">{selectedItems.length ? selectedItems.map((item) => <div key={item.id} className="flex items-start gap-2 border-b border-line/45 px-3 py-2.5 last:border-b-0"><span className="min-w-0 flex-1"><strong className="block truncate text-[10px] font-medium">{item.title}</strong><span className="mt-0.5 block truncate text-[8px] text-muted">{catalogLocation(item, sourceNames)}</span></span><button className="rounded p-1 text-muted hover:bg-elevated hover:text-foreground disabled:opacity-50" type="button" aria-label={`Remove ${item.title}`} disabled={running} onClick={() => toggleSelected(item)}><X size={13} /></button></div>) : <div className="px-4 py-10 text-center text-xs text-muted">Select at least one video.</div>}</div></div>
      </div> : null}

      <div className="mt-5 flex flex-wrap items-center gap-3 border-t border-line/55 pt-5"><Button disabled={busy || running || settingsChanged || !savedSettings.enabled || !provider?.selectedModelAvailable || !index?.staleItems || !validLimit || !validTarget} onClick={() => void start()}><Play size={14} /> Analyze up to {validLimit ? maxVideos : "—"} video{maxVideos === 1 ? "" : "s"}</Button>{running ? <Button variant="secondary" disabled={busy} onClick={() => void cancel()}><Square size={14} /> Cancel analysis</Button> : null}<span className={`text-[10px] ${validLimit && validTarget ? "text-muted" : "text-amber-200"}`}>{!validLimit ? "Choose a maximum from 1 to 500." : !validTarget ? scope === "folder" ? "Choose an imported folder." : "Select at least one video." : scope === "all" ? "Rerun to process the next stale group." : "Only stale videos within this target are processed."}</span></div>
    </section>

    <section className="mt-6" aria-labelledby="discovered-themes-title"><div className="flex items-end gap-4"><div className="min-w-0 flex-1"><h2 id="discovered-themes-title" className="flex items-center gap-2 text-sm font-semibold"><Tags className="text-accent" size={16} /> Discovered themes</h2><p className="mt-1 text-[10px] text-muted">Choose a theme to filter the Library across creators and sources.</p></div></div>{topThemes.length ? <div className="mt-3 flex flex-wrap gap-2">{topThemes.map((theme) => <button key={theme.key} className="rounded-[8px] border border-line/60 bg-panel/75 px-3 py-2 text-left transition hover:border-accent/35 hover:bg-accent/[0.07] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => filterByTheme(theme.key)}><span className="text-[11px] font-semibold">{theme.label}</span><span className="number-font ml-2 text-[9px] text-muted">{theme.itemCount}</span></button>)}</div> : <div className="mt-3 border border-dashed border-line/70 px-5 py-8 text-center text-xs text-muted">Analyze videos to build filterable themes.</div>}</section>

    <section className="panel-shadow mt-6 border border-line/60 bg-panel/75" aria-labelledby="theme-evidence-title"><header className="border-b border-line/60 px-5 py-4"><h2 id="theme-evidence-title" className="text-sm font-semibold">Recent analysis and evidence</h2><p className="mt-1 text-[10px] text-muted">Confidence is advisory. Evidence shows whether a theme came from sampled frames, transcript text, or metadata.</p></header>{analyses.length ? <div className="divide-y divide-line/50">{analyses.map((analysis) => <article key={analysis.itemId} className="px-5 py-4"><div className="flex flex-wrap items-start gap-4"><button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => { setView("library"); window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:open-library-item", { detail: { itemId: analysis.itemId } })), 0); }}><strong className="block truncate text-xs font-semibold hover:text-accent">{analysis.title}</strong><span className="mt-1 block text-[10px] leading-4 text-muted">{analysis.summary}</span></button><span className="flex shrink-0 items-center gap-1.5 rounded-[6px] bg-elevated px-2 py-1 text-[9px] text-muted">{analysis.mode === "deep" ? <Eye size={12} /> : <FileText size={12} />}{analysis.mode === "deep" ? "Deep" : "Fast"} · {Math.round(analysis.overallConfidence * 100)}%</span></div><div className="mt-3 flex flex-wrap gap-2">{analysis.themes.map((theme) => <details key={theme.key} className="group rounded-[6px] border border-line/55 bg-elevated/30 px-2.5 py-1.5"><summary className="cursor-pointer list-none text-[10px] font-medium"><span className="text-accent">{theme.label}</span><span className="number-font ml-1.5 text-muted">{Math.round(theme.confidence * 100)}%</span></summary>{theme.evidence.length ? <ul className="mt-2 max-w-xl space-y-1 border-t border-line/45 pt-2 text-[9px] leading-4 text-muted">{theme.evidence.map((evidence) => <li key={evidence}>• {evidence}</li>)}</ul> : <p className="mt-2 border-t border-line/45 pt-2 text-[9px] text-muted">No specific evidence was returned.</p>}</details>)}</div></article>)}</div> : <div className="px-5 py-12 text-center text-xs text-muted">No theme analyses yet.</div>}</section>
  </div>;
}

function ScopeButton({ active, disabled, title, detail, onClick }: { active: boolean; disabled: boolean; title: string; detail: string; onClick: () => void }) {
  return <button type="button" className={`rounded-[9px] border p-4 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${active ? "border-accent/45 bg-accent/[0.08]" : "border-line/60 bg-elevated/25 hover:border-line"}`} disabled={disabled} onClick={onClick}><strong className="flex items-center gap-2 text-xs font-semibold">{title === "Imported folder" ? <FolderTree size={14} /> : <ListChecks size={14} />}{title}</strong><span className="mt-1.5 block text-[9px] leading-4 text-muted">{detail}</span></button>;
}

function Metric({ label, value }: { label: string; value: number }) {
  return <div className="rounded-[8px] bg-elevated/55 px-3 py-3 text-center"><strong className="number-font block text-lg font-semibold">{value.toLocaleString()}</strong><span className="mt-1 block text-[9px] uppercase tracking-[0.08em] text-muted">{label}</span></div>;
}

function folderValue(choice: Pick<FolderChoice, "sourceId" | "path">) {
  return JSON.stringify([choice.sourceId, choice.path]);
}

function catalogLocation(item: LibraryItem, sourceNames: Map<string, string>) {
  const source = item.sourceId ? sourceNames.get(item.sourceId) : null;
  if (source && item.relativePath) return `${source} / ${item.relativePath}`;
  if (source) return source;
  return item.relativePath || "Library catalog";
}
