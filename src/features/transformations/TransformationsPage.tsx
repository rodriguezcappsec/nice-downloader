import { open, save } from "@tauri-apps/plugin-dialog";
import { Check, FileAudio, Film, FolderOpen, HardDrive, LoaderCircle, Scissors, Square, WandSparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { LibraryItem, PrivacyBlurRegion, PrivacyMuteRange, PrivacyRedactionSuggestion, StartTransformationRequest, TransformationOperation, TransformationTask } from "../../types";

type OperationKind = TransformationOperation["type"];

export function TransformationsPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [tasks, setTasks] = useState<TransformationTask[]>([]);
  const [inputPath, setInputPath] = useState("");
  const [outputPath, setOutputPath] = useState("");
  const [operation, setOperation] = useState<OperationKind>("trim");
  const [startSeconds, setStartSeconds] = useState("0");
  const [endSeconds, setEndSeconds] = useState("60");
  const [audioCodec, setAudioCodec] = useState<"mp3" | "aac" | "opus">("mp3");
  const [container, setContainer] = useState("mp4");
  const [videoCodec, setVideoCodec] = useState<"copy" | "h264" | "hevc" | "vp9" | "av1">("h264");
  const [transcodeAudioCodec, setTranscodeAudioCodec] = useState<"copy" | "aac" | "opus">("aac");
  const [stripMetadata, setStripMetadata] = useState(true);
  const [blurRegionsText, setBlurRegionsText] = useState("");
  const [muteRangesText, setMuteRangesText] = useState("");
  const [detectorModel, setDetectorModel] = useState("qwen2.5vl:7b");
  const [detecting, setDetecting] = useState(false);
  const [redactionSuggestions, setRedactionSuggestions] = useState<PrivacyRedactionSuggestion[]>([]);
  const [overwrite, setOverwrite] = useState(false);
  const [starting, setStarting] = useState(false);
  const [optimizerOpen, setOptimizerOpen] = useState(false);
  const [optimizerLoading, setOptimizerLoading] = useState(false);
  const [optimizerItems, setOptimizerItems] = useState<LibraryItem[]>([]);
  const [optimizerSelection, setOptimizerSelection] = useState<Set<string>>(new Set());
  const [optimizerDestination, setOptimizerDestination] = useState("");
  const [optimizerStarting, setOptimizerStarting] = useState(false);

  useEffect(() => {
    let active = true;
    let unlisten = () => {};
    void api.listTransformationTasks().then((value) => {
      if (!active) return;
      setTasks((current) => mergeTransformationTasks(value, current));
    }).catch((error) => setError(normalizeError(error)));
    void api.onTransformationUpdated((task) => {
      setTasks((current) => mergeTransformationTasks(current, [task]));
    }).then((cleanup) => { if (active) unlisten = cleanup; else cleanup(); });
    return () => { active = false; unlisten(); };
  }, [setError]);

  const request = useMemo<StartTransformationRequest | null>(() => {
    if (!inputPath || !outputPath) return null;
    let selectedOperation: TransformationOperation;
    if (operation === "trim") selectedOperation = { type: "trim", startSeconds: Number(startSeconds), endSeconds: Number(endSeconds) };
    else if (operation === "extractAudio") selectedOperation = { type: "extractAudio", codec: audioCodec };
    else if (operation === "transcode") selectedOperation = { type: "transcode", container, videoCodec, audioCodec: transcodeAudioCodec };
    else if (operation === "privacyExport") {
      const blurRegions = parseBlurRegions(blurRegionsText);
      const muteRanges = parseMuteRanges(muteRangesText);
      if (!blurRegions || !muteRanges || (!stripMetadata && blurRegions.length === 0 && muteRanges.length === 0)) return null;
      selectedOperation = { type: "privacyExport", stripMetadata, blurRegions, muteRanges };
    } else selectedOperation = { type: "splitChapters" };
    return { inputPath, outputPath, operation: selectedOperation, overwrite };
  }, [audioCodec, blurRegionsText, container, endSeconds, inputPath, muteRangesText, operation, outputPath, overwrite, startSeconds, stripMetadata, transcodeAudioCodec, videoCodec]);

  async function chooseInput() {
    const selected = await open({ multiple: false, directory: false, title: "Choose media to transform" });
    if (selected) setInputPath(selected);
  }

  async function chooseOutput() {
    if (operation === "splitChapters") {
      const selected = await open({ multiple: false, directory: true, title: "Choose chapter output folder" });
      if (selected) setOutputPath(selected);
      return;
    }
    const selected = await save({ title: "Save transformed media", defaultPath: suggestedOutput(inputPath, operation, audioCodec, container) });
    if (selected) setOutputPath(selected);
  }

  async function submit() {
    if (!request) return;
    setStarting(true);
    try {
      const task = await api.startTransformation(request);
      setTasks((current) => mergeTransformationTasks([task], current));
      setNotice({ message: "Transformation started.", view: "transformations" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setStarting(false);
    }
  }

  async function openOptimizer() {
    const opening = !optimizerOpen;
    setOptimizerOpen(opening);
    if (!opening || optimizerItems.length || optimizerLoading) return;
    setOptimizerLoading(true);
    try {
      const result = await api.queryLibraryItems({ page: 1, pageSize: 100, search: null, sourceId: null, folderSourceId: null, folderPath: null, folderScope: "tree", mediaKind: "video", resolution: null, container: null, duration: null, dateAdded: null, datePublished: null, availability: "available", tags: [], creators: [], series: [], sites: [], rating: null, collectionId: null, favoriteOnly: false, sort: "recent" });
      setOptimizerItems(result.items);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setOptimizerLoading(false);
    }
  }

  async function chooseOptimizerDestination() {
    const selected = await open({ multiple: false, directory: true, title: "Choose optimized-video output folder" });
    if (selected) setOptimizerDestination(selected);
  }

  async function startOptimization() {
    if (!optimizerSelection.size || !optimizerDestination || optimizerStarting) return;
    setOptimizerStarting(true);
    try {
      const started = await api.startLibraryOptimization([...optimizerSelection], optimizerDestination, false);
      setTasks((current) => mergeTransformationTasks(started, current));
      setOptimizerSelection(new Set());
      setNotice({ message: `Started ${started.length} compatibility optimization${started.length === 1 ? "" : "s"}. Originals remain unchanged.`, view: "transformations" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setOptimizerStarting(false);
    }
  }

  const optimizerSourceBytes = optimizerItems.filter((item) => optimizerSelection.has(item.id)).reduce((total, item) => total + (item.fileSizeBytes || 0), 0);

  async function detectPrivacyRegions() {
    if (!inputPath || !detectorModel.trim()) return;
    setDetecting(true);
    try {
      const suggestions = await api.suggestPrivacyRedactions({ inputPath, model: detectorModel.trim(), kinds: ["face", "licensePlate", "screen"] });
      setRedactionSuggestions(suggestions);
      setNotice({ message: suggestions.length ? `Found ${suggestions.length} possible privacy redaction${suggestions.length === 1 ? "" : "s"}. Review each suggestion before adding it.` : "The local detector found no confident privacy regions.", view: "transformations" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setDetecting(false);
    }
  }

  function acceptPrivacySuggestion(suggestion: PrivacyRedactionSuggestion, index: number) {
    const region = suggestion.region;
    const line = `${(region.x * 100).toFixed(2)}, ${(region.y * 100).toFixed(2)}, ${(region.width * 100).toFixed(2)}, ${(region.height * 100).toFixed(2)}, ${region.startSeconds.toFixed(2)}, ${region.endSeconds.toFixed(2)}`;
    setBlurRegionsText((current) => current.trim() ? `${current.trim()}\n${line}` : line);
    setRedactionSuggestions((current) => current.filter((_, suggestionIndex) => suggestionIndex !== index));
  }

  return (
    <div className="mx-auto w-full max-w-[1340px] px-8 pb-12 pt-8 xl:px-10">
      <header className="mb-7">
        <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Local tools</div>
        <h1 className="text-[28px] font-semibold tracking-[-0.035em]">Transform media</h1>
        <p className="mt-2 text-[13px] text-muted">Trim, extract, convert, or split local media with FFmpeg. Tasks continue in Rust when this page is hidden.</p>
      </header>

      <section className="panel-shadow mb-6 overflow-hidden rounded-[14px] border border-accent/20 bg-[linear-gradient(135deg,rgb(var(--accent)/0.08),rgb(var(--panel)/0.78)_42%)]" aria-labelledby="optimizer-title">
        <div className="flex flex-wrap items-center gap-4 px-5 py-5"><div className="grid size-11 shrink-0 place-items-center rounded-[11px] bg-accent/10 text-accent"><HardDrive size={20} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-accent">Playback readiness</p><h2 id="optimizer-title" className="mt-1 text-lg font-semibold">Optimize Library</h2><p className="mt-1 text-[12px] leading-5 text-muted">Create permanent H.264/AAC MP4 versions for selected videos. Every output is separate and the original stays untouched.</p></div><Button variant="secondary" onClick={() => void openOptimizer()}>{optimizerOpen ? "Close" : "Choose videos"}</Button></div>
        {optimizerOpen ? <div className="border-t border-line/60 px-5 py-5"><div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_360px]"><div><div className="mb-2 flex items-center justify-between gap-3"><p className="text-xs font-semibold">Available Library videos</p><button className="text-[10px] font-semibold text-accent" onClick={() => setOptimizerSelection(optimizerSelection.size === optimizerItems.length ? new Set() : new Set(optimizerItems.map((item) => item.id)))}>{optimizerSelection.size === optimizerItems.length && optimizerItems.length ? "Clear all" : "Select all shown"}</button></div><div className="max-h-72 space-y-1 overflow-y-auto rounded-[10px] border border-line/60 bg-canvas/35 p-2">{optimizerLoading ? <p className="flex items-center justify-center gap-2 py-12 text-xs text-muted"><LoaderCircle className="animate-spin" size={14} /> Loading videos…</p> : optimizerItems.length ? optimizerItems.map((item) => { const selected = optimizerSelection.has(item.id); return <button key={item.id} className={`flex w-full items-center gap-3 rounded-[8px] px-3 py-2 text-left ${selected ? "bg-accent/10 text-ink" : "text-muted hover:bg-elevated/60 hover:text-ink"}`} onClick={() => setOptimizerSelection((current) => { const next = new Set(current); if (next.has(item.id)) next.delete(item.id); else next.add(item.id); return next; })}><span className={`grid size-5 shrink-0 place-items-center rounded-[5px] border ${selected ? "border-accent bg-accent text-accentInk" : "border-line"}`}>{selected ? <Check size={12} /> : null}</span><span className="min-w-0 flex-1 truncate text-xs font-medium">{item.title}</span><span className="number-font shrink-0 text-[9px] text-muted">{item.fileSizeBytes ? formatBytes(item.fileSizeBytes) : "Unknown"}</span></button>; }) : <p className="py-12 text-center text-xs text-muted">No available Library videos were found.</p>}</div></div><aside className="rounded-[10px] border border-line/60 bg-elevated/30 p-4"><h3 className="text-xs font-semibold">Optimization plan</h3><dl className="mt-3 grid grid-cols-[1fr_auto] gap-y-2 text-[11px]"><dt className="text-muted">Selected videos</dt><dd className="number-font font-semibold">{optimizerSelection.size}</dd><dt className="text-muted">Source storage</dt><dd className="number-font font-semibold">{optimizerSourceBytes ? formatBytes(optimizerSourceBytes) : "Unknown"}</dd><dt className="text-muted">Output profile</dt><dd className="font-semibold">H.264 · AAC · MP4</dd></dl><p className="mt-3 text-[10px] leading-4 text-muted">Final size depends on the source and content. Each task is cancellable from Recent transformations.</p><Field label="Output folder"><div className="mt-2 flex gap-2"><Input className="h-9 text-xs" value={optimizerDestination} readOnly placeholder="Choose a folder" /><Button variant="secondary" className="h-9 px-3" onClick={() => void chooseOptimizerDestination()}><FolderOpen size={14} /></Button></div></Field><Button className="mt-4 w-full" disabled={!optimizerSelection.size || !optimizerDestination || optimizerStarting} onClick={() => void startOptimization()}>{optimizerStarting ? <LoaderCircle className="animate-spin" size={15} /> : <WandSparkles size={15} />} Start optimization</Button></aside></div></div> : null}
      </section>

      <section className="panel-shadow border border-line/60 bg-panel/75 p-5">
        <div className="grid gap-4 lg:grid-cols-2">
          <Field label="Input media">
            <div className="flex gap-2"><Input value={inputPath} onChange={(event) => setInputPath(event.target.value)} placeholder="Choose a local media file" /><Button variant="secondary" className="px-3" onClick={() => void chooseInput()}><FolderOpen size={17} /></Button></div>
          </Field>
          <Field label="Operation">
            <Select value={operation} onChange={(event) => { setOperation(event.target.value as OperationKind); setOutputPath(""); }}>
              <option value="trim">Trim without re-encoding</option><option value="extractAudio">Extract audio</option><option value="transcode">Convert format or codecs</option><option value="privacyExport">Create privacy-safe sharing copy</option><option value="splitChapters">Split chapters</option>
            </Select>
          </Field>
          {operation === "trim" ? <><Field label="Start (seconds)"><Input type="number" min="0" step="0.1" value={startSeconds} onChange={(event) => setStartSeconds(event.target.value)} /></Field><Field label="End (seconds)"><Input type="number" min="0" step="0.1" value={endSeconds} onChange={(event) => setEndSeconds(event.target.value)} /></Field></> : null}
          {operation === "extractAudio" ? <Field label="Audio format"><Select value={audioCodec} onChange={(event) => setAudioCodec(event.target.value as typeof audioCodec)}><option value="mp3">MP3</option><option value="aac">AAC</option><option value="opus">Opus</option></Select></Field> : null}
          {operation === "transcode" ? <><Field label="Container"><Select value={container} onChange={(event) => setContainer(event.target.value)}><option value="mp4">MP4</option><option value="mkv">MKV</option><option value="webm">WebM</option></Select></Field><Field label="Video codec"><Select value={videoCodec} onChange={(event) => setVideoCodec(event.target.value as typeof videoCodec)}><option value="copy">Copy stream</option><option value="h264">H.264</option><option value="hevc">HEVC</option><option value="vp9">VP9</option><option value="av1">AV1</option></Select></Field><Field label="Audio codec"><Select value={transcodeAudioCodec} onChange={(event) => setTranscodeAudioCodec(event.target.value as typeof transcodeAudioCodec)}><option value="copy">Copy stream</option><option value="aac">AAC</option><option value="opus">Opus</option></Select></Field></> : null}
          {operation === "privacyExport" ? <div className="grid gap-4 lg:col-span-2 lg:grid-cols-2"><div className="space-y-4"><label className="flex items-start gap-3 rounded-[9px] bg-elevated/45 p-3 text-xs text-muted"><input className="mt-0.5" type="checkbox" checked={stripMetadata} onChange={(event) => setStripMetadata(event.target.checked)} /><span><strong className="block text-ink">Remove metadata and chapters</strong>Strips container metadata from the separate copy. The original remains untouched.</span></label><Field label="Blur regions" hint="One per line: x%, y%, width%, height%, start seconds, end seconds. Example: 10, 15, 25, 20, 0, 30"><textarea className="min-h-28 w-full rounded-control border border-line/80 bg-elevated/70 px-3 py-2 text-xs text-ink outline-none focus:border-accent" value={blurRegionsText} onChange={(event) => setBlurRegionsText(event.target.value)} placeholder="10, 15, 25, 20, 0, 30" /></Field><Field label="Mute audio ranges" hint="One per line: start seconds, end seconds."><textarea className="min-h-20 w-full rounded-control border border-line/80 bg-elevated/70 px-3 py-2 text-xs text-ink outline-none focus:border-accent" value={muteRangesText} onChange={(event) => setMuteRangesText(event.target.value)} placeholder="12.5, 18" /></Field></div><PrivacyLayoutPreview regions={parseBlurRegions(blurRegionsText) || []} muteRanges={parseMuteRanges(muteRangesText) || []} /></div> : null}
          {operation === "privacyExport" ? <div className="rounded-[10px] border border-line/70 bg-elevated/30 p-4 lg:col-span-2"><div className="flex flex-wrap items-end gap-3"><Field label="Optional local detector model" hint="Uses Ollama on 127.0.0.1. Sampled frames are sent only to your local model and are not retained."><Input value={detectorModel} onChange={(event) => setDetectorModel(event.target.value)} placeholder="qwen2.5vl:7b" /></Field><Button variant="secondary" disabled={detecting || !inputPath || !detectorModel.trim()} onClick={() => void detectPrivacyRegions()}>{detecting ? <LoaderCircle className="animate-spin" size={15} /> : <WandSparkles size={15} />} {detecting ? "Scanning locally…" : "Suggest face, plate, and screen blurs"}</Button></div>{redactionSuggestions.length ? <div className="mt-4"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Review suggestions — none are applied automatically</p><div className="mt-2 grid gap-2 md:grid-cols-2">{redactionSuggestions.map((suggestion, index) => <article key={`${suggestion.frameTimeSeconds}-${index}`} className="flex items-center gap-3 rounded-[8px] border border-line/60 bg-panel/60 p-3"><div className="min-w-0 flex-1"><p className="text-xs font-medium text-ink">{suggestion.kind === "licensePlate" ? "License plate" : suggestion.kind === "screen" ? "Screen" : "Face"} · {Math.round(suggestion.confidence * 100)}%</p><p className="mt-1 text-[10px] text-muted">Near {suggestion.frameTimeSeconds.toFixed(1)}s · blur {suggestion.region.startSeconds.toFixed(1)}–{suggestion.region.endSeconds.toFixed(1)}s</p></div><Button className="h-8 px-2.5 text-xs" onClick={() => acceptPrivacySuggestion(suggestion, index)}>Add blur</Button></article>)}</div></div> : null}</div> : null}
          <Field label={operation === "splitChapters" ? "Output folder" : "Output file"}>
            <div className="flex gap-2"><Input value={outputPath} onChange={(event) => setOutputPath(event.target.value)} placeholder="Choose where to save the result" /><Button variant="secondary" className="px-3" onClick={() => void chooseOutput()}><FolderOpen size={17} /></Button></div>
          </Field>
        </div>
        <div className="mt-4 flex items-center justify-between border-t border-line/60 pt-4">
          <label className="flex items-center gap-2 text-xs text-muted"><input type="checkbox" checked={overwrite} onChange={(event) => setOverwrite(event.target.checked)} /> Replace an existing output</label>
          <Button disabled={!request || starting || (operation === "trim" && Number(endSeconds) <= Number(startSeconds))} onClick={() => void submit()}>{starting ? <LoaderCircle className="animate-spin" size={17} /> : <WandSparkles size={17} />} Start transformation</Button>
        </div>
      </section>

      <section className="mt-6">
        <h2 className="mb-3 text-sm font-semibold">Recent transformations</h2>
        <div className="space-y-2">
          {tasks.map((task) => <TransformationRow key={task.id} task={task} onCancel={() => void api.cancelTransformation(task.id).catch((error) => setError(normalizeError(error)))} />)}
          {tasks.length === 0 ? <div className="border border-dashed border-line p-10 text-center text-sm text-muted">No media transformations yet.</div> : null}
        </div>
      </section>
    </div>
  );
}

function TransformationRow({ task, onCancel }: { task: TransformationTask; onCancel: () => void }) {
  const Icon = task.operation.type === "trim" ? Scissors : task.operation.type === "extractAudio" ? FileAudio : Film;
  return <article className="panel-shadow flex items-center gap-4 border border-line/60 bg-panel/70 p-4">
    <div className="grid size-10 shrink-0 place-items-center rounded-[8px] bg-elevated text-accent"><Icon size={18} /></div>
    <div className="min-w-0 flex-1"><div className="flex justify-between gap-3 text-sm"><span className="truncate font-medium">{fileName(task.inputPath)}</span><span className="text-xs font-semibold text-muted">{task.state}</span></div><div className="mt-2 h-1.5 overflow-hidden bg-elevated"><div className="h-full bg-accent transition-all" style={{ width: `${task.progressPercent}%` }} /></div><div className={`mt-2 truncate text-xs ${task.error ? "text-red-400" : "text-muted"}`}>{task.error || task.currentOperation} · {Math.round(task.progressPercent)}%</div></div>
    {task.state === "Running" || task.state === "Queued" ? <Button variant="secondary" className="px-3" onClick={onCancel}><Square size={14} /> Cancel</Button> : null}
  </article>;
}

function fileName(path: string) { return path.split(/[\\/]/).pop() || path; }

function mergeTransformationTasks(
  baseline: TransformationTask[],
  newer: TransformationTask[],
): TransformationTask[] {
  const tasks = new Map(baseline.map((task) => [task.id, task]));
  for (const task of newer) {
    const existing = tasks.get(task.id);
    if (!existing || Date.parse(task.updatedAt) >= Date.parse(existing.updatedAt)) {
      tasks.set(task.id, task);
    }
  }
  return [...tasks.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
}

function suggestedOutput(input: string, operation: OperationKind, audioCodec: string, container: string) {
  if (!input) return undefined;
  const base = input.replace(/\.[^./\\]+$/, "");
  if (operation === "extractAudio") return `${base}.${audioCodec === "aac" ? "m4a" : audioCodec}`;
  if (operation === "privacyExport") return `${base}-privacy-safe.mp4`;
  return `${base}-${operation === "trim" ? "trimmed" : "converted"}.${container}`;
}

function parseBlurRegions(value: string): PrivacyBlurRegion[] | null {
  const lines = value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  if (lines.length > 32) return null;
  const regions: PrivacyBlurRegion[] = [];
  for (const line of lines) {
    const numbers = line.split(",").map((part) => Number(part.trim()));
    if (numbers.length !== 6 || numbers.some((number) => !Number.isFinite(number))) return null;
    const [x, y, width, height, startSeconds, endSeconds] = numbers;
    if (x < 0 || y < 0 || width <= 0 || height <= 0 || x + width > 100 || y + height > 100 || startSeconds < 0 || endSeconds <= startSeconds) return null;
    regions.push({ x: x / 100, y: y / 100, width: width / 100, height: height / 100, startSeconds, endSeconds });
  }
  return regions;
}

function parseMuteRanges(value: string): PrivacyMuteRange[] | null {
  const lines = value.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
  if (lines.length > 64) return null;
  const ranges: PrivacyMuteRange[] = [];
  for (const line of lines) {
    const numbers = line.split(",").map((part) => Number(part.trim()));
    if (numbers.length !== 2 || numbers.some((number) => !Number.isFinite(number))) return null;
    const [startSeconds, endSeconds] = numbers;
    if (startSeconds < 0 || endSeconds <= startSeconds) return null;
    ranges.push({ startSeconds, endSeconds });
  }
  return ranges;
}

function PrivacyLayoutPreview({ regions, muteRanges }: { regions: PrivacyBlurRegion[]; muteRanges: PrivacyMuteRange[] }) {
  return <div><p className="text-xs font-medium text-ink">Redaction layout preview</p><p className="mt-1 text-[11px] leading-4 text-muted">Every normalized blur region is shown below. Review the times before starting the export.</p><div className="relative mt-3 aspect-video overflow-hidden rounded-media border border-line/70 bg-[linear-gradient(145deg,rgba(255,255,255,.06),rgba(255,255,255,.015))]"><div className="absolute inset-0 grid place-items-center text-[10px] uppercase tracking-[0.12em] text-muted">Video frame area</div>{regions.map((region, index) => <div key={`${index}-${region.startSeconds}`} className="absolute grid place-items-center overflow-hidden border border-accent/80 bg-accent/25 text-[9px] font-semibold text-accent shadow-[0_0_0_1px_rgba(0,0,0,.35)]" style={{ left: `${region.x * 100}%`, top: `${region.y * 100}%`, width: `${region.width * 100}%`, height: `${region.height * 100}%` }} title={`Blur ${region.startSeconds}s–${region.endSeconds}s`}>{index + 1}</div>)}</div><div className="mt-3 space-y-1 text-[10px] text-muted">{regions.map((region, index) => <p key={index}>Blur {index + 1}: {region.startSeconds}s–{region.endSeconds}s · {Math.round(region.width * 100)}% × {Math.round(region.height * 100)}%</p>)}{muteRanges.map((range, index) => <p key={`mute-${index}`}>Mute {index + 1}: {range.startSeconds}s–{range.endSeconds}s</p>)}{regions.length === 0 && muteRanges.length === 0 ? <p>No visual or audio redactions entered. Metadata removal can still create a sanitized copy.</p> : null}</div></div>;
}
