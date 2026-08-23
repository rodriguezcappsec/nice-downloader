import { ExternalLink, GitCompareArrows, Images, LoaderCircle, RefreshCw, Search, ShieldCheck, Square, WandSparkles } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, Input, Select } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { loadLibraryArtwork } from "../../lib/library-artwork";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryItem,
  LibraryNearDuplicateMatch,
  LibraryVisualSimilarityJob,
  LibraryVisualSimilarityMatch,
  LibraryVisualSimilarityStatus,
} from "../../types";

export function SimilarityPage() {
  const setError = useAppStore((state) => state.setError);
  const [status, setStatus] = useState<LibraryVisualSimilarityStatus | null>(null);
  const [job, setJob] = useState<LibraryVisualSimilarityJob | null>(null);
  const [search, setSearch] = useState("");
  const [sources, setSources] = useState<LibraryItem[]>([]);
  const [sourceLoading, setSourceLoading] = useState(false);
  const [seed, setSeed] = useState<LibraryItem | null>(null);
  const [threshold, setThreshold] = useState("75");
  const [matches, setMatches] = useState<LibraryVisualSimilarityMatch[]>([]);
  const [matchLoading, setMatchLoading] = useState(false);
  const [seedIndexed, setSeedIndexed] = useState(true);
  const [nearMatches, setNearMatches] = useState<LibraryNearDuplicateMatch[]>([]);
  const [nearThreshold, setNearThreshold] = useState("85");
  const [durationTolerance, setDurationTolerance] = useState("5");
  const [nearLoading, setNearLoading] = useState(false);

  useEffect(() => {
    let disposed = false;
    void Promise.all([api.getLibraryVisualSimilarityStatus(), api.getLibraryVisualSimilarityJob()])
      .then(([nextStatus, nextJob]) => {
        if (!disposed) {
          setStatus(nextStatus);
          setJob(nextJob);
        }
      })
      .catch((error) => !disposed && setError(normalizeError(error)));
    let cleanup = () => {};
    void api.onLibraryVisualSimilarityUpdated((nextJob) => {
      if (disposed) return;
      setJob(nextJob);
      if (nextJob.state !== "running") {
        void api.getLibraryVisualSimilarityStatus().then(setStatus).catch((error) => setError(normalizeError(error)));
      }
    }).then((unlisten) => {
      if (disposed) unlisten();
      else cleanup = unlisten;
    });
    return () => { disposed = true; cleanup(); };
  }, [setError]);

  useEffect(() => {
    let disposed = false;
    const chooseReference = (event: Event) => {
      const itemId = (event as CustomEvent<{ itemId?: string }>).detail?.itemId;
      if (!itemId) return;
      void api.getLibraryItemsByIds([itemId])
        .then(([item]) => {
          if (!disposed && item) {
            setSeed(item);
            setSearch("");
          }
        })
        .catch((error) => { if (!disposed) setError(normalizeError(error)); });
    };
    window.addEventListener("nice-downloader:similarity-reference", chooseReference);
    return () => {
      disposed = true;
      window.removeEventListener("nice-downloader:similarity-reference", chooseReference);
    };
  }, [setError]);

  useEffect(() => {
    let disposed = false;
    const timer = window.setTimeout(() => {
      setSourceLoading(true);
      void api.queryLibraryItems({
        page: 1,
        pageSize: 24,
        search: search.trim() || null,
        sourceId: null,
        folderSourceId: null,
        folderPath: null,
        folderScope: "tree",
        mediaKind: null,
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
        rating: null,
        collectionId: null,
        favoriteOnly: false,
        sort: "recent",
      }).then((result) => {
        if (!disposed) setSources(result.items);
      }).catch((error) => {
        if (!disposed) setError(normalizeError(error));
      }).finally(() => {
        if (!disposed) setSourceLoading(false);
      });
    }, 250);
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [search, setError]);

  useEffect(() => {
    if (!seed || !status?.indexedItems || job?.state === "running") {
      setMatches([]);
      return;
    }
    let disposed = false;
    setMatchLoading(true);
    setSeedIndexed(true);
    void api.findVisuallySimilarLibraryItems(seed.id, 24, Number(threshold))
      .then((value) => !disposed && setMatches(value))
      .catch((error) => {
        if (!disposed) {
          const normalized = normalizeError(error);
          if (normalized.code === "visual_similarity_not_indexed") {
            setMatches([]);
            setSeedIndexed(false);
          }
          else setError(normalized);
        }
      })
      .finally(() => !disposed && setMatchLoading(false));
    return () => { disposed = true; };
  }, [job?.state, seed, setError, status?.indexedItems, threshold]);

  const progress = job?.totalItems ? Math.min(100, job.checkedItems / job.totalItems * 100) : 0;
  const indexCurrent = job?.state === "running";
  const indexedLabel = status ? `${status.indexedItems}/${status.eligibleItems}` : "—";
  async function startIndex() {
    try {
      setJob(await api.startLibraryVisualSimilarityIndex());
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function cancelIndex() {
    if (!job) return;
    try {
      await api.cancelLibraryVisualSimilarityIndex(job.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function findNearDuplicates() {
    setNearLoading(true);
    try {
      setNearMatches(await api.findLibraryNearDuplicateVideos(Number(nearThreshold), Number(durationTolerance), 100));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setNearLoading(false);
    }
  }

  return <div className="mx-auto w-full max-w-[1500px] px-8 pb-12 pt-8 xl:px-10">
    <header className="mb-7 flex flex-wrap items-end gap-5"><div className="min-w-0 flex-1"><p className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Discovery</p><h1 className="text-[28px] font-semibold tracking-[-0.035em]">Visual similarity explorer</h1><p className="mt-2 max-w-3xl text-[13px] leading-5 text-muted">Find media with a similar overall composition using a small local perceptual fingerprint. Results are approximate and are never presented as exact duplicate evidence.</p></div><div className="flex items-center gap-2"><div className="rounded-[8px] border border-line/60 bg-panel/70 px-4 py-2 text-right"><span className="block text-[9px] uppercase tracking-[0.1em] text-muted">Indexed</span><strong className="number-font text-sm text-ink">{indexedLabel}</strong></div>{indexCurrent ? <Button variant="secondary" onClick={() => void cancelIndex()}><Square size={14} /> Cancel</Button> : <Button onClick={() => void startIndex()}>{status?.indexedItems ? <RefreshCw size={15} /> : <WandSparkles size={15} />} {status?.indexedItems ? "Refresh index" : "Build index"}</Button>}</div></header>

    {indexCurrent ? <section className="panel-shadow mb-5 border border-accent/20 bg-panel/80 p-4" aria-live="polite"><div className="flex items-center gap-3 text-xs"><LoaderCircle className="animate-spin text-accent" size={15} /><span className="min-w-0 flex-1 truncate">Indexing {job.currentTitle || "Library media"}</span><span className="number-font text-muted">{job.checkedItems}/{job.totalItems}</span></div><div className="mt-3 h-1.5 overflow-hidden bg-elevated"><div className="h-full bg-accent transition-all" style={{ width: `${progress}%` }} /></div><p className="mt-2 text-[10px] text-muted">{job.indexedItems} indexed · {job.failedItems} skipped</p></section> : null}
    {job?.state === "failed" ? <p className="mb-5 border-l-2 border-red-400 bg-red-400/5 px-3 py-2 text-xs text-red-300">{job.error || "Visual indexing failed."}</p> : null}

    <section className="panel-shadow mb-5 overflow-hidden border border-line/60 bg-panel/75" aria-labelledby="near-duplicate-title">
      <header className="flex flex-wrap items-end gap-4 border-b border-line/55 px-5 py-4">
        <div className="min-w-0 flex-1"><p className="flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.14em] text-accent"><GitCompareArrows size={13} /> Review candidates</p><h2 id="near-duplicate-title" className="mt-1 text-sm font-semibold">Near-duplicate videos</h2><p className="mt-1 max-w-3xl text-[10px] leading-4 text-muted">Combines visual fingerprints with duration and technical differences. Exact SHA-256 duplicates are excluded, and these suggestions never enter exact-duplicate cleanup.</p></div>
        <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Visual match<Select className="mt-1 h-9 w-32 text-xs" value={nearThreshold} onChange={(event) => setNearThreshold(event.target.value)}><option value="95">95% strict</option><option value="90">90%</option><option value="85">85% balanced</option><option value="80">80%</option><option value="75">75% broad</option></Select></label>
        <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Duration delta<Select className="mt-1 h-9 w-32 text-xs" value={durationTolerance} onChange={(event) => setDurationTolerance(event.target.value)}><option value="1">Within 1 sec</option><option value="5">Within 5 sec</option><option value="15">Within 15 sec</option><option value="60">Within 1 min</option><option value="300">Within 5 min</option></Select></label>
        <Button disabled={nearLoading || indexCurrent || !status?.indexedItems} onClick={() => void findNearDuplicates()}>{nearLoading ? <LoaderCircle className="animate-spin" size={14} /> : <Search size={14} />} Find candidates</Button>
      </header>
      {nearMatches.length ? <div className="grid gap-3 p-4 lg:grid-cols-2">{nearMatches.map((match) => <article key={`${match.left.id}:${match.right.id}`} className="rounded-[9px] border border-line/55 bg-elevated/25 p-3"><div className="grid grid-cols-2 gap-2"><NearDuplicateItem item={match.left} onOpen={() => void api.openLibraryItem(match.left.id).catch((error) => setError(normalizeError(error)))} /><NearDuplicateItem item={match.right} onOpen={() => void api.openLibraryItem(match.right.id).catch((error) => setError(normalizeError(error)))} /></div><div className="mt-3 flex flex-wrap gap-1.5 text-[9px]"><Evidence label={`${match.visualSimilarityPercent.toFixed(1)}% visual`} strong /><Evidence label={`${match.durationDeltaSeconds.toFixed(2)}s duration delta`} /><Evidence label={match.sizeDeltaBytes != null ? `${formatBytes(match.sizeDeltaBytes)} size delta` : "Size unknown"} /><Evidence label={match.sameResolution ? "Same resolution" : "Different resolution"} /><Evidence label={match.sameVideoCodec ? "Same video codec" : "Different video codec"} /><Evidence label={match.sameAudioCodec ? "Same audio codec" : "Different audio codec"} /><Evidence label={match.sameContainer ? "Same container" : "Different container"} /></div><p className="mt-3 flex items-center gap-1.5 text-[9px] leading-4 text-muted"><ShieldCheck size={12} className="text-accent" />{match.exactHashCompared ? "Both exact hashes were available and differ." : "Exact hashes were unavailable for at least one file; review carefully."}</p></article>)}</div> : <p className="px-5 py-8 text-center text-xs text-muted">{nearLoading ? "Comparing indexed videos…" : "Run candidate review after the visual index is current."}</p>}
    </section>

    <div className="grid gap-5 xl:grid-cols-[340px_minmax(0,1fr)]">
      <aside className="panel-shadow min-w-0 border border-line/60 bg-panel/75 p-4"><h2 className="text-sm font-semibold">Choose reference media</h2><p className="mt-1 text-[10px] leading-4 text-muted">Images and videos can be compared together by their representative frame.</p><div className="relative mt-4"><Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-muted" size={14} /><Input className="pl-9" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search Library" /></div><div className="mt-2 max-h-[62vh] overflow-y-auto border border-line/55">{sourceLoading ? <p className="flex items-center gap-2 p-3 text-xs text-muted"><LoaderCircle className="animate-spin" size={13} /> Searching…</p> : sources.map((item) => <button key={item.id} className={`grid w-full grid-cols-[64px_minmax(0,1fr)] items-center gap-3 border-b border-line/40 p-2 text-left last:border-b-0 ${seed?.id === item.id ? "bg-accent/10" : "hover:bg-elevated/55"}`} onClick={() => setSeed(item)}><Artwork item={item} /><span className="min-w-0"><strong className="block truncate text-[11px] font-semibold">{item.title}</strong><span className="mt-1 block text-[9px] text-muted">{item.mediaKind || "media"}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</span></span></button>)}</div></aside>

      <main className="panel-shadow min-w-0 border border-line/60 bg-panel/75 p-5"><div className="flex flex-wrap items-end gap-4 border-b border-line/60 pb-4"><div className="min-w-0 flex-1"><h2 className="truncate text-lg font-semibold">{seed ? seed.title : "Select reference media"}</h2><p className="mt-1 text-[10px] text-muted">Approximate perceptual match · exact duplicate cleanup remains separate</p></div><label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Minimum similarity<Select className="mt-1 h-9 min-w-36 text-xs" value={threshold} onChange={(event) => setThreshold(event.target.value)}><option value="90">90% · strict</option><option value="80">80% · balanced</option><option value="75">75% · broader</option><option value="65">65% · exploratory</option><option value="50">50% · widest</option></Select></label></div>
        {!seed ? <Empty icon={<Images size={28} />} text="Choose an image or video to explore visually related media." /> : indexCurrent ? <Empty icon={<LoaderCircle className="animate-spin" size={25} />} text="Results will refresh when indexing finishes." /> : matchLoading ? <Empty icon={<LoaderCircle className="animate-spin" size={25} />} text="Comparing perceptual fingerprints…" /> : !seedIndexed ? <Empty icon={<WandSparkles size={26} />} text="This item is not indexed yet. Refresh the visual index." /> : matches.length ? <div className="mt-5 grid gap-3 sm:grid-cols-2 2xl:grid-cols-3">{matches.map((match) => <article key={match.item.id} className="overflow-hidden rounded-[9px] border border-line/55 bg-elevated/30"><Artwork item={match.item} large /><div className="p-3"><div className="flex items-center gap-3"><strong className="min-w-0 flex-1 truncate text-xs">{match.item.title}</strong><span className="number-font rounded-[5px] bg-accent/10 px-2 py-1 text-[10px] font-semibold text-accent">{match.similarityPercent.toFixed(1)}%</span></div><p className="mt-2 text-[9px] text-muted">Perceptual distance {match.hammingDistance}/64 · approximate</p><Button variant="secondary" className="mt-3 h-8 w-full text-[10px]" onClick={() => void api.openLibraryItem(match.item.id).catch((error) => setError(normalizeError(error)))}><ExternalLink size={12} /> Open original</Button></div></article>)}</div> : <Empty icon={<Search size={25} />} text={`No indexed media met the ${threshold}% threshold.`} />}
      </main>
    </div>
  </div>;
}

function NearDuplicateItem({ item, onOpen }: { item: LibraryItem; onOpen: () => void }) {
  return <button className="min-w-0 overflow-hidden rounded-[7px] border border-line/50 bg-panel/50 text-left transition hover:border-accent/30" onClick={onOpen}><Artwork item={item} large /><span className="block p-2"><strong className="block truncate text-[10px] font-semibold">{item.title}</strong><span className="mt-1 block text-[9px] text-muted">{item.durationSeconds != null ? formatDuration(item.durationSeconds) : "Unknown duration"} · {item.width && item.height ? `${item.width}×${item.height}` : "Unknown resolution"}</span></span></button>;
}

function Evidence({ label, strong = false }: { label: string; strong?: boolean }) {
  return <span className={`rounded-[5px] px-2 py-1 ${strong ? "bg-accent/10 font-semibold text-accent" : "bg-panel/70 text-muted"}`}>{label}</span>;
}

function Artwork({ item, large = false }: { item: LibraryItem; large?: boolean }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    let disposed = false;
    void loadLibraryArtwork(item.id).then((value) => !disposed && setUrl(value)).catch(() => {});
    return () => { disposed = true; };
  }, [item.id]);
  return <span className={`${large ? "aspect-video w-full" : "aspect-video w-16"} grid shrink-0 place-items-center overflow-hidden bg-[#0a0d12] text-white/25`}>{url ? <img className="h-full w-full object-cover" src={url} alt="" /> : <Images size={large ? 24 : 16} />}</span>;
}

function Empty({ icon, text }: { icon: ReactNode; text: string }) {
  return <div className="grid min-h-80 place-items-center text-center text-xs leading-5 text-muted"><span><span className="mx-auto mb-3 grid place-items-center text-accent/60">{icon}</span>{text}</span></div>;
}
