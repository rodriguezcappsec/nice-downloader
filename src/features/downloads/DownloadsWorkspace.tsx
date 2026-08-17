import { AlertTriangle, CalendarClock, CheckCircle2, Download, ListOrdered, Plus } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button } from "../../components/ui";
import { useAppStore } from "../../stores/app-store";
import type { ViewId } from "../../types";
import { DownloadsPage, type DownloadWorkspaceTab } from "./DownloadsPage";
import { HistoryPage } from "../history/HistoryPage";

type WorkspaceTab = DownloadWorkspaceTab | "completed";

const tabs: Array<{ id: WorkspaceTab; label: string; icon: typeof Download }> = [
  { id: "active", label: "Active", icon: Download },
  { id: "queue", label: "Queue", icon: ListOrdered },
  { id: "scheduled", label: "Scheduled", icon: CalendarClock },
  { id: "completed", label: "Completed", icon: CheckCircle2 },
  { id: "failed", label: "Failed", icon: AlertTriangle },
];

function tabForView(view: ViewId): WorkspaceTab {
  if (view === "queue") return "queue";
  if (view === "history") return "completed";
  return "active";
}

export function DownloadsWorkspace({ sourceView }: { sourceView: ViewId }) {
  const [tab, setTab] = useState<WorkspaceTab>(() => tabForView(sourceView));
  const setView = useAppStore((state) => state.setView);
  const jobs = useAppStore((state) => Object.values(state.jobs));
  const historyCount = useAppStore((state) => state.history.length);

  useEffect(() => {
    if (sourceView === "queue" || sourceView === "history") setTab(tabForView(sourceView));
  }, [sourceView]);

  const counts = useMemo(() => {
    let active = 0;
    let queue = 0;
    let scheduled = 0;
    let failed = 0;
    const now = Date.now();
    for (const job of jobs) {
      const future = Boolean(job.scheduledFor && Date.parse(job.scheduledFor) > now);
      if (["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state)) active += 1;
      else if (["Queued", "Paused"].includes(job.state)) future ? scheduled += 1 : queue += 1;
      else if (job.state === "Failed" || job.state === "Cancelled") failed += 1;
    }
    return { active, queue, scheduled, completed: historyCount, failed };
  }, [historyCount, jobs]);

  function openTab(next: WorkspaceTab) {
    setTab(next);
    if (next === "queue") setView("queue");
    else if (next === "completed") setView("history");
    else setView("downloads");
  }

  return (
    <div className="workspace-enter mx-auto w-full max-w-[1500px] px-7 pb-16 pt-8 xl:px-11">
      <header className="mb-7 flex flex-wrap items-end justify-between gap-5">
        <div>
          <p className="eyebrow">Background activity</p>
          <h1 className="mt-2 text-[34px] font-semibold leading-none tracking-[-0.05em]">Downloads</h1>
          <p className="mt-3 max-w-[64ch] text-[13px] leading-5 text-muted">One place for live transfers, queue order, scheduled work, completed files, and recovery.</p>
        </div>
        <Button onClick={() => setView("new")}><Plus size={16} /> New Download</Button>
      </header>

      <nav className="workspace-tabs mb-7 flex min-w-0 items-center gap-1 overflow-x-auto rounded-[13px] border border-line/45 bg-panel/55 p-1.5" aria-label="Download sections">
        {tabs.map((item) => {
          const Icon = item.icon;
          const count = counts[item.id];
          return (
            <button key={item.id} className={`flex h-10 shrink-0 items-center gap-2 rounded-[9px] px-3 text-[13px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${tab === item.id ? "bg-elevated text-ink shadow-control" : "text-muted hover:bg-elevated/60 hover:text-ink"}`} onClick={() => openTab(item.id)} aria-current={tab === item.id ? "page" : undefined}>
              <Icon size={15} /> {item.label}
              {count > 0 ? <span className={`number-font min-w-5 rounded-[6px] px-1.5 py-0.5 text-center text-[10px] ${tab === item.id ? "bg-accent/15 text-accent" : "bg-canvas/65 text-muted"}`}>{count}</span> : null}
            </button>
          );
        })}
      </nav>

      {tab === "completed" ? <HistoryPage embedded /> : <DownloadsPage embedded tab={tab} />}
    </div>
  );
}
