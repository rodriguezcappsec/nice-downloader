import { CheckCircle2, Copy, ExternalLink, FolderOpen, History, MoreHorizontal, RefreshCw, RotateCcw, Search, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { Page } from "../../components/Page";
import { Button, EmptyState, Input } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";

export function HistoryPage({ embedded = false }: { embedded?: boolean }) {
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
  const verifiedCount = history.filter((entry) => entry.currentOperation.includes("verified")).length;
  const warningCount = history.filter((entry) => entry.currentOperation.includes("warning")).length;
  const storedBytes = history.reduce((total, entry) => total + (entry.totalBytes ?? 0), 0);

  return (
    <Page
      title="History"
      description="Completed downloads persisted on this device."
      actions={<Button variant="secondary" disabled={loading} onClick={() => void loadHistory()}><RefreshCw className={loading ? "animate-spin" : ""} size={15} /> Refresh</Button>}
      embedded={embedded}
    >
      <section className="ingest-card relative mb-5 overflow-hidden rounded-media border border-line/45 px-6 py-6 sm:px-8" aria-label="Download history overview">
        <div className="pointer-events-none absolute -right-10 -top-24 size-72 rounded-full bg-accent/[0.08] blur-3xl" />
        <div className="relative grid items-end gap-6 lg:grid-cols-[minmax(0,1fr)_auto]">
          <div>
            <p className="text-[10px] font-semibold tracking-[0.1em] text-accent">Download ledger</p>
            <p className="number-font mt-3 text-[42px] font-semibold leading-none tracking-[-0.06em]">{history.length.toLocaleString()}</p>
            <p className="mt-2 max-w-lg text-xs leading-5 text-muted">A local record of finished transfers. Removing an entry here never removes the downloaded file or its archive identity.</p>
          </div>
          <dl className="grid grid-cols-3 gap-px overflow-hidden rounded-panel border border-line/50 bg-line/50 text-right">
            <HistoryMetric label="Verified" value={verifiedCount.toLocaleString()} />
            <HistoryMetric label="Attention" value={warningCount.toLocaleString()} tone={warningCount > 0 ? "warning" : "default"} />
            <HistoryMetric label="Recorded" value={storedBytes > 0 ? formatBytes(storedBytes) : "—"} />
          </dl>
        </div>
      </section>

      <section className="mb-4 flex flex-wrap items-center gap-3 rounded-panel bg-panel/55 p-2.5 shadow-control" aria-label="Search download history">
        <label className="relative min-w-64 flex-1">
          <span className="sr-only">Search download history</span>
          <Search className="pointer-events-none absolute left-3.5 top-3 text-muted" size={16} />
          <Input className="h-10 bg-elevated/65 pl-10" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search title, site, or saved path" />
        </label>
        <span className="number-font px-2 text-[10px] text-muted">{visible.length} of {history.length} records</span>
      </section>

      {history.length === 0 ? (
        <EmptyState icon={<History size={20} />} title="No completed downloads" description="Successfully completed jobs are saved here automatically." />
      ) : visible.length === 0 ? (
        <EmptyState icon={<Search size={20} />} title="No matching downloads" description="Try a title, source URL, or part of the destination path." />
      ) : (
        <section className="grid gap-2.5" aria-label="Completed downloads">
          {visible.map((entry) => {
            const verified = entry.currentOperation.includes("verified");
            const warning = entry.currentOperation.includes("warning");
            return (
              <article key={entry.id} className="group grid gap-4 rounded-panel border border-line/45 bg-panel/60 p-3.5 transition duration-200 hover:-translate-y-px hover:border-line/80 hover:bg-panel/85 hover:shadow-control lg:grid-cols-[44px_minmax(0,1fr)_auto] lg:items-center">
                <div className={`grid size-11 place-items-center rounded-control border ${warning ? "border-amber-400/20 bg-amber-400/[0.08] text-amber-300" : "border-emerald-400/15 bg-emerald-400/[0.07] text-emerald-300"}`}><CheckCircle2 size={19} /></div>
                <div className="min-w-0">
                  <div className="flex min-w-0 flex-wrap items-center gap-2">
                    <h2 className="min-w-0 truncate text-[13px] font-semibold tracking-[-0.015em]">{entry.title}</h2>
                    <span className="rounded-[6px] bg-elevated px-2 py-1 text-[9px] font-semibold text-muted">{entry.outputFormat.toUpperCase()}</span>
                    <span className="rounded-[6px] bg-elevated px-2 py-1 text-[9px] text-muted">{entry.selectedQuality}</span>
                  </div>
                  <p className="mt-1.5 truncate text-[11px] text-muted" title={entry.outputPath || entry.destination}>{entry.outputPath || entry.destination}</p>
                  <p className={`mt-1 text-[10px] ${verified ? "text-emerald-300" : warning ? "text-amber-300" : "text-muted"}`}>{entry.currentOperation}</p>
                </div>
                <div className="flex min-w-0 flex-wrap items-center justify-between gap-3 lg:justify-end">
                  <div className="number-font text-left text-[10px] leading-5 text-muted lg:text-right">
                    <p>{entry.totalBytes ? formatBytes(entry.totalBytes) : "Size unavailable"}</p>
                    <p>{formatDate(entry.completedAt)}</p>
                  </div>
                  <div className="flex items-center gap-1">
                    <Button variant="secondary" className="h-9 px-3 text-xs" onClick={() => void repeatDownload(entry.id)}><RotateCcw size={13} /> Again</Button>
                    <Button className="h-9 px-3 text-xs" onClick={() => void api.openDownloadOutput(entry.id, false).catch((error) => setError(normalizeError(error)))}><ExternalLink size={13} /> Open</Button>
                    <details className="relative">
                      <summary className="grid size-9 cursor-pointer list-none place-items-center rounded-control text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" aria-label={`More actions for ${entry.title}`}><MoreHorizontal size={16} /></summary>
                      <div className="panel-shadow absolute right-0 top-11 z-20 grid w-44 gap-1 rounded-control border border-line/70 bg-panel/95 p-1.5 backdrop-blur-xl">
                        <button className="flex items-center gap-2 rounded-[7px] px-2.5 py-2 text-left text-xs text-muted transition hover:bg-elevated hover:text-ink" onClick={() => void api.openDownloadOutput(entry.id, true).catch((error) => setError(normalizeError(error)))}><FolderOpen size={13} /> Reveal in folder</button>
                        <button className="flex items-center gap-2 rounded-[7px] px-2.5 py-2 text-left text-xs text-muted transition hover:bg-elevated hover:text-ink" onClick={() => void navigator.clipboard.writeText(entry.url)}><Copy size={13} /> Copy source URL</button>
                        <button className="flex items-center gap-2 rounded-[7px] px-2.5 py-2 text-left text-xs text-red-300 transition hover:bg-red-400/10" onClick={() => void api.deleteHistoryEntry(entry.id).then(() => loadHistory()).catch((error) => setError(normalizeError(error)))}><Trash2 size={13} /> Remove record</button>
                      </div>
                    </details>
                  </div>
                </div>
              </article>
            );
          })}
        </section>
      )}
    </Page>
  );
}

function HistoryMetric({ label, value, tone = "default" }: { label: string; value: string; tone?: "default" | "warning" }) {
  return <div className="min-w-24 bg-panel/75 px-4 py-3"><dt className="text-[9px] font-semibold tracking-[0.08em] text-muted">{label}</dt><dd className={`number-font mt-1 text-sm font-semibold ${tone === "warning" ? "text-amber-300" : "text-ink"}`}>{value}</dd></div>;
}

function formatDate(value?: string | null): string {
  if (!value) return "Unknown date";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
