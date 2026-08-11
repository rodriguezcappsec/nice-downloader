import { ArrowDown, ArrowUp, Ban, Download, ListOrdered, Pause, Play, RotateCcw } from "lucide-react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Progress } from "../../components/ui";
import { formatBytes, formatEta } from "../../lib/format";
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
        <div className="mb-4 flex flex-wrap items-center gap-4 border border-line bg-panel p-4 text-sm">
          <label className="flex items-center gap-2 text-muted">
            Concurrent downloads
            <select
              className="h-9 border border-line bg-canvas px-2 text-ink"
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
          <span className="text-xs text-muted">{queue.activeJobIds.length} active · {queue.orderedJobIds.length} waiting</span>
        </div>
      ) : null}
      {visible.length === 0 ? (
        <EmptyState
          icon={queueOnly ? <ListOrdered size={20} /> : <Download size={20} />}
          title={queueOnly ? "The queue is empty" : "No downloads yet"}
          description={queueOnly ? "Queued jobs appear here while the configured workers are busy." : "Analyze a URL and start a download to see live progress here."}
        />
      ) : (
        <div className="divide-y divide-line border border-line bg-panel">
          {visible.map((job, index) => <DownloadRow
            key={job.id}
            job={job}
            onCancel={() => void cancel(job.id)}
            onRetry={() => void retry(job.id)}
            onPause={() => void pause(job.id)}
            onResume={() => void resume(job.id)}
            queueControls={queueOnly ? {
              canMoveUp: index > 0,
              canMoveDown: index < visible.length - 1,
              moveUp: () => void moveQueued(job.id, index - 1),
              moveDown: () => void moveQueued(job.id, index + 1),
            } : undefined}
          />)}
        </div>
      )}
    </Page>
  );
}

function DownloadRow({ job, onCancel, onRetry, onPause, onResume, queueControls }: {
  job: DownloadJob;
  onCancel: () => void;
  onRetry: () => void;
  onPause: () => void;
  onResume: () => void;
  queueControls?: { canMoveUp: boolean; canMoveDown: boolean; moveUp: () => void; moveDown: () => void };
}) {
  const canCancel = !terminalStates.has(job.state);
  const stateTone = job.state === "Failed" ? "text-red-400" : job.state === "Completed" ? "text-emerald-400" : job.state === "Cancelled" ? "text-muted" : "text-blue-400";
  return (
    <article className="flex gap-4 p-4">
      <div className="h-[74px] w-[124px] shrink-0 overflow-hidden rounded-sm bg-elevated">
        {job.thumbnail ? <img className="h-full w-full object-cover" src={job.thumbnail} alt="" /> : <div className="grid h-full place-items-center text-muted"><Download size={22} /></div>}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0">
            <h3 className="truncate text-sm font-semibold">{job.title}</h3>
            <div className="mt-1 flex items-center gap-2 text-xs text-muted">
              <span className={stateTone}>{job.state}</span><span>•</span><span>{job.selectedQuality}</span><span>•</span><span>{job.outputFormat.toUpperCase()}</span>
            </div>
          </div>
          {canCancel ? (
            <div className="flex gap-1">
              {queueControls ? <>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveUp} onClick={queueControls.moveUp} aria-label="Move up"><ArrowUp size={14} /></Button>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveDown} onClick={queueControls.moveDown} aria-label="Move down"><ArrowDown size={14} /></Button>
              </> : null}
              <Button variant="secondary" className="h-8 px-2.5" onClick={job.state === "Paused" ? onResume : onPause}>
                {job.state === "Paused" ? <Play size={14} /> : <Pause size={14} />}
                {job.state === "Paused" ? "Resume" : "Pause"}
              </Button>
              <Button variant="danger" className="h-8 px-2.5" onClick={onCancel}><Ban size={14} /> Cancel</Button>
            </div>
          ) : job.state === "Failed" || job.state === "Cancelled" ? (
            <Button variant="ghost" className="h-8 px-2.5" onClick={onRetry}><RotateCcw size={14} /> Retry</Button>
          ) : null}
        </div>
        <div className="mt-3"><Progress value={job.progressPercent} /></div>
        <div className="mt-2 grid grid-cols-[1fr_auto_auto] gap-5 text-xs text-muted">
          <span className="truncate">{job.error || job.currentOperation}</span>
          <span>{job.downloadedBytes != null ? `${formatBytes(job.downloadedBytes)} / ${formatBytes(job.totalBytes)}` : ""}</span>
          <span>{job.speedBytesPerSecond != null ? `${formatBytes(job.speedBytesPerSecond)}/s · ETA ${formatEta(job.etaSeconds)}` : `${job.progressPercent.toFixed(0)}%`}</span>
        </div>
        {job.diagnostics.length > 0 ? (
          <details className="mt-3 text-xs text-muted">
            <summary className="cursor-pointer select-none font-medium text-ink">Diagnostics ({job.diagnostics.length})</summary>
            <pre className="mt-2 max-h-44 overflow-auto whitespace-pre-wrap border border-line bg-canvas p-3 font-mono leading-5">{job.diagnostics.join("\n")}</pre>
          </details>
        ) : null}
        {job.attemptCount > 1 ? <p className="mt-2 text-[11px] text-muted">Attempt {job.attemptCount}</p> : null}
      </div>
    </article>
  );
}
