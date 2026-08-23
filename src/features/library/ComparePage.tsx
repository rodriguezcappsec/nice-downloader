import {
  ArrowLeft,
  ExternalLink,
  Film,
  LayoutGrid,
  LoaderCircle,
  Pause,
  Play,
  RotateCcw,
  Save as SaveIcon,
  SlidersHorizontal,
  Trash2,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button, Input, Select, Textarea } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import {
  canOfferPlaybackFallback,
  classifyPlaybackMediaError,
  createPlaybackError,
  EMBEDDED_PLAYBACK_TIMEOUT_MS,
  isCurrentPlaybackSource,
} from "../../lib/playback";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  AppError,
  LibraryCompareSession,
  LibraryCompareSessionItem,
  LibraryItem,
  LibraryPlaybackPreflight,
  LibraryPlaybackPreparationProgress,
  LibraryPlaybackSession,
  LibrarySource,
} from "../../types";

type PendingPlayback = {
  preflight: LibraryPlaybackPreflight;
  fallback: boolean;
};

function ComparePreparationStatus({ preparation, cancelling }: { preparation: LibraryPlaybackPreparationProgress | null; cancelling: boolean }) {
  const percent = preparation?.progressPercent != null ? Math.round(preparation.progressPercent) : null;
  const label = cancelling ? "Cancelling…" : preparation?.phase === "queued" ? "Queued…" : preparation?.phase === "finalizing" ? "Finalizing…" : preparation?.reencodeRequired ? "Encoding…" : "Copying…";
  return <div className="mt-3 space-y-2" role="status" aria-live="polite"><div className="flex items-center justify-center gap-2 text-[10px] text-accent"><LoaderCircle className="animate-spin" size={12} /> {label}{percent != null ? ` ${percent}%` : ""}</div><div className="mx-auto h-1.5 max-w-xs overflow-hidden rounded-full bg-white/10" role="progressbar" aria-label="Playback copy preparation" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}><div className={`h-full bg-accent transition-[width] ${percent == null ? "w-1/3 animate-pulse" : ""}`} style={percent != null ? { width: `${percent}%` } : undefined} /></div></div>;
}

export function ComparePage() {
  const itemIds = useAppStore((state) => state.compareItemIds);
  const setCompareItems = useAppStore((state) => state.setCompareItems);
  const removeCompareItem = useAppStore((state) => state.removeCompareItem);
  const clearCompareItems = useAppStore((state) => state.clearCompareItems);
  const setView = useAppStore((state) => state.setView);
  const setNotice = useAppStore((state) => state.setNotice);
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [sources, setSources] = useState<LibrarySource[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [activeAudioId, setActiveAudioId] = useState<string | null>(itemIds[0] || null);
  const [multipleAudio, setMultipleAudio] = useState(false);
  const [readyIds, setReadyIds] = useState<Set<string>>(() => new Set());
  const [savedSessions, setSavedSessions] = useState<LibraryCompareSession[]>([]);
  const [activeSessionId, setActiveSessionId] = useState<string | null>(null);
  const [sessionName, setSessionName] = useState("");
  const [sessionNotes, setSessionNotes] = useState("");
  const [saveOpen, setSaveOpen] = useState(false);
  const [sessionBusy, setSessionBusy] = useState(false);
  const [restoreItems, setRestoreItems] = useState<Map<string, LibraryCompareSessionItem>>(() => new Map());
  const players = useRef<Map<string, HTMLVideoElement>>(new Map());

  useEffect(() => {
    let disposed = false;
    void api.listLibraryCompareSessions()
      .then((sessions) => { if (!disposed) setSavedSessions(sessions); })
      .catch(() => {});
    return () => { disposed = true; };
  }, []);

  useEffect(() => {
    window.dispatchEvent(new Event("nice-downloader:compare-start"));
    window.requestAnimationFrame(() => (document.getElementById("compare-workspace-heading") || document.getElementById("compare-choose-videos"))?.focus());
  }, []);

  useEffect(() => {
    if (activeAudioId && itemIds.includes(activeAudioId)) return;
    setActiveAudioId(itemIds[0] || null);
  }, [activeAudioId, itemIds]);

  useEffect(() => {
    let disposed = false;
    if (itemIds.length === 0) {
      setItems([]);
      setSources([]);
      setLoading(false);
      setLoadError(null);
      return;
    }
    setLoading(true);
    setLoadError(null);
    void Promise.all([api.getLibraryItemsByIds(itemIds), api.listLibrarySources()])
      .then(([loadedItems, loadedSources]) => {
        if (disposed) return;
        const byId = new Map(loadedItems.map((item) => [item.id, item]));
        const ordered = itemIds.map((id) => byId.get(id)).filter((item): item is LibraryItem => Boolean(item));
        const availableVideos = ordered.filter((item) => item.mediaKind === "video" && !item.missing);
        setItems(availableVideos);
        setSources(loadedSources);
        if (availableVideos.length !== itemIds.length) {
          setCompareItems(availableVideos.map((item) => item.id));
          setNotice({ message: "Unavailable or non-video records were removed from the comparison grid.", view: "compare" });
          window.setTimeout(() => (document.getElementById("compare-edit-selection") || document.getElementById("compare-choose-videos"))?.focus(), 0);
        }
      })
      .catch((error) => {
        if (!disposed) setLoadError(normalizeError(error).message);
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => { disposed = true; };
  }, [itemIds, setCompareItems, setNotice]);

  useEffect(() => {
    for (const [id, video] of players.current) {
      video.muted = multipleAudio ? false : id !== activeAudioId;
    }
  }, [activeAudioId, multipleAudio, readyIds]);

  const sourceNames = useMemo(() => new Map(sources.map((source) => [source.id, source.name])), [sources]);
  const validItems = items.filter((item) => item.mediaKind === "video" && !item.missing);

  useEffect(() => {
    if (validItems.length === 0 || validItems.some((item) => item.id === activeAudioId)) return;
    setActiveAudioId(validItems[0].id);
  }, [activeAudioId, validItems]);

  const registerPlayer = useCallback((itemId: string, video: HTMLVideoElement | null) => {
    if (video) players.current.set(itemId, video);
    else players.current.delete(itemId);
  }, []);

  const setPlayerReady = useCallback((itemId: string, ready: boolean) => {
    setReadyIds((current) => {
      const next = new Set(current);
      if (ready) next.add(itemId);
      else next.delete(itemId);
      return next;
    });
  }, []);

  function editSelection() {
    pauseAll();
    window.dispatchEvent(new CustomEvent("nice-downloader:start-compare-selection", { detail: { itemIds } }));
    setView("library");
    window.setTimeout(() => {
      document.getElementById("library-results")?.scrollIntoView({ block: "start" });
      document.getElementById("library-search")?.focus();
    }, 0);
  }

  function chooseVideos() {
    window.dispatchEvent(new CustomEvent("nice-downloader:start-compare-selection", { detail: { itemIds: [] } }));
    setView("library");
    window.setTimeout(() => document.getElementById("library-search")?.focus(), 0);
  }

  function activateAudio(itemId: string) {
    if (multipleAudio) return;
    setActiveAudioId(itemId);
  }

  function playAll() {
    for (const video of players.current.values()) void video.play().catch(() => {});
  }

  function pauseAll() {
    for (const video of players.current.values()) video.pause();
  }

  function restartAll() {
    for (const video of players.current.values()) video.currentTime = 0;
  }

  function seekAll(seconds: number) {
    for (const video of players.current.values()) {
      const duration = Number.isFinite(video.duration) ? video.duration : Number.MAX_SAFE_INTEGER;
      video.currentTime = Math.min(duration, Math.max(0, video.currentTime + seconds));
    }
  }

  function syncToAudioPlayer() {
    const leader = activeAudioId ? players.current.get(activeAudioId) : players.current.values().next().value;
    if (!leader || !Number.isFinite(leader.currentTime)) return;
    for (const video of players.current.values()) {
      const duration = Number.isFinite(video.duration) ? video.duration : leader.currentTime;
      video.currentTime = Math.min(duration, leader.currentTime);
    }
  }

  function toggleMultipleAudio() {
    setMultipleAudio((enabled) => !enabled);
  }

  function clearComparison() {
    pauseAll();
    clearCompareItems();
    setReadyIds(new Set());
    setActiveSessionId(null);
    setSessionName("");
    setSessionNotes("");
    setRestoreItems(new Map());
    window.setTimeout(() => document.getElementById("compare-choose-videos")?.focus(), 0);
  }

  function openSavedSession(session: LibraryCompareSession) {
    pauseAll();
    const ordered = [...session.items].sort((left, right) => left.position - right.position);
    setRestoreItems(new Map(ordered.map((item) => [item.itemId, item])));
    setCompareItems(ordered.map((item) => item.itemId));
    setActiveAudioId(session.activeAudioItemId || ordered[0]?.itemId || null);
    setMultipleAudio(session.multipleAudio);
    setActiveSessionId(session.id);
    setSessionName(session.name);
    setSessionNotes(session.notes);
    setSaveOpen(false);
  }

  async function saveComparisonSession() {
    if (sessionBusy || !sessionName.trim() || itemIds.length < 2) return;
    setSessionBusy(true);
    try {
      const saved = await api.saveLibraryCompareSession({
        id: activeSessionId,
        name: sessionName.trim(),
        notes: sessionNotes.trim(),
        items: itemIds.map((itemId, position) => {
          const player = players.current.get(itemId);
          const restored = restoreItems.get(itemId);
          return {
            itemId,
            position,
            playbackPositionSeconds: player && Number.isFinite(player.currentTime) ? player.currentTime : restored?.playbackPositionSeconds || 0,
            playbackRate: player?.playbackRate || restored?.playbackRate || 1,
          };
        }),
        activeAudioItemId: activeAudioId,
        multipleAudio,
      });
      setSavedSessions((sessions) => [saved, ...sessions.filter((session) => session.id !== saved.id)]);
      setActiveSessionId(saved.id);
      setSaveOpen(false);
      setNotice({ message: `Saved comparison session “${saved.name}”.`, view: "compare" });
    } catch (error) {
      setLoadError(normalizeError(error).message);
    } finally {
      setSessionBusy(false);
    }
  }

  async function deleteActiveSession() {
    if (!activeSessionId || sessionBusy) return;
    setSessionBusy(true);
    try {
      await api.deleteLibraryCompareSession(activeSessionId);
      setSavedSessions((sessions) => sessions.filter((session) => session.id !== activeSessionId));
      setActiveSessionId(null);
      setSessionName("");
      setSessionNotes("");
      setSaveOpen(false);
      setNotice({ message: "Deleted the saved comparison session." });
    } catch (error) {
      setLoadError(normalizeError(error).message);
    } finally {
      setSessionBusy(false);
    }
  }

  if (itemIds.length === 0) {
    return <div className="mx-auto grid min-h-[calc(100dvh-80px)] w-full max-w-[1100px] place-items-center px-8 py-16"><section className="w-full max-w-2xl text-center"><div className="mx-auto grid size-16 place-items-center rounded-[16px] border border-accent/20 bg-accent/10 text-accent"><LayoutGrid size={28} /></div><h1 className="mt-6 text-[30px] font-semibold tracking-[-0.04em]">Compare videos side by side</h1><p className="mx-auto mt-3 max-w-lg text-sm leading-6 text-muted">Choose between two and four available Library videos. Each video keeps independent controls while shared controls help you compare the same moment.</p><Button id="compare-choose-videos" className="mt-7" onClick={chooseVideos}><Film size={16} /> Choose videos from Library</Button>{savedSessions.length ? <div className="mt-10 border-t border-line/60 pt-6 text-left"><h2 className="text-xs font-semibold uppercase tracking-[0.11em] text-muted">Saved sessions</h2><div className="mt-3 grid gap-2 sm:grid-cols-2">{savedSessions.map((session) => <button key={session.id} className="rounded-[9px] border border-line/70 bg-panel/55 p-3 text-left transition hover:border-accent/35 hover:bg-elevated" onClick={() => openSavedSession(session)}><strong className="block truncate text-sm">{session.name}</strong><span className="mt-1 block text-[10px] text-muted">{session.items.length} video{session.items.length === 1 ? "" : "s"}{session.notes ? ` · ${session.notes}` : ""}</span></button>)}</div></div> : null}</section></div>;
  }

  return <div className="mx-auto w-full max-w-[1780px] px-6 pb-12 pt-5 xl:px-8">
    <header className="sticky top-0 z-20 -mx-2 flex flex-wrap items-center gap-4 border-b border-line/55 bg-canvas/94 px-2 py-3 backdrop-blur-xl">
      <button className="flex h-10 items-center gap-2 rounded-[8px] px-2.5 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => setView("library")}><ArrowLeft size={16} /> Library</button>
      <div className="h-6 w-px bg-line/70" />
      <div className="min-w-0"><p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-accent">Comparison workspace</p><h1 id="compare-workspace-heading" tabIndex={-1} className="truncate text-lg font-semibold tracking-[-0.025em] outline-none">{itemIds.length} video grid</h1></div>
      <div className="ml-auto flex flex-wrap items-center gap-2">{savedSessions.length ? <Select className="h-9 w-44 text-xs" value={activeSessionId || ""} onChange={(event) => { const session = savedSessions.find((candidate) => candidate.id === event.target.value); if (session) openSavedSession(session); }}><option value="">Saved sessions</option>{savedSessions.map((session) => <option key={session.id} value={session.id}>{session.name}</option>)}</Select> : null}<Button variant="secondary" className="h-9 text-xs" onClick={() => { if (!sessionName) setSessionName(`${items[0]?.title || "Video"} comparison`); setSaveOpen((open) => !open); }}><SaveIcon size={14} /> {activeSessionId ? "Update session" : "Save session"}</Button><Button id="compare-edit-selection" variant="secondary" className="h-9 text-xs" onClick={editSelection}><SlidersHorizontal size={14} /> Edit selection</Button><Button variant="ghost" className="h-9 text-xs" onClick={clearComparison}><Trash2 size={14} /> Clear grid</Button></div>
    </header>

    {saveOpen ? <section className="mt-3 grid gap-2 border border-line/70 bg-panel/65 p-4 md:grid-cols-[minmax(180px,0.5fr)_minmax(260px,1fr)_auto]" aria-label="Save comparison session"><Input className="h-9 text-xs" maxLength={80} value={sessionName} onChange={(event) => setSessionName(event.target.value)} placeholder="Session name" autoFocus /><Textarea className="min-h-9 text-xs" maxLength={2000} value={sessionNotes} onChange={(event) => setSessionNotes(event.target.value)} placeholder="What are you comparing?" /><div className="flex items-start gap-2"><Button className="h-9 text-xs" disabled={sessionBusy || !sessionName.trim() || itemIds.length < 2} onClick={() => void saveComparisonSession()}>{sessionBusy ? <LoaderCircle className="animate-spin" size={13} /> : <SaveIcon size={13} />} Save</Button>{activeSessionId ? <Button variant="ghost" className="h-9 text-xs text-rose-300" disabled={sessionBusy} onClick={() => void deleteActiveSession()}><Trash2 size={13} /> Delete</Button> : null}</div></section> : null}

    <section className="mt-4 flex flex-wrap items-center gap-2 border-y border-line/55 bg-panel/35 px-3 py-3" aria-label="Shared comparison controls">
      <span className="mr-2 text-[10px] font-semibold uppercase tracking-[0.11em] text-muted">All players</span>
      <Button className="h-9 text-xs" disabled={readyIds.size === 0} onClick={playAll}><Play size={14} fill="currentColor" /> Play all</Button>
      <Button variant="secondary" className="h-9 text-xs" disabled={readyIds.size === 0} onClick={pauseAll}><Pause size={14} /> Pause all</Button>
      <Button variant="secondary" className="h-9 text-xs" disabled={readyIds.size === 0} onClick={restartAll}><RotateCcw size={14} /> Restart</Button>
      <button className="h-9 rounded-[8px] px-3 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:opacity-35" disabled={readyIds.size === 0} onClick={() => seekAll(-10)}>−10 sec</button>
      <button className="h-9 rounded-[8px] px-3 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:opacity-35" disabled={readyIds.size === 0} onClick={() => seekAll(10)}>+10 sec</button>
      <Button variant="ghost" className="h-9 text-xs" disabled={readyIds.size < 2} onClick={syncToAudioPlayer}><SlidersHorizontal size={14} /> Sync to audio video</Button>
      <label className="ml-auto flex h-9 items-center gap-2 rounded-[8px] px-2.5 text-xs text-muted"><input type="checkbox" checked={multipleAudio} onChange={toggleMultipleAudio} /><span>Allow multiple audio</span></label>
      <span className="number-font text-[10px] text-muted" role="status" aria-live="polite">{readyIds.size}/{validItems.length} ready</span>
    </section>

    <p className="mt-3 text-[11px] leading-5 text-muted">Audio follows the selected tile by default. Shared synchronization uses absolute playback time and is not frame-perfect across different frame rates or codecs.</p>
    {itemIds.length < 2 ? <div className="mt-4 flex flex-wrap items-center gap-3 border-l-2 border-accent/60 bg-accent/5 px-4 py-3 text-xs text-muted"><span>A comparison needs at least one more video.</span><Button variant="ghost" className="ml-auto h-8 text-xs" onClick={editSelection}><Film size={13} /> Add another video</Button></div> : null}
    {loadError ? <div className="mt-4 border-l-2 border-amber-400/70 bg-amber-400/5 px-4 py-3 text-xs text-amber-100" role="alert">{loadError}</div> : null}
    {loading ? <div className="mt-12 flex items-center justify-center gap-2 text-sm text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={17} /> Loading comparison videos…</div> : null}

    {!loading ? <section className={`mt-5 grid items-start gap-4 ${items.length === 1 ? "grid-cols-1" : "md:grid-cols-2"}`} aria-label="Video comparison grid">
      {items.map((item, index) => <CompareVideoTile
        key={item.id}
        item={item}
        position={index + 1}
        sourceName={item.sourceId ? sourceNames.get(item.sourceId) || item.site || "Local media" : item.site || "Local media"}
        audible={multipleAudio || activeAudioId === item.id}
        multipleAudio={multipleAudio}
        initialPlayback={restoreItems.get(item.id)}
        onRequestAudio={() => activateAudio(item.id)}
        onRegister={registerPlayer}
        onReady={setPlayerReady}
        onRemove={() => {
          removeCompareItem(item.id);
          window.setTimeout(() => (document.getElementById("compare-edit-selection") || document.getElementById("compare-choose-videos"))?.focus(), 0);
        }}
      />)}
      {items.length === 3 ? <button className="hidden aspect-video min-h-56 place-items-center rounded-[12px] border border-dashed border-line/70 bg-panel/20 text-xs font-medium text-muted transition hover:border-accent/35 hover:bg-panel/40 hover:text-ink md:grid" onClick={editSelection}><span><Film className="mx-auto mb-2" size={22} />Add or replace a video</span></button> : null}
    </section> : null}
  </div>;
}

function CompareVideoTile({ item, position, sourceName, audible, multipleAudio, initialPlayback, onRequestAudio, onRegister, onReady, onRemove }: {
  item: LibraryItem;
  position: number;
  sourceName: string;
  audible: boolean;
  multipleAudio: boolean;
  initialPlayback?: LibraryCompareSessionItem;
  onRequestAudio: () => void;
  onRegister: (itemId: string, video: HTMLVideoElement | null) => void;
  onReady: (itemId: string, ready: boolean) => void;
  onRemove: () => void;
}) {
  const setError = useAppStore((state) => state.setError);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const mounted = useRef(true);
  const progress = useRef({ position: 0, duration: 0, lastSaved: 0, completed: false });
  const playbackWatchdog = useRef<number | null>(null);
  const playbackProgress = useRef<{ position: number; observedAt: number } | null>(null);
  const playbackTransition = useRef(0);
  const activePlaybackSourceUrl = useRef<string | null>(null);
  const [session, setSessionState] = useState<LibraryPlaybackSession | null>(null);
  const [pending, setPending] = useState<PendingPlayback | null>(null);
  const [loading, setLoading] = useState(!item.missing && item.mediaKind === "video");
  const [busy, setBusy] = useState(false);
  const [preparation, setPreparation] = useState<LibraryPlaybackPreparationProgress | null>(null);
  const [cancelBusy, setCancelBusy] = useState(false);
  const cancelRequested = useRef(false);
  const [playbackError, setPlaybackError] = useState<AppError | null>(null);
  const [playbackRate, setPlaybackRate] = useState(initialPlayback?.playbackRate || 1);

  function beginPlaybackTransition() {
    playbackTransition.current += 1;
    activePlaybackSourceUrl.current = null;
    setSessionState(null);
    return playbackTransition.current;
  }

  function activatePlaybackSession(prepared: LibraryPlaybackSession, transition: number) {
    if (transition !== playbackTransition.current) return false;
    activePlaybackSourceUrl.current = prepared.url;
    setSessionState(prepared);
    return true;
  }

  function isCurrentVideoEvent(video: HTMLVideoElement, eventSessionUrl: string) {
    return isCurrentPlaybackSource({
      activeSessionUrl: activePlaybackSourceUrl.current,
      eventSessionUrl,
      currentSrc: video.currentSrc,
      declaredSrc: video.src,
    });
  }

  useEffect(() => {
    if (!initialPlayback) return;
    setPlaybackRate(initialPlayback.playbackRate);
    const video = videoRef.current;
    if (!video) return;
    video.playbackRate = initialPlayback.playbackRate;
    if (Number.isFinite(video.duration) && initialPlayback.playbackPositionSeconds < video.duration) {
      video.currentTime = Math.max(0, initialPlayback.playbackPositionSeconds);
      progress.current.position = video.currentTime;
    }
  }, [initialPlayback]);

  const registerVideo = useCallback((video: HTMLVideoElement | null) => {
    videoRef.current = video;
    onRegister(item.id, video);
  }, [item.id, onRegister]);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    let disposed = false;
    const transition = beginPlaybackTransition();
    setPending(null);
    setPlaybackError(null);
    onReady(item.id, false);
    setLoading(!item.missing && item.mediaKind === "video");
    if (item.missing || item.mediaKind !== "video") return;
    void api.inspectLibraryPlayback(item.id)
      .then(async (preflight) => {
        if (disposed) return;
        if (preflight.requiresConfirmation) {
          setPending({ preflight, fallback: false });
          setLoading(false);
          return;
        }
        const prepared = await api.prepareLibraryPlayback(item.id, false);
        if (disposed) void api.releaseLibraryPlayback(item.id).catch(() => {});
        else activatePlaybackSession(prepared, transition);
      })
      .catch((error) => {
        if (!disposed) {
          setPlaybackError(normalizeError(error));
          setLoading(false);
        }
      });
    return () => { disposed = true; };
  }, [item.id, item.mediaKind, item.missing, onReady]);

  useEffect(() => {
    if (!session || !loading) return;
    const sourceUrl = session.url;
    const timer = window.setTimeout(() => {
      const video = videoRef.current;
      if (!video || !isCurrentVideoEvent(video, sourceUrl)) return;
      setPlaybackError(createPlaybackError("embedded_playback_timeout", "Embedded playback did not become ready. Try the H.264 compatibility option or your system player."));
      setLoading(false);
    }, EMBEDDED_PLAYBACK_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, [loading, session]);

  function clearPlaybackWatchdog() {
    if (playbackWatchdog.current == null) return;
    window.clearTimeout(playbackWatchdog.current);
    playbackWatchdog.current = null;
  }

  function notePlaybackProgress(video: HTMLVideoElement) {
    if (!Number.isFinite(video.currentTime)) return;
    const previous = playbackProgress.current;
    if (!previous || Math.abs(video.currentTime - previous.position) >= 0.01) {
      playbackProgress.current = { position: video.currentTime, observedAt: performance.now() };
    }
  }

  function armPlaybackWatchdog(sourceUrl: string) {
    if (playbackWatchdog.current != null) return;
    const checkProgress = () => {
      playbackWatchdog.current = null;
      const video = videoRef.current;
      if (!video || !isCurrentVideoEvent(video, sourceUrl) || video.paused || video.ended) return;
      notePlaybackProgress(video);
      const observedAt = playbackProgress.current?.observedAt ?? performance.now();
      const stalledFor = performance.now() - observedAt;
      if (stalledFor < EMBEDDED_PLAYBACK_TIMEOUT_MS) {
        playbackWatchdog.current = window.setTimeout(
          checkProgress,
          Math.max(50, EMBEDDED_PLAYBACK_TIMEOUT_MS - stalledFor),
        );
        return;
      }
      video.pause();
      onReady(item.id, false);
      setLoading(false);
      setPlaybackError(createPlaybackError(
        "embedded_playback_timeout",
        "Embedded playback stalled after starting. Try an H.264 copy or open the original in your system player.",
      ));
    };
    playbackWatchdog.current = window.setTimeout(checkProgress, EMBEDDED_PLAYBACK_TIMEOUT_MS);
  }

  function startPlaybackWatchdog(video: HTMLVideoElement, sourceUrl: string) {
    clearPlaybackWatchdog();
    if (Number.isFinite(video.currentTime)) {
      playbackProgress.current = { position: video.currentTime, observedAt: performance.now() };
    }
    armPlaybackWatchdog(sourceUrl);
  }

  function armPlaybackWatchdogWhilePlaying(video: HTMLVideoElement, sourceUrl: string) {
    if (!video.paused && !video.ended) armPlaybackWatchdog(sourceUrl);
  }

  useEffect(() => () => {
    clearPlaybackWatchdog();
    playbackProgress.current = null;
  }, [item.id, session?.url]);

  useEffect(() => {
    if (!busy || !pending) {
      setPreparation(null);
      return;
    }
    let disposed = false;
    const refresh = () => {
      void api.getLibraryPlaybackPreparation(item.id)
        .then((current) => { if (!disposed && current) setPreparation(current); })
        .catch(() => {});
    };
    refresh();
    const interval = window.setInterval(refresh, 300);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [busy, item.id, pending]);

  useEffect(() => () => {
    const snapshot = progress.current;
    if (snapshot.duration > 0) {
      void api.saveLibraryPlaybackProgress({
        itemId: item.id,
        positionSeconds: snapshot.position,
        durationSeconds: snapshot.duration,
        completed: snapshot.completed,
      }).catch(() => {});
    }
    onReady(item.id, false);
    void api.releaseLibraryPlayback(item.id).catch(() => {});
  }, [item.id, onReady]);

  function captureProgress(completed = false) {
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration) || video.duration <= 0 || !Number.isFinite(video.currentTime)) return;
    progress.current.position = Math.max(0, video.currentTime);
    progress.current.duration = video.duration;
    progress.current.completed = completed;
    if (completed || Math.abs(progress.current.position - progress.current.lastSaved) >= 5) {
      progress.current.lastSaved = progress.current.position;
      void api.saveLibraryPlaybackProgress({
        itemId: item.id,
        positionSeconds: progress.current.position,
        durationSeconds: video.duration,
        completed,
      }).catch(() => {});
    }
  }

  function onLoadedMetadata(video: HTMLVideoElement) {
    setLoading(false);
    onReady(item.id, true);
    video.playbackRate = playbackRate;
    const restoredPosition = initialPlayback?.playbackPositionSeconds;
    const saved = session?.playback;
    const position = restoredPosition != null && restoredPosition >= 0 && restoredPosition < video.duration
      ? restoredPosition
      : saved && !saved.completed && saved.positionSeconds >= 5 && saved.positionSeconds < video.duration - 10
        ? saved.positionSeconds
        : null;
    if (position != null) {
      video.currentTime = position;
      progress.current.position = position;
      progress.current.lastSaved = position;
    }
  }

  function handlePlaybackMediaError(video: HTMLVideoElement, eventSession: LibraryPlaybackSession) {
    if (!isCurrentVideoEvent(video, eventSession.url)) return;
    const failure = classifyPlaybackMediaError(video.error?.code, eventSession.liveTranscode);
    clearPlaybackWatchdog();
    onReady(item.id, false);
    setLoading(false);
    setPlaybackError(createPlaybackError(failure.code, failure.message));
  }

  async function approvePending() {
    if (!pending || busy) return;
    const transition = beginPlaybackTransition();
    setBusy(true);
    setLoading(true);
    setPreparation(null);
    cancelRequested.current = false;
    try {
      const prepared = pending.fallback
        ? await api.prepareLibraryPlaybackFallback(item.id, true)
        : await api.prepareLibraryPlayback(item.id, true);
      if (!mounted.current) {
        await api.releaseLibraryPlayback(item.id).catch(() => {});
        return;
      }
      if (!activatePlaybackSession(prepared, transition)) return;
      setPending(null);
    } catch (error) {
      if (mounted.current) {
        setPending(null);
        setPlaybackError(normalizeError(error));
        setLoading(false);
      }
    } finally {
      cancelRequested.current = false;
      if (mounted.current) setBusy(false);
    }
  }

  async function cancelPendingPreparation() {
    if (!busy || cancelBusy || !preparation) return;
    cancelRequested.current = true;
    setCancelBusy(true);
    try {
      await api.cancelLibraryPlaybackPreparation(item.id);
    } catch (error) {
      cancelRequested.current = false;
      setError(normalizeError(error));
    } finally {
      if (mounted.current) setCancelBusy(false);
    }
  }

  async function retryWithFallback() {
    if (busy) return;
    clearPlaybackWatchdog();
    const transition = beginPlaybackTransition();
    setBusy(true);
    videoRef.current?.pause();
    captureProgress(false);
    onReady(item.id, false);
    setPlaybackError(null);
    setLoading(true);
    try {
      await api.releaseLibraryPlayback(item.id);
      if (!mounted.current) return;
      const preflight = await api.inspectLibraryPlaybackFallback(item.id);
      if (!mounted.current) return;
      if (preflight.requiresConfirmation) {
        setPending({ preflight, fallback: true });
        setLoading(false);
      } else {
        const prepared = await api.prepareLibraryPlaybackFallback(item.id, false);
        if (!mounted.current) {
          await api.releaseLibraryPlayback(item.id).catch(() => {});
          return;
        }
        activatePlaybackSession(prepared, transition);
      }
    } catch (error) {
      if (mounted.current) {
        setPlaybackError(normalizeError(error));
        setLoading(false);
      }
    } finally {
      if (mounted.current) setBusy(false);
    }
  }

  async function openExternal() {
    try {
      videoRef.current?.pause();
      captureProgress(false);
      await api.openLibraryItem(item.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  const unavailable = item.missing || item.mediaKind !== "video";
  return <article className="panel-shadow min-w-0 overflow-hidden rounded-[12px] border border-white/10 bg-[#06080b] text-white" aria-labelledby={`compare-title-${item.id}`}>
    <header className="flex min-h-14 items-center gap-3 border-b border-white/10 bg-black/70 px-4 py-2.5">
      <span className="number-font grid size-7 shrink-0 place-items-center rounded-[7px] bg-white/8 text-[10px] text-white/60">{position}</span>
      <div className="min-w-0 flex-1"><h2 id={`compare-title-${item.id}`} className="truncate text-xs font-semibold">{item.title}</h2><p className="mt-0.5 truncate text-[10px] text-white/45">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</p></div>
      <button className={`grid size-8 place-items-center rounded-[7px] transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${audible ? "bg-accent/15 text-accent" : "text-white/45 hover:bg-white/10 hover:text-white"}`} disabled={multipleAudio} onClick={onRequestAudio} aria-label={multipleAudio ? "Multiple audio enabled" : audible ? `${item.title} is the active audio video` : `Listen to ${item.title}`} title={multipleAudio ? "Multiple audio is enabled" : audible ? "Active audio" : "Use this video's audio"}>{audible ? <Volume2 size={15} /> : <VolumeX size={15} />}</button>
      <button className="grid size-8 place-items-center rounded-[7px] text-white/45 transition hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onRemove} aria-label={`Remove ${item.title} from comparison`}><X size={15} /></button>
    </header>
    <div className="relative flex aspect-video min-h-56 items-center justify-center overflow-hidden bg-black">
      {loading ? <div className="absolute inset-0 z-[2] flex items-center justify-center gap-2 bg-black/80 text-xs text-white/55" role="status"><LoaderCircle className="animate-spin text-accent" size={16} /> Preparing video {position}…</div> : null}
      {unavailable ? <div className="max-w-md px-7 text-center"><Film className="mx-auto text-white/30" size={30} /><h3 className="mt-3 text-sm font-semibold">Video unavailable</h3><p className="mt-2 text-xs leading-5 text-white/50">{item.missing ? "This file is missing from its indexed location." : "Only videos can be used in the comparison grid."}</p><Button variant="secondary" className="mt-4 h-8 text-xs" onClick={onRemove}>Remove tile</Button></div> : null}
      {pending ? <div className="max-w-lg px-7 py-6 text-center"><Film className="mx-auto text-amber-300/70" size={30} /><h3 className="mt-3 text-sm font-semibold">Compatibility copy required</h3><p className="mt-2 text-xs leading-5 text-white/55">{pending.preflight.reason || "This video needs a private compatibility copy for embedded playback."}</p><p className="mt-2 text-[10px] text-white/40">Estimated copy: {formatBytes(pending.preflight.estimatedCopyBytes)} · original remains unchanged</p>{busy ? <ComparePreparationStatus preparation={preparation} cancelling={cancelBusy || cancelRequested.current} /> : null}<div className="mt-4 flex flex-wrap justify-center gap-2"><Button variant="secondary" className="h-8 text-xs" disabled={cancelBusy || (busy && !preparation)} onClick={busy ? () => void cancelPendingPreparation() : () => { setPending(null); setPlaybackError(createPlaybackError("embedded_playback_copy_skipped", "Compatibility copy was skipped for this tile.")); }}><X size={13} /> {busy ? cancelBusy ? "Cancelling…" : "Cancel copy" : "Skip"}</Button><Button className="h-8 text-xs" disabled={busy} onClick={() => void approvePending()}>{busy ? <LoaderCircle className="animate-spin" size={13} /> : <Film size={13} />} Create copy</Button></div></div> : null}
      {playbackError && !pending ? <div className="max-w-lg px-7 py-6 text-center"><Film className="mx-auto text-white/30" size={30} /><h3 className="mt-3 text-sm font-semibold">Could not play this tile</h3><p className="mt-2 text-xs leading-5 text-white/50">{playbackError.message}</p><div className="mt-4 flex flex-wrap justify-center gap-2">{canOfferPlaybackFallback(playbackError) ? <Button variant="secondary" className="h-8 text-xs" disabled={busy} onClick={() => void retryWithFallback()}><RotateCcw size={13} /> Try H.264 copy</Button> : null}<Button className="h-8 text-xs" onClick={() => void openExternal()}><ExternalLink size={13} /> System player</Button></div></div> : null}
      {session && !playbackError && !pending ? <video
        key={session.url}
        ref={registerVideo}
        aria-label={`Comparison video ${position}: ${item.title}`}
        className="h-full w-full max-h-full max-w-full bg-black object-contain"
        src={session.url}
        controls
        playsInline
        crossOrigin="anonymous"
        preload="metadata"
        onPointerDown={onRequestAudio}
        onVolumeChange={() => {
          const video = videoRef.current;
          if (!multipleAudio && !audible && video && !video.muted) onRequestAudio();
        }}
        onLoadedMetadata={(event) => { if (isCurrentVideoEvent(event.currentTarget, session.url)) onLoadedMetadata(event.currentTarget); }}
        onPlay={(event) => { if (isCurrentVideoEvent(event.currentTarget, session.url)) startPlaybackWatchdog(event.currentTarget, session.url); }}
        onPlaying={(event) => { if (isCurrentVideoEvent(event.currentTarget, session.url)) clearPlaybackWatchdog(); }}
        onWaiting={(event) => { if (isCurrentVideoEvent(event.currentTarget, session.url)) armPlaybackWatchdogWhilePlaying(event.currentTarget, session.url); }}
        onStalled={(event) => { if (isCurrentVideoEvent(event.currentTarget, session.url)) armPlaybackWatchdogWhilePlaying(event.currentTarget, session.url); }}
        onTimeUpdate={(event) => { if (!isCurrentVideoEvent(event.currentTarget, session.url)) return; notePlaybackProgress(event.currentTarget); captureProgress(false); }}
        onPause={(event) => { if (!isCurrentVideoEvent(event.currentTarget, session.url)) return; clearPlaybackWatchdog(); captureProgress(false); }}
        onEnded={(event) => { if (!isCurrentVideoEvent(event.currentTarget, session.url)) return; clearPlaybackWatchdog(); captureProgress(true); }}
        onError={(event) => handlePlaybackMediaError(event.currentTarget, session)}
      >{session.subtitles.map((track) => <track key={track.url} kind="subtitles" src={track.url} srcLang={track.language || "und"} label={track.label} />)}</video> : null}
    </div>
    <footer className="flex min-h-11 flex-wrap items-center gap-3 border-t border-white/10 bg-[#0c0f14] px-4 py-2 text-[10px] text-white/40"><span>{item.width && item.height ? `${item.width} × ${item.height}` : "Resolution unknown"}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}</span><label className="ml-auto flex items-center gap-1.5"><span>Speed</span><Select className="h-7 w-20 border-white/10 bg-white/5 text-[10px] text-white" value={playbackRate} disabled={!session} onChange={(event) => { const rate = Number(event.target.value); setPlaybackRate(rate); if (videoRef.current) videoRef.current.playbackRate = rate; }}><option value={0.5}>0.5×</option><option value={0.75}>0.75×</option><option value={1}>1×</option><option value={1.25}>1.25×</option><option value={1.5}>1.5×</option><option value={2}>2×</option></Select></label><Button variant="ghost" className="h-7 px-2 text-[10px] text-white/55 hover:text-white" disabled={unavailable} onClick={() => void openExternal()}><ExternalLink size={12} /> System player</Button></footer>
  </article>;
}
