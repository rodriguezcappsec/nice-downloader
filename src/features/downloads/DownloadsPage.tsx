import { Ban, Download, ListOrdered, RotateCcw } from "lucide-react";
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
  const setView = useAppStore((state) => state.setView);
  const visible = jobs
    .filter((job) => (queueOnly ? job.state === "Queued" : true))
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt));

  return (
    <Page
      title={queueOnly ? "Queue" : "Downloads"}
      description={queueOnly ? "Downloads waiting for an available worker." : "Live status from the Rust download queue."}
      actions={
        <Button onClick={() => setView("new")}><Download size={16} /> New download</Button>
      }
    >
      {visible.length === 0 ? (
        <EmptyState
          icon={queueOnly ? <ListOrdered size={20} /> : <Download size={20} />}
          title={queueOnly ? "The queue is empty" : "No downloads yet"}
          description={queueOnly ? "Queued jobs appear here while the configured workers are busy." : "Analyze a URL and start a download to see live progress here."}
        />
      ) : (
        <div className="divide-y divide-line border border-line bg-panel">
          {visible.map((job) => <DownloadRow key={job.id} job={job} onCancel={() => void cancel(job.id)} />)}
        </div>
      )}
    </Page>
  );
}

function DownloadRow({ job, onCancel }: { job: DownloadJob; onCancel: () => void }) {
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
            <Button variant="danger" className="h-8 px-2.5" onClick={onCancel}><Ban size={14} /> Cancel</Button>
          ) : job.state === "Failed" ? (
            <Button variant="ghost" className="h-8 px-2.5" disabled title="Retry is planned for milestone 2"><RotateCcw size={14} /> Retry</Button>
          ) : null}
        </div>
        <div className="mt-3"><Progress value={job.progressPercent} /></div>
        <div className="mt-2 grid grid-cols-[1fr_auto_auto] gap-5 text-xs text-muted">
          <span className="truncate">{job.error || job.currentOperation}</span>
          <span>{job.downloadedBytes != null ? `${formatBytes(job.downloadedBytes)} / ${formatBytes(job.totalBytes)}` : ""}</span>
          <span>{job.speedBytesPerSecond != null ? `${formatBytes(job.speedBytesPerSecond)}/s · ETA ${formatEta(job.etaSeconds)}` : `${job.progressPercent.toFixed(0)}%`}</span>
        </div>
      </div>
    </article>
  );
}
