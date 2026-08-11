import { CheckCircle2, Copy, ExternalLink, FolderOpen, History, RefreshCw, RotateCcw, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Input } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";

export function HistoryPage() {
  const history = useAppStore((state) => state.history);
  const loading = useAppStore((state) => state.historyLoading);
  const loadHistory = useAppStore((state) => state.loadHistory);
  const setError = useAppStore((state) => state.setError);
  const repeatDownload = useAppStore((state) => state.repeatDownload);
  const [search, setSearch] = useState("");
  const visible = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    return query ? history.filter((entry) => `${entry.title} ${entry.url} ${entry.outputPath ?? ""}`.toLocaleLowerCase().includes(query)) : history;
  }, [history, search]);
  return (
    <Page
      title="History"
      description="Completed downloads persisted on this device."
      actions={<Button variant="secondary" disabled={loading} onClick={() => void loadHistory()}><RefreshCw className={loading ? "animate-spin" : ""} size={15} /> Refresh</Button>}
    >
      <Input className="mb-4" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search title, site, or saved path" />
      {history.length === 0 ? (
        <EmptyState icon={<History size={20} />} title="No completed downloads" description="Successfully completed jobs are saved here automatically." />
      ) : (
        <div className="divide-y divide-line border border-line bg-panel">
          {visible.map((entry) => (
            <article key={entry.id} className="grid grid-cols-[42px_minmax(0,1fr)_auto] items-center gap-4 p-4">
              <div className="grid size-10 place-items-center rounded-md bg-emerald-500/10 text-emerald-400"><CheckCircle2 size={19} /></div>
              <div className="min-w-0">
                <h3 className="truncate text-sm font-semibold">{entry.title}</h3>
                <p className="mt-1 truncate text-xs text-muted">{entry.outputPath || entry.destination}</p>
                <p className={`mt-1 text-[11px] ${entry.currentOperation.includes("verified") ? "text-emerald-400" : entry.currentOperation.includes("warning") ? "text-amber-400" : "text-muted"}`}>{entry.currentOperation}</p>
              </div>
              <div className="text-right text-xs text-muted">
                <div>{entry.outputFormat.toUpperCase()} · {entry.selectedQuality}</div>
                <div className="mt-1">{entry.totalBytes ? formatBytes(entry.totalBytes) : "Completed"} · {formatDate(entry.completedAt)}</div>
                <div className="mt-2 flex justify-end gap-1">
                  <Button variant="ghost" className="h-7 px-2 text-xs" onClick={() => void repeatDownload(entry.id)}><RotateCcw size={13} /> Again</Button>
                  <Button variant="ghost" className="h-7 px-2 text-xs" onClick={() => void api.openDownloadOutput(entry.id, false).catch((error) => setError(normalizeError(error)))}><ExternalLink size={13} /> Open</Button>
                  <Button variant="ghost" className="h-7 px-2 text-xs" onClick={() => void api.openDownloadOutput(entry.id, true).catch((error) => setError(normalizeError(error)))}><FolderOpen size={13} /> Reveal</Button>
                  <Button variant="ghost" className="h-7 px-2 text-xs" onClick={() => void navigator.clipboard.writeText(entry.url)}><Copy size={13} /> URL</Button>
                  <Button variant="danger" className="h-7 px-2 text-xs" onClick={() => void api.deleteHistoryEntry(entry.id).then(() => loadHistory()).catch((error) => setError(normalizeError(error)))}><Trash2 size={13} /> Record</Button>
                </div>
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
