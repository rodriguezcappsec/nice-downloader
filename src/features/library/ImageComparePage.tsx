import {
  ArrowLeft,
  Columns2,
  ExternalLink,
  Images,
  Layers,
  LayoutGrid,
  LoaderCircle,
  Maximize2,
  Move,
  RotateCcw,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { LibraryImageSession, LibraryItem } from "../../types";

type CompareMode = "grid" | "overlay" | "split";
type Point = { x: number; y: number };
type DragState = Point & { pointerId: number; origin: Point };

const MIN_ZOOM = 0.25;
const MAX_ZOOM = 8;

export function ImageComparePage() {
  const itemIds = useAppStore((state) => state.imageCompareItemIds);
  const setImageCompareItems = useAppStore((state) => state.setImageCompareItems);
  const removeImageCompareItem = useAppStore((state) => state.removeImageCompareItem);
  const clearImageCompareItems = useAppStore((state) => state.clearImageCompareItems);
  const setView = useAppStore((state) => state.setView);
  const setNotice = useAppStore((state) => state.setNotice);
  const setError = useAppStore((state) => state.setError);
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [sessions, setSessions] = useState<Record<string, LibraryImageSession>>({});
  const [sessionErrors, setSessionErrors] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mode, setMode] = useState<CompareMode>("grid");
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<Point>({ x: 0, y: 0 });
  const [overlayOpacity, setOverlayOpacity] = useState(50);
  const [splitPosition, setSplitPosition] = useState(50);
  const drag = useRef<DragState | null>(null);

  useEffect(() => {
    window.dispatchEvent(new Event("nice-downloader:image-compare-start"));
    window.requestAnimationFrame(() => (document.getElementById("image-compare-heading") || document.getElementById("image-compare-choose"))?.focus());
  }, []);

  useEffect(() => {
    if (itemIds.length === 2 || mode === "grid") return;
    setMode("grid");
  }, [itemIds.length, mode]);

  useEffect(() => {
    let disposed = false;
    const requestIds = [...itemIds];
    if (requestIds.length === 0) {
      setItems([]);
      setSessions({});
      setSessionErrors({});
      setLoadError(null);
      setLoading(false);
      return () => { disposed = true; };
    }

    setLoading(true);
    setLoadError(null);
    setSessions({});
    setSessionErrors({});
    void api.getLibraryItemsByIds(requestIds)
      .then(async (loadedItems) => {
        if (disposed) return;
        const byId = new Map(loadedItems.map((item) => [item.id, item]));
        const ordered = requestIds.map((id) => byId.get(id)).filter((item): item is LibraryItem => Boolean(item));
        const availableImages = ordered.filter((item) => item.mediaKind === "image" && !item.missing);
        setItems(availableImages);
        if (availableImages.length !== requestIds.length) {
          setImageCompareItems(availableImages.map((item) => item.id));
          setNotice({ message: "Unavailable or non-image records were removed from the image studio.", view: "imageCompare" });
        }
        const prepared = await Promise.allSettled(availableImages.map((item) => api.prepareLibraryImageView(item.id)));
        if (disposed) return;
        const nextSessions: Record<string, LibraryImageSession> = {};
        const nextErrors: Record<string, string> = {};
        prepared.forEach((result, index) => {
          const item = availableImages[index];
          if (!item) return;
          if (result.status === "fulfilled") nextSessions[item.id] = result.value;
          else nextErrors[item.id] = normalizeError(result.reason).message;
        });
        setSessions(nextSessions);
        setSessionErrors(nextErrors);
      })
      .catch((error) => {
        if (!disposed) setLoadError(normalizeError(error).message);
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => { disposed = true; };
  }, [itemIds, setImageCompareItems, setNotice]);

  const resetView = useCallback(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);

  useEffect(() => {
    resetView();
  }, [itemIds, mode, resetView]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        setZoom((value) => clampZoom(value + 0.25));
      } else if (event.key === "-") {
        event.preventDefault();
        setZoom((value) => clampZoom(value - 0.25));
      } else if (event.key === "0") {
        event.preventDefault();
        resetView();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [resetView]);

  const gridClass = useMemo(() => {
    if (items.length <= 1) return "grid-cols-1";
    if (items.length === 2) return "grid-cols-1 xl:grid-cols-2";
    return "grid-cols-1 md:grid-cols-2";
  }, [items.length]);

  function editSelection() {
    window.dispatchEvent(new CustomEvent("nice-downloader:start-image-compare-selection", { detail: { itemIds } }));
    setView("library");
    window.setTimeout(() => document.getElementById("library-search")?.focus(), 0);
  }

  function chooseImages() {
    window.dispatchEvent(new CustomEvent("nice-downloader:start-image-compare-selection", { detail: { itemIds: [] } }));
    setView("library");
    window.setTimeout(() => document.getElementById("library-search")?.focus(), 0);
  }

  function clearComparison() {
    clearImageCompareItems();
    setItems([]);
    setSessions({});
    setSessionErrors({});
    setMode("grid");
    resetView();
    window.setTimeout(() => document.getElementById("image-compare-choose")?.focus(), 0);
  }

  function startPan(event: React.PointerEvent<HTMLElement>) {
    if (event.button !== 0 || zoom <= 1) return;
    drag.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, origin: pan };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function movePan(event: React.PointerEvent<HTMLElement>) {
    const start = drag.current;
    if (!start || start.pointerId !== event.pointerId) return;
    setPan({ x: start.origin.x + event.clientX - start.x, y: start.origin.y + event.clientY - start.y });
  }

  function finishPan(event: React.PointerEvent<HTMLElement>) {
    if (drag.current?.pointerId !== event.pointerId) return;
    drag.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  function zoomBy(delta: number) {
    setZoom((value) => {
      const next = clampZoom(value + delta);
      if (next === 1) setPan({ x: 0, y: 0 });
      return next;
    });
  }

  async function openExternal(item: LibraryItem) {
    try {
      await api.openLibraryItem(item.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  if (itemIds.length === 0) {
    return <div className="mx-auto grid min-h-[calc(100vh-64px)] max-w-[1500px] place-items-center px-8 py-12"><section className="panel-shadow w-full max-w-2xl border border-line/70 bg-panel/70 p-10 text-center"><div className="mx-auto grid size-14 place-items-center rounded-[12px] border border-accent/20 bg-accent/10 text-accent"><Images size={26} /></div><p className="mt-6 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Visual curation</p><h1 id="image-compare-heading" tabIndex={-1} className="mt-2 text-3xl font-semibold tracking-[-0.04em] outline-none">Compare images side by side</h1><p className="mx-auto mt-3 max-w-lg text-sm leading-6 text-muted">Choose two to four library images for a synchronized grid. With two images, inspect subtle differences using overlay and split views.</p><Button id="image-compare-choose" className="mt-7" onClick={chooseImages}><Images size={16} /> Choose images</Button></section></div>;
  }

  return <div className="mx-auto flex h-[calc(100vh-1px)] min-h-[680px] w-full max-w-[1800px] flex-col overflow-hidden px-5 pb-5 pt-4 xl:px-8">
    <header className="mb-4 flex flex-wrap items-center gap-3">
      <Button variant="ghost" className="h-9 px-2.5" onClick={() => setView("library")}><ArrowLeft size={15} /> Library</Button>
      <div className="h-6 w-px bg-line/70" />
      <div className="min-w-48 flex-1">
        <p className="text-[10px] font-semibold uppercase tracking-[0.16em] text-accent">Image comparison studio</p>
        <h1 id="image-compare-heading" tabIndex={-1} className="mt-0.5 text-xl font-semibold tracking-[-0.035em] outline-none">Inspect {items.length || itemIds.length} images together</h1>
      </div>
      <div className="flex items-center gap-1 border border-line/70 bg-panel/70 p-1" role="group" aria-label="Comparison mode">
        <ModeButton active={mode === "grid"} onClick={() => setMode("grid")} icon={<LayoutGrid size={14} />} label="Grid" />
        <ModeButton active={mode === "overlay"} disabled={itemIds.length !== 2} onClick={() => setMode("overlay")} icon={<Layers size={14} />} label="Overlay" />
        <ModeButton active={mode === "split"} disabled={itemIds.length !== 2} onClick={() => setMode("split")} icon={<Columns2 size={14} />} label="Split" />
      </div>
      <Button variant="secondary" className="h-9" onClick={editSelection}>Edit selection</Button>
      <Button variant="ghost" className="h-9" onClick={clearComparison}><RotateCcw size={14} /> Clear</Button>
    </header>

    <section className="panel-shadow flex min-h-0 flex-1 flex-col overflow-hidden border border-line/70 bg-[#0b0e13]" aria-label="Image comparison canvas">
      <div className="flex min-h-11 flex-wrap items-center gap-2 border-b border-white/10 bg-[#10141b] px-3 py-2 text-white">
        <button className="grid size-8 place-items-center text-white/70 hover:bg-white/10 hover:text-white disabled:opacity-35" disabled={zoom <= MIN_ZOOM} onClick={() => zoomBy(-0.25)} aria-label="Zoom out"><ZoomOut size={16} /></button>
        <span className="number-font min-w-12 text-center text-xs text-white/55">{Math.round(zoom * 100)}%</span>
        <button className="grid size-8 place-items-center text-white/70 hover:bg-white/10 hover:text-white disabled:opacity-35" disabled={zoom >= MAX_ZOOM} onClick={() => zoomBy(0.25)} aria-label="Zoom in"><ZoomIn size={16} /></button>
        <Button variant="ghost" className="h-8 text-white/65 hover:text-white" onClick={resetView}><Maximize2 size={14} /> Fit</Button>
        <div className="hidden items-center gap-1.5 text-[10px] text-white/35 sm:flex"><Move size={12} /> Drag to pan all images when zoomed</div>
        {mode === "overlay" ? <RangeControl label="Top image opacity" value={overlayOpacity} onChange={setOverlayOpacity} /> : null}
        {mode === "split" ? <RangeControl label="Split position" value={splitPosition} onChange={setSplitPosition} /> : null}
        <p className="ml-auto text-[10px] text-white/35">Keyboard: + / − / 0</p>
      </div>

      <div
        className={`relative min-h-0 flex-1 overflow-hidden ${zoom > 1 ? "cursor-grab active:cursor-grabbing" : "cursor-default"}`}
        onPointerDown={startPan}
        onPointerMove={movePan}
        onPointerUp={finishPan}
        onPointerCancel={finishPan}
      >
        {loading ? <div className="absolute inset-0 z-20 grid place-items-center bg-black/60"><span className="flex items-center gap-2 text-sm text-white/65"><LoaderCircle className="animate-spin text-accent" size={18} /> Preparing secure image views…</span></div> : null}
        {loadError ? <div className="absolute inset-0 z-20 grid place-items-center p-8"><div className="max-w-lg border border-red-400/20 bg-red-400/5 p-6 text-center text-sm text-red-200"><p className="font-semibold">The image studio could not load this selection.</p><p className="mt-2 text-xs leading-5 text-red-200/70">{loadError}</p><Button variant="secondary" className="mt-5" onClick={editSelection}>Return to selection</Button></div></div> : null}
        {!loading && !loadError && items.length < 2 ? <div className="absolute inset-0 z-10 grid place-items-center p-8"><div className="max-w-lg text-center text-white"><Images className="mx-auto text-white/25" size={36} /><h2 className="mt-4 text-lg font-semibold">Choose at least two available images</h2><p className="mt-2 text-sm text-white/45">The selection changed while the studio was opening.</p><Button className="mt-5" onClick={editSelection}>Edit selection</Button></div></div> : null}

        {items.length >= 2 && mode === "grid" ? <div className={`grid h-full min-h-0 gap-px overflow-hidden bg-white/10 ${gridClass}`}>{items.map((item) => <ImageCell key={item.id} item={item} session={sessions[item.id]} error={sessionErrors[item.id]} zoom={zoom} pan={pan} onRemove={() => removeImageCompareItem(item.id)} onOpenExternal={() => void openExternal(item)} />)}</div> : null}
        {items.length === 2 && mode !== "grid" ? <CompositeView mode={mode} items={items} sessions={sessions} errors={sessionErrors} zoom={zoom} pan={pan} overlayOpacity={overlayOpacity} splitPosition={splitPosition} onOpenExternal={(item) => void openExternal(item)} /> : null}
      </div>
    </section>
  </div>;
}

function ModeButton({ active, disabled, onClick, icon, label }: { active: boolean; disabled?: boolean; onClick: () => void; icon: React.ReactNode; label: string }) {
  return <button className={`flex h-7 items-center gap-1.5 px-2.5 text-[11px] font-semibold transition ${active ? "bg-accent text-[#081426]" : "text-muted hover:bg-elevated hover:text-ink"}`} disabled={disabled} onClick={onClick} aria-pressed={active} title={disabled ? `${label} view requires exactly two images` : undefined}>{icon}{label}</button>;
}

function RangeControl({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return <label className="ml-2 flex min-w-56 flex-1 items-center gap-3 text-[10px] text-white/50 sm:max-w-sm"><span className="shrink-0">{label}</span><input className="h-1 flex-1 accent-[rgb(var(--accent))]" type="range" min={0} max={100} value={value} onChange={(event) => onChange(Number(event.target.value))} /><span className="number-font w-8 text-right">{value}%</span></label>;
}

function ImageCell({ item, session, error, zoom, pan, onRemove, onOpenExternal }: { item: LibraryItem; session?: LibraryImageSession; error?: string; zoom: number; pan: Point; onRemove: () => void; onOpenExternal: () => void }) {
  const [decodeFailed, setDecodeFailed] = useState(false);
  return <article className="relative min-h-0 overflow-hidden bg-[#080a0e]">
    <div className="pointer-events-none absolute inset-x-0 top-0 z-10 flex items-start justify-between gap-4 bg-gradient-to-b from-black/75 to-transparent p-3 text-white">
      <div className="min-w-0"><h2 className="truncate text-xs font-semibold">{item.title}</h2><p className="mt-0.5 text-[10px] text-white/45">{imageMeta(item)}</p></div>
      <div className="pointer-events-auto flex shrink-0 gap-1"><button className="grid size-7 place-items-center bg-black/30 text-white/65 hover:bg-white/10 hover:text-white" onClick={onOpenExternal} aria-label={`Open ${item.title} in system viewer`}><ExternalLink size={13} /></button><button className="grid size-7 place-items-center bg-black/30 text-white/65 hover:bg-red-400/15 hover:text-red-200" onClick={onRemove} aria-label={`Remove ${item.title} from comparison`}><X size={14} /></button></div>
    </div>
    {session && !decodeFailed ? <img src={session.url} alt={item.title} draggable={false} className="h-full w-full select-none object-contain [image-orientation:from-image] will-change-transform" style={imageTransform(zoom, pan)} onError={() => setDecodeFailed(true)} /> : <ImageFailure message={decodeFailed ? "The WebView could not decode this image." : error || "Preparing image…"} onOpenExternal={onOpenExternal} />}
  </article>;
}

function CompositeView({ mode, items, sessions, errors, zoom, pan, overlayOpacity, splitPosition, onOpenExternal }: { mode: Exclude<CompareMode, "grid">; items: LibraryItem[]; sessions: Record<string, LibraryImageSession>; errors: Record<string, string>; zoom: number; pan: Point; overlayOpacity: number; splitPosition: number; onOpenExternal: (item: LibraryItem) => void }) {
  const [failedIds, setFailedIds] = useState<Set<string>>(() => new Set());
  const first = items[0];
  const second = items[1];
  useEffect(() => setFailedIds(new Set()), [first?.id, second?.id]);
  if (!first || !second) return null;
  const firstSession = sessions[first.id];
  const secondSession = sessions[second.id];
  return <div className="relative h-full w-full overflow-hidden bg-[#080a0e]">
    {firstSession && !failedIds.has(first.id) ? <img src={firstSession.url} alt={first.title} draggable={false} className="absolute inset-0 h-full w-full select-none object-contain [image-orientation:from-image] will-change-transform" style={imageTransform(zoom, pan)} onError={() => setFailedIds((current) => new Set(current).add(first.id))} /> : <ImageFailure message={failedIds.has(first.id) ? "The WebView could not decode the first image." : errors[first.id] || "Preparing first image…"} onOpenExternal={() => onOpenExternal(first)} />}
    {secondSession && !failedIds.has(second.id) ? <div className="pointer-events-none absolute inset-0 overflow-hidden" style={mode === "overlay" ? { opacity: overlayOpacity / 100 } : { clipPath: `inset(0 ${100 - splitPosition}% 0 0)` }}><img src={secondSession.url} alt={second.title} draggable={false} className="absolute inset-0 h-full w-full select-none object-contain [image-orientation:from-image] will-change-transform" style={imageTransform(zoom, pan)} onError={() => setFailedIds((current) => new Set(current).add(second.id))} /></div> : null}
    {mode === "split" ? <div className="pointer-events-none absolute inset-y-0 z-10 w-px bg-white shadow-[0_0_0_1px_rgb(0_0_0/0.7)]" style={{ left: `${splitPosition}%` }}><span className="absolute left-1/2 top-1/2 grid size-6 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-white text-black shadow-lg"><Columns2 size={12} /></span></div> : null}
    <ImageLabel side="left" item={first} onOpenExternal={() => onOpenExternal(first)} />
    <ImageLabel side="right" item={second} onOpenExternal={() => onOpenExternal(second)} />
  </div>;
}

function ImageLabel({ side, item, onOpenExternal }: { side: "left" | "right"; item: LibraryItem; onOpenExternal: () => void }) {
  return <div className={`absolute top-3 z-20 flex max-w-[42%] items-center gap-2 bg-black/65 px-3 py-2 text-white backdrop-blur ${side === "left" ? "left-3" : "right-3 flex-row-reverse text-right"}`}><div className="min-w-0"><p className="truncate text-xs font-semibold">{item.title}</p><p className="text-[10px] text-white/45">{imageMeta(item)}</p></div><button className="grid size-7 shrink-0 place-items-center text-white/60 hover:bg-white/10 hover:text-white" onClick={onOpenExternal} aria-label={`Open ${item.title} in system viewer`}><ExternalLink size={13} /></button></div>;
}

function ImageFailure({ message, onOpenExternal }: { message: string; onOpenExternal: () => void }) {
  return <div className="absolute inset-0 grid place-items-center p-8 text-center text-white"><div className="max-w-sm"><Images className="mx-auto text-white/20" size={30} /><p className="mt-3 text-xs font-medium text-white/70">Embedded view unavailable</p><p className="mt-1 line-clamp-3 text-[10px] leading-4 text-white/35">{message}</p><Button variant="ghost" className="mt-3 h-8 text-white/65 hover:text-white" onClick={onOpenExternal}><ExternalLink size={13} /> System viewer</Button></div></div>;
}

function imageTransform(zoom: number, pan: Point): React.CSSProperties {
  return { transform: `translate3d(${pan.x}px, ${pan.y}px, 0) scale(${zoom})`, transformOrigin: "center" };
}

function imageMeta(item: LibraryItem): string {
  const dimensions = item.width && item.height ? `${item.width} × ${item.height}` : "Unknown size";
  return item.fileSizeBytes ? `${dimensions} · ${formatBytes(item.fileSizeBytes)}` : dimensions;
}

function clampZoom(value: number): number {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(value * 100) / 100));
}
