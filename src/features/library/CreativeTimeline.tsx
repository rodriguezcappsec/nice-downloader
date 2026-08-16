import { save } from "@tauri-apps/plugin-dialog";
import {
  BookmarkPlus,
  Clock3,
  Download,
  Film,
  FileText,
  Image as ImageIcon,
  LoaderCircle,
  Play,
  Scissors,
  Trash2,
  WandSparkles,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Input, Select, Textarea } from "../../components/ui";
import { formatDuration } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryItem,
  LibraryChapterSuggestion,
  LibraryMoment,
  LibraryMomentColor,
  LibraryStoryboard,
} from "../../types";

const MOMENT_COLORS: Array<{ value: LibraryMomentColor; label: string }> = [
  { value: "accent", label: "Accent" },
  { value: "blue", label: "Blue" },
  { value: "amber", label: "Amber" },
  { value: "green", label: "Green" },
  { value: "rose", label: "Rose" },
];

const COLOR_CLASS: Record<LibraryMomentColor, string> = {
  accent: "bg-accent",
  blue: "bg-blue-400",
  amber: "bg-amber-400",
  green: "bg-emerald-400",
  rose: "bg-rose-400",
};

export function CreativeTimeline({
  item,
  currentTime,
  duration,
  playbackReady,
  onSeek,
}: {
  item: LibraryItem;
  currentTime: number;
  duration: number;
  playbackReady: boolean;
  onSeek: (seconds: number) => void;
}) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [moments, setMoments] = useState<LibraryMoment[]>([]);
  const [storyboard, setStoryboard] = useState<LibraryStoryboard | null>(null);
  const [loadingMoments, setLoadingMoments] = useState(true);
  const [storyboardBusy, setStoryboardBusy] = useState(false);
  const [saving, setSaving] = useState(false);
  const [exportingId, setExportingId] = useState<string | null>(null);
  const [exportingNotes, setExportingNotes] = useState(false);
  const [suggestions, setSuggestions] = useState<LibraryChapterSuggestion[]>([]);
  const [suggestionBusy, setSuggestionBusy] = useState(false);
  const [addingSuggestion, setAddingSuggestion] = useState<number | null>(null);
  const [label, setLabel] = useState("");
  const [note, setNote] = useState("");
  const [color, setColor] = useState<LibraryMomentColor>("accent");
  const [markIn, setMarkIn] = useState<number | null>(null);
  const [markOut, setMarkOut] = useState<number | null>(null);
  const requestRevision = useRef(0);

  useEffect(() => {
    const revision = ++requestRevision.current;
    setLoadingMoments(true);
    setMoments([]);
    setStoryboard(null);
    setLabel("");
    setNote("");
    setMarkIn(null);
    setMarkOut(null);
    setSuggestions([]);
    void api.listLibraryMoments(item.id)
      .then((loaded) => {
        if (requestRevision.current === revision) setMoments(loaded);
      })
      .catch((error) => {
        if (requestRevision.current === revision) setError(normalizeError(error));
      })
      .finally(() => {
        if (requestRevision.current === revision) setLoadingMoments(false);
      });
  }, [item.id, setError]);

  const safeDuration = Number.isFinite(duration) && duration > 0
    ? duration
    : item.durationSeconds || 0;
  const boundedTime = Math.min(Math.max(0, currentTime), Math.max(0, safeDuration));
  const validRange = markIn != null && markOut != null && markOut > markIn;
  const sortedMoments = useMemo(
    () => [...moments].sort((left, right) => left.startSeconds - right.startSeconds || left.createdAt.localeCompare(right.createdAt)),
    [moments],
  );

  function captureStart() {
    setMarkIn(boundedTime);
    if (markOut != null && markOut <= boundedTime) setMarkOut(null);
  }

  function captureEnd() {
    if (markIn == null) {
      setMarkIn(boundedTime);
      setMarkOut(null);
      return;
    }
    setMarkOut(boundedTime > markIn ? boundedTime : null);
  }

  async function saveMoment(pointOnly = false) {
    if (saving || !label.trim()) return;
    const start = pointOnly ? boundedTime : markIn ?? boundedTime;
    const end = pointOnly ? null : validRange ? markOut : null;
    setSaving(true);
    try {
      const saved = await api.saveLibraryMoment({
        itemId: item.id,
        label: label.trim(),
        note: note.trim(),
        startSeconds: start,
        endSeconds: end,
        color,
      });
      setMoments((current) => [...current.filter((moment) => moment.id !== saved.id), saved]);
      setLabel("");
      setNote("");
      setMarkIn(null);
      setMarkOut(null);
      setNotice({ message: end == null ? "Saved a timestamped moment." : "Saved a clip range." });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSaving(false);
    }
  }

  async function removeMoment(moment: LibraryMoment) {
    try {
      await api.deleteLibraryMoment(moment.id);
      setMoments((current) => current.filter((candidate) => candidate.id !== moment.id));
      setNotice({ message: `Removed “${moment.label}”.` });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function generateStoryboard() {
    if (storyboardBusy) return;
    setStoryboardBusy(true);
    try {
      const generated = await api.generateLibraryStoryboard(item.id, 12);
      setStoryboard(generated);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setStoryboardBusy(false);
    }
  }

  async function exportMoment(moment: LibraryMoment) {
    if (moment.endSeconds == null || exportingId) return;
    const extension = clipExtension(item.container);
    const outputPath = await save({
      title: "Export video clip",
      defaultPath: `${safeFileStem(item.title)}-${safeFileStem(moment.label)}.${extension}`,
      filters: [{ name: "Video", extensions: [extension] }],
    });
    if (!outputPath) return;
    setExportingId(moment.id);
    try {
      await api.startLibraryClipExport({
        itemId: item.id,
        outputPath,
        startSeconds: moment.startSeconds,
        endSeconds: moment.endSeconds,
        overwrite: false,
      });
      setNotice({ message: "Clip export started. Track its progress in Transformations.", view: "transformations" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setExportingId(null);
    }
  }

  async function exportNotes(format: "markdown" | "csv") {
    if (exportingNotes || moments.length === 0) return;
    const extension = format === "markdown" ? "md" : "csv";
    const outputPath = await save({
      title: "Export saved moment notes",
      defaultPath: `${safeFileStem(item.title)}-moments.${extension}`,
      filters: [{ name: format === "markdown" ? "Markdown" : "CSV", extensions: [extension] }],
    });
    if (!outputPath) return;
    setExportingNotes(true);
    try {
      const count = await api.exportLibraryMoments({ itemId: item.id, outputPath, format });
      setNotice({ message: `Exported ${count} saved moment${count === 1 ? "" : "s"}.` });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setExportingNotes(false);
    }
  }

  async function generateSuggestions() {
    if (suggestionBusy) return;
    setSuggestionBusy(true);
    try {
      const generated = await api.suggestLibraryChapters(item.id, 16);
      setSuggestions(generated);
      setNotice({ message: generated.length ? `Generated ${generated.length} reviewable chapter and highlight suggestions.` : "No strong local chapter evidence was found." });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSuggestionBusy(false);
    }
  }

  async function addSuggestion(suggestion: LibraryChapterSuggestion, index: number) {
    setAddingSuggestion(index);
    try {
      const saved = await api.saveLibraryMoment({
        itemId: item.id,
        label: suggestion.label,
        note: `Automatic suggestion reviewed by you. ${suggestion.reason}.`,
        startSeconds: suggestion.startSeconds,
        endSeconds: suggestion.endSeconds,
        color: suggestion.kind === "highlight" ? "amber" : "blue",
      });
      setMoments((current) => [...current, saved]);
      setSuggestions((current) => current.filter((_, candidateIndex) => candidateIndex !== index));
      setNotice({ message: suggestion.kind === "highlight" ? "Added suggested highlight as a saved range." : "Added suggested chapter as a saved moment." });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setAddingSuggestion(null);
    }
  }

  return <section className="creative-timeline border-t border-line/70 text-ink" aria-labelledby="creative-tools-title">
    <header className="flex flex-wrap items-center gap-3 border-b border-white/10 px-5 py-3">
      <div><p className="text-[9px] font-semibold uppercase tracking-[0.13em] text-accent">Creative tools</p><h2 id="creative-tools-title" className="mt-0.5 text-xs font-semibold">Moments and storyboard</h2></div>
      <p className="number-font ml-auto text-[10px] text-white/45" aria-live="polite">Playhead {formatDuration(boundedTime)}</p>
    </header>

    <div className="grid gap-5 p-5 2xl:grid-cols-[minmax(320px,0.72fr)_minmax(0,1.28fr)]">
      <div className="min-w-0">
        <div className="flex flex-wrap gap-2">
          <Button variant="secondary" className="h-8 text-[10px] text-white/75" disabled={!playbackReady} onClick={captureStart}><Clock3 size={12} /> Set start · {markIn == null ? "—" : formatDuration(markIn)}</Button>
          <Button variant="secondary" className="h-8 text-[10px] text-white/75" disabled={!playbackReady || markIn == null} onClick={captureEnd}><Clock3 size={12} /> Set end · {markOut == null ? "—" : formatDuration(markOut)}</Button>
        </div>
        <div className="mt-3 grid gap-2 sm:grid-cols-[minmax(0,1fr)_110px]">
          <Input className="h-9 border-white/10 bg-white/5 text-xs text-white" value={label} maxLength={100} onChange={(event) => setLabel(event.target.value)} placeholder="Moment label" />
          <Select className="h-9 border-white/10 bg-white/5 text-xs text-white" value={color} onChange={(event) => setColor(event.target.value as LibraryMomentColor)}>{MOMENT_COLORS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}</Select>
        </div>
        <Textarea className="mt-2 min-h-16 border-white/10 bg-white/5 text-xs text-white" value={note} maxLength={2000} onChange={(event) => setNote(event.target.value)} placeholder="Optional note" />
        <div className="mt-2 flex flex-wrap gap-2">
          <Button className="h-8 text-[10px]" disabled={saving || !playbackReady || !label.trim()} onClick={() => void saveMoment(false)}>{saving ? <LoaderCircle className="animate-spin" size={12} /> : validRange ? <Scissors size={12} /> : <BookmarkPlus size={12} />} {validRange ? "Save clip range" : "Save moment"}</Button>
          <Button variant="ghost" className="h-8 text-[10px] text-white/60" disabled={saving || !playbackReady || !label.trim()} onClick={() => void saveMoment(true)}><BookmarkPlus size={12} /> Save at playhead</Button>
        </div>
      </div>

      <div className="min-w-0 border-l-0 border-white/10 2xl:border-l 2xl:pl-5">
        <div className="flex items-center gap-3"><div><h3 className="text-xs font-semibold">Visual storyboard</h3><p className="mt-1 text-[10px] text-white/45">Generate 12 cached previews and jump directly to a scene.</p></div><Button variant="secondary" className="ml-auto h-8 text-[10px] text-white/70" disabled={storyboardBusy || safeDuration <= 0} onClick={() => void generateStoryboard()}>{storyboardBusy ? <LoaderCircle className="animate-spin" size={12} /> : <ImageIcon size={12} />} {storyboard ? "Refresh" : "Generate"}</Button></div>
        {storyboard ? <div className="mt-3 grid grid-cols-4 gap-1.5 sm:grid-cols-6" aria-label="Video storyboard">{storyboard.frames.map((frame) => <button key={frame.timeSeconds} className="group relative aspect-video overflow-hidden rounded-[5px] border border-white/10 bg-black focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/80" onClick={() => onSeek(frame.timeSeconds)} aria-label={`Seek to ${formatDuration(frame.timeSeconds)}`}><img className="h-full w-full object-cover transition duration-150 group-hover:scale-105 motion-reduce:transition-none" src={frame.imageDataUrl} alt="" /><span className="number-font absolute bottom-0.5 right-0.5 rounded-[3px] bg-black/75 px-1 py-0.5 text-[8px] text-white/80">{formatDuration(frame.timeSeconds)}</span></button>)}</div> : <div className="mt-3 grid min-h-24 place-items-center border border-dashed border-white/10 text-center text-[10px] text-white/35"><span><Film className="mx-auto mb-1.5" size={18} />No storyboard generated yet</span></div>}
      </div>
    </div>

    <div className="border-t border-white/10 px-5 py-4">
      <div className="flex flex-wrap items-center gap-3"><div className="min-w-0 flex-1"><h3 className="flex items-center gap-2 text-xs font-semibold"><WandSparkles className="text-accent" size={13} /> Automatic chapters and highlights</h3><p className="mt-1 text-[9px] leading-4 text-white/40">Uses local transcript pauses, wording, and FFmpeg scene changes. Suggestions are temporary until you explicitly add them.</p></div><Button variant="secondary" className="h-8 text-[10px] text-white/70" disabled={suggestionBusy || safeDuration <= 0} onClick={() => void generateSuggestions()}>{suggestionBusy ? <LoaderCircle className="animate-spin" size={12} /> : <WandSparkles size={12} />} {suggestions.length ? "Regenerate" : "Suggest"}</Button></div>
      {suggestions.length ? <div className="mt-3 grid gap-2 lg:grid-cols-2">{suggestions.map((suggestion, index) => <article key={`${suggestion.kind}:${suggestion.startSeconds}:${index}`} className="flex min-w-0 items-start gap-3 rounded-[7px] border border-white/10 bg-white/[0.035] p-3"><button className="number-font shrink-0 rounded-[5px] bg-white/5 px-2 py-1 text-[9px] text-accent" onClick={() => onSeek(suggestion.startSeconds)}>{formatDuration(suggestion.startSeconds)}</button><div className="min-w-0 flex-1"><div className="flex flex-wrap items-center gap-2"><strong className="truncate text-[10px] font-semibold text-white/80">{suggestion.label}</strong><span className={`rounded-[4px] px-1.5 py-0.5 text-[8px] font-semibold uppercase ${suggestion.kind === "highlight" ? "bg-amber-400/10 text-amber-200" : "bg-accent/10 text-accent"}`}>{suggestion.kind}</span><span className="number-font text-[8px] text-white/35">{Math.round(suggestion.confidence * 100)}%</span></div><p className="mt-1 line-clamp-2 text-[9px] leading-4 text-white/35">{suggestion.reason}{suggestion.endSeconds != null ? ` · ${formatDuration(suggestion.startSeconds)}–${formatDuration(suggestion.endSeconds)}` : ""}</p></div><Button className="h-7 shrink-0 px-2 text-[9px]" disabled={addingSuggestion != null} onClick={() => void addSuggestion(suggestion, index)}>{addingSuggestion === index ? <LoaderCircle className="animate-spin" size={10} /> : <BookmarkPlus size={10} />} Add</Button></article>)}</div> : null}
    </div>

    <div className="border-t border-white/10 px-5 py-4">
      <div className="mb-3 flex flex-wrap items-center gap-2"><h3 className="text-xs font-semibold">Saved moments</h3><span className="number-font text-[9px] text-white/40">{moments.length}/500</span>{moments.length ? <div className="ml-auto flex gap-1"><Button variant="ghost" className="h-7 px-2 text-[9px] text-white/45" disabled={exportingNotes} onClick={() => void exportNotes("markdown")}>{exportingNotes ? <LoaderCircle className="animate-spin" size={10} /> : <FileText size={10} />} Markdown</Button><Button variant="ghost" className="h-7 px-2 text-[9px] text-white/45" disabled={exportingNotes} onClick={() => void exportNotes("csv")}><FileText size={10} /> CSV</Button></div> : null}</div>
      {loadingMoments ? <p className="flex items-center gap-2 text-[10px] text-white/45" role="status"><LoaderCircle className="animate-spin" size={12} /> Loading moments…</p> : sortedMoments.length ? <div className="grid gap-2 lg:grid-cols-2">{sortedMoments.map((moment) => <article key={moment.id} className="flex min-w-0 items-start gap-3 rounded-[7px] border border-white/10 bg-white/[0.035] p-3"><span className={`mt-1.5 size-2 shrink-0 rounded-full ${COLOR_CLASS[moment.color]}`} /><button className="min-w-0 flex-1 text-left" onClick={() => onSeek(moment.startSeconds)}><span className="flex flex-wrap items-center gap-2"><strong className="truncate text-[11px] font-semibold text-white/85">{moment.label}</strong><span className="number-font text-[9px] text-accent">{formatDuration(moment.startSeconds)}{moment.endSeconds != null ? `–${formatDuration(moment.endSeconds)}` : ""}</span></span>{moment.note ? <span className="mt-1 line-clamp-2 block text-[9px] leading-4 text-white/40">{moment.note}</span> : null}</button><button className="grid size-7 shrink-0 place-items-center rounded-[5px] text-white/40 hover:bg-white/10 hover:text-white" onClick={() => onSeek(moment.startSeconds)} aria-label={`Play ${moment.label}`}><Play size={11} fill="currentColor" /></button>{moment.endSeconds != null ? <button className="grid size-7 shrink-0 place-items-center rounded-[5px] text-white/40 hover:bg-white/10 hover:text-white disabled:opacity-35" disabled={exportingId != null} onClick={() => void exportMoment(moment)} aria-label={`Export ${moment.label}`}>{exportingId === moment.id ? <LoaderCircle className="animate-spin" size={11} /> : <Download size={11} />}</button> : null}<button className="grid size-7 shrink-0 place-items-center rounded-[5px] text-white/35 hover:bg-rose-400/10 hover:text-rose-300" onClick={() => void removeMoment(moment)} aria-label={`Delete ${moment.label}`}><Trash2 size={11} /></button></article>)}</div> : <p className="text-[10px] text-white/35">No saved moments. Pause at an interesting scene, add a label, and save it.</p>}
    </div>
  </section>;
}

function clipExtension(container?: string | null): string {
  const normalized = container?.toLocaleLowerCase().replace(/^\./, "");
  return normalized && ["mkv", "mp4", "webm", "mov"].includes(normalized) ? normalized : "mkv";
}

function safeFileStem(value: string): string {
  const safe = value
    .normalize("NFKC")
    .replace(/[\\/:*?"<>|\u0000-\u001f]/g, "-")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/[. ]+$/g, "")
    .slice(0, 80);
  return safe || "clip";
}
