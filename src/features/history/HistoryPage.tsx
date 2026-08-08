import { CheckCircle2, History, RefreshCw } from "lucide-react";
import { Page } from "../../components/Page";
import { Button, EmptyState } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { useAppStore } from "../../stores/app-store";

export function HistoryPage() {
  const history = useAppStore((state) => state.history);
  const loading = useAppStore((state) => state.historyLoading);
  const loadHistory = useAppStore((state) => state.loadHistory);
  return (
    <Page
      title="History"
      description="Completed downloads persisted on this device."
      actions={<Button variant="secondary" disabled={loading} onClick={() => void loadHistory()}><RefreshCw className={loading ? "animate-spin" : ""} size={15} /> Refresh</Button>}
    >
      {history.length === 0 ? (
        <EmptyState icon={<History size={20} />} title="No completed downloads" description="Successfully completed jobs are saved here automatically." />
      ) : (
        <div className="divide-y divide-line border border-line bg-panel">
          {history.map((entry) => (
            <article key={entry.id} className="grid grid-cols-[42px_minmax(0,1fr)_auto] items-center gap-4 p-4">
              <div className="grid size-10 place-items-center rounded-md bg-emerald-500/10 text-emerald-400"><CheckCircle2 size={19} /></div>
              <div className="min-w-0">
                <h3 className="truncate text-sm font-semibold">{entry.title}</h3>
                <p className="mt-1 truncate text-xs text-muted">{entry.outputPath || entry.destination}</p>
              </div>
              <div className="text-right text-xs text-muted">
                <div>{entry.outputFormat.toUpperCase()} · {entry.selectedQuality}</div>
                <div className="mt-1">{entry.totalBytes ? formatBytes(entry.totalBytes) : "Completed"} · {formatDate(entry.completedAt)}</div>
              </div>
            </article>
          ))}
        </div>
      )}
    </Page>
  );
}

function formatDate(value?: string | null): string {
  if (!value) return "Unknown date";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
