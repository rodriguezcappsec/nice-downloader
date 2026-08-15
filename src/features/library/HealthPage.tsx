import {
  AlertTriangle,
  CheckCircle2,
  Database,
  Film,
  FolderX,
  HardDrive,
  HeartPulse,
  LoaderCircle,
  RefreshCw,
  ScanSearch,
} from "lucide-react";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Button, Select } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryArtworkCacheStatus,
  LibraryHealthIssueKind,
  LibraryHealthSummary,
  LibraryPlaybackCacheStatus,
  LibrarySearchStatus,
  LibrarySource,
  LibraryVisualSimilarityStatus,
} from "../../types";

type HealthFilter = "all" | LibraryHealthIssueKind;

const ISSUE_LABELS: Record<LibraryHealthIssueKind, string> = {
  missing: "Missing",
  changed: "Changed",
  metadataPending: "Awaiting inspection",
  metadataFailed: "Inspection failed",
  compatibilityRisk: "Native/copy advised",
};

export function HealthPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const [summary, setSummary] = useState<LibraryHealthSummary | null>(null);
  const [search, setSearch] = useState<LibrarySearchStatus | null>(null);
  const [similarity, setSimilarity] = useState<LibraryVisualSimilarityStatus | null>(null);
  const [playbackCache, setPlaybackCache] = useState<LibraryPlaybackCacheStatus | null>(null);
  const [artworkCache, setArtworkCache] = useState<LibraryArtworkCacheStatus | null>(null);
  const [sources, setSources] = useState<LibrarySource[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState<HealthFilter>("all");

  async function load() {
    setLoading(true);
    try {
      const [nextSummary, nextSearch, nextSimilarity, nextPlayback, nextArtwork, nextSources] = await Promise.all([
        api.getLibraryHealthSummary(),
        api.getLibrarySearchStatus(),
        api.getLibraryVisualSimilarityStatus(),
        api.getLibraryPlaybackCacheStatus(),
        api.getLibraryArtworkCacheStatus(),
        api.listLibrarySources(),
      ]);
      setSummary(nextSummary);
      setSearch(nextSearch);
      setSimilarity(nextSimilarity);
      setPlaybackCache(nextPlayback);
      setArtworkCache(nextArtwork);
      setSources(nextSources);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void load(); }, []);

  function openLibrary(detail: Record<string, unknown>) {
    setView("library");
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:library-health-filter", { detail })), 0);
  }

  function openIssue(itemId: string) {
    setView("library");
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:open-library-item", { detail: { itemId } })), 0);
  }

  async function repairSearch() {
    setBusy(true);
    try {
      setSearch(await api.rebuildLibrarySearchIndex());
      setNotice({ message: "Library search index rebuilt.", view: "health" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function clearPlaybackCache() {
    setBusy(true);
    try {
      const result = await api.clearLibraryPlaybackCache();
      setPlaybackCache(await api.getLibraryPlaybackCacheStatus());
      setNotice({ message: result.removedFiles ? `Removed ${result.removedFiles} playback copies and freed ${formatBytes(result.freedBytes)}.` : "Playback-copy cache is already empty.", view: "health" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function clearArtworkCache() {
    setBusy(true);
    try {
      const result = await api.clearLibraryArtworkCache();
      setArtworkCache(await api.getLibraryArtworkCacheStatus());
      setNotice({ message: result.removedFiles ? `Removed ${result.removedFiles} previews and freed ${formatBytes(result.freedBytes)}.` : "Artwork cache is already empty.", view: "health" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  const offlineSources = sources.filter((source) => source.status === "offline" || source.status === "error");
  const filteredIssues = useMemo(() => summary?.issues.filter((issue) => filter === "all" || issue.kinds.includes(filter)) || [], [filter, summary]);
  const indexedSimilarity = similarity ? Math.max(0, similarity.eligibleItems - similarity.indexedItems) : 0;
  const searchMissing = search ? Math.max(0, search.catalogItems - search.indexedItems) : 0;

  return <div className="mx-auto w-full max-w-[1480px] px-8 pb-14 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end gap-5"><div className="min-w-0 flex-1"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Library maintenance</p><h1 className="flex items-center gap-3 text-[28px] font-semibold tracking-[-0.035em]"><HeartPulse className="text-accent" size={27} /> Media health</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">One place for unavailable files, changed sources, inspection failures, derived indexes, embedded-playback risks, and application cache use.</p></div><Button variant="secondary" disabled={loading || busy} onClick={() => void load()}>{loading ? <LoaderCircle className="animate-spin" size={15} /> : <RefreshCw size={15} />} Refresh</Button></header>

    {loading && !summary ? <div className="grid min-h-64 place-items-center text-sm text-muted" role="status"><span className="flex items-center gap-2"><LoaderCircle className="animate-spin text-accent" size={17} /> Checking Library health…</span></div> : summary ? <>
      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4" aria-label="Library health summary">
        <HealthCard icon={<CheckCircle2 size={19} />} label="Available" value={summary.availableItems} detail={`${summary.totalItems.toLocaleString()} catalog records`} tone="good" />
        <HealthCard icon={<FolderX size={19} />} label="Missing files" value={summary.missingItems} detail="Source path unavailable" tone={summary.missingItems ? "warning" : "good"} action={summary.missingItems ? () => openLibrary({ availability: "missing" }) : undefined} />
        <HealthCard icon={<RefreshCw size={19} />} label="Changed files" value={summary.changedItems} detail="Awaiting review" tone={summary.changedItems ? "warning" : "good"} action={summary.changedItems ? () => openLibrary({ manage: true }) : undefined} />
        <HealthCard icon={<AlertTriangle size={19} />} label="Inspection problems" value={summary.metadataPendingItems + summary.metadataFailedItems} detail={`${summary.metadataFailedItems} failed · ${summary.metadataPendingItems} pending`} tone={summary.metadataFailedItems ? "danger" : summary.metadataPendingItems ? "warning" : "good"} action={summary.metadataPendingItems + summary.metadataFailedItems ? () => openLibrary({ manage: true }) : undefined} />
        <HealthCard icon={<Film size={19} />} label="Playback caution" value={summary.compatibilityRiskItems} detail="Native player or copy may be advised" tone={summary.compatibilityRiskItems ? "neutral" : "good"} />
        <HealthCard icon={<Database size={19} />} label="Search index" value={searchMissing} detail={search?.consistent ? "Current" : "Repair recommended"} tone={search?.consistent ? "good" : "warning"} action={!search?.consistent ? () => void repairSearch() : undefined} />
        <HealthCard icon={<ScanSearch size={19} />} label="Similarity index" value={indexedSimilarity} detail="Eligible media not indexed" tone={indexedSimilarity ? "neutral" : "good"} action={indexedSimilarity ? () => setView("similarity") : undefined} />
        <HealthCard icon={<HardDrive size={19} />} label="Source connections" value={offlineSources.length} detail={offlineSources.length ? offlineSources.map((source) => source.name).slice(0, 2).join(", ") : "All sources reachable"} tone={offlineSources.length ? "danger" : "good"} action={offlineSources.length ? () => openLibrary({ manage: true }) : undefined} />
      </section>

      <section className="mt-6 grid gap-4 lg:grid-cols-2" aria-label="Application cache usage">
        <CacheCard title="Playback copies" status={playbackCache ? `${playbackCache.fileCount} files · ${formatBytes(playbackCache.totalBytes)}` : "Unavailable"} detail="Compatibility derivatives only; original files are never changed." disabled={busy || !playbackCache?.fileCount} onClear={() => void clearPlaybackCache()} />
        <CacheCard title="Artwork previews" status={artworkCache ? `${artworkCache.fileCount} files · ${formatBytes(artworkCache.totalBytes)}` : "Unavailable"} detail="Rebuildable thumbnails and previews stored in the application cache." disabled={busy || !artworkCache?.fileCount} onClear={() => void clearArtworkCache()} />
      </section>

      <section className="panel-shadow mt-6 border border-line/60 bg-panel/75" aria-labelledby="health-issues-title"><header className="flex flex-wrap items-end gap-4 border-b border-line/60 px-5 py-4"><div className="min-w-0 flex-1"><h2 id="health-issues-title" className="text-sm font-semibold">Files needing attention</h2><p className="mt-1 text-[10px] text-muted">Showing up to 100 recent issue records · compatibility is a conservative advisory, not a decoder verdict</p></div><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Issue<Select className="mt-1 h-9 min-w-48 text-xs" value={filter} onChange={(event) => setFilter(event.target.value as HealthFilter)}><option value="all">All sampled ({summary.issues.length})</option>{Object.entries(ISSUE_LABELS).map(([kind, label]) => <option key={kind} value={kind}>{label}</option>)}</Select></label></header>
        {filteredIssues.length ? <div className="divide-y divide-line/50">{filteredIssues.map((issue) => <button key={issue.itemId} className="flex w-full items-center gap-4 px-5 py-3 text-left transition hover:bg-elevated/55 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent/60" onClick={() => openIssue(issue.itemId)}><span className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-elevated text-accent">{issue.mediaKind === "image" ? "IMG" : <Film size={15} />}</span><span className="min-w-0 flex-1"><strong className="block truncate text-xs font-medium">{issue.title}</strong><span className="mt-1 flex flex-wrap gap-1.5">{issue.kinds.map((kind) => <span key={kind} className={`rounded-[4px] px-1.5 py-0.5 text-[9px] font-medium ${kind === "metadataFailed" || kind === "missing" ? "bg-red-400/10 text-red-300" : kind === "changed" || kind === "metadataPending" ? "bg-amber-400/10 text-amber-200" : "bg-accent/10 text-accent"}`}>{ISSUE_LABELS[kind]}</span>)}</span></span><span className="text-[10px] font-medium text-muted">Open details</span></button>)}</div> : <div className="px-5 py-12 text-center"><CheckCircle2 className="mx-auto text-accent" size={28} /><p className="mt-3 text-sm font-medium">No matching health issues</p><p className="mt-1 text-xs text-muted">This filter has nothing requiring attention.</p></div>}
      </section>
    </> : null}
  </div>;
}

function HealthCard({ icon, label, value, detail, tone, action }: { icon: ReactNode; label: string; value: number; detail: string; tone: "good" | "warning" | "danger" | "neutral"; action?: () => void }) {
  const colors = tone === "danger" ? "text-red-300 bg-red-400/10" : tone === "warning" ? "text-amber-200 bg-amber-400/10" : tone === "good" ? "text-accent bg-accent/10" : "text-sky-300 bg-sky-400/10";
  const content = <><div className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${colors}`}>{icon}</div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">{label}</p><strong className="number-font mt-1 block text-2xl font-semibold">{value.toLocaleString()}</strong><p className="mt-1 truncate text-[10px] text-muted" title={detail}>{detail}</p></div></>;
  return action ? <button className="panel-shadow flex min-w-0 items-start gap-3 border border-line/60 bg-panel/75 p-4 text-left transition hover:-translate-y-0.5 hover:border-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={action}>{content}</button> : <article className="panel-shadow flex min-w-0 items-start gap-3 border border-line/60 bg-panel/75 p-4">{content}</article>;
}

function CacheCard({ title, status, detail, disabled, onClear }: { title: string; status: string; detail: string; disabled: boolean; onClear: () => void }) {
  return <article className="panel-shadow flex items-center gap-4 border border-line/60 bg-panel/75 p-5"><div className="grid size-10 shrink-0 place-items-center rounded-[9px] bg-elevated text-accent"><HardDrive size={18} /></div><div className="min-w-0 flex-1"><h3 className="text-sm font-semibold">{title}</h3><p className="number-font mt-1 text-xs text-ink">{status}</p><p className="mt-1 text-[10px] leading-4 text-muted">{detail}</p></div><Button variant="secondary" className="shrink-0" disabled={disabled} onClick={onClear}>Clear cache</Button></article>;
}
