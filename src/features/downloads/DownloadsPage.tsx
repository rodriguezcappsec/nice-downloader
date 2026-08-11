import { ArrowDown, ArrowUp, Ban, Download, FolderOpen, GripVertical, ListOrdered, Pause, Play, RotateCcw } from "lucide-react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Progress } from "../../components/ui";
import { formatBytes, formatEta } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { DownloadJob } from "../../types";

const terminalStates = new Set(["Completed", "Failed", "Cancelled"]);

export function DownloadsPage({ queueOnly = false }: { queueOnly?: boolean }) {
  const jobMap = useAppStore((state) => state.jobs);
  const jobs = Object.values(jobMap);
  const cancel = useAppStore((state) => state.cancelDownload);
  const retry = useAppStore((state) => state.retryDownload);
  const pause = useAppStore((state) => state.pauseDownload);
  const resume = useAppStore((state) => state.resumeDownload);
  const queue = useAppStore((state) => state.queue);
  const updateQueue = useAppStore((state) => state.updateQueueSettings);
  const moveQueued = useAppStore((state) => state.moveQueuedDownload);
  const setView = useAppStore((state) => state.setView);
  const setError = useAppStore((state) => state.setError);
  const visible = jobs
    .filter((job) => (queueOnly ? ["Queued", "Paused"].includes(job.state) && queue.orderedJobIds.includes(job.id) : true))
    .sort((a, b) => queueOnly
      ? queue.orderedJobIds.indexOf(a.id) - queue.orderedJobIds.indexOf(b.id)
      : b.createdAt.localeCompare(a.createdAt));

  return (
    <Page
      title={queueOnly ? "Queue" : "Downloads"}
      description={queueOnly ? "Downloads waiting for an available worker." : "Live status from the Rust download queue."}
      actions={
        <Button onClick={() => setView("new")}><Download size={16} /> New download</Button>
      }
    >
      {queueOnly ? (
        <div className="panel-shadow mb-5 flex flex-wrap items-center gap-x-5 gap-y-3 rounded-[11px] border border-line/60 bg-panel/75 p-4 text-sm">
          <label className="flex items-center gap-2 text-muted">
            Concurrent downloads
            <select
              className="h-9 rounded-[7px] border border-line/80 bg-canvas/70 px-2 text-ink outline-none focus:border-accent"
              value={queue.settings.concurrentDownloads}
              onChange={(event) => void updateQueue({ ...queue.settings, concurrentDownloads: Number(event.target.value) })}
            >
              {[1, 2, 3, 4, 6, 8].map((count) => <option key={count} value={count}>{count}</option>)}
            </select>
          </label>
          <Button
            variant="secondary"
            className="h-9"
            onClick={() => void updateQueue({ ...queue.settings, paused: !queue.settings.paused })}
          >
            {queue.settings.paused ? <Play size={15} /> : <Pause size={15} />}
            {queue.settings.paused ? "Start queue" : "Pause queue"}
          </Button>
          <label className="flex items-center gap-2 text-xs text-muted">
            <input
              type="checkbox"
              checked={queue.settings.notifyOnCompletion}
              onChange={(event) => void updateQueue({ ...queue.settings, notifyOnCompletion: event.target.checked })}
            />
            Desktop notification on completion
          </label>
          <label className="flex items-center gap-2 text-xs text-muted">
            After each download
            <select className="h-8 rounded-[7px] border border-line/80 bg-canvas/70 px-2 text-ink outline-none focus:border-accent" value={queue.settings.completionAction} onChange={(event) => void updateQueue({ ...queue.settings, completionAction: event.target.value as "none" | "openFolder" })}>
              <option value="none">Do nothing</option><option value="openFolder">Open folder</option>
            </select>
          </label>
          <span className="number-font ml-auto rounded-[6px] bg-canvas/60 px-2.5 py-1.5 text-[10px] text-muted">{queue.activeJobIds.length} active · {queue.orderedJobIds.length} waiting</span>
        </div>
      ) : null}
      {visible.length === 0 ? (
        <EmptyState
          icon={queueOnly ? <ListOrdered size={20} /> : <Download size={20} />}
          title={queueOnly ? "The queue is empty" : "No downloads yet"}
          description={queueOnly ? "Queued jobs appear here while the configured workers are busy." : "Analyze a URL and start a download to see live progress here."}
        />
      ) : (
        <div className="panel-shadow overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
          <div className="grid grid-cols-[minmax(180px,1fr)_minmax(120px,.65fr)_96px_142px] gap-3 border-b border-line/60 bg-elevated/45 px-4 py-2.5 text-[10px] font-semibold tracking-[0.04em] text-muted">
            <span>Download</span><span>Status</span><span className="text-right">Transfer</span><span className="text-right">Actions</span>
          </div>
          <div className="divide-y divide-line/65">
          {visible.map((job, index) => <DownloadRow
            key={job.id}
            job={job}
            onCancel={() => void cancel(job.id)}
            onRetry={() => void retry(job.id)}
            onPause={() => void pause(job.id)}
            onResume={() => void resume(job.id)}
            onReveal={() => void api.openDownloadOutput(job.id, true).catch((error) => setError(normalizeError(error)))}
            queueControls={queueOnly ? {
              canMoveUp: index > 0,
              canMoveDown: index < visible.length - 1,
              moveUp: () => void moveQueued(job.id, index - 1),
              moveDown: () => void moveQueued(job.id, index + 1),
              moveTo: (jobId, targetIndex) => void moveQueued(jobId, targetIndex),
              targetIndex: index,
            } : undefined}
          />)}
          </div>
        </div>
      )}
    </Page>
  );
}

function DownloadRow({ job, onCancel, onRetry, onPause, onResume, onReveal, queueControls }: {
  job: DownloadJob;
  onCancel: () => void;
  onRetry: () => void;
  onPause: () => void;
  onResume: () => void;
  onReveal: () => void;
  queueControls?: { canMoveUp: boolean; canMoveDown: boolean; moveUp: () => void; moveDown: () => void; moveTo: (jobId: string, targetIndex: number) => void; targetIndex: number };
}) {
  const canCancel = !terminalStates.has(job.state);
  const progressTone = job.state === "Failed" ? "red" : job.state === "Completed" ? "green" : job.currentOperation.toLocaleLowerCase().includes("retry") ? "amber" : "blue";
  return (
    <article
      className="grid grid-cols-[minmax(180px,1fr)_minmax(120px,.65fr)_96px_142px] items-center gap-3 px-4 py-3.5 transition-colors duration-200 hover:bg-elevated/30"
      draggable={Boolean(queueControls)}
      onDragStart={(event) => event.dataTransfer.setData("text/plain", job.id)}
      onDragOver={(event) => { if (queueControls) event.preventDefault(); }}
      onDrop={(event) => { event.preventDefault(); queueControls?.moveTo(event.dataTransfer.getData("text/plain"), queueControls.targetIndex); }}
    >
      <div className="flex min-w-0 items-center gap-3">
      {queueControls ? <GripVertical size={15} className="shrink-0 text-muted" aria-label="Drag to reorder" /> : null}
      <div className="hidden h-14 w-24 shrink-0 overflow-hidden rounded-[7px] border border-line/50 bg-elevated xl:block">
        {job.thumbnail ? <img className="h-full w-full object-cover" src={job.thumbnail} alt={`${job.title} thumbnail`} /> : <div className="grid h-full place-items-center text-muted"><Download size={22} /></div>}
      </div>
      <div className="min-w-0 flex-1">
            <h3 className="truncate text-[13px] font-semibold tracking-[-0.01em]">{job.title}</h3>
            <div className="mt-1 flex items-center gap-2 text-xs text-muted">
              <span>{job.selectedQuality}</span><span>•</span><span>{job.outputFormat.toUpperCase()}</span>
            </div>
        <div className="mt-2"><Progress value={job.progressPercent} tone={progressTone} /></div>
      </div></div>
      <div className="min-w-0">
        <StatusBadge job={job} />
        <p className={`mt-1 truncate text-xs ${job.error ? "text-red-400" : "text-muted"}`}>{job.error || job.currentOperation}</p>
        {job.attemptCount > 1 ? <p className="mt-1 text-[10px] text-amber-400">Attempt {job.attemptCount} of 3</p> : null}
      </div>
      <div className="number-font text-right text-[11px] text-muted">
        <div>{job.speedBytesPerSecond != null ? `${formatBytes(job.speedBytesPerSecond)}/s` : `${job.progressPercent.toFixed(0)}%`}</div>
        <div className="mt-1">{job.speedBytesPerSecond != null ? `ETA ${formatEta(job.etaSeconds)}` : job.downloadedBytes != null ? formatBytes(job.downloadedBytes) : "—"}</div>
      </div>
      <div className="flex justify-end gap-1">
          {canCancel ? (
            <>
              {queueControls ? <>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveUp} onClick={queueControls.moveUp} aria-label="Move up"><ArrowUp size={14} /></Button>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveDown} onClick={queueControls.moveDown} aria-label="Move down"><ArrowDown size={14} /></Button>
              </> : null}
              <Button variant="ghost" className="h-8 px-2" onClick={job.state === "Paused" ? onResume : onPause} aria-label={job.state === "Paused" ? "Resume" : "Pause"}>
                {job.state === "Paused" ? <Play size={14} /> : <Pause size={14} />}
              </Button>
              <Button variant="danger" className="h-8 px-2" onClick={onCancel} aria-label="Stop and keep partial file"><Ban size={14} /></Button>
            </>
          ) : job.state === "Failed" || job.state === "Cancelled" ? (
            <Button variant="ghost" className="h-8 px-2.5" onClick={onRetry}><RotateCcw size={14} /> Retry</Button>
          ) : job.state === "Completed" ? (
            <Button variant="ghost" className="h-8 px-2.5" onClick={onReveal}><FolderOpen size={14} /> Reveal</Button>
          ) : null}
      </div>
      <div className="col-span-4">
        {job.diagnostics.length > 0 ? (
          <details className="text-xs text-muted">
            <summary className="cursor-pointer select-none font-medium text-muted hover:text-ink">Engine log ({job.diagnostics.length})</summary>
            <pre className="number-font mt-2 max-h-44 overflow-auto whitespace-pre-wrap rounded-[7px] border border-line/70 bg-canvas/70 p-3 leading-5">{job.diagnostics.join("\n")}</pre>
          </details>
        ) : null}
      </div>
    </article>
  );
}

function StatusBadge({ job }: { job: DownloadJob }) {
  const retrying = job.currentOperation.toLocaleLowerCase().includes("retry");
  const label = retrying ? "Retrying" : job.state;
  const tone = job.state === "Failed" ? "bg-red-500/10 text-red-400" : job.state === "Completed" ? "bg-emerald-500/10 text-emerald-400" : retrying ? "bg-amber-500/10 text-amber-400" : job.state === "Verifying" ? "bg-violet-500/10 text-violet-400" : job.state === "Paused" || job.state === "Cancelled" ? "bg-line/50 text-muted" : "bg-accent/10 text-accent";
  return <span className={`inline-flex rounded-[5px] px-2 py-1 text-[10px] font-semibold tracking-[0.02em] ${tone}`}>{label}</span>;
}
