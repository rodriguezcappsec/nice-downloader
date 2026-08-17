import { ArrowDown, ArrowUp, Ban, Download, FolderOpen, GripVertical, ListOrdered, Pause, Play, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Progress } from "../../components/ui";
import { formatBytes, formatEta } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type { DownloadJob } from "../../types";

const terminalStates = new Set(["Completed", "Failed", "Cancelled"]);

export type DownloadWorkspaceTab = "active" | "queue" | "scheduled" | "failed";

export function DownloadsPage({ queueOnly = false, embedded = false, tab = queueOnly ? "queue" : "active" }: { queueOnly?: boolean; embedded?: boolean; tab?: DownloadWorkspaceTab }) {
  const jobMap = useAppStore((state) => state.jobs);
  const jobs = Object.values(jobMap);
  const cancel = useAppStore((state) => state.cancelDownload);
  const dismissDownloads = useAppStore((state) => state.dismissDownloads);
  const retry = useAppStore((state) => state.retryDownload);
  const retryFailedGalleryFiles = useAppStore((state) => state.retryFailedGalleryFiles);
  const pause = useAppStore((state) => state.pauseDownload);
  const resume = useAppStore((state) => state.resumeDownload);
  const queue = useAppStore((state) => state.queue);
  const updateQueue = useAppStore((state) => state.updateQueueSettings);
  const moveQueued = useAppStore((state) => state.moveQueuedDownload);
  const setView = useAppStore((state) => state.setView);
  const setError = useAppStore((state) => state.setError);
  const [selectedJobIds, setSelectedJobIds] = useState<string[]>([]);
  const isQueue = tab === "queue";
  const visible = jobs
    .filter((job) => {
      const scheduled = Boolean(job.scheduledFor && Date.parse(job.scheduledFor) > Date.now());
      if (tab === "active") return ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state);
      if (tab === "scheduled") return ["Queued", "Paused"].includes(job.state) && scheduled;
      if (tab === "failed") return job.state === "Failed" || job.state === "Cancelled";
      return ["Queued", "Paused"].includes(job.state) && queue.orderedJobIds.includes(job.id) && !scheduled;
    })
    .sort((a, b) => isQueue || tab === "scheduled"
      ? queue.orderedJobIds.indexOf(a.id) - queue.orderedJobIds.indexOf(b.id)
      : b.createdAt.localeCompare(a.createdAt));
  const removable = visible.filter((job) => terminalStates.has(job.state));
  const selectedRemovable = selectedJobIds.filter((id) => removable.some((job) => job.id === id));
  const activeCount = jobs.filter((job) => ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state)).length;
  const waitingCount = jobs.filter((job) => job.state === "Queued").length;
  const completedCount = jobs.filter((job) => job.state === "Completed").length;
  const attentionCount = jobs.filter((job) => job.state === "Failed" || job.state === "Cancelled").length;

  async function removeRecords(ids: string[], confirmBulk = false) {
    const removableIds = ids.filter((id) => removable.some((job) => job.id === id));
    if (removableIds.length === 0) return;
    if (confirmBulk && !window.confirm(`Remove ${removableIds.length} download record${removableIds.length === 1 ? "" : "s"} from this list? Completed downloads will remain in History and files will not be deleted.`)) return;
    const dismissed = await dismissDownloads(removableIds);
    setSelectedJobIds((selected) => selected.filter((id) => !dismissed.includes(id)));
  }

  return (
    <Page
      title={isQueue ? "Queue" : tab === "scheduled" ? "Scheduled" : tab === "failed" ? "Failed" : "Downloads"}
      description={isQueue ? "Shape what downloads next and how many transfers run together." : tab === "scheduled" ? "Downloads waiting for their locally scheduled start time." : tab === "failed" ? "Review interrupted work and choose what should be retried." : "Follow every transfer from first byte to finished file."}
      actions={
        <Button onClick={() => setView("new")}><Download size={16} /> New download</Button>
      }
      embedded={embedded}
    >
      <DownloadOverview
        queueOnly={isQueue}
        activeCount={activeCount}
        waitingCount={waitingCount}
        completedCount={completedCount}
        attentionCount={attentionCount}
        paused={queue.settings.paused}
        workerLimit={queue.settings.concurrentDownloads}
      />
      {isQueue ? (
        <div className="panel-shadow mb-6 flex flex-wrap items-center gap-x-5 gap-y-3 rounded-panel border border-line/45 bg-panel/70 p-5 text-sm">
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
              disabled={!queue.completionNotificationsSupported}
              onChange={(event) => void updateQueue({ ...queue.settings, notifyOnCompletion: event.target.checked })}
            />
            {queue.completionNotificationsSupported ? "Desktop notification on completion" : "Desktop notifications unavailable on this platform"}
          </label>
          <label className="flex items-center gap-2 text-xs text-muted">
            After each download
            <select className="h-8 rounded-[7px] border border-line/80 bg-canvas/70 px-2 text-ink outline-none focus:border-accent" value={queue.settings.completionAction} onChange={(event) => void updateQueue({ ...queue.settings, completionAction: event.target.value as "none" | "openFolder" })}>
              <option value="none">Do nothing</option><option value="openFolder">Open folder</option>
            </select>
          </label>
          <span className="number-font ml-auto rounded-control bg-canvas/60 px-3 py-2 text-[10px] text-muted">{queue.activeJobIds.length} active · {queue.orderedJobIds.length} waiting</span>
        </div>
      ) : null}
      {!isQueue && removable.length > 0 ? (
        <div className="mb-5 flex flex-wrap items-center gap-2 rounded-panel border border-line/45 bg-panel/55 p-3.5 shadow-control">
          <span className="mr-2 text-xs text-muted">{selectedRemovable.length > 0 ? `${selectedRemovable.length} selected` : `${removable.length} finished records`}</span>
          <Button variant="secondary" className="h-8 px-2.5 text-xs" onClick={() => setSelectedJobIds(removable.map((job) => job.id))}>Select finished</Button>
          {selectedRemovable.length > 0 ? <Button variant="secondary" className="h-8 px-2.5 text-xs" onClick={() => setSelectedJobIds([])}>Clear selection</Button> : null}
          <Button variant="danger" className="h-8 px-2.5 text-xs" disabled={selectedRemovable.length === 0} onClick={() => void removeRecords(selectedRemovable, true)}><Trash2 size={14} /> Remove selected</Button>
          <div className="ml-auto flex gap-2">
            <Button variant="ghost" className="h-8 px-2.5 text-xs" disabled={!removable.some((job) => job.state === "Completed")} onClick={() => void removeRecords(removable.filter((job) => job.state === "Completed").map((job) => job.id), true)}>Clear completed</Button>
            <Button variant="ghost" className="h-8 px-2.5 text-xs" disabled={!removable.some((job) => job.state === "Failed" || job.state === "Cancelled")} onClick={() => void removeRecords(removable.filter((job) => job.state === "Failed" || job.state === "Cancelled").map((job) => job.id), true)}>Clear failed/cancelled</Button>
          </div>
        </div>
      ) : null}
      {visible.length === 0 ? (
        <EmptyState
          icon={isQueue ? <ListOrdered size={20} /> : <Download size={20} />}
          title={isQueue ? "The queue is empty" : tab === "scheduled" ? "Nothing is scheduled" : tab === "failed" ? "No downloads need attention" : "No active downloads"}
          description={isQueue ? "Queued jobs appear here while the configured workers are busy." : tab === "scheduled" ? "Choose a future start time in New Download and it will appear here." : tab === "failed" ? "Failed and cancelled transfers stay here until you retry or remove them." : "New transfers appear here as soon as they begin."}
        />
      ) : (
        <div className="panel-shadow overflow-hidden rounded-media border border-line/45 bg-panel/70">
          <div className="grid grid-cols-[28px_minmax(180px,1fr)_minmax(120px,.65fr)_96px_174px] gap-3 border-b border-line/45 bg-elevated/35 px-5 py-3 text-[10px] font-semibold tracking-[0.045em] text-muted">
            <span>{!isQueue && removable.length > 0 ? <input type="checkbox" aria-label="Select all finished downloads" checked={selectedRemovable.length === removable.length} onChange={(event) => setSelectedJobIds(event.target.checked ? removable.map((job) => job.id) : [])} /> : null}</span><span>Download</span><span>Status</span><span className="text-right">Transfer</span><span className="text-right">Actions</span>
          </div>
          <div className="divide-y divide-line/65">
          {visible.map((job, index) => <DownloadRow
            key={job.id}
            job={job}
            onCancel={() => void cancel(job.id)}
            onRetry={() => void retry(job.id)}
            onRetryFailedGalleryFiles={() => void retryFailedGalleryFiles(job.id)}
            onPause={() => void pause(job.id)}
            onResume={() => void resume(job.id)}
            pauseSupported={queue.pauseSupported || !queue.activeJobIds.includes(job.id)}
            onReveal={() => void api.openDownloadOutput(job.id, true).catch((error) => setError(normalizeError(error)))}
            selected={selectedJobIds.includes(job.id)}
            onSelect={(selected) => setSelectedJobIds((current) => selected ? [...new Set([...current, job.id])] : current.filter((id) => id !== job.id))}
            onDismiss={() => void removeRecords([job.id])}
            queueControls={isQueue || tab === "scheduled" ? {
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

function DownloadOverview({ queueOnly, activeCount, waitingCount, completedCount, attentionCount, paused, workerLimit }: {
  queueOnly: boolean;
  activeCount: number;
  waitingCount: number;
  completedCount: number;
  attentionCount: number;
  paused: boolean;
  workerLimit: number;
}) {
  const metrics = queueOnly
    ? [
      { label: "Workers", value: `${activeCount}/${workerLimit}` },
      { label: "Waiting", value: waitingCount.toLocaleString() },
      { label: "Needs attention", value: attentionCount.toLocaleString() },
    ]
    : [
      { label: "In motion", value: activeCount.toLocaleString() },
      { label: "Waiting", value: waitingCount.toLocaleString() },
      { label: "Finished", value: completedCount.toLocaleString() },
    ];
  const quiet = activeCount === 0 && waitingCount === 0;
  const heading = queueOnly
    ? paused ? "Queue paused" : waitingCount > 0 ? "Queue is moving" : "Queue is ready"
    : quiet ? "All quiet" : activeCount > 0 ? "Transfers in progress" : "Downloads are waiting";
  const detail = queueOnly
    ? paused ? "No new jobs will start until you resume the queue." : `${workerLimit} worker${workerLimit === 1 ? "" : "s"} can run at the same time.`
    : quiet ? "New transfers will appear here as soon as they start." : "Progress and recovery stay owned by the local Rust queue.";

  return (
    <section className="panel-shadow mb-6 grid overflow-hidden rounded-media border border-line/40 bg-panel/65 lg:grid-cols-[minmax(0,1.45fr)_repeat(3,minmax(110px,.45fr))]" aria-label={queueOnly ? "Queue overview" : "Download overview"}>
      <div className="ingest-card flex min-h-28 items-center gap-4 border-b border-line/40 px-6 py-5 lg:border-b-0 lg:border-r">
        <span className={`grid size-11 shrink-0 place-items-center rounded-panel ${quiet || paused ? "bg-elevated text-muted" : "bg-accent text-accentInk shadow-[0_12px_32px_rgb(var(--accent)/0.18)]"}`}>
          {queueOnly ? <ListOrdered size={19} /> : <Download size={19} />}
        </span>
        <span className="min-w-0"><strong className="block text-[17px] font-semibold tracking-[-0.025em]">{heading}</strong><span className="mt-1 block max-w-md text-[11px] leading-4 text-muted">{detail}</span></span>
      </div>
      {metrics.map((metric) => (
        <div key={metric.label} className="flex min-h-24 flex-col justify-center border-t border-line/35 px-5 py-4 first:border-t-0 lg:min-h-28 lg:border-l lg:border-t-0">
          <span className="number-font text-[22px] font-semibold tracking-[-0.04em] text-ink">{metric.value}</span>
          <span className="mt-1 text-[10px] font-medium tracking-[0.035em] text-muted">{metric.label}</span>
        </div>
      ))}
    </section>
  );
}

function DownloadRow({ job, onCancel, onRetry, onRetryFailedGalleryFiles, onPause, onResume, onReveal, onDismiss, selected, onSelect, pauseSupported, queueControls }: {
  job: DownloadJob;
  onCancel: () => void;
  onRetry: () => void;
  onRetryFailedGalleryFiles: () => void;
  onPause: () => void;
  onResume: () => void;
  pauseSupported: boolean;
  onReveal: () => void;
  onDismiss: () => void;
  selected: boolean;
  onSelect: (selected: boolean) => void;
  queueControls?: { canMoveUp: boolean; canMoveDown: boolean; moveUp: () => void; moveDown: () => void; moveTo: (jobId: string, targetIndex: number) => void; targetIndex: number };
}) {
  const retryScheduled = job.currentOperation.toLocaleLowerCase().includes("automatic retry");
  const failedGalleryFiles = job.galleryFiles.filter((file) => file.state === "failed");
  const canCancel = !terminalStates.has(job.state) || retryScheduled;
  const progressTone = job.state === "Failed" ? "red" : job.state === "Completed" ? "green" : job.currentOperation.toLocaleLowerCase().includes("retry") ? "amber" : "blue";
  return (
    <article
      className="grid grid-cols-[28px_minmax(180px,1fr)_minmax(120px,.65fr)_96px_174px] items-center gap-3 px-5 py-4 transition-colors duration-200 hover:bg-elevated/35"
      draggable={Boolean(queueControls)}
      onDragStart={(event) => event.dataTransfer.setData("text/plain", job.id)}
      onDragOver={(event) => { if (queueControls) event.preventDefault(); }}
      onDrop={(event) => { event.preventDefault(); queueControls?.moveTo(event.dataTransfer.getData("text/plain"), queueControls.targetIndex); }}
    >
      <div>{terminalStates.has(job.state) && !queueControls ? <input type="checkbox" checked={selected} onChange={(event) => onSelect(event.target.checked)} aria-label={`Select ${job.title}`} /> : null}</div>
      <div className="flex min-w-0 items-center gap-3">
      {queueControls ? <GripVertical size={15} className="shrink-0 text-muted" aria-label="Drag to reorder" /> : null}
      <div className="hidden h-14 w-24 shrink-0 overflow-hidden rounded-control border border-line/40 bg-elevated xl:block">
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
          {retryScheduled ? (
            <Button variant="danger" className="h-8 px-2.5" onClick={onCancel}><Ban size={14} /> Cancel retry</Button>
          ) : canCancel ? (
            <>
              {queueControls ? <>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveUp} onClick={queueControls.moveUp} aria-label="Move up"><ArrowUp size={14} /></Button>
                <Button variant="ghost" className="h-8 px-2" disabled={!queueControls.canMoveDown} onClick={queueControls.moveDown} aria-label="Move down"><ArrowDown size={14} /></Button>
              </> : null}
              <Button variant="ghost" className="h-8 px-2" disabled={!pauseSupported} onClick={job.state === "Paused" ? onResume : onPause} aria-label={pauseSupported ? (job.state === "Paused" ? "Resume" : "Pause") : "Pausing active downloads is unavailable on this platform"}>
                {job.state === "Paused" ? <Play size={14} /> : <Pause size={14} />}
              </Button>
              <Button variant="danger" className="h-8 px-2" onClick={onCancel} aria-label="Stop and keep partial file"><Ban size={14} /></Button>
            </>
          ) : job.state === "Failed" || job.state === "Cancelled" ? (
            <><Button variant="ghost" className="h-8 px-2.5" onClick={failedGalleryFiles.length > 0 ? onRetryFailedGalleryFiles : onRetry}><RotateCcw size={14} /> {failedGalleryFiles.length > 0 ? `Retry ${failedGalleryFiles.length} failed` : "Retry"}</Button><Button variant="danger" className="h-8 px-2" onClick={onDismiss} aria-label="Remove download record"><Trash2 size={14} /></Button></>
          ) : job.state === "Completed" ? (
            <><Button variant="ghost" className="h-8 px-2.5" onClick={onReveal}><FolderOpen size={14} /> Reveal</Button><Button variant="danger" className="h-8 px-2" onClick={onDismiss} aria-label="Remove download record"><Trash2 size={14} /></Button></>
          ) : null}
      </div>
      <div className="col-span-5">
        {job.galleryFiles.length > 0 ? <GalleryFileProgressList job={job} /> : null}
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

function GalleryFileProgressList({ job }: { job: DownloadJob }) {
  const completed = job.galleryFiles.filter((file) => file.state === "completed").length;
  const skipped = job.galleryFiles.filter((file) => file.state === "skipped").length;
  const failed = job.galleryFiles.filter((file) => file.state === "failed").length;
  return (
    <details className="mb-2 text-xs text-muted">
      <summary className="cursor-pointer select-none font-medium text-muted hover:text-ink">
        File progress ({completed} completed{skipped > 0 ? `, ${skipped} skipped` : ""}{failed > 0 ? `, ${failed} failed` : ""})
      </summary>
      <div className="mt-2 max-h-52 divide-y divide-line/60 overflow-auto rounded-[7px] border border-line/70 bg-canvas/55">
        {job.galleryFiles.map((file) => (
          <div key={file.index} className="grid grid-cols-[36px_minmax(0,1fr)_90px] items-center gap-2 px-3 py-2">
            <span className="number-font text-[10px] text-muted">#{file.index}</span>
            <div className="min-w-0">
              <p className="truncate text-ink">{file.filename}</p>
              <p className="mt-0.5 text-[10px]">{file.sizeBytes ? formatBytes(file.sizeBytes) : "Size unknown"}</p>
            </div>
            <span className={`text-right text-[10px] font-semibold uppercase tracking-[0.04em] ${file.state === "failed" ? "text-red-400" : file.state === "completed" ? "text-emerald-400" : file.state === "downloading" ? "text-accent" : "text-muted"}`}>
              {file.state === "downloading" ? `${file.progressPercent.toFixed(0)}%` : file.state}
            </span>
          </div>
        ))}
      </div>
    </details>
  );
}

function StatusBadge({ job }: { job: DownloadJob }) {
  const retrying = job.currentOperation.toLocaleLowerCase().includes("retry");
  const label = retrying ? "Retrying" : job.state;
  const tone = job.state === "Failed" ? "bg-red-500/10 text-red-400" : job.state === "Completed" ? "bg-emerald-500/10 text-emerald-400" : retrying ? "bg-amber-500/10 text-amber-400" : job.state === "Verifying" ? "bg-violet-500/10 text-violet-400" : job.state === "Paused" || job.state === "Cancelled" ? "bg-line/50 text-muted" : "bg-accent/10 text-accent";
  return <span className={`inline-flex rounded-full px-2.5 py-1 text-[10px] font-semibold tracking-[0.02em] ${tone}`}>{label}</span>;
}
