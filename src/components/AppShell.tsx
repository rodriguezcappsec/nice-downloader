import {
  Activity,
  Clock3,
  Film,
  Keyboard,
  LoaderCircle,
  ListOrdered,
  PanelLeftClose,
  PanelLeftOpen,
  Search,
  ScanSearch,
  SquarePlay,
  Share2,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type ReactNode } from "react";
import { useAppStore } from "../stores/app-store";
import { api } from "../lib/tauri";
import { allCommandNavigation, primaryNavigation, primaryViewFor, type NavigationItem } from "../lib/navigation";
import type { LibraryItem, LibraryPrivacyStatus, ViewId } from "../types";

export function AppShell({ children }: { children: ReactNode }) {
  const [collapsed, setCollapsed] = useState(
    () => window.localStorage.getItem("nice-downloader.sidebar") === "collapsed" || window.matchMedia("(max-width: 1120px)").matches,
  );
  const [commandOpen, setCommandOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [commandQuery, setCommandQuery] = useState("");
  const [commandMedia, setCommandMedia] = useState<LibraryItem[]>([]);
  const [privacyStatus, setPrivacyStatus] = useState<LibraryPrivacyStatus | null>(null);
  const privacyStatusRef = useRef<LibraryPrivacyStatus | null>(null);
  const [commandMediaLoading, setCommandMediaLoading] = useState(false);
  const restoreFocus = useRef<HTMLElement | null>(null);
  const view = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const jobMap = useAppStore((state) => state.jobs);
  const compareCount = useAppStore((state) => state.compareItemIds.length);
  const imageCompareCount = useAppStore((state) => state.imageCompareItemIds.length);
  const enqueuePlayNext = useAppStore((state) => state.enqueuePlayNext);
  const setNotice = useAppStore((state) => state.setNotice);

  useEffect(() => {
    const media = window.matchMedia("(max-width: 1120px)");
    const adapt = (event: MediaQueryListEvent) => { if (event.matches) setCollapsed(true); };
    media.addEventListener("change", adapt);
    return () => media.removeEventListener("change", adapt);
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const updatePrivacyStatus = (status: LibraryPrivacyStatus) => {
      if (disposed) return;
      const previous = privacyStatusRef.current;
      privacyStatusRef.current = status;
      setPrivacyStatus(status);
      if (previous?.configured && !previous.locked && status.locked) {
        setCommandMedia([]);
        window.location.reload();
      }
    };
    void api.getLibraryPrivacyStatus().then(updatePrivacyStatus).catch(() => undefined);
    void api.onLibraryPrivacyChanged(updatePrivacyStatus).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!privacyStatus?.configured || privacyStatus.locked) return;
    let lastPing = 0;
    const recordActivity = () => {
      const now = Date.now();
      if (now - lastPing < 30_000) return;
      lastPing = now;
      void api.touchLibraryPrivacy().catch(() => undefined);
    };
    const lockWhenHidden = () => {
      if (document.visibilityState === "hidden") {
        void api.lockLibraryPrivacy().then((status) => {
          privacyStatusRef.current = status;
          setPrivacyStatus(status);
        }).catch(() => undefined);
      }
    };
    window.addEventListener("pointerdown", recordActivity, { passive: true });
    window.addEventListener("keydown", recordActivity);
    document.addEventListener("visibilitychange", lockWhenHidden);
    recordActivity();
    return () => {
      window.removeEventListener("pointerdown", recordActivity);
      window.removeEventListener("keydown", recordActivity);
      document.removeEventListener("visibilitychange", lockWhenHidden);
    };
  }, [privacyStatus?.configured, privacyStatus?.locked]);
  const jobs = Object.values(jobMap);
  const activeCount = jobs.filter((job) =>
    ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state),
  ).length;
  const queuedCount = jobs.filter((job) => job.state === "Queued").length;
  const allNavigation = useMemo(() => allCommandNavigation(), []);
  const matchingNavigation = allNavigation.filter((item) => item.label.toLocaleLowerCase().includes(commandQuery.trim().toLocaleLowerCase()));
  const primaryView = primaryViewFor(view);
  const activeJobs = jobs.filter((job) => ["Analyzing", "Preparing", "Downloading", "PostProcessing", "Merging", "Verifying"].includes(job.state));
  const activeProgress = activeJobs.length > 0 ? activeJobs.reduce((total, job) => total + job.progressPercent, 0) / activeJobs.length : 0;

  useEffect(() => {
    let disposed = false;
    const query = commandQuery.trim();
    if (!commandOpen || query.length < 2) {
      setCommandMedia([]);
      setCommandMediaLoading(false);
      return;
    }
    setCommandMediaLoading(true);
    const timer = window.setTimeout(() => {
      void api.queryLibraryItems({ page: 1, pageSize: 8, search: query, sourceId: null, folderSourceId: null, folderPath: null, folderScope: "tree", mediaKind: null, resolution: null, container: null, duration: null, dateAdded: null, datePublished: null, availability: "available", tags: [], creators: [], series: [], sites: [], rating: null, collectionId: null, favoriteOnly: false, sort: "recent" })
        .then((result) => { if (!disposed) setCommandMedia(result.items); })
        .catch(() => { if (!disposed) setCommandMedia([]); })
        .finally(() => { if (!disposed) setCommandMediaLoading(false); });
    }, 180);
    return () => { disposed = true; window.clearTimeout(timer); };
  }, [commandOpen, commandQuery]);

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

  function openMediaFromCommand(itemId: string) {
    setView("library");
    setCommandOpen(false);
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:open-library-item", { detail: { itemId } })), 0);
  }

  function queueMediaFromCommand(item: LibraryItem) {
    enqueuePlayNext(item.id);
    setNotice({ message: `Queued “${item.title}” to play next.`, view: "library" });
    setCommandOpen(false);
  }

  function shareMediaFromCommand(itemId: string) {
    setView("sharing");
    setCommandOpen(false);
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:sharing-select", { detail: { itemId } })), 0);
  }

  function findSimilarFromCommand(itemId: string) {
    setView("similarity");
    setCommandOpen(false);
    window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:similarity-reference", { detail: { itemId } })), 0);
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
      <div className={`grid h-screen min-h-[680px] overflow-hidden text-ink transition-[grid-template-columns] duration-300 ${collapsed ? "grid-cols-[76px_minmax(0,1fr)]" : "grid-cols-[264px_minmax(0,1fr)]"}`}>
        <aside className="cinema-sidebar relative z-30 flex min-h-0 flex-col border-r border-line/45 backdrop-blur-xl">
          <div className={`flex h-[82px] items-center gap-3.5 ${collapsed ? "justify-center px-2" : "px-5"}`}>
            <div className="brand-mark relative grid size-11 shrink-0 place-items-center overflow-hidden rounded-[13px] bg-accent text-accentInk">
              <SquarePlay size={20} strokeWidth={2.5} />
              <span className="absolute inset-x-2 bottom-1.5 h-px bg-white/35" />
            </div>
            <div className={collapsed ? "hidden" : "min-w-0"}>
              <div className="truncate text-[16px] font-semibold tracking-[-0.035em]">Nice Downloader</div>
              <div className="mt-1 text-[11px] font-medium tracking-[0.02em] text-muted">Your private media room</div>
            </div>
          </div>

          <nav className="min-h-0 flex-1 overflow-y-auto px-3 pb-3 pt-1" aria-label="Primary navigation">
            <NavigationGroup label="Navigate" items={primaryNavigation} view={primaryView} collapsed={collapsed} setView={setView} badgeFor={badgeFor} />
          </nav>

          <div className="border-t border-line/45 bg-canvas/20 p-3">
            <button
              title={collapsed ? "Search media and commands" : undefined}
              onClick={() => { restoreFocus.current = document.activeElement as HTMLElement | null; setCommandQuery(""); setCommandOpen(true); }}
              className={`mb-1 flex h-9 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[12px] font-medium text-muted transition hover:bg-elevated/60 hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "justify-center" : ""}`}
              aria-label={collapsed ? "Search media and commands" : undefined}
            >
              <Search size={16} strokeWidth={1.8} />
              <span className={collapsed ? "hidden" : "flex-1"}>Search</span>
              {!collapsed ? <kbd className="text-[10px] font-medium text-muted">Ctrl K</kbd> : null}
            </button>
            <button
              title={collapsed ? "Commands and shortcuts" : undefined}
              onClick={() => { restoreFocus.current = document.activeElement as HTMLElement | null; setHelpOpen(true); }}
              className={`mb-1 flex h-9 w-full items-center gap-3 rounded-[8px] px-3 text-left text-[12px] font-medium text-muted transition hover:bg-elevated/60 hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "justify-center" : ""}`}
            >
              <Keyboard size={16} strokeWidth={1.8} />
              <span className={collapsed ? "hidden" : "flex-1"}>Shortcuts</span>
              {!collapsed ? <kbd className="text-[10px] font-medium text-muted">?</kbd> : null}
            </button>
            <div className={`mx-2 my-2.5 h-px bg-line/60 ${collapsed ? "mx-1" : ""}`} />
            <button className={`relative flex w-full items-center gap-3 rounded-[10px] px-2 py-2 text-left transition hover:bg-elevated/55 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "justify-center" : ""}`} onClick={() => setView("downloads")} aria-label={`${activeCount} active and ${queuedCount} queued downloads`} title={collapsed ? "Download status" : undefined}>
              <div className={`relative grid size-8 shrink-0 place-items-center rounded-[8px] ${activeCount > 0 ? "bg-accent/12 text-accent" : "bg-elevated text-muted"}`}>
                {activeCount > 0 ? <Activity size={15} /> : <Clock3 size={15} />}
                {activeCount > 0 ? <span className="absolute right-0.5 top-0.5 size-1.5 rounded-full bg-accent shadow-[0_0_0_3px_rgb(var(--panel))]" /> : null}
              </div>
              <div className={collapsed ? "hidden" : "min-w-0 flex-1"}>
                <div className="text-[11px] font-semibold text-ink">{activeCount > 0 ? `${activeCount} downloading` : "Media hub ready"}</div>
                <div className="mt-0.5 truncate text-[11px] text-muted">{queuedCount > 0 ? `${queuedCount} waiting in queue` : "Background queue is clear"}</div>
                {activeCount > 0 ? <span className="mt-2 block h-1 overflow-hidden rounded-full bg-elevated"><span className="block h-full bg-accent transition-[width]" style={{ width: `${activeProgress}%` }} /></span> : null}
              </div>
            </button>
            <div className={`mt-1 flex items-center px-2 ${collapsed ? "justify-center" : "justify-end"}`}>
              <button className={`grid size-8 shrink-0 place-items-center rounded-[7px] text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 ${collapsed ? "absolute bottom-[76px] right-[-16px] border border-line bg-panel shadow-control" : ""}`} onClick={toggleSidebar} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
                {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
              </button>
            </div>
          </div>
        </aside>

        <main id="main-content" tabIndex={-1} className="cinema-main relative min-h-0 min-w-0 overflow-y-auto bg-canvas/[0.88] outline-none">
          {children}
        </main>
      </div>
      {commandOpen ? <div className="fixed inset-0 z-[90] flex items-start justify-center bg-black/65 px-5 pt-[8vh] backdrop-blur-sm" onMouseDown={closeOverlay}><section className="panel-shadow w-full max-w-2xl overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="command-palette-title" onKeyDown={trapDialogFocus} onMouseDown={(event) => event.stopPropagation()}><h2 id="command-palette-title" className="sr-only">Search media and run Nice Downloader commands</h2><label className="relative block border-b border-line/70"><Search className="absolute left-4 top-4 text-muted" size={18} /><input autoFocus value={commandQuery} onChange={(event) => setCommandQuery(event.target.value)} className="h-14 w-full bg-transparent py-4 pl-12 pr-4 text-sm text-ink outline-none placeholder:text-muted" placeholder="Search media or jump to a workspace…" aria-controls="command-results" /></label><div id="command-results" className="max-h-[520px] overflow-y-auto p-2">{matchingNavigation.length ? <section><p className="px-3 pb-1 pt-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-muted">Workspaces and tools</p>{matchingNavigation.map((item) => { const Icon = item.icon; return <button key={item.id} className="flex w-full items-center gap-3 rounded-[9px] px-3 py-2.5 text-left text-sm text-muted transition hover:bg-elevated hover:text-ink focus-visible:bg-elevated focus-visible:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => navigateFromCommand(item.id)}><span className="grid size-8 place-items-center rounded-[7px] bg-elevated text-accent"><Icon size={16} /></span><span className="flex-1 font-medium">{item.label}</span>{view === item.id ? <span className="text-[10px] font-semibold uppercase tracking-[0.08em] text-accent">Current</span> : null}</button>; })}</section> : null}{commandQuery.trim().length >= 2 ? <section className="mt-2 border-t border-line/50 pt-2"><p className="flex items-center justify-between px-3 pb-1 pt-2 text-[9px] font-semibold uppercase tracking-[0.12em] text-muted"><span>Library media</span>{commandMediaLoading ? <LoaderCircle className="animate-spin" size={12} /> : null}</p>{commandMedia.map((item) => <div key={item.id} className="group flex items-center gap-2 rounded-[9px] px-2 py-1.5 hover:bg-elevated"><button className="flex min-w-0 flex-1 items-center gap-3 px-1 py-1.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => openMediaFromCommand(item.id)}><span className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-accent"><Film size={15} /></span><span className="min-w-0 flex-1"><strong className="block truncate text-xs font-medium text-ink">{item.title}</strong><span className="mt-0.5 block text-[9px] text-muted">{item.mediaKind === "image" ? "Image" : "Video"} · Open details</span></span></button>{item.mediaKind === "video" ? <button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => queueMediaFromCommand(item)} aria-label={`Queue ${item.title} to play next`} title="Play next"><ListOrdered size={14} /></button> : null}<button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => findSimilarFromCommand(item.id)} aria-label={`Find media similar to ${item.title}`} title="Find similar"><ScanSearch size={14} /></button><button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => shareMediaFromCommand(item.id)} aria-label={`Share ${item.title} temporarily`} title="Share temporarily"><Share2 size={14} /></button></div>)}{!commandMediaLoading && commandMedia.length === 0 ? <p className="px-4 py-5 text-center text-xs text-muted">No available media matched.</p> : null}</section> : null}{matchingNavigation.length === 0 && commandQuery.trim().length < 2 ? <p className="px-4 py-8 text-center text-sm text-muted">Type at least two characters to search your Library.</p> : null}</div><div className="flex items-center justify-between border-t border-line/60 px-4 py-2.5 text-[10px] text-muted"><span>Open details · queue · find similar · share</span><kbd>Esc closes</kbd></div></section></div> : null}
      {helpOpen ? <div className="fixed inset-0 z-[90] grid place-items-center bg-black/65 p-5 backdrop-blur-sm" onMouseDown={closeOverlay}><section className="panel-shadow w-full max-w-lg rounded-[14px] border border-line bg-panel p-5" role="dialog" aria-modal="true" aria-labelledby="shortcut-help-title" onKeyDown={trapDialogFocus} onMouseDown={(event) => event.stopPropagation()}><div className="flex items-start justify-between gap-4"><div><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Keyboard access</p><h2 id="shortcut-help-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Commands and shortcuts</h2><p className="mt-2 text-xs leading-5 text-muted">Navigate the app without reaching for the mouse. Tab and Shift+Tab move through every control.</p></div><button autoFocus className="grid size-8 place-items-center rounded-[7px] text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={closeOverlay} aria-label="Close shortcut help">×</button></div><dl className="mt-5 divide-y divide-line/60 border-y border-line/60">{[["Ctrl K", "Search media and commands"], ["Ctrl N", "Start a new download"], ["Ctrl ,", "Open Settings"], ["?", "Show this shortcut guide"], ["←  →", "Browse open Library details"], ["Esc", "Close the current panel or dialog"]].map(([keys, description]) => <div key={keys} className="flex items-center justify-between gap-5 py-3 text-xs"><dt className="text-muted">{description}</dt><dd><kbd className="rounded-[6px] border border-line bg-elevated px-2 py-1 font-semibold text-ink shadow-control">{keys}</kbd></dd></div>)}</dl><button className="mt-5 w-full rounded-[8px] bg-elevated px-3 py-2.5 text-xs font-semibold text-ink transition hover:brightness-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => { setHelpOpen(false); setCommandQuery(""); setCommandOpen(true); }}>Open command palette</button></section></div> : null}
    </>
  );
}

function NavigationGroup({ label, items, view, collapsed, setView, badgeFor }: { label: string; items: NavigationItem[]; view: ViewId; collapsed: boolean; setView: (view: ViewId) => void; badgeFor: (id: ViewId) => number }) {
  return (
    <section>
      <p className={`mb-2.5 px-3 text-[11px] font-medium tracking-[0.04em] text-muted/80 ${collapsed ? "sr-only" : ""}`}>{label}</p>
      <div className="grid gap-1">
        {items.map((item) => {
          const Icon = item.icon;
          const badge = badgeFor(item.id);
          const active = view === item.id;
          return (
            <button
              key={item.id}
              title={collapsed ? item.label : undefined}
              aria-label={collapsed ? item.label : undefined}
              onClick={() => setView(item.id)}
              aria-current={active ? "page" : undefined}
              className={`relative flex h-11 w-full items-center gap-3 rounded-[10px] px-3 text-left text-[14px] font-medium transition-all duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 active:scale-[0.985] ${active ? "bg-accent/[0.10] text-ink shadow-[inset_3px_0_0_rgb(var(--accent))] ring-1 ring-inset ring-accent/10" : "text-muted hover:bg-elevated/65 hover:text-ink"} ${collapsed ? "justify-center" : ""}`}
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
