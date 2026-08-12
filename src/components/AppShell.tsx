import {
  Activity,
  Clock3,
  Download,
  History,
  ListOrdered,
  Library,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Rss,
  Settings,
  SlidersHorizontal,
  WandSparkles,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { useAppStore } from "../stores/app-store";
import type { ViewId } from "../types";

const workspaceNavigation: { id: ViewId; label: string; icon: typeof Plus }[] = [
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "queue", label: "Queue", icon: ListOrdered },
  { id: "history", label: "History", icon: History },
  { id: "subscriptions", label: "Subscriptions", icon: Rss },
  { id: "transformations", label: "Transform", icon: WandSparkles },
  { id: "library", label: "Library", icon: Library },
  { id: "presets", label: "Presets", icon: SlidersHorizontal },
];

export function AppShell({ children }: { children: ReactNode }) {
  const [collapsed, setCollapsed] = useState(
    () => window.localStorage.getItem("nice-downloader.sidebar") === "collapsed",
  );
  const view = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const jobMap = useAppStore((state) => state.jobs);
  const jobs = Object.values(jobMap);
  const activeCount = jobs.filter((job) =>
    ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state),
  ).length;
  const queuedCount = jobs.filter((job) => job.state === "Queued").length;

  function toggleSidebar() {
    setCollapsed((value) => {
      window.localStorage.setItem("nice-downloader.sidebar", value ? "expanded" : "collapsed");
      return !value;
    });
  }

  return (
    <>
      <a className="skip-link" href="#main-content">Skip to main content</a>
      <div className={`grid h-screen min-h-[620px] overflow-hidden text-ink transition-[grid-template-columns] duration-300 ${collapsed ? "grid-cols-[76px_minmax(0,1fr)]" : "grid-cols-[252px_minmax(0,1fr)]"}`}>
        <aside className="relative z-30 flex min-h-0 flex-col border-r border-line/70 bg-panel/95 shadow-[10px_0_40px_rgb(0_0_0/0.08)] backdrop-blur-xl">
          <div className={`flex h-[76px] items-center gap-3 border-b border-line/60 ${collapsed ? "justify-center px-2" : "px-5"}`}>
            <div className="relative grid size-10 shrink-0 place-items-center overflow-hidden rounded-[10px] bg-accent text-[#06251b] shadow-[0_9px_24px_rgb(var(--accent)/0.22)]">
              <Download size={19} strokeWidth={2.6} />
              <span className="absolute inset-x-0 bottom-0 h-px bg-white/35" />
            </div>
            <div className={collapsed ? "hidden" : "min-w-0"}>
              <div className="truncate text-[15px] font-semibold tracking-[-0.025em]">Nice Downloader</div>
              <div className="mt-0.5 text-[10px] font-medium uppercase tracking-[0.16em] text-muted">Desktop media utility</div>
            </div>
          </div>

          <div className="px-3 pb-2 pt-4">
            <button
              title={collapsed ? "New download" : undefined}
              onClick={() => setView("new")}
              className={`group flex h-11 w-full items-center gap-3 rounded-[9px] border px-3 text-left text-sm font-semibold transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${
                view === "new"
                  ? "border-accent bg-accent text-[#06251b] shadow-[0_10px_26px_rgb(var(--accent)/0.16)]"
                  : "border-line/80 bg-elevated/60 text-ink shadow-control hover:-translate-y-0.5 hover:border-accent/50 hover:bg-elevated"
              } ${collapsed ? "justify-center" : ""}`}
            >
              <Plus size={17} strokeWidth={2.5} />
              <span className={collapsed ? "hidden" : ""}>New download</span>
              {!collapsed ? <kbd className={`ml-auto text-[10px] font-medium ${view === "new" ? "text-[#06251b]/65" : "text-muted"}`}>Ctrl N</kbd> : null}
            </button>
          </div>

          <nav className="min-h-0 flex-1 px-3 py-3" aria-label="Primary navigation">
            <p className={`mb-2 px-3 text-[10px] font-semibold uppercase tracking-[0.18em] text-muted/70 ${collapsed ? "sr-only" : ""}`}>Workspace</p>
            <div className="space-y-1">
              {workspaceNavigation.map((item) => {
                const Icon = item.icon;
                const badge = item.id === "downloads" ? activeCount : item.id === "queue" ? queuedCount : 0;
                const active = view === item.id;
                return (
                  <button
                    key={item.id}
                    title={collapsed ? item.label : undefined}
                    onClick={() => setView(item.id)}
                    className={`relative flex h-10 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[13px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${
                      active ? "bg-elevated text-ink shadow-[inset_3px_0_0_rgb(var(--accent))]" : "text-muted hover:bg-elevated/60 hover:text-ink"
                    } ${collapsed ? "justify-center" : ""}`}
                  >
                    <Icon size={17} strokeWidth={active ? 2.25 : 1.8} className={active ? "text-accent" : ""} />
                    <span className={`flex-1 ${collapsed ? "hidden" : ""}`}>{item.label}</span>
                    {badge > 0 ? (
                      <span className={`${collapsed ? "absolute right-1.5 top-1.5 size-2 p-0 text-transparent" : "min-w-5 px-1.5 py-0.5 text-center text-[10px]"} rounded-[5px] bg-accent/15 font-semibold text-accent`}>{badge}</span>
                    ) : null}
                  </button>
                );
              })}
            </div>
          </nav>

          <div className="border-t border-line/60 p-3">
            <button
              title={collapsed ? "Settings" : undefined}
              onClick={() => setView("settings")}
              className={`flex h-10 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[13px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${view === "settings" ? "bg-elevated text-ink shadow-[inset_3px_0_0_rgb(var(--accent))]" : "text-muted hover:bg-elevated/60 hover:text-ink"} ${collapsed ? "justify-center" : ""}`}
            >
              <Settings size={17} strokeWidth={view === "settings" ? 2.25 : 1.8} className={view === "settings" ? "text-accent" : ""} />
              <span className={collapsed ? "hidden" : "flex-1"}>Settings</span>
              {!collapsed ? <kbd className="text-[10px] font-medium text-muted">Ctrl ,</kbd> : null}
            </button>

            <div className={`mx-2 my-3 h-px bg-line/60 ${collapsed ? "mx-1" : ""}`} />
            <div className={`flex items-center gap-3 px-2 ${collapsed ? "justify-center" : ""}`}>
              <div className={`relative grid size-8 shrink-0 place-items-center rounded-[8px] ${activeCount > 0 ? "bg-accent/12 text-accent" : "bg-elevated text-muted"}`}>
                {activeCount > 0 ? <Activity size={15} /> : <Clock3 size={15} />}
                {activeCount > 0 ? <span className="absolute right-0.5 top-0.5 size-1.5 rounded-full bg-accent shadow-[0_0_0_3px_rgb(var(--panel))]" /> : null}
              </div>
              <div className={collapsed ? "hidden" : "min-w-0 flex-1"}>
                <div className="text-[11px] font-semibold text-ink">{activeCount > 0 ? `${activeCount} active` : "System ready"}</div>
                <div className="mt-0.5 truncate text-[10px] text-muted">{queuedCount > 0 ? `${queuedCount} waiting in queue` : "Queue is standing by"}</div>
              </div>
              <button className={`grid size-8 shrink-0 place-items-center rounded-[7px] text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "absolute bottom-[76px] right-[-16px] border border-line bg-panel shadow-control" : ""}`} onClick={toggleSidebar} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
                {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
              </button>
            </div>
          </div>
        </aside>

        <main id="main-content" tabIndex={-1} className="relative min-h-0 overflow-y-auto bg-canvas/90 outline-none">
          {children}
        </main>
      </div>
    </>
  );
}
