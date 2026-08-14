import { open, save } from "@tauri-apps/plugin-dialog";
import { FileAudio, Film, FolderOpen, LoaderCircle, Scissors, Square, WandSparkles } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select } from "../../components/ui";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { StartTransformationRequest, TransformationOperation, TransformationTask } from "../../types";

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
  const [overwrite, setOverwrite] = useState(false);
  const [starting, setStarting] = useState(false);

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
    else selectedOperation = { type: "splitChapters" };
    return { inputPath, outputPath, operation: selectedOperation, overwrite };
  }, [audioCodec, container, endSeconds, inputPath, operation, outputPath, overwrite, startSeconds, transcodeAudioCodec, videoCodec]);

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

  return (
    <div className="mx-auto w-full max-w-[1340px] px-8 pb-12 pt-8 xl:px-10">
      <header className="mb-7">
        <div className="mb-2 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent/90"><span className="h-px w-5 bg-accent/70" /> Local tools</div>
        <h1 className="text-[28px] font-semibold tracking-[-0.035em]">Transform media</h1>
        <p className="mt-2 text-[13px] text-muted">Trim, extract, convert, or split local media with FFmpeg. Tasks continue in Rust when this page is hidden.</p>
      </header>

      <section className="panel-shadow border border-line/60 bg-panel/75 p-5">
        <div className="grid gap-4 lg:grid-cols-2">
          <Field label="Input media">
            <div className="flex gap-2"><Input value={inputPath} onChange={(event) => setInputPath(event.target.value)} placeholder="Choose a local media file" /><Button variant="secondary" className="px-3" onClick={() => void chooseInput()}><FolderOpen size={17} /></Button></div>
          </Field>
          <Field label="Operation">
            <Select value={operation} onChange={(event) => { setOperation(event.target.value as OperationKind); setOutputPath(""); }}>
              <option value="trim">Trim without re-encoding</option><option value="extractAudio">Extract audio</option><option value="transcode">Convert format or codecs</option><option value="splitChapters">Split chapters</option>
            </Select>
          </Field>
          {operation === "trim" ? <><Field label="Start (seconds)"><Input type="number" min="0" step="0.1" value={startSeconds} onChange={(event) => setStartSeconds(event.target.value)} /></Field><Field label="End (seconds)"><Input type="number" min="0" step="0.1" value={endSeconds} onChange={(event) => setEndSeconds(event.target.value)} /></Field></> : null}
          {operation === "extractAudio" ? <Field label="Audio format"><Select value={audioCodec} onChange={(event) => setAudioCodec(event.target.value as typeof audioCodec)}><option value="mp3">MP3</option><option value="aac">AAC</option><option value="opus">Opus</option></Select></Field> : null}
          {operation === "transcode" ? <><Field label="Container"><Select value={container} onChange={(event) => setContainer(event.target.value)}><option value="mp4">MP4</option><option value="mkv">MKV</option><option value="webm">WebM</option></Select></Field><Field label="Video codec"><Select value={videoCodec} onChange={(event) => setVideoCodec(event.target.value as typeof videoCodec)}><option value="copy">Copy stream</option><option value="h264">H.264</option><option value="hevc">HEVC</option><option value="vp9">VP9</option><option value="av1">AV1</option></Select></Field><Field label="Audio codec"><Select value={transcodeAudioCodec} onChange={(event) => setTranscodeAudioCodec(event.target.value as typeof transcodeAudioCodec)}><option value="copy">Copy stream</option><option value="aac">AAC</option><option value="opus">Opus</option></Select></Field></> : null}
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
  return `${base}-${operation === "trim" ? "trimmed" : "converted"}.${container}`;
}
