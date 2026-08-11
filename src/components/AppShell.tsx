import {
  Clock3,
  Download,
  Gauge,
  History,
  ListOrdered,
  Plus,
  PanelLeftClose,
  PanelLeftOpen,
  Settings,
  SlidersHorizontal,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { useAppStore } from "../stores/app-store";
import type { ViewId } from "../types";

const navigation: { id: ViewId; label: string; icon: typeof Plus }[] = [
  { id: "new", label: "New Download", icon: Plus },
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "queue", label: "Queue", icon: ListOrdered },
  { id: "history", label: "History", icon: History },
  { id: "presets", label: "Presets", icon: SlidersHorizontal },
  { id: "settings", label: "Settings", icon: Settings },
];

export function AppShell({ children }: { children: ReactNode }) {
  const [collapsed, setCollapsed] = useState(() => window.localStorage.getItem("nice-downloader.sidebar") === "collapsed");
  const view = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const jobMap = useAppStore((state) => state.jobs);
  const jobs = Object.values(jobMap);
  const activeCount = jobs.filter((job) =>
    ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state),
  ).length;
  const queuedCount = jobs.filter((job) => job.state === "Queued").length;

  return (
    <div className={`grid h-screen min-h-[620px] overflow-hidden bg-canvas text-ink transition-[grid-template-columns] ${collapsed ? "grid-cols-[68px_minmax(0,1fr)]" : "grid-cols-[228px_minmax(0,1fr)]"}`}>
      <aside className="flex min-h-0 flex-col border-r border-line bg-panel">
        <div className={`flex h-16 items-center gap-3 border-b border-line ${collapsed ? "justify-center px-2" : "px-5"}`}>
          <div className="grid size-9 place-items-center rounded-md bg-accent text-white shadow-panel">
            <Download size={19} strokeWidth={2.4} />
          </div>
          <div className={collapsed ? "hidden" : ""}>
            <div className="text-sm font-semibold tracking-tight">Nice Downloader</div>
            <div className="text-[11px] text-muted">Media download manager</div>
          </div>
        </div>

        <nav className="flex-1 space-y-1 p-3" aria-label="Primary navigation">
          {navigation.map((item) => {
            const Icon = item.icon;
            const badge = item.id === "downloads" ? activeCount : item.id === "queue" ? queuedCount : 0;
            return (
              <button
                key={item.id}
                title={collapsed ? item.label : undefined}
                onClick={() => setView(item.id)}
                className={`flex h-10 w-full items-center gap-3 rounded-md px-3 text-left text-sm transition-colors ${
                  view === item.id ? "bg-elevated text-ink" : "text-muted hover:bg-elevated/60 hover:text-ink"
                }`}
              >
                <Icon size={17} />
                <span className={`flex-1 ${collapsed ? "hidden" : ""}`}>{item.label}</span>
                {badge > 0 ? (
                  <span className="min-w-5 rounded-full bg-accent/15 px-1.5 text-center text-[11px] font-semibold text-blue-400">
                    {badge}
                  </span>
                ) : null}
              </button>
            );
          })}
        </nav>

        <div className="border-t border-line p-3">
          <div className={`flex items-center gap-3 text-xs text-muted ${collapsed ? "justify-center" : ""}`}>
            {activeCount > 0 ? <Gauge size={15} className="text-blue-400" /> : <Clock3 size={15} />}
            <span className={collapsed ? "hidden" : ""}>{activeCount > 0 ? `${activeCount} active download${activeCount === 1 ? "" : "s"}` : "Ready"}</span>
          </div>
          <button className="mt-3 flex h-8 w-full items-center justify-center gap-2 rounded text-xs text-muted hover:bg-elevated hover:text-ink" onClick={() => setCollapsed((value) => { window.localStorage.setItem("nice-downloader.sidebar", value ? "expanded" : "collapsed"); return !value; })} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>{collapsed ? <PanelLeftOpen size={15} /> : <><PanelLeftClose size={15} /><span>Collapse sidebar</span></>}</button>
        </div>
      </aside>
      <main className="min-h-0 overflow-y-auto">{children}</main>
    </div>
  );
}
