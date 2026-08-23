import {
  ArrowLeft,
  ArrowRight,
  Check,
  ExternalLink,
  FolderInput,
  Heart,
  Image as ImageIcon,
  ListChecks,
  LoaderCircle,
  Settings2,
  Star,
  Tag,
  Video,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button, Input } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { loadLibraryArtwork } from "../../lib/library-artwork";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { LibraryBulkAction, LibraryCurationDecision, LibraryItem } from "../../types";

type QueueMode = "unreviewed" | "reviewLater";

export function CurationPage() {
  const setView = useAppStore((state) => state.setView);
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [mode, setMode] = useState<QueueMode>("unreviewed");
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [index, setIndex] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [artwork, setArtwork] = useState<string | null>(null);
  const [tagOpen, setTagOpen] = useState(false);
  const [tag, setTag] = useState("");
  const requestRevision = useRef(0);
  const current = items[index] || null;

  const loadQueue = useCallback(async (nextMode: QueueMode) => {
    const revision = ++requestRevision.current;
    setLoading(true);
    try {
      const queue = await api.listLibraryCurationQueue(nextMode === "reviewLater", 200);
      if (revision !== requestRevision.current) return;
      setItems(queue);
      setIndex(0);
    } catch (error) {
      if (revision === requestRevision.current) setError(normalizeError(error));
    } finally {
      if (revision === requestRevision.current) setLoading(false);
    }
  }, [setError]);

  useEffect(() => {
    void loadQueue(mode);
    return () => { requestRevision.current += 1; };
  }, [loadQueue, mode]);

  useEffect(() => {
    let disposed = false;
    setArtwork(null);
    if (!current) return () => { disposed = true; };
    void loadLibraryArtwork(current.id)
      .then((value) => { if (!disposed) setArtwork(value); })
      .catch(() => { if (!disposed) setArtwork(null); });
    return () => { disposed = true; };
  }, [current?.id]);

  const decide = useCallback(async (decision: LibraryCurationDecision) => {
    if (!current || busy) return;
    const itemId = current.id;
    setBusy(true);
    try {
      await api.setLibraryCurationDecision(itemId, decision);
      if (items.length <= 1) {
        await loadQueue(mode);
      } else {
        setItems((queue) => queue.filter((item) => item.id !== itemId));
        setIndex((position) => Math.min(position, Math.max(0, items.length - 2)));
      }
      setNotice({ message: decision === "keep" ? "Marked as reviewed and kept." : "Saved for another review pass.", view: "curation" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }, [busy, current, items.length, loadQueue, mode, setError, setNotice]);

  const applyAction = useCallback(async (action: LibraryBulkAction, message: string) => {
    if (!current || busy) return;
    setBusy(true);
    try {
      const [updated] = await api.applyLibraryBulkAction([current.id], action);
      if (updated) setItems((queue) => queue.map((item) => item.id === updated.id ? updated : item));
      setNotice({ message, view: "curation" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }, [busy, current, setError, setNotice]);

  const openSystem = useCallback(async () => {
    if (!current) return;
    try {
      await api.openLibraryItem(current.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }, [current, setError]);

  const openLibraryDetails = useCallback(() => {
    if (!current) return;
    window.dispatchEvent(new CustomEvent("nice-downloader:open-library-item", { detail: { itemId: current.id } }));
    setView("library");
  }, [current, setView]);

  const moveCurrent = useCallback(() => {
    if (!current) return;
    window.dispatchEvent(new CustomEvent("nice-downloader:start-bulk-move", { detail: { itemIds: [current.id] } }));
    setView("library");
  }, [current, setView]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      if (event.key === "ArrowRight" || event.key.toLocaleLowerCase() === "j") {
        event.preventDefault();
        setIndex((position) => Math.max(0, Math.min(items.length - 1, position + 1)));
      } else if (event.key === "ArrowLeft") {
        event.preventDefault();
        setIndex((position) => Math.max(0, position - 1));
      } else if (event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        void decide("keep");
      } else if (event.key.toLocaleLowerCase() === "l") {
        event.preventDefault();
        void decide("reviewLater");
      } else if (event.key.toLocaleLowerCase() === "f") {
        event.preventDefault();
        void applyAction({ type: "setFavorite", favorite: !current?.favorite }, current?.favorite ? "Removed from favorites." : "Added to favorites.");
      } else if (/^[1-5]$/.test(event.key)) {
        event.preventDefault();
        void applyAction({ type: "setRating", rating: Number(event.key) }, `Rated ${event.key} stars.`);
      } else if (event.key.toLocaleLowerCase() === "t") {
        event.preventDefault();
        setTagOpen(true);
        window.setTimeout(() => document.getElementById("curation-tag")?.focus(), 0);
      } else if (event.key.toLocaleLowerCase() === "m") {
        event.preventDefault();
        moveCurrent();
      } else if (event.key.toLocaleLowerCase() === "o") {
        event.preventDefault();
        void openSystem();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [applyAction, current, decide, items.length, moveCurrent, openSystem]);

  const metadata = useMemo(() => current ? [
    current.mediaKind === "video" && current.durationSeconds != null ? formatDuration(current.durationSeconds) : null,
    current.width && current.height ? `${current.width} × ${current.height}` : null,
    current.fileSizeBytes ? formatBytes(current.fileSizeBytes) : null,
    current.container?.toLocaleUpperCase() || null,
  ].filter(Boolean).join(" · ") : "", [current]);

  async function addTag() {
    const value = tag.trim();
    if (!value) return;
    await applyAction({ type: "addTags", tags: [value] }, `Added #${value}.`);
    setTag("");
    setTagOpen(false);
  }

  return <div className="mx-auto flex min-h-full w-full max-w-[1700px] flex-col px-6 pb-12 pt-6 xl:px-9"><header className="flex flex-wrap items-end gap-4 border-b border-line/60 pb-5"><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-accent">Keyboard-first curation</p><h1 className="mt-1 text-3xl font-semibold tracking-[-0.045em]">Review your library quickly</h1><p className="mt-2 text-sm text-muted">Make a durable decision, add useful metadata, or send a file to the existing safe move workflow.</p></div><div className="flex border border-line/70 bg-panel/60 p-1" role="group" aria-label="Curation queue"><button className={`h-8 px-3 text-xs font-semibold ${mode === "unreviewed" ? "bg-accent text-[#081426]" : "text-muted hover:bg-elevated hover:text-ink"}`} onClick={() => setMode("unreviewed")}>Unreviewed</button><button className={`h-8 px-3 text-xs font-semibold ${mode === "reviewLater" ? "bg-accent text-[#081426]" : "text-muted hover:bg-elevated hover:text-ink"}`} onClick={() => setMode("reviewLater")}>Review later</button></div></header>

    {loading ? <div className="grid min-h-[560px] place-items-center text-sm text-muted"><span className="flex items-center gap-2"><LoaderCircle className="animate-spin text-accent" size={17} /> Loading review queue…</span></div> : !current ? <div className="grid min-h-[560px] place-items-center"><section className="max-w-xl text-center"><div className="mx-auto grid size-14 place-items-center rounded-[12px] bg-accent/10 text-accent"><ListChecks size={25} /></div><h2 className="mt-5 text-2xl font-semibold tracking-[-0.035em]">{mode === "unreviewed" ? "You’re caught up" : "Nothing waiting for another pass"}</h2><p className="mt-2 text-sm leading-6 text-muted">{mode === "unreviewed" ? "Newly indexed media will appear here until you keep it or defer it." : "Items marked Review later stay here until you make a final keep decision."}</p><Button className="mt-6" variant="secondary" onClick={() => setView("library")}><ArrowLeft size={15} /> Media home</Button></section></div> : <div className="grid min-h-0 flex-1 gap-5 pt-5 lg:grid-cols-[minmax(0,1.45fr)_minmax(340px,0.55fr)]"><section className="panel-shadow relative min-h-[500px] overflow-hidden border border-line/70 bg-[#080b10]" aria-label={`Reviewing ${current.title}`}>{artwork ? <img key={current.id} src={artwork} alt="" className="absolute inset-0 h-full w-full object-contain" /> : <div className="absolute inset-0 grid place-items-center text-white/20">{current.mediaKind === "image" ? <ImageIcon size={64} /> : <Video size={64} />}</div>}<div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black via-black/80 to-transparent px-6 pb-6 pt-24 text-white"><p className="text-[10px] font-semibold uppercase tracking-[0.15em] text-accent">{current.mediaKind || "Media"} · {index + 1} of {items.length}</p><h2 className="mt-2 text-2xl font-semibold tracking-[-0.035em]">{current.title}</h2><p className="mt-2 text-xs text-white/50">{metadata}</p></div></section><aside className="flex min-h-0 flex-col border border-line/70 bg-panel/60 p-5"><div className="flex items-start justify-between gap-4"><div><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted">Decision</p><h2 className="mt-1 text-lg font-semibold">What should happen next?</h2></div>{busy ? <LoaderCircle className="animate-spin text-accent" size={17} /> : null}</div><div className="mt-5 grid grid-cols-2 gap-2"><Button disabled={busy} onClick={() => void decide("keep")}><Check size={15} /> Keep <kbd className="ml-auto text-[9px] opacity-60">K</kbd></Button><Button variant="secondary" disabled={busy} onClick={() => void decide("reviewLater")}><ListChecks size={15} /> Later <kbd className="ml-auto text-[9px] opacity-60">L</kbd></Button></div><div className="mt-5 border-t border-line/60 pt-5"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted">Quick metadata</p><div className="mt-3 grid gap-2"><Button variant="secondary" disabled={busy} onClick={() => void applyAction({ type: "setFavorite", favorite: !current.favorite }, current.favorite ? "Removed from favorites." : "Added to favorites.")}><Heart size={15} fill={current.favorite ? "currentColor" : "none"} /> {current.favorite ? "Favorited" : "Favorite"}<kbd className="ml-auto text-[9px] text-muted">F</kbd></Button><div className="flex items-center gap-1" aria-label="Set rating">{[1, 2, 3, 4, 5].map((rating) => <button key={rating} className={`grid h-9 flex-1 place-items-center border ${current.rating && rating <= current.rating ? "border-amber-400/30 bg-amber-400/10 text-amber-300" : "border-line/60 text-muted hover:bg-elevated hover:text-ink"}`} disabled={busy} onClick={() => void applyAction({ type: "setRating", rating }, `Rated ${rating} stars.`)} aria-label={`${rating} stars`}><Star size={14} fill={current.rating && rating <= current.rating ? "currentColor" : "none"} /></button>)}</div>{tagOpen ? <div className="flex"><Input id="curation-tag" className="h-9 rounded-r-none text-xs" value={tag} onChange={(event) => setTag(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void addTag(); else if (event.key === "Escape") setTagOpen(false); }} placeholder="Tag name" /><Button className="h-9 rounded-l-none px-3" disabled={!tag.trim() || busy} onClick={() => void addTag()}>Add</Button></div> : <Button variant="secondary" disabled={busy} onClick={() => setTagOpen(true)}><Tag size={15} /> Add tag <kbd className="ml-auto text-[9px] text-muted">T</kbd></Button>}</div></div><div className="mt-5 border-t border-line/60 pt-5"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted">File and details</p><div className="mt-3 grid gap-2"><Button variant="secondary" disabled={busy} onClick={moveCurrent}><FolderInput size={15} /> Safe move <kbd className="ml-auto text-[9px] text-muted">M</kbd></Button><Button variant="secondary" onClick={openLibraryDetails}><Settings2 size={15} /> Details and organize</Button><Button variant="ghost" onClick={() => void openSystem()}><ExternalLink size={15} /> Open in system app <kbd className="ml-auto text-[9px] text-muted">O</kbd></Button></div></div><div className="mt-auto flex items-center justify-between gap-3 border-t border-line/60 pt-5"><Button variant="ghost" className="px-2" disabled={index === 0} onClick={() => setIndex((position) => Math.max(0, position - 1))}><ArrowLeft size={15} /> Previous</Button><span className="number-font text-[10px] text-muted">← / →</span><Button variant="ghost" className="px-2" disabled={index + 1 >= items.length} onClick={() => setIndex((position) => Math.min(items.length - 1, position + 1))}>Next <ArrowRight size={15} /></Button></div></aside></div>}
  </div>;
}
