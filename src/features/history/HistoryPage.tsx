import { CheckCircle2, Copy, ExternalLink, FolderOpen, History, RefreshCw, RotateCcw, Search, Trash2 } from "lucide-react";
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
      <div className="mb-5 flex items-center gap-3">
        <div className="relative max-w-xl flex-1">
          <Search className="pointer-events-none absolute left-3.5 top-3.5 text-muted" size={16} />
          <Input className="pl-10" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search title, site, or saved path" aria-label="Search download history" />
        </div>
        <span className="number-font text-[10px] text-muted">{visible.length} of {history.length} records</span>
      </div>
      {history.length === 0 ? (
        <EmptyState icon={<History size={20} />} title="No completed downloads" description="Successfully completed jobs are saved here automatically." />
      ) : visible.length === 0 ? (
        <EmptyState icon={<Search size={20} />} title="No matching downloads" description="Try a title, source URL, or part of the destination path." />
      ) : (
        <div className="panel-shadow divide-y divide-line/65 overflow-hidden rounded-[11px] border border-line/60 bg-panel/70">
          {visible.map((entry) => (
            <article key={entry.id} className="grid grid-cols-[42px_minmax(0,1fr)_auto] items-center gap-4 p-4 transition-colors duration-200 hover:bg-elevated/25">
              <div className="grid size-10 place-items-center rounded-[9px] border border-emerald-500/15 bg-emerald-500/[0.07] text-emerald-400"><CheckCircle2 size={18} /></div>
              <div className="min-w-0">
                <h3 className="truncate text-[13px] font-semibold tracking-[-0.01em]">{entry.title}</h3>
                <p className="mt-1 truncate text-xs text-muted">{entry.outputPath || entry.destination}</p>
                <p className={`mt-1 text-[11px] ${entry.currentOperation.includes("verified") ? "text-emerald-400" : entry.currentOperation.includes("warning") ? "text-amber-400" : "text-muted"}`}>{entry.currentOperation}</p>
              </div>
              <div className="text-right text-[11px] text-muted">
                <div>{entry.outputFormat.toUpperCase()} · {entry.selectedQuality}</div>
                <div className="number-font mt-1">{entry.totalBytes ? formatBytes(entry.totalBytes) : "Completed"} · {formatDate(entry.completedAt)}</div>
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
