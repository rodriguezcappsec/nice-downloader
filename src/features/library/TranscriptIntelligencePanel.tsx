import { open } from "@tauri-apps/plugin-dialog";
import {
  BrainCircuit,
  Captions,
  CheckCircle2,
  ChevronRight,
  FileAudio,
  LoaderCircle,
  Play,
  RefreshCw,
  ServerOff,
  Square,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Input, Select } from "../../components/ui";
import { formatDuration } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryLocalInferenceJob,
  LibrarySemanticIndexStatus,
  LibrarySemanticTranscriptMatch,
  LibraryTranscriptionProviderStatus,
} from "../../types";

const DEFAULT_EMBEDDING_MODEL = "nomic-embed-text";

interface TranscriptIntelligencePanelProps {
  query: string;
  selectedItemIds: string[];
  onPlay: (match: LibrarySemanticTranscriptMatch) => void;
  onTranscriptChanged: () => void;
}

export function TranscriptIntelligencePanel({
  query,
  selectedItemIds,
  onPlay,
  onTranscriptChanged,
}: TranscriptIntelligencePanelProps) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [expanded, setExpanded] = useState(false);
  const [meaningSearch, setMeaningSearch] = useState(false);
  const [model, setModel] = useState(
    () => window.localStorage.getItem("nice-downloader.semantic-transcript-model") || DEFAULT_EMBEDDING_MODEL,
  );
  const [maxVideos, setMaxVideos] = useState(100);
  const [modelPath, setModelPath] = useState("");
  const [language, setLanguage] = useState("auto");
  const [provider, setProvider] = useState<LibraryTranscriptionProviderStatus | null>(null);
  const [indexStatus, setIndexStatus] = useState<LibrarySemanticIndexStatus | null>(null);
  const [job, setJob] = useState<LibraryLocalInferenceJob | null>(null);
  const [matches, setMatches] = useState<LibrarySemanticTranscriptMatch[]>([]);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const terminalJob = useRef<string | null>(null);
  const normalizedModel = model.trim() || DEFAULT_EMBEDDING_MODEL;
  const running = job?.state === "running";

  const progress = useMemo(() => {
    if (!job?.totalItems) return 0;
    return Math.min(100, Math.round((job.checkedItems / job.totalItems) * 100));
  }, [job]);

  async function refresh() {
    const [nextProvider, nextJob, nextStatus] = await Promise.all([
      api.getLibraryTranscriptionProviderStatus(),
      api.getLibraryLocalInferenceJob(),
      api.getLibrarySemanticTranscriptIndexStatus(normalizedModel),
    ]);
    setProvider(nextProvider);
    setJob(nextJob);
    setIndexStatus(nextStatus);
  }

  useEffect(() => {
    void refresh().catch((error) => setError(normalizeError(error)));
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void api.onLibraryLocalInferenceUpdated((nextJob) => {
      if (disposed) return;
      setJob(nextJob);
      if (nextJob.state !== "running" && terminalJob.current !== nextJob.id) {
        terminalJob.current = nextJob.id;
        const statusModel = nextJob.kind === "semanticIndex"
          ? nextJob.model
          : window.localStorage.getItem("nice-downloader.semantic-transcript-model") || DEFAULT_EMBEDDING_MODEL;
        void api.getLibrarySemanticTranscriptIndexStatus(statusModel).then(setIndexStatus).catch(() => undefined);
        if (nextJob.kind === "transcription" && nextJob.state === "completed") {
          onTranscriptChanged();
        }
      }
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
    // The event listener is intentionally process-scoped; terminal events carry their model.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!meaningSearch || query.trim().length < 2 || !indexStatus?.indexedItems) {
      setMatches([]);
      setLoading(false);
      return;
    }
    let disposed = false;
    setLoading(true);
    void api.searchLibrarySemanticTranscripts(query.trim(), normalizedModel, 12)
      .then((results) => {
        if (!disposed) setMatches(results);
      })
      .catch((error) => {
        if (!disposed) {
          setMatches([]);
          setError(normalizeError(error));
        }
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [indexStatus?.indexedItems, meaningSearch, normalizedModel, query, setError]);

  async function chooseWhisperModel() {
    const selected = await open({
      title: "Choose a whisper.cpp model",
      directory: false,
      multiple: false,
      filters: [{ name: "whisper.cpp model", extensions: ["bin", "gguf"] }],
    });
    if (typeof selected === "string") setModelPath(selected);
  }

  async function startTranscription() {
    if (!selectedItemIds.length || !modelPath) return;
    setBusy(true);
    try {
      const selectedItems = await api.getLibraryItemsByIds(selectedItemIds.slice(0, 100));
      const videoIds = selectedItems
        .filter((item) => !item.missing && (item.mediaKind === "video" || item.mediaKind === "both"))
        .map((item) => item.id);
      if (!videoIds.length) {
        setError({ code: "no_transcription_videos", message: "Select at least one available video to transcribe." });
        return;
      }
      const nextJob = await api.startLibraryTranscription({
        itemIds: videoIds,
        modelPath,
        language: language === "auto" ? null : language,
      });
      setJob(nextJob);
      setNotice({
        message: `Started private local transcription for ${nextJob.totalItems} video${nextJob.totalItems === 1 ? "" : "s"}.`,
        view: "library",
      });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function buildSemanticIndex() {
    setBusy(true);
    try {
      const selectedModel = normalizedModel;
      window.localStorage.setItem("nice-downloader.semantic-transcript-model", selectedModel);
      const nextJob = await api.startLibrarySemanticTranscriptIndex({
        itemIds: selectedItemIds.slice(0, 500),
        maxVideos: Math.min(500, Math.max(1, Math.trunc(maxVideos) || 1)),
        model: selectedModel,
      });
      setModel(selectedModel);
      setJob(nextJob);
      setIndexStatus(await api.getLibrarySemanticTranscriptIndexStatus(selectedModel));
      setNotice({
        message: nextJob.totalItems
          ? `Building meaning search for ${nextJob.totalItems} transcript${nextJob.totalItems === 1 ? "" : "s"}.`
          : "Those transcripts are already current for this embedding model.",
        view: "library",
      });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function cancelJob() {
    if (!job || job.state !== "running") return;
    try {
      await api.cancelLibraryLocalInferenceJob(job.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  return (
    <section className="mt-5 overflow-hidden rounded-[11px] border border-line/60 bg-panel/45" aria-labelledby="transcript-intelligence-title">
      <header className="flex flex-wrap items-center gap-3 px-4 py-3">
        <span className="grid size-8 place-items-center rounded-[7px] bg-accent/10 text-accent"><BrainCircuit size={16} /></span>
        <span className="min-w-0 flex-1">
          <h2 id="transcript-intelligence-title" className="text-xs font-semibold">Local transcript intelligence</h2>
          <p className="mt-0.5 text-[10px] text-muted">Create transcripts with whisper.cpp, then search their meaning with a local Ollama embedding model.</p>
        </span>
        {query.trim().length >= 2 ? (
          <Button
            variant={meaningSearch ? "primary" : "secondary"}
            className="h-8 text-xs"
            disabled={!indexStatus?.indexedItems}
            onClick={() => setMeaningSearch((value) => !value)}
          >
            <BrainCircuit size={13} /> {meaningSearch ? "Meaning search on" : "Search by meaning"}
          </Button>
        ) : null}
        <Button variant="ghost" className="h-8 text-xs" onClick={() => setExpanded((value) => !value)} aria-expanded={expanded}>
          Setup <ChevronRight className={`transition-transform ${expanded ? "rotate-90" : ""}`} size={13} />
        </Button>
      </header>

      {expanded ? (
        <div className="grid gap-4 border-t border-line/55 p-4 lg:grid-cols-2">
          <article className="rounded-[9px] border border-line/55 bg-elevated/25 p-4">
            <div className="flex items-start gap-3">
              <span className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${provider?.available ? "bg-accent/10 text-accent" : "bg-red-400/10 text-red-300"}`}>
                {provider?.available ? <FileAudio size={17} /> : <ServerOff size={17} />}
              </span>
              <div className="min-w-0"><h3 className="text-xs font-semibold">1. Transcribe selected videos</h3><p className="mt-1 text-[10px] leading-4 text-muted">{provider?.detail || "Checking for whisper.cpp…"}</p></div>
            </div>
            <div className="mt-4 flex gap-2">
              <Input className="min-w-0 flex-1 text-xs" value={modelPath} readOnly placeholder="Choose a local .bin or .gguf model" />
              <Button variant="secondary" className="shrink-0 text-xs" onClick={() => void chooseWhisperModel()}>Choose model</Button>
            </div>
            <div className="mt-3 flex flex-wrap items-end gap-2">
              <label className="min-w-28 flex-1 text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Language<Select className="mt-1 normal-case" value={language} onChange={(event) => setLanguage(event.target.value)}><option value="auto">Auto detect</option><option value="en">English</option><option value="es">Spanish</option><option value="fr">French</option><option value="de">German</option><option value="ja">Japanese</option><option value="ko">Korean</option><option value="zh">Chinese</option></Select></label>
              <Button disabled={busy || running || !provider?.available || !modelPath || selectedItemIds.length === 0} onClick={() => void startTranscription()}><Captions size={14} /> Transcribe {selectedItemIds.length ? `${Math.min(100, selectedItemIds.length)} selected` : "selection"}</Button>
            </div>
            <p className="mt-3 text-[9px] leading-4 text-muted">Audio is extracted into a temporary private workspace and removed after each run. Originals are never changed. Select videos in the Library first.</p>
          </article>

          <article className="rounded-[9px] border border-line/55 bg-elevated/25 p-4">
            <div className="flex items-start gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-accent/10 text-accent"><BrainCircuit size={17} /></span>
              <div className="min-w-0"><h3 className="text-xs font-semibold">2. Build meaning search</h3><p className="mt-1 text-[10px] leading-4 text-muted">Ollama is contacted only at 127.0.0.1. Transcript text is not sent to a cloud service.</p></div>
            </div>
            <div className="mt-4 grid gap-2 sm:grid-cols-[minmax(0,1fr)_110px]">
              <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Embedding model<Input className="mt-1 normal-case" value={model} maxLength={100} onChange={(event) => setModel(event.target.value)} /></label>
              <label className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Maximum<Input className="number-font mt-1 normal-case" type="number" min={1} max={500} value={maxVideos} onChange={(event) => setMaxVideos(Number(event.target.value))} /></label>
            </div>
            <div className="mt-3 flex flex-wrap items-center gap-2">
              <Button disabled={busy || running} onClick={() => void buildSemanticIndex()}><RefreshCw size={14} /> {selectedItemIds.length ? "Index selected transcripts" : "Index Library transcripts"}</Button>
              <span className="text-[10px] text-muted">{indexStatus ? `${indexStatus.indexedItems} ready · ${indexStatus.staleItems} remaining` : "Index status unavailable"}</span>
            </div>
          </article>

          {job ? (
            <div className="rounded-[9px] border border-line/55 bg-panel/60 p-4 lg:col-span-2">
              <div className="flex items-center gap-3">
                {job.state === "running" ? <LoaderCircle className="animate-spin text-accent" size={16} /> : <CheckCircle2 className="text-accent" size={16} />}
                <div className="min-w-0 flex-1"><p className="truncate text-xs font-semibold">{job.currentTitle || (job.kind === "transcription" ? "Local transcription" : "Semantic transcript index")}</p><p className="mt-1 text-[10px] text-muted">{job.completedItems} completed · {job.failedItems} skipped · {job.checkedItems}/{job.totalItems}</p></div>
                {job.state === "running" ? <Button variant="secondary" className="h-8 text-xs" onClick={() => void cancelJob()}><Square size={12} /> Cancel</Button> : null}
              </div>
              {job.state === "running" ? <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-elevated"><div className="h-full bg-accent transition-[width]" style={{ width: `${progress}%` }} /></div> : null}
              {job.error ? <p className="mt-2 text-[10px] text-red-300">{job.error}</p> : null}
            </div>
          ) : null}
        </div>
      ) : null}

      {meaningSearch ? (
        <div className="border-t border-line/55">
          {loading ? <p className="flex items-center gap-2 px-4 py-5 text-xs text-muted"><LoaderCircle className="animate-spin text-accent" size={14} /> Comparing local transcript meanings…</p> : matches.length ? (
            <div className="divide-y divide-line/45">{matches.map((match) => (
              <button key={match.item.id} className="group grid w-full grid-cols-[70px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-3 text-left transition hover:bg-elevated/55" onClick={() => onPlay(match)}>
                <span className="text-center"><span className="number-font block rounded-[6px] bg-elevated px-2 py-1 text-[10px] font-semibold text-accent">{Math.round(match.scorePercent)}% match</span><span className="number-font mt-1 block text-[9px] text-muted">{formatDuration(match.startSeconds)}</span></span>
                <span className="min-w-0"><strong className="block truncate text-xs font-semibold">{match.item.title}</strong><span className="mt-1 line-clamp-2 block text-[10px] leading-4 text-muted">“{match.excerpt}”</span></span>
                <span className="flex items-center gap-1 text-[10px] font-semibold text-muted group-hover:text-accent"><Play size={12} /> Play</span>
              </button>
            ))}</div>
          ) : <p className="px-4 py-5 text-xs text-muted">No semantic transcript matches. Build the meaning index or try a broader description.</p>}
        </div>
      ) : null}
    </section>
  );
}
