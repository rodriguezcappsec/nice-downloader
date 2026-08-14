import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  ArchiveRestore,
  Ban,
  BellDot,
  Check,
  Copy,
  ChevronRight,
  FileImage,
  FileVideo,
  Film,
  Fingerprint,
  FolderOpen,
  FolderPlus,
  FolderSearch,
  FolderInput,
  HardDrive,
  Heart,
  ListPlus,
  LayoutGrid,
  List,
  Image as ImageIcon,
  Library,
  LoaderCircle,
  MonitorPlay,
  Play,
  Database,
  Eraser,
  Save,
  Search,
  Square,
  CheckSquare2,
  Star,
  Tags,
  Settings2,
  Trash2,
  Undo2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Button, Field, Input, Select } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import type {
  LibraryItem,
  LibraryBulkAction,
  LibraryChange,
  LibraryDuplicateAnalysisJob,
  LibraryDuplicateGroup,
  LibraryFileOperation,
  LibraryFileOperationKind,
  LibraryFileOperationPlan,
  LibraryFileOperationResult,
  LibraryCollection,
  LibraryMediaKind,
  LibraryScanJob,
  LibrarySource,
} from "../../types";

type MediaFilter = "all" | "video" | "image";
type ResolutionFilter = "all" | "2160" | "1440" | "1080" | "720" | "sd" | "unknown";
type DurationFilter = "all" | "short" | "medium" | "long" | "unknown";
type DateFilter = "all" | "week" | "month" | "year";
type AvailabilityFilter = "all" | "available" | "missing";
type FolderSelection = { sourceId: string; path: string };
type FolderOption = FolderSelection & { value: string; label: string; count: number };
const LIBRARY_PAGE_SIZE = 60;

export function LibraryPage() {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [sources, setSources] = useState<LibrarySource[]>([]);
  const [scanJobs, setScanJobs] = useState<LibraryScanJob[]>([]);
  const [collections, setCollections] = useState<LibraryCollection[]>([]);
  const [changes, setChanges] = useState<LibraryChange[]>([]);
  const [duplicateJob, setDuplicateJob] = useState<LibraryDuplicateAnalysisJob | null>(null);
  const [duplicateGroups, setDuplicateGroups] = useState<LibraryDuplicateGroup[]>([]);
  const [fileOperations, setFileOperations] = useState<LibraryFileOperation[]>([]);
  const [search, setSearch] = useState("");
  const [sourceFilter, setSourceFilter] = useState("all");
  const [folderFilter, setFolderFilter] = useState("all");
  const [mediaFilter, setMediaFilter] = useState<MediaFilter>("all");
  const [resolutionFilter, setResolutionFilter] = useState<ResolutionFilter>("all");
  const [containerFilter, setContainerFilter] = useState("all");
  const [durationFilter, setDurationFilter] = useState<DurationFilter>("all");
  const [dateFilter, setDateFilter] = useState<DateFilter>("all");
  const [availabilityFilter, setAvailabilityFilter] = useState<AvailabilityFilter>("all");
  const [tagFilter, setTagFilter] = useState("all");
  const [ratingFilter, setRatingFilter] = useState("all");
  const [collectionFilter, setCollectionFilter] = useState("all");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [viewMode, setViewMode] = useState<"grid" | "list">(() => window.localStorage.getItem("nice-downloader.library-view") === "list" ? "list" : "grid");
  const [selectedItem, setSelectedItem] = useState<LibraryItem | null>(null);
  const [manageOpen, setManageOpen] = useState(false);
  const [showSourceForm, setShowSourceForm] = useState(false);
  const [importTags, setImportTags] = useState("");
  const [busy, setBusy] = useState(false);
  const [resultPage, setResultPage] = useState(1);

  async function load() {
    try {
      const [nextItems, nextSources, nextJobs, nextCollections, nextChanges, nextDuplicateJob, nextDuplicateGroups, nextFileOperations] = await Promise.all([
        api.listLibraryItems(),
        api.listLibrarySources(),
        api.listLibraryScanJobs(),
        api.listLibraryCollections(),
        api.listLibraryChanges(),
        api.getLibraryDuplicateAnalysis(),
        api.listLibraryDuplicateGroups(),
        api.listLibraryFileOperations(),
      ]);
      setItems(nextItems);
      setSources(nextSources);
      setScanJobs(nextJobs);
      setCollections(nextCollections);
      setChanges(nextChanges);
      setDuplicateJob(nextDuplicateJob);
      setDuplicateGroups(nextDuplicateGroups);
      setFileOperations(nextFileOperations);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  useEffect(() => {
    void load();
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void api.onLibraryScanUpdated((job) => {
      setScanJobs((current) => upsertById(current, job));
      if (job.state !== "running") {
        void Promise.all([api.listLibrarySources(), api.listLibraryItems()]).then(([nextSources, nextItems]) => {
          if (!disposed) {
            setSources(nextSources);
            setItems(nextItems);
          }
        });
      }
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryItemIndexed((item) => {
      setItems((current) => upsertByPath(current, item));
      setSelectedItem((current) => current?.id === item.id || current?.path === item.path ? item : current);
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibrarySourceUpdated((source) => {
      setSources((current) => upsertById(current, source));
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryChangeUpdated((change) => {
      setChanges((current) => upsertById(current.filter((entry) => entry.itemId !== change.itemId), change));
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryDuplicateAnalysisUpdated((job) => {
      setDuplicateJob(job);
      if (job.state === "completed") {
        void api.listLibraryDuplicateGroups().then((groups) => { if (!disposed) setDuplicateGroups(groups); });
      }
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  const sourceNames = useMemo(() => new Map(sources.map((source) => [source.id, source.name])), [sources]);
  const containers = useMemo(() => Array.from(new Set(items.map((item) => item.container).filter((value): value is string => Boolean(value)))).sort(), [items]);
  const tags = useMemo(() => Array.from(new Set(items.flatMap((item) => item.tags))).sort((a, b) => a.localeCompare(b)), [items]);
  const folderOptions = useMemo(() => buildFolderOptions(items, sources, sourceFilter), [items, sourceFilter, sources]);
  const filtered = useMemo(() => {
    const query = search.trim().toLocaleLowerCase();
    const collectionIds = collectionFilter === "all" ? null : new Set(collections.find((collection) => collection.id === collectionFilter)?.itemIds || []);
    const selectedFolder = parseFolderFilter(folderFilter);
    return items.filter((item) => {
      if (sourceFilter !== "all" && item.sourceId !== sourceFilter) return false;
      if (selectedFolder && !itemMatchesFolder(item, selectedFolder)) return false;
      if (mediaFilter !== "all" && item.mediaKind !== mediaFilter) return false;
      if (containerFilter !== "all" && item.container !== containerFilter) return false;
      if (!matchesResolution(item.height, resolutionFilter)) return false;
      if (!matchesDuration(item.durationSeconds, durationFilter)) return false;
      if (!matchesDate(item.addedAt, dateFilter)) return false;
      if (availabilityFilter === "available" && item.missing) return false;
      if (availabilityFilter === "missing" && !item.missing) return false;
      if (tagFilter !== "all" && !item.tags.includes(tagFilter)) return false;
      if (!matchesRating(item.rating, ratingFilter)) return false;
      if (favoriteOnly && !item.favorite) return false;
      if (collectionIds && !collectionIds.has(item.id)) return false;
      return !query || `${item.title} ${item.site || ""} ${item.path} ${item.tags.join(" ")} ${item.container || ""} ${item.videoCodec || ""} ${item.audioCodec || ""}`.toLocaleLowerCase().includes(query);
    });
  }, [availabilityFilter, collectionFilter, collections, containerFilter, dateFilter, durationFilter, favoriteOnly, folderFilter, items, mediaFilter, ratingFilter, resolutionFilter, search, sourceFilter, tagFilter]);
  const featured = filtered.find((item) => !item.missing) || filtered[0];
  const videos = filtered.filter((item) => item.mediaKind !== "image");
  const images = filtered.filter((item) => item.mediaKind === "image");
  const resultPageCount = Math.max(1, Math.ceil(filtered.length / LIBRARY_PAGE_SIZE));
  const currentResultPage = Math.min(resultPage, resultPageCount);
  const resultStart = (currentResultPage - 1) * LIBRARY_PAGE_SIZE;
  const resultEnd = Math.min(resultStart + LIBRARY_PAGE_SIZE, filtered.length);
  const visibleFiltered = filtered.slice(resultStart, resultEnd);
  const isFiltered = Boolean(search.trim()) || sourceFilter !== "all" || folderFilter !== "all" || mediaFilter !== "all" || resolutionFilter !== "all" || containerFilter !== "all" || durationFilter !== "all" || dateFilter !== "all" || availabilityFilter !== "all" || tagFilter !== "all" || ratingFilter !== "all" || collectionFilter !== "all" || favoriteOnly;

  useEffect(() => {
    setResultPage(1);
  }, [availabilityFilter, collectionFilter, containerFilter, dateFilter, durationFilter, favoriteOnly, folderFilter, mediaFilter, ratingFilter, resolutionFilter, search, selectionMode, sourceFilter, tagFilter]);

  useEffect(() => {
    if (resultPage > resultPageCount) setResultPage(resultPageCount);
  }, [resultPage, resultPageCount]);

  useEffect(() => {
    if (folderFilter === "all") return;
    const selected = parseFolderFilter(folderFilter);
    if (!selected || !folderOptions.some((option) => option.value === folderFilter) || (sourceFilter !== "all" && selected.sourceId !== sourceFilter)) {
      setFolderFilter("all");
    }
  }, [folderFilter, folderOptions, sourceFilter]);

  useEffect(() => {
    if (!selectedItem) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      if (event.key === "Escape") {
        event.preventDefault();
        setSelectedItem(null);
        return;
      }
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      const index = filtered.findIndex((item) => item.id === selectedItem.id);
      if (index < 0) return;
      const next = event.key === "ArrowRight" ? index + 1 : index - 1;
      if (next >= 0 && next < filtered.length) {
        event.preventDefault();
        setSelectedItem(filtered[next]);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filtered, selectedItem]);

  function updateItem(changed: LibraryItem) {
    setItems((current) => current.map((item) => item.id === changed.id ? changed : item));
    setSelectedItem((current) => current?.id === changed.id ? changed : current);
  }

  function removeItem(id: string) {
    setItems((current) => current.filter((item) => item.id !== id));
    setSelectedItem((current) => current?.id === id ? null : current);
  }

  function updateItems(changed: LibraryItem[]) {
    const byId = new Map(changed.map((item) => [item.id, item]));
    setItems((current) => current.map((item) => byId.get(item.id) || item));
    setSelectedItem((current) => current ? byId.get(current.id) || current : null);
  }

  function applyFileOperationResult(result: LibraryFileOperationResult) {
    setFileOperations((current) => upsertById(current, result.operation));
    setItems((current) => {
      let next = current;
      if (result.operation.state === "undone" && result.operation.kind === "copy" && result.operation.outputItemId) {
        next = next.filter((item) => item.id !== result.operation.outputItemId);
      }
      const changedById = new Map(result.items.map((item) => [item.id, item]));
      next = next.map((item) => changedById.get(item.id) || item);
      for (const changed of result.items) {
        if (!next.some((item) => item.id === changed.id)) next = [changed, ...next];
      }
      return next;
    });
    setSelectedItem((current) => {
      if (!current) return current;
      if (result.operation.state === "undone" && result.operation.kind === "copy" && current.id === result.operation.outputItemId) return null;
      return result.items.find((item) => item.id === current.id) || current;
    });
    void api.listLibrarySources().then(setSources).catch((error) => setError(normalizeError(error)));
  }

  function toggleSelection(id: string) {
    if (!selectedIds.has(id) && selectedIds.size >= 500) {
      setNotice({ message: "Bulk actions are limited to 500 items at a time.", view: "library" });
      return;
    }
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  function closeSelectionMode() {
    setSelectionMode(false);
    setSelectedIds(new Set());
  }

  function resetFilters() {
    setMediaFilter("all"); setSourceFilter("all"); setFolderFilter("all"); setResolutionFilter("all"); setContainerFilter("all");
    setDurationFilter("all"); setDateFilter("all"); setAvailabilityFilter("all"); setTagFilter("all");
    setRatingFilter("all"); setCollectionFilter("all"); setFavoriteOnly(false);
  }

  function changeViewMode(mode: "grid" | "list") {
    setViewMode(mode);
    window.localStorage.setItem("nice-downloader.library-view", mode);
  }

  function changeResultPage(page: number) {
    const next = Math.min(Math.max(1, page), resultPageCount);
    setResultPage(next);
    window.requestAnimationFrame(() => {
      document.getElementById("library-results")?.scrollIntoView({
        behavior: document.documentElement.classList.contains("reduce-motion") ? "auto" : "smooth",
        block: "start",
      });
    });
  }

  async function applyBulkAction(action: LibraryBulkAction, message: string) {
    if (selectedIds.size === 0) return;
    setBusy(true);
    try {
      const changed = await api.applyLibraryBulkAction(Array.from(selectedIds), action);
      updateItems(changed);
      if (action.type === "addToCollection" || action.type === "removeFromCollection") {
        setCollections(await api.listLibraryCollections());
      }
      setNotice({ message, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function importFiles() {
    const paths = await open({ multiple: true, directory: false, title: "Add media files to library" });
    if (!paths) return;
    setBusy(true);
    try {
      await api.importLibraryFiles(paths, importTags.split(",").map((tag) => tag.trim()).filter(Boolean));
      await load();
      setNotice({ message: `Added ${paths.length} media file${paths.length === 1 ? "" : "s"}.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function reconcile() {
    setBusy(true);
    try {
      const result = await api.reconcileLibrary();
      await load();
      setNotice({ message: `Checked ${result.checkedCount} files; ${result.missingCount} missing.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function exportBackup() {
    const path = await save({ title: "Export library metadata", defaultPath: "nice-downloader-library.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      await api.exportLibrary(path);
      setNotice({ message: "Library metadata exported." });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function restoreBackup() {
    const path = await open({ multiple: false, directory: false, title: "Restore library metadata", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      await api.restoreLibrary(path);
      await load();
      setNotice({ message: "Library metadata restored.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function clearArtworkCache() {
    setBusy(true);
    try {
      const result = await api.clearLibraryArtworkCache();
      setNotice({ message: result.removedFiles > 0 ? `Cleared ${result.removedFiles} cached image${result.removedFiles === 1 ? "" : "s"} (${formatBytes(result.freedBytes)}).` : "The artwork cache is already empty.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  if (manageOpen) {
    return (
      <div className="mx-auto w-full max-w-[1440px] px-8 pb-14 pt-8 xl:px-10">
        <header className="mb-7 flex flex-wrap items-end justify-between gap-4">
          <div><p className="text-[11px] font-medium text-accent">Library settings</p><h1 className="mt-1 text-[30px] font-semibold tracking-[-0.04em]">Sources and catalog</h1><p className="mt-2 max-w-2xl text-[13px] leading-5 text-muted">Choose what Nice Downloader indexes. Removing a source here removes catalog records, never the original media.</p></div>
          <Button variant="secondary" onClick={() => setManageOpen(false)}><ChevronRight className="rotate-180" size={16} /> Back to library</Button>
        </header>

        <div className="mb-5 flex flex-wrap gap-2">
          <Button onClick={() => setShowSourceForm((visible) => !visible)}><FolderPlus size={16} /> Add folder</Button>
          <Button variant="secondary" onClick={() => void reconcile()}><FolderSearch size={16} /> Verify files</Button>
        </div>
        {showSourceForm ? <SourceForm onClose={() => setShowSourceForm(false)} onSaved={(source, job) => {
          setSources((current) => upsertById(current, source));
          setScanJobs((current) => upsertById(current, job));
          setShowSourceForm(false);
        }} /> : null}
        <SourceSection
          sources={sources}
          scanJobs={scanJobs}
          onJob={(job) => setScanJobs((current) => upsertById(current, job))}
          onRemoved={(id) => {
            setSources((current) => current.filter((source) => source.id !== id));
            setItems((current) => current.filter((item) => item.sourceId !== id));
            if (sourceFilter === id) setSourceFilter("all");
          }}
        />
        <CollectionSection collections={collections} onChanged={setCollections} />
        <ReviewSection changes={changes} items={items} sourceNames={sourceNames} onChanged={setChanges} />
        <DuplicateSection job={duplicateJob} groups={duplicateGroups} items={items} onJob={setDuplicateJob} onTrashed={(itemId) => {
          removeItem(itemId);
          void Promise.all([api.listLibraryDuplicateGroups(), api.listLibraryCollections(), api.listLibraryChanges()]).then(([groups, nextCollections, nextChanges]) => {
            setDuplicateGroups(groups);
            setCollections(nextCollections);
            setChanges(nextChanges);
          }).catch((error) => setError(normalizeError(error)));
        }} />
        <FileOperationsSection operations={fileOperations} onResult={applyFileOperationResult} onChanged={setFileOperations} />

        <section className="mt-8 grid gap-4 border-t border-line/60 pt-7 lg:grid-cols-3">
          <div className="bg-panel/55 p-5">
            <div className="flex items-start gap-3"><div className="grid size-10 place-items-center bg-elevated text-accent"><Library size={18} /></div><div><h2 className="text-sm font-semibold">Add individual files</h2><p className="mt-1 text-xs leading-5 text-muted">Useful for media that does not belong to a watched folder.</p></div></div>
            <Field label="Tags for this import"><Input className="mt-4" value={importTags} onChange={(event) => setImportTags(event.target.value)} placeholder="concert, family, reference" /></Field>
            <Button className="mt-4" variant="secondary" onClick={() => void importFiles()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Library size={15} />} Choose files</Button>
          </div>
          <div className="bg-panel/55 p-5">
            <div className="flex items-start gap-3"><div className="grid size-10 place-items-center bg-elevated text-accent"><Save size={18} /></div><div><h2 className="text-sm font-semibold">Catalog backup</h2><p className="mt-1 text-xs leading-5 text-muted">Export titles and tags or restore an existing Nice Downloader catalog.</p></div></div>
            <div className="mt-5 flex flex-wrap gap-2"><Button variant="secondary" onClick={() => void exportBackup()}><Save size={15} /> Export metadata</Button><Button variant="secondary" onClick={() => void restoreBackup()}><ArchiveRestore size={15} /> Restore backup</Button></div>
          </div>
          <div className="bg-panel/55 p-5">
            <div className="flex items-start gap-3"><div className="grid size-10 place-items-center bg-elevated text-accent"><Database size={18} strokeWidth={1.8} /></div><div><h2 className="text-sm font-semibold">Artwork cache</h2><p className="mt-1 text-xs leading-5 text-muted">Previews are versioned and stored in the app cache, never beside your media.</p></div></div>
            <Button className="mt-5" variant="secondary" disabled={busy} onClick={() => void clearArtworkCache()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Eraser size={15} strokeWidth={1.8} />} Clear cached artwork</Button>
          </div>
        </section>
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-[1520px] px-7 pb-16 pt-6 xl:px-9">
      <header className="sticky top-0 z-10 -mx-2 flex items-center gap-5 bg-canvas/90 px-2 py-3 backdrop-blur-xl">
        <div className="min-w-fit"><h1 className="text-[25px] font-semibold tracking-[-0.04em]">Your library</h1><p className="text-[11px] text-muted">{items.length} item{items.length === 1 ? "" : "s"} across {sources.length} source{sources.length === 1 ? "" : "s"}</p></div>
        <label className="relative mx-auto w-full max-w-2xl"><span className="sr-only">Search library</span><Search className="absolute left-4 top-3.5 text-muted" size={17} /><Input className="h-12 rounded-[12px] bg-panel/85 pl-11 pr-4 text-sm shadow-control" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search your videos, images, tags, and folders" /></label>
        <div className="flex shrink-0 items-center gap-1"><div className="flex rounded-[8px] bg-elevated/70 p-0.5" role="group" aria-label="Library view"><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "grid" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("grid")} aria-pressed={viewMode === "grid"} aria-label="Grid view"><LayoutGrid size={15} /></button><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "list" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("list")} aria-pressed={viewMode === "list"} aria-label="List view"><List size={15} /></button></div><Button variant={selectionMode ? "secondary" : "ghost"} onClick={() => selectionMode ? closeSelectionMode() : setSelectionMode(true)}>{selectionMode ? <X size={16} /> : <CheckSquare2 size={16} />} {selectionMode ? "Done" : "Select"}</Button><Button variant={changes.length > 0 ? "secondary" : "ghost"} onClick={() => { setManageOpen(true); void api.listLibraryFileOperations().then(setFileOperations).catch((error) => setError(normalizeError(error))); }}><Settings2 size={16} /> Manage{changes.length > 0 ? <span className="number-font rounded bg-accent/15 px-1.5 py-0.5 text-[10px] text-accent">{changes.length}</span> : null}</Button></div>
      </header>

      <nav className="mt-4 flex items-center gap-2 overflow-x-auto pb-2" aria-label="Library filters">
        <FilterChip active={!isFiltered} onClick={resetFilters}>Home</FilterChip>
        <FilterChip active={mediaFilter === "video"} onClick={() => setMediaFilter("video")}><Film size={14} /> Videos</FilterChip>
        <FilterChip active={mediaFilter === "image"} onClick={() => setMediaFilter("image")}><ImageIcon size={14} /> Images</FilterChip>
        <FilterChip active={favoriteOnly} onClick={() => setFavoriteOnly((value) => !value)}><Heart size={14} fill={favoriteOnly ? "currentColor" : "none"} /> Favorites</FilterChip>
        <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        {sources.map((source) => <FilterChip key={source.id} active={sourceFilter === source.id} onClick={() => { const next = sourceFilter === source.id ? "all" : source.id; setSourceFilter(next); const selectedFolder = parseFolderFilter(folderFilter); if (next === "all" || selectedFolder?.sourceId !== next) setFolderFilter("all"); }}><span className="grid size-5 place-items-center rounded-[5px] bg-accent/12 text-[10px] font-bold text-accent">{source.name.charAt(0).toLocaleUpperCase()}</span>{source.name}</FilterChip>)}
        {collections.map((collection) => <FilterChip key={collection.id} active={collectionFilter === collection.id} onClick={() => setCollectionFilter(collectionFilter === collection.id ? "all" : collection.id)}><ListPlus size={14} />{collection.name}</FilterChip>)}
        <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        <label className="relative shrink-0"><span className="sr-only">Filter by source subfolder</span><FolderOpen className="pointer-events-none absolute left-3 top-2.5 text-muted" size={14} strokeWidth={1.8} /><Select className="h-9 w-[220px] bg-elevated/70 pl-9 text-xs" value={folderFilter} onChange={(event) => setFolderFilter(event.target.value)} disabled={folderOptions.length === 0}><option value="all">{folderOptions.length === 0 ? "No indexed subfolders" : "Any subfolder"}</option>{folderOptions.map((option) => <option key={option.value} value={option.value}>{option.label} ({option.count})</option>)}</Select></label>
        <label className="relative shrink-0"><span className="sr-only">Filter by resolution</span><MonitorPlay className="pointer-events-none absolute left-3 top-2.5 text-muted" size={14} strokeWidth={1.8} /><Select className="h-9 w-[148px] bg-elevated/70 pl-9 text-xs" value={resolutionFilter} onChange={(event) => setResolutionFilter(event.target.value as ResolutionFilter)}><option value="all">Any resolution</option><option value="2160">4K and higher</option><option value="1440">1440p</option><option value="1080">1080p</option><option value="720">720p</option><option value="sd">Below 720p</option><option value="unknown">Not inspected</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by container</span><Select className="h-9 w-[130px] bg-elevated/70 text-xs" value={containerFilter} onChange={(event) => setContainerFilter(event.target.value)}><option value="all">Any format</option>{containers.map((container) => <option key={container} value={container}>{container.toLocaleUpperCase()}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by duration</span><Select className="h-9 w-[130px] bg-elevated/70 text-xs" value={durationFilter} onChange={(event) => setDurationFilter(event.target.value as DurationFilter)}><option value="all">Any duration</option><option value="short">Under 5 min</option><option value="medium">5–30 min</option><option value="long">Over 30 min</option><option value="unknown">Unknown time</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by date added</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={dateFilter} onChange={(event) => setDateFilter(event.target.value as DateFilter)}><option value="all">Any date</option><option value="week">This week</option><option value="month">This month</option><option value="year">This year</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by availability</span><Select className="h-9 w-[125px] bg-elevated/70 text-xs" value={availabilityFilter} onChange={(event) => setAvailabilityFilter(event.target.value as AvailabilityFilter)}><option value="all">Any status</option><option value="available">Available</option><option value="missing">Missing files</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by tag</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={tagFilter} onChange={(event) => setTagFilter(event.target.value)}><option value="all">Any tag</option>{tags.map((tag) => <option key={tag} value={tag}>#{tag}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by rating</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={ratingFilter} onChange={(event) => setRatingFilter(event.target.value)}><option value="all">Any rating</option><option value="rated">Rated</option><option value="5">5 stars</option><option value="4">4 stars</option><option value="3">3 stars</option><option value="unrated">Unrated</option></Select></label>
      </nav>

      {items.length === 0 ? (
        <EmptyLibrary onAdd={() => { setManageOpen(true); setShowSourceForm(true); }} />
      ) : isFiltered || selectionMode ? (
        <><div id="library-results" className="scroll-mt-24"><MediaSection title={selectionMode ? "Select media" : search.trim() ? `Results for “${search.trim()}”` : mediaFilter === "video" ? "Videos" : mediaFilter === "image" ? "Images" : sourceNames.get(sourceFilter) || "All media"} subtitle={filtered.length === 0 ? "No matching items" : selectionMode ? `${selectedIds.size} selected · showing ${resultStart + 1}–${resultEnd} of ${filtered.length}` : `Showing ${resultStart + 1}–${resultEnd} of ${filtered.length} matching item${filtered.length === 1 ? "" : "s"}`} items={visibleFiltered} sourceNames={sourceNames} onSelect={setSelectedItem} selectionMode={selectionMode} selectedIds={selectedIds} onToggleSelection={toggleSelection} viewMode={viewMode} /></div><LibraryPagination currentPage={currentResultPage} pageCount={resultPageCount} totalItems={filtered.length} onPageChange={changeResultPage} /></>
      ) : (
        <>
          {featured ? <FeaturedMedia item={featured} sourceName={sourceLabel(featured, sourceNames)} onSelect={() => setSelectedItem(featured)} /> : null}
          <MediaSection title="Recently added" subtitle="The newest media across your folders" items={filtered.slice(0, 12)} sourceNames={sourceNames} onSelect={setSelectedItem} viewMode={viewMode} />
          {videos.length > 0 ? <MediaSection title="Videos" subtitle={`${videos.length} video${videos.length === 1 ? "" : "s"} ready to watch`} items={videos.slice(0, 12)} sourceNames={sourceNames} onSelect={setSelectedItem} viewMode={viewMode} /> : null}
          {images.length > 0 ? <MediaSection title="Images" subtitle={`${images.length} image${images.length === 1 ? "" : "s"} in your catalog`} items={images.slice(0, 12)} sourceNames={sourceNames} onSelect={setSelectedItem} viewMode={viewMode} /> : null}
        </>
      )}

      {selectedItem ? <MediaDetails item={selectedItem} sourceName={sourceLabel(selectedItem, sourceNames)} onClose={() => setSelectedItem(null)} onChanged={updateItem} onRemoved={() => removeItem(selectedItem.id)} onOperation={applyFileOperationResult} /> : null}
      {selectionMode ? <BulkActionBar selectedIds={selectedIds} collections={collections} busy={busy} onApply={applyBulkAction} onClose={closeSelectionMode} /> : null}
    </div>
  );
}

function FilterChip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return <button onClick={onClick} className={`flex h-9 shrink-0 items-center gap-2 rounded-[8px] px-3.5 text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${active ? "bg-ink text-canvas" : "bg-elevated/70 text-muted hover:bg-elevated hover:text-ink"}`} aria-pressed={active}>{children}</button>;
}

function LibraryPagination({ currentPage, pageCount, totalItems, onPageChange }: { currentPage: number; pageCount: number; totalItems: number; onPageChange: (page: number) => void }) {
  if (pageCount <= 1 || totalItems === 0) return null;
  const pages = paginationPages(currentPage, pageCount);
  return <nav className="mt-9 flex flex-wrap items-center justify-between gap-4 border-t border-line/60 pt-5" aria-label="Library result pages"><p className="text-xs text-muted">Page <span className="number-font text-ink">{currentPage}</span> of <span className="number-font text-ink">{pageCount}</span> · {totalItems} items</p><p className="sr-only" role="status" aria-live="polite">Showing library result page {currentPage} of {pageCount}.</p><div className="flex items-center gap-1"><button className="flex h-9 items-center gap-1 rounded-[8px] px-3 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:cursor-not-allowed disabled:opacity-35" disabled={currentPage === 1} onClick={() => onPageChange(currentPage - 1)}><ChevronRight className="rotate-180" size={14} /> Previous</button>{pages.map((page, index) => page === "ellipsis" ? <span key={`ellipsis-${index}`} className="grid size-9 place-items-center text-xs text-muted" aria-hidden="true">…</span> : <button key={page} className={`number-font grid size-9 place-items-center rounded-[8px] text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${page === currentPage ? "bg-accent text-[#081426]" : "text-muted hover:bg-elevated hover:text-ink"}`} aria-current={page === currentPage ? "page" : undefined} aria-label={`Page ${page}`} onClick={() => onPageChange(page)}>{page}</button>)}<button className="flex h-9 items-center gap-1 rounded-[8px] px-3 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:cursor-not-allowed disabled:opacity-35" disabled={currentPage === pageCount} onClick={() => onPageChange(currentPage + 1)}>Next <ChevronRight size={14} /></button></div></nav>;
}

function paginationPages(currentPage: number, pageCount: number): Array<number | "ellipsis"> {
  if (pageCount <= 7) return Array.from({ length: pageCount }, (_, index) => index + 1);
  const visible = Array.from(new Set([1, pageCount, currentPage - 1, currentPage, currentPage + 1].filter((page) => page >= 1 && page <= pageCount))).sort((left, right) => left - right);
  const pages: Array<number | "ellipsis"> = [];
  visible.forEach((page, index) => {
    if (index > 0) {
      const previous = visible[index - 1];
      if (page - previous === 2) pages.push(previous + 1);
      else if (page - previous > 2) pages.push("ellipsis");
    }
    pages.push(page);
  });
  return pages;
}

function EmptyLibrary({ onAdd }: { onAdd: () => void }) {
  return <section className="relative mt-8 min-h-[460px] overflow-hidden bg-panel/55 px-8 py-14"><div className="absolute -right-24 -top-28 size-[32rem] rounded-full bg-accent/8 blur-3xl" /><div className="relative max-w-xl"><div className="mb-7 grid size-16 place-items-center rounded-[16px] bg-accent text-[#081426] shadow-[0_18px_50px_rgb(var(--accent)/0.2)]"><Play size={27} fill="currentColor" /></div><p className="text-xs font-semibold text-accent">Your private media home</p><h2 className="mt-3 text-[40px] font-semibold leading-[1.05] tracking-[-0.055em] text-balance">Bring every video and image into one library.</h2><p className="mt-5 max-w-lg text-[14px] leading-6 text-muted">Add folders from this computer or an external drive. Nice Downloader indexes them in place, so your originals stay exactly where they are.</p><Button className="mt-7 h-11" onClick={onAdd}><FolderPlus size={17} /> Add your first folder</Button></div></section>;
}

function FeaturedMedia({ item, sourceName, onSelect }: { item: LibraryItem; sourceName: string; onSelect: () => void }) {
  return <section className="relative mt-6 grid min-h-[330px] overflow-hidden bg-panel lg:grid-cols-[1.4fr_1fr]"><div className="absolute inset-0 opacity-20"><MediaArtwork item={item} eager /></div><div className="absolute inset-0 bg-gradient-to-r from-canvas via-canvas/95 to-canvas/25" /><div className="relative z-[1] flex flex-col justify-end px-7 py-9 lg:px-10"><p className="text-[11px] font-semibold text-accent">Recently added</p><h2 className="mt-3 max-w-2xl text-[34px] font-semibold leading-[1.08] tracking-[-0.045em] text-balance">{item.title}</h2><p className="mt-3 text-sm text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""} · {new Date(item.addedAt).toLocaleDateString()}</p><div className="mt-6 flex gap-2"><Button onClick={onSelect}><Play size={16} fill="currentColor" /> {item.mediaKind === "image" ? "View image" : "Watch"}</Button><Button variant="secondary" onClick={onSelect}>Details</Button></div></div><button className="group relative z-[1] m-6 hidden overflow-hidden rounded-[12px] bg-elevated shadow-[0_25px_70px_rgb(0_0_0/0.35)] lg:block" onClick={onSelect} aria-label={`Open ${item.title}`}><MediaArtwork item={item} eager /><span className="absolute inset-0 grid place-items-center bg-black/0 transition group-hover:bg-black/25"><span className="grid size-14 scale-90 place-items-center rounded-full bg-white/90 text-black opacity-0 shadow-xl transition group-hover:scale-100 group-hover:opacity-100"><Play size={22} fill="currentColor" /></span></span></button></section>;
}

function MediaSection({ title, subtitle, items, sourceNames, onSelect, selectionMode = false, selectedIds = new Set(), onToggleSelection, viewMode = "grid" }: { title: string; subtitle: string; items: LibraryItem[]; sourceNames: Map<string, string>; onSelect: (item: LibraryItem) => void; selectionMode?: boolean; selectedIds?: Set<string>; onToggleSelection?: (id: string) => void; viewMode?: "grid" | "list" }) {
  return <section className="mt-10" aria-labelledby={`library-${title.replace(/\W+/g, "-").toLocaleLowerCase()}`}><div className="mb-4 flex items-end justify-between gap-4"><div><h2 id={`library-${title.replace(/\W+/g, "-").toLocaleLowerCase()}`} className="text-[20px] font-semibold tracking-[-0.03em]">{title}</h2><p className="mt-1 text-[12px] text-muted">{subtitle}</p></div></div>{items.length > 0 ? <div className={viewMode === "grid" ? "grid gap-x-4 gap-y-7 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4" : "grid gap-2"}>{items.map((item) => <MediaCard key={item.id} item={item} sourceName={sourceLabel(item, sourceNames)} selected={selectedIds.has(item.id)} selectionMode={selectionMode} viewMode={viewMode} onSelect={() => selectionMode ? onToggleSelection?.(item.id) : onSelect(item)} />)}</div> : <div className="border border-dashed border-line/70 px-6 py-12 text-center text-sm text-muted">Nothing matches these filters.</div>}</section>;
}

function MediaCard({ item, sourceName, onSelect, selectionMode = false, selected = false, viewMode = "grid" }: { item: LibraryItem; sourceName: string; onSelect: () => void; selectionMode?: boolean; selected?: boolean; viewMode?: "grid" | "list" }) {
  const quality = resolutionLabel(item);
  const listView = viewMode === "list";
  return (
    <article className="group min-w-0">
      <button className={`${listView ? "grid grid-cols-[180px_minmax(0,1fr)] items-center gap-4 bg-panel/45 p-2 hover:bg-panel/70" : "block"} w-full text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/80 focus-visible:ring-offset-4 focus-visible:ring-offset-canvas ${selected && listView ? "ring-2 ring-accent ring-offset-2 ring-offset-canvas" : ""}`} onClick={onSelect} aria-pressed={selectionMode ? selected : undefined}>
        <div className={`relative aspect-video overflow-hidden rounded-[9px] bg-elevated shadow-[0_14px_35px_rgb(3_11_24/0.24)] transition duration-200 group-hover:-translate-y-0.5 group-hover:shadow-[0_20px_45px_rgb(3_11_24/0.36)] ${selected ? "ring-2 ring-accent ring-offset-2 ring-offset-canvas" : ""}`}>
          <MediaArtwork item={item} />
          <span className="absolute inset-0 grid place-items-center bg-black/0 transition group-hover:bg-black/20"><span className={`grid size-11 place-items-center rounded-full shadow-lg transition ${selectionMode ? "scale-100 bg-panel/95 text-ink opacity-100" : "scale-90 bg-white/90 text-black opacity-0 group-hover:scale-100 group-hover:opacity-100"}`}>{selectionMode ? selected ? <CheckSquare2 className="text-accent" size={21} /> : <Square size={21} /> : item.mediaKind === "image" ? <ImageIcon size={19} strokeWidth={1.8} /> : <Play size={18} strokeWidth={1.8} fill="currentColor" />}</span></span>
          <span className="number-font absolute bottom-2 right-2 rounded-[4px] bg-black/82 px-1.5 py-0.5 text-[10px] font-semibold text-white">{item.durationSeconds != null ? formatDuration(item.durationSeconds) : quality}</span>
          {item.durationSeconds != null && quality ? <span className="absolute bottom-2 left-2 rounded-[4px] bg-black/75 px-1.5 py-0.5 text-[9px] font-semibold text-white/90">{quality}</span> : null}
          {item.favorite ? <span className="absolute left-2 top-2 grid size-6 place-items-center rounded-[5px] bg-black/70 text-rose-300"><Heart size={13} fill="currentColor" /></span> : null}
          {item.metadataState === "pending" ? <span className="absolute right-2 top-2 grid size-6 place-items-center rounded-[5px] bg-black/70 text-white/80" title="Inspecting media"><LoaderCircle className="animate-spin" size={12} /></span> : null}
          {item.missing ? <span className="absolute left-2 top-2 bg-red-500/90 px-2 py-1 text-[9px] font-bold uppercase tracking-wider text-white">Missing</span> : null}
        </div>
        <div className={`${listView ? "mt-0" : "mt-3"} flex min-w-0 gap-3`}><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-[11px] font-bold text-accent">{sourceName.charAt(0).toLocaleUpperCase()}</div><div className="min-w-0"><h3 className={`${listView ? "truncate text-sm" : "line-clamp-2 text-[13px]"} font-semibold leading-[1.35] text-ink`}>{item.title}</h3><p className="mt-1 truncate text-[11px] text-muted">{sourceName}{item.rating ? ` · ${item.rating} ★` : ""}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{listView && item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}{listView ? ` · ${new Date(item.addedAt).toLocaleDateString()}` : ""}</p>{listView && item.tags.length > 0 ? <p className="mt-2 truncate text-[10px] text-muted/80">{item.tags.map((tag) => `#${tag}`).join("  ")}</p> : null}</div></div>
      </button>
    </article>
  );
}

function MediaArtwork({ item, eager = false }: { item: LibraryItem; eager?: boolean }) {
  const inlineArtwork = item.thumbnail?.startsWith("data:") ? item.thumbnail : null;
  const [artwork, setArtwork] = useState<string | null>(inlineArtwork);
  useEffect(() => {
    setArtwork(inlineArtwork);
    if (inlineArtwork || item.missing) return;
    let disposed = false;
    void api.getLibraryArtwork(item.id).then((value) => { if (!disposed) setArtwork(value); }).catch(() => {});
    return () => { disposed = true; };
  }, [inlineArtwork, item.id, item.missing]);
  if (artwork) return <img className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.025]" src={artwork} alt={`${item.title} artwork`} loading={eager ? "eager" : "lazy"} />;
  const variant = item.id.charCodeAt(0) % 3;
  const background = variant === 0 ? "from-[#1b2b45] via-[#172235] to-[#101419]" : variant === 1 ? "from-[#33291d] via-[#26231d] to-[#111417]" : "from-[#202b3a] via-[#1b252f] to-[#101419]";
  return <div className={`relative grid h-full w-full place-items-center bg-gradient-to-br ${background}`}><div className="absolute inset-0 opacity-[0.08] [background-image:radial-gradient(circle_at_center,white_1px,transparent_1px)] [background-size:18px_18px]" />{item.mediaKind === "image" ? <FileImage className="relative text-white/35" size={34} /> : <FileVideo className="relative text-white/35" size={34} />}</div>;
}

function MediaDetails({ item, sourceName, onClose, onChanged, onRemoved, onOperation }: { item: LibraryItem; sourceName: string; onClose: () => void; onChanged: (item: LibraryItem) => void; onRemoved: () => void; onOperation: (result: LibraryFileOperationResult) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [editing, setEditing] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [tags, setTags] = useState(item.tags.join(", "));
  const [organizeOpen, setOrganizeOpen] = useState(false);
  const [operationKind, setOperationKind] = useState<LibraryFileOperationKind>("rename");
  const [destinationDirectory, setDestinationDirectory] = useState("");
  const [newFileName, setNewFileName] = useState(fileNameFromPath(item.path));
  const [operationPlan, setOperationPlan] = useState<LibraryFileOperationPlan | null>(null);
  const [operationBusy, setOperationBusy] = useState(false);

  useEffect(() => {
    setTitle(item.title);
    setTags(item.tags.join(", "));
    setEditing(false);
    setConfirmRemove(false);
    setOrganizeOpen(false);
    setOperationKind("rename");
    setDestinationDirectory("");
    setNewFileName(fileNameFromPath(item.path));
    setOperationPlan(null);
  }, [item.id, item.path]);

  async function openItem() {
    try { await api.openLibraryItem(item.id); } catch (error) { setError(normalizeError(error)); }
  }
  async function update() {
    try {
      const changed = await api.updateLibraryItem({ id: item.id, title, tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean), favorite: item.favorite, rating: item.rating });
      onChanged(changed);
      setEditing(false);
    } catch (error) { setError(normalizeError(error)); }
  }
  async function remove() {
    if (!confirmRemove) { setConfirmRemove(true); return; }
    try { await api.removeLibraryItem(item.id); onRemoved(); } catch (error) { setError(normalizeError(error)); }
  }
  async function updateCuration(favorite: boolean, rating: number | null | undefined) {
    try {
      const changed = await api.updateLibraryItem({ id: item.id, title: item.title, tags: item.tags, favorite, rating });
      onChanged(changed);
    } catch (error) { setError(normalizeError(error)); }
  }

  async function chooseOperationFolder() {
    const selected = await open({ multiple: false, directory: true, title: operationKind === "copy" ? "Choose copy destination" : "Choose move destination" });
    if (selected) {
      setDestinationDirectory(selected);
      setOperationPlan(null);
    }
  }
  async function previewOperation() {
    setOperationBusy(true);
    try {
      setOperationPlan(await api.previewLibraryFileOperation({
        itemId: item.id,
        kind: operationKind,
        destinationDirectory: operationKind === "rename" ? null : destinationDirectory,
        newFileName,
      }));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setOperationBusy(false);
    }
  }
  async function executeOperation() {
    if (!operationPlan) return;
    setOperationBusy(true);
    try {
      const result = await api.executeLibraryFileOperation(operationPlan.operation.id);
      onOperation(result);
      setOperationPlan(null);
      setOrganizeOpen(false);
      setNotice({
        message: operationKind === "copy" ? "Media copied without changing the original." : operationKind === "move" ? "Media moved and the catalog was updated." : "Media file renamed.",
        view: "library",
      });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setOperationBusy(false);
    }
  }

  return <aside className="fixed bottom-0 right-0 top-0 z-50 w-[min(500px,100vw)] overflow-y-auto border-l border-line/70 bg-panel/98 shadow-[-28px_0_80px_rgb(0_0_0/0.32)] backdrop-blur-xl" role="dialog" aria-modal="true" aria-label={`${item.title} details`}><div className="sticky top-0 z-[1] flex items-center justify-between border-b border-line/60 bg-panel/90 px-5 py-4 backdrop-blur-xl"><div><p className="text-xs font-semibold">Media details</p><p className="mt-0.5 text-[10px] text-muted">Use ← → to browse · Esc to close</p></div><button className="grid size-8 place-items-center rounded-[7px] text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onClose} aria-label="Close media details"><X size={17} /></button></div><div className="p-5"><div className="aspect-video overflow-hidden rounded-[12px] bg-elevated"><MediaArtwork item={item} eager /></div>{editing ? <div className="mt-5 space-y-3"><Field label="Title"><Input value={title} onChange={(event) => setTitle(event.target.value)} /></Field><Field label="Tags"><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="Comma-separated tags" /></Field><div className="flex gap-2"><Button onClick={() => void update()}>Save changes</Button><Button variant="secondary" onClick={() => setEditing(false)}>Cancel</Button></div></div> : <><h2 className="mt-5 text-[22px] font-semibold leading-[1.2] tracking-[-0.035em] text-balance">{item.title}</h2><p className="mt-2 text-xs text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</p><div className="mt-5 flex gap-2"><Button className="flex-1" disabled={item.missing} onClick={() => void openItem()}><Play size={16} fill="currentColor" /> {item.mediaKind === "image" ? "Open image" : "Open in player"}</Button><Button variant="secondary" onClick={() => setEditing(true)}>Edit</Button></div></>}
      <div className="mt-4 flex items-center gap-2"><Button variant="secondary" className={item.favorite ? "text-rose-300" : ""} onClick={() => void updateCuration(!item.favorite, item.rating)}><Heart size={15} fill={item.favorite ? "currentColor" : "none"} /> {item.favorite ? "Favorited" : "Favorite"}</Button><label className="flex flex-1 items-center gap-2 text-xs text-muted"><Star size={15} /><span className="sr-only">Rating</span><Select className="h-10" value={item.rating ?? ""} onChange={(event) => void updateCuration(item.favorite, event.target.value ? Number(event.target.value) : null)}><option value="">Not rated</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}</Select></label></div>
      <dl className="mt-7 grid grid-cols-[110px_1fr] gap-x-4 gap-y-3 border-t border-line/60 pt-5 text-xs"><dt className="text-muted">Type</dt><dd>{item.mediaKind === "image" ? "Image" : "Video"}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}</dd>{item.width && item.height ? <><dt className="text-muted">Dimensions</dt><dd className="number-font">{item.width} × {item.height}{item.fps ? ` · ${formatFps(item.fps)} fps` : ""}{item.dynamicRange ? ` · ${item.dynamicRange}` : ""}</dd></> : null}{item.videoCodec || item.audioCodec ? <><dt className="text-muted">Codecs</dt><dd>{[formatCodec(item.videoCodec), formatCodec(item.audioCodec)].filter(Boolean).join(" + ")}</dd></> : null}{item.fileSizeBytes != null ? <><dt className="text-muted">File size</dt><dd className="number-font">{formatBytes(item.fileSizeBytes)}</dd></> : null}<dt className="text-muted">Added</dt><dd>{new Date(item.addedAt).toLocaleString()}</dd>{item.modifiedAtMs ? <><dt className="text-muted">Modified</dt><dd>{new Date(item.modifiedAtMs).toLocaleString()}</dd></> : null}<dt className="text-muted">Location</dt><dd className="break-all text-muted" title={item.path}>{item.relativePath || item.path}</dd>{item.site ? <><dt className="text-muted">Origin</dt><dd>{item.site}</dd></> : null}</dl>
      {item.metadataState === "pending" ? <p className="mt-5 flex items-center gap-2 text-[11px] text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={13} /> Inspecting resolution, codecs, and duration…</p> : null}
      {item.metadataError ? <details className="mt-5 border-l-2 border-amber-400/50 pl-3 text-[11px] text-muted"><summary className="cursor-pointer font-medium text-amber-300">Media inspection unavailable</summary><p className="mt-2 leading-5">{item.metadataError}</p></details> : null}
      {item.tags.length > 0 ? <div className="mt-6 flex flex-wrap gap-1.5">{item.tags.map((tag) => <span key={tag} className="rounded-[6px] bg-elevated px-2 py-1 text-[10px] text-muted">#{tag}</span>)}</div> : null}
      <section className="mt-7 border-t border-line/60 pt-5" aria-labelledby="organize-media-title"><button className="flex w-full items-center justify-between text-left" onClick={() => { setOrganizeOpen((value) => !value); setOperationPlan(null); }} aria-expanded={organizeOpen}><span><span id="organize-media-title" className="block text-sm font-semibold">Organize file</span><span className="mt-1 block text-[11px] text-muted">Preview every rename, move, or copy before it changes disk.</span></span><FolderInput className="text-accent" size={18} /></button>{organizeOpen ? <div className="mt-4 space-y-3"><Field label="Action"><Select value={operationKind} onChange={(event) => { setOperationKind(event.target.value as LibraryFileOperationKind); setOperationPlan(null); }}><option value="rename">Rename in this folder</option><option value="move">Move to another folder</option><option value="copy">Copy to another folder</option></Select></Field>{operationKind !== "rename" ? <Field label="Destination folder"><div className="flex gap-2"><Input value={destinationDirectory} readOnly placeholder="Choose a folder" /><Button variant="secondary" className="shrink-0 px-3" onClick={() => void chooseOperationFolder()}><FolderOpen size={14} /> Browse</Button></div></Field> : null}<Field label="Filename"><Input value={newFileName} onChange={(event) => { setNewFileName(event.target.value); setOperationPlan(null); }} /></Field>{operationPlan ? <div className="border border-accent/25 bg-accent/5 p-3"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Ready to confirm</p><div className="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-[11px]"><span className="min-w-0 break-all text-muted">{operationPlan.sourceDisplayName}</span><ArrowRight className="shrink-0 text-accent" size={14} /><span className="min-w-0 break-all text-ink">{operationPlan.destinationDisplayName}</span></div><ul className="mt-3 space-y-1 border-t border-line/60 pt-3 text-[10px] leading-4 text-muted">{operationPlan.warnings.map((warning) => <li key={warning}>• {warning}</li>)}</ul></div> : null}<div className="flex gap-2">{operationPlan ? <Button variant="secondary" disabled={operationBusy} onClick={() => setOperationPlan(null)}>Change plan</Button> : <Button variant="secondary" disabled={operationBusy || !newFileName.trim() || (operationKind !== "rename" && !destinationDirectory)} onClick={() => void previewOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : null} Preview</Button>}{operationPlan ? <Button disabled={operationBusy} onClick={() => void executeOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : <Check size={14} />} Confirm {operationKind}</Button> : null}</div></div> : null}</section>
      <div className="mt-8 border-t border-line/60 pt-5"><p className="text-[11px] leading-5 text-muted">Removing this record does not delete the original file.</p><div className="mt-3 flex items-center gap-2">{confirmRemove ? <Button variant="secondary" className="h-8 text-xs" onClick={() => setConfirmRemove(false)}>Keep record</Button> : null}<Button variant={confirmRemove ? "danger" : "ghost"} className="h-8 text-xs" onClick={() => void remove()}><Trash2 size={13} /> {confirmRemove ? "Confirm removal" : "Remove record"}</Button></div></div></div></aside>;
}

function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

function matchesResolution(height: number | null | undefined, filter: ResolutionFilter): boolean {
  if (filter === "all") return true;
  if (filter === "unknown") return height == null;
  if (height == null) return false;
  if (filter === "2160") return height >= 2160;
  if (filter === "1440") return height >= 1440 && height < 2160;
  if (filter === "1080") return height >= 1080 && height < 1440;
  if (filter === "720") return height >= 720 && height < 1080;
  return height < 720;
}

function buildFolderOptions(items: LibraryItem[], sources: LibrarySource[], sourceFilter: string): FolderOption[] {
  const sourceNames = new Map(sources.map((source) => [source.id, source.name]));
  const counts = new Map<string, FolderOption>();
  for (const item of items) {
    if (!item.sourceId || !item.relativePath || (sourceFilter !== "all" && item.sourceId !== sourceFilter)) continue;
    const segments = item.relativePath.replace(/\\/g, "/").split("/").filter(Boolean);
    if (segments.length < 2) continue;
    for (let depth = 1; depth < segments.length; depth += 1) {
      const path = segments.slice(0, depth).join("/");
      const value = folderFilterValue(item.sourceId, path);
      const existing = counts.get(value);
      if (existing) existing.count += 1;
      else counts.set(value, {
        sourceId: item.sourceId,
        path,
        value,
        label: sourceFilter === "all" ? `${sourceNames.get(item.sourceId) || "Source"} / ${path}` : path,
        count: 1,
      });
    }
  }
  return Array.from(counts.values()).sort((left, right) => left.label.localeCompare(right.label, undefined, { numeric: true }));
}

function folderFilterValue(sourceId: string, path: string): string {
  return `folder:${encodeURIComponent(sourceId)}:${encodeURIComponent(path)}`;
}

function parseFolderFilter(value: string): FolderSelection | null {
  if (!value.startsWith("folder:")) return null;
  const encoded = value.slice(7);
  const separator = encoded.indexOf(":");
  if (separator < 1) return null;
  try {
    return {
      sourceId: decodeURIComponent(encoded.slice(0, separator)),
      path: decodeURIComponent(encoded.slice(separator + 1)),
    };
  } catch {
    return null;
  }
}

function itemMatchesFolder(item: LibraryItem, folder: FolderSelection): boolean {
  if (item.sourceId !== folder.sourceId || !item.relativePath) return false;
  const normalized = item.relativePath.replace(/\\/g, "/");
  const separator = normalized.lastIndexOf("/");
  if (separator < 0) return false;
  const directory = normalized.slice(0, separator);
  return directory === folder.path || directory.startsWith(`${folder.path}/`);
}

function matchesDuration(duration: number | null | undefined, filter: DurationFilter): boolean {
  if (filter === "all") return true;
  if (filter === "unknown") return duration == null;
  if (duration == null) return false;
  if (filter === "short") return duration < 5 * 60;
  if (filter === "medium") return duration >= 5 * 60 && duration <= 30 * 60;
  return duration > 30 * 60;
}

function matchesDate(addedAt: string, filter: DateFilter): boolean {
  if (filter === "all") return true;
  const age = Date.now() - new Date(addedAt).getTime();
  const day = 24 * 60 * 60 * 1000;
  if (!Number.isFinite(age)) return false;
  if (filter === "week") return age <= 7 * day;
  if (filter === "month") return age <= 31 * day;
  return age <= 366 * day;
}

function matchesRating(rating: number | null | undefined, filter: string): boolean {
  if (filter === "all") return true;
  if (filter === "rated") return rating != null;
  if (filter === "unrated") return rating == null;
  return rating === Number(filter);
}

function resolutionLabel(item: LibraryItem): string {
  const height = item.height;
  if (height == null) return item.metadataState === "pending" ? "Inspecting" : "Unknown";
  if (height >= 2160) return "4K";
  if (height >= 1440) return "1440p";
  if (height >= 1080) return "1080p";
  if (height >= 720) return "720p";
  return `${height}p`;
}

function formatCodec(codec: string | null | undefined): string | null {
  if (!codec) return null;
  const labels: Record<string, string> = { h264: "H.264", hevc: "HEVC", av1: "AV1", vp9: "VP9", aac: "AAC", opus: "Opus", mp3: "MP3", vorbis: "Vorbis" };
  return labels[codec.toLocaleLowerCase()] || codec.toLocaleUpperCase();
}

function formatFps(fps: number): string {
  return Number.isInteger(fps) ? fps.toFixed(0) : fps.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

function sourceLabel(item: LibraryItem, sourceNames: Map<string, string>): string {
  return item.sourceId ? sourceNames.get(item.sourceId) || item.site || "Local media" : item.site || "Local media";
}

function BulkActionBar({ selectedIds, collections, busy, onApply, onClose }: { selectedIds: Set<string>; collections: LibraryCollection[]; busy: boolean; onApply: (action: LibraryBulkAction, message: string) => Promise<void>; onClose: () => void }) {
  const [tag, setTag] = useState("");
  const disabled = busy || selectedIds.size === 0;
  return <aside className="panel-shadow fixed bottom-6 left-1/2 z-40 flex w-[min(1080px,calc(100vw-320px))] -translate-x-1/2 flex-wrap items-center gap-2 border border-line/80 bg-panel/95 p-3 backdrop-blur-xl" aria-label="Bulk library actions"><div className="mr-2 min-w-24"><p className="number-font text-sm font-semibold">{selectedIds.size} selected</p><button className="mt-0.5 text-[10px] font-medium text-muted hover:text-ink" onClick={onClose}>Clear selection</button></div><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: true }, `Marked ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} as favorites.`)}><Heart size={14} /> Favorite</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: false }, `Removed ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} from favorites.`)}><Heart size={14} fill="none" /> Unfavorite</Button><label><span className="sr-only">Set rating</span><Select className="h-9 w-32 text-xs" disabled={disabled} defaultValue="" onChange={(event) => { const value = event.target.value; if (value !== "") void onApply({ type: "setRating", rating: value === "clear" ? null : Number(value) }, value === "clear" ? "Cleared selected ratings." : `Rated selected media ${value} stars.`); event.currentTarget.value = ""; }}><option value="" disabled>Set rating</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}<option value="clear">Clear rating</option></Select></label><label><span className="sr-only">Add to collection</span><Select className="h-9 w-44 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "addToCollection", collectionId: event.target.value }, "Added selected media to the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Add to collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><label><span className="sr-only">Remove from collection</span><Select className="h-9 w-48 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "removeFromCollection", collectionId: event.target.value }, "Removed selected media from the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Remove from collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><div className="flex min-w-48 flex-1"><Input className="h-9 rounded-r-none text-xs" value={tag} onChange={(event) => setTag(event.target.value)} placeholder="Add a tag" /><Button variant="secondary" className="h-9 rounded-l-none px-3 text-xs" disabled={disabled || !tag.trim()} onClick={() => { const value = tag.trim(); if (!value) return; void onApply({ type: "addTags", tags: [value] }, `Added #${value} to selected media.`); setTag(""); }}><Tags size={14} /> Add</Button></div></aside>;
}

function ReviewSection({ changes, items, sourceNames, onChanged }: { changes: LibraryChange[]; items: LibraryItem[]; sourceNames: Map<string, string>; onChanged: (changes: LibraryChange[]) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [busyIds, setBusyIds] = useState<Set<string>>(() => new Set());
  const itemsById = useMemo(() => new Map(items.map((item) => [item.id, item])), [items]);

  async function review(ids: string[]) {
    if (ids.length === 0) return;
    setBusyIds((current) => new Set([...current, ...ids]));
    try {
      const reviewed = await api.reviewLibraryChanges(ids);
      const reviewedIds = new Set(ids);
      onChanged(changes.filter((change) => !reviewedIds.has(change.id)));
      setNotice({ message: `Reviewed ${reviewed} library change${reviewed === 1 ? "" : "s"}.`, view: "library" });
    } catch (error) { setError(normalizeError(error)); } finally {
      setBusyIds((current) => {
        const next = new Set(current);
        ids.forEach((id) => next.delete(id));
        return next;
      });
    }
  }

  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="library-review-title"><div className="mb-4 flex flex-wrap items-end justify-between gap-3"><div><div className="flex items-center gap-2"><BellDot className="text-accent" size={18} /><h2 id="library-review-title" className="text-lg font-semibold tracking-[-0.025em]">Library changes</h2>{changes.length > 0 ? <span className="number-font rounded bg-accent/12 px-2 py-0.5 text-[10px] font-semibold text-accent">{changes.length}</span> : null}</div><p className="mt-1 text-xs text-muted">Review changes found by scans. This list never changes or removes the original files.</p></div>{changes.length > 0 ? <Button variant="secondary" className="h-9 text-xs" disabled={busyIds.size > 0} onClick={() => void review(changes.map((change) => change.id))}><Check size={14} /> Mark all reviewed</Button> : null}</div>{changes.length === 0 ? <div className="border border-dashed border-line/70 bg-panel/25 px-5 py-7 text-center text-xs text-muted">No unreviewed library changes.</div> : <div className="divide-y divide-line/60 border-y border-line/60">{changes.map((change) => { const item = itemsById.get(change.itemId); const label = libraryChangeLabel(change.kind); const sourceName = change.sourceId ? sourceNames.get(change.sourceId) : null; return <article key={change.id} className="flex items-center gap-4 px-2 py-3.5"><div className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${change.kind === "missing" ? "bg-red-400/10 text-red-300" : change.kind === "restored" ? "bg-emerald-400/10 text-emerald-300" : "bg-accent/10 text-accent"}`}><BellDot size={16} /></div><div className="min-w-0 flex-1"><div className="flex items-center gap-2"><span className="text-xs font-semibold text-ink">{label}</span><span className="text-[10px] text-muted">{new Date(change.detectedAt).toLocaleString()}</span></div><p className="mt-1 truncate text-[12px] text-ink">{item?.title || "Media record"}</p><p className="mt-0.5 truncate text-[10px] text-muted" title={item?.path || change.previousPath || undefined}>{sourceName ? `${sourceName} · ` : ""}{change.kind === "renamed" && change.previousPath ? `${change.previousPath} → ` : ""}{item?.relativePath || item?.path || change.previousPath || "Location unavailable"}</p></div><Button variant="ghost" className="h-8 shrink-0 px-2 text-xs" disabled={busyIds.has(change.id)} onClick={() => void review([change.id])}>{busyIds.has(change.id) ? <LoaderCircle className="animate-spin" size={13} /> : <Check size={13} />} Reviewed</Button></article>; })}</div>}</section>;
}

function libraryChangeLabel(kind: LibraryChange["kind"]): string {
  const labels: Record<LibraryChange["kind"], string> = {
    added: "New media",
    modified: "File changed",
    renamed: "File renamed",
    missing: "File missing",
    restored: "File restored",
  };
  return labels[kind];
}

function DuplicateSection({ job, groups, items, onJob, onTrashed }: { job: LibraryDuplicateAnalysisJob | null; groups: LibraryDuplicateGroup[]; items: LibraryItem[]; onJob: (job: LibraryDuplicateAnalysisJob) => void; onTrashed: (itemId: string) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [confirmTrash, setConfirmTrash] = useState<string | null>(null);
  const [trashBusy, setTrashBusy] = useState<string | null>(null);
  const running = job?.state === "running";
  const itemsById = useMemo(() => new Map(items.map((item) => [item.id, item])), [items]);
  const progress = job && job.candidateItems > 0 ? Math.min(100, (job.hashedItems + job.skippedItems) / job.candidateItems * 100) : 0;

  async function start() {
    try { onJob(await api.startLibraryDuplicateAnalysis()); } catch (error) { setError(normalizeError(error)); }
  }
  async function cancel() {
    if (!job) return;
    try { await api.cancelLibraryDuplicateAnalysis(job.id); } catch (error) { setError(normalizeError(error)); }
  }
  async function openItem(id: string) {
    try { await api.openLibraryItem(id); } catch (error) { setError(normalizeError(error)); }
  }
  async function moveToTrash(groupId: string, itemId: string) {
    if (confirmTrash !== itemId) {
      setConfirmTrash(itemId);
      return;
    }
    setTrashBusy(itemId);
    try {
      const result = await api.trashLibraryDuplicateItem(groupId, itemId);
      onTrashed(result.itemId);
      setConfirmTrash(null);
      setNotice({ message: `Moved one verified duplicate (${formatBytes(result.movedBytes)}) to the system Trash. It can be restored from your desktop Trash.`, view: "library" });
    } catch (error) { setError(normalizeError(error)); } finally { setTrashBusy(null); }
  }

  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="library-duplicates-title"><div className="mb-4 flex flex-wrap items-end justify-between gap-3"><div><div className="flex items-center gap-2"><Fingerprint className="text-accent" size={18} /><h2 id="library-duplicates-title" className="text-lg font-semibold tracking-[-0.025em]">Duplicate intelligence</h2></div><p className="mt-1 max-w-2xl text-xs leading-5 text-muted">Size identifies candidates; streaming SHA-256 verifies byte-for-byte matches. Cleanup is always an explicit, confirmed move to the system Trash.</p></div>{running ? <Button variant="secondary" className="h-9 text-xs" onClick={() => void cancel()}><Ban size={14} /> Cancel analysis</Button> : <Button variant="secondary" className="h-9 text-xs" onClick={() => void start()}><Fingerprint size={14} /> {groups.length > 0 ? "Analyze again" : "Analyze duplicates"}</Button>}</div>{running && job ? <div className="bg-panel/55 p-4" role="status" aria-live="polite"><div className="flex items-center justify-between gap-4 text-xs"><span className="flex min-w-0 items-center gap-2 text-ink"><LoaderCircle className="shrink-0 animate-spin text-accent" size={14} /><span className="truncate">{job.currentTitle ? `Checking ${job.currentTitle}` : "Finding same-size candidates"}</span></span><span className="number-font shrink-0 text-muted">{job.hashedItems + job.skippedItems} / {job.candidateItems}</span></div><div className="mt-3 h-1.5 overflow-hidden rounded-full bg-elevated"><div className="h-full bg-accent transition-[width]" style={{ width: `${progress}%` }} /></div><p className="mt-2 text-[10px] text-muted">{formatBytes(job.hashedBytes)} verified · {job.skippedItems} skipped · cached hashes are reused while fingerprints match</p></div> : null}{job?.state === "failed" ? <p className="border-l-2 border-red-400/60 pl-3 text-xs text-red-300">{job.error || "Duplicate analysis failed."}</p> : null}{!running && groups.length === 0 ? <div className="border border-dashed border-line/70 bg-panel/25 px-5 py-7 text-center text-xs text-muted">{job?.state === "completed" ? "No byte-for-byte duplicate groups were found." : "Run analysis to verify possible duplicates without changing any files."}</div> : <div className="space-y-3">{groups.map((group, index) => <article key={group.id} className="bg-panel/55 p-4"><div className="flex flex-wrap items-start justify-between gap-3"><div><div className="flex items-center gap-2"><Copy className="text-accent" size={16} /><h3 className="text-sm font-semibold">Exact duplicate group {index + 1}</h3></div><p className="mt-1 text-[10px] text-muted">SHA-256 verified · {formatBytes(group.fileSizeBytes)} each · up to {formatBytes(group.potentialReclaimBytes)} duplicated data</p></div><span className="number-font rounded bg-elevated px-2 py-1 text-[10px] text-muted">{group.itemIds.length} files</span></div><div className="mt-3 divide-y divide-line/60 border-t border-line/60">{group.itemIds.map((id) => { const item = itemsById.get(id); const confirming = confirmTrash === id; return <div key={id} className="flex items-center gap-3 py-2.5"><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-accent">{item?.mediaKind === "image" ? <ImageIcon size={14} /> : <Film size={14} />}</div><div className="min-w-0 flex-1"><p className="truncate text-xs font-medium text-ink">{item?.title || "Media record"}</p><p className="mt-0.5 truncate text-[10px] text-muted" title={item?.path}>{item?.path || "Location unavailable"}</p></div>{item && !item.missing ? <><Button variant="ghost" className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => void openItem(item.id)}>Open</Button>{confirming ? <Button variant="secondary" className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => setConfirmTrash(null)}>Keep</Button> : null}<Button variant={confirming ? "danger" : "ghost"} className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => void moveToTrash(group.id, item.id)}>{trashBusy === item.id ? <LoaderCircle className="animate-spin" size={13} /> : <Trash2 size={13} />} {confirming ? "Confirm Trash" : "Move to Trash"}</Button></> : null}</div>; })}</div></article>)}</div>}<p className="mt-3 text-[10px] leading-4 text-muted">Before cleanup, Rust re-hashes the selected file and a retained copy. Potential duplicated data is informational; hard links or filesystem compression can make actual recoverable space different.</p></section>;
}

function FileOperationsSection({ operations, onResult, onChanged }: { operations: LibraryFileOperation[]; onResult: (result: LibraryFileOperationResult) => void; onChanged: (operations: LibraryFileOperation[]) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [busyId, setBusyId] = useState<string | null>(null);

  async function undo(operation: LibraryFileOperation) {
    setBusyId(operation.id);
    try {
      const result = await api.undoLibraryFileOperation(operation.id);
      onResult(result);
      setNotice({
        message: operation.kind === "copy" ? "The copied file was moved to system Trash." : "The file was returned to its original location.",
        view: "library",
      });
    } catch (error) {
      setError(normalizeError(error));
      void api.listLibraryFileOperations().then(onChanged).catch(() => {});
    } finally {
      setBusyId(null);
    }
  }

  const labels: Record<LibraryFileOperationKind, string> = { rename: "Renamed", move: "Moved", copy: "Copied" };
  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="file-operations-title"><div className="mb-4"><div className="flex items-center gap-2"><FolderInput className="text-accent" size={18} /><h2 id="file-operations-title" className="text-lg font-semibold tracking-[-0.025em]">File operation history</h2></div><p className="mt-1 max-w-2xl text-xs leading-5 text-muted">Nice Downloader journals confirmed file changes. Completed moves and renames can be reversed while both paths remain safe; copied files are undone through system Trash.</p></div>{operations.length === 0 ? <div className="border border-dashed border-line/70 bg-panel/25 px-5 py-7 text-center text-xs text-muted">No file operations yet. Open a media item and choose Organize file.</div> : <div className="divide-y divide-line/60 bg-panel/55">{operations.map((operation) => <article key={operation.id} className="flex flex-wrap items-center gap-3 px-4 py-3"><div className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${operation.state === "failed" ? "bg-red-400/10 text-red-300" : operation.state === "undone" ? "bg-elevated text-muted" : "bg-accent/10 text-accent"}`}>{operation.state === "undone" ? <Undo2 size={16} /> : <FolderInput size={16} />}</div><div className="min-w-[240px] flex-1"><div className="flex flex-wrap items-center gap-2"><h3 className="text-xs font-semibold">{labels[operation.kind]}</h3><span className="rounded bg-elevated px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.08em] text-muted">{operation.state}</span><span className="text-[10px] text-muted">{new Date(operation.updatedAt).toLocaleString()}</span></div><div className="mt-1 flex min-w-0 items-center gap-1.5 text-[10px] text-muted"><span className="truncate" title={operation.sourcePath}>{operation.sourcePath}</span><ArrowRight className="shrink-0" size={11} /><span className="truncate text-ink/80" title={operation.destinationPath}>{operation.destinationPath}</span></div>{operation.error ? <p className="mt-1 text-[10px] text-red-300">{operation.error}</p> : null}</div>{operation.state === "completed" ? <Button variant="secondary" className="h-8 px-2.5 text-xs" disabled={busyId != null} onClick={() => void undo(operation)}>{busyId === operation.id ? <LoaderCircle className="animate-spin" size={13} /> : <Undo2 size={13} />} Undo</Button> : null}</article>)}</div>}</section>;
}

function CollectionSection({ collections, onChanged }: { collections: LibraryCollection[]; onChanged: (collections: LibraryCollection[]) => void }) {
  const setError = useAppStore((state) => state.setError);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  async function create() {
    if (!name.trim()) return;
    setBusy(true);
    try {
      await api.saveLibraryCollection({ name: name.trim() });
      onChanged(await api.listLibraryCollections());
      setName("");
    } catch (error) { setError(normalizeError(error)); } finally { setBusy(false); }
  }
  async function remove(id: string) {
    if (confirmDelete !== id) { setConfirmDelete(id); return; }
    setBusy(true);
    try {
      await api.deleteLibraryCollection(id);
      onChanged(collections.filter((collection) => collection.id !== id));
      setConfirmDelete(null);
    } catch (error) { setError(normalizeError(error)); } finally { setBusy(false); }
  }
  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="library-collections-title"><div className="mb-4 flex flex-wrap items-end justify-between gap-3"><div><h2 id="library-collections-title" className="text-lg font-semibold tracking-[-0.025em]">Collections</h2><p className="mt-1 text-xs text-muted">Build personal shelves without moving original files.</p></div><div className="flex w-full max-w-sm"><Input className="h-9 rounded-r-none" value={name} onChange={(event) => setName(event.target.value)} placeholder="New collection name" onKeyDown={(event) => { if (event.key === "Enter") void create(); }} /><Button className="h-9 rounded-l-none" disabled={busy || !name.trim()} onClick={() => void create()}><ListPlus size={15} /> Create</Button></div></div>{collections.length === 0 ? <p className="border border-dashed border-line/70 px-5 py-7 text-center text-xs text-muted">No collections yet. Create one, then select media from Library home.</p> : <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">{collections.map((collection) => <article key={collection.id} className="flex items-center gap-3 bg-panel/65 p-4"><div className="grid size-10 place-items-center rounded-[8px] bg-elevated text-accent"><ListPlus size={18} /></div><div className="min-w-0 flex-1"><h3 className="truncate text-sm font-semibold">{collection.name}</h3><p className="mt-1 text-[11px] text-muted">{collection.itemIds.length} item{collection.itemIds.length === 1 ? "" : "s"}</p></div>{confirmDelete === collection.id ? <button className="text-[11px] text-muted hover:text-ink" onClick={() => setConfirmDelete(null)}>Keep</button> : null}<Button variant={confirmDelete === collection.id ? "danger" : "ghost"} className="h-8 px-2 text-xs" disabled={busy} onClick={() => void remove(collection.id)}><Trash2 size={13} /> {confirmDelete === collection.id ? "Confirm" : "Remove"}</Button></article>)}</div>}</section>;
}

function SourceForm({ onClose, onSaved }: { onClose: () => void; onSaved: (source: LibrarySource, job: LibraryScanJob) => void }) {
  const setError = useAppStore((state) => state.setError);
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [mediaKind, setMediaKind] = useState<LibraryMediaKind>("both");
  const [recursive, setRecursive] = useState(true);
  const [watchEnabled, setWatchEnabled] = useState(false);
  const [privateSource, setPrivateSource] = useState(false);
  const [busy, setBusy] = useState(false);

  async function chooseFolder() {
    const selected = await open({ multiple: false, directory: true, title: "Choose a media folder" });
    if (!selected) return;
    setPath(selected);
    if (!name) setName(selected.split(/[\\/]/).filter(Boolean).at(-1) || "Media folder");
  }

  async function addSource() {
    setBusy(true);
    try {
      const source = await api.saveLibrarySource({ name, path, mediaKind, recursive, watchEnabled, private: privateSource, includePatterns: [], excludePatterns: [] });
      const job = await api.startLibraryScan(source.id);
      onSaved(source, job);
    } catch (error) { setError(normalizeError(error)); } finally { setBusy(false); }
  }

  return <section className="panel-shadow mb-6 border border-accent/25 bg-panel/90 p-5" aria-labelledby="add-source-title"><div className="flex items-start justify-between gap-4"><div><h2 id="add-source-title" className="text-sm font-semibold">Add a library source</h2><p className="mt-1 text-xs text-muted">Files stay in this folder. Nice Downloader only creates catalog records.</p></div><button className="text-muted hover:text-ink" onClick={onClose} aria-label="Close add folder form"><X size={18} /></button></div><div className="mt-4 grid gap-3 lg:grid-cols-[1fr_1fr_180px]"><Field label="Folder"><div className="flex gap-2"><Input value={path} readOnly placeholder="Choose a folder" /><Button variant="secondary" className="shrink-0" onClick={() => void chooseFolder()}><FolderOpen size={15} /> Browse</Button></div></Field><Field label="Source name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Media archive" /></Field><Field label="Media"><Select value={mediaKind} onChange={(event) => setMediaKind(event.target.value as LibraryMediaKind)}><option value="both">Videos and images</option><option value="video">Videos only</option><option value="image">Images only</option></Select></Field></div><div className="mt-4 flex flex-wrap items-center justify-between gap-4 border-t border-line/60 pt-4"><div className="flex flex-wrap gap-5 text-xs text-muted"><label className="flex items-center gap-2"><input type="checkbox" checked={recursive} onChange={(event) => setRecursive(event.target.checked)} /> Include subfolders</label><label className="flex items-center gap-2"><input type="checkbox" checked={watchEnabled} onChange={(event) => setWatchEnabled(event.target.checked)} /> Auto scan changes</label><label className="flex items-center gap-2"><input type="checkbox" checked={privateSource} onChange={(event) => setPrivateSource(event.target.checked)} /> Mark as private</label></div><div className="flex gap-2"><Button variant="secondary" onClick={onClose}>Cancel</Button><Button disabled={busy || !path || !name.trim()} onClick={() => void addSource()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Play size={15} />} Add and scan</Button></div></div></section>;
}

function SourceSection({ sources, scanJobs, onJob, onRemoved }: { sources: LibrarySource[]; scanJobs: LibraryScanJob[]; onJob: (job: LibraryScanJob) => void; onRemoved: (id: string) => void }) {
  if (sources.length === 0) return <div className="border border-dashed border-line/70 bg-panel/30 px-5 py-8 text-center text-xs text-muted">No folders are indexed yet.</div>;
  return <section aria-labelledby="library-sources-title"><div className="mb-3 flex items-center justify-between"><div><h2 id="library-sources-title" className="text-lg font-semibold tracking-[-0.025em]">Folders</h2><p className="mt-1 text-xs text-muted">Scans index media in place.</p></div><span className="text-[11px] text-muted">{sources.length} source{sources.length === 1 ? "" : "s"}</span></div><div className="grid gap-3 lg:grid-cols-2 2xl:grid-cols-3">{sources.map((source) => <SourceCard key={source.id} source={source} latestJob={scanJobs.find((job) => job.sourceId === source.id)} onJob={onJob} onRemoved={() => onRemoved(source.id)} />)}</div></section>;
}

function SourceCard({ source, latestJob, onJob, onRemoved }: { source: LibrarySource; latestJob?: LibraryScanJob; onJob: (job: LibraryScanJob) => void; onRemoved: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [watchBusy, setWatchBusy] = useState(false);
  const [watchEnabled, setWatchEnabled] = useState(source.watchEnabled);
  useEffect(() => setWatchEnabled(source.watchEnabled), [source.watchEnabled]);
  const runningJob = latestJob?.state === "running" ? latestJob : undefined;
  async function scan() { try { onJob(await api.startLibraryScan(source.id)); } catch (error) { setError(normalizeError(error)); } }
  async function cancel() { if (!runningJob) return; try { await api.cancelLibraryScan(runningJob.id); } catch (error) { setError(normalizeError(error)); } }
  async function remove() { if (!confirmRemove) { setConfirmRemove(true); return; } try { await api.removeLibrarySource(source.id); onRemoved(); } catch (error) { setError(normalizeError(error)); } }
  async function toggleWatch() {
    const next = !watchEnabled;
    setWatchBusy(true);
    try {
      const changed = await api.saveLibrarySource({ id: source.id, name: source.name, path: source.path, mediaKind: source.mediaKind, recursive: source.recursive, watchEnabled: next, private: source.private, includePatterns: source.includePatterns, excludePatterns: source.excludePatterns });
      setWatchEnabled(changed.watchEnabled);
      setNotice({ message: changed.watchEnabled ? `${source.name} will rescan automatically after folder changes.` : `Automatic scans disabled for ${source.name}.`, view: "library" });
    } catch (error) { setError(normalizeError(error)); } finally { setWatchBusy(false); }
  }
  return <article className="bg-panel/65 p-4"><div className="flex min-w-0 items-start gap-3"><div className="grid size-10 shrink-0 place-items-center rounded-[9px] bg-elevated text-accent"><HardDrive size={18} /></div><div className="min-w-0 flex-1"><div className="flex items-center gap-2"><h3 className="truncate text-sm font-semibold">{source.name}</h3><SourceStatus status={runningJob ? "scanning" : source.status} />{source.private ? <span className="text-[9px] font-semibold uppercase tracking-[0.08em] text-muted">Private</span> : null}</div><p className="mt-1 truncate text-[11px] text-muted" title={source.path}>{source.path}</p></div></div>{runningJob ? <div className="mt-3 border-t border-line/60 pt-3" role="status"><div className="flex items-center justify-between text-[11px] text-muted"><span className="flex items-center gap-1.5"><LoaderCircle className="animate-spin" size={12} /> Indexing {runningJob.currentPath || "folder"}</span><span className="number-font">{runningJob.indexedCount} added</span></div><p className="mt-1 truncate text-[10px] text-muted/70">{runningJob.discoveredCount} discovered · {runningJob.skippedCount} skipped · {runningJob.warningCount} warnings</p></div> : <p className="mt-3 text-[11px] text-muted">{source.itemCount} indexed · {source.mediaKind === "both" ? "videos and images" : `${source.mediaKind}s`}{source.lastScannedAt ? ` · scanned ${new Date(source.lastScannedAt).toLocaleDateString()}` : " · not scanned"}</p>}{source.lastError && !runningJob ? <p className="mt-2 text-[11px] text-red-400">{source.lastError}</p> : null}{latestJob && !runningJob && latestJob.warningCount > 0 ? <details className="mt-2 text-[11px] text-amber-400"><summary className="cursor-pointer">{latestJob.warningCount} scan warning{latestJob.warningCount === 1 ? "" : "s"}</summary><ul className="mt-1 space-y-1 pl-4 text-muted">{latestJob.warnings.map((warning, index) => <li key={`${latestJob.id}-${index}`} className="list-disc">{warning}</li>)}</ul></details> : null}<div className="mt-3 flex flex-wrap items-center gap-2 border-t border-line/60 pt-3">{runningJob ? <Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => void cancel()}><Ban size={13} /> Cancel scan</Button> : <Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => void scan()}><FolderSearch size={13} /> Scan now</Button>}<label className="flex items-center gap-2 px-1 text-[11px] text-muted"><input type="checkbox" checked={watchEnabled} disabled={watchBusy} onChange={() => void toggleWatch()} /> Auto scan</label>{confirmRemove ? <button className="ml-auto text-[11px] text-muted hover:text-ink" onClick={() => setConfirmRemove(false)}>Keep source</button> : null}<Button variant={confirmRemove ? "danger" : "ghost"} className="ml-auto h-7 px-2 text-xs" disabled={Boolean(runningJob)} onClick={() => void remove()}><Trash2 size={13} /> {confirmRemove ? "Remove catalog records" : "Remove"}</Button></div></article>;
}

function SourceStatus({ status }: { status: LibrarySource["status"] }) {
  const labels = { online: "Online", offline: "Offline", scanning: "Scanning", error: "Needs attention" };
  const colors = { online: "bg-emerald-400/10 text-emerald-400", offline: "bg-amber-400/10 text-amber-400", scanning: "bg-accent/10 text-accent", error: "bg-red-400/10 text-red-400" };
  return <span className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.08em] ${colors[status]}`}>{labels[status]}</span>;
}

function upsertById<T extends { id: string }>(items: T[], next: T): T[] {
  return items.some((item) => item.id === next.id) ? items.map((item) => item.id === next.id ? next : item) : [next, ...items];
}

function upsertByPath(items: LibraryItem[], next: LibraryItem): LibraryItem[] {
  return items.some((item) => item.path === next.path) ? items.map((item) => item.path === next.path ? next : item) : [next, ...items];
}
