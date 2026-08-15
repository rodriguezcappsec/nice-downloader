import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  ArchiveRestore,
  Ban,
  BellDot,
  Bookmark,
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
  Layers,
  List,
  Image as ImageIcon,
  Library,
  LoaderCircle,
  MonitorPlay,
  Pause,
  Play,
  Database,
  Eraser,
  ExternalLink,
  Maximize,
  RotateCcw,
  RotateCw,
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
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Field, Input, Select, Textarea } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { canOfferPlaybackFallback, createPlaybackError, EMBEDDED_PLAYBACK_TIMEOUT_MS } from "../../lib/playback";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import { CreativeTimeline } from "./CreativeTimeline";
import type {
  AppError,
  LibraryArtworkCacheStatus,
  LibraryItem,
  LibraryImageSession,
  LibraryBulkAction,
  LibraryBulkMovePlan,
  LibraryBulkMoveResult,
  LibraryChange,
  LibraryDuplicateAnalysisJob,
  LibraryDuplicateGroup,
  LibraryFileOperation,
  LibraryFileOperationKind,
  LibraryFileOperationPlan,
  LibraryFileOperationResult,
  LibraryCollection,
  LibraryFacets,
  LibraryFolderFacet,
  LibraryMediaKind,
  LibraryMediaStack,
  LibraryMediaStackRole,
  LibraryPlaybackEntry,
  LibraryPlaybackPreflight,
  LibraryPlaybackPreparationProgress,
  LibraryPlaybackSession,
  LibraryPlaylist,
  LibraryRichMetadata,
  LibrarySearchStatus,
  LibrarySmartView,
  LibrarySmartViewQuery,
  LibraryWatchState,
  LibraryQueryRequest,
  LibraryQueryResult,
  LibrarySort,
  LibraryScanJob,
  LibrarySource,
} from "../../types";

type MediaFilter = "all" | "video" | "image";
type ResolutionFilter = "all" | "2160" | "1440" | "1080" | "720" | "sd" | "unknown";
type DurationFilter = "all" | "short" | "medium" | "long" | "unknown";
type DateFilter = "all" | "week" | "month" | "year";
type AvailabilityFilter = "all" | "available" | "missing";
type FolderScope = "tree" | "direct";
type FolderSelection = { sourceId: string; path: string };
type FolderOption = FolderSelection & { value: string; label: string; directCount: number; treeCount: number };
const LIBRARY_PAGE_SIZE = 60;
const WATCH_BROWSE_PAGE_SIZE = 24;
const EMPTY_LIBRARY_RESULT: LibraryQueryResult = { items: [], totalItems: 0, page: 1, pageSize: LIBRARY_PAGE_SIZE, totalPages: 1 };
const EMPTY_LIBRARY_FACETS: LibraryFacets = { totalItems: 0, availableItems: 0, missingItems: 0, containers: [], tags: [], creators: [], series: [], sites: [], folders: [] };
type LiveAnnouncement = { signature: string; message: string; assertive?: boolean };

export function LibraryPage({ active = true }: { active?: boolean }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const currentView = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const setCompareItems = useAppStore((state) => state.setCompareItems);
  const setImageCompareItems = useAppStore((state) => state.setImageCompareItems);
  const [queryResult, setQueryResult] = useState<LibraryQueryResult>(EMPTY_LIBRARY_RESULT);
  const [recentResult, setRecentResult] = useState<LibraryQueryResult>({ ...EMPTY_LIBRARY_RESULT, pageSize: 12 });
  const [videoResult, setVideoResult] = useState<LibraryQueryResult>({ ...EMPTY_LIBRARY_RESULT, pageSize: 12 });
  const [imageResult, setImageResult] = useState<LibraryQueryResult>({ ...EMPTY_LIBRARY_RESULT, pageSize: 12 });
  const [continueWatching, setContinueWatching] = useState<LibraryPlaybackEntry[]>([]);
  const [watchLater, setWatchLater] = useState<LibraryItem[]>([]);
  const [recentlyWatched, setRecentlyWatched] = useState<LibraryPlaybackEntry[]>([]);
  const [autoplayNext, setAutoplayNext] = useState(() => window.localStorage.getItem("nice-downloader.library-autoplay") !== "false");
  const [facets, setFacets] = useState<LibraryFacets>(EMPTY_LIBRARY_FACETS);
  const [facetsContext, setFacetsContext] = useState<string | null>(null);
  const [manageItems, setManageItems] = useState<LibraryItem[]>([]);
  const [sources, setSources] = useState<LibrarySource[]>([]);
  const [scanJobs, setScanJobs] = useState<LibraryScanJob[]>([]);
  const [collections, setCollections] = useState<LibraryCollection[]>([]);
  const [smartViews, setSmartViews] = useState<LibrarySmartView[]>([]);
  const [activeSmartViewId, setActiveSmartViewId] = useState<string | null>(null);
  const [smartViewName, setSmartViewName] = useState("");
  const [smartViewFormOpen, setSmartViewFormOpen] = useState(false);
  const [smartViewBusy, setSmartViewBusy] = useState(false);
  const [confirmDeleteSmartViewId, setConfirmDeleteSmartViewId] = useState<string | null>(null);
  const [searchStatus, setSearchStatus] = useState<LibrarySearchStatus | null>(null);
  const [playlists, setPlaylists] = useState<LibraryPlaylist[]>([]);
  const [mediaStacks, setMediaStacks] = useState<LibraryMediaStack[]>([]);
  const [activeMediaStack, setActiveMediaStack] = useState<LibraryMediaStack | null>(null);
  const [activeMediaStackItems, setActiveMediaStackItems] = useState<LibraryItem[]>([]);
  const [mediaStackLoading, setMediaStackLoading] = useState(false);
  const [stackDraftItems, setStackDraftItems] = useState<LibraryItem[] | null>(null);
  const [activePlaylist, setActivePlaylist] = useState<LibraryPlaylist | null>(null);
  const [activePlaylistItems, setActivePlaylistItems] = useState<LibraryItem[]>([]);
  const [playlistLoading, setPlaylistLoading] = useState(false);
  const [playlistMutationBusy, setPlaylistMutationBusy] = useState(false);
  const [playbackContext, setPlaybackContext] = useState<LibraryItem[] | null>(null);
  const [changes, setChanges] = useState<LibraryChange[]>([]);
  const [duplicateJob, setDuplicateJob] = useState<LibraryDuplicateAnalysisJob | null>(null);
  const [duplicateGroups, setDuplicateGroups] = useState<LibraryDuplicateGroup[]>([]);
  const [fileOperations, setFileOperations] = useState<LibraryFileOperation[]>([]);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [sourceFilter, setSourceFilter] = useState("all");
  const [folderFilter, setFolderFilter] = useState("all");
  const [folderScope, setFolderScope] = useState<FolderScope>("tree");
  const [mediaFilter, setMediaFilter] = useState<MediaFilter>("all");
  const [resolutionFilter, setResolutionFilter] = useState<ResolutionFilter>("all");
  const [containerFilter, setContainerFilter] = useState("all");
  const [durationFilter, setDurationFilter] = useState<DurationFilter>("all");
  const [dateFilter, setDateFilter] = useState<DateFilter>("all");
  const [publishedFilter, setPublishedFilter] = useState<DateFilter>("all");
  const [sort, setSort] = useState<LibrarySort>("recent");
  const [availabilityFilter, setAvailabilityFilter] = useState<AvailabilityFilter>("available");
  const [tagFilters, setTagFilters] = useState<string[]>([]);
  const [creatorFilters, setCreatorFilters] = useState<string[]>([]);
  const [seriesFilters, setSeriesFilters] = useState<string[]>([]);
  const [siteFilters, setSiteFilters] = useState<string[]>([]);
  const [ratingFilter, setRatingFilter] = useState("all");
  const [collectionFilter, setCollectionFilter] = useState("all");
  const [favoriteOnly, setFavoriteOnly] = useState(false);
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [bulkMoveOpen, setBulkMoveOpen] = useState(false);
  const [viewMode, setViewMode] = useState<"grid" | "list">(() => window.localStorage.getItem("nice-downloader.library-view") === "list" ? "list" : "grid");
  const [selectedItem, setSelectedItem] = useState<LibraryItem | null>(null);
  const [detailsExtraItems, setDetailsExtraItems] = useState<LibraryItem[]>([]);
  const [watchItem, setWatchItem] = useState<LibraryItem | null>(null);
  const [viewImage, setViewImage] = useState<LibraryItem | null>(null);
  const [watchMinimized, setWatchMinimized] = useState(false);
  const [watchSearch, setWatchSearch] = useState("");
  const [watchDebouncedSearch, setWatchDebouncedSearch] = useState("");
  const [watchBrowsePage, setWatchBrowsePage] = useState(1);
  const [watchBrowseResult, setWatchBrowseResult] = useState<LibraryQueryResult>({ ...EMPTY_LIBRARY_RESULT, pageSize: WATCH_BROWSE_PAGE_SIZE });
  const [watchBrowseLoading, setWatchBrowseLoading] = useState(false);
  const [manageOpen, setManageOpen] = useState(false);
  const [showSourceForm, setShowSourceForm] = useState(false);
  const [importTags, setImportTags] = useState("");
  const [busy, setBusy] = useState(false);
  const [resultPage, setResultPage] = useState(1);
  const [artworkCacheStatus, setArtworkCacheStatus] = useState<LibraryArtworkCacheStatus | null>(null);
  const [queryLoading, setQueryLoading] = useState(true);
  const [libraryRevision, setLibraryRevision] = useState(0);
  const [libraryAnnouncement, setLibraryAnnouncement] = useState("");
  const [libraryAlert, setLibraryAlert] = useState("");
  const refreshTimer = useRef<number | null>(null);
  const scanAnnouncementSignatures = useRef<Map<string, string>>(new Map());
  const duplicateAnnouncementSignature = useRef("");
  const resultAnnouncementSignature = useRef("");
  const smartViewsRequestRevision = useRef(0);
  const detailsReturnFocus = useRef<HTMLElement | null>(null);
  const imageReturnFocus = useRef<HTMLElement | null>(null);
  const bulkMoveReturnFocus = useRef<HTMLElement | null>(null);
  const sourceFormReturnFocus = useRef<HTMLElement | null>(null);
  const playlistLoadRevision = useRef(0);
  const mediaStackLoadRevision = useRef(0);

  function activeElement(): HTMLElement | null {
    return document.activeElement instanceof HTMLElement ? document.activeElement : null;
  }

  function restoreFocus(target: HTMLElement | null, fallbackId: string) {
    window.setTimeout(() => {
      if (target?.isConnected) {
        target.focus();
        return;
      }
      (document.getElementById(fallbackId) || document.getElementById("main-content"))?.focus();
    }, 0);
  }

  useEffect(() => {
    if (!active && watchItem) setWatchMinimized(true);
  }, [active, watchItem]);

  function startWatching(item: LibraryItem, queue: LibraryItem[] | null = null) {
    setViewImage(null);
    imageReturnFocus.current = null;
    setWatchItem(item);
    setPlaybackContext(queue);
    if (queue) setWatchSearch("");
    setWatchMinimized(false);
    setView("library");
    window.setTimeout(() => document.getElementById("main-content")?.scrollTo({ top: 0 }), 0);
  }

  function startViewingImage(item: LibraryItem, returnTarget = activeElement()) {
    if (!viewImage) imageReturnFocus.current = returnTarget;
    setViewImage(item);
    if (watchItem) setWatchMinimized(true);
    setView("library");
    window.setTimeout(() => document.getElementById("main-content")?.scrollTo({ top: 0 }), 0);
  }

  function closeImageViewer() {
    const target = imageReturnFocus.current;
    imageReturnFocus.current = null;
    setViewImage(null);
    restoreFocus(target, "library-search");
  }

  function minimizeWatching() {
    setWatchMinimized(true);
    scheduleLibraryRefresh(0);
    restoreFocus(null, "library-search");
  }

  function restoreWatching() {
    setViewImage(null);
    imageReturnFocus.current = null;
    setView("library");
    setWatchMinimized(false);
    window.setTimeout(() => document.getElementById("main-content")?.scrollTo({ top: 0 }), 0);
    restoreFocus(null, "watch-details-button");
  }

  function closeWatching() {
    setWatchItem(null);
    setPlaybackContext(null);
    setWatchMinimized(false);
    scheduleLibraryRefresh(0);
  }

  function openDetails(item: LibraryItem) {
    if (!selectedItem) detailsReturnFocus.current = activeElement();
    setDetailsExtraItems([]);
    setSelectedItem(item);
  }

  function openRelatedDetails(item: LibraryItem) {
    setDetailsExtraItems((current) => Array.from(new Map([...current, item].map((candidate) => [candidate.id, candidate])).values()));
    setSelectedItem(item);
  }

  function closeDetails(shouldRestoreFocus = true) {
    const target = detailsReturnFocus.current;
    detailsReturnFocus.current = null;
    setSelectedItem(null);
    setDetailsExtraItems([]);
    if (shouldRestoreFocus) {
      restoreFocus(
        target,
        active && viewImage
          ? "image-details-button"
          : active && watchItem && !watchMinimized
            ? "watch-details-button"
            : "library-search",
      );
    }
  }

  function openSelectedMedia() {
    if (!selectedItem) return;
    if (selectedItem.mediaKind === "video") {
      startWatching(selectedItem);
      closeDetails(false);
      restoreFocus(null, "watch-details-button");
      return;
    }
    startViewingImage(selectedItem, detailsReturnFocus.current);
    closeDetails(false);
    restoreFocus(null, "image-details-button");
  }

  function openBulkMove() {
    bulkMoveReturnFocus.current = activeElement();
    setBulkMoveOpen(true);
  }

  function closeBulkMove() {
    const target = bulkMoveReturnFocus.current;
    bulkMoveReturnFocus.current = null;
    setBulkMoveOpen(false);
    restoreFocus(target, "library-search");
  }

  function openSourceForm() {
    sourceFormReturnFocus.current = activeElement();
    setShowSourceForm(true);
  }

  function closeSourceForm() {
    const target = sourceFormReturnFocus.current;
    sourceFormReturnFocus.current = null;
    setShowSourceForm(false);
    restoreFocus(target, "library-add-folder-button");
  }

  function publishAnnouncement(announcement: LiveAnnouncement) {
    if (announcement.assertive) setLibraryAlert(announcement.message);
    else setLibraryAnnouncement(announcement.message);
  }

  function announceScan(job: LibraryScanJob) {
    const announcement = libraryScanAnnouncement(job);
    if (scanAnnouncementSignatures.current.get(job.id) === announcement.signature) return;
    if (!scanAnnouncementSignatures.current.has(job.id) && scanAnnouncementSignatures.current.size >= 200) {
      const oldestJobId = scanAnnouncementSignatures.current.keys().next().value;
      if (oldestJobId) scanAnnouncementSignatures.current.delete(oldestJobId);
    }
    scanAnnouncementSignatures.current.set(job.id, announcement.signature);
    publishAnnouncement(announcement);
  }

  function announceDuplicateAnalysis(job: LibraryDuplicateAnalysisJob) {
    const announcement = duplicateAnalysisAnnouncement(job);
    if (duplicateAnnouncementSignature.current === announcement.signature) return;
    duplicateAnnouncementSignature.current = announcement.signature;
    publishAnnouncement(announcement);
  }

  async function load() {
    const smartViewsRequest = ++smartViewsRequestRevision.current;
    try {
      const [nextSources, nextJobs, nextCollections, nextPlaylists, nextMediaStacks, nextChanges, nextDuplicateJob, nextDuplicateGroups, nextFileOperations, nextSmartViews, nextSearchStatus] = await Promise.all([
        api.listLibrarySources(),
        api.listLibraryScanJobs(),
        api.listLibraryCollections(),
        api.listLibraryPlaylists(),
        api.listLibraryMediaStacks(),
        api.listLibraryChanges(),
        api.getLibraryDuplicateAnalysis(),
        api.listLibraryDuplicateGroups(),
        api.listLibraryFileOperations(),
        api.listLibrarySmartViews(),
        api.getLibrarySearchStatus(),
      ]);
      setSources(nextSources);
      setScanJobs(nextJobs);
      setCollections(nextCollections);
      setPlaylists(nextPlaylists);
      setMediaStacks(nextMediaStacks);
      setChanges(nextChanges);
      setDuplicateJob(nextDuplicateJob);
      setDuplicateGroups(nextDuplicateGroups);
      setFileOperations(nextFileOperations);
      if (smartViewsRequest === smartViewsRequestRevision.current) {
        setSmartViews(nextSmartViews);
      }
      setSearchStatus(nextSearchStatus);
      nextJobs.filter((job) => job.state === "running").forEach(announceScan);
      if (nextDuplicateJob?.state === "running") announceDuplicateAnalysis(nextDuplicateJob);
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  function scheduleLibraryRefresh(delay = 150) {
    if (refreshTimer.current != null) window.clearTimeout(refreshTimer.current);
    refreshTimer.current = window.setTimeout(() => {
      refreshTimer.current = null;
      setLibraryRevision((revision) => revision + 1);
    }, delay);
  }

  useEffect(() => {
    const refreshWatchShelves = () => scheduleLibraryRefresh(0);
    window.addEventListener("nice-downloader:library-watch-state", refreshWatchShelves);
    return () => window.removeEventListener("nice-downloader:library-watch-state", refreshWatchShelves);
  }, []);

  useEffect(() => {
    let disposed = false;
    const startSelection = (event: Event) => {
      const requestedIds = (event as CustomEvent<{ itemIds?: string[] }>).detail?.itemIds || [];
      setSelectedIds(new Set(Array.from(new Set(requestedIds)).slice(0, 4)));
      setSelectionMode(true);
      setActivePlaylist(null);
      setActivePlaylistItems([]);
    };
    const startBulkMoveFromWorkspace = (event: Event) => {
      const requestedIds = (event as CustomEvent<{ itemIds?: string[] }>).detail?.itemIds || [];
      if (requestedIds.length === 0) return;
      setSelectedIds(new Set(Array.from(new Set(requestedIds)).slice(0, 500)));
      setSelectionMode(true);
      setBulkMoveOpen(true);
      setActivePlaylist(null);
      setActiveMediaStack(null);
    };
    const openLibraryItemFromWorkspace = (event: Event) => {
      const itemId = (event as CustomEvent<{ itemId?: string }>).detail?.itemId;
      if (!itemId) return;
      void api.getLibraryItemsByIds([itemId]).then(([item]) => {
        if (!disposed && item) openDetails(item);
      }).catch((error) => { if (!disposed) setError(normalizeError(error)); });
    };
    const stopSinglePlayback = () => {
      setWatchItem(null);
      setPlaybackContext(null);
      setWatchMinimized(false);
      setViewImage(null);
      imageReturnFocus.current = null;
      setSelectedItem(null);
      setDetailsExtraItems([]);
    };
    window.addEventListener("nice-downloader:start-compare-selection", startSelection);
    window.addEventListener("nice-downloader:start-image-compare-selection", startSelection);
    window.addEventListener("nice-downloader:compare-start", stopSinglePlayback);
    window.addEventListener("nice-downloader:image-compare-start", stopSinglePlayback);
    window.addEventListener("nice-downloader:start-bulk-move", startBulkMoveFromWorkspace);
    window.addEventListener("nice-downloader:open-library-item", openLibraryItemFromWorkspace);
    return () => {
      disposed = true;
      window.removeEventListener("nice-downloader:start-compare-selection", startSelection);
      window.removeEventListener("nice-downloader:start-image-compare-selection", startSelection);
      window.removeEventListener("nice-downloader:compare-start", stopSinglePlayback);
      window.removeEventListener("nice-downloader:image-compare-start", stopSinglePlayback);
      window.removeEventListener("nice-downloader:start-bulk-move", startBulkMoveFromWorkspace);
      window.removeEventListener("nice-downloader:open-library-item", openLibraryItemFromWorkspace);
    };
  }, [setError]);

  async function refreshArtworkCacheStatus() {
    try {
      setArtworkCacheStatus(await api.getLibraryArtworkCacheStatus());
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  useEffect(() => {
    void load();
    void refreshArtworkCacheStatus();
    let disposed = false;
    const unlisteners: Array<() => void> = [];
    void api.onLibraryScanUpdated((job) => {
      setScanJobs((current) => upsertById(current, job));
      announceScan(job);
      if (job.state !== "running") {
        void Promise.all([api.listLibrarySources(), api.getLibrarySearchStatus()]).then(([nextSources, nextSearchStatus]) => {
          if (!disposed) {
            setSources(nextSources);
            setSearchStatus(nextSearchStatus);
            scheduleLibraryRefresh(0);
          }
        });
      }
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryItemIndexed((item) => {
      setSelectedItem((current) => current?.id === item.id || current?.path === item.path ? item : current);
      scheduleLibraryRefresh();
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibrarySourceUpdated((source) => {
      setSources((current) => upsertById(current, source));
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryChangeUpdated((change) => {
      setChanges((current) => upsertById(current.filter((entry) => entry.itemId !== change.itemId), change));
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    void api.onLibraryDuplicateAnalysisUpdated((job) => {
      setDuplicateJob(job);
      announceDuplicateAnalysis(job);
      if (job.state === "completed") {
        void api.listLibraryDuplicateGroups().then((groups) => { if (!disposed) setDuplicateGroups(groups); });
      }
    }).then((unlisten) => disposed ? unlisten() : unlisteners.push(unlisten));
    return () => {
      disposed = true;
      if (refreshTimer.current != null) window.clearTimeout(refreshTimer.current);
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    const timer = window.setTimeout(() => setWatchDebouncedSearch(watchSearch.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [watchSearch]);

  useEffect(() => {
    setWatchBrowsePage(1);
  }, [watchDebouncedSearch]);

  useEffect(() => {
    if (!watchItem) return;
    const folder = parseFolderFilter(folderFilter);
    let disposed = false;
    setWatchBrowseLoading(true);
    void api.queryLibraryItems({
      page: watchBrowsePage,
      pageSize: WATCH_BROWSE_PAGE_SIZE,
      search: watchDebouncedSearch || null,
      sourceId: sourceFilter === "all" ? null : sourceFilter,
      folderSourceId: folder?.sourceId || null,
      folderPath: folder?.path ?? null,
      folderScope,
      mediaKind: "video",
      resolution: null,
      container: null,
      duration: null,
      dateAdded: null,
      availability: "available",
      tags: [],
      creators: [],
      series: [],
      sites: [],
      rating: null,
      collectionId: null,
      favoriteOnly: false,
      sort: "recent",
    }).then((result) => {
      if (!disposed) {
        setWatchBrowseResult(result);
        if (result.page !== watchBrowsePage) setWatchBrowsePage(result.page);
      }
    }).catch((error) => {
      if (!disposed) setError(normalizeError(error));
    }).finally(() => {
      if (!disposed) setWatchBrowseLoading(false);
    });
    return () => { disposed = true; };
  }, [folderFilter, folderScope, libraryRevision, setError, sourceFilter, watchBrowsePage, watchDebouncedSearch, watchItem]);

  const libraryRequest = useMemo<LibraryQueryRequest>(() => {
    const folder = parseFolderFilter(folderFilter);
    return {
      page: resultPage,
      pageSize: LIBRARY_PAGE_SIZE,
      search: debouncedSearch || null,
      sourceId: sourceFilter === "all" ? null : sourceFilter,
      folderSourceId: folder?.sourceId || null,
      folderPath: folder?.path ?? null,
      folderScope,
      mediaKind: mediaFilter === "all" ? null : mediaFilter,
      resolution: resolutionFilter === "all" ? null : resolutionFilter,
      container: containerFilter === "all" ? null : containerFilter,
      duration: durationFilter === "all" ? null : durationFilter,
      dateAdded: dateFilter === "all" ? null : dateFilter,
      datePublished: publishedFilter === "all" ? null : publishedFilter,
      availability: availabilityFilter,
      tags: tagFilters,
      creators: creatorFilters,
      series: seriesFilters,
      sites: siteFilters,
      rating: ratingFilter === "all" ? null : ratingFilter,
      collectionId: collectionFilter === "all" ? null : collectionFilter,
      favoriteOnly,
      sort,
    };
  }, [availabilityFilter, collectionFilter, containerFilter, creatorFilters, dateFilter, debouncedSearch, durationFilter, favoriteOnly, folderFilter, folderScope, mediaFilter, publishedFilter, ratingFilter, resolutionFilter, resultPage, seriesFilters, siteFilters, sort, sourceFilter, tagFilters]);
  const currentSmartViewQuery = useMemo<LibrarySmartViewQuery>(() => {
    const { page: _page, pageSize: _pageSize, ...query } = libraryRequest;
    return { ...query, search: search.trim() || null };
  }, [libraryRequest, search]);

  useEffect(() => {
    if (!activeSmartViewId) return;
    const activeView = smartViews.find((view) => view.id === activeSmartViewId);
    if (!activeView || JSON.stringify(activeView.query) !== JSON.stringify(currentSmartViewQuery)) {
      setActiveSmartViewId(null);
    }
  }, [activeSmartViewId, currentSmartViewQuery, smartViews]);

  useEffect(() => {
    let disposed = false;
    setQueryLoading(true);
    void api.queryLibraryItems(libraryRequest).then((result) => {
      if (disposed) return;
      setQueryResult(result);
      if (result.page !== resultPage) setResultPage(result.page);
      const signature = JSON.stringify(libraryRequest);
      if (resultAnnouncementSignature.current !== signature) {
        resultAnnouncementSignature.current = signature;
        const start = result.totalItems === 0 || result.items.length === 0 ? 0 : (result.page - 1) * result.pageSize + 1;
        const end = start === 0 ? 0 : Math.min(start + result.items.length - 1, result.totalItems);
        setLibraryAnnouncement(result.totalItems === 0 || result.items.length === 0
          ? "No library items match the current filters."
          : `Library results updated. Showing ${start} through ${end} of ${result.totalItems}, page ${result.page} of ${result.totalPages}.`);
      }
    }).catch((error) => {
      if (!disposed) setError(normalizeError(error));
    }).finally(() => {
      if (!disposed) setQueryLoading(false);
    });
    return () => { disposed = true; };
  }, [libraryRequest, libraryRevision, resultPage, setError]);

  useEffect(() => {
    let disposed = false;
    const homeRequest = (mediaKind: "video" | "image" | null): LibraryQueryRequest => ({
      page: 1,
      pageSize: 12,
      search: null,
      sourceId: null,
      folderSourceId: null,
      folderPath: null,
      folderScope: "tree",
      mediaKind,
      resolution: null,
      container: null,
      duration: null,
      dateAdded: null,
      availability: "available",
      tags: [],
      creators: [],
      series: [],
      sites: [],
      rating: null,
      collectionId: null,
      favoriteOnly: false,
      sort: "recent",
    });
    void Promise.all([
      api.queryLibraryItems(homeRequest(null)),
      api.queryLibraryItems(homeRequest("video")),
      api.queryLibraryItems(homeRequest("image")),
      api.listContinueWatching(12),
      api.listLibraryWatchLater(12),
      api.listRecentlyWatched(12),
    ]).then(([recent, videos, images, continuing, savedForLater, watched]) => {
      if (!disposed) {
        setRecentResult(recent);
        setVideoResult(videos);
        setImageResult(images);
        setContinueWatching(continuing);
        setWatchLater(savedForLater);
        setRecentlyWatched(watched);
      }
    }).catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [libraryRevision, setError]);

  useEffect(() => {
    let disposed = false;
    const context = `${sourceFilter}:${availabilityFilter}`;
    void api.getLibraryFacets({
      sourceId: sourceFilter === "all" ? null : sourceFilter,
      availability: availabilityFilter,
    }).then((next) => {
      if (!disposed) {
        setFacets(next);
        setFacetsContext(context);
      }
    })
      .catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [availabilityFilter, libraryRevision, setError, sourceFilter]);

  useEffect(() => {
    if (!manageOpen) return;
    const ids = Array.from(new Set([
      ...changes.map((change) => change.itemId),
      ...duplicateGroups.flatMap((group) => group.itemIds),
    ])).slice(0, 500);
    let disposed = false;
    void api.getLibraryItemsByIds(ids)
      .then((next) => { if (!disposed) setManageItems(next); })
      .catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [changes, duplicateGroups, libraryRevision, manageOpen, setError]);

  const sourceNames = useMemo(() => new Map(sources.map((source) => [source.id, source.name])), [sources]);
  const items = queryResult.items;
  const missingItemCount = facets.missingItems;
  const availableItemCount = facets.availableItems;
  const containers = facets.containers;
  const tags = facets.tags;
  const creators = facets.creators;
  const seriesOptions = facets.series;
  const sites = facets.sites;
  const folderOptions = useMemo(() => buildFolderOptions(facets.folders, sources, sourceFilter), [facets.folders, sourceFilter, sources]);
  const featured = recentResult.items[0];
  const videos = videoResult.items;
  const images = imageResult.items;
  const resultPageCount = queryResult.totalPages;
  const currentResultPage = queryResult.page;
  const resultStart = queryResult.totalItems === 0 ? 0 : (currentResultPage - 1) * queryResult.pageSize;
  const resultEnd = Math.min(resultStart + items.length, queryResult.totalItems);
  const isFiltered = Boolean(search.trim()) || sourceFilter !== "all" || folderFilter !== "all" || mediaFilter !== "all" || resolutionFilter !== "all" || containerFilter !== "all" || durationFilter !== "all" || dateFilter !== "all" || publishedFilter !== "all" || availabilityFilter !== "available" || tagFilters.length > 0 || creatorFilters.length > 0 || seriesFilters.length > 0 || siteFilters.length > 0 || ratingFilter !== "all" || collectionFilter !== "all" || favoriteOnly || sort !== "recent";
  const resultTitle = search.trim()
    ? `Results for “${search.trim()}”`
    : creatorFilters.length === 1
      ? `From ${creatorFilters[0]}`
      : seriesFilters.length === 1
        ? seriesFilters[0]
        : siteFilters.length === 1
          ? siteFilters[0]
          : mediaFilter === "video"
            ? "Videos"
            : mediaFilter === "image"
              ? "Images"
              : sourceNames.get(sourceFilter) || "All media";
  const discoveryContext = creatorFilters.length === 1 && seriesFilters.length === 0
    ? { kind: "creator" as const, value: creatorFilters[0] }
    : seriesFilters.length === 1 && creatorFilters.length === 0
      ? { kind: "series" as const, value: seriesFilters[0] }
      : null;

  useEffect(() => {
    setResultPage(1);
  }, [availabilityFilter, collectionFilter, containerFilter, creatorFilters, dateFilter, durationFilter, favoriteOnly, folderFilter, folderScope, mediaFilter, publishedFilter, ratingFilter, resolutionFilter, search, selectionMode, seriesFilters, siteFilters, sort, sourceFilter, tagFilters]);

  useEffect(() => {
    if (folderFilter === "all") return;
    if (facetsContext !== `${sourceFilter}:${availabilityFilter}`) return;
    const selected = parseFolderFilter(folderFilter);
    if (!selected || !folderOptions.some((option) => option.value === folderFilter) || (sourceFilter !== "all" && selected.sourceId !== sourceFilter)) {
      setFolderFilter("all");
      setFolderScope("tree");
    }
  }, [availabilityFilter, facetsContext, folderFilter, folderOptions, sourceFilter]);

  const navigationItems = useMemo(() => activeMediaStack ? activeMediaStackItems : isFiltered || selectionMode
    ? Array.from(new Map([...items, ...detailsExtraItems].map((item) => [item.id, item])).values())
    : Array.from(new Map([...continueWatching.map((entry) => entry.item), ...watchLater, ...recentlyWatched.map((entry) => entry.item), ...recentResult.items, ...videos, ...images, ...detailsExtraItems].map((item) => [item.id, item])).values()),
    [activeMediaStack, activeMediaStackItems, continueWatching, detailsExtraItems, images, isFiltered, items, recentResult.items, recentlyWatched, selectionMode, videos, watchLater]);
  const watchPlaybackQueue = useMemo(() => {
    if (playbackContext && !watchDebouncedSearch) {
      return playbackContext.filter((item) => item.mediaKind === "video" && !item.missing);
    }
    const browseVideos = watchBrowseResult.items.filter(
      (item) => item.mediaKind === "video" && !item.missing,
    );

    if (watchItem && !watchItem.missing && !browseVideos.some((item) => item.id === watchItem.id)) {
      return [watchItem, ...browseVideos];
    }

    return browseVideos;
  }, [playbackContext, watchBrowseResult.items, watchDebouncedSearch, watchItem]);
  const imageNavigationItems = useMemo(() => {
    const candidates = [...navigationItems];
    if (viewImage && !candidates.some((item) => item.id === viewImage.id)) candidates.unshift(viewImage);
    return Array.from(new Map(candidates.filter((item) => item.mediaKind === "image" && !item.missing).map((item) => [item.id, item])).values());
  }, [navigationItems, viewImage]);
  const fullWatchVisible = Boolean(active && watchItem && !watchMinimized);
  const detailsNavigationItems = fullWatchVisible ? watchPlaybackQueue : viewImage ? imageNavigationItems : navigationItems;
  const continueProgress = useMemo(() => new Map(continueWatching.map((entry) => [entry.item.id, Math.min(100, entry.playback.positionSeconds / entry.playback.durationSeconds * 100)])), [continueWatching]);

  useEffect(() => {
    if (selectedItem && !detailsNavigationItems.some((item) => item.id === selectedItem.id)) {
      closeDetails();
    }
  }, [detailsNavigationItems, selectedItem]);

  useEffect(() => {
    if (!selectedItem) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      if (event.key === "Escape") {
        event.preventDefault();
        closeDetails();
        return;
      }
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      const index = detailsNavigationItems.findIndex((item) => item.id === selectedItem.id);
      if (index < 0) return;
      const next = event.key === "ArrowRight" ? index + 1 : index - 1;
      if (next >= 0 && next < detailsNavigationItems.length) {
        event.preventDefault();
        setSelectedItem(detailsNavigationItems[next]);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [detailsNavigationItems, selectedItem]);

  function updateItem(changed: LibraryItem) {
    setManageItems((current) => current.map((item) => item.id === changed.id ? changed : item));
    setSelectedItem((current) => current?.id === changed.id ? changed : current);
    setWatchItem((current) => current?.id === changed.id ? changed : current);
    setViewImage((current) => current?.id === changed.id ? changed : current);
    scheduleLibraryRefresh(0);
  }

  function updateAutoplayNext(enabled: boolean) {
    setAutoplayNext(enabled);
    window.localStorage.setItem("nice-downloader.library-autoplay", String(enabled));
  }

  function removeItem(id: string) {
    setManageItems((current) => current.filter((item) => item.id !== id));
    if (selectedItem?.id === id) closeDetails();
    setWatchItem((current) => current?.id === id ? null : current);
    if (viewImage?.id === id) closeImageViewer();
    setPlaylists((current) => current.map((playlist) => ({
      ...playlist,
      itemIds: playlist.itemIds.filter((itemId) => itemId !== id),
    })));
    setActivePlaylistItems((current) => current.filter((item) => item.id !== id));
    setPlaybackContext((current) => current?.filter((item) => item.id !== id) || null);
    scheduleLibraryRefresh(0);
  }

  function updateItems(changed: LibraryItem[]) {
    const byId = new Map(changed.map((item) => [item.id, item]));
    setManageItems((current) => current.map((item) => byId.get(item.id) || item));
    setSelectedItem((current) => current ? byId.get(current.id) || current : null);
    setWatchItem((current) => current ? byId.get(current.id) || current : null);
    setViewImage((current) => current ? byId.get(current.id) || current : null);
    scheduleLibraryRefresh(0);
  }

  function applyFileOperationResult(result: LibraryFileOperationResult) {
    setFileOperations((current) => upsertById(current, result.operation));
    setManageItems((current) => {
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
    setWatchItem((current) => current ? result.items.find((item) => item.id === current.id) || current : null);
    setViewImage((current) => current ? result.items.find((item) => item.id === current.id) || current : null);
    void api.listLibrarySources().then(setSources).catch((error) => setError(normalizeError(error)));
    scheduleLibraryRefresh(0);
  }

  function applyBulkMoveResult(result: LibraryBulkMoveResult) {
    const movedItemIds = new Set(result.results.map((entry) => entry.operation.itemId));
    const changedItems = result.results.flatMap((entry) => entry.items);
    const changedById = new Map(changedItems.map((item) => [item.id, item]));
    setManageItems((current) => current.map((item) => changedById.get(item.id) || item));
    setSelectedItem((current) => current ? changedById.get(current.id) || current : null);
    setWatchItem((current) => current ? changedById.get(current.id) || current : null);
    setViewImage((current) => current ? changedById.get(current.id) || current : null);
    setSelectedIds((current) => new Set(Array.from(current).filter((id) => !movedItemIds.has(id))));
    void Promise.all([api.listLibrarySources(), api.listLibraryFileOperations()])
      .then(([nextSources, nextOperations]) => {
        setSources(nextSources);
        setFileOperations(nextOperations);
      })
      .catch((error) => setError(normalizeError(error)));
    scheduleLibraryRefresh(0);
    const moved = result.results.length;
    const failed = result.failures.length;
    if (failed === 0) setSelectionMode(false);
    setNotice({
      message: failed > 0
        ? `Moved ${moved} item${moved === 1 ? "" : "s"}; ${failed} could not be moved and remain selected.`
        : `Moved ${moved} item${moved === 1 ? "" : "s"} to the selected folder.`,
      view: "library",
    });
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
    if (bulkMoveOpen) closeBulkMove();
    setSelectionMode(false);
    setSelectedIds(new Set());
  }

  async function openCompareWorkspace() {
    const itemIds = Array.from(selectedIds);
    if (itemIds.length < 2 || itemIds.length > 4 || busy) return;
    setBusy(true);
    try {
      const selected = await api.getLibraryItemsByIds(itemIds);
      const selectedById = new Map(selected.map((item) => [item.id, item]));
      const ordered = itemIds.map((id) => selectedById.get(id)).filter((item): item is LibraryItem => Boolean(item));
      if (ordered.length !== itemIds.length) {
        setNotice({ message: "One or more selected records no longer exist. Refresh the Library and choose the videos again.", view: "library" });
        scheduleLibraryRefresh(0);
        return;
      }
      if (ordered.some((item) => item.mediaKind !== "video")) {
        setNotice({ message: "Video comparison accepts videos only. Remove images from the selection and try again.", view: "library" });
        return;
      }
      if (ordered.some((item) => item.missing)) {
        setNotice({ message: "Unavailable videos cannot be compared. Restore or remove the missing records first.", view: "library" });
        return;
      }
      setCompareItems(itemIds);
      setWatchItem(null);
      setPlaybackContext(null);
      setViewImage(null);
      imageReturnFocus.current = null;
      setSelectedItem(null);
      setDetailsExtraItems([]);
      closeSelectionMode();
      window.dispatchEvent(new Event("nice-downloader:compare-start"));
      setView("compare");
      window.setTimeout(() => document.getElementById("compare-workspace-heading")?.focus(), 0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function openImageCompareWorkspace() {
    const itemIds = Array.from(selectedIds);
    if (itemIds.length < 2 || itemIds.length > 4 || busy) return;
    setBusy(true);
    try {
      const selected = await api.getLibraryItemsByIds(itemIds);
      const selectedById = new Map(selected.map((item) => [item.id, item]));
      const ordered = itemIds.map((id) => selectedById.get(id)).filter((item): item is LibraryItem => Boolean(item));
      if (ordered.length !== itemIds.length) {
        setNotice({ message: "One or more selected records no longer exist. Refresh the Library and choose the images again.", view: "library" });
        scheduleLibraryRefresh(0);
        return;
      }
      if (ordered.some((item) => item.mediaKind !== "image")) {
        setNotice({ message: "Image comparison accepts images only. Remove videos from the selection and try again.", view: "library" });
        return;
      }
      if (ordered.some((item) => item.missing)) {
        setNotice({ message: "Unavailable images cannot be compared. Restore or remove the missing records first.", view: "library" });
        return;
      }
      setImageCompareItems(itemIds);
      setWatchItem(null);
      setPlaybackContext(null);
      setViewImage(null);
      imageReturnFocus.current = null;
      setSelectedItem(null);
      setDetailsExtraItems([]);
      closeSelectionMode();
      window.dispatchEvent(new Event("nice-downloader:image-compare-start"));
      setView("imageCompare");
      window.setTimeout(() => document.getElementById("image-compare-heading")?.focus(), 0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  function resetFilters() {
    setMediaFilter("all"); setSourceFilter("all"); setFolderFilter("all"); setFolderScope("tree"); setResolutionFilter("all"); setContainerFilter("all");
    setDurationFilter("all"); setDateFilter("all"); setPublishedFilter("all"); setAvailabilityFilter("available"); setTagFilters([]); setCreatorFilters([]);
    setSeriesFilters([]); setSiteFilters([]);
    setRatingFilter("all"); setCollectionFilter("all"); setFavoriteOnly(false);
    setSort("recent");
  }

  function applySmartView(view: LibrarySmartView) {
    const query = view.query;
    setActivePlaylist(null);
    setActivePlaylistItems([]);
    setSearch(query.search || "");
    setSourceFilter(query.sourceId || "all");
    setFolderFilter(query.folderSourceId && query.folderPath != null ? folderFilterValue(query.folderSourceId, query.folderPath) : "all");
    setFolderScope(query.folderScope);
    setMediaFilter(query.mediaKind || "all");
    setResolutionFilter((query.resolution || "all") as ResolutionFilter);
    setContainerFilter(query.container || "all");
    setDurationFilter((query.duration || "all") as DurationFilter);
    setDateFilter((query.dateAdded || "all") as DateFilter);
    setPublishedFilter((query.datePublished || "all") as DateFilter);
    setAvailabilityFilter(query.availability);
    setTagFilters(query.tags);
    setCreatorFilters(query.creators);
    setSeriesFilters(query.series);
    setSiteFilters(query.sites);
    setRatingFilter(query.rating || "all");
    setCollectionFilter(query.collectionId || "all");
    setFavoriteOnly(query.favoriteOnly);
    setSort(query.sort);
    setResultPage(1);
    setActiveSmartViewId(view.id);
  }

  async function saveCurrentSmartView() {
    const name = smartViewName.trim();
    if (!name || smartViewBusy) return;
    setSmartViewBusy(true);
    try {
      const stored = await api.saveLibrarySmartView({ name, query: currentSmartViewQuery });
      smartViewsRequestRevision.current += 1;
      setSmartViews((current) => upsertById(current, stored).sort((left, right) => left.name.localeCompare(right.name)));
      applySmartView(stored);
      setSmartViewName("");
      setSmartViewFormOpen(false);
      setNotice({ message: `Saved “${stored.name}” as an automatic collection.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSmartViewBusy(false);
    }
  }

  async function deleteSmartView(id: string) {
    if (smartViewBusy) return;
    if (confirmDeleteSmartViewId !== id) {
      setConfirmDeleteSmartViewId(id);
      return;
    }
    setSmartViewBusy(true);
    try {
      await api.deleteLibrarySmartView(id);
      smartViewsRequestRevision.current += 1;
      setSmartViews((current) => current.filter((view) => view.id !== id));
      if (activeSmartViewId === id) setActiveSmartViewId(null);
      setConfirmDeleteSmartViewId(null);
      setNotice({ message: "Automatic collection removed. Media files were not changed.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSmartViewBusy(false);
    }
  }

  async function rebuildSearchIndex() {
    if (smartViewBusy) return;
    setSmartViewBusy(true);
    try {
      const status = await api.rebuildLibrarySearchIndex();
      setSearchStatus(status);
      scheduleLibraryRefresh(0);
      setNotice({ message: `Library search rebuilt for ${status.indexedItems.toLocaleString()} items.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSmartViewBusy(false);
    }
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

  async function openPlaylist(playlist: LibraryPlaylist) {
    const revision = ++playlistLoadRevision.current;
    setActivePlaylist(playlist);
    setActivePlaylistItems([]);
    setPlaylistLoading(true);
    try {
      const items = await api.listLibraryPlaylistItems(playlist.id);
      if (revision !== playlistLoadRevision.current) return;
      setActivePlaylistItems(items);
      window.requestAnimationFrame(() => {
        document.getElementById("library-playlist-title")?.focus();
      });
    } catch (error) {
      if (revision !== playlistLoadRevision.current) return;
      setActivePlaylist(null);
      setError(normalizeError(error));
    } finally {
      if (revision === playlistLoadRevision.current) setPlaylistLoading(false);
    }
  }

  function closePlaylist() {
    playlistLoadRevision.current += 1;
    setPlaylistLoading(false);
    setActivePlaylist(null);
    setActivePlaylistItems([]);
    restoreFocus(null, "library-search");
  }

  async function openMediaStack(stack: LibraryMediaStack) {
    const revision = ++mediaStackLoadRevision.current;
    setActivePlaylist(null);
    setActiveMediaStack(stack);
    setActiveMediaStackItems([]);
    setMediaStackLoading(true);
    try {
      const loaded = await api.getLibraryItemsByIds(stack.members.map((member) => member.itemId));
      if (revision !== mediaStackLoadRevision.current) return;
      const byId = new Map(loaded.map((item) => [item.id, item]));
      setActiveMediaStackItems(stack.members.map((member) => byId.get(member.itemId)).filter((item): item is LibraryItem => Boolean(item)));
      window.requestAnimationFrame(() => document.getElementById("library-media-stack-title")?.focus());
    } catch (error) {
      if (revision !== mediaStackLoadRevision.current) return;
      setActiveMediaStack(null);
      setError(normalizeError(error));
    } finally {
      if (revision === mediaStackLoadRevision.current) setMediaStackLoading(false);
    }
  }

  function closeMediaStack() {
    mediaStackLoadRevision.current += 1;
    setMediaStackLoading(false);
    setActiveMediaStack(null);
    setActiveMediaStackItems([]);
    restoreFocus(null, "library-search");
  }

  async function openStackCreator() {
    const ids = Array.from(selectedIds);
    if (busy || ids.length < 2 || ids.length > 100) return;
    setBusy(true);
    try {
      const loaded = await api.getLibraryItemsByIds(ids);
      const byId = new Map(loaded.map((item) => [item.id, item]));
      const ordered = ids.map((id) => byId.get(id)).filter((item): item is LibraryItem => Boolean(item));
      const kind = ordered[0]?.mediaKind;
      if (ordered.length !== ids.length || ordered.some((item) => item.missing || !item.mediaKind)) {
        setNotice({ message: "Media stacks require available library items. Refresh the library and adjust the selection.", view: "library" });
        scheduleLibraryRefresh(0);
        return;
      }
      if (!kind || ordered.some((item) => item.mediaKind !== kind)) {
        setNotice({ message: "A media stack cannot mix videos and images.", view: "library" });
        return;
      }
      setStackDraftItems(ordered);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  function updatePlaylist(updated: LibraryPlaylist) {
    setPlaylists((current) => upsertById(current, updated));
    setActivePlaylist((current) => current?.id === updated.id ? updated : current);
    if (activePlaylist?.id === updated.id) {
      const revision = ++playlistLoadRevision.current;
      void api.listLibraryPlaylistItems(updated.id).then((items) => {
        if (revision !== playlistLoadRevision.current) return;
        setActivePlaylistItems(items);
        setPlaybackContext((current) => current
          ? items.filter((item) => item.mediaKind === "video" && !item.missing)
          : null);
      }).catch((error) => setError(normalizeError(error)));
    }
  }

  async function movePlaylistItem(itemId: string, newIndex: number) {
    if (!activePlaylist || playlistMutationBusy) return;
    setPlaylistMutationBusy(true);
    try {
      const updated = await api.moveLibraryPlaylistItem(activePlaylist.id, itemId, newIndex);
      const items = await api.listLibraryPlaylistItems(activePlaylist.id);
      setPlaylists((current) => upsertById(current, updated));
      setActivePlaylist(updated);
      setActivePlaylistItems(items);
      setPlaybackContext((current) => current
        ? items.filter((item) => item.mediaKind === "video" && !item.missing)
        : null);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setPlaylistMutationBusy(false);
    }
  }

  async function removePlaylistItem(itemId: string) {
    if (!activePlaylist || playlistMutationBusy) return;
    setPlaylistMutationBusy(true);
    try {
      const updated = await api.removeLibraryPlaylistItem(activePlaylist.id, itemId);
      setPlaylists((current) => upsertById(current, updated));
      setActivePlaylist(updated);
      setActivePlaylistItems((current) => current.filter((item) => item.id !== itemId));
      setPlaybackContext((current) => current?.filter((item) => item.id !== itemId) || null);
      setNotice({ message: "Removed video from playlist.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setPlaylistMutationBusy(false);
    }
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
      setNotice({ message: "Library backup exported with sources, collections, stacks, review decisions, and media metadata." });
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
      setNotice({ message: "Library backup merged safely. Existing records were not deleted.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function clearArtworkCache() {
    setBusy(true);
    try {
      const result = await api.clearLibraryArtworkCache();
      await refreshArtworkCacheStatus();
      setNotice({ message: result.removedFiles > 0 ? `Cleared ${result.removedFiles} cached image${result.removedFiles === 1 ? "" : "s"} (${formatBytes(result.freedBytes)}).` : "The artwork cache is already empty.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  const watchIndex = watchItem ? watchPlaybackQueue.findIndex((item) => item.id === watchItem.id) : -1;
  const orderedPlayback = Boolean(playbackContext && !watchDebouncedSearch);
  const previousVideo = watchItem && watchPlaybackQueue.length > 1
    ? watchIndex > 0
      ? watchPlaybackQueue[watchIndex - 1]
      : orderedPlayback ? null : watchPlaybackQueue.at(-1) || null
    : null;
  const nextVideo = watchItem && watchPlaybackQueue.length > 1
    ? watchIndex >= 0 && watchIndex + 1 < watchPlaybackQueue.length
      ? watchPlaybackQueue[watchIndex + 1]
      : orderedPlayback ? null : watchPlaybackQueue[0]
    : null;
  const autoNextVideo = watchIndex >= 0 && watchIndex + 1 < watchPlaybackQueue.length
    ? watchPlaybackQueue[watchIndex + 1]
    : null;
  const compactPlayer = Boolean(watchItem && (watchMinimized || !active));
  const imageIndex = viewImage ? imageNavigationItems.findIndex((item) => item.id === viewImage.id) : -1;
  const previousImage = imageIndex > 0 ? imageNavigationItems[imageIndex - 1] : null;
  const nextImage = imageIndex >= 0 && imageIndex + 1 < imageNavigationItems.length ? imageNavigationItems[imageIndex + 1] : null;
  const watchWorkspace = watchItem && currentView !== "compare" && currentView !== "imageCompare" ? <WatchWorkspace
    key={watchItem.id}
    item={watchItem}
    sourceName={sourceLabel(watchItem, sourceNames)}
    browseItems={watchPlaybackQueue}
    browseTotal={orderedPlayback ? watchPlaybackQueue.length : watchBrowseResult.totalItems}
    browsePage={orderedPlayback ? 1 : watchBrowseResult.page}
    browsePages={orderedPlayback ? 1 : watchBrowseResult.totalPages}
    browseLoading={watchBrowseLoading}
    search={watchSearch}
    sourceNames={sourceNames}
    compact={compactPlayer}
    autoplayNext={autoplayNext}
    playlists={playlists}
    onSearch={setWatchSearch}
    onBack={minimizeWatching}
    onRestore={restoreWatching}
    onClose={closeWatching}
    onDetails={() => openDetails(watchItem)}
    onSelectVideo={(item) => { setSelectedItem(null); setWatchItem(item); setWatchMinimized(false); }}
    onPrevious={previousVideo ? () => setWatchItem(previousVideo) : undefined}
    onNext={nextVideo ? () => setWatchItem(nextVideo) : undefined}
    onAutoplayNext={updateAutoplayNext}
    onPlaylistChanged={updatePlaylist}
    onPlaybackEnded={autoplayNext && autoNextVideo ? () => setWatchItem(autoNextVideo) : undefined}
    onBrowsePage={(page) => {
      if (!orderedPlayback) {
        setWatchBrowsePage(Math.min(Math.max(1, page), watchBrowseResult.totalPages));
      }
    }}
  /> : null;

  const imageWorkspace = viewImage && active ? <ImageWorkspace
    item={viewImage}
    sourceName={sourceLabel(viewImage, sourceNames)}
    position={imageIndex >= 0 ? imageIndex + 1 : 1}
    total={imageNavigationItems.length}
    shortcutsEnabled={!selectedItem}
    onBack={closeImageViewer}
    onDetails={() => openDetails(viewImage)}
    onPrevious={previousImage ? () => setViewImage(previousImage) : undefined}
    onNext={nextImage ? () => setViewImage(nextImage) : undefined}
  /> : null;

  if (manageOpen && active) {
    return (
      <><div className="mx-auto w-full max-w-[1440px] px-8 pb-14 pt-8 xl:px-10">
        <LibraryLiveRegions polite={libraryAnnouncement} assertive={libraryAlert} />
        <header className="mb-7 flex flex-wrap items-end justify-between gap-4">
          <div><p className="text-[11px] font-medium text-accent">Library settings</p><h1 className="mt-1 text-[30px] font-semibold tracking-[-0.04em]">Sources and catalog</h1><p className="mt-2 max-w-2xl text-[13px] leading-5 text-muted">Choose what Nice Downloader indexes. Removing a source here removes catalog records, never the original media.</p></div>
          <Button variant="secondary" onClick={() => setManageOpen(false)}><ChevronRight className="rotate-180" size={16} /> Back to library</Button>
        </header>

        <div className="mb-5 flex flex-wrap gap-2">
          <Button id="library-add-folder-button" onClick={() => showSourceForm ? closeSourceForm() : openSourceForm()}><FolderPlus size={16} /> Add folder</Button>
          <Button variant="secondary" onClick={() => void reconcile()}><FolderSearch size={16} /> Verify files</Button>
        </div>
        {showSourceForm ? <SourceForm onClose={closeSourceForm} onSaved={(source, job) => {
          setSources((current) => upsertById(current, source));
          setScanJobs((current) => upsertById(current, job));
          closeSourceForm();
        }} /> : null}
        <SourceSection
          sources={sources}
          scanJobs={scanJobs}
          onJob={(job) => setScanJobs((current) => upsertById(current, job))}
          onRemoved={(id) => {
            setSources((current) => current.filter((source) => source.id !== id));
            if (sourceFilter === id) setSourceFilter("all");
            scheduleLibraryRefresh(0);
          }}
        />
        <CollectionSection collections={collections} onChanged={setCollections} />
        <ReviewSection changes={changes} items={manageItems} sourceNames={sourceNames} onChanged={setChanges} />
        <DuplicateSection job={duplicateJob} groups={duplicateGroups} items={manageItems} onJob={setDuplicateJob} onTrashed={(itemId) => {
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
            <div className="flex items-start gap-3"><div className="grid size-10 place-items-center bg-elevated text-accent"><Save size={18} /></div><div><h2 className="text-sm font-semibold">Library backup</h2><p className="mt-1 text-xs leading-5 text-muted">Export sources, collections, stacks, curation, and inspected metadata. Restore merges records without deleting your current catalog.</p></div></div>
            <div className="mt-5 flex flex-wrap gap-2"><Button variant="secondary" onClick={() => void exportBackup()}><Save size={15} /> Export backup</Button><Button variant="secondary" onClick={() => void restoreBackup()}><ArchiveRestore size={15} /> Restore backup</Button></div>
          </div>
          <div className="bg-panel/55 p-5">
            <div className="flex items-start gap-3"><div className="grid size-10 place-items-center bg-elevated text-accent"><Database size={18} strokeWidth={1.8} /></div><div><h2 className="text-sm font-semibold">Artwork cache</h2><p className="mt-1 text-xs leading-5 text-muted">Previews stay in the app cache and the oldest are removed automatically before it exceeds its limits.</p>{artworkCacheStatus ? <p className="number-font mt-2 text-[10px] text-muted">{artworkCacheStatus.fileCount.toLocaleString()} preview{artworkCacheStatus.fileCount === 1 ? "" : "s"} · {formatBytes(artworkCacheStatus.totalBytes)} of {formatBytes(artworkCacheStatus.maxBytes)} · {artworkCacheStatus.maxFiles.toLocaleString()} file limit</p> : null}</div></div>
            <Button className="mt-5" variant="secondary" disabled={busy || artworkCacheStatus?.fileCount === 0} onClick={() => void clearArtworkCache()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Eraser size={15} strokeWidth={1.8} />} Clear cached artwork</Button>
          </div>
        </section>
      </div>{imageWorkspace}{watchWorkspace}</>
    );
  }

  return (
    <><div className={`${active && !viewImage && (!watchItem || compactPlayer) ? "" : "hidden"} mx-auto w-full max-w-[1520px] px-7 pb-16 pt-6 xl:px-9`}>
      <LibraryLiveRegions polite={libraryAnnouncement} assertive={libraryAlert} />
      <header className="sticky top-0 z-10 -mx-2 flex items-center gap-5 bg-canvas/90 px-2 py-3 backdrop-blur-xl">
        <div className="min-w-fit"><h1 className="text-[25px] font-semibold tracking-[-0.04em]">Your library</h1><p className="text-[11px] text-muted">{availableItemCount} available item{availableItemCount === 1 ? "" : "s"} across {sources.length} source{sources.length === 1 ? "" : "s"}{missingItemCount > 0 ? <button className="ml-1 font-medium text-amber-300/85 hover:text-amber-200" onClick={() => { resetFilters(); setAvailabilityFilter("missing"); }}>· {missingItemCount} missing</button> : null}</p></div>
        <label className="relative mx-auto w-full max-w-2xl"><span className="sr-only">Search library</span><Search className="absolute left-4 top-3.5 text-muted" size={17} /><Input id="library-search" className="h-12 rounded-[12px] bg-panel/85 pl-11 pr-4 text-sm shadow-control" value={search} onChange={(event) => { setActivePlaylist(null); setSearch(event.target.value); }} placeholder="Search your videos, images, tags, and folders" /></label>
        <div className="flex shrink-0 items-center gap-1"><div className="flex rounded-[8px] bg-elevated/70 p-0.5" role="group" aria-label="Library view"><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "grid" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("grid")} aria-pressed={viewMode === "grid"} aria-label="Grid view"><LayoutGrid size={15} /></button><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "list" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("list")} aria-pressed={viewMode === "list"} aria-label="List view"><List size={15} /></button></div><Button variant={selectionMode ? "secondary" : "ghost"} onClick={() => selectionMode ? closeSelectionMode() : setSelectionMode(true)}>{selectionMode ? <X size={16} /> : <CheckSquare2 size={16} />} {selectionMode ? "Done" : "Select"}</Button><Button variant={changes.length > 0 ? "secondary" : "ghost"} onClick={() => { setManageOpen(true); void api.listLibraryFileOperations().then(setFileOperations).catch((error) => setError(normalizeError(error))); }}><Settings2 size={16} /> Manage{changes.length > 0 ? <span className="number-font rounded bg-accent/15 px-1.5 py-0.5 text-[10px] text-accent">{changes.length}</span> : null}</Button></div>
      </header>

      <section className="mt-3 flex flex-wrap items-center gap-2 border-y border-line/45 py-2.5" aria-label="Automatic collections">
        <span className="mr-1 inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.12em] text-muted"><Bookmark size={13} /> Automatic collections</span>
        {smartViews.length === 0 ? <span className="text-[11px] text-muted">Save the current filters as a collection that updates itself.</span> : smartViews.map((view) => <span key={view.id} className={`inline-flex items-center overflow-hidden rounded-[7px] border ${activeSmartViewId === view.id ? "border-accent/35 bg-accent/12 text-accent" : "border-line/70 bg-elevated/50 text-muted"}`}><button className="h-7 max-w-[220px] truncate px-2.5 text-[11px] font-medium hover:text-ink" onClick={() => applySmartView(view)} title={`Open ${view.name}`}>{view.name}</button><button className={`grid size-7 place-items-center border-l transition ${confirmDeleteSmartViewId === view.id ? "border-red-400/30 bg-red-400/10 text-red-300" : "border-line/60 hover:bg-panel hover:text-ink"}`} disabled={smartViewBusy} onClick={() => void deleteSmartView(view.id)} aria-label={confirmDeleteSmartViewId === view.id ? `Confirm removing ${view.name}` : `Remove ${view.name}`} title={confirmDeleteSmartViewId === view.id ? "Click again to confirm" : "Remove automatic collection"}>{confirmDeleteSmartViewId === view.id ? <Trash2 size={12} /> : <X size={12} />}</button></span>)}
        <span className="ml-auto flex items-center gap-2">{smartViewFormOpen ? <><Input className="h-8 w-[190px] text-xs" value={smartViewName} maxLength={80} autoFocus placeholder="Collection name" onChange={(event) => setSmartViewName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void saveCurrentSmartView(); if (event.key === "Escape") { setSmartViewFormOpen(false); setSmartViewName(""); } }} /><Button className="h-8 text-xs" disabled={!smartViewName.trim() || smartViewBusy} onClick={() => void saveCurrentSmartView()}>{smartViewBusy ? <LoaderCircle className="animate-spin" size={13} /> : <Save size={13} />} Save</Button><Button variant="ghost" className="h-8 text-xs" onClick={() => { setSmartViewFormOpen(false); setSmartViewName(""); }}>Cancel</Button></> : <Button variant="ghost" className="h-8 text-xs" onClick={() => setSmartViewFormOpen(true)}><Bookmark size={13} /> Save automatic collection</Button>}<button className={`inline-flex h-8 items-center gap-1.5 rounded-[7px] px-2 text-[10px] transition hover:bg-elevated hover:text-ink ${searchStatus?.consistent === false ? "text-amber-300" : "text-muted"}`} disabled={smartViewBusy} onClick={() => void rebuildSearchIndex()} title="Rebuild the local Library search index"><Database size={12} />{searchStatus?.consistent === false ? "Repair search" : searchStatus ? "Search ready" : "Search index"}</button></span>
      </section>

      {searchStatus && !searchStatus.consistent ? <div className="mt-3 flex items-center gap-3 border-l-2 border-amber-400/60 bg-amber-400/5 px-3 py-2 text-xs text-amber-100" role="alert"><span>Search index has {searchStatus.indexedItems.toLocaleString()} of {searchStatus.catalogItems.toLocaleString()} Library items.</span><Button variant="secondary" className="ml-auto h-8 text-xs" disabled={smartViewBusy} onClick={() => void rebuildSearchIndex()}>{smartViewBusy ? <LoaderCircle className="animate-spin" size={13} /> : <RotateCcw size={13} />} Rebuild search</Button></div> : null}

      <nav className="mt-4 flex items-center gap-2 overflow-x-auto pb-2" aria-label="Library filters">
        <FilterChip active={!isFiltered && !activePlaylist} onClick={() => { setActivePlaylist(null); setActivePlaylistItems([]); resetFilters(); }}>Home</FilterChip>
        <FilterChip active={mediaFilter === "video"} onClick={() => setMediaFilter("video")}><Film size={14} /> Videos</FilterChip>
        <FilterChip active={mediaFilter === "image"} onClick={() => setMediaFilter("image")}><ImageIcon size={14} /> Images</FilterChip>
        <FilterChip active={favoriteOnly} onClick={() => setFavoriteOnly((value) => !value)}><Heart size={14} fill={favoriteOnly ? "currentColor" : "none"} /> Favorites</FilterChip>
        <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        {sources.map((source) => <FilterChip key={source.id} active={sourceFilter === source.id} onClick={() => { const next = sourceFilter === source.id ? "all" : source.id; setSourceFilter(next); const selectedFolder = parseFolderFilter(folderFilter); if (next === "all" || selectedFolder?.sourceId !== next) { setFolderFilter("all"); setFolderScope("tree"); } }}><span className="grid size-5 place-items-center rounded-[5px] bg-accent/12 text-[10px] font-bold text-accent">{source.name.charAt(0).toLocaleUpperCase()}</span>{source.name}</FilterChip>)}
        {collections.map((collection) => <FilterChip key={collection.id} active={collectionFilter === collection.id} onClick={() => setCollectionFilter(collectionFilter === collection.id ? "all" : collection.id)}><ListPlus size={14} />{collection.name}</FilterChip>)}
        <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        <label className="relative shrink-0"><span className="sr-only">Filter by source folder</span><FolderOpen className="pointer-events-none absolute left-3 top-2.5 text-muted" size={14} strokeWidth={1.8} /><Select className="h-9 w-[220px] bg-elevated/70 pl-9 text-xs" value={folderFilter} onChange={(event) => { const value = event.target.value; setFolderFilter(value); const folder = parseFolderFilter(value); if (!folder) setFolderScope("tree"); else if (folder.path === "") setFolderScope("direct"); }} disabled={folderOptions.length === 0}><option value="all">{folderOptions.length === 0 ? "No indexed folders" : "All folders"}</option>{folderOptions.map((option) => <option key={option.value} value={option.value}>{option.label} ({folderScope === "direct" ? option.directCount : option.treeCount})</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Choose whether nested folders are included</span><Select className="h-9 w-[145px] bg-elevated/70 text-xs" value={folderScope} onChange={(event) => setFolderScope(event.target.value as FolderScope)} disabled={folderFilter === "all"}><option value="tree">Include nested</option><option value="direct">This folder only</option></Select></label>
        <label className="relative shrink-0"><span className="sr-only">Filter by resolution</span><MonitorPlay className="pointer-events-none absolute left-3 top-2.5 text-muted" size={14} strokeWidth={1.8} /><Select className="h-9 w-[148px] bg-elevated/70 pl-9 text-xs" value={resolutionFilter} onChange={(event) => setResolutionFilter(event.target.value as ResolutionFilter)}><option value="all">Any resolution</option><option value="2160">4K and higher</option><option value="1440">1440p</option><option value="1080">1080p</option><option value="720">720p</option><option value="sd">Below 720p</option><option value="unknown">Not inspected</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by container</span><Select className="h-9 w-[130px] bg-elevated/70 text-xs" value={containerFilter} onChange={(event) => setContainerFilter(event.target.value)}><option value="all">Any format</option>{containers.map((container) => <option key={container} value={container}>{container.toLocaleUpperCase()}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by duration</span><Select className="h-9 w-[130px] bg-elevated/70 text-xs" value={durationFilter} onChange={(event) => setDurationFilter(event.target.value as DurationFilter)}><option value="all">Any duration</option><option value="short">Under 5 min</option><option value="medium">5–30 min</option><option value="long">Over 30 min</option><option value="unknown">Unknown time</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by date added</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={dateFilter} onChange={(event) => setDateFilter(event.target.value as DateFilter)}><option value="all">Any date</option><option value="week">This week</option><option value="month">This month</option><option value="year">This year</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by published date</span><Select className="h-9 w-[145px] bg-elevated/70 text-xs" value={publishedFilter} onChange={(event) => setPublishedFilter(event.target.value as DateFilter)}><option value="all">Any published date</option><option value="week">Published this week</option><option value="month">Published this month</option><option value="year">Published this year</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by availability</span><Select className="h-9 w-[130px] bg-elevated/70 text-xs" value={availabilityFilter} onChange={(event) => setAvailabilityFilter(event.target.value as AvailabilityFilter)}><option value="available">Available</option><option value="missing">Missing files</option><option value="all">All records</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Add tag filter</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value="" onChange={(event) => { const value = event.target.value; if (value && !tagFilters.includes(value)) setTagFilters((current) => [...current, value]); }}><option value="">Add tag…</option>{tags.filter((tag) => !tagFilters.includes(tag)).map((tag) => <option key={tag} value={tag}>#{tag}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Add creator filter</span><Select className="h-9 w-[170px] bg-elevated/70 text-xs" value="" onChange={(event) => { const value = event.target.value; if (value && !creatorFilters.includes(value)) setCreatorFilters((current) => [...current, value]); }}><option value="">Add creator…</option>{creators.filter((creator) => !creatorFilters.includes(creator)).map((creator) => <option key={creator} value={creator}>{creator}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Add series filter</span><Select className="h-9 w-[170px] bg-elevated/70 text-xs" value="" onChange={(event) => { const value = event.target.value; if (value && !seriesFilters.includes(value)) setSeriesFilters((current) => [...current, value]); }}><option value="">Add series…</option>{seriesOptions.filter((series) => !seriesFilters.includes(series)).map((series) => <option key={series} value={series}>{series}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Add site filter</span><Select className="h-9 w-[145px] bg-elevated/70 text-xs" value="" onChange={(event) => { const value = event.target.value; if (value && !siteFilters.includes(value)) setSiteFilters((current) => [...current, value]); }}><option value="">Add site…</option>{sites.filter((site) => !siteFilters.includes(site)).map((site) => <option key={site} value={site}>{site}</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by rating</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={ratingFilter} onChange={(event) => setRatingFilter(event.target.value)}><option value="all">Any rating</option><option value="rated">Rated</option><option value="5">5 stars</option><option value="4">4 stars</option><option value="3">3 stars</option><option value="unrated">Unrated</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Sort library results</span><Select className="h-9 w-[175px] bg-elevated/70 text-xs" value={sort} onChange={(event) => setSort(event.target.value as LibrarySort)}><option value="recent">Newest added</option><option value="oldest">Oldest added</option><option value="publishedDesc">Newest published</option><option value="publishedAsc">Oldest published</option><option value="fileDateDesc">Newest file date</option><option value="fileDateAsc">Oldest file date</option><option value="titleAsc">Title A–Z</option><option value="titleDesc">Title Z–A</option><option value="durationDesc">Longest first</option><option value="durationAsc">Shortest first</option><option value="sizeDesc">Largest first</option><option value="sizeAsc">Smallest first</option></Select></label>
      </nav>

      {tagFilters.length + creatorFilters.length + seriesFilters.length + siteFilters.length > 0 ? <div className="mt-1 flex flex-wrap items-center gap-2" aria-label="Active Library filters"><span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-muted">Matching any within each group</span>{tagFilters.map((value) => <ActiveFilterChip key={`tag:${value}`} label={`#${value}`} onRemove={() => setTagFilters((current) => current.filter((item) => item !== value))} />)}{creatorFilters.map((value) => <ActiveFilterChip key={`creator:${value}`} label={`Creator: ${value}`} onRemove={() => setCreatorFilters((current) => current.filter((item) => item !== value))} />)}{seriesFilters.map((value) => <ActiveFilterChip key={`series:${value}`} label={`Series: ${value}`} onRemove={() => setSeriesFilters((current) => current.filter((item) => item !== value))} />)}{siteFilters.map((value) => <ActiveFilterChip key={`site:${value}`} label={`Site: ${value}`} onRemove={() => setSiteFilters((current) => current.filter((item) => item !== value))} />)}<button className="text-[11px] font-medium text-muted hover:text-ink" onClick={() => { setTagFilters([]); setCreatorFilters([]); setSeriesFilters([]); setSiteFilters([]); }}>Clear groups</button></div> : null}

      {activeMediaStack ? (
        <MediaStackWorkspace stack={activeMediaStack} items={activeMediaStackItems} loading={mediaStackLoading} onBack={closeMediaStack} onSelect={openDetails} onOpen={(item) => item.mediaKind === "video" ? startWatching(item, activeMediaStackItems.filter((candidate) => candidate.mediaKind === "video" && !candidate.missing)) : startViewingImage(item)} />
      ) : activePlaylist ? (
        <PlaylistWorkspace
          playlist={activePlaylist}
        items={activePlaylistItems}
        loading={playlistLoading}
        busy={playlistMutationBusy}
        sourceNames={sourceNames}
        onBack={closePlaylist}
          onPlay={(item) => startWatching(item, activePlaylistItems.filter((candidate) => candidate.mediaKind === "video" && !candidate.missing))}
          onDetails={openDetails}
          onMove={(itemId, newIndex) => void movePlaylistItem(itemId, newIndex)}
          onRemove={(itemId) => void removePlaylistItem(itemId)}
        />
      ) : queryLoading && facets.totalItems === 0 ? (
        <div className="mt-16 flex items-center justify-center gap-2 text-sm text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={17} /> Loading your library…</div>
      ) : facets.totalItems === 0 ? (
        <EmptyLibrary onAdd={() => { setManageOpen(true); openSourceForm(); }} />
      ) : isFiltered || selectionMode ? (
        <>{discoveryContext && !selectionMode ? <DiscoveryHeader kind={discoveryContext.kind} value={discoveryContext.value} totalItems={queryResult.totalItems} representative={queryResult.items[0]} onClear={() => discoveryContext.kind === "creator" ? setCreatorFilters([]) : setSeriesFilters([])} /> : null}<div id="library-results" className={`scroll-mt-24 transition-opacity ${queryLoading ? "opacity-55" : "opacity-100"}`} aria-busy={queryLoading}><MediaSection title={selectionMode ? "Select media" : resultTitle} subtitle={queryLoading ? "Updating results…" : queryResult.totalItems === 0 ? "No matching items" : selectionMode ? `${selectedIds.size} selected · showing ${resultStart + 1}–${resultEnd} of ${queryResult.totalItems}` : `Showing ${resultStart + 1}–${resultEnd} of ${queryResult.totalItems} matching item${queryResult.totalItems === 1 ? "" : "s"}`} items={items} sourceNames={sourceNames} onSelect={openDetails} selectionMode={selectionMode} selectedIds={selectedIds} onToggleSelection={toggleSelection} viewMode={viewMode} /></div><LibraryPagination currentPage={currentResultPage} pageCount={resultPageCount} totalItems={queryResult.totalItems} onPageChange={changeResultPage} /></>
      ) : (
        <>
          <MediaStackShelf stacks={mediaStacks} onOpen={(stack) => void openMediaStack(stack)} onChanged={setMediaStacks} />
          <PlaylistShelf playlists={playlists} onOpen={(playlist) => void openPlaylist(playlist)} onChanged={setPlaylists} />
          {featured ? <FeaturedMedia item={featured} sourceName={sourceLabel(featured, sourceNames)} onSelect={() => openDetails(featured)} onWatch={() => featured.mediaKind === "video" ? startWatching(featured) : startViewingImage(featured)} /> : null}
          {continueWatching.length > 0 ? <MediaSection title="Continue watching" subtitle="Pick up where you left off" items={continueWatching.map((entry) => entry.item)} progress={continueProgress} sourceNames={sourceNames} onSelect={startWatching} viewMode={viewMode} /> : null}
          {watchLater.length > 0 ? <MediaSection title="Watch later" subtitle="Videos you saved for another time" items={watchLater} sourceNames={sourceNames} onSelect={startWatching} viewMode={viewMode} /> : null}
          {recentlyWatched.length > 0 ? <MediaSection title="Recently watched" subtitle="Your private playback history on this device" items={recentlyWatched.map((entry) => entry.item)} sourceNames={sourceNames} onSelect={startWatching} viewMode={viewMode} /> : null}
          <MediaSection title="Recently added" subtitle="The newest media across your folders" items={recentResult.items} sourceNames={sourceNames} onSelect={openDetails} viewMode={viewMode} />
          {videos.length > 0 ? <MediaSection title="Videos" subtitle={`${videoResult.totalItems} video${videoResult.totalItems === 1 ? "" : "s"} ready to watch`} items={videos} sourceNames={sourceNames} onSelect={openDetails} viewMode={viewMode} /> : null}
          {images.length > 0 ? <MediaSection title="Images" subtitle={`${imageResult.totalItems} image${imageResult.totalItems === 1 ? "" : "s"} in your catalog`} items={images} sourceNames={sourceNames} onSelect={openDetails} viewMode={viewMode} /> : null}
        </>
      )}

{selectionMode ? <BulkActionBar selectedIds={selectedIds} collections={collections} busy={busy} onCompare={openCompareWorkspace} onCompareImages={openImageCompareWorkspace} onCreateStack={openStackCreator} onMove={openBulkMove} onApply={applyBulkAction} onClose={closeSelectionMode} /> : null}
      {bulkMoveOpen ? <BulkMoveDialog selectedIds={selectedIds} onClose={closeBulkMove} onComplete={(result) => { applyBulkMoveResult(result); closeBulkMove(); }} /> : null}
      {stackDraftItems ? <MediaStackDialog items={stackDraftItems} onClose={() => setStackDraftItems(null)} onSaved={(stack) => { setMediaStacks((current) => upsertById(current, stack)); setStackDraftItems(null); closeSelectionMode(); setNotice({ message: `Created media stack “${stack.name}”.`, view: "library" }); void openMediaStack(stack); }} /> : null}
</div>{imageWorkspace}{active && selectedItem ? <div className="relative z-[80]"><MediaDetails item={selectedItem} sourceName={sourceLabel(selectedItem, sourceNames)} onClose={closeDetails} onWatch={openSelectedMedia} onChanged={updateItem} onMetadataChanged={() => scheduleLibraryRefresh(0)} onBrowseCreator={(creator) => { closeDetails(false); resetFilters(); setCreatorFilters([creator]); setResultPage(1); }} onBrowseSeries={(series) => { closeDetails(false); resetFilters(); setSeriesFilters([series]); setResultPage(1); }} onBrowseSite={(site) => { closeDetails(false); resetFilters(); setSiteFilters([site]); setResultPage(1); }} onSelectRelated={openRelatedDetails} onRemoved={() => removeItem(selectedItem.id)} onOperation={applyFileOperationResult} /></div> : null}{watchWorkspace}</>
  );
}

function LibraryLiveRegions({ polite, assertive }: { polite: string; assertive: string }) {
  return <div className="sr-only">
    <p role="status" aria-live="polite" aria-atomic="true">{polite}</p>
    <p role="alert" aria-live="assertive" aria-atomic="true">{assertive}</p>
  </div>;
}

function libraryScanAnnouncement(job: LibraryScanJob): LiveAnnouncement {
  if (job.state === "completed") {
    return {
      signature: "completed",
      message: `Library scan completed. ${job.discoveredCount} files discovered, ${job.indexedCount} indexed, ${job.skippedCount} skipped, and ${job.warningCount} warnings.`,
    };
  }
  if (job.state === "cancelled") {
    return {
      signature: "cancelled",
      message: `Library scan cancelled after discovering ${job.discoveredCount} files and indexing ${job.indexedCount}.`,
    };
  }
  if (job.state === "failed") {
    return {
      signature: "failed",
      message: "Library scan failed. Open Library management for details and recovery actions.",
      assertive: true,
    };
  }
  const bucket = Math.floor(job.discoveredCount / 100);
  return {
    signature: `running:${bucket}`,
    message: job.discoveredCount === 0
      ? "Library scan started."
      : `Library scan in progress. ${job.discoveredCount} files discovered and ${job.indexedCount} indexed.`,
  };
}

function duplicateAnalysisAnnouncement(job: LibraryDuplicateAnalysisJob): LiveAnnouncement {
  if (job.state === "completed") {
    return {
      signature: "completed",
      message: `Duplicate analysis completed. ${job.groupsFound} exact duplicate group${job.groupsFound === 1 ? "" : "s"} found; ${job.hashedItems} files verified and ${job.skippedItems} skipped.`,
    };
  }
  if (job.state === "cancelled") {
    return {
      signature: "cancelled",
      message: `Duplicate analysis cancelled after checking ${job.hashedItems + job.skippedItems} files.`,
    };
  }
  if (job.state === "failed") {
    return {
      signature: "failed",
      message: "Duplicate analysis failed. Open Library management for details and recovery actions.",
      assertive: true,
    };
  }
  if (job.candidateItems === 0) {
    return { signature: "running:candidates", message: "Duplicate analysis started. Finding same-size candidates." };
  }
  const checked = job.hashedItems + job.skippedItems;
  const percent = Math.min(100, Math.floor(checked / job.candidateItems * 100));
  const bucket = Math.floor(percent / 20) * 20;
  return {
    signature: `running:${bucket}`,
    message: `Duplicate analysis ${bucket} percent complete. ${checked} of ${job.candidateItems} candidate files checked.`,
  };
}

function FilterChip({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return <button onClick={onClick} className={`flex h-9 shrink-0 items-center gap-2 rounded-[8px] px-3.5 text-xs font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${active ? "bg-ink text-canvas" : "bg-elevated/70 text-muted hover:bg-elevated hover:text-ink"}`} aria-pressed={active}>{children}</button>;
}

function ActiveFilterChip({ label, onRemove }: { label: string; onRemove: () => void }) {
  return <button className="inline-flex h-7 max-w-[260px] items-center gap-1.5 rounded-[7px] border border-accent/20 bg-accent/10 px-2.5 text-[11px] font-medium text-accent transition hover:border-accent/40 hover:bg-accent/15" onClick={onRemove} title={`Remove ${label}`}><span className="truncate">{label}</span><X size={12} aria-hidden="true" /><span className="sr-only">Remove filter</span></button>;
}

function DiscoveryHeader({ kind, value, totalItems, representative, onClear }: { kind: "creator" | "series"; value: string; totalItems: number; representative?: LibraryItem; onClear: () => void }) {
  const initial = value.trim().charAt(0).toLocaleUpperCase() || (kind === "creator" ? "C" : "S");
  return <section className="mb-6 mt-5 flex items-center gap-5 border-b border-line/55 pb-6" aria-labelledby="discovery-heading"><div className="grid size-20 shrink-0 place-items-center overflow-hidden rounded-full border border-accent/20 bg-accent/10 text-2xl font-semibold text-accent shadow-control">{representative ? <MediaArtwork item={representative} eager /> : initial}</div><div className="min-w-0"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-muted">{kind === "creator" ? "Creator channel" : "Series"}</p><h2 id="discovery-heading" className="mt-1 truncate text-[28px] font-semibold tracking-[-0.04em] text-ink">{value}</h2><p className="mt-1 text-xs text-muted">{totalItems.toLocaleString()} matching item{totalItems === 1 ? "" : "s"} · updates automatically when the Library is scanned</p></div><Button variant="ghost" className="ml-auto shrink-0" onClick={onClear}><X size={15} /> Close</Button></section>;
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

function FeaturedMedia({ item, sourceName, onSelect, onWatch }: { item: LibraryItem; sourceName: string; onSelect: () => void; onWatch: () => void }) {
  return <section className="relative mt-6 grid min-h-[330px] overflow-hidden bg-panel lg:grid-cols-[1.4fr_1fr]"><div className="absolute inset-0 opacity-20"><MediaArtwork item={item} eager /></div><div className="absolute inset-0 bg-gradient-to-r from-canvas via-canvas/95 to-canvas/25" /><div className="relative z-[1] flex flex-col justify-end px-7 py-9 lg:px-10"><p className="text-[11px] font-semibold text-accent">Recently added</p><h2 className="mt-3 max-w-2xl text-[34px] font-semibold leading-[1.08] tracking-[-0.045em] text-balance">{item.title}</h2><p className="mt-3 text-sm text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""} · {new Date(item.addedAt).toLocaleDateString()}</p><div className="mt-6 flex gap-2"><Button onClick={onWatch}><Play size={16} fill="currentColor" /> {item.mediaKind === "image" ? "View image" : "Watch"}</Button><Button variant="secondary" onClick={onSelect}>Details</Button></div></div><button className="group relative z-[1] m-6 hidden overflow-hidden rounded-[12px] bg-elevated shadow-[0_25px_70px_rgb(0_0_0/0.35)] lg:block" onClick={onWatch} aria-label={`Open ${item.title}`}><MediaArtwork item={item} eager /><span className="absolute inset-0 grid place-items-center bg-black/0 transition group-hover:bg-black/25"><span className="grid size-14 scale-90 place-items-center rounded-full bg-white/90 text-black opacity-0 shadow-xl transition group-hover:scale-100 group-hover:opacity-100"><Play size={22} fill="currentColor" /></span></span></button></section>;
}

function MediaStackShelf({ stacks, onOpen, onChanged }: { stacks: LibraryMediaStack[]; onOpen: (stack: LibraryMediaStack) => void; onChanged: (stacks: LibraryMediaStack[]) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (stacks.length === 0) return null;

  async function removeStack(id: string) {
    if (confirmDelete !== id) {
      setConfirmDelete(id);
      return;
    }
    setBusy(true);
    try {
      await api.deleteLibraryMediaStack(id);
      onChanged(stacks.filter((stack) => stack.id !== id));
      setConfirmDelete(null);
      setNotice({ message: "Removed the media stack. Its files were not changed.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  return <section className="mt-7 border-b border-line/60 pb-8" aria-labelledby="library-media-stacks-title"><div className="mb-4"><p className="text-[10px] font-medium uppercase tracking-[0.14em] text-accent">Visual organization</p><h2 id="library-media-stacks-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Media stacks</h2><p className="mt-1 text-xs text-muted">Keep originals, edits, alternates, and generated versions together without moving files.</p></div><div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">{stacks.map((stack) => <article key={stack.id} className="group flex min-h-28 items-center gap-4 rounded-[10px] border border-line/60 bg-panel/60 p-4 transition hover:border-accent/30 hover:bg-panel"><button className="relative grid size-14 shrink-0 place-items-center rounded-[9px] bg-accent/10 text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpen(stack)} aria-label={`Open ${stack.name}`}><Layers size={22} /><span className="number-font absolute -bottom-1 -right-1 min-w-5 rounded bg-ink px-1 py-0.5 text-[9px] text-canvas">{stack.members.length}</span></button><button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpen(stack)}><strong className="block truncate text-sm font-semibold">{stack.name}</strong><span className="mt-1 line-clamp-2 text-[10px] leading-4 text-muted">{stack.notes || `${stack.members.length} related versions`}</span><span className="number-font mt-2 block text-[10px] font-medium text-accent">{stack.members.length} item{stack.members.length === 1 ? "" : "s"}</span></button>{confirmDelete === stack.id ? <button className="text-[10px] text-muted hover:text-ink" onClick={() => setConfirmDelete(null)}>Keep</button> : null}<Button variant={confirmDelete === stack.id ? "danger" : "ghost"} className="h-8 px-2 text-xs opacity-0 group-hover:opacity-100 focus:opacity-100" disabled={busy} onClick={() => void removeStack(stack.id)}><Trash2 size={13} /> {confirmDelete === stack.id ? "Confirm" : "Remove"}</Button></article>)}</div></section>;
}

function MediaStackWorkspace({ stack, items, loading, onBack, onSelect, onOpen }: { stack: LibraryMediaStack; items: LibraryItem[]; loading: boolean; onBack: () => void; onSelect: (item: LibraryItem) => void; onOpen: (item: LibraryItem) => void }) {
  const memberById = new Map(stack.members.map((member) => [member.itemId, member]));
  return <section className="mt-5" aria-labelledby="library-media-stack-title"><header className="mb-6 flex flex-wrap items-center gap-4 border-b border-line/60 pb-5"><Button variant="ghost" className="px-2" onClick={onBack}><ChevronRight className="rotate-180" size={15} /> Library</Button><div className="grid size-11 place-items-center rounded-[10px] bg-accent/10 text-accent"><Layers size={20} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-accent">Media stack</p><h2 id="library-media-stack-title" tabIndex={-1} className="truncate text-2xl font-semibold tracking-[-0.035em] outline-none">{stack.name}</h2><p className="mt-1 text-xs text-muted">{stack.notes || "Related versions kept together"}</p></div><span className="number-font text-xs text-muted">{stack.members.length} items</span></header>{loading ? <div className="grid min-h-64 place-items-center text-sm text-muted"><span className="flex items-center gap-2"><LoaderCircle className="animate-spin text-accent" size={16} /> Loading stack…</span></div> : <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">{items.map((item) => { const member = memberById.get(item.id); return <article key={item.id} className="group overflow-hidden rounded-[10px] border border-line/60 bg-panel/60"><button className="relative aspect-video w-full overflow-hidden bg-elevated text-left" disabled={item.missing} onClick={() => onOpen(item)}><MediaArtwork item={item} eager /><span className="absolute left-2 top-2 rounded bg-black/70 px-2 py-1 text-[9px] font-semibold uppercase tracking-[0.1em] text-white">{mediaStackRoleLabel(member?.role || "other")}</span>{stack.coverItemId === item.id ? <span className="absolute right-2 top-2 rounded bg-accent px-2 py-1 text-[9px] font-semibold text-[#081426]">Cover</span> : null}</button><div className="flex items-start gap-3 p-3"><div className="min-w-0 flex-1"><h3 className="truncate text-xs font-semibold">{item.title}</h3><p className="mt-1 text-[10px] text-muted">{item.width && item.height ? `${item.width} × ${item.height}` : item.durationSeconds != null ? formatDuration(item.durationSeconds) : item.container?.toLocaleUpperCase() || "Media"}{item.missing ? " · Missing" : ""}</p></div><Button variant="ghost" className="h-8 px-2 text-xs" onClick={() => onSelect(item)}>Details</Button></div></article>; })}</div>}</section>;
}

function MediaStackDialog({ items, onClose, onSaved }: { items: LibraryItem[]; onClose: () => void; onSaved: (stack: LibraryMediaStack) => void }) {
  const setError = useAppStore((state) => state.setError);
  const [name, setName] = useState("");
  const [notes, setNotes] = useState("");
  const [coverItemId, setCoverItemId] = useState(items[0]?.id || "");
  const [originalItemId, setOriginalItemId] = useState(items[0]?.id || "");
  const [roles, setRoles] = useState<Record<string, LibraryMediaStackRole>>(() => Object.fromEntries(items.map((item, index) => [item.id, index === 0 ? "original" : "alternate"])));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.requestAnimationFrame(() => document.getElementById("media-stack-name")?.focus());
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  async function saveStack() {
    if (!name.trim() || !originalItemId || busy) return;
    setBusy(true);
    try {
      const stack = await api.saveLibraryMediaStack({
        name: name.trim(),
        notes: notes.trim(),
        coverItemId,
        members: items.map((item, position) => ({
          itemId: item.id,
          position,
          role: item.id === originalItemId ? "original" : roles[item.id] === "original" ? "alternate" : roles[item.id] || "other",
        })),
      });
      onSaved(stack);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  function changeRole(itemId: string, role: LibraryMediaStackRole) {
    if (role === "original") {
      setOriginalItemId(itemId);
      setRoles((current) => ({ ...current, [itemId]: "original", [originalItemId]: originalItemId === itemId ? "original" : "alternate" }));
    } else {
      setRoles((current) => ({ ...current, [itemId]: role }));
    }
  }

  return <div className="fixed inset-0 z-[100] grid place-items-center bg-black/70 p-5 backdrop-blur-sm" onMouseDown={() => { if (!busy) onClose(); }}><section className="panel-shadow flex max-h-[min(820px,calc(100vh-40px))] w-full max-w-3xl flex-col overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="media-stack-dialog-title" onMouseDown={(event) => event.stopPropagation()}><header className="flex items-start gap-4 border-b border-line/70 px-5 py-5"><div className="grid size-11 shrink-0 place-items-center rounded-[10px] bg-accent/10 text-accent"><Layers size={20} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-accent">Non-destructive organization</p><h2 id="media-stack-dialog-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Create media stack</h2><p className="mt-1 text-xs text-muted">Label related versions. Files remain in their current folders.</p></div><button className="grid size-8 place-items-center text-muted hover:bg-elevated hover:text-ink" disabled={busy} onClick={onClose} aria-label="Close"><X size={16} /></button></header><div className="min-h-0 flex-1 overflow-y-auto px-5 py-5"><div className="grid gap-4 sm:grid-cols-2"><Field label="Stack name"><Input id="media-stack-name" value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="Example: Poster revisions" /></Field><Field label="Cover item"><Select value={coverItemId} onChange={(event) => setCoverItemId(event.target.value)}>{items.map((item) => <option key={item.id} value={item.id}>{item.title}</option>)}</Select></Field></div><Field label="Notes"><Textarea className="mt-4" value={notes} maxLength={2000} onChange={(event) => setNotes(event.target.value)} placeholder="What connects these versions?" /></Field><div className="mt-5 space-y-2"><p className="text-xs font-semibold">Version roles</p>{items.map((item) => <div key={item.id} className="grid items-center gap-3 border border-line/60 bg-elevated/35 p-3 sm:grid-cols-[minmax(0,1fr)_160px]"><div className="min-w-0"><p className="truncate text-xs font-medium">{item.title}</p><p className="mt-0.5 truncate text-[10px] text-muted">{item.path}</p></div><Select className="h-9 text-xs" value={item.id === originalItemId ? "original" : roles[item.id] === "original" ? "alternate" : roles[item.id] || "other"} onChange={(event) => changeRole(item.id, event.target.value as LibraryMediaStackRole)}><option value="original">Original</option><option value="alternate">Alternate</option><option value="edit">Edit</option><option value="generated">Generated</option><option value="other">Other</option></Select></div>)}</div></div><footer className="flex items-center justify-end gap-2 border-t border-line/70 bg-elevated/25 px-5 py-4"><Button variant="ghost" disabled={busy} onClick={onClose}>Cancel</Button><Button disabled={busy || !name.trim()} onClick={() => void saveStack()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Layers size={15} />} Create stack</Button></footer></section></div>;
}

function mediaStackRoleLabel(role: LibraryMediaStackRole): string {
  return { original: "Original", alternate: "Alternate", edit: "Edit", generated: "Generated", other: "Other" }[role];
}

function PlaylistShelf({ playlists, onOpen, onChanged }: { playlists: LibraryPlaylist[]; onOpen: (playlist: LibraryPlaylist) => void; onChanged: (playlists: LibraryPlaylist[]) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  async function createPlaylist() {
    if (!name.trim() || busy) return;
    setBusy(true);
    try {
      const playlist = await api.saveLibraryPlaylist({ name: name.trim(), description: description.trim() });
      onChanged(await api.listLibraryPlaylists());
      setName("");
      setDescription("");
      setNotice({ message: `Created playlist ${playlist.name}.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function deletePlaylist(id: string) {
    if (busy) return;
    setBusy(true);
    try {
      await api.deleteLibraryPlaylist(id);
      onChanged(playlists.filter((playlist) => playlist.id !== id));
      setConfirmDelete(null);
      setNotice({ message: "Playlist removed. Original videos were not changed.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  return <section className="mt-7 border-b border-line/60 pb-8" aria-labelledby="library-playlists-title">
    <div className="mb-4 flex flex-wrap items-end justify-between gap-4"><div><p className="text-[10px] font-medium uppercase tracking-[0.14em] text-accent">Your queues</p><h2 id="library-playlists-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Playlists</h2><p className="mt-1 text-xs text-muted">Build an ordered video queue that continues without looping.</p></div><div className="grid w-full max-w-2xl gap-2 sm:grid-cols-[1fr_1.4fr_auto]"><Input className="h-9 text-xs" value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="Playlist name" onKeyDown={(event) => { if (event.key === "Enter") void createPlaylist(); }} /><Input className="h-9 text-xs" value={description} maxLength={500} onChange={(event) => setDescription(event.target.value)} placeholder="Optional description" /><Button className="h-9" disabled={busy || !name.trim()} onClick={() => void createPlaylist()}>{busy ? <LoaderCircle className="animate-spin" size={14} /> : <ListPlus size={14} />} Create</Button></div></div>
    {playlists.length === 0 ? <div className="border border-dashed border-line/70 px-5 py-6 text-center text-xs text-muted">No playlists yet. Create one, then add videos from the player.</div> : <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">{playlists.map((playlist) => <article key={playlist.id} className="group flex min-h-28 items-center gap-4 rounded-[10px] border border-line/60 bg-panel/60 p-4 transition hover:border-accent/30 hover:bg-panel"><button className="grid size-14 shrink-0 place-items-center rounded-[9px] bg-accent/10 text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpen(playlist)} aria-label={`Open ${playlist.name}`}><ListPlus size={22} /></button><button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpen(playlist)}><strong className="block truncate text-sm font-semibold">{playlist.name}</strong><span className="mt-1 line-clamp-2 text-[10px] leading-4 text-muted">{playlist.description || `${playlist.itemIds.length} ordered video${playlist.itemIds.length === 1 ? "" : "s"}`}</span><span className="number-font mt-2 block text-[10px] font-medium text-accent">{playlist.itemIds.length} video{playlist.itemIds.length === 1 ? "" : "s"}</span></button>{confirmDelete === playlist.id ? <div className="flex flex-col gap-1"><button className="text-[10px] text-muted hover:text-ink" onClick={() => setConfirmDelete(null)}>Keep</button><button className="text-[10px] font-semibold text-red-300 hover:text-red-200" onClick={() => void deletePlaylist(playlist.id)}>Delete</button></div> : <button className="grid size-8 place-items-center rounded-[6px] text-muted opacity-0 transition hover:bg-elevated hover:text-red-300 focus:opacity-100 group-hover:opacity-100" onClick={() => setConfirmDelete(playlist.id)} aria-label={`Delete ${playlist.name}`}><Trash2 size={14} /></button>}</article>)}</div>}
  </section>;
}

function PlaylistWorkspace({ playlist, items, loading, busy, sourceNames, onBack, onPlay, onDetails, onMove, onRemove }: { playlist: LibraryPlaylist; items: LibraryItem[]; loading: boolean; busy: boolean; sourceNames: Map<string, string>; onBack: () => void; onPlay: (item: LibraryItem) => void; onDetails: (item: LibraryItem) => void; onMove: (itemId: string, newIndex: number) => void; onRemove: (itemId: string) => void }) {
  const firstAvailable = items.find((item) => item.mediaKind === "video" && !item.missing);
  return <section className="mt-8" aria-labelledby="library-playlist-title">
    <header className="mb-6 flex flex-wrap items-end justify-between gap-4 border-b border-line/60 pb-6"><div className="flex items-start gap-4"><button className="mt-1 grid size-9 place-items-center rounded-[7px] bg-elevated text-muted hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onBack} aria-label="Back to library"><ChevronRight className="rotate-180" size={16} /></button><div><p className="text-[10px] font-medium uppercase tracking-[0.14em] text-accent">Playlist</p><h2 id="library-playlist-title" tabIndex={-1} className="mt-1 text-[30px] font-semibold tracking-[-0.04em] outline-none">{playlist.name}</h2><p className="mt-2 max-w-2xl text-xs leading-5 text-muted">{playlist.description || "An ordered local playback queue."}</p><p className="number-font mt-2 text-[10px] text-muted">{items.length} video{items.length === 1 ? "" : "s"}</p></div></div><Button disabled={!firstAvailable} onClick={() => firstAvailable && onPlay(firstAvailable)}><Play size={15} fill="currentColor" /> Play all</Button></header>
    {loading ? <div className="flex items-center justify-center gap-2 py-16 text-sm text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={17} /> Loading playlist…</div> : items.length === 0 ? <div className="border border-dashed border-line/70 px-6 py-14 text-center"><ListPlus className="mx-auto text-muted/40" size={30} /><p className="mt-3 text-sm font-semibold">This playlist is empty</p><p className="mt-1 text-xs text-muted">Open a video and choose Add to playlist.</p></div> : <ol className="space-y-2" aria-busy={busy}>{items.map((item, index) => <li key={item.id} className="grid grid-cols-[36px_150px_minmax(0,1fr)_auto] items-center gap-3 rounded-[10px] border border-line/55 bg-panel/55 p-2.5"><span className="number-font text-center text-xs text-muted">{index + 1}</span><button className="relative aspect-video overflow-hidden rounded-[7px] bg-elevated focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" disabled={item.missing} onClick={() => onPlay(item)}><MediaArtwork item={item} /></button><button className="min-w-0 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onDetails(item)}><strong className="block truncate text-xs font-semibold">{item.title}</strong><span className="mt-1 block truncate text-[10px] text-muted">{item.missing ? "Missing file" : `${sourceLabel(item, sourceNames)}${item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}`}</span></button><div className="flex items-center gap-1"><button className="grid size-8 place-items-center rounded-[6px] text-muted hover:bg-elevated hover:text-ink disabled:opacity-30" disabled={busy || index === 0} onClick={() => onMove(item.id, index - 1)} aria-label={`Move ${item.title} up`}><ArrowRight className="-rotate-90" size={14} /></button><button className="grid size-8 place-items-center rounded-[6px] text-muted hover:bg-elevated hover:text-ink disabled:opacity-30" disabled={busy || index + 1 === items.length} onClick={() => onMove(item.id, index + 1)} aria-label={`Move ${item.title} down`}><ArrowRight className="rotate-90" size={14} /></button><button className="grid size-8 place-items-center rounded-[6px] text-muted hover:bg-elevated hover:text-red-300 disabled:opacity-30" disabled={busy} onClick={() => onRemove(item.id)} aria-label={`Remove ${item.title} from playlist`}><X size={14} /></button></div></li>)}</ol>}
  </section>;
}

function MediaSection({ title, subtitle, items, sourceNames, onSelect, progress, selectionMode = false, selectedIds = new Set(), onToggleSelection, viewMode = "grid" }: { title: string; subtitle: string; items: LibraryItem[]; sourceNames: Map<string, string>; onSelect: (item: LibraryItem) => void; progress?: Map<string, number>; selectionMode?: boolean; selectedIds?: Set<string>; onToggleSelection?: (id: string) => void; viewMode?: "grid" | "list" }) {
  return <section className="mt-10" aria-labelledby={`library-${title.replace(/\W+/g, "-").toLocaleLowerCase()}`}><div className="mb-4 flex items-end justify-between gap-4"><div><h2 id={`library-${title.replace(/\W+/g, "-").toLocaleLowerCase()}`} className="text-[20px] font-semibold tracking-[-0.03em]">{title}</h2><p className="mt-1 text-[12px] text-muted">{subtitle}</p></div></div>{items.length > 0 ? <div className={viewMode === "grid" ? "grid gap-x-4 gap-y-7 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4" : "grid gap-2"}>{items.map((item) => <MediaCard key={item.id} item={item} sourceName={sourceLabel(item, sourceNames)} progress={progress?.get(item.id)} selected={selectedIds.has(item.id)} selectionMode={selectionMode} initialWatchLater={title === "Watch later"} viewMode={viewMode} onSelect={() => selectionMode ? onToggleSelection?.(item.id) : onSelect(item)} />)}</div> : <div className="border border-dashed border-line/70 px-6 py-12 text-center text-sm text-muted">Nothing matches these filters.</div>}</section>;
}

function MediaCard({ item, sourceName, onSelect, progress, selectionMode = false, selected = false, initialWatchLater = false, viewMode = "grid" }: { item: LibraryItem; sourceName: string; onSelect: () => void; progress?: number; selectionMode?: boolean; selected?: boolean; initialWatchLater?: boolean; viewMode?: "grid" | "list" }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const quality = resolutionLabel(item);
  const listView = viewMode === "list";
  const [watchLater, setWatchLater] = useState<boolean | null>(initialWatchLater ? true : null);
  const [watchLaterBusy, setWatchLaterBusy] = useState(false);

  async function toggleWatchLater() {
    if (watchLaterBusy) return;
    setWatchLaterBusy(true);
    try {
      const current = watchLater ?? (await api.getLibraryWatchState(item.id)).watchLater;
      const next = await api.setLibraryWatchLater(item.id, !current);
      setWatchLater(next.watchLater);
      setNotice({ message: next.watchLater ? "Saved to Watch Later." : "Removed from Watch Later." });
      window.dispatchEvent(new Event("nice-downloader:library-watch-state"));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setWatchLaterBusy(false);
    }
  }

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
          {progress != null ? <span className="absolute inset-x-0 bottom-0 h-1 bg-black/60"><span className="block h-full bg-accent" style={{ width: `${progress}%` }} /></span> : null}
        </div>
        <div className={`${listView ? "mt-0" : "mt-3"} flex min-w-0 gap-3`}><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-[11px] font-bold text-accent">{sourceName.charAt(0).toLocaleUpperCase()}</div><div className="min-w-0"><h3 className={`${listView ? "truncate text-sm" : "line-clamp-2 text-[13px]"} font-semibold leading-[1.35] text-ink`}>{item.title}</h3><p className="mt-1 truncate text-[11px] text-muted">{sourceName}{item.rating ? ` · ${item.rating} ★` : ""}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{listView && item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}{listView ? ` · ${new Date(item.addedAt).toLocaleDateString()}` : ""}</p>{listView && item.tags.length > 0 ? <p className="mt-2 truncate text-[10px] text-muted/80">{item.tags.map((tag) => `#${tag}`).join("  ")}</p> : null}</div></div>
      </button>
      {!selectionMode && item.mediaKind === "video" ? <button className={`mt-2 flex h-8 items-center gap-1.5 rounded-[7px] px-2 text-[10px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${watchLater ? "bg-accent/10 text-accent" : "text-muted hover:bg-elevated hover:text-ink"}`} disabled={watchLaterBusy} onClick={() => void toggleWatchLater()} aria-pressed={watchLater ?? undefined} aria-label={`${watchLater ? "Remove" : "Add"} ${item.title} ${watchLater ? "from" : "to"} Watch Later`}>{watchLaterBusy ? <LoaderCircle className="animate-spin" size={12} /> : <ListPlus size={12} />}{watchLater ? "Saved for later" : "Watch later"}</button> : null}
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

function ImageWorkspace({ item, sourceName, position, total, shortcutsEnabled, onBack, onDetails, onPrevious, onNext }: {
  item: LibraryItem;
  sourceName: string;
  position: number;
  total: number;
  shortcutsEnabled: boolean;
  onBack: () => void;
  onDetails: () => void;
  onPrevious?: () => void;
  onNext?: () => void;
}) {
  const setError = useAppStore((state) => state.setError);
  const [session, setSession] = useState<LibraryImageSession | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<{ itemId: string; message: string } | null>(null);
  const [reloadNonce, setReloadNonce] = useState(0);
  const [largeImageApprovedFor, setLargeImageApprovedFor] = useState<string | null>(null);
  const [fit, setFit] = useState(true);
  const [zoom, setZoom] = useState(1);
  const [rotation, setRotation] = useState(0);
  const [naturalSize, setNaturalSize] = useState<{ width: number; height: number } | null>(null);
  const [viewportSize, setViewportSize] = useState({ width: 0, height: 0 });
  const [slideshow, setSlideshow] = useState(false);
  const [slideshowSeconds, setSlideshowSeconds] = useState(5);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const pan = useRef<{ pointerId: number; x: number; y: number; left: number; top: number } | null>(null);
  const unusuallyLarge = (item.fileSizeBytes ?? 0) >= 100 * 1024 * 1024
    || (item.width ?? 0) * (item.height ?? 0) >= 100_000_000;
  const awaitingLargeApproval = unusuallyLarge && largeImageApprovedFor !== item.id;

  useEffect(() => {
    closeButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const updateSize = () => setViewportSize({
      width: Math.max(1, viewport.clientWidth - 64),
      height: Math.max(1, viewport.clientHeight - 64),
    });
    updateSize();
    const observer = new ResizeObserver(updateSize);
    observer.observe(viewport);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    let disposed = false;
    setSession(null);
    setLoading(true);
    setLoadError(null);
    setFit(true);
    setZoom(1);
    setRotation(0);
    setNaturalSize(null);
    if (unusuallyLarge && largeImageApprovedFor !== item.id) {
      setSlideshow(false);
      setLoading(false);
      return () => { disposed = true; };
    }
    void api.prepareLibraryImageView(item.id)
      .then((prepared) => { if (!disposed) setSession(prepared); })
      .catch((error) => {
        if (!disposed) {
          setLoadError({ itemId: item.id, message: normalizeError(error).message });
          setLoading(false);
        }
      });
    return () => { disposed = true; };
  }, [item.id, largeImageApprovedFor, reloadNonce, unusuallyLarge]);

  useEffect(() => {
    if (!slideshow) return;
    if (!onNext) {
      setSlideshow(false);
      return;
    }
    if (loading || session?.itemId !== item.id) return;
    const timer = window.setTimeout(onNext, slideshowSeconds * 1_000);
    return () => window.clearTimeout(timer);
  }, [loading, onNext, session, slideshow, slideshowSeconds]);

  function zoomBy(amount: number) {
    setFit(false);
    setZoom((current) => Math.min(8, Math.max(0.1, (fit ? 1 : current) + amount)));
  }

  function fitImage() {
    setFit(true);
    setZoom(1);
    viewportRef.current?.scrollTo({ top: 0, left: 0 });
  }

  function actualSize() {
    setFit(false);
    setZoom(1);
  }

  function rotateImage() {
    setRotation((current) => (current + 90) % 360);
  }

  async function openExternal() {
    try {
      await api.openLibraryItem(item.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  useEffect(() => {
    if (!shortcutsEnabled) return;
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("button, input, textarea, select, [contenteditable='true']")) return;
      const imageReady = session?.itemId === item.id;
      if (event.key === "Escape") { event.preventDefault(); onBack(); }
      else if (event.key === "ArrowLeft" && onPrevious) { event.preventDefault(); onPrevious(); }
      else if (event.key === "ArrowRight" && onNext) { event.preventDefault(); onNext(); }
      else if (imageReady && (event.key === "+" || event.key === "=")) { event.preventDefault(); zoomBy(0.25); }
      else if (imageReady && event.key === "-") { event.preventDefault(); zoomBy(-0.25); }
      else if (imageReady && event.key === "0") { event.preventDefault(); fitImage(); }
      else if (imageReady && event.key.toLocaleLowerCase() === "r") { event.preventDefault(); rotateImage(); }
      else if (imageReady && event.key === " ") { event.preventDefault(); setSlideshow((current) => !current && Boolean(onNext)); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [fit, item.id, onBack, onNext, onPrevious, session, shortcutsEnabled]);

  function startPan(event: React.PointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || fit) return;
    const viewport = viewportRef.current;
    if (!viewport) return;
    pan.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, left: viewport.scrollLeft, top: viewport.scrollTop };
    viewport.setPointerCapture(event.pointerId);
  }

  function movePan(event: React.PointerEvent<HTMLDivElement>) {
    const start = pan.current;
    const viewport = viewportRef.current;
    if (!start || !viewport || start.pointerId !== event.pointerId) return;
    viewport.scrollLeft = start.left - (event.clientX - start.x);
    viewport.scrollTop = start.top - (event.clientY - start.y);
  }

  function stopPan(event: React.PointerEvent<HTMLDivElement>) {
    if (pan.current?.pointerId !== event.pointerId) return;
    pan.current = null;
    if (viewportRef.current?.hasPointerCapture(event.pointerId)) viewportRef.current.releasePointerCapture(event.pointerId);
  }

  const quarterTurn = rotation % 180 !== 0;
  const rotatedNaturalWidth = naturalSize ? (quarterTurn ? naturalSize.height : naturalSize.width) : 0;
  const rotatedNaturalHeight = naturalSize ? (quarterTurn ? naturalSize.width : naturalSize.height) : 0;
  const fitScale = naturalSize && viewportSize.width > 0 && viewportSize.height > 0
    ? Math.min(1, viewportSize.width / rotatedNaturalWidth, viewportSize.height / rotatedNaturalHeight)
    : 1;
  const renderedWidth = naturalSize ? naturalSize.width * (fit ? fitScale : zoom) : 0;
  const renderedHeight = naturalSize ? naturalSize.height * (fit ? fitScale : zoom) : 0;
  const imageStyle = naturalSize ? {
    position: "absolute" as const,
    left: "50%",
    top: "50%",
    width: `${renderedWidth}px`,
    height: `${renderedHeight}px`,
    maxWidth: "none",
    maxHeight: "none",
    transform: `translate(-50%, -50%) rotate(${rotation}deg)`,
  } : { transform: `rotate(${rotation}deg)` };
  const imageFrameStyle = naturalSize ? {
    width: `${quarterTurn ? renderedHeight : renderedWidth}px`,
    height: `${quarterTurn ? renderedWidth : renderedHeight}px`,
  } : undefined;
  const currentSession = session?.itemId === item.id ? session : null;
  const currentError = loadError?.itemId === item.id ? loadError.message : null;
  const preparing = !awaitingLargeApproval && (loading || (!currentSession && !currentError));

  return <div className="mx-auto w-full max-w-[1640px] px-6 pb-12 pt-5 xl:px-8">
    <header className="sticky top-0 z-10 -mx-2 mb-5 flex items-center gap-4 bg-canvas/92 px-2 py-3 backdrop-blur-xl">
      <button ref={closeButtonRef} className="flex h-10 shrink-0 items-center gap-2 rounded-[8px] px-2.5 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onBack}><ChevronRight className="rotate-180" size={16} /> Library</button>
      <div className="h-6 w-px bg-line/70" />
      <div className="min-w-0 flex-1"><p className="text-[10px] font-medium text-accent">Viewing image {position} of {Math.max(1, total)}</p><h1 className="truncate text-sm font-semibold tracking-[-0.02em]">{item.title}</h1></div>
      <Button id="image-details-button" variant="secondary" onClick={onDetails}><Settings2 size={15} /> Details and organize</Button>
      <Button variant="ghost" onClick={() => void openExternal()}><ExternalLink size={15} /> Open in system viewer</Button>
    </header>

    <section className="panel-shadow overflow-hidden rounded-[12px] border border-line/70 bg-[#05070a]" aria-label={`Viewing ${item.title}`}>
      <div
        ref={viewportRef}
        className={`relative grid h-[clamp(300px,calc(100dvh-245px),760px)] min-h-0 place-items-center overflow-auto p-8 ${fit ? "cursor-default" : "cursor-grab active:cursor-grabbing"}`}
        onPointerDown={startPan}
        onPointerMove={movePan}
        onPointerUp={stopPan}
        onPointerCancel={stopPan}
      >
        {preparing ? <div className="absolute inset-0 z-[1] grid place-items-center bg-black/75 text-sm text-white/60" role="status"><span className="flex items-center gap-2"><LoaderCircle className="animate-spin text-accent" size={18} /> Loading full-resolution image…</span></div> : null}
        {awaitingLargeApproval ? <div className="max-w-lg border border-white/10 bg-white/5 p-7 text-center text-white"><FileImage className="mx-auto text-amber-300/70" size={36} /><h2 className="mt-4 text-lg font-semibold">Load this unusually large image?</h2><p className="mt-2 text-sm leading-6 text-white/55">Decoding {item.fileSizeBytes ? formatBytes(item.fileSizeBytes) : "this image"}{item.width && item.height ? ` at ${item.width} × ${item.height}` : ""} inside the WebView may use substantial memory. The original will not be modified.</p><div className="mt-5 flex flex-wrap justify-center gap-2"><Button variant="secondary" onClick={() => void openExternal()}><ExternalLink size={15} /> Open in system viewer</Button><Button onClick={() => setLargeImageApprovedFor(item.id)}><FileImage size={15} /> Load full image</Button></div></div> : null}
        {currentError ? <div className="max-w-lg border border-white/10 bg-white/5 p-7 text-center text-white"><FileImage className="mx-auto text-white/35" size={36} /><h2 className="mt-4 text-lg font-semibold">This image could not be displayed here</h2><p className="mt-2 text-sm leading-6 text-white/55">{currentError}</p><div className="mt-5 flex flex-wrap justify-center gap-2"><Button variant="secondary" onClick={() => setReloadNonce((current) => current + 1)}><RotateCcw size={15} /> Retry viewer</Button><Button onClick={() => void openExternal()}><ExternalLink size={15} /> Open in system viewer</Button></div></div> : null}
        {currentSession && !currentError ? <div className={naturalSize ? "relative shrink-0" : "contents"} style={imageFrameStyle}><img
          key={currentSession.url}
          src={currentSession.url}
          alt={item.title}
          draggable={false}
          className={`${naturalSize ? "select-none" : "max-h-full max-w-full object-contain"} [image-orientation:from-image] shadow-[0_28px_100px_rgb(0_0_0/0.58)] transition-transform motion-reduce:transition-none`}
          style={imageStyle}
          onLoad={(event) => { setNaturalSize({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight }); setLoading(false); }}
          onError={() => { setLoading(false); setLoadError({ itemId: item.id, message: "The WebView could not decode this image. The original file is unchanged and can still be opened in your system viewer." }); }}
        /></div> : null}
      </div>

      <footer className="flex flex-wrap items-center gap-2 border-t border-white/10 bg-[#0c0f14] px-4 py-3 text-white">
        <Button variant="ghost" className="h-9 text-white/70" disabled={!onPrevious} onClick={onPrevious}><ChevronRight className="rotate-180" size={15} /> Previous</Button>
        <Button variant="ghost" className="h-9 text-white/70" disabled={!onNext} onClick={onNext}>Next <ChevronRight size={15} /></Button>
        <span className="mx-1 h-5 w-px bg-white/10" />
        <button className="grid size-9 place-items-center rounded-[8px] text-white/70 hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 disabled:opacity-35" disabled={!currentSession} onClick={() => zoomBy(-0.25)} aria-label="Zoom out"><ZoomOut size={17} /></button>
        <span className="number-font min-w-12 text-center text-[10px] text-white/55">{fit ? "Fit" : `${Math.round(zoom * 100)}%`}</span>
        <button className="grid size-9 place-items-center rounded-[8px] text-white/70 hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 disabled:opacity-35" disabled={!currentSession} onClick={() => zoomBy(0.25)} aria-label="Zoom in"><ZoomIn size={17} /></button>
        <Button variant="ghost" className="h-9 text-white/70" disabled={!currentSession} onClick={fitImage}><Maximize size={15} /> Fit</Button>
        <Button variant="ghost" className="h-9 text-white/70" disabled={!currentSession} onClick={actualSize}>1:1</Button>
        <Button variant="ghost" className="h-9 text-white/70" disabled={!currentSession} onClick={rotateImage}><RotateCw size={15} /> Rotate</Button>
        <span className="mx-1 h-5 w-px bg-white/10" />
        <Button variant="ghost" className="h-9 text-white/70" disabled={!currentSession || (!onNext && !slideshow)} onClick={() => setSlideshow((current) => !current && Boolean(onNext))}>{slideshow ? <Pause size={15} /> : <Play size={15} />} {slideshow ? "Pause" : "Slideshow"}</Button>
        <label className="flex items-center gap-2 text-[10px] text-white/55"><span>Every</span><Select className="h-8 w-20 border-white/10 bg-white/5 text-xs text-white" value={slideshowSeconds} onChange={(event) => setSlideshowSeconds(Number(event.target.value))}><option value={3}>3 sec</option><option value={5}>5 sec</option><option value={10}>10 sec</option></Select></label>
        <p className="ml-auto text-[10px] text-white/40">←/→ browse · +/- zoom · 0 fit · R rotate · Space slideshow · Esc close</p>
      </footer>
    </section>
    <p className="mt-3 text-[11px] text-muted">{sourceName}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}{naturalSize ? ` · decoded ${naturalSize.width} × ${naturalSize.height}` : ""}</p>
  </div>;
}

function WatchWorkspace({ item, sourceName, browseItems, browseTotal, browsePage, browsePages, browseLoading, search, sourceNames, compact, autoplayNext, playlists, onSearch, onBack, onRestore, onClose, onDetails, onSelectVideo, onPrevious, onNext, onAutoplayNext, onPlaylistChanged, onPlaybackEnded, onBrowsePage }: {
  item: LibraryItem;
  sourceName: string;
  browseItems: LibraryItem[];
  browseTotal: number;
  browsePage: number;
  browsePages: number;
  browseLoading: boolean;
  search: string;
  sourceNames: Map<string, string>;
  compact: boolean;
  autoplayNext: boolean;
  playlists: LibraryPlaylist[];
  onSearch: (value: string) => void;
  onBack: () => void;
  onRestore: () => void;
  onClose: () => void;
  onDetails: () => void;
  onSelectVideo: (item: LibraryItem) => void;
  onPrevious?: () => void;
  onNext?: () => void;
  onAutoplayNext: (enabled: boolean) => void;
  onPlaylistChanged: (playlist: LibraryPlaylist) => void;
  onPlaybackEnded?: () => void;
  onBrowsePage: (page: number) => void;
}) {
  return <section className={compact ? "fixed bottom-5 right-5 z-[65] w-[min(420px,calc(100vw-32px))]" : "mx-auto w-full max-w-[1640px] px-6 pb-12 pt-5 xl:px-8"} aria-label={compact ? "Mini player" : "Watch workspace"}>
    <LibraryLiveRegions polite="" assertive="" />
    <header className={`${compact ? "hidden" : "sticky"} top-0 z-10 -mx-2 mb-5 flex items-center gap-4 bg-canvas/92 px-2 py-3 backdrop-blur-xl`}>
      <button autoFocus className="flex h-10 shrink-0 items-center gap-2 rounded-[8px] px-2.5 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onBack}><ChevronRight className="rotate-180" size={16} /> Library</button>
      <div className="h-6 w-px bg-line/70" />
      <div className="min-w-0"><p className="text-[10px] font-medium text-accent">Now watching</p><h1 className="max-w-xl truncate text-sm font-semibold tracking-[-0.02em]">{item.title}</h1></div>
      <label className="relative ml-auto w-full max-w-md"><span className="sr-only">Search videos while watching</span><Search className="absolute left-3.5 top-3 text-muted" size={15} /><Input className="h-10 rounded-[9px] bg-panel/85 pl-10 text-xs shadow-control" value={search} onChange={(event) => onSearch(event.target.value)} placeholder="Find another video" /></label>
    </header>

    <div className={compact ? "block" : "grid min-w-0 gap-6 xl:grid-cols-[minmax(0,1fr)_370px] 2xl:grid-cols-[minmax(0,1fr)_410px]"}>
      <article className="min-w-0">
        <EmbeddedVideoPlayer key={item.id} item={item} sourceName={sourceName} compact={compact} onRestore={onRestore} onClose={onClose} onPrevious={onPrevious} onNext={onNext} onPlaybackEnded={onPlaybackEnded} />
        <section className={`${compact ? "hidden" : "block"} mt-5 border-b border-line/60 pb-6`} aria-labelledby="watch-title">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div className="min-w-0 max-w-4xl"><h2 id="watch-title" className="text-[24px] font-semibold leading-tight tracking-[-0.035em] text-balance">{item.title}</h2><p className="mt-2 text-xs text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}</p></div>
            <div className="flex flex-wrap items-center gap-2"><WatchStateActions itemId={item.id} playlists={playlists} onPlaylistChanged={onPlaylistChanged} /><Button id="watch-details-button" variant="secondary" onClick={onDetails}><Settings2 size={15} /> Details and organize</Button></div>
          </div>
          {item.tags.length > 0 ? <div className="mt-4 flex flex-wrap gap-1.5">{item.tags.map((tag) => <span key={tag} className="rounded-[5px] bg-elevated px-2 py-1 text-[10px] font-medium text-muted">#{tag}</span>)}</div> : null}
          <p className="mt-4 max-w-[76ch] break-all text-[11px] leading-5 text-muted/80" title={item.path}>{item.relativePath || item.path}</p>
        </section>
      </article>

      <aside className={`${compact ? "hidden" : "flex"} panel-shadow max-h-[calc(100vh-104px)] min-h-[540px] flex-col overflow-hidden rounded-[12px] border border-line/70 bg-panel/65 xl:sticky xl:top-[76px] xl:self-start`} aria-label="Browse videos while watching">
        <div className="flex items-start justify-between gap-3 border-b border-line/60 px-4 py-4"><div><h2 className="text-sm font-semibold">{search.trim() ? "Search results" : "Up next"}</h2><p className="mt-1 text-[10px] text-muted">{browseLoading ? "Updating videos…" : `${browseTotal} available video${browseTotal === 1 ? "" : "s"}`}</p><p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{browseLoading ? "Updating watch browser results." : `Watch browser showing page ${browsePage} of ${browsePages}, with ${browseTotal} available videos.`}</p></div><div className="flex items-center gap-3">{!search.trim() ? <label className="flex items-center gap-2 text-[10px] font-medium text-muted"><span>Autoplay</span><input type="checkbox" checked={autoplayNext} onChange={(event) => onAutoplayNext(event.target.checked)} /></label> : null}{browsePages > 1 ? <span className="number-font text-[10px] text-muted">{browsePage}/{browsePages}</span> : null}</div></div>
        <div className="min-h-0 flex-1 overflow-y-auto p-2" aria-busy={browseLoading}>
          {browseLoading && browseItems.length === 0 ? <WatchQueueSkeleton /> : browseItems.length > 0 ? browseItems.map((candidate) => <WatchQueueItem key={candidate.id} item={candidate} sourceName={sourceLabel(candidate, sourceNames)} active={candidate.id === item.id} onSelect={() => onSelectVideo(candidate)} />) : <div className="px-5 py-12 text-center"><Film className="mx-auto text-muted/40" size={28} /><p className="mt-3 text-xs font-medium">No videos found</p><p className="mt-1 text-[10px] leading-4 text-muted">Try a title, tag, filename, or folder name.</p></div>}
        </div>
        {browsePages > 1 ? <nav className="flex items-center justify-between border-t border-line/60 px-3 py-2.5" aria-label="Watch browser pages"><button className="flex h-8 items-center gap-1 rounded-[7px] px-2 text-[10px] font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:opacity-35" disabled={browsePage <= 1 || browseLoading} onClick={() => onBrowsePage(browsePage - 1)}><ChevronRight className="rotate-180" size={13} /> Previous</button><button className="flex h-8 items-center gap-1 rounded-[7px] px-2 text-[10px] font-semibold text-muted transition hover:bg-elevated hover:text-ink disabled:opacity-35" disabled={browsePage >= browsePages || browseLoading} onClick={() => onBrowsePage(browsePage + 1)}>Next <ChevronRight size={13} /></button></nav> : null}
      </aside>
    </div>
  </section>;
}

function WatchStateActions({ itemId, playlists, onPlaylistChanged }: { itemId: string; playlists: LibraryPlaylist[]; onPlaylistChanged: (playlist: LibraryPlaylist) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [state, setState] = useState<LibraryWatchState | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let disposed = false;
    setState(null);
    void api.getLibraryWatchState(itemId)
      .then((next) => { if (!disposed) setState(next); })
      .catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [itemId, setError]);

  async function saveForLater() {
    if (!state || busy) return;
    setBusy(true);
    try {
      const next = await api.setLibraryWatchLater(itemId, !state.watchLater);
      setState(next);
      setNotice({ message: next.watchLater ? "Saved to Watch Later." : "Removed from Watch Later." });
      window.dispatchEvent(new Event("nice-downloader:library-watch-state"));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function changeWatched(watched: boolean) {
    if (busy) return;
    setBusy(true);
    try {
      const next = await api.setLibraryWatched(itemId, watched);
      setState(next);
      setNotice({ message: watched ? "Marked as watched." : "Playback history reset." });
      if (!watched) window.dispatchEvent(new CustomEvent("nice-downloader:library-reset-playback", { detail: { itemId } }));
      window.dispatchEvent(new Event("nice-downloader:library-watch-state"));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function addToPlaylist(playlistId: string) {
    if (!playlistId || busy) return;
    setBusy(true);
    try {
      const playlist = await api.addLibraryPlaylistItem(playlistId, itemId);
      onPlaylistChanged(playlist);
      setNotice({ message: `Added to ${playlist.name}.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  const completed = state?.playback?.completed === true;
  return <div className="flex flex-wrap gap-2" aria-label="Playback organization"><Button variant="secondary" disabled={!state || busy} onClick={() => void saveForLater()}><ListPlus size={15} /> {state?.watchLater ? "Saved for later" : "Watch later"}</Button>{playlists.length > 0 ? <label><span className="sr-only">Add video to playlist</span><Select className="h-10 min-w-44 text-xs" value="" disabled={busy} onChange={(event) => { const playlistId = event.target.value; event.currentTarget.value = ""; void addToPlaylist(playlistId); }}><option value="">Add to playlist…</option>{playlists.map((playlist) => <option key={playlist.id} value={playlist.id} disabled={playlist.itemIds.includes(itemId)}>{playlist.itemIds.includes(itemId) ? `Saved · ${playlist.name}` : playlist.name}</option>)}</Select></label> : null}<Button variant="secondary" disabled={!state || busy} onClick={() => void changeWatched(!completed)}><Check size={15} /> {completed ? "Mark unwatched" : "Mark watched"}</Button>{state?.playback && !completed ? <Button variant="ghost" disabled={busy} onClick={() => void changeWatched(false)}><RotateCcw size={14} /> Reset progress</Button> : null}</div>;
}

function WatchQueueItem({ item, sourceName, active, onSelect }: { item: LibraryItem; sourceName: string; active: boolean; onSelect: () => void }) {
  return <button className={`group grid w-full grid-cols-[128px_minmax(0,1fr)] gap-3 rounded-[9px] p-2 text-left transition active:scale-[0.99] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${active ? "bg-accent/[0.09] shadow-[inset_2px_0_rgb(var(--accent))]" : "hover:bg-elevated/75"}`} onClick={onSelect} aria-current={active ? "true" : undefined}>
    <span className="relative aspect-video overflow-hidden rounded-[7px] bg-elevated"><MediaArtwork item={item} />{item.durationSeconds != null ? <span className="number-font absolute bottom-1 right-1 rounded-[4px] bg-black/80 px-1.5 py-0.5 text-[9px] font-semibold text-white">{formatDuration(item.durationSeconds)}</span> : null}{active ? <span className="absolute inset-0 grid place-items-center bg-black/40"><span className="grid size-8 place-items-center rounded-full bg-accent text-[#081426]"><Play size={13} fill="currentColor" /></span></span> : null}</span>
    <span className="min-w-0 py-0.5"><strong className="line-clamp-2 text-[11px] font-semibold leading-[1.35] text-ink">{item.title}</strong><span className="mt-1.5 block truncate text-[9px] text-muted">{sourceName}</span><span className={`mt-1 block text-[9px] font-medium ${active ? "text-accent" : "text-muted/75"}`}>{active ? "Playing now" : [resolutionLabel(item), item.container?.toLocaleUpperCase()].filter(Boolean).join(" · ") || "Video"}</span></span>
  </button>;
}

function WatchQueueSkeleton() {
  return <div className="space-y-2 p-2" role="status" aria-label="Loading videos">{Array.from({ length: 5 }, (_, index) => <div key={index} className="grid animate-pulse grid-cols-[128px_1fr] gap-3 rounded-[9px] p-2"><div className="aspect-video rounded-[7px] bg-elevated" /><div className="py-1"><div className="h-2.5 w-full rounded bg-elevated" /><div className="mt-2 h-2.5 w-3/4 rounded bg-elevated" /><div className="mt-3 h-2 w-1/2 rounded bg-elevated/70" /></div></div>)}</div>;
}

function PlaybackPreparationStatus({ preparation, reencodeRequired, cancelling }: { preparation: LibraryPlaybackPreparationProgress | null; reencodeRequired: boolean; cancelling: boolean }) {
  const percent = preparation?.progressPercent != null ? Math.round(preparation.progressPercent) : null;
  const label = cancelling
    ? "Cancelling playback copy…"
    : preparation?.phase === "queued"
      ? "Waiting for a playback-copy slot…"
      : preparation?.phase === "finalizing"
        ? "Finalizing playback copy…"
        : reencodeRequired
          ? "Encoding H.264 playback copy…"
          : "Copying streams without re-encoding…";
  return <div className="space-y-2" role="status" aria-live="polite">
    <div className="flex items-center justify-between gap-3 text-xs text-accent"><span className="flex items-center gap-2"><LoaderCircle className="animate-spin" size={14} /> {label}</span>{percent != null ? <span className="number-font">{percent}%</span> : null}</div>
    <div className="h-1.5 overflow-hidden rounded-full bg-elevated" role="progressbar" aria-label="Playback copy preparation" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent ?? undefined}><div className={`h-full bg-accent transition-[width] ${percent == null ? "w-1/3 animate-pulse" : ""}`} style={percent != null ? { width: `${percent}%` } : undefined} /></div>
    <p className="text-[10px] leading-4 text-muted">You can cancel safely. The original video is never changed.</p>
  </div>;
}

function EmbeddedVideoPlayer({ item, sourceName, compact = false, onRestore, onClose, onPrevious, onNext, onPlaybackEnded }: { item: LibraryItem; sourceName: string; compact?: boolean; onRestore?: () => void; onClose: () => void; onPrevious?: () => void; onNext?: () => void; onPlaybackEnded?: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const playerRef = useRef<HTMLElement | null>(null);
  const consentDialogRef = useRef<HTMLElement | null>(null);
  const consentReturnFocus = useRef<HTMLElement | null>(null);
  const lastSavedPosition = useRef(0);
  const playbackWatchdog = useRef<number | null>(null);
  const [session, setSession] = useState<LibraryPlaybackSession | null>(null);
  const [preflight, setPreflight] = useState<LibraryPlaybackPreflight | null>(null);
  const [consent, setConsent] = useState<LibraryPlaybackPreflight | null>(null);
  const [consentBusy, setConsentBusy] = useState(false);
  const [copyPreparationBusy, setCopyPreparationBusy] = useState(false);
  const [preparation, setPreparation] = useState<LibraryPlaybackPreparationProgress | null>(null);
  const [cancelBusy, setCancelBusy] = useState(false);
  const cancelRequested = useRef(false);
  const [rememberAutomatic, setRememberAutomatic] = useState(false);
  const [loading, setLoading] = useState(true);
  const [playbackError, setPlaybackError] = useState<AppError | null>(null);
  const [resumedAt, setResumedAt] = useState<number | null>(null);
  const [playbackRate, setPlaybackRate] = useState(1);
  const [currentPosition, setCurrentPosition] = useState(0);
  const [playbackDuration, setPlaybackDuration] = useState(item.durationSeconds || 0);

  useEffect(() => {
    let disposed = false;
    setSession(null);
    setPreflight(null);
    setConsent(null);
    setConsentBusy(false);
    setCopyPreparationBusy(false);
    setPreparation(null);
    setCancelBusy(false);
    cancelRequested.current = false;
    setRememberAutomatic(false);
    setLoading(true);
    setPlaybackError(null);
    setCurrentPosition(0);
    setPlaybackDuration(item.durationSeconds || 0);
    void api.inspectLibraryPlayback(item.id)
      .then(async (inspected) => {
        if (disposed) return;
        setPreflight(inspected);
        if (inspected.requiresConfirmation) {
          consentReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
          setConsent(inspected);
          setLoading(false);
          return;
        }
        const prepared = await api.prepareLibraryPlayback(item.id, false);
        if (!disposed) setSession(prepared);
      })
      .catch((error) => {
        if (!disposed) {
          setPlaybackError(normalizeError(error));
          setLoading(false);
        }
      });
    return () => { disposed = true; };
  }, [item.id]);

  useEffect(() => {
    if (!copyPreparationBusy) {
      setPreparation(null);
      return;
    }
    let disposed = false;
    const refresh = () => {
      void api.getLibraryPlaybackPreparation(item.id)
        .then((progress) => { if (!disposed && progress) setPreparation(progress); })
        .catch(() => {});
    };
    refresh();
    const interval = window.setInterval(refresh, 300);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [copyPreparationBusy, item.id]);

  useEffect(() => {
    if (!consent) return;
    consentDialogRef.current?.querySelector<HTMLElement>("[data-consent-cancel]")?.focus();
  }, [consent]);

  useEffect(() => {
    if (!session || !loading) return;
    const timer = window.setTimeout(() => {
      setPlaybackError(createPlaybackError("embedded_playback_timeout", "Embedded playback did not become ready. Your video is unchanged; try opening it with your system player."));
      setLoading(false);
    }, EMBEDDED_PLAYBACK_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, [session, loading]);

  function clearPlaybackWatchdog() {
    if (playbackWatchdog.current == null) return;
    window.clearTimeout(playbackWatchdog.current);
    playbackWatchdog.current = null;
  }

  function armPlaybackWatchdog() {
    clearPlaybackWatchdog();
    playbackWatchdog.current = window.setTimeout(() => {
      playbackWatchdog.current = null;
      videoRef.current?.pause();
      setLoading(false);
      setPlaybackError(createPlaybackError(
        "embedded_playback_timeout",
        "Embedded playback stalled after starting. Try an H.264 playback copy or open the original in your system player.",
      ));
    }, EMBEDDED_PLAYBACK_TIMEOUT_MS);
  }

  function armPlaybackWatchdogWhilePlaying() {
    const video = videoRef.current;
    if (video && !video.paused && !video.ended) armPlaybackWatchdog();
  }

  useEffect(() => () => clearPlaybackWatchdog(), [item.id, session?.url]);

  function persistProgress(completed = false) {
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration) || video.duration <= 0 || !Number.isFinite(video.currentTime)) return;
    lastSavedPosition.current = video.currentTime;
    void api.saveLibraryPlaybackProgress({
      itemId: item.id,
      positionSeconds: Math.max(0, video.currentTime),
      durationSeconds: video.duration,
      completed,
    }).catch(() => {});
  }

  useEffect(() => () => persistProgress(false), [item.id]);

  useEffect(() => () => {
    void api.releaseLibraryPlayback(item.id).catch(() => {});
  }, [item.id]);

  useEffect(() => {
    const resetPlayback = (event: Event) => {
      const resetItemId = (event as CustomEvent<{ itemId: string }>).detail?.itemId;
      const video = videoRef.current;
      if (resetItemId !== item.id || !video) return;
      video.currentTime = 0;
      lastSavedPosition.current = 0;
      setResumedAt(null);
    };
    window.addEventListener("nice-downloader:library-reset-playback", resetPlayback);
    return () => window.removeEventListener("nice-downloader:library-reset-playback", resetPlayback);
  }, [item.id]);

  function closePlayer() {
    persistProgress(false);
    onClose();
  }

  function changeVideo(change: (() => void) | undefined) {
    if (!change) return;
    persistProgress(false);
    change();
  }

  function togglePlayback() {
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) void video.play().catch(() => {});
    else video.pause();
  }

  function seek(seconds: number) {
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration)) return;
    video.currentTime = Math.min(video.duration, Math.max(0, video.currentTime + seconds));
  }

  function seekTo(seconds: number) {
    const video = videoRef.current;
    if (!video || !Number.isFinite(seconds)) return;
    const duration = Number.isFinite(video.duration) ? video.duration : playbackDuration;
    video.currentTime = Math.min(Math.max(0, duration), Math.max(0, seconds));
    setCurrentPosition(video.currentTime);
  }

  function toggleFullscreen() {
    if (document.fullscreenElement) void document.exitFullscreen();
    else if (playerRef.current) void playerRef.current.requestFullscreen();
  }

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (consent) return;
      if (compact && !playerRef.current?.contains(document.activeElement)) return;
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, [contenteditable='true']")) return;
      const key = event.key.toLocaleLowerCase();
      if (key === "escape" && !document.fullscreenElement) { event.preventDefault(); closePlayer(); }
      else if (key === " " || key === "k") { event.preventDefault(); togglePlayback(); }
      else if (key === "j" || key === "arrowleft") { event.preventDefault(); seek(-10); }
      else if (key === "l" || key === "arrowright") { event.preventDefault(); seek(10); }
      else if (key === "m" && videoRef.current) { event.preventDefault(); videoRef.current.muted = !videoRef.current.muted; }
      else if (key === "f") { event.preventDefault(); toggleFullscreen(); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [compact, consent]);

  function restoreConsentFocus() {
    window.requestAnimationFrame(() => {
      if (consentReturnFocus.current?.isConnected) consentReturnFocus.current.focus();
      else playerRef.current?.querySelector<HTMLElement>("button")?.focus();
    });
  }

  function dismissConsent() {
    if (consentBusy) return;
    setConsent(null);
    setLoading(false);
    setPlaybackError(createPlaybackError("embedded_playback_cancelled", "Embedded playback was not started. The original video is unchanged and can still be opened with your system player."));
    restoreConsentFocus();
  }

  function trapConsentFocus(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      dismissConsent();
      return;
    }
    if (event.key !== "Tab") return;
    const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), select:not(:disabled), [tabindex]:not([tabindex='-1'])"));
    if (controls.length === 0) return;
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  }

  async function rememberCompatibilityPreference() {
    if (!rememberAutomatic) return;
    try {
      const current = await api.getDesktopPreferences();
      const saved = await api.updateDesktopPreferences({ ...current, playbackCompatibility: "automatic" });
      window.dispatchEvent(new CustomEvent("nice-downloader:desktop-preferences", { detail: saved }));
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function approveCompatibilityCopy() {
    if (!consent || consentBusy) return;
    setConsentBusy(true);
    setCopyPreparationBusy(true);
    setPreparation(null);
    cancelRequested.current = false;
    setLoading(true);
    if (!consent.reencodeRequired) await rememberCompatibilityPreference();
    try {
      const prepared = consent.reencodeRequired
        ? await api.prepareLibraryPlaybackFallback(item.id, true)
        : await api.prepareLibraryPlayback(item.id, true);
      setSession(prepared);
      setConsent(null);
      restoreConsentFocus();
    } catch (error) {
      const normalized = normalizeError(error);
      setConsent(null);
      if (!(cancelRequested.current && normalized.code === "embedded_playback_preparation_cancelled")) {
        setPlaybackError(normalized);
      }
      setLoading(false);
      restoreConsentFocus();
    } finally {
      cancelRequested.current = false;
      setCopyPreparationBusy(false);
      setConsentBusy(false);
    }
  }

  async function cancelCompatibilityCopy() {
    if (!copyPreparationBusy || cancelBusy || !preparation) return;
    cancelRequested.current = true;
    setCancelBusy(true);
    try {
      await api.cancelLibraryPlaybackPreparation(item.id);
    } catch (error) {
      cancelRequested.current = false;
      setError(normalizeError(error));
    } finally {
      setCancelBusy(false);
    }
  }

  async function openOriginalFromConsent() {
    if (consentBusy) return;
    setConsentBusy(true);
    try {
      await api.openLibraryItem(item.id);
      setConsent(null);
      setLoading(false);
      setPlaybackError(createPlaybackError("embedded_playback_opened_externally", "The original video was opened in your system player. No compatibility copy was created."));
      restoreConsentFocus();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setConsentBusy(false);
    }
  }

  async function openExternal() {
    try {
      videoRef.current?.pause();
      persistProgress(false);
      await api.openLibraryItem(item.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function retryEmbeddedPlayback() {
    clearPlaybackWatchdog();
    setPlaybackError(null);
    setSession(null);
    setLoading(true);
    try {
      const inspected = await api.inspectLibraryPlaybackFallback(item.id);
      setPreflight(inspected);
      if (inspected.requiresConfirmation) {
        consentReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setConsent(inspected);
        setLoading(false);
        return;
      }
      const prepared = await api.prepareLibraryPlaybackFallback(item.id, false);
      setSession(prepared);
    } catch (error) {
      setPlaybackError(normalizeError(error));
      setLoading(false);
    }
  }

  function onLoadedMetadata() {
    const video = videoRef.current;
    if (!video) return;
    setLoading(false);
    video.playbackRate = playbackRate;
    setPlaybackDuration(video.duration);
    setCurrentPosition(video.currentTime);
    const saved = session?.playback;
    if (saved && !saved.completed && saved.positionSeconds >= 5 && saved.positionSeconds < video.duration - 10) {
      video.currentTime = saved.positionSeconds;
      lastSavedPosition.current = saved.positionSeconds;
      setResumedAt(saved.positionSeconds);
    }
  }

  return <><section ref={playerRef} className={`panel-shadow min-w-0 overflow-hidden border border-white/10 bg-[#06080b] text-white ${compact ? "rounded-[10px] shadow-[0_28px_80px_rgb(0_0_0/0.58)]" : "flex max-h-[calc(100dvh-96px)] flex-col rounded-[12px]"}`} aria-label={`${compact ? "Mini player" : "Playing"} ${item.title}`}>
    <header className={`flex items-center border-b border-white/10 bg-black/70 backdrop-blur-xl ${compact ? "min-h-11 gap-2 px-3 py-2" : "min-h-16 gap-4 px-5 py-3"}`}>
      <div className="min-w-0 flex-1"><h1 className={`${compact ? "text-[11px]" : "text-sm"} truncate font-semibold`}>{item.title}</h1>{!compact ? <p className="mt-0.5 truncate text-[11px] text-white/55">{sourceName}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}</p> : null}</div>
      <Button variant="ghost" className={`${compact ? "size-8 px-0" : ""} text-white/75 hover:text-white`} onClick={() => void openExternal()} aria-label="Open in system player"><ExternalLink size={15} />{compact ? null : " Open in system player"}</Button>
      {compact ? <button className="grid size-8 place-items-center rounded-[7px] text-white/70 hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onRestore} aria-label="Return to full watch page"><Maximize size={15} /></button> : <button className="grid size-9 place-items-center rounded-[8px] text-white/70 hover:bg-white/10 hover:text-white" onClick={toggleFullscreen} aria-label="Toggle fullscreen"><Maximize size={17} /></button>}
      {compact ? <button className="grid size-8 place-items-center rounded-[7px] text-white/70 hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={closePlayer} aria-label="Close mini player"><X size={16} /></button> : null}
    </header>
    <div className={`relative flex min-h-0 items-center justify-center overflow-auto bg-black ${compact ? "aspect-video" : "aspect-video min-h-[240px] max-h-[calc(100dvh-250px)]"}`}>
      {loading ? <div className="absolute inset-0 z-[1] flex items-center justify-center gap-2 bg-black/80 text-sm text-white/60" role="status"><LoaderCircle className="animate-spin text-accent" size={18} /> Preparing embedded playback…</div> : null}
      {playbackError ? <div className="max-w-lg border border-white/10 bg-white/5 p-6 text-center"><Film className="mx-auto text-white/35" size={34} /><h2 className="mt-4 text-lg font-semibold">This video could not be played here</h2><p className="mt-2 text-sm leading-6 text-white/55">{playbackError.message}</p><div className="mt-5 flex flex-wrap justify-center gap-2">{canOfferPlaybackFallback(playbackError) && !session?.compatibilityTranscoded ? <Button variant="secondary" onClick={() => void retryEmbeddedPlayback()}><RotateCcw size={15} /> Create H.264 playback copy</Button> : null}<Button onClick={() => void openExternal()}><ExternalLink size={15} /> Open in system player</Button></div></div> : null}
      {session && !playbackError ? <video
        ref={videoRef}
        className="h-full w-full max-h-full max-w-full bg-black object-contain shadow-[0_30px_100px_rgb(0_0_0/0.65)]"
        src={session.url}
        controls
        playsInline
        crossOrigin="anonymous"
        preload="metadata"
        onLoadedMetadata={onLoadedMetadata}
        onPlay={armPlaybackWatchdog}
        onPlaying={clearPlaybackWatchdog}
        onWaiting={armPlaybackWatchdogWhilePlaying}
        onStalled={armPlaybackWatchdogWhilePlaying}
        onTimeUpdate={() => { const video = videoRef.current; if (!video) return; setCurrentPosition(video.currentTime); if (Math.abs(video.currentTime - lastSavedPosition.current) >= 5) persistProgress(false); }}
        onPause={() => { clearPlaybackWatchdog(); persistProgress(false); }}
        onEnded={() => { clearPlaybackWatchdog(); persistProgress(true); window.setTimeout(() => onPlaybackEnded?.(), 0); }}
        onError={() => { clearPlaybackWatchdog(); setLoading(false); setPlaybackError(createPlaybackError("embedded_playback_decode_failed", "The WebView does not support this file's container or codecs. The original file is unchanged and can still be opened with your system player.")); }}
      >
        {session.subtitles.map((track) => <track key={track.url} kind="subtitles" src={track.url} srcLang={track.language || "und"} label={track.label} />)}
      </video> : null}
    </div>
    <footer className={`${compact ? "hidden" : "flex"} flex-wrap items-center gap-3 border-t border-white/10 bg-[#0c0f14] px-5 py-3`}>
      <Button variant="ghost" className="h-9 text-white/70" disabled={!onPrevious} onClick={() => changeVideo(onPrevious)}><ChevronRight className="rotate-180" size={15} /> Previous</Button>
      <Button variant="ghost" className="h-9 text-white/70" disabled={!onNext} onClick={() => changeVideo(onNext)}>Next <ChevronRight size={15} /></Button>
      <span className="h-5 w-px bg-white/10" />
      <label className="flex items-center gap-2 text-xs text-white/55"><span>Speed</span><Select className="h-8 w-24 border-white/10 bg-white/5 text-xs text-white" value={playbackRate} onChange={(event) => { const rate = Number(event.target.value); setPlaybackRate(rate); if (videoRef.current) videoRef.current.playbackRate = rate; }}><option value={0.5}>0.5×</option><option value={0.75}>0.75×</option><option value={1}>1×</option><option value={1.25}>1.25×</option><option value={1.5}>1.5×</option><option value={2}>2×</option></Select></label>
      <p className="ml-auto text-[10px] text-white/40">Space/K play · J/L seek · M mute · F fullscreen · Esc close</p>
      {resumedAt != null ? <p className="w-full text-[10px] text-accent" role="status">Resumed at {formatDuration(resumedAt)}</p> : null}
      {session?.compatibilityMode ? <p className="w-full text-[10px] text-accent" role="status">{session.compatibilityTranscoded ? "Using an H.264/AAC embedded-playback copy." : preflight?.compatibilityCached ? "Using an existing cached compatibility copy. AV1 timing is optimized for the embedded player." : "A lossless AV1 timing copy was created for this playback."} The original is unchanged.</p> : null}
    </footer>
  </section>{!compact ? <CreativeTimeline item={item} currentTime={currentPosition} duration={playbackDuration} playbackReady={Boolean(session && !playbackError)} onSeek={seekTo} /> : null}{consent ? <div className="fixed inset-0 z-[90] grid place-items-center bg-black/75 p-5 backdrop-blur-sm" onMouseDown={dismissConsent}><section ref={consentDialogRef} className="panel-shadow w-full max-w-xl overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="compatibility-copy-title" aria-describedby="compatibility-copy-description" aria-busy={consentBusy} onKeyDown={trapConsentFocus} onMouseDown={(event) => event.stopPropagation()}><header className="flex items-start gap-4 border-b border-line/70 px-5 py-5"><div className="grid size-11 shrink-0 place-items-center rounded-[10px] border border-accent/20 bg-accent/10 text-accent"><Copy size={20} /></div><div><p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-accent">Embedded playback compatibility</p><h2 id="compatibility-copy-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">{consent.reencodeRequired ? "Create an H.264 playback copy?" : "Create a temporary playback copy?"}</h2><p id="compatibility-copy-description" className="mt-2 text-xs leading-5 text-muted">{consent.reason}</p></div></header><div className="space-y-4 px-5 py-5"><dl className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-5 gap-y-2.5 border-y border-line/60 py-4 text-xs"><dt className="text-muted">Estimated copy size</dt><dd className="number-font font-medium text-ink">{formatBytes(consent.estimatedCopyBytes)}</dd><dt className="text-muted">Free space available</dt><dd className="number-font font-medium text-ink">{consent.availableSpaceBytes != null ? formatBytes(consent.availableSpaceBytes) : "Unknown"}</dd><dt className="text-muted">Conversion</dt><dd className="font-medium text-ink">{consent.reencodeRequired ? "H.264/AAC · video re-encode" : "Stream-copy · no re-encoding"}</dd></dl>{consent.availableSpaceBytes != null && consent.estimatedCopyBytes > consent.availableSpaceBytes ? <p className="border-l-2 border-red-400/70 bg-red-400/5 px-3 py-2 text-xs leading-5 text-red-300" role="alert">There may not be enough free space to create this copy.</p> : null}{consent.unusuallyLarge ? <p className="border-l-2 border-amber-400/70 bg-amber-400/5 px-3 py-2 text-xs leading-5 text-amber-200">This is an unusually large copy, so Nice Downloader always asks before creating it—even when automatic copies are enabled.</p> : null}<div className="rounded-[9px] bg-elevated/55 p-3 text-[11px] leading-5 text-muted"><p><strong className="font-semibold text-ink">Your original stays untouched.</strong> The copy exists only in Nice Downloader’s private playback cache.</p><p className="mt-1">The oldest copies are removed automatically before the cache exceeds 4 files or 8 GiB. You can review or clear the cache in Settings.</p></div>{!consent.reencodeRequired && consent.preference === "never" ? <p className="text-[11px] leading-5 text-muted">Your saved preference is <strong className="text-ink">Never create automatically</strong>. “Create copy and play” is a one-time override unless you enable the option below.</p> : null}{!consent.reencodeRequired ? <label className="flex items-start gap-3 text-xs text-muted"><input className="mt-0.5" type="checkbox" checked={rememberAutomatic} onChange={(event) => setRememberAutomatic(event.target.checked)} /><span><strong className="block font-medium text-ink">Automatically create compatible copies in the future</strong>Large copies will still require confirmation.</span></label> : <p className="text-[11px] leading-5 text-muted">This fallback is never created automatically because it changes the cached copy’s video codec. The original file and its quality remain unchanged.</p>}{copyPreparationBusy ? <PlaybackPreparationStatus preparation={preparation} reencodeRequired={consent.reencodeRequired} cancelling={cancelBusy || cancelRequested.current} /> : null}</div><footer className="flex flex-wrap items-center justify-end gap-2 border-t border-line/70 bg-elevated/25 px-5 py-4"><Button data-consent-cancel variant="ghost" disabled={cancelBusy || (copyPreparationBusy && !preparation)} onClick={copyPreparationBusy ? () => void cancelCompatibilityCopy() : dismissConsent}>{copyPreparationBusy ? cancelBusy ? "Cancelling…" : "Cancel copy" : "Cancel"}</Button><Button variant="secondary" disabled={consentBusy} onClick={() => void openOriginalFromConsent()}><ExternalLink size={15} /> Open original in system player</Button><Button disabled={consentBusy} onClick={() => void approveCompatibilityCopy()}>{consentBusy ? <LoaderCircle className="animate-spin" size={15} /> : <Copy size={15} />} {consentBusy && consent.reencodeRequired ? "Encoding…" : consent.reencodeRequired ? "Create H.264 copy and play" : "Create copy and play"}</Button></footer></section></div> : null}</>;
}

function MediaDetails({ item, sourceName, onClose, onWatch, onChanged, onMetadataChanged, onBrowseCreator, onBrowseSeries, onBrowseSite, onSelectRelated, onRemoved, onOperation }: { item: LibraryItem; sourceName: string; onClose: () => void; onWatch: () => void; onChanged: (item: LibraryItem) => void; onMetadataChanged: () => void; onBrowseCreator: (creator: string) => void; onBrowseSeries: (series: string) => void; onBrowseSite: (site: string) => void; onSelectRelated: (item: LibraryItem) => void; onRemoved: () => void; onOperation: (result: LibraryFileOperationResult) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [editing, setEditing] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [tags, setTags] = useState(item.tags.join(", "));
  const [richMetadata, setRichMetadata] = useState<LibraryRichMetadata | null>(null);
  const [relatedItems, setRelatedItems] = useState<LibraryItem[]>([]);
  const [relatedLoading, setRelatedLoading] = useState(false);
  const [relatedRevision, setRelatedRevision] = useState(0);
  const [metadataEditing, setMetadataEditing] = useState(false);
  const [metadataBusy, setMetadataBusy] = useState(false);
  const [creator, setCreator] = useState("");
  const [description, setDescription] = useState("");
  const [uploadDate, setUploadDate] = useState("");
  const [series, setSeries] = useState("");
  const [episode, setEpisode] = useState("");
  const [originalUrl, setOriginalUrl] = useState("");
  const activeItemId = useRef(item.id);
  activeItemId.current = item.id;
  const [organizeOpen, setOrganizeOpen] = useState(false);
  const [operationKind, setOperationKind] = useState<LibraryFileOperationKind>("rename");
  const [destinationDirectory, setDestinationDirectory] = useState("");
  const [newFileName, setNewFileName] = useState(fileNameFromPath(item.path));
  const [operationPlan, setOperationPlan] = useState<LibraryFileOperationPlan | null>(null);
  const [operationBusy, setOperationBusy] = useState(false);
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
  }, []);

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
    setRichMetadata(null);
    setRelatedItems([]);
    setRelatedLoading(true);
    setMetadataEditing(false);
    let disposed = false;
    void api.getLibraryRichMetadata(item.id)
      .then((metadata) => {
        if (disposed) return;
        setRichMetadata(metadata);
        loadMetadataForm(metadata);
      })
      .catch((error) => { if (!disposed) setError(normalizeError(error)); });
    void api.getRelatedLibraryItems(item.id, 6)
      .then((related) => { if (!disposed) setRelatedItems(related); })
      .catch(() => { if (!disposed) setRelatedItems([]); })
      .finally(() => { if (!disposed) setRelatedLoading(false); });
    return () => { disposed = true; };
  }, [item.id, item.path, relatedRevision]);

  function loadMetadataForm(metadata: LibraryRichMetadata) {
    setCreator(metadata.effective.creator || "");
    setDescription(metadata.effective.description || "");
    setUploadDate(metadata.effective.uploadDate || "");
    setSeries(metadata.effective.series || "");
    setEpisode(metadata.effective.episode || "");
    setOriginalUrl(metadata.effective.originalUrl || "");
  }

  async function saveRichMetadata() {
    if (metadataBusy) return;
    const requestedItemId = item.id;
    setMetadataBusy(true);
    try {
      const metadata = await api.saveLibraryRichMetadata({
        itemId: requestedItemId,
        creator: creator || null,
        description: description || null,
        uploadDate: uploadDate || null,
        series: series || null,
        episode: episode || null,
        originalUrl: originalUrl || null,
      });
      if (activeItemId.current !== requestedItemId) return;
      setRichMetadata(metadata);
      loadMetadataForm(metadata);
      setMetadataEditing(false);
      onMetadataChanged();
      setRelatedRevision((revision) => revision + 1);
      setNotice({ message: "Metadata overrides saved. The original media was not changed.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setMetadataBusy(false);
    }
  }

  async function resetRichMetadata() {
    if (metadataBusy) return;
    const requestedItemId = item.id;
    setMetadataBusy(true);
    try {
      const metadata = await api.resetLibraryRichMetadata(requestedItemId);
      if (activeItemId.current !== requestedItemId) return;
      setRichMetadata(metadata);
      loadMetadataForm(metadata);
      setMetadataEditing(false);
      onMetadataChanged();
      setRelatedRevision((revision) => revision + 1);
      setNotice({ message: "Using extracted sidecar metadata again.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setMetadataBusy(false);
    }
  }

  async function chooseArtwork() {
    const selected = await open({ multiple: false, directory: false, title: "Choose library artwork", filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "webp"] }] });
    if (!selected) return;
    try {
      onChanged(await api.setLibraryArtwork({ itemId: item.id, path: selected }));
      setNotice({ message: "Custom artwork selected. The source image was not modified.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function clearArtwork() {
    try {
      onChanged(await api.setLibraryArtwork({ itemId: item.id, path: null }));
      setNotice({ message: "Custom artwork cleared. A generated preview will be used.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function openItem() {
    try {
      await api.openLibraryItem(item.id);
    } catch (error) {
      const normalized = normalizeError(error);
      if (normalized.code === "library_file_not_found") {
        onChanged({ ...item, missing: true });
        setNotice({ message: "The missing file was removed from normal Library results. Choose Missing files to restore or remove its record.", view: "library" });
      } else {
        setError(normalized);
      }
    }
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

  function trapFocus(event: React.KeyboardEvent<HTMLElement>) {
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

  return <aside className="fixed bottom-0 right-0 top-0 z-50 w-[min(500px,100vw)] overflow-y-auto border-l border-line/70 bg-panel/98 shadow-[-28px_0_80px_rgb(0_0_0/0.32)] backdrop-blur-xl" role="dialog" aria-modal="true" aria-label={`${item.title} details`} onKeyDown={trapFocus}><div className="sticky top-0 z-[1] flex items-center justify-between border-b border-line/60 bg-panel/90 px-5 py-4 backdrop-blur-xl"><div><p className="text-xs font-semibold">Media details</p><p className="mt-0.5 text-[10px] text-muted">Use ← → to browse · Esc to close</p></div><button ref={closeButtonRef} className="grid size-8 place-items-center rounded-[7px] text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onClose} aria-label="Close media details"><X size={17} /></button></div><div className="p-5"><div className="aspect-video overflow-hidden rounded-[12px] bg-elevated"><MediaArtwork item={item} eager /></div>{editing ? <div className="mt-5 space-y-3"><Field label="Title"><Input value={title} onChange={(event) => setTitle(event.target.value)} /></Field><Field label="Tags"><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="Comma-separated tags" /></Field><div className="flex gap-2"><Button onClick={() => void update()}>Save changes</Button><Button variant="secondary" onClick={() => setEditing(false)}>Cancel</Button></div></div> : <><h2 className="mt-5 text-[22px] font-semibold leading-[1.2] tracking-[-0.035em] text-balance">{item.title}</h2><p className="mt-2 text-xs text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</p><div className="mt-5 flex flex-wrap gap-2">{item.mediaKind === "video" ? <Button className="flex-1" disabled={item.missing} onClick={onWatch}><Play size={16} fill="currentColor" /> Watch here</Button> : <Button className="flex-1" disabled={item.missing} onClick={onWatch}><FileImage size={16} /> View image</Button>}<Button variant="secondary" disabled={item.missing} onClick={() => void openItem()}><ExternalLink size={15} /> {item.mediaKind === "image" ? "System viewer" : "System player"}</Button><Button variant="secondary" onClick={() => setEditing(true)}>Edit</Button></div></>}
      <div className="mt-4 flex items-center gap-2"><Button variant="secondary" className={item.favorite ? "text-rose-300" : ""} onClick={() => void updateCuration(!item.favorite, item.rating)}><Heart size={15} fill={item.favorite ? "currentColor" : "none"} /> {item.favorite ? "Favorited" : "Favorite"}</Button><label className="flex flex-1 items-center gap-2 text-xs text-muted"><Star size={15} /><span className="sr-only">Rating</span><Select className="h-10" value={item.rating ?? ""} onChange={(event) => void updateCuration(item.favorite, event.target.value ? Number(event.target.value) : null)}><option value="">Not rated</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}</Select></label></div>
      <section className="mt-7 border-t border-line/60 pt-5" aria-labelledby="rich-metadata-title">
        <div className="flex items-start justify-between gap-3"><div><h3 id="rich-metadata-title" className="text-sm font-semibold">Media information</h3><p className="mt-1 text-[11px] leading-5 text-muted">Imported from yt-dlp sidecars. Your edits stay in Nice Downloader and survive rescans.</p></div><Button variant="ghost" className="h-8 text-xs" disabled={!richMetadata || metadataBusy} onClick={() => { if (richMetadata) loadMetadataForm(richMetadata); setMetadataEditing((value) => !value); }}>{metadataEditing ? "Cancel" : "Edit metadata"}</Button></div>
        {metadataEditing ? <div className="mt-4 space-y-3"><Field label="Creator or channel"><Input value={creator} maxLength={200} onChange={(event) => setCreator(event.target.value)} /></Field><Field label="Description"><Textarea className="min-h-28" value={description} maxLength={10000} onChange={(event) => setDescription(event.target.value)} /></Field><div className="grid gap-3 sm:grid-cols-2"><Field label="Upload date"><Input type="date" value={uploadDate} onChange={(event) => setUploadDate(event.target.value)} /></Field><Field label="Series"><Input value={series} maxLength={200} onChange={(event) => setSeries(event.target.value)} /></Field><Field label="Episode"><Input value={episode} maxLength={200} onChange={(event) => setEpisode(event.target.value)} /></Field><Field label="Original URL"><Input type="url" value={originalUrl} maxLength={2000} onChange={(event) => setOriginalUrl(event.target.value)} /></Field></div><div className="flex flex-wrap gap-2"><Button disabled={metadataBusy} onClick={() => void saveRichMetadata()}>{metadataBusy ? <LoaderCircle className="animate-spin" size={14} /> : <Save size={14} />} Save metadata</Button>{richMetadata && richMetadata.overriddenFields.length > 0 ? <Button variant="secondary" disabled={metadataBusy} onClick={() => void resetRichMetadata()}><RotateCcw size={14} /> Use extracted values</Button> : null}</div></div> : richMetadata ? <div className="mt-4"><dl className="grid grid-cols-[92px_1fr] gap-x-3 gap-y-2 text-xs">{richMetadata.effective.creator ? <><dt className="text-muted">Creator</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseCreator(richMetadata.effective.creator!)}>{richMetadata.effective.creator}</button></dd></> : null}{richMetadata.effective.uploadDate ? <><dt className="text-muted">Published</dt><dd>{new Date(`${richMetadata.effective.uploadDate}T00:00:00`).toLocaleDateString()}</dd></> : null}{richMetadata.effective.series ? <><dt className="text-muted">Series</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseSeries(richMetadata.effective.series!)}>{richMetadata.effective.series}</button>{richMetadata.effective.episode ? ` · ${richMetadata.effective.episode}` : ""}</dd></> : richMetadata.effective.episode ? <><dt className="text-muted">Episode</dt><dd>{richMetadata.effective.episode}</dd></> : null}{richMetadata.effective.originalUrl ? <><dt className="text-muted">Original</dt><dd className="break-all text-muted" title={richMetadata.effective.originalUrl}>{richMetadata.effective.originalUrl}</dd></> : null}</dl>{richMetadata.effective.description ? <p className="mt-4 whitespace-pre-wrap text-xs leading-5 text-muted">{richMetadata.effective.description}</p> : null}{richMetadata.overriddenFields.length > 0 ? <p className="mt-3 text-[10px] text-accent">User overrides: {richMetadata.overriddenFields.join(", ")}</p> : <p className="mt-3 text-[10px] text-muted">Extracted metadata · no user overrides</p>}</div> : <p className="mt-4 flex items-center gap-2 text-xs text-muted" role="status"><LoaderCircle className="animate-spin" size={13} /> Loading metadata…</p>}
        <div className="mt-5 flex flex-wrap gap-2 border-t border-line/50 pt-4"><Button variant="secondary" className="h-8 text-xs" onClick={() => void chooseArtwork()}><ImageIcon size={13} /> Choose artwork</Button>{item.thumbnail ? <Button variant="ghost" className="h-8 text-xs" onClick={() => void clearArtwork()}><RotateCcw size={13} /> Use generated artwork</Button> : null}</div>
      </section>
      {relatedLoading || relatedItems.length > 0 ? <section className="mt-7 border-t border-line/60 pt-5" aria-labelledby="related-media-title"><div className="flex items-center justify-between"><h3 id="related-media-title" className="text-sm font-semibold">Related media</h3>{relatedLoading ? <LoaderCircle className="animate-spin text-muted" size={14} /> : <span className="text-[10px] text-muted">Same series, creator, or site</span>}</div>{relatedItems.length > 0 ? <div className="mt-3 grid gap-2">{relatedItems.map((related) => <button key={related.id} className="group flex min-w-0 items-center gap-3 rounded-[8px] bg-elevated/45 p-2 text-left transition hover:bg-elevated" onClick={() => onSelectRelated(related)}><div className="aspect-video w-24 shrink-0 overflow-hidden rounded-[6px] bg-canvas"><MediaArtwork item={related} /></div><span className="min-w-0"><span className="line-clamp-2 block text-xs font-medium leading-4 text-ink group-hover:text-accent">{related.title}</span><span className="mt-1 block truncate text-[10px] text-muted">{related.site || related.container?.toLocaleUpperCase() || (related.mediaKind === "video" ? "Video" : "Image")}</span></span></button>)}</div> : null}</section> : null}
      <dl className="mt-7 grid grid-cols-[110px_1fr] gap-x-4 gap-y-3 border-t border-line/60 pt-5 text-xs"><dt className="text-muted">Type</dt><dd>{item.mediaKind === "image" ? "Image" : "Video"}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}</dd>{item.width && item.height ? <><dt className="text-muted">Dimensions</dt><dd className="number-font">{item.width} × {item.height}{item.fps ? ` · ${formatFps(item.fps)} fps` : ""}{item.dynamicRange ? ` · ${item.dynamicRange}` : ""}</dd></> : null}{item.videoCodec || item.audioCodec ? <><dt className="text-muted">Codecs</dt><dd>{[formatCodec(item.videoCodec), formatCodec(item.audioCodec)].filter(Boolean).join(" + ")}</dd></> : null}{item.fileSizeBytes != null ? <><dt className="text-muted">File size</dt><dd className="number-font">{formatBytes(item.fileSizeBytes)}</dd></> : null}<dt className="text-muted">Added</dt><dd>{new Date(item.addedAt).toLocaleString()}</dd>{item.modifiedAtMs ? <><dt className="text-muted">Modified</dt><dd>{new Date(item.modifiedAtMs).toLocaleString()}</dd></> : null}<dt className="text-muted">Location</dt><dd className="break-all text-muted" title={item.path}>{item.relativePath || item.path}</dd>{item.site ? <><dt className="text-muted">Origin</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseSite(item.site!)}>{item.site}</button></dd></> : null}</dl>
      {item.metadataState === "pending" ? <p className="mt-5 flex items-center gap-2 text-[11px] text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={13} /> Inspecting resolution, codecs, and duration…</p> : null}
      {item.metadataError ? <details className="mt-5 border-l-2 border-amber-400/50 pl-3 text-[11px] text-muted"><summary className="cursor-pointer font-medium text-amber-300">Media inspection unavailable</summary><p className="mt-2 leading-5">{item.metadataError}</p></details> : null}
      {item.tags.length > 0 ? <div className="mt-6 flex flex-wrap gap-1.5">{item.tags.map((tag) => <span key={tag} className="rounded-[6px] bg-elevated px-2 py-1 text-[10px] text-muted">#{tag}</span>)}</div> : null}
      <section className="mt-7 border-t border-line/60 pt-5" aria-labelledby="organize-media-title"><button className="flex w-full items-center justify-between text-left" onClick={() => { setOrganizeOpen((value) => !value); setOperationPlan(null); }} aria-expanded={organizeOpen}><span><span id="organize-media-title" className="block text-sm font-semibold">Organize file</span><span className="mt-1 block text-[11px] text-muted">Preview every rename, move, or copy before it changes disk.</span></span><FolderInput className="text-accent" size={18} /></button>{organizeOpen ? <div className="mt-4 space-y-3"><Field label="Action"><Select value={operationKind} onChange={(event) => { setOperationKind(event.target.value as LibraryFileOperationKind); setOperationPlan(null); }}><option value="rename">Rename in this folder</option><option value="move">Move to another folder</option><option value="copy">Copy to another folder</option></Select></Field>{operationKind !== "rename" ? <Field label="Destination folder"><div className="flex gap-2"><Input value={destinationDirectory} readOnly placeholder="Choose a folder" /><Button variant="secondary" className="shrink-0 px-3" onClick={() => void chooseOperationFolder()}><FolderOpen size={14} /> Browse</Button></div></Field> : null}<Field label="Filename"><Input value={newFileName} onChange={(event) => { setNewFileName(event.target.value); setOperationPlan(null); }} /></Field>{operationPlan ? <div className="border border-accent/25 bg-accent/5 p-3"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Ready to confirm</p><div className="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-[11px]"><span className="min-w-0 break-all text-muted">{operationPlan.sourceDisplayName}</span><ArrowRight className="shrink-0 text-accent" size={14} /><span className="min-w-0 break-all text-ink">{operationPlan.destinationDisplayName}</span></div><ul className="mt-3 space-y-1 border-t border-line/60 pt-3 text-[10px] leading-4 text-muted">{operationPlan.warnings.map((warning) => <li key={warning}>• {warning}</li>)}</ul></div> : null}<div className="flex gap-2">{operationPlan ? <Button variant="secondary" disabled={operationBusy} onClick={() => setOperationPlan(null)}>Change plan</Button> : <Button variant="secondary" disabled={operationBusy || !newFileName.trim() || (operationKind !== "rename" && !destinationDirectory)} onClick={() => void previewOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : null} Preview</Button>}{operationPlan ? <Button disabled={operationBusy} onClick={() => void executeOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : <Check size={14} />} Confirm {operationKind}</Button> : null}</div></div> : null}</section>
      <div className="mt-8 border-t border-line/60 pt-5"><p className="text-[11px] leading-5 text-muted">Removing this record does not delete the original file.</p><div className="mt-3 flex items-center gap-2">{confirmRemove ? <Button variant="secondary" className="h-8 text-xs" onClick={() => setConfirmRemove(false)}>Keep record</Button> : null}<Button variant={confirmRemove ? "danger" : "ghost"} className="h-8 text-xs" onClick={() => void remove()}><Trash2 size={13} /> {confirmRemove ? "Confirm removal" : "Remove record"}</Button></div></div></div></aside>;
}

function fileNameFromPath(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) || path;
}

function buildFolderOptions(folders: LibraryFolderFacet[], sources: LibrarySource[], sourceFilter: string): FolderOption[] {
  const sourceNames = new Map(sources.map((source) => [source.id, source.name]));
  return folders
    .filter((folder) => sourceFilter === "all" || folder.sourceId === sourceFilter)
    .map((folder) => ({
      ...folder,
      value: folderFilterValue(folder.sourceId, folder.path),
      label: folder.path
        ? sourceFilter === "all" ? `${sourceNames.get(folder.sourceId) || "Source"} / ${folder.path}` : folder.path
        : sourceFilter === "all" ? `${sourceNames.get(folder.sourceId) || "Source"} / Source root` : "Source root",
    }))
    .sort((left, right) => {
    if (left.sourceId === right.sourceId && left.path === "") return -1;
    if (left.sourceId === right.sourceId && right.path === "") return 1;
    return left.label.localeCompare(right.label, undefined, { numeric: true });
  });
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

function BulkActionBar({ selectedIds, collections, busy, onCompare, onCompareImages, onCreateStack, onMove, onApply, onClose }: { selectedIds: Set<string>; collections: LibraryCollection[]; busy: boolean; onCompare: () => Promise<void>; onCompareImages: () => Promise<void>; onCreateStack: () => Promise<void>; onMove: () => void; onApply: (action: LibraryBulkAction, message: string) => Promise<void>; onClose: () => void }) {
  const [tag, setTag] = useState("");
  const disabled = busy || selectedIds.size === 0;
  const compareDisabled = busy || selectedIds.size < 2 || selectedIds.size > 4;
  const stackDisabled = busy || selectedIds.size < 2 || selectedIds.size > 100;
  return <aside className="panel-shadow fixed bottom-6 left-1/2 z-40 flex w-[min(1280px,calc(100vw-300px))] -translate-x-1/2 flex-wrap items-center gap-2 border border-line/80 bg-panel/95 p-3 backdrop-blur-xl" aria-label="Bulk library actions"><div className="mr-2 min-w-24"><p className="number-font text-sm font-semibold">{selectedIds.size} selected</p><button className="mt-0.5 text-[10px] font-medium text-muted hover:text-ink" onClick={onClose}>Clear selection</button></div><Button className="h-9 px-3 text-xs" disabled={compareDisabled} onClick={() => void onCompare()} title={compareDisabled ? "Select between 2 and 4 videos to compare" : "Open selected videos in the comparison grid"}><LayoutGrid size={14} /> Videos</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={compareDisabled} onClick={() => void onCompareImages()} title={compareDisabled ? "Select between 2 and 4 images to compare" : "Open selected images in the comparison studio"}><ImageIcon size={14} /> Images</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={stackDisabled} onClick={() => void onCreateStack()} title={stackDisabled ? "Select between 2 and 100 items of one media type" : "Group versions without moving files"}><Layers size={14} /> Stack</Button><Button className="h-9 px-3 text-xs" disabled={disabled} onClick={onMove}><FolderInput size={14} /> Move</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: true }, `Marked ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} as favorites.`)}><Heart size={14} /> Favorite</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: false }, `Removed ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} from favorites.`)}><Heart size={14} fill="none" /> Unfavorite</Button><label><span className="sr-only">Set rating</span><Select className="h-9 w-32 text-xs" disabled={disabled} defaultValue="" onChange={(event) => { const value = event.target.value; if (value !== "") void onApply({ type: "setRating", rating: value === "clear" ? null : Number(value) }, value === "clear" ? "Cleared selected ratings." : `Rated selected media ${value} stars.`); event.currentTarget.value = ""; }}><option value="" disabled>Set rating</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}<option value="clear">Clear rating</option></Select></label><label><span className="sr-only">Add to collection</span><Select className="h-9 w-44 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "addToCollection", collectionId: event.target.value }, "Added selected media to the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Add to collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><label><span className="sr-only">Remove from collection</span><Select className="h-9 w-48 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "removeFromCollection", collectionId: event.target.value }, "Removed selected media from the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Remove from collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><div className="flex min-w-48 flex-1"><Input className="h-9 rounded-r-none text-xs" value={tag} onChange={(event) => setTag(event.target.value)} placeholder="Add a tag" /><Button variant="secondary" className="h-9 rounded-l-none px-3 text-xs" disabled={disabled || !tag.trim()} onClick={() => { const value = tag.trim(); if (!value) return; void onApply({ type: "addTags", tags: [value] }, `Added #${value} to selected media.`); setTag(""); }}><Tags size={14} /> Add</Button></div></aside>;
}

function BulkMoveDialog({ selectedIds, onClose, onComplete }: { selectedIds: Set<string>; onClose: () => void; onComplete: (result: LibraryBulkMoveResult) => void }) {
  const [destination, setDestination] = useState("");
  const [plan, setPlan] = useState<LibraryBulkMovePlan | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) {
        event.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  async function chooseDestination() {
    const path = await open({ directory: true, multiple: false, title: "Move selected media to folder" });
    if (!path) return;
    setDestination(path);
    setPlan(null);
    setError(null);
  }

  async function previewMove() {
    if (!destination || selectedIds.size === 0) return;
    setBusy(true);
    setError(null);
    try {
      setPlan(await api.previewLibraryBulkMove({ itemIds: Array.from(selectedIds), destinationDirectory: destination }));
    } catch (cause) {
      setPlan(null);
      setError(normalizeError(cause).message);
    } finally {
      setBusy(false);
    }
  }

  async function executeMove() {
    if (!plan) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.executeLibraryBulkMove(plan.operations.map((entry) => entry.operation.id));
      onComplete(result);
    } catch (cause) {
      setError(normalizeError(cause).message);
      setPlan(null);
    } finally {
      setBusy(false);
    }
  }

  function trapFocus(event: React.KeyboardEvent<HTMLElement>) {
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

  return <div className="fixed inset-0 z-[70] grid place-items-center bg-black/70 p-5 backdrop-blur-sm" onMouseDown={() => { if (!busy) onClose(); }}><section className="panel-shadow flex max-h-[min(760px,calc(100vh-40px))] w-full max-w-2xl flex-col overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="bulk-move-title" aria-busy={busy} onKeyDown={trapFocus} onMouseDown={(event) => event.stopPropagation()}><header className="flex items-start justify-between gap-4 border-b border-line/70 px-5 py-4"><div><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Organize library</p><h2 id="bulk-move-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Move {selectedIds.size} selected item{selectedIds.size === 1 ? "" : "s"}</h2><p className="mt-1.5 text-xs leading-5 text-muted">Choose a subfolder, review every destination, then confirm the move.</p></div><button autoFocus className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 disabled:opacity-40" disabled={busy} onClick={onClose} aria-label="Close bulk move"><X size={17} /></button></header><div className="overflow-y-auto p-5"><Field label="Destination folder" hint="Select an existing subfolder on the same filesystem as all selected files."><div className="flex gap-2"><Input value={destination} readOnly placeholder="Choose a destination folder" /><Button variant="secondary" className="shrink-0" disabled={busy} onClick={() => void chooseDestination()}><FolderOpen size={15} /> Browse</Button></div></Field>{error ? <p className="mt-4 border-l-2 border-red-400/60 bg-red-400/5 px-3 py-2 text-xs leading-5 text-red-300" role="alert">{error}</p> : null}{plan ? <div className="mt-5 space-y-4"><div className="flex flex-wrap items-center justify-between gap-3 border-y border-line/60 py-3 text-xs"><span><strong className="number-font text-ink">{plan.operations.length}</strong> files ready</span><span className="number-font text-muted">{formatBytes(plan.totalSizeBytes)}</span></div><div><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">Move preview</p><div className="mt-2 max-h-56 divide-y divide-line/60 overflow-y-auto border-y border-line/60">{plan.operations.map((entry) => <div key={entry.operation.id} className="grid grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3 py-2.5 text-[11px]"><span className="truncate text-muted" title={entry.operation.sourcePath}>{entry.sourceDisplayName}</span><ArrowRight className="shrink-0 text-accent" size={14} /><span className="truncate text-ink" title={entry.operation.destinationPath}>{entry.destinationDisplayName}</span></div>)}</div></div><ul className="space-y-1.5 bg-elevated/45 p-3 text-[11px] leading-4 text-muted">{plan.warnings.map((warning) => <li key={warning}>• {warning}</li>)}</ul></div> : <div className="mt-5 border border-dashed border-line/70 bg-elevated/25 px-4 py-5"><p className="text-xs font-medium text-ink">Selected files</p><p className="mt-1 text-[11px] text-muted">{selectedIds.size} selected files will be validated before the move. Filenames are preserved.</p></div>}</div><footer className="flex items-center justify-end gap-2 border-t border-line/70 bg-elevated/25 px-5 py-4"><Button variant="ghost" disabled={busy} onClick={onClose}>Cancel</Button>{plan ? <Button variant="secondary" disabled={busy} onClick={() => setPlan(null)}>Change destination</Button> : null}{plan ? <Button disabled={busy} onClick={() => void executeMove()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <FolderInput size={15} />} Confirm move</Button> : <Button disabled={busy || !destination || selectedIds.size === 0} onClick={() => void previewMove()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <ArrowRight size={15} />} Preview move</Button>}</footer></section></div>;
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

  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="library-duplicates-title"><div className="mb-4 flex flex-wrap items-end justify-between gap-3"><div><div className="flex items-center gap-2"><Fingerprint className="text-accent" size={18} /><h2 id="library-duplicates-title" className="text-lg font-semibold tracking-[-0.025em]">Duplicate intelligence</h2></div><p className="mt-1 max-w-2xl text-xs leading-5 text-muted">Size identifies candidates; streaming SHA-256 verifies byte-for-byte matches. Cleanup is always an explicit, confirmed move to the system Trash.</p></div>{running ? <Button variant="secondary" className="h-9 text-xs" onClick={() => void cancel()}><Ban size={14} /> Cancel analysis</Button> : <Button variant="secondary" className="h-9 text-xs" onClick={() => void start()}><Fingerprint size={14} /> {groups.length > 0 ? "Analyze again" : "Analyze duplicates"}</Button>}</div>{running && job ? <div className="bg-panel/55 p-4" aria-label="Duplicate analysis progress"><div className="flex items-center justify-between gap-4 text-xs"><span className="flex min-w-0 items-center gap-2 text-ink"><LoaderCircle className="shrink-0 animate-spin text-accent" size={14} /><span className="truncate">{job.currentTitle ? `Checking ${job.currentTitle}` : "Finding same-size candidates"}</span></span><span className="number-font shrink-0 text-muted">{job.hashedItems + job.skippedItems} / {job.candidateItems}</span></div><div className="mt-3 h-1.5 overflow-hidden rounded-full bg-elevated" role="progressbar" aria-label="Exact duplicate analysis" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progress)}><div className="h-full bg-accent transition-[width]" style={{ width: `${progress}%` }} /></div><p className="mt-2 text-[10px] text-muted">{formatBytes(job.hashedBytes)} verified · {job.skippedItems} skipped · cached hashes are reused while fingerprints match</p></div> : null}{job?.state === "failed" ? <p className="border-l-2 border-red-400/60 pl-3 text-xs text-red-300">{job.error || "Duplicate analysis failed."}</p> : null}{!running && groups.length === 0 ? <div className="border border-dashed border-line/70 bg-panel/25 px-5 py-7 text-center text-xs text-muted">{job?.state === "completed" ? "No byte-for-byte duplicate groups were found." : "Run analysis to verify possible duplicates without changing any files."}</div> : <div className="space-y-3">{groups.map((group, index) => <article key={group.id} className="bg-panel/55 p-4"><div className="flex flex-wrap items-start justify-between gap-3"><div><div className="flex items-center gap-2"><Copy className="text-accent" size={16} /><h3 className="text-sm font-semibold">Exact duplicate group {index + 1}</h3></div><p className="mt-1 text-[10px] text-muted">SHA-256 verified · {formatBytes(group.fileSizeBytes)} each · up to {formatBytes(group.potentialReclaimBytes)} duplicated data</p></div><span className="number-font rounded bg-elevated px-2 py-1 text-[10px] text-muted">{group.itemIds.length} files</span></div><div className="mt-3 divide-y divide-line/60 border-t border-line/60">{group.itemIds.map((id) => { const item = itemsById.get(id); const confirming = confirmTrash === id; return <div key={id} className="flex items-center gap-3 py-2.5"><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-accent">{item?.mediaKind === "image" ? <ImageIcon size={14} /> : <Film size={14} />}</div><div className="min-w-0 flex-1"><p className="truncate text-xs font-medium text-ink">{item?.title || "Media record"}</p><p className="mt-0.5 truncate text-[10px] text-muted" title={item?.path}>{item?.path || "Location unavailable"}</p></div>{item && !item.missing ? <><Button variant="ghost" className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => void openItem(item.id)}>Open</Button>{confirming ? <Button variant="secondary" className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => setConfirmTrash(null)}>Keep</Button> : null}<Button variant={confirming ? "danger" : "ghost"} className="h-8 px-2 text-xs" disabled={trashBusy != null} onClick={() => void moveToTrash(group.id, item.id)}>{trashBusy === item.id ? <LoaderCircle className="animate-spin" size={13} /> : <Trash2 size={13} />} {confirming ? "Confirm Trash" : "Move to Trash"}</Button></> : null}</div>; })}</div></article>)}</div>}<p className="mt-3 text-[10px] leading-4 text-muted">Before cleanup, Rust re-hashes the selected file and a retained copy. Potential duplicated data is informational; hard links or filesystem compression can make actual recoverable space different.</p></section>;
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

  return <section className="panel-shadow mb-6 border border-accent/25 bg-panel/90 p-5" aria-labelledby="add-source-title"><div className="flex items-start justify-between gap-4"><div><h2 id="add-source-title" className="text-sm font-semibold">Add a library source</h2><p className="mt-1 text-xs text-muted">Files stay in this folder. Nice Downloader only creates catalog records.</p></div><button className="text-muted hover:text-ink" onClick={onClose} aria-label="Close add folder form"><X size={18} /></button></div><div className="mt-4 grid gap-3 lg:grid-cols-[1fr_1fr_180px]"><Field label="Folder"><div className="flex gap-2"><Input value={path} readOnly placeholder="Choose a folder" /><Button autoFocus variant="secondary" className="shrink-0" onClick={() => void chooseFolder()}><FolderOpen size={15} /> Browse</Button></div></Field><Field label="Source name"><Input value={name} onChange={(event) => setName(event.target.value)} placeholder="Media archive" /></Field><Field label="Media"><Select value={mediaKind} onChange={(event) => setMediaKind(event.target.value as LibraryMediaKind)}><option value="both">Videos and images</option><option value="video">Videos only</option><option value="image">Images only</option></Select></Field></div><div className="mt-4 flex flex-wrap items-center justify-between gap-4 border-t border-line/60 pt-4"><div className="flex flex-wrap gap-5 text-xs text-muted"><label className="flex items-center gap-2"><input type="checkbox" checked={recursive} onChange={(event) => setRecursive(event.target.checked)} /> Include subfolders</label><label className="flex items-center gap-2"><input type="checkbox" checked={watchEnabled} onChange={(event) => setWatchEnabled(event.target.checked)} /> Auto scan changes</label><label className="flex items-center gap-2"><input type="checkbox" checked={privateSource} onChange={(event) => setPrivateSource(event.target.checked)} /> Mark as private</label></div><div className="flex gap-2"><Button variant="secondary" onClick={onClose}>Cancel</Button><Button disabled={busy || !path || !name.trim()} onClick={() => void addSource()}>{busy ? <LoaderCircle className="animate-spin" size={15} /> : <Play size={15} />} Add and scan</Button></div></div></section>;
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
  return <article className="bg-panel/65 p-4"><div className="flex min-w-0 items-start gap-3"><div className="grid size-10 shrink-0 place-items-center rounded-[9px] bg-elevated text-accent"><HardDrive size={18} /></div><div className="min-w-0 flex-1"><div className="flex items-center gap-2"><h3 className="truncate text-sm font-semibold">{source.name}</h3><SourceStatus status={runningJob ? "scanning" : source.status} />{source.private ? <span className="text-[9px] font-semibold uppercase tracking-[0.08em] text-muted">Private</span> : null}</div><p className="mt-1 truncate text-[11px] text-muted" title={source.path}>{source.path}</p></div></div>{runningJob ? <div className="mt-3 border-t border-line/60 pt-3" aria-label={`Scan progress for ${source.name}`}><div className="flex items-center justify-between text-[11px] text-muted"><span className="flex items-center gap-1.5"><LoaderCircle className="animate-spin" size={12} /> Indexing {runningJob.currentPath || "folder"}</span><span className="number-font">{runningJob.indexedCount} added</span></div><p className="mt-1 truncate text-[10px] text-muted/70">{runningJob.discoveredCount} discovered · {runningJob.skippedCount} skipped · {runningJob.warningCount} warnings</p></div> : <p className="mt-3 text-[11px] text-muted">{source.itemCount} indexed · {source.mediaKind === "both" ? "videos and images" : `${source.mediaKind}s`}{source.lastScannedAt ? ` · scanned ${new Date(source.lastScannedAt).toLocaleDateString()}` : " · not scanned"}</p>}{source.lastError && !runningJob ? <p className="mt-2 text-[11px] text-red-400">{source.lastError}</p> : null}{latestJob && !runningJob && latestJob.warningCount > 0 ? <details className="mt-2 text-[11px] text-amber-400"><summary className="cursor-pointer">{latestJob.warningCount} scan warning{latestJob.warningCount === 1 ? "" : "s"}</summary><ul className="mt-1 space-y-1 pl-4 text-muted">{latestJob.warnings.map((warning, index) => <li key={`${latestJob.id}-${index}`} className="list-disc">{warning}</li>)}</ul></details> : null}<div className="mt-3 flex flex-wrap items-center gap-2 border-t border-line/60 pt-3">{runningJob ? <Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => void cancel()}><Ban size={13} /> Cancel scan</Button> : <Button variant="secondary" className="h-7 px-2 text-xs" onClick={() => void scan()}><FolderSearch size={13} /> Scan now</Button>}<label className="flex items-center gap-2 px-1 text-[11px] text-muted"><input type="checkbox" checked={watchEnabled} disabled={watchBusy} onChange={() => void toggleWatch()} /> Auto scan</label>{confirmRemove ? <button className="ml-auto text-[11px] text-muted hover:text-ink" onClick={() => setConfirmRemove(false)}>Keep source</button> : null}<Button variant={confirmRemove ? "danger" : "ghost"} className="ml-auto h-7 px-2 text-xs" disabled={Boolean(runningJob)} onClick={() => void remove()}><Trash2 size={13} /> {confirmRemove ? "Remove catalog records" : "Remove"}</Button></div></article>;
}

function SourceStatus({ status }: { status: LibrarySource["status"] }) {
  const labels = { online: "Online", offline: "Offline", scanning: "Scanning", error: "Needs attention" };
  const colors = { online: "bg-emerald-400/10 text-emerald-400", offline: "bg-amber-400/10 text-amber-400", scanning: "bg-accent/10 text-accent", error: "bg-red-400/10 text-red-400" };
  return <span className={`shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-[0.08em] ${colors[status]}`}>{labels[status]}</span>;
}

function upsertById<T extends { id: string }>(items: T[], next: T): T[] {
  return items.some((item) => item.id === next.id) ? items.map((item) => item.id === next.id ? next : item) : [next, ...items];
}
