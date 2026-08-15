import {
  Activity,
  Clock3,
  Download,
  History,
  House,
  Images,
  Keyboard,
  LayoutGrid,
  ListOrdered,
  ListChecks,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Rss,
  Search,
  Settings,
  SlidersHorizontal,
  SquarePlay,
  WandSparkles,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { useAppStore } from "../stores/app-store";
import type { ViewId } from "../types";

interface NavigationItem {
  id: ViewId;
  label: string;
  icon: LucideIcon;
}

const mediaNavigation: NavigationItem[] = [
  { id: "library", label: "Media home", icon: House },
  { id: "compare", label: "Compare videos", icon: LayoutGrid },
  { id: "imageCompare", label: "Compare images", icon: Images },
  { id: "curation", label: "Review library", icon: ListChecks },
];

const downloadNavigation: NavigationItem[] = [
  { id: "new", label: "New download", icon: Plus },
  { id: "downloads", label: "Active downloads", icon: Download },
  { id: "queue", label: "Queue", icon: ListOrdered },
  { id: "history", label: "Download history", icon: History },
  { id: "subscriptions", label: "Subscriptions", icon: Rss },
];

const toolNavigation: NavigationItem[] = [
  { id: "transformations", label: "Transform media", icon: WandSparkles },
  { id: "presets", label: "Download presets", icon: SlidersHorizontal },
];

export function AppShell({ children }: { children: ReactNode }) {
  const [collapsed, setCollapsed] = useState(
    () => window.localStorage.getItem("nice-downloader.sidebar") === "collapsed",
  );
  const [commandOpen, setCommandOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [commandQuery, setCommandQuery] = useState("");
  const restoreFocus = useRef<HTMLElement | null>(null);
  const view = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const jobMap = useAppStore((state) => state.jobs);
  const compareCount = useAppStore((state) => state.compareItemIds.length);
  const imageCompareCount = useAppStore((state) => state.imageCompareItemIds.length);
  const jobs = Object.values(jobMap);
  const activeCount = jobs.filter((job) =>
    ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state),
  ).length;
  const queuedCount = jobs.filter((job) => job.state === "Queued").length;
  const allNavigation = useMemo(() => [
    ...mediaNavigation,
    ...downloadNavigation,
    ...toolNavigation,
    { id: "settings" as ViewId, label: "Settings", icon: Settings },
  ], []);
  const matchingNavigation = allNavigation.filter((item) => item.label.toLocaleLowerCase().includes(commandQuery.trim().toLocaleLowerCase()));

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        restoreFocus.current = document.activeElement as HTMLElement | null;
        setCommandQuery("");
        setCommandOpen(true);
        setHelpOpen(false);
      } else if (event.key === "?" && !event.ctrlKey && !event.metaKey && !event.altKey && !target?.matches("input, textarea, select, [contenteditable='true']")) {
        event.preventDefault();
        restoreFocus.current = document.activeElement as HTMLElement | null;
        setHelpOpen(true);
        setCommandOpen(false);
      } else if (event.key === "Escape" && (commandOpen || helpOpen)) {
        event.preventDefault();
        setCommandOpen(false);
        setHelpOpen(false);
        window.setTimeout(() => restoreFocus.current?.focus(), 0);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [commandOpen, helpOpen]);

  function navigateFromCommand(nextView: ViewId) {
    setView(nextView);
    setCommandOpen(false);
    window.setTimeout(() => document.getElementById("main-content")?.focus(), 0);
  }

  function closeOverlay() {
    setCommandOpen(false);
    setHelpOpen(false);
    window.setTimeout(() => restoreFocus.current?.focus(), 0);
  }

  function trapDialogFocus(event: ReactKeyboardEvent<HTMLElement>) {
    if (event.key !== "Tab") return;
    const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), [href], [tabindex]:not([tabindex='-1'])"));
    const first = controls[0];
    const last = controls.at(-1);
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function toggleSidebar() {
    setCollapsed((value) => {
      window.localStorage.setItem("nice-downloader.sidebar", value ? "expanded" : "collapsed");
      return !value;
    });
  }

  function badgeFor(id: ViewId): number {
    if (id === "downloads") return activeCount;
    if (id === "queue") return queuedCount;
    if (id === "compare") return compareCount;
    if (id === "imageCompare") return imageCompareCount;
    return 0;
  }

  return (
    <>
      <a className="skip-link" href="#main-content">Skip to main content</a>
      <div className={`grid h-screen min-h-[680px] overflow-hidden text-ink transition-[grid-template-columns] duration-300 ${collapsed ? "grid-cols-[72px_minmax(0,1fr)]" : "grid-cols-[238px_minmax(0,1fr)]"}`}>
        <aside className="relative z-30 flex min-h-0 flex-col border-r border-line/60 bg-panel/96 shadow-[12px_0_45px_rgb(3_20_16/0.11)] backdrop-blur-xl">
          <div className={`flex h-[76px] items-center gap-3 ${collapsed ? "justify-center px-2" : "px-5"}`}>
            <div className="relative grid size-10 shrink-0 place-items-center overflow-hidden rounded-[11px] bg-accent text-[#081426] shadow-[0_10px_26px_rgb(var(--accent)/0.22)]">
              <SquarePlay size={20} strokeWidth={2.5} />
              <span className="absolute inset-x-1 bottom-1 h-px bg-white/35" />
            </div>
            <div className={collapsed ? "hidden" : "min-w-0"}>
              <div className="truncate text-[15px] font-semibold tracking-[-0.025em]">Nice Downloader</div>
              <div className="mt-0.5 text-[10px] font-medium tracking-[0.08em] text-muted">Your private media hub</div>
            </div>
          </div>

          <nav className="min-h-0 flex-1 overflow-y-auto px-3 pb-4 pt-2" aria-label="Primary navigation">
            <NavigationGroup label="Library" items={mediaNavigation} view={view} collapsed={collapsed} setView={setView} badgeFor={badgeFor} />
            <NavigationGroup label="Downloads" items={downloadNavigation} view={view} collapsed={collapsed} setView={setView} badgeFor={badgeFor} />
            <NavigationGroup label="Tools" items={toolNavigation} view={view} collapsed={collapsed} setView={setView} badgeFor={badgeFor} />
          </nav>

          <div className="border-t border-line/60 p-3">
            <button
              title={collapsed ? "Commands and shortcuts" : undefined}
              onClick={() => { restoreFocus.current = document.activeElement as HTMLElement | null; setHelpOpen(true); }}
              className={`mb-1 flex h-9 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[12px] font-medium text-muted transition hover:bg-elevated/60 hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "justify-center" : ""}`}
            >
              <Keyboard size={16} strokeWidth={1.8} />
              <span className={collapsed ? "hidden" : "flex-1"}>Shortcuts</span>
              {!collapsed ? <kbd className="text-[10px] font-medium text-muted">?</kbd> : null}
            </button>
            <button
              title={collapsed ? "Settings" : undefined}
              onClick={() => setView("settings")}
              className={`flex h-10 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[13px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${view === "settings" ? "bg-elevated text-ink shadow-[inset_3px_0_0_rgb(var(--accent))]" : "text-muted hover:bg-elevated/60 hover:text-ink"} ${collapsed ? "justify-center" : ""}`}
            >
              <Settings size={17} strokeWidth={1.8} className={view === "settings" ? "text-accent" : ""} />
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
                <div className="text-[11px] font-semibold text-ink">{activeCount > 0 ? `${activeCount} downloading` : "Media hub ready"}</div>
                <div className="mt-0.5 truncate text-[10px] text-muted">{queuedCount > 0 ? `${queuedCount} waiting in queue` : "Downloads run in the background"}</div>
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
      {commandOpen ? <div className="fixed inset-0 z-[90] flex items-start justify-center bg-black/65 px-5 pt-[12vh] backdrop-blur-sm" onMouseDown={closeOverlay}><section className="panel-shadow w-full max-w-xl overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="command-palette-title" onKeyDown={trapDialogFocus} onMouseDown={(event) => event.stopPropagation()}><h2 id="command-palette-title" className="sr-only">Go to a Nice Downloader workspace</h2><label className="relative block border-b border-line/70"><Search className="absolute left-4 top-4 text-muted" size={18} /><input autoFocus value={commandQuery} onChange={(event) => setCommandQuery(event.target.value)} className="h-14 w-full bg-transparent py-4 pl-12 pr-4 text-sm text-ink outline-none placeholder:text-muted" placeholder="Go to a workspace…" aria-controls="command-results" /></label><div id="command-results" className="max-h-[420px] overflow-y-auto p-2">{matchingNavigation.map((item) => { const Icon = item.icon; return <button key={item.id} className="flex w-full items-center gap-3 rounded-[9px] px-3 py-3 text-left text-sm text-muted transition hover:bg-elevated hover:text-ink focus-visible:bg-elevated focus-visible:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => navigateFromCommand(item.id)}><span className="grid size-8 place-items-center rounded-[7px] bg-elevated text-accent"><Icon size={16} /></span><span className="flex-1 font-medium">{item.label}</span>{view === item.id ? <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-accent">Current</span> : null}</button>; })}{matchingNavigation.length === 0 ? <p className="px-4 py-8 text-center text-sm text-muted">No matching workspace.</p> : null}</div><div className="flex items-center justify-between border-t border-line/60 px-4 py-2.5 text-[10px] text-muted"><span>Tab to move · Enter to open</span><kbd>Esc closes</kbd></div></section></div> : null}
      {helpOpen ? <div className="fixed inset-0 z-[90] grid place-items-center bg-black/65 p-5 backdrop-blur-sm" onMouseDown={closeOverlay}><section className="panel-shadow w-full max-w-lg rounded-[14px] border border-line bg-panel p-5" role="dialog" aria-modal="true" aria-labelledby="shortcut-help-title" onKeyDown={trapDialogFocus} onMouseDown={(event) => event.stopPropagation()}><div className="flex items-start justify-between gap-4"><div><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Keyboard access</p><h2 id="shortcut-help-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Commands and shortcuts</h2><p className="mt-2 text-xs leading-5 text-muted">Navigate the app without reaching for the mouse. Tab and Shift+Tab move through every control.</p></div><button autoFocus className="grid size-8 place-items-center rounded-[7px] text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={closeOverlay} aria-label="Close shortcut help">×</button></div><dl className="mt-5 divide-y divide-line/60 border-y border-line/60">{[["Ctrl K", "Open workspace commands"], ["Ctrl N", "Start a new download"], ["Ctrl ,", "Open Settings"], ["?", "Show this shortcut guide"], ["←  →", "Browse open Library details"], ["Esc", "Close the current panel or dialog"]].map(([keys, description]) => <div key={keys} className="flex items-center justify-between gap-5 py-3 text-xs"><dt className="text-muted">{description}</dt><dd><kbd className="rounded-[6px] border border-line bg-elevated px-2 py-1 font-semibold text-ink shadow-control">{keys}</kbd></dd></div>)}</dl><button className="mt-5 w-full rounded-[8px] bg-elevated px-3 py-2.5 text-xs font-semibold text-ink transition hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => { setHelpOpen(false); setCommandQuery(""); setCommandOpen(true); }}>Open command palette</button></section></div> : null}
    </>
  );
}

function NavigationGroup({ label, items, view, collapsed, setView, badgeFor }: { label: string; items: NavigationItem[]; view: ViewId; collapsed: boolean; setView: (view: ViewId) => void; badgeFor: (id: ViewId) => number }) {
  return (
    <section className="mb-5">
      <p className={`mb-2 px-3 text-[10px] font-semibold tracking-[0.09em] text-muted/90 ${collapsed ? "sr-only" : ""}`}>{label}</p>
      <div className="space-y-1">
        {items.map((item) => {
          const Icon = item.icon;
          const badge = badgeFor(item.id);
          const active = view === item.id;
          return (
            <button
              key={item.id}
              title={collapsed ? item.label : undefined}
              onClick={() => setView(item.id)}
              aria-current={active ? "page" : undefined}
              className={`relative flex h-10 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[13px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${active ? "bg-elevated text-ink shadow-[inset_3px_0_0_rgb(var(--accent))]" : "text-muted hover:bg-elevated/60 hover:text-ink"} ${collapsed ? "justify-center" : ""}`}
            >
              <Icon size={17} strokeWidth={1.8} className={active ? "text-accent" : ""} />
              <span className={`flex-1 ${collapsed ? "hidden" : ""}`}>{item.label}</span>
              {item.id === "new" && !collapsed ? <kbd className="text-[10px] font-medium text-muted">Ctrl N</kbd> : null}
              {badge > 0 ? <span className={`${collapsed ? "absolute right-1.5 top-1.5 size-2 p-0 text-transparent" : "min-w-5 px-1.5 py-0.5 text-center text-[10px]"} rounded-[5px] bg-accent/15 font-semibold text-accent`}>{badge}</span> : null}
            </button>
          );
        })}
      </div>
    </section>
  );
}
