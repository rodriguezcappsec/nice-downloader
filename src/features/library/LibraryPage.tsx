import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowRight,
  ArrowDown,
  ArrowUp,
  ArchiveRestore,
  Ban,
  BellDot,
  Bookmark,
  Camera,
  Captions,
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
  GripHorizontal,
  HardDrive,
  Heart,
  ListPlus,
  ListVideo,
  LayoutGrid,
  Layers,
  List,
  Lock,
  Image as ImageIcon,
  Library,
  LoaderCircle,
  MonitorPlay,
  MoreHorizontal,
  Pause,
  Play,
  Database,
  Eraser,
  ExternalLink,
  Maximize,
  Minimize2,
  RotateCcw,
  RotateCw,
  Repeat2,
  Scaling,
  Save,
  Search,
  ScanSearch,
  Share2,
  Square,
  CheckSquare2,
  Star,
  Tags,
  Settings2,
  SkipBack,
  SkipForward,
  Trash2,
  Tv,
  Undo2,
  Volume2,
  VolumeX,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import { memo, useEffect, useMemo, useRef, useState, type CSSProperties, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { Button, Field, Input, Select, Textarea } from "../../components/ui";
import { formatBytes, formatDuration } from "../../lib/format";
import { invalidateLibraryArtwork, loadLibraryArtwork } from "../../lib/library-artwork";
import { assessEmbeddedCompatibility, embeddedPlaybackFingerprint, formatMediaCodec } from "../../lib/media-compatibility";
import { canOfferPlaybackFallback, createPlaybackError, EMBEDDED_PLAYBACK_TIMEOUT_MS } from "../../lib/playback";
import { api, normalizeError } from "../../lib/tauri";
import { useAppStore } from "../../stores/app-store";
import { CreativeTimeline } from "./CreativeTimeline";
import { TranscriptIntelligencePanel } from "./TranscriptIntelligencePanel";
import { readLibraryWorkspace, writeLibraryWorkspace, type LibraryWorkspaceSnapshot } from "./library-workspace";
import type {
  AppError,
  EmbeddedMpvBounds,
  EmbeddedMpvSession,
  EmbeddedMpvStatus,
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
  LibraryHealthSummary,
  LibraryMediaKind,
  LibraryMediaStack,
  LibraryMediaStackRole,
  LibraryPlaybackEntry,
  LibraryPlaybackPreflight,
  LibraryPlaybackPreparationProgress,
  LibraryPlaybackSession,
  LibraryVideoDisplaySettings,
  LibraryVideoFit,
  NativePlaybackSession,
  NativePlaybackStatus,
  LibraryPlaylist,
  LibraryPrivacyRules,
  LibraryPrivacyStatus,
  LibraryRichMetadata,
  LibrarySearchStatus,
  LibraryTranscriptIndexStatus,
  LibraryTranscriptMatch,
  LibrarySmartView,
  LibrarySmartViewQuery,
  LibraryStoryboard,
  LibrarySubtitleTrack,
  LibraryWatchState,
  LibraryQueryRequest,
  LibraryQueryResult,
  LibrarySort,
  LibraryScanJob,
  LibrarySource,
  LibrarySourceAutomation,
} from "../../types";

type MediaFilter = "all" | "video" | "image";
type ResolutionFilter = "all" | "2160" | "1440" | "1080" | "720" | "sd" | "unknown";
type DurationFilter = "all" | "short" | "medium" | "long" | "unknown";
type DateFilter = "all" | "week" | "month" | "year";
type AvailabilityFilter = "all" | "available" | "missing";
type FolderScope = "tree" | "direct";
type FolderSelection = { sourceId: string; path: string };
type FolderOption = FolderSelection & { value: string; label: string; directCount: number; treeCount: number };
type MiniPlayerLayout = { x: number; y: number; width: number };
type MiniPlayerGesture = {
  kind: "move" | "resize";
  pointerId: number;
  startClientX: number;
  startClientY: number;
  startFrame: MiniPlayerLayout;
  startHeight: number;
  previousCursor: string;
  previousUserSelect: string;
};
type SubtitlePosition = "bottom" | "middle" | "top";
type SubtitleColor = "white" | "yellow";
type SubtitleBackground = "none" | "soft" | "strong";
type SubtitleCueOrigin = { startTime: number; endTime: number; line: number | "auto"; snapToLines: boolean };
const DEFAULT_VIDEO_DISPLAY_SETTINGS: LibraryVideoDisplaySettings = {
  fit: "fit",
  zoom: 1,
  panX: 0,
  panY: 0,
  rotation: 0,
  flipHorizontal: false,
  flipVertical: false,
  brightness: 0,
  contrast: 0,
  saturation: 0,
  gamma: 0,
};
const LIBRARY_PAGE_SIZE = 60;
const WATCH_BROWSE_PAGE_SIZE = 24;
const MINI_PLAYER_LAYOUT_KEY = "nice-downloader.mini-player-layout";
const MINI_PLAYER_MARGIN = 16;
const MINI_PLAYER_DEFAULT_WIDTH = 420;
const MINI_PLAYER_MIN_WIDTH = 340;
const MINI_PLAYER_MAX_WIDTH = 720;
const MEDIA_HOVER_PREVIEW_DELAY_MS = 450;
const MEDIA_HOVER_STORYBOARD_FRAMES = 8;
const MEDIA_HOVER_STORYBOARD_CACHE_LIMIT = 12;
const mediaHoverStoryboardCache = new Map<string, LibraryStoryboard>();
const mediaHoverStoryboardRequests = new Map<string, Promise<LibraryStoryboard>>();
const EMPTY_LIBRARY_RESULT: LibraryQueryResult = { items: [], totalItems: 0, page: 1, pageSize: LIBRARY_PAGE_SIZE, totalPages: 1 };
const EMPTY_LIBRARY_FACETS: LibraryFacets = { totalItems: 0, availableItems: 0, missingItems: 0, containers: [], tags: [], creators: [], series: [], sites: [], themes: [], folders: [] };
type LiveAnnouncement = { signature: string; message: string; assertive?: boolean };
type TranscriptSeekRequest = { id: number; itemId: string; seconds: number };

function smartCollectionRules(
  query: LibrarySmartViewQuery,
  sourceNames: Map<string, string>,
  collectionNames: Map<string, string>,
): string[] {
  const rules: string[] = [];
  if (query.search) rules.push(`Search “${query.search}”`);
  if (query.mediaKind === "video") rules.push("Videos");
  if (query.mediaKind === "image") rules.push("Images");
  if (query.sourceId) rules.push(`Source: ${sourceNames.get(query.sourceId) || "Saved source"}`);
  if (query.folderPath != null) rules.push(query.folderPath ? `Folder: ${query.folderPath}` : "Source root only");
  if (query.resolution) rules.push(`${query.resolution} resolution`);
  if (query.container) rules.push(query.container.toLocaleUpperCase());
  if (query.duration) rules.push(`${query.duration} duration`);
  if (query.dateAdded) rules.push(`Added: ${query.dateAdded}`);
  if (query.datePublished) rules.push(`Published: ${query.datePublished}`);
  if (query.availability !== "available") rules.push(query.availability === "missing" ? "Missing files" : "Any availability");
  rules.push(...query.tags.map((tag) => `#${tag}`));
  rules.push(...query.creators.map((creator) => `Creator: ${creator}`));
  rules.push(...query.series.map((series) => `Series: ${series}`));
  rules.push(...query.sites.map((site) => `Site: ${site}`));
  if (query.theme) rules.push(`Theme: ${query.theme}`);
  if (query.rating) rules.push(`Rating: ${query.rating}`);
  if (query.collectionId) rules.push(`Collection: ${collectionNames.get(query.collectionId) || "Saved collection"}`);
  if (query.favoriteOnly) rules.push("Favorites");
  if (query.privateOnly) rules.push("Private media");
  if (query.sort !== "recent") rules.push(`Sort: ${query.sort}`);
  return rules.length > 0 ? rules : ["All available media"];
}

function readMiniPlayerLayout(): MiniPlayerLayout | null {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(MINI_PLAYER_LAYOUT_KEY) || "null") as Partial<MiniPlayerLayout> | null;
    if (!parsed || !Number.isFinite(parsed.x) || !Number.isFinite(parsed.y) || !Number.isFinite(parsed.width)) return null;
    return { x: Number(parsed.x), y: Number(parsed.y), width: Number(parsed.width) };
  } catch {
    return null;
  }
}

function clampMiniPlayerLayout(layout: MiniPlayerLayout, height: number): MiniPlayerLayout {
  const viewportWidth = Math.max(0, window.innerWidth);
  const viewportHeight = Math.max(0, window.innerHeight);
  const availableWidth = Math.max(0, viewportWidth - MINI_PLAYER_MARGIN * 2);
  const minimumWidth = Math.min(MINI_PLAYER_MIN_WIDTH, availableWidth);
  const width = Math.min(Math.max(minimumWidth, layout.width), Math.min(MINI_PLAYER_MAX_WIDTH, availableWidth));
  const maximumX = Math.max(MINI_PLAYER_MARGIN, viewportWidth - width - MINI_PLAYER_MARGIN);
  const visibleHeight = Math.min(Math.max(0, height), Math.max(0, viewportHeight - MINI_PLAYER_MARGIN * 2));
  const maximumY = Math.max(MINI_PLAYER_MARGIN, viewportHeight - visibleHeight - MINI_PLAYER_MARGIN);
  return {
    x: Math.min(Math.max(MINI_PLAYER_MARGIN, layout.x), maximumX),
    y: Math.min(Math.max(MINI_PLAYER_MARGIN, layout.y), maximumY),
    width,
  };
}

function sameMiniPlayerLayout(left: MiniPlayerLayout | null, right: MiniPlayerLayout) {
  return Boolean(left
    && Math.abs(left.x - right.x) < 0.5
    && Math.abs(left.y - right.y) < 0.5
    && Math.abs(left.width - right.width) < 0.5);
}

type LibraryWorkspaceSection = "home" | "all" | "videos" | "images" | "playlists" | "watchLater" | "private";

export function LibraryPage({ active = true, workspace = "library" }: { active?: boolean; workspace?: "home" | "library" }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const currentView = useAppStore((state) => state.view);
  const setView = useAppStore((state) => state.setView);
  const setCompareItems = useAppStore((state) => state.setCompareItems);
  const setImageCompareItems = useAppStore((state) => state.setImageCompareItems);
  const playNextItemIds = useAppStore((state) => state.playNextItemIds);
  const setPlayNextItems = useAppStore((state) => state.setPlayNextItems);
  const removePlayNextItem = useAppStore((state) => state.removePlayNextItem);
  const movePlayNextItem = useAppStore((state) => state.movePlayNextItem);
  const clearPlayNext = useAppStore((state) => state.clearPlayNext);
  const initialWorkspace = useMemo(readLibraryWorkspace, []);
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
  const [smartViewCounts, setSmartViewCounts] = useState<Record<string, number>>({});
  const [smartViewCountsLoading, setSmartViewCountsLoading] = useState(false);
  const [searchStatus, setSearchStatus] = useState<LibrarySearchStatus | null>(null);
  const [transcriptStatus, setTranscriptStatus] = useState<LibraryTranscriptIndexStatus | null>(null);
  const [transcriptMatches, setTranscriptMatches] = useState<LibraryTranscriptMatch[]>([]);
  const [transcriptItems, setTranscriptItems] = useState<Map<string, LibraryItem>>(new Map());
  const [transcriptLoading, setTranscriptLoading] = useState(false);
  const [transcriptRebuilding, setTranscriptRebuilding] = useState(false);
  const [transcriptRevision, setTranscriptRevision] = useState(0);
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
  const [playNextItems, setPlayNextQueueItems] = useState<LibraryItem[]>([]);
  const [changes, setChanges] = useState<LibraryChange[]>([]);
  const [healthSummary, setHealthSummary] = useState<LibraryHealthSummary | null>(null);
  const [curationQueueSummary, setCurationQueueSummary] = useState({ count: 0, capped: false });
  const [duplicateJob, setDuplicateJob] = useState<LibraryDuplicateAnalysisJob | null>(null);
  const [duplicateGroups, setDuplicateGroups] = useState<LibraryDuplicateGroup[]>([]);
  const [fileOperations, setFileOperations] = useState<LibraryFileOperation[]>([]);
  const [search, setSearch] = useState(initialWorkspace.search);
  const [debouncedSearch, setDebouncedSearch] = useState(initialWorkspace.search);
  const [sourceFilter, setSourceFilter] = useState(initialWorkspace.sourceFilter);
  const [folderFilter, setFolderFilter] = useState(initialWorkspace.folderFilter);
  const [folderScope, setFolderScope] = useState<FolderScope>(initialWorkspace.folderScope);
  const [mediaFilter, setMediaFilter] = useState<MediaFilter>(initialWorkspace.mediaFilter);
  const [resolutionFilter, setResolutionFilter] = useState<ResolutionFilter>(initialWorkspace.resolutionFilter);
  const [containerFilter, setContainerFilter] = useState(initialWorkspace.containerFilter);
  const [durationFilter, setDurationFilter] = useState<DurationFilter>(initialWorkspace.durationFilter);
  const [dateFilter, setDateFilter] = useState<DateFilter>(initialWorkspace.dateFilter);
  const [publishedFilter, setPublishedFilter] = useState<DateFilter>(initialWorkspace.publishedFilter);
  const [sort, setSort] = useState<LibrarySort>(initialWorkspace.sort);
  const [availabilityFilter, setAvailabilityFilter] = useState<AvailabilityFilter>(initialWorkspace.availabilityFilter);
  const [tagFilters, setTagFilters] = useState<string[]>(initialWorkspace.tagFilters);
  const [creatorFilters, setCreatorFilters] = useState<string[]>(initialWorkspace.creatorFilters);
  const [seriesFilters, setSeriesFilters] = useState<string[]>(initialWorkspace.seriesFilters);
  const [siteFilters, setSiteFilters] = useState<string[]>(initialWorkspace.siteFilters);
  const [themeFilter, setThemeFilter] = useState(initialWorkspace.themeFilter);
  const [ratingFilter, setRatingFilter] = useState(initialWorkspace.ratingFilter);
  const [collectionFilter, setCollectionFilter] = useState(initialWorkspace.collectionFilter);
  const [favoriteOnly, setFavoriteOnly] = useState(initialWorkspace.favoriteOnly);
  const [moreFiltersOpen, setMoreFiltersOpen] = useState(false);
  const [workspaceSection, setWorkspaceSection] = useState<LibraryWorkspaceSection>(workspace === "home" ? "home" : "all");
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set());
  const [bulkMoveOpen, setBulkMoveOpen] = useState(false);
  const [viewMode, setViewMode] = useState<"grid" | "list">(initialWorkspace.viewMode);
  const [selectedItem, setSelectedItem] = useState<LibraryItem | null>(null);
  const [detailsExtraItems, setDetailsExtraItems] = useState<LibraryItem[]>([]);
  const [watchItem, setWatchItem] = useState<LibraryItem | null>(null);
  const [transcriptSeekRequest, setTranscriptSeekRequest] = useState<TranscriptSeekRequest | null>(null);
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
  const [resultPage, setResultPage] = useState(initialWorkspace.resultPage);
  const [artworkCacheStatus, setArtworkCacheStatus] = useState<LibraryArtworkCacheStatus | null>(null);
  const [queryLoading, setQueryLoading] = useState(true);
  const [libraryCatalogContextLoaded, setLibraryCatalogContextLoaded] = useState(false);
  const [catalogRevision, setCatalogRevision] = useState(0);
  const [watchRevision, setWatchRevision] = useState(0);
  const [privacyStatus, setPrivacyStatus] = useState<LibraryPrivacyStatus | null>(null);
  const [privacyRules, setPrivacyRules] = useState<LibraryPrivacyRules | null>(null);
  const [privacyOnly, setPrivacyOnly] = useState(false);
  const [privacyManagerOpen, setPrivacyManagerOpen] = useState(false);
  const [libraryAnnouncement, setLibraryAnnouncement] = useState("");
  const [libraryAlert, setLibraryAlert] = useState("");
  const refreshTimer = useRef<number | null>(null);
  const watchRefreshTimer = useRef<number | null>(null);
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
  const transcriptSeekSequence = useRef(0);
  const filterResetReady = useRef(false);
  const workspaceScrollTop = useRef(initialWorkspace.scrollTop);
  const workspaceScrollRestored = useRef(false);
  const workspaceSelectionRestored = useRef(false);
  const workspaceSnapshotRef = useRef<LibraryWorkspaceSnapshot>(initialWorkspace);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const loadRules = (status: LibraryPrivacyStatus) => {
      if (!status.configured || status.locked) {
        setPrivacyRules(null);
        setPrivacyOnly(false);
        setPrivacyManagerOpen(false);
        return;
      }
      void api.listLibraryPrivacyRules().then((rules) => {
        if (!disposed) setPrivacyRules(rules);
      }).catch((error) => {
        if (!disposed) setError(normalizeError(error));
      });
    };
    const update = (status: LibraryPrivacyStatus) => {
      if (disposed) return;
      invalidateLibraryArtwork();
      setPrivacyStatus(status);
      loadRules(status);
      setSelectedItem(null);
      setWatchItem(null);
      setSelectedIds(new Set());
      setSelectionMode(false);
      setCatalogRevision((value) => value + 1);
      setWatchRevision((value) => value + 1);
    };
    void api.getLibraryPrivacyStatus().then((status) => {
      if (!disposed) {
        setPrivacyStatus(status);
        loadRules(status);
      }
    }).catch((error) => setError(normalizeError(error)));
    void api.onLibraryPrivacyChanged(update).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setError]);

  function activeElement(): HTMLElement | null {
    return document.activeElement instanceof HTMLElement ? document.activeElement : null;
  }

  function resetMainScroll() {
    const main = document.getElementById("main-content");
    if (main) main.scrollTop = 0;
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

  workspaceSnapshotRef.current = {
    search,
    sourceFilter,
    folderFilter,
    folderScope,
    mediaFilter,
    resolutionFilter,
    containerFilter,
    durationFilter,
    dateFilter,
    publishedFilter,
    sort,
    availabilityFilter,
    tagFilters,
    creatorFilters,
    seriesFilters,
    siteFilters,
    themeFilter,
    ratingFilter,
    collectionFilter,
    favoriteOnly,
    resultPage,
    viewMode,
    selectedItemId: workspaceSelectionRestored.current ? selectedItem?.id || null : initialWorkspace.selectedItemId,
    scrollTop: workspaceScrollTop.current,
  };

  useEffect(() => {
    const timer = window.setTimeout(() => writeLibraryWorkspace(workspaceSnapshotRef.current), 150);
    return () => window.clearTimeout(timer);
  }, [availabilityFilter, collectionFilter, containerFilter, creatorFilters, dateFilter, durationFilter, favoriteOnly, folderFilter, folderScope, mediaFilter, publishedFilter, ratingFilter, resolutionFilter, resultPage, search, selectedItem?.id, seriesFilters, siteFilters, sort, sourceFilter, tagFilters, themeFilter, viewMode]);

  useEffect(() => {
    if (!active) return;
    const main = document.getElementById("main-content");
    if (!main) return;
    if (!queryLoading && !workspaceScrollRestored.current) {
      workspaceScrollRestored.current = true;
      window.requestAnimationFrame(() => { main.scrollTop = workspaceScrollTop.current; });
    }
    let timer = 0;
    const onScroll = () => {
      workspaceScrollTop.current = main.scrollTop;
      window.clearTimeout(timer);
      timer = window.setTimeout(() => writeLibraryWorkspace({ ...workspaceSnapshotRef.current, scrollTop: main.scrollTop }), 180);
    };
    main.addEventListener("scroll", onScroll, { passive: true });
    return () => { window.clearTimeout(timer); main.removeEventListener("scroll", onScroll); };
  }, [active, queryLoading]);

  useEffect(() => {
    if (!active || workspaceSelectionRestored.current) return;
    workspaceSelectionRestored.current = true;
    const itemId = initialWorkspace.selectedItemId;
    if (!itemId) return;
    let disposed = false;
    void api.getLibraryItemsByIds([itemId])
      .then(([item]) => {
        if (disposed) return;
        if (item) setSelectedItem(item);
        else writeLibraryWorkspace({ ...workspaceSnapshotRef.current, selectedItemId: null });
      })
      .catch(() => writeLibraryWorkspace({ ...workspaceSnapshotRef.current, selectedItemId: null }));
    return () => { disposed = true; };
  }, [active, initialWorkspace.selectedItemId]);

  useEffect(() => {
    let disposed = false;
    if (!playNextItemIds.length) {
      setPlayNextQueueItems([]);
      return;
    }
    void api.getLibraryItemsByIds(playNextItemIds).then((items) => {
      if (disposed) return;
      const byId = new Map(items.filter((item) => item.mediaKind === "video" && !item.missing).map((item) => [item.id, item]));
      const ordered = playNextItemIds.flatMap((id) => byId.get(id) ? [byId.get(id)!] : []);
      setPlayNextQueueItems(ordered);
      const validIds = ordered.map((item) => item.id);
      if (validIds.length !== playNextItemIds.length) setPlayNextItems(validIds);
    }).catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [playNextItemIds, setError, setPlayNextItems]);

  useEffect(() => {
    const applyHealthFilter = (event: Event) => {
      const detail = (event as CustomEvent<{ availability?: AvailabilityFilter; manage?: boolean }>).detail || {};
      if (detail.availability) {
        resetFilters();
        setAvailabilityFilter(detail.availability);
        setResultPage(1);
      }
      if (detail.manage) setManageOpen(true);
    };
    window.addEventListener("nice-downloader:library-health-filter", applyHealthFilter);
    return () => window.removeEventListener("nice-downloader:library-health-filter", applyHealthFilter);
  }, []);

  useEffect(() => {
    const applyThemeFilter = (event: Event) => {
      const key = (event as CustomEvent<{ key?: string }>).detail?.key;
      if (!key) return;
      resetFilters();
      setThemeFilter(key);
      setResultPage(1);
    };
    window.addEventListener("nice-downloader:library-theme-filter", applyThemeFilter);
    return () => window.removeEventListener("nice-downloader:library-theme-filter", applyThemeFilter);
  }, []);

  function startWatching(item: LibraryItem, queue: LibraryItem[] | null = null, startAtSeconds?: number) {
    // The native surface reads viewport-relative DOM geometry as soon as Watch mounts.
    // Reset a scrolled catalog before mounting it so Videos -> Details -> Watch cannot
    // initialize libmpv against the catalog's transient off-screen position.
    resetMainScroll();
    setViewImage(null);
    imageReturnFocus.current = null;
    setTranscriptSeekRequest(Number.isFinite(startAtSeconds)
      ? { id: ++transcriptSeekSequence.current, itemId: item.id, seconds: Math.max(0, startAtSeconds!) }
      : null);
    setWatchItem(item);
    setPlaybackContext(queue);
    if (queue) setWatchSearch("");
    setWatchMinimized(false);
    setView("library");
  }

  function playFirstQueued() {
    const next = playNextItems[0];
    if (!next) return;
    removePlayNextItem(next.id);
    setPlayNextQueueItems((current) => current.filter((item) => item.id !== next.id));
    setSelectedItem(null);
    setPlaybackContext(null);
    setTranscriptSeekRequest(null);
    setWatchItem(next);
    setWatchMinimized(false);
  }

  function playQueuedItem(item: LibraryItem) {
    removePlayNextItem(item.id);
    setPlayNextQueueItems((current) => current.filter((candidate) => candidate.id !== item.id));
    setSelectedItem(null);
    setPlaybackContext(null);
    setTranscriptSeekRequest(null);
    setWatchItem(item);
    setWatchMinimized(false);
  }

  async function rebuildTranscriptIndex() {
    if (transcriptRebuilding) return;
    setTranscriptRebuilding(true);
    try {
      const status = await api.rebuildLibraryTranscriptIndex();
      setTranscriptStatus(status);
      setTranscriptRevision((revision) => revision + 1);
      setNotice({
        message: `Indexed ${status.cueCount.toLocaleString()} subtitle cue${status.cueCount === 1 ? "" : "s"} across ${status.indexedItems.toLocaleString()} video${status.indexedItems === 1 ? "" : "s"}.`,
        view: "library",
      });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setTranscriptRebuilding(false);
    }
  }

  function startViewingImage(item: LibraryItem, returnTarget = activeElement()) {
    if (!viewImage) imageReturnFocus.current = returnTarget;
    setViewImage(item);
    if (watchItem) setWatchMinimized(true);
    setView("library");
    resetMainScroll();
  }

  function closeImageViewer() {
    const target = imageReturnFocus.current;
    imageReturnFocus.current = null;
    setViewImage(null);
    restoreFocus(target, "library-search");
  }

  function minimizeWatching() {
    setWatchMinimized(true);
    scheduleWatchRefresh(0);
    restoreFocus(null, "library-search");
  }

  function restoreWatching() {
    resetMainScroll();
    setViewImage(null);
    imageReturnFocus.current = null;
    setView("library");
    setWatchMinimized(false);
    restoreFocus(null, "watch-details-button");
  }

  function closeWatching() {
    setWatchItem(null);
    setPlaybackContext(null);
    setWatchMinimized(false);
    scheduleWatchRefresh(0);
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
      setLibraryCatalogContextLoaded(true);
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
      setCatalogRevision((revision) => revision + 1);
    }, delay);
  }

  function scheduleWatchRefresh(delay = 150) {
    if (watchRefreshTimer.current != null) window.clearTimeout(watchRefreshTimer.current);
    watchRefreshTimer.current = window.setTimeout(() => {
      watchRefreshTimer.current = null;
      setWatchRevision((revision) => revision + 1);
    }, delay);
  }

  useEffect(() => {
    const refreshWatchShelves = () => scheduleWatchRefresh(0);
    const refreshCatalog = () => scheduleLibraryRefresh(0);
    window.addEventListener("nice-downloader:library-watch-state", refreshWatchShelves);
    window.addEventListener("nice-downloader:library-items-changed", refreshCatalog);
    return () => {
      window.removeEventListener("nice-downloader:library-watch-state", refreshWatchShelves);
      window.removeEventListener("nice-downloader:library-items-changed", refreshCatalog);
    };
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
        void Promise.all([api.listLibrarySources(), api.getLibrarySearchStatus(), api.getLibraryTranscriptIndexStatus()]).then(([nextSources, nextSearchStatus, nextTranscriptStatus]) => {
          if (!disposed) {
            setSources(nextSources);
            setSearchStatus(nextSearchStatus);
            setTranscriptStatus(nextTranscriptStatus);
            setTranscriptRevision((revision) => revision + 1);
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
      if (watchRefreshTimer.current != null) window.clearTimeout(watchRefreshTimer.current);
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedSearch(search.trim()), 250);
    return () => window.clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    let disposed = false;
    let unlisten = () => {};
    void api.onLibraryThemeAnalysisUpdated((job) => {
      if (!disposed && job.state !== "running") scheduleLibraryRefresh(0);
    }).then((cleanup) => disposed ? cleanup() : unlisten = cleanup);
    return () => { disposed = true; unlisten(); };
  }, []);

  useEffect(() => {
    void api.getLibraryTranscriptIndexStatus()
      .then(setTranscriptStatus)
      .catch(() => setTranscriptStatus(null));
  }, []);

  useEffect(() => {
    const query = debouncedSearch.trim();
    if (query.length < 2) {
      setTranscriptMatches([]);
      setTranscriptItems(new Map());
      setTranscriptLoading(false);
      return;
    }
    let disposed = false;
    setTranscriptLoading(true);
    void api.searchLibraryTranscripts(query, 12)
      .then(async (matches) => {
        const ids = Array.from(new Set(matches.map((match) => match.itemId)));
        const items = ids.length > 0 ? await api.getLibraryItemsByIds(ids) : [];
        if (disposed) return;
        setTranscriptMatches(matches);
        setTranscriptItems(new Map(items.map((item) => [item.id, item])));
      })
      .catch(() => {
        if (!disposed) {
          setTranscriptMatches([]);
          setTranscriptItems(new Map());
        }
      })
      .finally(() => { if (!disposed) setTranscriptLoading(false); });
    return () => { disposed = true; };
  }, [debouncedSearch, transcriptRevision]);

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
  }, [catalogRevision, folderFilter, folderScope, setError, sourceFilter, watchBrowsePage, watchDebouncedSearch, watchItem]);

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
      theme: themeFilter === "all" ? null : themeFilter,
      rating: ratingFilter === "all" ? null : ratingFilter,
      collectionId: collectionFilter === "all" ? null : collectionFilter,
      favoriteOnly,
      privateOnly: privacyOnly,
      sort,
    };
  }, [availabilityFilter, collectionFilter, containerFilter, creatorFilters, dateFilter, debouncedSearch, durationFilter, favoriteOnly, folderFilter, folderScope, mediaFilter, privacyOnly, publishedFilter, ratingFilter, resolutionFilter, resultPage, seriesFilters, siteFilters, sort, sourceFilter, tagFilters, themeFilter]);
  const currentSmartViewQuery = useMemo<LibrarySmartViewQuery>(() => {
    const { page: _page, pageSize: _pageSize, ...query } = libraryRequest;
    return { ...query, search: search.trim() || null };
  }, [libraryRequest, search]);

  const visibleSmartViews = useMemo(
    () => smartViews.filter((view) => !view.query.privateOnly || Boolean(privacyStatus?.configured && !privacyStatus.locked)),
    [privacyStatus?.configured, privacyStatus?.locked, smartViews],
  );

  useEffect(() => {
    if (!active || visibleSmartViews.length === 0) {
      setSmartViewCounts({});
      setSmartViewCountsLoading(false);
      return;
    }
    let disposed = false;
    setSmartViewCountsLoading(true);
    void api.getLibrarySmartViewCounts()
      .then((counts) => {
        if (!disposed) setSmartViewCounts(counts);
      })
      .catch(() => {
        if (!disposed) setSmartViewCounts({});
      })
      .finally(() => {
        if (!disposed) setSmartViewCountsLoading(false);
      });
    return () => { disposed = true; };
  }, [active, catalogRevision, visibleSmartViews]);

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
  }, [catalogRevision, libraryRequest, resultPage, setError]);

  useEffect(() => {
    let disposed = false;
    void api.getLibraryHomeSnapshot().then((snapshot) => {
      if (!disposed) {
        setRecentResult(snapshot.recent);
        setVideoResult(snapshot.videos);
        setImageResult(snapshot.images);
        setContinueWatching(snapshot.continueWatching);
        setWatchLater(snapshot.watchLater);
        setRecentlyWatched(snapshot.recentlyWatched);
      }
    }).catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [catalogRevision, setError, watchRevision]);

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
  }, [availabilityFilter, catalogRevision, setError, sourceFilter]);

  useEffect(() => {
    let disposed = false;
    void Promise.all([
      api.getLibraryHealthSummary(),
      api.listLibraryCurationQueue(false, 500),
      api.listLibraryCurationQueue(true, 500),
    ]).then(([health, unreviewed, reviewLater]) => {
      if (disposed) return;
      setHealthSummary(health);
      setCurationQueueSummary({
        count: unreviewed.length + reviewLater.length,
        capped: unreviewed.length === 500 || reviewLater.length === 500,
      });
    }).catch((error) => { if (!disposed) setError(normalizeError(error)); });
    return () => { disposed = true; };
  }, [catalogRevision, setError]);

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
  }, [catalogRevision, changes, duplicateGroups, manageOpen, setError]);

const sourceNames = useMemo(() => new Map(sources.map((source) => [source.id, source.name])), [sources]);
const latestScanWarningCount = useMemo(() => {
    const visibleSourceIds = new Set(sources.map((source) => source.id));
    const latestBySource = new Map<string, LibraryScanJob>();
    for (const job of scanJobs) {
      if (!visibleSourceIds.has(job.sourceId)) continue;
      const current = latestBySource.get(job.sourceId);
      if (!current || Date.parse(job.updatedAt) > Date.parse(current.updatedAt)) latestBySource.set(job.sourceId, job);
    }
    return Array.from(latestBySource.values()).reduce((total, job) => total + job.warningCount, 0);
  }, [scanJobs, sources]);
const collectionNames = useMemo(() => new Map(collections.map((collection) => [collection.id, collection.name])), [collections]);
const smartCollectionCards = useMemo(() => visibleSmartViews.map((view) => ({
    view,
    rules: smartCollectionRules(view.query, sourceNames, collectionNames),
  })), [collectionNames, sourceNames, visibleSmartViews]);
const items = queryResult.items;
const privateMediaIds = useMemo(() => {
    if (!privacyRules || privacyStatus?.locked) return new Set<string>();
    const explicitItems = new Set(privacyRules.itemIds);
    const privateSources = new Set(sources.filter((source) => source.private).map((source) => source.id));
    const candidates = [
      ...queryResult.items,
      ...recentResult.items,
      ...videoResult.items,
      ...imageResult.items,
      ...continueWatching.map((entry) => entry.item),
      ...watchLater,
      ...recentlyWatched.map((entry) => entry.item),
    ];
    return new Set(candidates.filter((item) => {
      if (explicitItems.has(item.id) || (item.sourceId && privateSources.has(item.sourceId))) return true;
      const relativePath = (item.relativePath || "").replaceAll("\\", "/").replace(/^\/+|\/+$/g, "");
      return privacyRules.folders.some((folder) => folder.sourceId === item.sourceId
        && (!folder.relativePath || relativePath === folder.relativePath || relativePath.startsWith(`${folder.relativePath}/`)));
    }).map((item) => item.id));
  }, [continueWatching, imageResult.items, privacyRules, privacyStatus?.locked, queryResult.items, recentResult.items, recentlyWatched, sources, videoResult.items, watchLater]);
  const missingItemCount = facets.missingItems;
  const availableItemCount = facets.availableItems;
  const containers = facets.containers;
  const tags = facets.tags;
  const creators = facets.creators;
  const seriesOptions = facets.series;
  const sites = facets.sites;
  const themes = facets.themes;
  const folderOptions = useMemo(() => buildFolderOptions(facets.folders, sources, sourceFilter), [facets.folders, sourceFilter, sources]);
  const featured = continueWatching[0]?.item || watchLater[0] || recentResult.items[0];
  const videos = videoResult.items;
  const images = imageResult.items;
  const resultPageCount = queryResult.totalPages;
  const currentResultPage = queryResult.page;
  const resultStart = queryResult.totalItems === 0 ? 0 : (currentResultPage - 1) * queryResult.pageSize;
  const resultEnd = Math.min(resultStart + items.length, queryResult.totalItems);
const isFiltered = Boolean(search.trim()) || sourceFilter !== "all" || folderFilter !== "all" || mediaFilter !== "all" || resolutionFilter !== "all" || containerFilter !== "all" || durationFilter !== "all" || dateFilter !== "all" || publishedFilter !== "all" || availabilityFilter !== "available" || tagFilters.length > 0 || creatorFilters.length > 0 || seriesFilters.length > 0 || siteFilters.length > 0 || themeFilter !== "all" || ratingFilter !== "all" || collectionFilter !== "all" || favoriteOnly || privacyOnly || sort !== "recent";
  const secondaryFilterCount = [
    resolutionFilter !== "all",
    containerFilter !== "all",
    durationFilter !== "all",
    dateFilter !== "all",
    publishedFilter !== "all",
    availabilityFilter !== "available",
    themeFilter !== "all",
    ratingFilter !== "all",
    sort !== "recent",
  ].filter(Boolean).length + tagFilters.length + creatorFilters.length + seriesFilters.length + siteFilters.length;
const resultTitle = search.trim()
    ? `Results for “${search.trim()}”`
    : privacyOnly
      ? "Private media"
    : themeFilter !== "all"
      ? `Theme: ${themes.find((theme) => theme.key === themeFilter)?.label || themeFilter}`
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
    if (!filterResetReady.current) {
      filterResetReady.current = true;
      return;
    }
    setResultPage(1);
  }, [availabilityFilter, collectionFilter, containerFilter, creatorFilters, dateFilter, durationFilter, favoriteOnly, folderFilter, folderScope, mediaFilter, privacyOnly, publishedFilter, ratingFilter, resolutionFilter, search, selectionMode, seriesFilters, siteFilters, sort, sourceFilter, tagFilters, themeFilter]);

  useEffect(() => {
    if (folderFilter === "all") return;
    if (facetsContext !== `${sourceFilter}:${availabilityFilter}`) return;
    const selected = parseFolderFilter(folderFilter);
    if (!selected || !folderOptions.some((option) => option.value === folderFilter) || (sourceFilter !== "all" && selected.sourceId !== sourceFilter)) {
      setFolderFilter("all");
      setFolderScope("tree");
    }
  }, [availabilityFilter, facetsContext, folderFilter, folderOptions, sourceFilter]);

  useEffect(() => {
    if (!libraryCatalogContextLoaded) return;
    if (sourceFilter !== "all" && !sources.some((source) => source.id === sourceFilter)) setSourceFilter("all");
    if (collectionFilter !== "all" && !collections.some((collection) => collection.id === collectionFilter)) setCollectionFilter("all");
    if (facetsContext === `${sourceFilter}:${availabilityFilter}` && containerFilter !== "all" && !containers.includes(containerFilter)) setContainerFilter("all");
    if (facetsContext === `${sourceFilter}:${availabilityFilter}` && themeFilter !== "all" && !themes.some((theme) => theme.key === themeFilter)) setThemeFilter("all");
  }, [availabilityFilter, collectionFilter, collections, containerFilter, containers, facetsContext, libraryCatalogContextLoaded, sourceFilter, sources, themeFilter, themes]);

  const navigationItems = useMemo(() => activeMediaStack ? activeMediaStackItems : workspaceSection === "all" || isFiltered || selectionMode
    ? Array.from(new Map([...items, ...detailsExtraItems].map((item) => [item.id, item])).values())
    : Array.from(new Map([...continueWatching.map((entry) => entry.item), ...watchLater, ...recentlyWatched.map((entry) => entry.item), ...recentResult.items, ...videos, ...images, ...detailsExtraItems].map((item) => [item.id, item])).values()),
    [activeMediaStack, activeMediaStackItems, continueWatching, detailsExtraItems, images, isFiltered, items, recentResult.items, recentlyWatched, selectionMode, videos, watchLater, workspaceSection]);
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

  async function setSelectedPrivacy(privateValue: boolean) {
    if (selectedIds.size === 0 || busy) return;
    if (!privacyStatus?.configured || privacyStatus.locked) {
      setNotice({ message: privacyStatus?.configured ? "Unlock the private Library in Settings first." : "Enable Library privacy in Settings first.", view: "library" });
      setView("settings");
      return;
    }
    setBusy(true);
    try {
      const rules = await api.setLibraryItemsPrivate({ itemIds: Array.from(selectedIds), private: privateValue });
      setPrivacyRules(rules);
      setPrivacyStatus(await api.getLibraryPrivacyStatus());
      setNotice({ message: privateValue ? `Marked ${selectedIds.size} selected item${selectedIds.size === 1 ? "" : "s"} private.` : `Removed the individual privacy rule from ${selectedIds.size} selected item${selectedIds.size === 1 ? "" : "s"}.`, view: "library" });
      closeSelectionMode();
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function setCurrentFolderPrivacy(privateValue: boolean) {
    const folder = parseFolderFilter(folderFilter);
    if (!folder) return;
    if (!privacyStatus?.configured || privacyStatus.locked) {
      setNotice({ message: privacyStatus?.configured ? "Unlock the private Library in Settings first." : "Enable Library privacy in Settings first.", view: "library" });
      setView("settings");
      return;
    }
    setBusy(true);
    try {
      const rules = await api.setLibraryFolderPrivate({ sourceId: folder.sourceId, relativePath: folder.path, private: privateValue });
      setPrivacyRules(rules);
      setPrivacyStatus(await api.getLibraryPrivacyStatus());
      setNotice({ message: privateValue ? "This folder and its nested media are now private." : "Removed the privacy rule from this folder.", view: "library" });
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removePrivateItemRule(itemId: string) {
    if (busy || privacyStatus?.locked) return;
    setBusy(true);
    try {
      setPrivacyRules(await api.setLibraryItemsPrivate({ itemIds: [itemId], private: false }));
      setPrivacyStatus(await api.getLibraryPrivacyStatus());
      setNotice({ message: "Removed the individual privacy rule.", view: "library" });
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removePrivateFolderRule(sourceId: string, relativePath: string) {
    if (busy || privacyStatus?.locked) return;
    setBusy(true);
    try {
      setPrivacyRules(await api.setLibraryFolderPrivate({ sourceId, relativePath, private: false }));
      setPrivacyStatus(await api.getLibraryPrivacyStatus());
      setNotice({ message: "Removed the private-folder rule.", view: "library" });
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function removePrivateSourceRule(source: LibrarySource) {
    if (busy || privacyStatus?.locked) return;
    setBusy(true);
    try {
      const changed = await api.saveLibrarySource({ id: source.id, name: source.name, path: source.path, mediaKind: source.mediaKind, recursive: source.recursive, watchEnabled: source.watchEnabled, private: false, includePatterns: source.includePatterns, excludePatterns: source.excludePatterns });
      setSources((current) => current.map((candidate) => candidate.id === changed.id ? changed : candidate));
      setPrivacyStatus(await api.getLibraryPrivacyStatus());
      setNotice({ message: `${source.name} is no longer a private source.`, view: "library" });
      scheduleLibraryRefresh(0);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
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
    setThemeFilter("all");
    setRatingFilter("all"); setCollectionFilter("all"); setFavoriteOnly(false);
    setPrivacyOnly(false);
    setSort("recent");
  }

  function openWorkspaceSection(section: LibraryWorkspaceSection) {
    setWorkspaceSection(section);
    setActivePlaylist(null);
    setActivePlaylistItems([]);
    setSearch("");
    resetFilters();
    setResultPage(1);
    if (section === "home") {
      setView("home");
      return;
    }
    setView("library");
    if (section === "videos") setMediaFilter("video");
    if (section === "images") setMediaFilter("image");
    if (section === "private") setPrivacyOnly(true);
  }

  function openLibraryManagerSection(targetId: string) {
    setManageOpen(true);
    window.setTimeout(() => {
      const target = document.getElementById(targetId);
      if (target) target.tabIndex = -1;
      target?.scrollIntoView({ block: "start" });
      target?.focus();
    }, 0);
  }

  function openMissingMedia() {
    setWorkspaceSection("all");
    setActivePlaylist(null);
    setActivePlaylistItems([]);
    setSearch("");
    resetFilters();
    setAvailabilityFilter("missing");
    setResultPage(1);
    setView("library");
  }

  const previousWorkspace = useRef(workspace);
  useEffect(() => {
    if (!active || previousWorkspace.current === workspace) return;
    previousWorkspace.current = workspace;
    openWorkspaceSection(workspace === "home" ? "home" : "all");
  }, [active, workspace]);

  function applySmartView(view: LibrarySmartView) {
    const query = view.query;
    if (query.privateOnly && (!privacyStatus?.configured || privacyStatus.locked)) {
      setNotice({ message: "Unlock Private Library in Settings to open this Smart Collection.", view: "library" });
      return;
    }
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
    setThemeFilter(query.theme || "all");
    setRatingFilter(query.rating || "all");
    setCollectionFilter(query.collectionId || "all");
    setFavoriteOnly(query.favoriteOnly);
    setPrivacyOnly(Boolean(query.privateOnly && privacyStatus?.configured && !privacyStatus.locked));
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
      invalidateLibraryArtwork();
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
  const browseNextVideo = watchItem && watchPlaybackQueue.length > 1
    ? watchIndex >= 0 && watchIndex + 1 < watchPlaybackQueue.length
      ? watchPlaybackQueue[watchIndex + 1]
      : orderedPlayback ? null : watchPlaybackQueue[0]
    : null;
  const nextVideo = playNextItems[0] || browseNextVideo;
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
    playNextItems={playNextItems}
    seekRequest={transcriptSeekRequest?.itemId === watchItem.id ? transcriptSeekRequest : null}
    onSearch={setWatchSearch}
    onBack={minimizeWatching}
    onRestore={restoreWatching}
    onClose={closeWatching}
    onDetails={() => openDetails(watchItem)}
    onSelectVideo={(item) => { setSelectedItem(null); setWatchItem(item); setWatchMinimized(false); }}
    onPrevious={previousVideo ? () => setWatchItem(previousVideo) : undefined}
    onNext={nextVideo ? playNextItems.length ? playFirstQueued : () => setWatchItem(nextVideo) : undefined}
    onAutoplayNext={updateAutoplayNext}
    onPlaylistChanged={updatePlaylist}
    onPlaybackEnded={playNextItems.length ? playFirstQueued : autoplayNext && autoNextVideo ? () => setWatchItem(autoNextVideo) : undefined}
    onPlayNextSelect={playQueuedItem}
    onPlayNextMove={(itemId, newIndex) => {
      movePlayNextItem(itemId, newIndex);
      setPlayNextQueueItems((current) => {
        const currentIndex = current.findIndex((item) => item.id === itemId);
        if (currentIndex < 0) return current;
        const next = [...current];
        const [moved] = next.splice(currentIndex, 1);
        next.splice(Math.min(Math.max(0, newIndex), next.length), 0, moved);
        return next;
      });
    }}
    onPlayNextRemove={(itemId) => {
      removePlayNextItem(itemId);
      setPlayNextQueueItems((current) => current.filter((item) => item.id !== itemId));
    }}
    onPlayNextClear={() => {
      clearPlayNext();
      setPlayNextQueueItems([]);
    }}
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
        <WatchAutomationSection sources={sources} collections={collections} />
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
      <><div className={`${active && !viewImage && (!watchItem || compactPlayer) ? "" : "hidden"} workspace-enter mx-auto w-full max-w-[1560px] px-8 pb-16 pt-7 xl:px-11`}>
      <LibraryLiveRegions polite={libraryAnnouncement} assertive={libraryAlert} />
        <header className="sticky top-0 z-10 -mx-3 flex items-center gap-6 border-b border-line/30 bg-canvas/[0.88] px-3 py-4 backdrop-blur-xl">
          <div className="min-w-fit"><p className="mb-1 text-[11px] font-semibold tracking-[0.1em] text-accent">Screening room</p><h1 className="text-[30px] font-semibold leading-none tracking-[-0.05em]">Your library</h1><p className="mt-1.5 text-[12px] text-muted">{availableItemCount} available item{availableItemCount === 1 ? "" : "s"} across {sources.length} source{sources.length === 1 ? "" : "s"}{missingItemCount > 0 ? <button className="ml-1 font-medium text-amber-300/85 hover:text-amber-200" onClick={() => { resetFilters(); setAvailabilityFilter("missing"); }}>· {missingItemCount} missing</button> : null}</p></div>
        <label className="relative mx-auto w-full max-w-2xl"><span className="sr-only">Search library</span><Search className="absolute left-4 top-3.5 text-muted" size={17} /><Input id="library-search" className="h-12 rounded-[12px] bg-panel/85 pl-11 pr-4 text-sm shadow-control" value={search} onChange={(event) => { setActivePlaylist(null); setWorkspaceSection("all"); setView("library"); setSearch(event.target.value); }} placeholder="Search titles, creators, tags, transcripts, and folders" /></label>
        <div className="flex shrink-0 items-center gap-1"><div className="flex rounded-[8px] bg-elevated/70 p-0.5" role="group" aria-label="Library view"><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "grid" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("grid")} aria-pressed={viewMode === "grid"} aria-label="Grid view"><LayoutGrid size={15} /></button><button className={`grid size-8 place-items-center rounded-[6px] transition ${viewMode === "list" ? "bg-panel text-ink shadow-control" : "text-muted hover:text-ink"}`} onClick={() => changeViewMode("list")} aria-pressed={viewMode === "list"} aria-label="List view"><List size={15} /></button></div><Button variant={selectionMode ? "secondary" : "ghost"} onClick={() => selectionMode ? closeSelectionMode() : setSelectionMode(true)}>{selectionMode ? <X size={16} /> : <CheckSquare2 size={16} />} {selectionMode ? "Done" : "Select"}</Button><Button variant={changes.length > 0 ? "secondary" : "ghost"} onClick={() => { setManageOpen(true); void api.listLibraryFileOperations().then(setFileOperations).catch((error) => setError(normalizeError(error))); }}><Settings2 size={16} /> Manage{changes.length > 0 ? <span className="number-font rounded bg-accent/15 px-1.5 py-0.5 text-[10px] text-accent">{changes.length}</span> : null}</Button></div>
      </header>

      <nav className="library-tabs mt-4 flex min-w-0 items-center gap-1 overflow-x-auto border-b border-line/50 pb-2" aria-label="Library sections">
        <LibraryTab active={workspaceSection === "home"} onClick={() => openWorkspaceSection("home")}>Home</LibraryTab>
        <LibraryTab active={workspaceSection === "all"} onClick={() => openWorkspaceSection("all")}>All Media</LibraryTab>
        <LibraryTab active={workspaceSection === "videos"} onClick={() => openWorkspaceSection("videos")}><Film size={14} /> Videos</LibraryTab>
        <LibraryTab active={workspaceSection === "images"} onClick={() => openWorkspaceSection("images")}><ImageIcon size={14} /> Images</LibraryTab>
        <LibraryTab active={workspaceSection === "playlists"} onClick={() => openWorkspaceSection("playlists")}><ListPlus size={14} /> Playlists</LibraryTab>
        <LibraryTab active={workspaceSection === "watchLater"} onClick={() => openWorkspaceSection("watchLater")}><Bookmark size={14} /> Watch Later</LibraryTab>
        {privacyStatus?.configured && !privacyStatus.locked ? <LibraryTab active={workspaceSection === "private"} onClick={() => openWorkspaceSection("private")}><Lock size={14} /> Private</LibraryTab> : null}
      </nav>

      <div className="mt-3 flex items-center justify-end gap-2">
        <Button variant={privacyStatus?.configured ? "secondary" : "ghost"} onClick={() => { if (privacyStatus?.configured && !privacyStatus.locked) void api.lockLibraryPrivacy().then(setPrivacyStatus).catch((error) => setError(normalizeError(error))); else setView("settings"); }} title={privacyStatus?.locked ? "Unlock in Settings" : privacyStatus?.configured ? "Lock private media now" : "Set up Library privacy"}>
          <Lock size={15} /> {privacyStatus?.locked ? "Private media locked" : privacyStatus?.configured ? "Lock private media" : "Set up privacy"}
        </Button>
        {privacyStatus?.configured && !privacyStatus.locked ? <Button variant="ghost" disabled={!privacyRules} onClick={() => setPrivacyManagerOpen(true)}><Settings2 size={14} /> Manage private media</Button> : null}
        {folderFilter !== "all" && privacyStatus?.configured && !privacyStatus.locked ? <><Button variant="ghost" disabled={busy} onClick={() => void setCurrentFolderPrivacy(true)}><Lock size={14} /> Make current folder private</Button><Button variant="ghost" disabled={busy} onClick={() => void setCurrentFolderPrivacy(false)}>Remove folder privacy rule</Button></> : null}
      </div>

      {workspaceSection === "home" ? <LibraryActivityInbox
        curationCount={curationQueueSummary.count}
        curationCapped={curationQueueSummary.capped}
        changeCount={changes.length}
        scanWarningCount={latestScanWarningCount}
        health={healthSummary}
        onOpenCuration={() => setView("curation")}
        onOpenChanges={() => openLibraryManagerSection("library-review-title")}
        onOpenScans={() => openLibraryManagerSection("library-sources-title")}
        onOpenMissing={openMissingMedia}
        onOpenHealth={() => setView("health")}
      /> : null}

      <div className={workspaceSection === "home" ? "hidden" : ""}>
      <section className="mt-4 rounded-panel bg-panel/55 p-4 shadow-control" aria-labelledby="smart-collections-heading">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div>
            <h2 id="smart-collections-heading" className="flex items-center gap-2 text-sm font-semibold"><Bookmark className="text-accent" size={16} /> Smart Collections</h2>
            <p className="mt-1 max-w-2xl text-xs leading-5 text-muted">Saved rules update automatically as media is added, inspected, rated, or organized.</p>
          </div>
          <div className="flex flex-wrap items-center justify-end gap-2">
            {smartViewFormOpen ? <><Input aria-label="Smart Collection name" className="h-9 w-[210px] text-xs" value={smartViewName} maxLength={80} autoFocus placeholder="Collection name" onChange={(event) => setSmartViewName(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void saveCurrentSmartView(); if (event.key === "Escape") { setSmartViewFormOpen(false); setSmartViewName(""); } }} /><Button className="h-9 text-xs" disabled={!smartViewName.trim() || smartViewBusy} onClick={() => void saveCurrentSmartView()}>{smartViewBusy ? <LoaderCircle className="animate-spin" size={13} /> : <Save size={13} />} Save</Button><Button variant="ghost" className="h-9 text-xs" onClick={() => { setSmartViewFormOpen(false); setSmartViewName(""); }}>Cancel</Button></> : <Button variant="secondary" className="h-9 text-xs" onClick={() => setSmartViewFormOpen(true)}><Bookmark size={13} /> Create from current filters</Button>}
            <button className={`inline-flex h-9 items-center gap-1.5 rounded-[7px] px-2.5 text-[11px] transition hover:bg-elevated hover:text-ink ${searchStatus?.consistent === false ? "text-amber-300" : "text-muted"}`} disabled={smartViewBusy} onClick={() => void rebuildSearchIndex()} title="Rebuild the local Library search index"><Database size={13} />{searchStatus?.consistent === false ? "Repair search" : searchStatus ? "Search ready" : "Search index"}</button>
          </div>
        </div>
        {smartCollectionCards.length === 0 ? <div className="mt-4 rounded-[10px] border border-dashed border-line/70 px-4 py-5 text-center"><p className="text-sm font-medium">Build a shelf around what matters to you</p><p className="mx-auto mt-1 max-w-xl text-xs leading-5 text-muted">Choose any combination of search, creator, folder, theme, format, rating, or availability filters, then save those rules here.</p></div> : <div className="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-3">{smartCollectionCards.map(({ view, rules }) => {
          const count = smartViewCounts[view.id];
          const previewRules = rules.slice(0, 3);
          return <article key={view.id} className={`group relative min-w-0 rounded-[10px] border p-3 transition ${activeSmartViewId === view.id ? "border-accent/45 bg-accent/10" : "border-line/60 bg-elevated/45 hover:border-line"}`}>
            <div className="flex items-start gap-3"><button className="min-w-0 flex-1 text-left" onClick={() => applySmartView(view)} aria-label={`Open Smart Collection ${view.name}`}><span className="flex items-center gap-2"><span className="truncate text-sm font-semibold text-ink">{view.name}</span>{view.query.privateOnly ? <Lock className="shrink-0 text-muted" size={12} aria-label="Private rules" /> : null}</span><span className="mt-1 block text-[11px] text-muted">{smartViewCountsLoading && count == null ? "Updating count…" : count == null ? "Count unavailable" : `${count.toLocaleString()} matching item${count === 1 ? "" : "s"}`}</span></button><button className={`grid size-8 shrink-0 place-items-center rounded-[7px] transition ${confirmDeleteSmartViewId === view.id ? "bg-red-400/12 text-red-300" : "text-muted hover:bg-panel hover:text-ink"}`} disabled={smartViewBusy} onClick={() => void deleteSmartView(view.id)} aria-label={confirmDeleteSmartViewId === view.id ? `Confirm removing ${view.name}` : `Remove ${view.name}`} title={confirmDeleteSmartViewId === view.id ? "Click again to confirm" : "Remove Smart Collection"}>{confirmDeleteSmartViewId === view.id ? <Trash2 size={14} /> : <X size={14} />}</button></div>
            <button className="mt-3 flex w-full flex-wrap gap-1.5 text-left" onClick={() => applySmartView(view)} aria-label={`Apply rules for ${view.name}`}>{previewRules.map((rule) => <span key={rule} className="max-w-full truncate rounded-[6px] bg-panel/75 px-2 py-1 text-[10px] text-muted">{rule}</span>)}{rules.length > previewRules.length ? <span className="rounded-[6px] bg-panel/75 px-2 py-1 text-[10px] text-muted">+{rules.length - previewRules.length} rules</span> : null}</button>
          </article>;
        })}</div>}
        {smartViews.length > visibleSmartViews.length ? <p className="mt-3 flex items-center gap-1.5 text-[11px] text-muted"><Lock size={12} /> Private Smart Collections stay hidden while the Private Library is locked.</p> : null}
      </section>

      {searchStatus && !searchStatus.consistent ? <div className="mt-3 flex items-center gap-3 border-l-2 border-amber-400/60 bg-amber-400/5 px-3 py-2 text-xs text-amber-100" role="alert"><span>Search index has {searchStatus.indexedItems.toLocaleString()} of {searchStatus.catalogItems.toLocaleString()} Library items.</span><Button variant="secondary" className="ml-auto h-8 text-xs" disabled={smartViewBusy} onClick={() => void rebuildSearchIndex()}>{smartViewBusy ? <LoaderCircle className="animate-spin" size={13} /> : <RotateCcw size={13} />} Rebuild search</Button></div> : null}

 <nav className="mt-5 flex flex-wrap items-center gap-2 rounded-panel bg-panel/55 p-2.5 shadow-control" aria-label="Library filters">
      <FilterChip active={favoriteOnly} onClick={() => setFavoriteOnly((value) => !value)}><Heart size={14} fill={favoriteOnly ? "currentColor" : "none"} /> Favorites</FilterChip>
      <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        {sources.map((source) => <FilterChip key={source.id} active={sourceFilter === source.id} onClick={() => { const next = sourceFilter === source.id ? "all" : source.id; setSourceFilter(next); const selectedFolder = parseFolderFilter(folderFilter); if (next === "all" || selectedFolder?.sourceId !== next) { setFolderFilter("all"); setFolderScope("tree"); } }}><span className="grid size-5 place-items-center rounded-[5px] bg-accent/12 text-[10px] font-bold text-accent">{source.name.charAt(0).toLocaleUpperCase()}</span>{source.name}</FilterChip>)}
        {collections.map((collection) => <FilterChip key={collection.id} active={collectionFilter === collection.id} onClick={() => setCollectionFilter(collectionFilter === collection.id ? "all" : collection.id)}><ListPlus size={14} />{collection.name}</FilterChip>)}
        <span className="mx-1 h-6 w-px shrink-0 bg-line/70" />
        <label className="relative shrink-0"><span className="sr-only">Filter by source folder</span><FolderOpen className="pointer-events-none absolute left-3 top-2.5 text-muted" size={14} strokeWidth={1.8} /><Select className="h-9 w-[220px] bg-elevated/70 pl-9 text-xs" value={folderFilter} onChange={(event) => { const value = event.target.value; setFolderFilter(value); const folder = parseFolderFilter(value); if (!folder) setFolderScope("tree"); else if (folder.path === "") setFolderScope("direct"); }} disabled={folderOptions.length === 0}><option value="all">{folderOptions.length === 0 ? "No indexed folders" : "All folders"}</option>{folderOptions.map((option) => <option key={option.value} value={option.value}>{option.label} ({folderScope === "direct" ? option.directCount : option.treeCount})</option>)}</Select></label>
 <label className="shrink-0"><span className="sr-only">Choose whether nested folders are included</span><Select className="h-9 w-[145px] bg-elevated/70 text-xs" value={folderScope} onChange={(event) => setFolderScope(event.target.value as FolderScope)} disabled={folderFilter === "all"}><option value="tree">Include nested</option><option value="direct">This folder only</option></Select></label>
 <Button variant="secondary" className="ml-auto h-9 shrink-0 px-3 text-xs" onClick={() => setMoreFiltersOpen((open) => !open)} aria-expanded={moreFiltersOpen} aria-controls="library-more-filters"><Settings2 size={14} /> More filters{secondaryFilterCount > 0 ? <span className="number-font grid size-5 place-items-center rounded-[6px] bg-accent/15 text-[10px] font-semibold text-accent">{secondaryFilterCount}</span> : null}<ChevronRight className={`transition-transform ${moreFiltersOpen ? "rotate-90" : ""}`} size={13} /></Button>
 </nav>
 {moreFiltersOpen ? <section id="library-more-filters" className="panel-shadow mt-2 rounded-panel border border-line/55 bg-panel/75 p-4" aria-label="More library filters"><div className="mb-3 flex items-center justify-between gap-3"><div><h2 className="text-xs font-semibold">Refine this view</h2><p className="mt-1 text-[10px] text-muted">Narrow technical details, origin, dates, and personal organization.</p></div>{secondaryFilterCount > 0 ? <button className="shrink-0 text-[10px] font-semibold text-accent transition hover:text-ink" onClick={() => { setResolutionFilter("all"); setContainerFilter("all"); setDurationFilter("all"); setDateFilter("all"); setPublishedFilter("all"); setAvailabilityFilter("available"); setTagFilters([]); setCreatorFilters([]); setSeriesFilters([]); setSiteFilters([]); setThemeFilter("all"); setRatingFilter("all"); setSort("recent"); }}>Clear active filters</button> : null}</div><div className="grid grid-cols-[repeat(auto-fit,minmax(145px,1fr))] items-end gap-2 [&_label]:min-w-0 [&_select]:w-full">
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
        <label className="shrink-0"><span className="sr-only">Filter by AI-discovered theme</span><Select className="h-9 w-[175px] bg-elevated/70 text-xs" value={themeFilter} onChange={(event) => setThemeFilter(event.target.value)}><option value="all">Any theme</option>{themes.map((theme) => <option key={theme.key} value={theme.key}>{theme.label} ({theme.itemCount})</option>)}</Select></label>
        <label className="shrink-0"><span className="sr-only">Filter by rating</span><Select className="h-9 w-[120px] bg-elevated/70 text-xs" value={ratingFilter} onChange={(event) => setRatingFilter(event.target.value)}><option value="all">Any rating</option><option value="rated">Rated</option><option value="5">5 stars</option><option value="4">4 stars</option><option value="3">3 stars</option><option value="unrated">Unrated</option></Select></label>
        <label className="shrink-0"><span className="sr-only">Sort library results</span><Select className="h-9 w-[175px] bg-elevated/70 text-xs" value={sort} onChange={(event) => setSort(event.target.value as LibrarySort)}><option value="recent">Newest added</option><option value="oldest">Oldest added</option><option value="publishedDesc">Newest published</option><option value="publishedAsc">Oldest published</option><option value="fileDateDesc">Newest file date</option><option value="fileDateAsc">Oldest file date</option><option value="titleAsc">Title A–Z</option><option value="titleDesc">Title Z–A</option><option value="durationDesc">Longest first</option><option value="durationAsc">Shortest first</option><option value="sizeDesc">Largest first</option><option value="sizeAsc">Smallest first</option></Select></label>
 </div></section> : null}

      {tagFilters.length + creatorFilters.length + seriesFilters.length + siteFilters.length > 0 ? <div className="mt-1 flex flex-wrap items-center gap-2" aria-label="Active Library filters"><span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-muted">Matching any within each group</span>{tagFilters.map((value) => <ActiveFilterChip key={`tag:${value}`} label={`#${value}`} onRemove={() => setTagFilters((current) => current.filter((item) => item !== value))} />)}{creatorFilters.map((value) => <ActiveFilterChip key={`creator:${value}`} label={`Creator: ${value}`} onRemove={() => setCreatorFilters((current) => current.filter((item) => item !== value))} />)}{seriesFilters.map((value) => <ActiveFilterChip key={`series:${value}`} label={`Series: ${value}`} onRemove={() => setSeriesFilters((current) => current.filter((item) => item !== value))} />)}{siteFilters.map((value) => <ActiveFilterChip key={`site:${value}`} label={`Site: ${value}`} onRemove={() => setSiteFilters((current) => current.filter((item) => item !== value))} />)}<button className="text-[11px] font-medium text-muted hover:text-ink" onClick={() => { setTagFilters([]); setCreatorFilters([]); setSeriesFilters([]); setSiteFilters([]); }}>Clear groups</button></div> : null}
      {themeFilter !== "all" ? <div className="mt-1 flex flex-wrap items-center gap-2" aria-label="Active theme filter"><span className="text-[10px] font-semibold uppercase tracking-[0.12em] text-muted">AI theme</span><ActiveFilterChip label={themes.find((theme) => theme.key === themeFilter)?.label || themeFilter} onRemove={() => setThemeFilter("all")} /></div> : null}

 <TranscriptIntelligencePanel
 selectedItemIds={Array.from(selectedIds)}
 query={debouncedSearch}
 onTranscriptChanged={() => {
   void api.getLibraryTranscriptIndexStatus().then(setTranscriptStatus).catch(() => undefined);
   setTranscriptRevision((revision) => revision + 1);
 }}
 onPlay={(match) => {
   setSelectedItem(null);
   startWatching(match.item, null, match.startSeconds);
 }}
 />

      {debouncedSearch.length >= 2 ? <TranscriptSearchResults
          query={debouncedSearch}
          matches={transcriptMatches}
          items={transcriptItems}
          loading={transcriptLoading}
          status={transcriptStatus}
          rebuilding={transcriptRebuilding}
          onRebuild={() => void rebuildTranscriptIndex()}
          onSelect={(match, item) => {
            setSelectedItem(null);
            startWatching(item, null, match.startSeconds);
          }}
      /> : null}
      </div>

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
        ) : workspaceSection === "playlists" ? (
          <PlaylistShelf playlists={playlists} onOpen={(playlist) => void openPlaylist(playlist)} onChanged={setPlaylists} />
        ) : workspaceSection === "watchLater" ? (
          <MediaSection title="Watch Later" subtitle={`${watchLater.length} saved video${watchLater.length === 1 ? "" : "s"}`} items={watchLater} sourceNames={sourceNames} onSelect={openDetails} privateItemIds={privateMediaIds} viewMode={viewMode} />
        ) : workspaceSection !== "home" || isFiltered || selectionMode ? (
        <>{discoveryContext && !selectionMode ? <DiscoveryHeader kind={discoveryContext.kind} value={discoveryContext.value} totalItems={queryResult.totalItems} representative={queryResult.items[0]} onClear={() => discoveryContext.kind === "creator" ? setCreatorFilters([]) : setSeriesFilters([])} /> : null}<div id="library-results" className={`scroll-mt-24 transition-opacity ${queryLoading ? "opacity-55" : "opacity-100"}`} aria-busy={queryLoading}><MediaSection title={selectionMode ? "Select media" : resultTitle} subtitle={queryLoading ? "Updating results…" : queryResult.totalItems === 0 ? "No matching items" : selectionMode ? `${selectedIds.size} selected · showing ${resultStart + 1}–${resultEnd} of ${queryResult.totalItems}` : `Showing ${resultStart + 1}–${resultEnd} of ${queryResult.totalItems} matching item${queryResult.totalItems === 1 ? "" : "s"}`} items={items} sourceNames={sourceNames} onSelect={openDetails} selectionMode={selectionMode} selectedIds={selectedIds} onToggleSelection={toggleSelection} privateItemIds={privateMediaIds} viewMode={viewMode} /></div><LibraryPagination currentPage={currentResultPage} pageCount={resultPageCount} totalItems={queryResult.totalItems} onPageChange={changeResultPage} /></>
      ) : (
        <>
          {mediaStacks.length > 0 ? <MediaStackShelf stacks={mediaStacks} onOpen={(stack) => void openMediaStack(stack)} onChanged={setMediaStacks} /> : null}
          {playlists.length > 0 ? <PlaylistShelf playlists={playlists} onOpen={(playlist) => void openPlaylist(playlist)} onChanged={setPlaylists} /> : null}

          {!featured && mediaStacks.length === 0 && playlists.length === 0 && continueWatching.length === 0 && watchLater.length === 0 && recentlyWatched.length === 0 && recentResult.items.length === 0 && videos.length === 0 && images.length === 0 ? <HomeLibraryEmpty privateLocked={Boolean(privacyStatus?.configured && privacyStatus.locked)} onAction={() => privacyStatus?.configured && privacyStatus.locked ? setView("settings") : openWorkspaceSection("all")} /> : null}
        {featured ? <FeaturedMedia item={featured} sourceName={sourceLabel(featured, sourceNames)} privateMedia={privateMediaIds.has(featured.id)} resumeProgress={continueProgress.get(featured.id)} initialWatchLater={watchLater.some((item) => item.id === featured.id)} onSelect={() => openDetails(featured)} onWatch={() => featured.mediaKind === "video" ? startWatching(featured) : startViewingImage(featured)} /> : null}
          {continueWatching.length > 0 ? <MediaSection horizontal title="Continue watching" subtitle="Pick up where you left off" items={continueWatching.map((entry) => entry.item)} progress={continueProgress} sourceNames={sourceNames} onSelect={startWatching} onSeeAll={() => openWorkspaceSection("all")} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
          {watchLater.length > 0 ? <MediaSection horizontal title="Watch later" subtitle="Videos you saved for another time" items={watchLater} sourceNames={sourceNames} onSelect={startWatching} onSeeAll={() => openWorkspaceSection("watchLater")} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
          {recentlyWatched.length > 0 ? <MediaSection horizontal title="Recently watched" subtitle="Your private playback history on this device" items={recentlyWatched.map((entry) => entry.item)} sourceNames={sourceNames} onSelect={startWatching} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
          {recentResult.items.length > 0 ? <MediaSection horizontal title="Recently added" subtitle="The newest media across your folders" items={recentResult.items} sourceNames={sourceNames} onSelect={openDetails} onSeeAll={() => openWorkspaceSection("all")} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
          {videos.length > 0 ? <MediaSection horizontal title="Videos" subtitle={`${videoResult.totalItems} video${videoResult.totalItems === 1 ? "" : "s"} ready to watch`} items={videos} sourceNames={sourceNames} onSelect={openDetails} onSeeAll={() => openWorkspaceSection("videos")} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
          {images.length > 0 ? <MediaSection horizontal title="Images" subtitle={`${imageResult.totalItems} image${imageResult.totalItems === 1 ? "" : "s"} in your catalog`} items={images} sourceNames={sourceNames} onSelect={openDetails} onSeeAll={() => openWorkspaceSection("images")} privateItemIds={privateMediaIds} viewMode={viewMode} /> : null}
        </>
      )}

</div>{privacyManagerOpen && privacyStatus && privacyRules ? <PrivacyManagerDialog status={privacyStatus} rules={privacyRules} sources={sources} busy={busy} onClose={() => setPrivacyManagerOpen(false)} onShowPrivate={() => { setPrivacyManagerOpen(false); setActivePlaylist(null); setActivePlaylistItems([]); setPrivacyOnly(true); setResultPage(1); }} onOpenItem={(item) => { setPrivacyManagerOpen(false); openDetails(item); }} onRemoveItem={removePrivateItemRule} onRemoveFolder={removePrivateFolderRule} onRemoveSource={removePrivateSourceRule} /> : null}{selectionMode ? <BulkActionBar selectedIds={selectedIds} collections={collections} busy={busy} onCompare={openCompareWorkspace} onCompareImages={openImageCompareWorkspace} onCreateStack={openStackCreator} onMove={openBulkMove} onSetPrivacy={setSelectedPrivacy} onApply={applyBulkAction} onClose={closeSelectionMode} /> : null}{bulkMoveOpen ? <BulkMoveDialog selectedIds={selectedIds} onClose={closeBulkMove} onComplete={(result) => { applyBulkMoveResult(result); closeBulkMove(); }} /> : null}{stackDraftItems ? <MediaStackDialog items={stackDraftItems} onClose={() => setStackDraftItems(null)} onSaved={(stack) => { setMediaStacks((current) => upsertById(current, stack)); setStackDraftItems(null); closeSelectionMode(); setNotice({ message: `Created media stack “${stack.name}”.`, view: "library" }); void openMediaStack(stack); }} /> : null}{imageWorkspace}{active && selectedItem ? <div className="relative z-[80]"><MediaDetails item={selectedItem} sourceName={sourceLabel(selectedItem, sourceNames)} onClose={closeDetails} onWatch={openSelectedMedia} onChanged={updateItem} onMetadataChanged={() => scheduleLibraryRefresh(0)} onBrowseCreator={(creator) => { closeDetails(false); resetFilters(); setCreatorFilters([creator]); setResultPage(1); }} onBrowseSeries={(series) => { closeDetails(false); resetFilters(); setSeriesFilters([series]); setResultPage(1); }} onBrowseSite={(site) => { closeDetails(false); resetFilters(); setSiteFilters([site]); setResultPage(1); }} onSelectRelated={openRelatedDetails} onRemoved={() => removeItem(selectedItem.id)} onOperation={applyFileOperationResult} /></div> : null}{watchWorkspace}</>
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

function LibraryTab({ active, onClick, children }: { active: boolean; onClick: () => void; children: React.ReactNode }) {
  return <button className={`relative flex h-10 shrink-0 items-center gap-2 rounded-[9px] px-3 text-[13px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${active ? "bg-elevated/80 text-ink after:absolute after:inset-x-3 after:-bottom-[9px] after:h-0.5 after:rounded-full after:bg-accent" : "text-muted hover:bg-elevated/45 hover:text-ink"}`} onClick={onClick} aria-current={active ? "page" : undefined}>{children}</button>;
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
  return <section className="ingest-card relative mt-8 min-h-[480px] overflow-hidden rounded-media border border-line/40 px-10 py-16"><div className="absolute -right-24 -top-28 size-[32rem] rounded-full bg-accent/8 blur-3xl" /><div className="relative max-w-xl"><div className="brand-mark relative mb-8 grid size-16 place-items-center rounded-[17px] bg-accent text-accentInk"><Play size={27} fill="currentColor" /></div><p className="text-[11px] font-semibold tracking-[0.08em] text-accent">Your private screening room</p><h2 className="mt-4 text-[44px] font-semibold leading-[1.01] tracking-[-0.06em] text-balance">Bring every video and image into one library.</h2><p className="mt-5 max-w-lg text-[14px] leading-6 text-muted">Add folders from this computer or an external drive. Nice Downloader indexes them in place, so your originals stay exactly where they are.</p><Button className="mt-8 h-11" onClick={onAdd}><FolderPlus size={17} /> Add your first folder</Button></div></section>;
}

function HomeLibraryEmpty({ privateLocked, onAction }: { privateLocked: boolean; onAction: () => void }) {
  return <section className="ingest-card relative mt-7 min-h-[310px] overflow-hidden rounded-media border border-line/40 px-8 py-12"><div className="absolute -right-24 -top-36 size-[28rem] rounded-full bg-accent/8 blur-3xl" /><div className="relative max-w-xl"><div className="brand-mark mb-6 grid size-12 place-items-center rounded-[14px] bg-accent/12 text-accent">{privateLocked ? <Lock size={21} /> : <Library size={21} />}</div><p className="eyebrow">Your private media room</p><h2 className="mt-3 text-[32px] font-semibold leading-[1.08] tracking-[-0.045em] text-balance">{privateLocked ? "Your private shelves are locked." : "Your shelves are ready for something worth watching."}</h2><p className="mt-4 max-w-lg text-[13px] leading-6 text-muted">{privateLocked ? "Unlock the private library in Settings to restore its artwork, activity, search results, and recommendations." : "No available media matches the current library state. Browse the complete catalog to review filters, missing files, and indexed sources."}</p><Button className="mt-6 h-10" variant={privateLocked ? "secondary" : "primary"} onClick={onAction}>{privateLocked ? <Lock size={15} /> : <Library size={15} />} {privateLocked ? "Open privacy settings" : "Browse all media"}</Button></div></section>;
}

function TranscriptSearchResults({ query, matches, items, loading, status, rebuilding, onRebuild, onSelect }: { query: string; matches: LibraryTranscriptMatch[]; items: Map<string, LibraryItem>; loading: boolean; status: LibraryTranscriptIndexStatus | null; rebuilding: boolean; onRebuild: () => void; onSelect: (match: LibraryTranscriptMatch, item: LibraryItem) => void }) {
  return <section className="mt-5 overflow-hidden rounded-[11px] border border-line/60 bg-panel/45" aria-labelledby="transcript-results-title" aria-busy={loading || rebuilding}>
    <header className="flex flex-wrap items-center gap-3 border-b border-line/55 px-4 py-3"><span className="grid size-8 place-items-center rounded-[7px] bg-accent/10 text-accent"><Captions size={16} /></span><span className="min-w-0 flex-1"><h2 id="transcript-results-title" className="text-xs font-semibold">Spoken in your videos</h2><p className="mt-0.5 text-[10px] text-muted">Searches trusted local WebVTT subtitles · {status ? `${status.cueCount.toLocaleString()} cues in ${status.indexedItems.toLocaleString()} videos` : "index status unavailable"}</p></span><Button variant="ghost" className="h-8 text-xs" disabled={rebuilding} onClick={onRebuild}>{rebuilding ? <LoaderCircle className="animate-spin" size={13} /> : <RotateCcw size={13} />} {status?.cueCount ? "Refresh subtitles" : "Index subtitles"}</Button></header>
    {loading ? <p className="flex items-center gap-2 px-4 py-5 text-xs text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={14} /> Searching subtitle text…</p> : matches.length > 0 ? <div className="grid divide-y divide-line/45">{matches.map((match, index) => { const item = items.get(match.itemId); return <button key={`${match.itemId}-${match.startSeconds}-${index}`} className="group grid min-w-0 grid-cols-[70px_minmax(0,1fr)_auto] items-center gap-3 px-4 py-3 text-left transition hover:bg-elevated/55 disabled:cursor-not-allowed disabled:opacity-55" disabled={!item} onClick={() => item && onSelect(match, item)}><span className="text-center"><span className="number-font block rounded-[6px] bg-elevated px-2 py-1 text-[10px] font-semibold text-accent">{formatDuration(match.startSeconds)}</span>{match.language ? <span className="mt-1 block text-[9px] font-semibold uppercase tracking-[0.08em] text-muted">{match.language}</span> : null}</span><span className="min-w-0"><strong className="block truncate text-[11px] font-semibold text-ink">{item?.title || "Loading video…"}</strong><span className="mt-1 line-clamp-2 block text-[11px] leading-4 text-muted">“{match.text}”</span></span><span className="flex items-center gap-1 text-[10px] font-semibold text-muted transition group-hover:text-accent"><Play size={12} fill="currentColor" /> Play</span></button>; })}</div> : <div className="flex flex-wrap items-center gap-3 px-4 py-5 text-xs text-muted"><span className="flex-1">{status?.cueCount ? `No subtitle text matched “${query}”.` : "No subtitle cues are indexed yet. Scan your sources or index existing WebVTT tracks now."}</span>{!status?.cueCount ? <Button variant="secondary" className="h-8 text-xs" disabled={rebuilding} onClick={onRebuild}>{rebuilding ? <LoaderCircle className="animate-spin" size={13} /> : <Captions size={13} />} Index subtitles</Button> : null}</div>}
  </section>;
}

function FeaturedMedia({ item, sourceName, privateMedia, resumeProgress, initialWatchLater, onSelect, onWatch }: { item: LibraryItem; sourceName: string; privateMedia: boolean; resumeProgress?: number; initialWatchLater: boolean; onSelect: () => void; onWatch: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const [watchLater, setWatchLater] = useState(initialWatchLater);
  const [saving, setSaving] = useState(false);
  async function toggleWatchLater() {
    if (saving || item.mediaKind !== "video") return;
    setSaving(true);
    try {
      const state = await api.setLibraryWatchLater(item.id, !watchLater);
      setWatchLater(state.watchLater);
      window.dispatchEvent(new Event("nice-downloader:library-watch-state"));
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setSaving(false);
    }
  }
  const resumable = item.mediaKind === "video" && resumeProgress != null && resumeProgress > 0;
  return <section className="featured-media panel-shadow relative mt-6 grid min-h-[330px] max-h-[420px] overflow-hidden rounded-media border border-line/35 bg-panel lg:grid-cols-[1.35fr_1fr]"><div className="absolute inset-0 scale-105 opacity-30 blur-[2px]"><MediaArtwork item={item} eager /></div><div className="absolute inset-0 bg-gradient-to-r from-canvas via-canvas/95 to-canvas/25" /><div className="relative z-[1] flex flex-col justify-end px-8 py-8 lg:px-10"><p className="eyebrow">{resumable ? "Continue watching" : watchLater ? "From Watch Later" : "Featured from your library"}</p><div className="mt-3 flex flex-wrap items-center gap-2">{privateMedia ? <span className="inline-flex h-6 items-center gap-1 rounded-[6px] bg-elevated/85 px-2 text-[11px] font-semibold text-muted"><Lock size={11} /> Private</span> : null}<span className="rounded-[6px] bg-elevated/75 px-2 py-1 text-[11px] text-muted">{item.mediaKind === "image" ? "Image" : "Video"}</span></div><h2 className="mt-3 max-w-2xl text-[clamp(1.8rem,3.1vw,2.75rem)] font-semibold leading-[1.03] tracking-[-0.052em] text-balance">{item.title}</h2><p className="mt-3 text-[13px] text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""} · {new Date(item.addedAt).toLocaleDateString()}</p>{item.mediaKind === "video" ? <div className="mt-3 opacity-80"><EmbeddedCompatibilityLabel item={item} variant="badge" /></div> : null}{resumable ? <div className="mt-4 max-w-xl"><div className="mb-1.5 flex items-center justify-between text-[11px] text-muted"><span>Resume progress</span><span className="number-font">{Math.round(resumeProgress)}%</span></div><div className="h-1.5 overflow-hidden rounded-full bg-elevated"><div className="h-full rounded-full bg-accent" style={{ width: `${resumeProgress}%` }} /></div></div> : null}<div className="mt-6 flex flex-wrap gap-2"><Button onClick={onWatch}><Play size={16} fill="currentColor" /> {item.mediaKind === "image" ? "View" : resumable ? "Resume" : "Watch"}</Button><Button variant="secondary" onClick={onSelect}>Details</Button>{item.mediaKind === "video" ? <Button variant="ghost" disabled={saving} onClick={() => void toggleWatchLater()}><Bookmark size={15} fill={watchLater ? "currentColor" : "none"} /> {watchLater ? "Saved" : "Watch Later"}</Button> : null}</div></div><button className="group relative z-[1] m-6 hidden overflow-hidden rounded-[16px] border border-white/10 bg-elevated shadow-[0_30px_80px_rgb(0_0_0/0.4)] lg:block" onClick={onWatch} aria-label={`Open ${item.title}`}><MediaArtwork item={item} eager /><span className="absolute inset-0 grid place-items-center bg-black/0 transition duration-300 group-hover:bg-black/25 group-focus-visible:bg-black/25"><span className="grid size-14 scale-90 place-items-center rounded-full bg-white/90 text-black opacity-0 shadow-xl transition duration-300 group-hover:scale-100 group-hover:opacity-100 group-focus-visible:scale-100 group-focus-visible:opacity-100"><Play size={22} fill="currentColor" /></span></span></button></section>;
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

function LibraryActivityInbox({ curationCount, curationCapped, changeCount, scanWarningCount, health, onOpenCuration, onOpenChanges, onOpenScans, onOpenMissing, onOpenHealth }: { curationCount: number; curationCapped: boolean; changeCount: number; scanWarningCount: number; health: LibraryHealthSummary | null; onOpenCuration: () => void; onOpenChanges: () => void; onOpenScans: () => void; onOpenMissing: () => void; onOpenHealth: () => void }) {
  const activities = [
    { key: "curation", label: "Curation queue", detail: "New and saved-for-later media awaiting a decision", count: curationCount, countLabel: curationCapped ? `${curationCount}+` : String(curationCount), action: onOpenCuration },
    { key: "changes", label: "Library changes", detail: "Added, moved, restored, or changed files to review", count: changeCount, countLabel: String(changeCount), action: onOpenChanges },
    { key: "scans", label: "Scan warnings", detail: "Warnings from the latest scan of each watched folder", count: scanWarningCount, countLabel: String(scanWarningCount), action: onOpenScans },
    { key: "missing", label: "Missing media", detail: "Catalog entries whose original file is unavailable", count: health?.missingItems || 0, countLabel: String(health?.missingItems || 0), action: onOpenMissing },
    { key: "metadata", label: "Inspection failures", detail: "Files whose technical metadata could not be inspected", count: health?.metadataFailedItems || 0, countLabel: String(health?.metadataFailedItems || 0), action: onOpenHealth },
    { key: "health", label: "Other health notices", detail: "Pending inspection and conservative playback advisories", count: (health?.metadataPendingItems || 0) + (health?.compatibilityRiskItems || 0), countLabel: String((health?.metadataPendingItems || 0) + (health?.compatibilityRiskItems || 0)), action: onOpenHealth },
  ].filter((activity) => activity.count > 0);
  const total = activities.reduce((sum, activity) => sum + activity.count, 0);

  function iconFor(key: string) {
    if (key === "curation") return <CheckSquare2 size={15} />;
    if (key === "changes") return <BellDot size={15} />;
    if (key === "scans") return <FolderSearch size={15} />;
    if (key === "missing") return <FileVideo size={15} />;
    if (key === "metadata") return <Database size={15} />;
    return <Film size={15} />;
  }

  return <details className="group mt-4 overflow-hidden rounded-[12px] border border-line/55 bg-panel/55 shadow-control">
    <summary className="flex min-h-12 cursor-pointer list-none items-center gap-3 px-4 py-2.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent/70">
      <span className={`grid size-8 shrink-0 place-items-center rounded-[8px] ${total ? "bg-accent/10 text-accent" : "bg-elevated text-muted"}`}><BellDot size={15} /></span>
      <span className="min-w-0 flex-1"><strong className="block text-[12px] font-semibold">Library activity</strong><span className="mt-0.5 block truncate text-[10px] text-muted">{total ? `${total.toLocaleString()} attention signal${total === 1 ? "" : "s"} across review and media health` : "Everything visible is reviewed and healthy"}</span></span>
      {total ? <span className="number-font rounded-[6px] bg-accent/10 px-2 py-1 text-[10px] font-semibold text-accent">{total.toLocaleString()}</span> : <Check size={15} className="text-emerald-300" />}
      <ChevronRight className="text-muted transition-transform group-open:rotate-90" size={15} />
    </summary>
    <div className="border-t border-line/50 p-3">
      {activities.length ? <div className="grid gap-2 md:grid-cols-2 xl:grid-cols-3">{activities.map((activity) => <button key={activity.key} className="flex min-h-16 items-center gap-3 rounded-[9px] bg-elevated/40 px-3 py-2.5 text-left transition hover:bg-elevated/75 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={activity.action}><span className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-panel text-accent">{iconFor(activity.key)}</span><span className="min-w-0 flex-1"><strong className="block text-[11px] font-semibold">{activity.label}</strong><span className="mt-0.5 block text-[9px] leading-4 text-muted">{activity.detail}</span></span><span className="number-font shrink-0 text-xs font-semibold text-accent">{activity.countLabel}</span></button>)}</div> : <div className="flex items-center gap-3 px-2 py-3 text-[11px] text-muted"><Check className="text-emerald-300" size={16} /><span>No visible media needs review. Private activity remains hidden while the vault is locked.</span></div>}
    </div>
  </details>;
}

function MediaSection({ title, subtitle, items, sourceNames, onSelect, onSeeAll, progress, horizontal = false, selectionMode = false, selectedIds = new Set(), onToggleSelection, privateItemIds = new Set(), viewMode = "grid" }: { title: string; subtitle: string; items: LibraryItem[]; sourceNames: Map<string, string>; onSelect: (item: LibraryItem) => void; onSeeAll?: () => void; progress?: Map<string, number>; horizontal?: boolean; selectionMode?: boolean; selectedIds?: Set<string>; onToggleSelection?: (id: string) => void; privateItemIds?: Set<string>; viewMode?: "grid" | "list" }) {
  const shelfRef = useRef<HTMLDivElement | null>(null);
  const shelf = horizontal && viewMode === "grid" && !selectionMode;
  function moveShelf(direction: -1 | 1) {
    const element = shelfRef.current;
    if (!element) return;
    const reduced = document.documentElement.classList.contains("reduce-motion") || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    element.scrollBy({ left: direction * Math.max(320, element.clientWidth * 0.72), behavior: reduced ? "auto" : "smooth" });
  }
  const headingId = `library-${title.replace(/\W+/g, "-").toLocaleLowerCase()}`;
  return <section className="mt-10" aria-labelledby={headingId}><div className="mb-4 flex items-end justify-between gap-4"><div><h2 id={headingId} className="text-[21px] font-semibold tracking-[-0.035em]">{title}</h2><p className="mt-1 text-[12px] text-muted">{subtitle}</p></div><div className="flex items-center gap-2">{onSeeAll ? <button className="h-9 rounded-control px-3 text-[12px] font-semibold text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onSeeAll}>See all</button> : null}{shelf && items.length > 1 ? <div className="flex gap-1"><button className="grid size-9 place-items-center rounded-full bg-elevated/70 text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => moveShelf(-1)} aria-label={`Scroll ${title} left`}><ChevronRight className="rotate-180" size={16} /></button><button className="grid size-9 place-items-center rounded-full bg-elevated/70 text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => moveShelf(1)} aria-label={`Scroll ${title} right`}><ChevronRight size={16} /></button></div> : null}</div></div>{items.length > 0 ? <div ref={shelfRef} tabIndex={shelf ? 0 : undefined} onKeyDown={shelf ? (event) => { if (event.key === "ArrowLeft") { event.preventDefault(); moveShelf(-1); } else if (event.key === "ArrowRight") { event.preventDefault(); moveShelf(1); } } : undefined} className={shelf ? "media-shelf flex snap-x snap-mandatory gap-4 overflow-x-auto pb-3 outline-none focus-visible:ring-2 focus-visible:ring-accent/60" : viewMode === "grid" ? "grid gap-x-4 gap-y-7 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4" : "grid gap-2"} aria-label={shelf ? `${title} media shelf. Use Left and Right Arrow keys to scroll.` : undefined}>{items.map((item) => <div key={item.id} className={shelf ? "w-[clamp(220px,24vw,330px)] shrink-0 snap-start" : ""}><MediaCard item={item} sourceName={sourceLabel(item, sourceNames)} progress={progress?.get(item.id)} selected={selectedIds.has(item.id)} selectionMode={selectionMode} privateMedia={privateItemIds.has(item.id)} initialWatchLater={title.toLocaleLowerCase() === "watch later"} viewMode={viewMode} onSelect={() => selectionMode ? onToggleSelection?.(item.id) : onSelect(item)} /></div>)}</div> : <div className="border border-dashed border-line/70 px-6 py-12 text-center text-sm text-muted">Nothing matches these filters.</div>}</section>;
}

function mediaHoverStoryboardKey(item: LibraryItem) {
  return `${item.id}:${item.modifiedAtMs ?? item.updatedAt}:${MEDIA_HOVER_STORYBOARD_FRAMES}`;
}

function rememberMediaHoverStoryboard(key: string, storyboard: LibraryStoryboard) {
  mediaHoverStoryboardCache.delete(key);
  mediaHoverStoryboardCache.set(key, storyboard);
  while (mediaHoverStoryboardCache.size > MEDIA_HOVER_STORYBOARD_CACHE_LIMIT) {
    const oldest = mediaHoverStoryboardCache.keys().next().value;
    if (oldest == null) break;
    mediaHoverStoryboardCache.delete(oldest);
  }
}

async function loadMediaHoverStoryboard(item: LibraryItem, privateMedia: boolean) {
  const key = mediaHoverStoryboardKey(item);
  if (!privateMedia) {
    const cached = mediaHoverStoryboardCache.get(key);
    if (cached) {
      rememberMediaHoverStoryboard(key, cached);
      return cached;
    }
    const pending = mediaHoverStoryboardRequests.get(key);
    if (pending) return pending;
  }
  const request = api.generateLibraryStoryboard(item.id, MEDIA_HOVER_STORYBOARD_FRAMES);
  if (!privateMedia) mediaHoverStoryboardRequests.set(key, request);
  try {
    const storyboard = await request;
    if (!privateMedia) rememberMediaHoverStoryboard(key, storyboard);
    return storyboard;
  } finally {
    if (!privateMedia && mediaHoverStoryboardRequests.get(key) === request) mediaHoverStoryboardRequests.delete(key);
  }
}

function MediaCard({ item, sourceName, onSelect, progress, selectionMode = false, selected = false, privateMedia = false, initialWatchLater = false, viewMode = "grid" }: { item: LibraryItem; sourceName: string; onSelect: () => void; progress?: number; selectionMode?: boolean; selected?: boolean; privateMedia?: boolean; initialWatchLater?: boolean; viewMode?: "grid" | "list" }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const quality = resolutionLabel(item);
  const listView = viewMode === "list";
  const [watchLater, setWatchLater] = useState<boolean | null>(initialWatchLater ? true : null);
  const [watchLaterBusy, setWatchLaterBusy] = useState(false);
  const [hoverPreviewActive, setHoverPreviewActive] = useState(false);
  const [hoverStoryboard, setHoverStoryboard] = useState<LibraryStoryboard | null>(null);
  const [hoverPreviewFrame, setHoverPreviewFrame] = useState(0);
  const [hoverPreviewBusy, setHoverPreviewBusy] = useState(false);
  const [hoverPreviewUnavailable, setHoverPreviewUnavailable] = useState(false);
  const hoverPreviewDelay = useRef<number | null>(null);
  const hoverPreviewPercent = useRef(0.5);
  const hoverPreviewRequest = useRef(0);

  useEffect(() => () => {
    if (hoverPreviewDelay.current != null) window.clearTimeout(hoverPreviewDelay.current);
    hoverPreviewRequest.current += 1;
  }, []);

  const previewEligible = item.mediaKind === "video"
    && !item.missing
    && !selectionMode
    && item.durationSeconds != null
    && item.durationSeconds > 0.1;

  async function requestHoverPreview() {
    if (!previewEligible || hoverStoryboard || hoverPreviewBusy || hoverPreviewUnavailable) return;
    const requestId = ++hoverPreviewRequest.current;
    setHoverPreviewBusy(true);
    try {
      const storyboard = await loadMediaHoverStoryboard(item, privateMedia);
      if (hoverPreviewRequest.current !== requestId) return;
      setHoverStoryboard(storyboard);
      setHoverPreviewFrame(Math.min(storyboard.frames.length - 1, Math.floor(hoverPreviewPercent.current * storyboard.frames.length)));
    } catch {
      if (hoverPreviewRequest.current === requestId) setHoverPreviewUnavailable(true);
    } finally {
      if (hoverPreviewRequest.current === requestId) setHoverPreviewBusy(false);
    }
  }

  function startHoverPreview() {
    if (!previewEligible) return;
    const reducedMotion = document.documentElement.classList.contains("reduce-motion")
      || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedMotion) return;
    setHoverPreviewActive(true);
    if (hoverStoryboard || hoverPreviewBusy || hoverPreviewUnavailable || hoverPreviewDelay.current != null) return;
    hoverPreviewDelay.current = window.setTimeout(() => {
      hoverPreviewDelay.current = null;
      void requestHoverPreview();
    }, MEDIA_HOVER_PREVIEW_DELAY_MS);
  }

  function scrubHoverPreview(event: ReactPointerEvent<HTMLDivElement>) {
    if (!previewEligible) return;
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const percent = Math.min(0.999, Math.max(0, (event.clientX - rect.left) / rect.width));
    hoverPreviewPercent.current = percent;
    if (hoverStoryboard?.frames.length) setHoverPreviewFrame(Math.floor(percent * hoverStoryboard.frames.length));
  }

  function stopHoverPreview() {
    setHoverPreviewActive(false);
    if (hoverPreviewDelay.current != null) {
      window.clearTimeout(hoverPreviewDelay.current);
      hoverPreviewDelay.current = null;
    }
  }

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
        <div className={`relative aspect-video overflow-hidden rounded-[9px] bg-elevated shadow-[0_14px_35px_rgb(3_11_24/0.24)] transition duration-200 group-hover:-translate-y-0.5 group-hover:shadow-[0_20px_45px_rgb(3_11_24/0.36)] ${selected ? "ring-2 ring-accent ring-offset-2 ring-offset-canvas" : ""}`} onPointerEnter={startHoverPreview} onPointerMove={scrubHoverPreview} onPointerLeave={stopHoverPreview}>
          <MediaArtwork item={item} />
          {hoverPreviewActive && hoverStoryboard?.frames[hoverPreviewFrame] ? <span className="absolute inset-0"><img className="h-full w-full object-cover" src={hoverStoryboard.frames[hoverPreviewFrame].imageDataUrl} alt="" aria-hidden="true" /><span className="number-font absolute bottom-2 left-1/2 z-[1] -translate-x-1/2 rounded-[5px] bg-black/78 px-2 py-1 text-[9px] font-semibold text-white/90 shadow-control">Preview · {formatDuration(hoverStoryboard.frames[hoverPreviewFrame].timeSeconds)}</span></span> : hoverPreviewActive && hoverPreviewBusy ? <span className="absolute bottom-2 left-1/2 z-[1] inline-flex -translate-x-1/2 items-center gap-1.5 rounded-[5px] bg-black/78 px-2 py-1 text-[9px] font-medium text-white/75"><LoaderCircle className="animate-spin" size={10} /> Preparing preview</span> : hoverPreviewActive && hoverPreviewUnavailable ? <span className="absolute bottom-2 left-1/2 z-[1] -translate-x-1/2 rounded-[5px] bg-black/78 px-2 py-1 text-[9px] font-medium text-white/65">Preview unavailable</span> : null}
          <span className="absolute inset-0 grid place-items-center bg-black/0 transition group-hover:bg-black/20"><span className={`grid size-11 place-items-center rounded-full shadow-lg transition ${selectionMode ? "scale-100 bg-panel/95 text-ink opacity-100" : "scale-90 bg-white/90 text-black opacity-0 group-hover:scale-100 group-hover:opacity-100"}`}>{selectionMode ? selected ? <CheckSquare2 className="text-accent" size={21} /> : <Square size={21} /> : item.mediaKind === "image" ? <ImageIcon size={19} strokeWidth={1.8} /> : <Play size={18} strokeWidth={1.8} fill="currentColor" />}</span></span>
          <span className="number-font absolute bottom-2 right-2 rounded-[4px] bg-black/82 px-1.5 py-0.5 text-[10px] font-semibold text-white">{item.durationSeconds != null ? formatDuration(item.durationSeconds) : quality}</span>
          {item.durationSeconds != null && quality ? <span className="absolute bottom-2 left-2 rounded-[4px] bg-black/75 px-1.5 py-0.5 text-[9px] font-semibold text-white/90">{quality}</span> : null}
          <span className="absolute left-2 top-2 flex items-center gap-1">{privateMedia ? <span className="inline-flex h-6 items-center gap-1 rounded-[5px] bg-black/78 px-1.5 text-[9px] font-semibold text-white"><Lock size={11} /> Private</span> : null}{item.favorite ? <span className="grid size-6 place-items-center rounded-[5px] bg-black/70 text-rose-300"><Heart size={13} fill="currentColor" /></span> : null}{item.missing ? <span className="bg-red-500/90 px-2 py-1 text-[9px] font-bold uppercase tracking-wider text-white">Missing</span> : null}</span>
          {item.metadataState === "pending" ? <span className="absolute right-2 top-2 grid size-6 place-items-center rounded-[5px] bg-black/70 text-white/80" title="Inspecting media"><LoaderCircle className="animate-spin" size={12} /></span> : null}
          {item.mediaKind === "video" && item.metadataState !== "pending" ? <EmbeddedCompatibilityLabel item={item} variant="overlay" /> : null}
          {progress != null ? <span className="absolute inset-x-0 bottom-0 h-1 bg-black/60"><span className="block h-full bg-accent" style={{ width: `${progress}%` }} /></span> : null}
        </div>
        <div className={`${listView ? "mt-0" : "mt-3"} flex min-w-0 gap-3`}><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-elevated text-[11px] font-bold text-accent">{sourceName.charAt(0).toLocaleUpperCase()}</div><div className="min-w-0"><h3 className={`${listView ? "truncate text-sm" : "line-clamp-2 text-[13px]"} font-semibold leading-[1.35] text-ink`}>{item.title}</h3><p className="mt-1 truncate text-[11px] text-muted">{sourceName}{item.rating ? ` · ${item.rating} ★` : ""}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{listView && item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}{listView ? ` · ${new Date(item.addedAt).toLocaleDateString()}` : ""}</p>{listView && item.tags.length > 0 ? <p className="mt-2 truncate text-[10px] text-muted/80">{item.tags.map((tag) => `#${tag}`).join("  ")}</p> : null}</div></div>
      </button>
      {!selectionMode && item.mediaKind === "video" ? <button className={`mt-2 flex h-8 items-center gap-1.5 rounded-[7px] px-2 text-[10px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70 ${watchLater ? "bg-accent/10 text-accent" : "text-muted hover:bg-elevated hover:text-ink"}`} disabled={watchLaterBusy} onClick={() => void toggleWatchLater()} aria-pressed={watchLater ?? undefined} aria-label={`${watchLater ? "Remove" : "Add"} ${item.title} ${watchLater ? "from" : "to"} Watch Later`}>{watchLaterBusy ? <LoaderCircle className="animate-spin" size={12} /> : <ListPlus size={12} />}{watchLater ? "Saved for later" : "Watch later"}</button> : null}
      {!selectionMode ? <details className="relative mt-1 inline-block"><summary className="flex h-8 cursor-pointer list-none items-center gap-1.5 rounded-[7px] px-2 text-[11px] font-semibold text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70"><MoreHorizontal size={14} /> More</summary><div className="panel-shadow absolute bottom-9 left-0 z-20 grid w-48 gap-1 rounded-control border border-line/70 bg-panel/95 p-1.5 backdrop-blur-xl"><button className="flex items-center gap-2 rounded-[7px] px-2.5 py-2 text-left text-[12px] text-muted hover:bg-elevated hover:text-ink" onClick={() => { setView("similarity"); window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:similarity-reference", { detail: { itemId: item.id } })), 0); }}><ScanSearch size={14} /> Find similar</button><button className="flex items-center gap-2 rounded-[7px] px-2.5 py-2 text-left text-[12px] text-muted hover:bg-elevated hover:text-ink" onClick={() => { setView("sharing"); window.setTimeout(() => window.dispatchEvent(new CustomEvent("nice-downloader:sharing-select", { detail: { itemId: item.id } })), 0); }}><Share2 size={14} /> Share temporarily</button></div></details> : null}
    </article>
  );
}

const MediaArtwork = memo(function MediaArtwork({ item, eager = false }: { item: LibraryItem; eager?: boolean }) {
  const inlineArtwork = item.thumbnail?.startsWith("data:") ? item.thumbnail : null;
  const [artwork, setArtwork] = useState<string | null>(inlineArtwork);
  const [nearViewport, setNearViewport] = useState(eager);
  const artworkRoot = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (eager || inlineArtwork || item.missing) {
      setNearViewport(true);
      return;
    }
    const element = artworkRoot.current;
    if (!element || typeof IntersectionObserver === "undefined") {
      setNearViewport(true);
      return;
    }
    setNearViewport(false);
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry?.isIntersecting) return;
      setNearViewport(true);
      observer.disconnect();
    }, { rootMargin: "240px" });
    observer.observe(element);
    return () => observer.disconnect();
  }, [eager, inlineArtwork, item.id, item.missing]);
  useEffect(() => {
    setArtwork(inlineArtwork);
    if (inlineArtwork || item.missing || !nearViewport) return;
    let disposed = false;
    void loadLibraryArtwork(item.id)
      .then((value) => { if (!disposed) setArtwork(value); })
      .catch((error) => { if (!disposed) console.warn(`Could not load artwork for ${item.id}`, error); });
    return () => { disposed = true; };
  }, [inlineArtwork, item.id, item.missing, nearViewport]);
  if (artwork) return <div ref={artworkRoot} className="h-full w-full"><img className="h-full w-full object-cover transition duration-300 group-hover:scale-[1.025]" src={artwork} alt={`${item.title} artwork`} loading={eager ? "eager" : "lazy"} decoding="async" /></div>;
  const variant = item.id.charCodeAt(0) % 3;
  const background = variant === 0 ? "from-[#1b2b45] via-[#172235] to-[#101419]" : variant === 1 ? "from-[#33291d] via-[#26231d] to-[#111417]" : "from-[#202b3a] via-[#1b252f] to-[#101419]";
  return <div ref={artworkRoot} className={`relative grid h-full w-full place-items-center bg-gradient-to-br ${background}`}><div className="absolute inset-0 opacity-[0.08] [background-image:radial-gradient(circle_at_center,white_1px,transparent_1px)] [background-size:18px_18px]" />{item.mediaKind === "image" ? <FileImage className="relative text-white/35" size={34} /> : <FileVideo className="relative text-white/35" size={34} />}</div>;
});

function EmbeddedCompatibilityLabel({ item, preflight, variant = "panel" }: { item: LibraryItem; preflight?: LibraryPlaybackPreflight | null; variant?: "badge" | "overlay" | "panel" }) {
  const fingerprint = embeddedPlaybackFingerprint(item);
  const evidence = useAppStore((state) => state.embeddedPlaybackEvidence[fingerprint]);
  const assessment = assessEmbeddedCompatibility(item, preflight, evidence);
  const tone = assessment.kind === "expected"
    ? "border-emerald-300/25 bg-emerald-400/12 text-emerald-300"
    : assessment.kind === "copy"
      ? "border-accent/25 bg-accent/10 text-accent"
      : assessment.kind === "native"
        ? "border-amber-300/25 bg-amber-400/12 text-amber-200"
        : "border-white/15 bg-black/65 text-white/70";
  const icon = assessment.kind === "expected"
    ? <Check size={variant === "panel" ? 17 : 11} />
    : assessment.kind === "native"
      ? <MonitorPlay size={variant === "panel" ? 17 : 11} />
      : assessment.kind === "copy"
        ? <Copy size={variant === "panel" ? 17 : 11} />
        : <Film size={variant === "panel" ? 17 : 11} />;
  const videoCodec = formatMediaCodec(item.videoCodec);

  if (variant !== "panel") {
    return <span className={`${variant === "overlay" ? "absolute right-2 top-2 z-[1] max-w-[calc(100%-1rem)]" : "relative"} inline-flex h-6 items-center gap-1.5 rounded-[5px] border px-2 text-[9px] font-semibold backdrop-blur-md ${tone}`} title={`${assessment.heading}. ${assessment.codecSummary}. ${assessment.detail}`}>{icon}<span className="truncate">{videoCodec ? `${videoCodec} · ` : ""}{assessment.badge}</span></span>;
  }

  return <section className={`mt-4 flex items-start gap-3 rounded-[9px] border p-3 ${tone}`} aria-label="Embedded playback compatibility"><span className="mt-0.5 shrink-0">{icon}</span><span className="min-w-0"><strong className="block text-xs font-semibold">{assessment.heading}</strong><span className="mt-1 block text-[10px] font-medium opacity-90">{assessment.codecSummary}</span><span className="mt-1 block text-[10px] leading-4 opacity-75">{assessment.detail}</span></span></section>;
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

function WatchWorkspace({ item, sourceName, browseItems, browseTotal, browsePage, browsePages, browseLoading, search, sourceNames, compact, autoplayNext, playlists, playNextItems, seekRequest, onSearch, onBack, onRestore, onClose, onDetails, onSelectVideo, onPrevious, onNext, onAutoplayNext, onPlaylistChanged, onPlaybackEnded, onBrowsePage, onPlayNextSelect, onPlayNextMove, onPlayNextRemove, onPlayNextClear }: {
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
  playNextItems: LibraryItem[];
  seekRequest: TranscriptSeekRequest | null;
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
  onPlayNextSelect: (item: LibraryItem) => void;
  onPlayNextMove: (itemId: string, newIndex: number) => void;
  onPlayNextRemove: (itemId: string) => void;
  onPlayNextClear: () => void;
}) {
  const [leanBack, setLeanBack] = useState(false);
  const leanBackReturnFocus = useRef<HTMLElement | null>(null);
  const miniPlayerRef = useRef<HTMLElement | null>(null);
  const miniPlayerGesture = useRef<MiniPlayerGesture | null>(null);
  const miniPlayerBoundsFrame = useRef(0);
  const [miniPlayerLayout, setMiniPlayerLayout] = useState<MiniPlayerLayout | null>(readMiniPlayerLayout);

  function enterLeanBack() {
    leanBackReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setLeanBack(true);
  }

  function exitLeanBack() {
    setLeanBack(false);
  }

  useEffect(() => {
    if (!leanBack) return;
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    window.requestAnimationFrame(() => document.querySelector<HTMLElement>("[data-lean-back-exit]")?.focus());
    return () => {
      document.body.style.overflow = previousOverflow;
      window.requestAnimationFrame(() => {
        if (leanBackReturnFocus.current?.isConnected) leanBackReturnFocus.current.focus();
      });
    };
  }, [leanBack]);

  useEffect(() => {
    if (compact) setLeanBack(false);
  }, [compact]);

  useEffect(() => {
    if (!miniPlayerLayout) return;
    window.localStorage.setItem(MINI_PLAYER_LAYOUT_KEY, JSON.stringify(miniPlayerLayout));
  }, [miniPlayerLayout]);

  useEffect(() => {
    if (!compact) return;
    const frame = window.requestAnimationFrame(() => {
      const element = miniPlayerRef.current;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      const next = clampMiniPlayerLayout(
        miniPlayerLayout || { x: rect.left, y: rect.top, width: rect.width },
        rect.height,
      );
      setMiniPlayerLayout((current) => sameMiniPlayerLayout(current, next) ? current : next);
    });
    const keepInsideViewport = () => {
      const element = miniPlayerRef.current;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      setMiniPlayerLayout((current) => {
        const next = clampMiniPlayerLayout(current || { x: rect.left, y: rect.top, width: rect.width }, rect.height);
        return sameMiniPlayerLayout(current, next) ? current : next;
      });
    };
    const observer = new ResizeObserver(keepInsideViewport);
    if (miniPlayerRef.current) observer.observe(miniPlayerRef.current);
    window.addEventListener("resize", keepInsideViewport);
    return () => {
      window.cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener("resize", keepInsideViewport);
    };
  }, [compact]);

  useEffect(() => {
    const scheduleBoundsSync = () => {
      if (miniPlayerBoundsFrame.current) return;
      miniPlayerBoundsFrame.current = window.requestAnimationFrame(() => {
        miniPlayerBoundsFrame.current = 0;
        window.dispatchEvent(new Event("nice-downloader:player-bounds-changed"));
      });
    };
    const finishGesture = (pointerId?: number) => {
      const gesture = miniPlayerGesture.current;
      if (!gesture || (pointerId != null && pointerId !== gesture.pointerId)) return;
      document.body.style.cursor = gesture.previousCursor;
      document.body.style.userSelect = gesture.previousUserSelect;
      miniPlayerGesture.current = null;
    };
    const updateGesture = (event: PointerEvent) => {
      const gesture = miniPlayerGesture.current;
      if (!gesture || event.pointerId !== gesture.pointerId) return;
      event.preventDefault();
      if (gesture.kind === "move") {
        const next = clampMiniPlayerLayout({
          x: gesture.startFrame.x + event.clientX - gesture.startClientX,
          y: gesture.startFrame.y + event.clientY - gesture.startClientY,
          width: gesture.startFrame.width,
        }, gesture.startHeight);
        setMiniPlayerLayout(next);
        scheduleBoundsSync();
        return;
      }
      const horizontalDelta = event.clientX - gesture.startClientX;
      const chromeHeight = Math.max(0, gesture.startHeight - gesture.startFrame.width * 9 / 16);
      const maximumHeightWidth = Math.max(MINI_PLAYER_MIN_WIDTH, (window.innerHeight - MINI_PLAYER_MARGIN * 2 - chromeHeight) * 16 / 9);
      const width = Math.min(
        MINI_PLAYER_MAX_WIDTH,
        window.innerWidth - MINI_PLAYER_MARGIN * 2,
        maximumHeightWidth,
        Math.max(MINI_PLAYER_MIN_WIDTH, gesture.startFrame.width + horizontalDelta * 2),
      );
      const estimatedHeight = chromeHeight + width * 9 / 16;
      setMiniPlayerLayout(clampMiniPlayerLayout({
        x: gesture.startFrame.x + (gesture.startFrame.width - width) / 2,
        y: gesture.startFrame.y + (gesture.startHeight - estimatedHeight) / 2,
        width,
      }, estimatedHeight));
      scheduleBoundsSync();
    };
    const endGesture = (event: PointerEvent) => finishGesture(event.pointerId);
    window.addEventListener("pointermove", updateGesture, { passive: false });
    window.addEventListener("pointerup", endGesture);
    window.addEventListener("pointercancel", endGesture);
    return () => {
      finishGesture();
      window.cancelAnimationFrame(miniPlayerBoundsFrame.current);
      miniPlayerBoundsFrame.current = 0;
      window.removeEventListener("pointermove", updateGesture);
      window.removeEventListener("pointerup", endGesture);
      window.removeEventListener("pointercancel", endGesture);
    };
  }, []);

  function startMiniPlayerGesture(kind: MiniPlayerGesture["kind"], event: ReactPointerEvent<HTMLElement>) {
    if (!compact || event.button !== 0) return;
    const element = miniPlayerRef.current;
    if (!element) return;
    event.preventDefault();
    event.stopPropagation();
    const rect = element.getBoundingClientRect();
    const startFrame = clampMiniPlayerLayout({ x: rect.left, y: rect.top, width: rect.width }, rect.height);
    setMiniPlayerLayout(startFrame);
    miniPlayerGesture.current = {
      kind,
      pointerId: event.pointerId,
      startClientX: event.clientX,
      startClientY: event.clientY,
      startFrame,
      startHeight: rect.height,
      previousCursor: document.body.style.cursor,
      previousUserSelect: document.body.style.userSelect,
    };
    document.body.style.cursor = kind === "move" ? "grabbing" : "ew-resize";
    document.body.style.userSelect = "none";
  }

  function moveMiniPlayerBy(deltaX: number, deltaY: number) {
    const element = miniPlayerRef.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    setMiniPlayerLayout(clampMiniPlayerLayout({ x: rect.left + deltaX, y: rect.top + deltaY, width: rect.width }, rect.height));
  }

  function resizeMiniPlayerBy(deltaWidth: number) {
    const element = miniPlayerRef.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    const width = Math.min(MINI_PLAYER_MAX_WIDTH, Math.max(MINI_PLAYER_MIN_WIDTH, rect.width + deltaWidth));
    const estimatedHeight = rect.height + (width - rect.width) * 9 / 16;
    setMiniPlayerLayout(clampMiniPlayerLayout({
      x: rect.left + (rect.width - width) / 2,
      y: rect.top + (rect.height - estimatedHeight) / 2,
      width,
    }, estimatedHeight));
  }

  function handleMiniPlayerMoveKey(event: ReactKeyboardEvent<HTMLElement>) {
    const amount = event.shiftKey ? 48 : 16;
    if (event.key === "ArrowLeft") moveMiniPlayerBy(-amount, 0);
    else if (event.key === "ArrowRight") moveMiniPlayerBy(amount, 0);
    else if (event.key === "ArrowUp") moveMiniPlayerBy(0, -amount);
    else if (event.key === "ArrowDown") moveMiniPlayerBy(0, amount);
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  function handleMiniPlayerResizeKey(event: ReactKeyboardEvent<HTMLElement>) {
    const amount = event.shiftKey ? 80 : 32;
    if (event.key === "ArrowRight" || event.key === "ArrowUp") resizeMiniPlayerBy(amount);
    else if (event.key === "ArrowLeft" || event.key === "ArrowDown") resizeMiniPlayerBy(-amount);
    else return;
    event.preventDefault();
    event.stopPropagation();
  }

  function resetMiniPlayerLayout() {
    const element = miniPlayerRef.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    const width = Math.min(MINI_PLAYER_DEFAULT_WIDTH, window.innerWidth - MINI_PLAYER_MARGIN * 2);
    const estimatedHeight = rect.height + (width - rect.width) * 9 / 16;
    setMiniPlayerLayout(clampMiniPlayerLayout({
      x: window.innerWidth - width - 20,
      y: window.innerHeight - estimatedHeight - 20,
      width,
    }, estimatedHeight));
  }

  const miniPlayerStyle = compact
    ? miniPlayerLayout
      ? { left: miniPlayerLayout.x, top: miniPlayerLayout.y, width: miniPlayerLayout.width }
      : { bottom: 20, right: 20, width: `min(${MINI_PLAYER_DEFAULT_WIDTH}px, calc(100vw - 32px))` }
    : undefined;

  return <section ref={miniPlayerRef} style={miniPlayerStyle} className={leanBack ? "fixed inset-0 z-[80] flex min-h-0 flex-col bg-[#080705] px-5 pb-5 text-white sm:px-8 sm:pb-8" : compact ? "fixed z-[65]" : "mx-auto w-full max-w-[1640px] px-6 pb-12 pt-5 xl:px-8"} aria-label={leanBack ? "Lean-back player" : compact ? "Mini player" : "Watch workspace"}>
    <LibraryLiveRegions polite="" assertive="" />
      <header className={`${compact ? "hidden" : leanBack ? "flex" : "sticky"} top-0 z-10 -mx-2 mb-5 items-center gap-4 px-2 py-3 ${leanBack ? "min-h-20 bg-[#080705]" : "bg-canvas/92 backdrop-blur-xl"}`}>
      {leanBack ? <><div className="grid size-11 shrink-0 place-items-center rounded-full bg-accent/15 text-accent"><Tv size={21} /></div><div className="min-w-0 flex-1"><p className="text-[11px] font-semibold uppercase tracking-[0.15em] text-accent">Lean-back mode</p><h1 className="mt-1 truncate text-lg font-semibold tracking-[-0.025em] sm:text-2xl">{item.title}</h1></div><p className="hidden text-xs text-white/45 lg:block">Keyboard or gamepad supported</p><Button data-lean-back-exit variant="secondary" className="h-12 shrink-0 border-white/10 bg-white/10 px-5 text-white hover:bg-white/15" onClick={exitLeanBack}><Minimize2 size={18} /> Exit</Button></> : <><button autoFocus className="flex h-10 shrink-0 items-center gap-2 rounded-[8px] px-2.5 text-xs font-semibold text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onBack}><ChevronRight className="rotate-180" size={16} /> Library</button><div className="h-6 w-px bg-line/70" /><div className="min-w-0"><p className="text-[10px] font-medium text-accent">Now watching</p><h1 className="max-w-xl truncate text-sm font-semibold tracking-[-0.02em]">{item.title}</h1></div><Button variant="secondary" className="ml-auto h-10 shrink-0" onClick={enterLeanBack}><Tv size={16} /> Lean back</Button><label className="relative w-full max-w-md"><span className="sr-only">Search videos while watching</span><Search className="absolute left-3.5 top-3 text-muted" size={15} /><Input className="h-10 rounded-[9px] bg-panel/85 pl-10 text-xs shadow-control" value={search} onChange={(event) => onSearch(event.target.value)} placeholder="Find another video" /></label></>}
    </header>

    <div className={leanBack ? "flex min-h-0 flex-1" : compact ? "block" : "grid min-w-0 gap-6 xl:grid-cols-[minmax(0,1fr)_370px] 2xl:grid-cols-[minmax(0,1fr)_410px]"}>
      <article className={leanBack ? "flex min-h-0 min-w-0 flex-1 flex-col" : "min-w-0"}>
        <EmbeddedVideoPlayer key={item.id} item={item} sourceName={sourceName} compact={compact} leanBack={leanBack} seekRequest={seekRequest} onRestore={onRestore} onClose={onClose} onExitLeanBack={exitLeanBack} onPrevious={onPrevious} onNext={onNext} onPlaybackEnded={onPlaybackEnded} onMiniPlayerMoveStart={(event) => startMiniPlayerGesture("move", event)} onMiniPlayerMoveKey={handleMiniPlayerMoveKey} onMiniPlayerResizeStart={(event) => startMiniPlayerGesture("resize", event)} onMiniPlayerResizeKey={handleMiniPlayerResizeKey} onMiniPlayerReset={resetMiniPlayerLayout} />
        <section className={`${compact || leanBack ? "hidden" : "block"} mt-5 border-b border-line/60 pb-6`} aria-labelledby="watch-title">
          <div className="flex flex-wrap items-start justify-between gap-4">
            <div className="min-w-0 max-w-4xl"><h2 id="watch-title" className="text-[24px] font-semibold leading-tight tracking-[-0.035em] text-balance">{item.title}</h2><p className="mt-2 text-xs text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}</p></div>
            <div className="flex flex-wrap items-center gap-2"><WatchStateActions itemId={item.id} playlists={playlists} onPlaylistChanged={onPlaylistChanged} /><Button id="watch-details-button" variant="secondary" onClick={onDetails}><Settings2 size={15} /> Details and organize</Button></div>
          </div>
          {item.tags.length > 0 ? <div className="mt-4 flex flex-wrap gap-1.5">{item.tags.map((tag) => <span key={tag} className="rounded-[5px] bg-elevated px-2 py-1 text-[10px] font-medium text-muted">#{tag}</span>)}</div> : null}
          <p className="mt-4 max-w-[76ch] break-all text-[11px] leading-5 text-muted/80" title={item.path}>{item.relativePath || item.path}</p>
        </section>
      </article>

      <aside className={`${compact || leanBack ? "hidden" : "flex"} panel-shadow max-h-[calc(100vh-104px)] min-h-[540px] flex-col overflow-hidden rounded-[12px] border border-line/70 bg-panel/65 xl:sticky xl:top-[76px] xl:self-start`} aria-label="Browse videos while watching">
        <div className="flex items-start justify-between gap-3 border-b border-line/60 px-4 py-4"><div><h2 className="text-sm font-semibold">{search.trim() ? "Search results" : "Up next"}</h2><p className="mt-1 text-[10px] text-muted">{browseLoading ? "Updating videos…" : `${browseTotal} available video${browseTotal === 1 ? "" : "s"}`}</p><p className="sr-only" role="status" aria-live="polite" aria-atomic="true">{browseLoading ? "Updating watch browser results." : `Watch browser showing page ${browsePage} of ${browsePages}, with ${browseTotal} available videos.`}</p></div><div className="flex items-center gap-3">{!search.trim() ? <label className="flex items-center gap-2 text-[10px] font-medium text-muted"><span>Autoplay</span><input type="checkbox" checked={autoplayNext} onChange={(event) => onAutoplayNext(event.target.checked)} /></label> : null}{browsePages > 1 ? <span className="number-font text-[10px] text-muted">{browsePage}/{browsePages}</span> : null}</div></div>
        {playNextItems.length ? <section className="border-b border-line/60 bg-accent/[0.035]" aria-labelledby="play-next-title"><header className="flex items-center gap-2 px-4 py-2.5"><ListVideo className="text-accent" size={14} /><div className="min-w-0 flex-1"><h3 id="play-next-title" className="text-[11px] font-semibold">Play Next queue</h3><p className="text-[9px] text-muted">{playNextItems.length} temporary item{playNextItems.length === 1 ? "" : "s"}</p></div><button className="rounded-[6px] px-2 py-1 text-[9px] font-medium text-muted hover:bg-elevated hover:text-ink" onClick={onPlayNextClear}>Clear</button></header><div className="max-h-48 overflow-y-auto px-2 pb-2">{playNextItems.map((queued, index) => <div key={queued.id} className="flex items-center gap-1 rounded-[7px] px-1.5 py-1 hover:bg-elevated/60"><button className="min-w-0 flex-1 truncate px-1 py-1.5 text-left text-[10px] font-medium focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60" onClick={() => onPlayNextSelect(queued)} title={queued.title}><span className="mr-2 text-accent">{index + 1}</span>{queued.title}</button><button className="grid size-7 shrink-0 place-items-center rounded-[6px] text-muted hover:bg-panel hover:text-ink disabled:opacity-30" disabled={index === 0} onClick={() => onPlayNextMove(queued.id, index - 1)} aria-label={`Move ${queued.title} earlier`}><ArrowUp size={12} /></button><button className="grid size-7 shrink-0 place-items-center rounded-[6px] text-muted hover:bg-panel hover:text-ink disabled:opacity-30" disabled={index === playNextItems.length - 1} onClick={() => onPlayNextMove(queued.id, index + 1)} aria-label={`Move ${queued.title} later`}><ArrowDown size={12} /></button><button className="grid size-7 shrink-0 place-items-center rounded-[6px] text-muted hover:bg-red-400/10 hover:text-red-300" onClick={() => onPlayNextRemove(queued.id)} aria-label={`Remove ${queued.title} from Play Next`}><X size={12} /></button></div>)}</div></section> : null}
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
  const playNextItemIds = useAppStore((state) => state.playNextItemIds);
  const enqueuePlayNext = useAppStore((state) => state.enqueuePlayNext);
  const setNotice = useAppStore((state) => state.setNotice);
  const queued = playNextItemIds.includes(item.id);
  return <div className={`group flex w-full items-center rounded-[9px] pr-1 transition ${active ? "bg-accent/[0.09] shadow-[inset_2px_0_rgb(var(--accent))]" : "hover:bg-elevated/75"}`}>
    <button className="grid min-w-0 flex-1 grid-cols-[128px_minmax(0,1fr)] gap-3 p-2 text-left active:scale-[0.99] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onSelect} aria-current={active ? "true" : undefined}>
      <span className="relative aspect-video overflow-hidden rounded-[7px] bg-elevated"><MediaArtwork item={item} />{item.durationSeconds != null ? <span className="number-font absolute bottom-1 right-1 rounded-[4px] bg-black/80 px-1.5 py-0.5 text-[9px] font-semibold text-white">{formatDuration(item.durationSeconds)}</span> : null}{active ? <span className="absolute inset-0 grid place-items-center bg-black/40"><span className="grid size-8 place-items-center rounded-full bg-accent text-[#081426]"><Play size={13} fill="currentColor" /></span></span> : null}</span>
      <span className="min-w-0 py-0.5"><strong className="line-clamp-2 text-[11px] font-semibold leading-[1.35] text-ink">{item.title}</strong><span className="mt-1.5 block truncate text-[9px] text-muted">{sourceName}</span><span className={`mt-1 block text-[9px] font-medium ${active ? "text-accent" : "text-muted/75"}`}>{active ? "Playing now" : [resolutionLabel(item), item.container?.toLocaleUpperCase()].filter(Boolean).join(" · ") || "Video"}</span></span>
    </button>
    <button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/60 disabled:opacity-35" disabled={active || queued} onClick={() => { enqueuePlayNext(item.id); setNotice({ message: `Queued “${item.title}” to play next.`, view: "library" }); }} aria-label={queued ? `${item.title} is already in Play Next` : `Queue ${item.title} to play next`} title={queued ? "Already queued" : "Play next"}><ListVideo size={14} /></button>
  </div>;
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

function PlaybackPreviewTimeline({ currentTime, duration, pendingTime, compact, isPlaying, muted, volume, subtitlesAvailable, subtitlesOpen, storyboard, storyboardBusy, storyboardUnavailable, onPreviewRequested, onPendingTime, onCommit, onTogglePlayback, onSeekRelative, onToggleMuted, onVolumeChange, onToggleSubtitles, onToggleFullscreen }: {
  currentTime: number;
  duration: number;
  pendingTime: number | null;
  compact: boolean;
  isPlaying: boolean;
  muted: boolean;
  volume: number;
  subtitlesAvailable: boolean;
  subtitlesOpen: boolean;
  storyboard: LibraryStoryboard | null;
  storyboardBusy: boolean;
  storyboardUnavailable: boolean;
  onPreviewRequested: () => void;
  onPendingTime: (seconds: number | null) => void;
  onCommit: (seconds: number) => void;
  onTogglePlayback: () => void;
  onSeekRelative: (seconds: number) => void;
  onToggleMuted: () => void;
  onVolumeChange: (volume: number) => void;
  onToggleSubtitles?: () => void;
  onToggleFullscreen: () => void;
}) {
  const [hover, setHover] = useState<{ seconds: number; percent: number } | null>(null);
  const safeDuration = Math.max(0.1, Number.isFinite(duration) ? duration : 0.1);
  const value = Math.min(Math.max(0, pendingTime ?? currentTime), safeDuration);
  const previewFrame = hover && storyboard?.frames.length
    ? storyboard.frames.reduce((nearest, frame) => Math.abs(frame.timeSeconds - hover.seconds) < Math.abs(nearest.timeSeconds - hover.seconds) ? frame : nearest)
    : null;

  function updateHover(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    if (rect.width <= 0) return;
    const percent = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    setHover({ percent, seconds: percent * safeDuration });
  }

  const controlClass = "grid size-9 shrink-0 place-items-center rounded-[8px] text-white/65 transition-[background-color,color,transform] duration-150 hover:bg-white/10 hover:text-white active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75";

  return <div className={`relative border-t border-white/10 bg-[linear-gradient(180deg,#14110d,#0c0b09)] ${compact ? "px-2 py-2" : "px-4 py-2.5"}`} aria-label="Playback controls with frame previews">
    <div className="flex items-center gap-1.5">
      <button className={controlClass} onClick={() => onSeekRelative(-15)} aria-label="Go back 15 seconds" title="Back 15 seconds"><span className="relative grid place-items-center"><RotateCcw size={18} strokeWidth={1.8} /><span className="number-font absolute mt-px text-[7px] font-bold leading-none">15</span></span></button>
      <button className="grid size-9 shrink-0 place-items-center rounded-[9px] bg-accent text-accentInk shadow-[0_5px_18px_rgb(var(--accent)/0.24),inset_0_1px_0_rgba(255,255,255,0.34)] transition-[filter,transform] duration-150 hover:brightness-110 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/80 focus-visible:ring-offset-2 focus-visible:ring-offset-[#0d0c09]" onClick={onTogglePlayback} aria-label={isPlaying ? "Pause" : "Play"} title={isPlaying ? "Pause" : "Play"}>{isPlaying ? <Pause size={15} fill="currentColor" /> : <Play className="translate-x-px" size={15} fill="currentColor" />}</button>
      <button className={controlClass} onClick={() => onSeekRelative(15)} aria-label="Go forward 15 seconds" title="Forward 15 seconds"><span className="relative grid place-items-center"><RotateCw size={18} strokeWidth={1.8} /><span className="number-font absolute mt-px text-[7px] font-bold leading-none">15</span></span></button>
      <span className={`${compact ? "hidden" : "block"} number-font ml-1 min-w-9 text-right text-[10px] text-white/55`}>{formatDuration(value)}</span>
      <div className="relative mx-1 min-w-0 flex-1" onPointerEnter={onPreviewRequested} onPointerMove={updateHover} onPointerLeave={() => setHover(null)}>
        {hover ? <div className="pointer-events-none absolute bottom-[calc(100%+10px)] z-20 -translate-x-1/2 overflow-hidden rounded-[8px] border border-white/15 bg-[#090806] shadow-[0_16px_45px_rgb(0_0_0/0.65)]" style={{ left: `${Math.min(90, Math.max(10, hover.percent * 100))}%` }} aria-hidden="true">
          <div className={`${compact ? "w-28" : "w-40"} aspect-video bg-black`}>
            {previewFrame ? <img className="h-full w-full object-cover" src={previewFrame.imageDataUrl} alt="" /> : <div className="grid h-full place-items-center text-white/35">{storyboardBusy ? <LoaderCircle className="animate-spin" size={15} /> : <Film size={17} />}</div>}
          </div>
          <div className="number-font flex items-center justify-between gap-3 px-2 py-1 text-[9px] text-white/65"><span>{formatDuration(hover.seconds)}</span><span className="text-white/35">{storyboardUnavailable ? "Preview unavailable" : storyboardBusy ? "Preparing…" : previewFrame ? "Frame preview" : "Hover preview"}</span></div>
        </div> : null}
        <input className="h-7 w-full cursor-pointer accent-[var(--color-accent)]" type="range" min={0} max={safeDuration} step={0.1} value={value} aria-label="Seek through video with frame previews" aria-valuetext={`${formatDuration(value)} of ${formatDuration(duration)}`} onFocus={onPreviewRequested} onChange={(event) => onPendingTime(Number(event.currentTarget.value))} onPointerUp={(event) => onCommit(Number(event.currentTarget.value))} onPointerCancel={() => onPendingTime(null)} onKeyUp={(event) => onCommit(Number(event.currentTarget.value))} onBlur={(event) => { if (pendingTime != null) onCommit(Number(event.currentTarget.value)); }} />
      </div>
      <span className={`${compact ? "hidden" : "block"} number-font min-w-9 text-[10px] text-white/45`}>{formatDuration(duration)}</span>
      <button className={controlClass} onClick={onToggleMuted} aria-label={muted || volume === 0 ? "Unmute" : "Mute"} title={muted || volume === 0 ? "Unmute" : "Mute"}>{muted || volume === 0 ? <VolumeX size={16} /> : <Volume2 size={16} />}</button>
      {!compact ? <input className="h-7 w-20 cursor-pointer accent-[var(--color-accent)]" type="range" min={0} max={1} step={0.05} value={muted ? 0 : volume} onChange={(event) => onVolumeChange(Number(event.currentTarget.value))} aria-label="Playback volume" aria-valuetext={`${Math.round((muted ? 0 : volume) * 100)} percent`} /> : null}
      {subtitlesAvailable && onToggleSubtitles ? <button className={`${controlClass} ${subtitlesOpen ? "bg-white/10 text-accent" : ""}`} onClick={onToggleSubtitles} aria-expanded={subtitlesOpen} aria-controls="subtitle-center" aria-label="Subtitle Center" title="Subtitle Center"><Captions size={16} /></button> : null}
      {!compact ? <button className={controlClass} onClick={onToggleFullscreen} aria-label="Toggle fullscreen" title="Fullscreen"><Maximize size={16} /></button> : null}
    </div>
    {compact ? <div className="number-font mt-0.5 flex items-center justify-center gap-1 text-[9px] text-white/45"><span>{formatDuration(value)}</span><span aria-hidden="true">/</span><span>{formatDuration(duration)}</span></div> : null}
  </div>;
}

function PlaybackTools({ currentTime, loopStart, loopEnd, frameStepDisabled, screenshotBusy, onSetLoopStart, onSetLoopEnd, onClearLoop, onFrameStep, onScreenshot }: { currentTime: number; loopStart: number | null; loopEnd: number | null; frameStepDisabled: boolean; screenshotBusy: boolean; onSetLoopStart: () => void; onSetLoopEnd: () => void; onClearLoop: () => void; onFrameStep: (direction: -1 | 1) => void; onScreenshot: () => void }) {
  const loopActive = loopStart != null && loopEnd != null && loopEnd > loopStart;
  return <section className="flex flex-wrap items-center gap-2 border-t border-white/10 bg-[#100e0b] px-5 py-2.5" aria-label="Precision playback tools"><span className="mr-1 inline-flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-[0.1em] text-white/45"><Repeat2 size={13} /> Playback tools</span><button className={`h-8 rounded-[7px] border px-2.5 text-[10px] font-semibold transition ${loopStart != null ? "border-accent/30 bg-accent/10 text-accent" : "border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white"}`} onClick={onSetLoopStart} title="Set loop start at current position">A · {loopStart == null ? "Set start" : formatDuration(loopStart)}</button><button className={`h-8 rounded-[7px] border px-2.5 text-[10px] font-semibold transition ${loopEnd != null ? "border-accent/30 bg-accent/10 text-accent" : "border-white/10 bg-white/5 text-white/60 hover:bg-white/10 hover:text-white"}`} disabled={loopStart == null} onClick={onSetLoopEnd} title="Set loop end at current position">B · {loopEnd == null ? "Set end" : formatDuration(loopEnd)}</button>{loopStart != null || loopEnd != null ? <button className="h-8 rounded-[7px] px-2 text-[10px] text-white/45 transition hover:bg-white/10 hover:text-white" onClick={onClearLoop}>Clear loop</button> : null}<span className="mx-1 hidden h-5 w-px bg-white/10 sm:block" /><button className="inline-flex h-8 items-center gap-1.5 rounded-[7px] border border-white/10 bg-white/5 px-2.5 text-[10px] font-semibold text-white/60 transition hover:bg-white/10 hover:text-white disabled:pointer-events-none disabled:opacity-35" disabled={frameStepDisabled} onClick={() => onFrameStep(-1)} title={frameStepDisabled ? "Frame stepping is unavailable for live compatibility streams" : "Previous frame (comma)"}><SkipBack size={12} /> Frame</button><button className="inline-flex h-8 items-center gap-1.5 rounded-[7px] border border-white/10 bg-white/5 px-2.5 text-[10px] font-semibold text-white/60 transition hover:bg-white/10 hover:text-white disabled:pointer-events-none disabled:opacity-35" disabled={frameStepDisabled} onClick={() => onFrameStep(1)} title={frameStepDisabled ? "Frame stepping is unavailable for live compatibility streams" : "Next frame (period)"}>Frame <SkipForward size={12} /></button><button className="ml-auto inline-flex h-8 items-center gap-1.5 rounded-[7px] border border-white/10 bg-white/5 px-2.5 text-[10px] font-semibold text-white/60 transition hover:bg-white/10 hover:text-white disabled:pointer-events-none disabled:opacity-45" disabled={screenshotBusy} onClick={onScreenshot}>{screenshotBusy ? <LoaderCircle className="animate-spin" size={12} /> : <Camera size={12} />} Capture frame</button><span className="number-font w-full text-[9px] text-white/35 sm:w-auto">{loopActive ? `Repeating ${formatDuration(loopStart)}–${formatDuration(loopEnd)}` : `At ${formatDuration(currentTime)}`}</span></section>;
}

function VideoDisplayPanel({ settings, remembered, rememberBusy, renderer, onChange, onReset, onRemember, onClose }: {
  settings: LibraryVideoDisplaySettings;
  remembered: boolean;
  rememberBusy: boolean;
  renderer: "WebView" | "libmpv";
  onChange: (patch: Partial<LibraryVideoDisplaySettings>) => void;
  onReset: () => void;
  onRemember: (remember: boolean) => void;
  onClose: () => void;
}) {
  const fitOptions: { value: LibraryVideoFit; label: string; detail: string }[] = [
    { value: "fit", label: "Fit", detail: "Show the entire frame" },
    { value: "fill", label: "Fill", detail: "Fill the player and crop edges" },
    { value: "original", label: "Original", detail: "Use the video's native size" },
    { value: "stretch", label: "Stretch", detail: "Fill without preserving aspect ratio" },
  ];
  const adjustmentClass = "h-6 w-full cursor-pointer accent-[var(--color-accent)]";
  const actionClass = "inline-flex h-9 items-center justify-center gap-1.5 rounded-[8px] border border-white/10 bg-white/5 px-3 text-[11px] font-semibold text-white/65 transition hover:border-white/20 hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75";
  const nextRotation = (delta: -90 | 90) => ((settings.rotation + delta + 360) % 360) as LibraryVideoDisplaySettings["rotation"];
  return <section id="video-display-panel" className="border-t border-white/10 bg-[linear-gradient(180deg,#15120e,#0d0c09)] px-5 py-4" aria-labelledby="video-display-title">
    <div className="flex items-start gap-3">
      <div className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-accent/10 text-accent"><Settings2 size={16} /></div>
      <div className="min-w-0 flex-1"><h2 id="video-display-title" className="text-sm font-semibold text-white/90">Video fit and appearance</h2><p className="mt-1 text-xs leading-5 text-white/45">Applied non-destructively by {renderer}. Drag the picture to pan while zoomed, or use the sliders below.</p></div>
      <button className="grid size-8 place-items-center rounded-[8px] text-white/55 transition hover:bg-white/10 hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75" onClick={onClose} aria-label="Close video appearance controls"><X size={15} /></button>
    </div>
    <div className="mt-4 grid gap-5 xl:grid-cols-[minmax(0,1.05fr)_minmax(0,1fr)]">
      <div className="space-y-4">
        <fieldset><legend className="text-xs font-semibold text-white/65">Frame fit</legend><div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4">{fitOptions.map((option) => <button key={option.value} className={`min-h-14 rounded-[9px] border px-3 py-2 text-left transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75 ${settings.fit === option.value ? "border-accent/40 bg-accent/12 text-accent" : "border-white/10 bg-white/[0.035] text-white/65 hover:border-white/20 hover:bg-white/[0.07]"}`} onClick={() => onChange({ fit: option.value })} aria-pressed={settings.fit === option.value}><span className="block text-xs font-semibold">{option.label}</span><span className="mt-0.5 block text-[11px] leading-4 text-white/40">{option.detail}</span></button>)}</div></fieldset>
        <div><p className="text-xs font-semibold text-white/65">Orientation</p><div className="mt-2 flex flex-wrap gap-2"><button className={actionClass} onClick={() => onChange({ rotation: nextRotation(-90) })}><RotateCcw size={14} /> Rotate left</button><button className={actionClass} onClick={() => onChange({ rotation: nextRotation(90) })}><RotateCw size={14} /> Rotate right</button><button className={`${actionClass} ${settings.flipHorizontal ? "border-accent/35 bg-accent/10 text-accent" : ""}`} onClick={() => onChange({ flipHorizontal: !settings.flipHorizontal })} aria-pressed={settings.flipHorizontal}><Repeat2 size={14} /> Flip horizontal</button><button className={`${actionClass} ${settings.flipVertical ? "border-accent/35 bg-accent/10 text-accent" : ""}`} onClick={() => onChange({ flipVertical: !settings.flipVertical })} aria-pressed={settings.flipVertical}><Repeat2 className="rotate-90" size={14} /> Flip vertical</button></div><p className="number-font mt-2 text-[11px] text-white/40">Rotation {settings.rotation}°</p></div>
        <div className="grid gap-3 sm:grid-cols-3">
          <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Zoom</span><span className="number-font">{settings.zoom.toFixed(2)}×</span></span><input className={adjustmentClass} type="range" min={1} max={3} step={0.05} value={settings.zoom} onChange={(event) => onChange({ zoom: Number(event.currentTarget.value) })} /></label>
          <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Pan X</span><span className="number-font">{Math.round(settings.panX * 100)}</span></span><input className={adjustmentClass} type="range" min={-1} max={1} step={0.01} value={settings.panX} disabled={settings.zoom <= 1} onChange={(event) => onChange({ panX: Number(event.currentTarget.value) })} /></label>
          <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Pan Y</span><span className="number-font">{Math.round(settings.panY * 100)}</span></span><input className={adjustmentClass} type="range" min={-1} max={1} step={0.01} value={settings.panY} disabled={settings.zoom <= 1} onChange={(event) => onChange({ panY: Number(event.currentTarget.value) })} /></label>
        </div>
      </div>
      <div className="space-y-3 rounded-[10px] border border-white/[0.07] bg-black/15 p-4">
        <p className="text-xs font-semibold text-white/65">Picture tuning</p>
        <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Brightness</span><span className="number-font">{settings.brightness > 0 ? "+" : ""}{Math.round(settings.brightness)}</span></span><input className={adjustmentClass} type="range" min={-100} max={100} step={1} value={settings.brightness} onChange={(event) => onChange({ brightness: Number(event.currentTarget.value) })} /></label>
        <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Contrast</span><span className="number-font">{settings.contrast > 0 ? "+" : ""}{Math.round(settings.contrast)}</span></span><input className={adjustmentClass} type="range" min={-100} max={100} step={1} value={settings.contrast} onChange={(event) => onChange({ contrast: Number(event.currentTarget.value) })} /></label>
        <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Saturation</span><span className="number-font">{settings.saturation > 0 ? "+" : ""}{Math.round(settings.saturation)}</span></span><input className={adjustmentClass} type="range" min={-100} max={100} step={1} value={settings.saturation} onChange={(event) => onChange({ saturation: Number(event.currentTarget.value) })} /></label>
        <label><span className="flex justify-between text-[11px] font-medium text-white/55"><span>Gamma</span><span className="number-font">{settings.gamma > 0 ? "+" : ""}{Math.round(settings.gamma)}</span></span><input className={adjustmentClass} type="range" min={-100} max={100} step={1} value={settings.gamma} onChange={(event) => onChange({ gamma: Number(event.currentTarget.value) })} /></label>
      </div>
    </div>
    <div className="mt-4 flex flex-wrap items-center gap-3 border-t border-white/[0.07] pt-4"><label className="flex min-h-9 cursor-pointer items-center gap-2.5 rounded-[8px] px-2 text-xs text-white/65 hover:bg-white/5"><input className="accent-[var(--color-accent)]" type="checkbox" checked={remembered} disabled={rememberBusy} onChange={(event) => onRemember(event.currentTarget.checked)} /><span>{rememberBusy ? "Saving preference…" : "Remember for this video"}</span></label><p className="mr-auto text-[11px] text-white/35">Otherwise these adjustments last only for this session.</p><Button variant="ghost" className="h-9 border border-white/10 text-xs text-white/60" onClick={onReset}><Undo2 size={14} /> Reset appearance</Button></div>
  </section>;
}

function PlaybackDiagnostics({ item, session, preflight, nativeSession, embeddedMpvSession, subtitleCount, error }: { item: LibraryItem; session: LibraryPlaybackSession | null; preflight: LibraryPlaybackPreflight | null; nativeSession: NativePlaybackSession | null; embeddedMpvSession: EmbeddedMpvSession | null; subtitleCount: number; error: AppError | null }) {
  const renderer = embeddedMpvSession
    ? "Embedded libmpv"
    : nativeSession
      ? "Native mpv window"
      : session?.segmentedStream
        ? "HLS compatibility stream"
        : session?.liveTranscode
          ? "Fragmented MP4 compatibility stream"
          : session?.compatibilityTranscoded
            ? "H.264/AAC compatibility copy"
            : session?.compatibilityMode
              ? "Stream-copy compatibility file"
              : session
                ? "Direct WebView video"
                : error
                  ? "Playback unavailable"
                  : "Preparing renderer";
  const delivery = session?.segmentedStream
    ? "Temporary segmented stream"
    : session?.liveTranscode
      ? "Temporary live stream"
      : session?.compatibilityMode
        ? preflight?.compatibilityCached
          ? "Cached playback copy"
          : "New playback copy"
        : embeddedMpvSession || nativeSession
          ? "Original file through mpv"
          : session
            ? "Original file through WebView"
            : "Not started";
  const processing = session?.compatibilityTranscoded || session?.liveTranscode
    ? "Video and audio converted to H.264/AAC"
    : session?.compatibilityMode
      ? "Streams copied without codec conversion"
      : "No compatibility conversion";
  const reason = preflight?.reason || error?.message || (preflight?.compatibilityRequired
    ? "This file needs a compatibility playback route in the embedded WebView."
    : "The WebView preflight did not request a compatibility route.");

  return <details className="group border-t border-white/10 bg-[#0d0c09] text-white/70">
    <summary className="flex min-h-10 cursor-pointer list-none items-center gap-2 px-5 py-2 text-[11px] font-semibold transition hover:bg-white/[0.035] hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-accent/70">
      <Film size={13} className="text-accent" />
      Playback diagnostics
      <span className="ml-1 truncate text-[10px] font-normal text-white/40">{renderer}</span>
      <ChevronRight className="ml-auto transition-transform group-open:rotate-90" size={14} />
    </summary>
    <div className="border-t border-white/[0.07] px-5 py-4">
      <dl className="grid gap-x-8 gap-y-3 text-[11px] sm:grid-cols-2">
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Renderer</dt><dd className="mt-1 font-medium text-white/80">{renderer}</dd></div>
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Delivery</dt><dd className="mt-1 font-medium text-white/80">{delivery}</dd></div>
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Container and codecs</dt><dd className="number-font mt-1 text-white/70">{item.container?.toLocaleUpperCase() || "Unknown container"} · {formatMediaCodec(item.videoCodec)} video · {formatMediaCodec(item.audioCodec)} audio</dd></div>
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Playback processing</dt><dd className="mt-1 text-white/70">{processing}</dd></div>
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Stream start</dt><dd className="number-font mt-1 text-white/70">{session && session.streamStartSeconds > 0 ? formatDuration(session.streamStartSeconds) : "Beginning of media"}</dd></div>
        <div><dt className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/35">Subtitles</dt><dd className="number-font mt-1 text-white/70">{subtitleCount} available track{subtitleCount === 1 ? "" : "s"}</dd></div>
      </dl>
      <div className="mt-4 border-l-2 border-accent/45 bg-white/[0.025] px-3 py-2.5">
        <p className="text-[9px] font-semibold uppercase tracking-[0.1em] text-accent">Preflight explanation</p>
        <p className="mt-1 text-[11px] leading-5 text-white/55">{reason}</p>
      </div>
      <p className="mt-3 text-[9px] leading-4 text-white/30">Diagnostics intentionally omit local paths, server tokens, and process arguments.</p>
    </div>
  </details>;
}

function EmbeddedVideoPlayer({ item, sourceName, compact = false, leanBack = false, seekRequest, onRestore, onClose, onExitLeanBack, onPrevious, onNext, onPlaybackEnded, onMiniPlayerMoveStart, onMiniPlayerMoveKey, onMiniPlayerResizeStart, onMiniPlayerResizeKey, onMiniPlayerReset }: { item: LibraryItem; sourceName: string; compact?: boolean; leanBack?: boolean; seekRequest: TranscriptSeekRequest | null; onRestore?: () => void; onClose: () => void; onExitLeanBack?: () => void; onPrevious?: () => void; onNext?: () => void; onPlaybackEnded?: () => void; onMiniPlayerMoveStart?: (event: ReactPointerEvent<HTMLElement>) => void; onMiniPlayerMoveKey?: (event: ReactKeyboardEvent<HTMLElement>) => void; onMiniPlayerResizeStart?: (event: ReactPointerEvent<HTMLElement>) => void; onMiniPlayerResizeKey?: (event: ReactKeyboardEvent<HTMLElement>) => void; onMiniPlayerReset?: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const recordEmbeddedPlaybackEvidence = useAppStore((state) => state.recordEmbeddedPlaybackEvidence);
  const playbackFingerprint = embeddedPlaybackFingerprint(item);
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const playerRef = useRef<HTMLElement | null>(null);
  const playbackSurfaceRef = useRef<HTMLDivElement | null>(null);
  const consentDialogRef = useRef<HTMLElement | null>(null);
  const consentReturnFocus = useRef<HTMLElement | null>(null);
  const lastSavedPosition = useRef(0);
  const playbackWatchdog = useRef<number | null>(null);
  const playbackProgress = useRef<{ position: number; observedAt: number } | null>(null);
  const playbackVerification = useRef<{ lastPosition: number; accumulatedSeconds: number; recorded: boolean } | null>(null);
  const webPlaybackHandoff = useRef<{ positionSeconds: number; shouldPlay: boolean } | null>(null);
  const hlsFallbackAttempted = useRef(false);
  const liveFallbackAttempted = useRef(false);
  const livePlaybackShouldPlay = useRef(false);
  const embeddedMpvStopRequested = useRef(false);
  const nativeInitializationItemId = useRef(item.id);
  const subtitleCueOrigins = useRef(new WeakMap<TextTrackCue, SubtitleCueOrigin>());
  const displayPanGesture = useRef<{ pointerId: number; startX: number; startY: number; panX: number; panY: number } | null>(null);
  const displaySettingsLoaded = useRef(false);
  const lastSavedDisplaySettings = useRef<string | null>(null);
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
  const [nativeStatus, setNativeStatus] = useState<NativePlaybackStatus | null>(null);
  const [nativeSession, setNativeSession] = useState<NativePlaybackSession | null>(null);
  const [nativeBusy, setNativeBusy] = useState(false);
  const [embeddedMpvStatus, setEmbeddedMpvStatus] = useState<EmbeddedMpvStatus | null>(null);
  const [embeddedMpvSession, setEmbeddedMpvSession] = useState<EmbeddedMpvSession | null>(null);
  const [embeddedMpvBusy, setEmbeddedMpvBusy] = useState(false);
  const [playbackMuted, setPlaybackMuted] = useState(false);
  const [playbackVolume, setPlaybackVolume] = useState(1);
  const [playbackRenderer, setPlaybackRenderer] = useState<"native" | "web">("web");
  const [nativeFallbackMessage, setNativeFallbackMessage] = useState<string | null>(null);
  const [resumedAt, setResumedAt] = useState<number | null>(null);
  const [playbackRate, setPlaybackRate] = useState(1);
  const [isPlaying, setIsPlaying] = useState(false);
  const [currentPosition, setCurrentPosition] = useState(0);
  const [playbackDuration, setPlaybackDuration] = useState(item.durationSeconds || 0);
  const [loopStart, setLoopStart] = useState<number | null>(null);
  const [loopEnd, setLoopEnd] = useState<number | null>(null);
  const [screenshotBusy, setScreenshotBusy] = useState(false);
  const [pendingNativeSeek, setPendingNativeSeek] = useState<number | null>(null);
  const [previewStoryboard, setPreviewStoryboard] = useState<LibraryStoryboard | null>(null);
  const [previewStoryboardBusy, setPreviewStoryboardBusy] = useState(false);
  const [previewStoryboardUnavailable, setPreviewStoryboardUnavailable] = useState(false);
  const [subtitleCenterOpen, setSubtitleCenterOpen] = useState(false);
  const [subtitleTracks, setSubtitleTracks] = useState<LibrarySubtitleTrack[]>([]);
  const [embeddedSubtitleBusy, setEmbeddedSubtitleBusy] = useState(false);
  const [embeddedSubtitleScanned, setEmbeddedSubtitleScanned] = useState(false);
  const [embeddedSubtitleMessage, setEmbeddedSubtitleMessage] = useState<string | null>(null);
  const [selectedSubtitleIndex, setSelectedSubtitleIndex] = useState(-1);
  const [subtitleOffsetSeconds, setSubtitleOffsetSeconds] = useState(0);
  const [subtitleSizePercent, setSubtitleSizePercent] = useState(100);
  const [subtitlePosition, setSubtitlePosition] = useState<SubtitlePosition>("bottom");
  const [subtitleColor, setSubtitleColor] = useState<SubtitleColor>("white");
  const [subtitleBackground, setSubtitleBackground] = useState<SubtitleBackground>("soft");
  const [displaySettings, setDisplaySettings] = useState<LibraryVideoDisplaySettings>(DEFAULT_VIDEO_DISPLAY_SETTINGS);
  const [displayPanelOpen, setDisplayPanelOpen] = useState(false);
  const [displayRemembered, setDisplayRemembered] = useState(false);
  const [displayRememberBusy, setDisplayRememberBusy] = useState(false);

  function recordDirectPlaybackEvidence(evidence: "played" | "failed") {
    if (!session || session.compatibilityMode || session.liveTranscode) return;
    recordEmbeddedPlaybackEvidence(playbackFingerprint, evidence);
  }

  useEffect(() => {
    let disposed = false;
    nativeInitializationItemId.current = item.id;
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
    setNativeSession(null);
    setNativeBusy(false);
    setEmbeddedMpvSession(null);
    setEmbeddedMpvBusy(false);
    setPlaybackMuted(false);
    setPlaybackVolume(1);
    setPlaybackRenderer("web");
    setNativeFallbackMessage(null);
    setIsPlaying(false);
    setCurrentPosition(0);
    setPlaybackDuration(item.durationSeconds || 0);
    setLoopStart(null);
    setLoopEnd(null);
    setScreenshotBusy(false);
    setPendingNativeSeek(null);
    setPreviewStoryboard(null);
    setPreviewStoryboardBusy(false);
    setPreviewStoryboardUnavailable(false);
    setSubtitleCenterOpen(false);
    setDisplayPanelOpen(false);
    setSubtitleTracks([]);
    setEmbeddedSubtitleBusy(false);
    setEmbeddedSubtitleScanned(false);
    setEmbeddedSubtitleMessage(null);
    setSelectedSubtitleIndex(-1);
    setSubtitleOffsetSeconds(0);
    subtitleCueOrigins.current = new WeakMap<TextTrackCue, SubtitleCueOrigin>();
    webPlaybackHandoff.current = null;
    hlsFallbackAttempted.current = false;
    liveFallbackAttempted.current = false;
    livePlaybackShouldPlay.current = false;

    const isCurrent = () => !disposed && nativeInitializationItemId.current === item.id;
    const prepareWebPlayback = async () => {
      if (!isCurrent()) return;
      setPlaybackRenderer("web");
      const inspected = await api.inspectLibraryPlayback(item.id);
      if (!isCurrent()) return;
      setPreflight(inspected);
      if (inspected.compatibilityRequired) {
        hlsFallbackAttempted.current = true;
        livePlaybackShouldPlay.current = true;
        let requestedStart = seekRequest?.itemId === item.id ? seekRequest.seconds : 0;
        let prepared: LibraryPlaybackSession;
        try {
          prepared = await api.prepareLibraryHlsPlayback(item.id, requestedStart);
        } catch {
          liveFallbackAttempted.current = true;
          prepared = await api.prepareLibraryLivePlayback(item.id, requestedStart);
        }
        const saved = prepared.playback;
        if (requestedStart === 0 && saved && !saved.completed && saved.positionSeconds >= 5
          && (!item.durationSeconds || saved.positionSeconds < item.durationSeconds - 10)) {
          requestedStart = saved.positionSeconds;
          prepared = prepared.segmentedStream
            ? await api.prepareLibraryHlsPlayback(item.id, requestedStart)
            : await api.prepareLibraryLivePlayback(item.id, requestedStart);
        }
        if (isCurrent()) setSession(prepared);
        return;
      }
      if (inspected.requiresConfirmation) {
        consentReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setConsent(inspected);
        setLoading(false);
        return;
      }
      const prepared = await api.prepareLibraryPlayback(item.id, false);
      if (isCurrent()) setSession(prepared);
    };

    void (async () => {
      try {
        const status = await api.getEmbeddedMpvStatus();
        if (!isCurrent()) return;
        setEmbeddedMpvStatus(status);
        if (!status.available) {
          setNativeFallbackMessage(status.detail);
        }
      } catch (error) {
        if (!isCurrent()) return;
        setNativeFallbackMessage(normalizeError(error).message);
      }
      try {
        await prepareWebPlayback();
      } catch (error) {
        if (isCurrent()) {
          setPlaybackError(normalizeError(error));
          setLoading(false);
        }
      }
    })();
    return () => {
      disposed = true;
    };
  }, [item.id]);

  useEffect(() => {
    let disposed = false;
    displaySettingsLoaded.current = false;
    lastSavedDisplaySettings.current = null;
    setDisplaySettings(DEFAULT_VIDEO_DISPLAY_SETTINGS);
    setDisplayRemembered(false);
    setDisplayRememberBusy(false);
    void api.getLibraryVideoDisplaySettings(item.id)
      .then((saved) => {
        if (disposed) return;
        setDisplaySettings(saved || DEFAULT_VIDEO_DISPLAY_SETTINGS);
        setDisplayRemembered(Boolean(saved));
        lastSavedDisplaySettings.current = saved ? JSON.stringify(saved) : null;
        displaySettingsLoaded.current = true;
      })
      .catch((error) => {
        if (disposed) return;
        displaySettingsLoaded.current = true;
        setError(normalizeError(error));
      });
    return () => {
      disposed = true;
    };
  }, [item.id, setError]);

  useEffect(() => {
    if (!displaySettingsLoaded.current || !displayRemembered) return;
    const signature = JSON.stringify(displaySettings);
    if (signature === lastSavedDisplaySettings.current) return;
    const timer = window.setTimeout(() => {
      setDisplayRememberBusy(true);
      void api.saveLibraryVideoDisplaySettings({ itemId: item.id, settings: displaySettings })
        .then(() => {
          lastSavedDisplaySettings.current = signature;
        })
        .catch((error) => setError(normalizeError(error)))
        .finally(() => setDisplayRememberBusy(false));
    }, 300);
    return () => window.clearTimeout(timer);
  }, [displayRemembered, displaySettings, item.id, setError]);

  useEffect(() => {
    if (compact || leanBack) setDisplayPanelOpen(false);
  }, [compact, leanBack]);

  const effectiveDisplaySettings = compact ? DEFAULT_VIDEO_DISPLAY_SETTINGS : displaySettings;

  useEffect(() => {
    if (!embeddedMpvSession) return;
    void api.controlEmbeddedMpv({ action: "setDisplay", display: effectiveDisplaySettings })
      .catch((error) => setError(normalizeError(error)));
  }, [effectiveDisplaySettings, embeddedMpvSession?.itemId, setError]);

  useEffect(() => {
    let disposed = false;
    void api.getNativePlaybackStatus()
      .then((status) => {
        if (disposed) return;
        setNativeStatus(status);
        if (status.active?.itemId === item.id) setNativeSession(status.active);
      })
      .catch(() => {
        if (!disposed) setNativeStatus(null);
      });
    return () => { disposed = true; };
  }, [item.id]);

  useEffect(() => {
    if (!embeddedMpvSession) return;
    let disposed = false;
    let previouslyActive = true;
    let unlisten: (() => void) | undefined;
    const applyStatus = (status: EmbeddedMpvStatus) => {
      if (disposed) return;
      setEmbeddedMpvStatus(status);
      if (status.active?.itemId === item.id) {
        setEmbeddedMpvSession(status.active);
        setCurrentPosition(status.active.positionSeconds);
        setPlaybackDuration(status.active.durationSeconds);
        setIsPlaying(!status.active.paused);
        previouslyActive = true;
      } else if (previouslyActive) {
        previouslyActive = false;
        setEmbeddedMpvSession(null);
        setIsPlaying(false);
        if (embeddedMpvStopRequested.current) {
          embeddedMpvStopRequested.current = false;
        } else if (status.lastError) {
          setPlaybackError(createPlaybackError("embedded_mpv_runtime_failed", status.lastError));
        } else {
          window.setTimeout(() => onPlaybackEnded?.(), 0);
        }
      }
    };
    const refreshFallback = () => {
      void api.getEmbeddedMpvStatus().then(applyStatus).catch(() => {});
    };
    refreshFallback();
    void api.onEmbeddedMpvStatus(applyStatus).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    const interval = window.setInterval(refreshFallback, 2_000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
      unlisten?.();
    };
  }, [embeddedMpvSession?.itemId, item.id, onPlaybackEnded]);

  function readEmbeddedMpvBounds(): EmbeddedMpvBounds | null {
    const surface = playbackSurfaceRef.current;
    if (!surface) return null;
    const rect = surface.getBoundingClientRect();
    return {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
      devicePixelRatio: window.devicePixelRatio || 1,
      visible: document.visibilityState === "visible"
        && rect.width >= 2
        && rect.height >= 2
        && rect.right > 0
        && rect.bottom > 0
        && rect.left < window.innerWidth
        && rect.top < window.innerHeight,
    };
  }

  useEffect(() => {
    if (!embeddedMpvSession) return;
    let disposed = false;
    let updateInFlight = false;
    let pendingBounds: EmbeddedMpvBounds | null = null;
    let lastSentSignature: string | null = null;

    const boundsSignature = (bounds: EmbeddedMpvBounds) => [
      Math.round(bounds.x),
      Math.round(bounds.y),
      Math.round(bounds.width),
      Math.round(bounds.height),
      bounds.devicePixelRatio,
      bounds.visible,
    ].join(":");

    const flushBounds = () => {
      if (disposed || updateInFlight || !pendingBounds) return;
      const bounds = pendingBounds;
      pendingBounds = null;
      const signature = boundsSignature(bounds);
      if (signature === lastSentSignature) return;

      updateInFlight = true;
      lastSentSignature = signature;
      void api.updateEmbeddedMpvBounds(bounds)
        .catch(() => {})
        .finally(() => {
          updateInFlight = false;
          flushBounds();
        });
    };

    const syncBounds = () => {
      const bounds = readEmbeddedMpvBounds();
      if (!bounds) return;
      if (boundsSignature(bounds) === lastSentSignature) {
        pendingBounds = null;
        return;
      }
      pendingBounds = bounds;
      flushBounds();
    };

    const observer = new ResizeObserver(syncBounds);
    if (playbackSurfaceRef.current) observer.observe(playbackSurfaceRef.current);
    window.addEventListener("resize", syncBounds);
    window.addEventListener("scroll", syncBounds, true);
    window.addEventListener("nice-downloader:player-bounds-changed", syncBounds);
    document.addEventListener("fullscreenchange", syncBounds);
    document.addEventListener("visibilitychange", syncBounds);
    window.visualViewport?.addEventListener("resize", syncBounds);
    window.visualViewport?.addEventListener("scroll", syncBounds);
    syncBounds();
    // ResizeObserver does not report position-only movement. A low-frequency safety sample
    // keeps the native GTK surface aligned during CSS transitions and mini-player dragging
    // without forcing layout on every animation frame.
    const positionSample = window.setInterval(syncBounds, compact ? 500 : 250);
    return () => {
      disposed = true;
      window.clearInterval(positionSample);
      observer.disconnect();
      window.removeEventListener("resize", syncBounds);
      window.removeEventListener("scroll", syncBounds, true);
      window.removeEventListener("nice-downloader:player-bounds-changed", syncBounds);
      document.removeEventListener("fullscreenchange", syncBounds);
      document.removeEventListener("visibilitychange", syncBounds);
      window.visualViewport?.removeEventListener("resize", syncBounds);
      window.visualViewport?.removeEventListener("scroll", syncBounds);
    };
  }, [compact, embeddedMpvSession?.itemId]);

  useEffect(() => {
    if (!nativeSession) return;
    let disposed = false;
    const refresh = () => {
      void api.getNativePlaybackStatus()
        .then((status) => {
          if (disposed) return;
          setNativeStatus(status);
          if (status.active?.itemId !== item.id) {
            setNativeSession(null);
            void restartEmbeddedPlaybackAfterNative();
          }
        })
        .catch(() => {});
    };
    const interval = window.setInterval(refresh, 1_000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [item.id, nativeSession]);

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
      recordDirectPlaybackEvidence("failed");
      if (advanceCompatibilitySource(videoRef.current, true)) return;
      setPlaybackError(createPlaybackError(
        "embedded_playback_timeout",
        session.liveTranscode
          ? "The available compatibility streams did not become ready. Try libmpv, a cached H.264 copy, or your system player."
          : "Embedded playback did not become ready. Your video is unchanged; try opening it with your system player.",
      ));
      setLoading(false);
    }, EMBEDDED_PLAYBACK_TIMEOUT_MS);
    return () => window.clearTimeout(timer);
  }, [session, loading]);

  function clearPlaybackWatchdog() {
    if (playbackWatchdog.current == null) return;
    window.clearTimeout(playbackWatchdog.current);
    playbackWatchdog.current = null;
  }

  function logicalVideoPosition(video: HTMLVideoElement) {
    const position = session?.liveTranscode
      ? session.streamStartSeconds + video.currentTime
      : video.currentTime;
    return Number.isFinite(position) ? Math.max(0, position) : 0;
  }

  function logicalVideoDuration(video: HTMLVideoElement) {
    if (session?.liveTranscode) {
      return item.durationSeconds && item.durationSeconds > 0
        ? item.durationSeconds
        : Math.max(playbackDuration, logicalVideoPosition(video));
    }
    return video.duration;
  }

  async function startHlsPlaybackAt(positionSeconds: number, shouldPlay: boolean) {
    if (!Number.isFinite(positionSeconds)) return;
    clearPlaybackWatchdog();
    hlsFallbackAttempted.current = true;
    livePlaybackShouldPlay.current = shouldPlay;
    videoRef.current?.pause();
    setPlaybackError(null);
    setLoading(true);
    const maximum = item.durationSeconds ? Math.max(0, item.durationSeconds - 0.25) : Number.MAX_SAFE_INTEGER;
    const target = Math.min(maximum, Math.max(0, positionSeconds));
    try {
      const prepared = await api.prepareLibraryHlsPlayback(item.id, target);
      setSession(prepared);
      setCurrentPosition(target);
      setPlaybackDuration(item.durationSeconds || playbackDuration);
    } catch {
      if (!liveFallbackAttempted.current) {
        await startLivePlaybackAt(target, shouldPlay);
        return;
      }
      setPlaybackError(createPlaybackError(
        "segmented_playback_unavailable",
        "Segmented compatibility playback could not start. Try libmpv, a cached playback copy, or your system player.",
      ));
      setLoading(false);
    }
  }

  async function startLivePlaybackAt(positionSeconds: number, shouldPlay: boolean) {
    if (!Number.isFinite(positionSeconds)) return;
    clearPlaybackWatchdog();
    liveFallbackAttempted.current = true;
    livePlaybackShouldPlay.current = shouldPlay;
    videoRef.current?.pause();
    setPlaybackError(null);
    setLoading(true);
    try {
      const maximum = item.durationSeconds ? Math.max(0, item.durationSeconds - 0.25) : Number.MAX_SAFE_INTEGER;
      const target = Math.min(maximum, Math.max(0, positionSeconds));
      const prepared = await api.prepareLibraryLivePlayback(item.id, target);
      setSession(prepared);
      setCurrentPosition(target);
      setPlaybackDuration(item.durationSeconds || playbackDuration);
    } catch (error) {
      livePlaybackShouldPlay.current = false;
      setPlaybackError(normalizeError(error));
      setLoading(false);
    }
  }

  function restartCompatibilityPlaybackAt(positionSeconds: number, shouldPlay: boolean) {
    if (session?.segmentedStream) void startHlsPlaybackAt(positionSeconds, shouldPlay);
    else void startLivePlaybackAt(positionSeconds, shouldPlay);
  }

  function advanceCompatibilitySource(video: HTMLVideoElement | null, shouldPlay: boolean) {
    const position = video ? logicalVideoPosition(video) : session?.playback?.positionSeconds || 0;
    if (!session?.liveTranscode && !hlsFallbackAttempted.current) {
      void startHlsPlaybackAt(position, shouldPlay);
      return true;
    }
    if (session?.segmentedStream && !liveFallbackAttempted.current) {
      void startLivePlaybackAt(position, shouldPlay);
      return true;
    }
    return false;
  }

  function notePlaybackProgress(video: HTMLVideoElement) {
    if (!Number.isFinite(video.currentTime)) return;
    if (session && !session.compatibilityMode && !session.liveTranscode) {
      const verification = playbackVerification.current;
      if (!verification) {
        playbackVerification.current = { lastPosition: video.currentTime, accumulatedSeconds: 0, recorded: false };
      } else {
        const delta = video.currentTime - verification.lastPosition;
        verification.lastPosition = video.currentTime;
        if (delta > 0 && delta <= 2) verification.accumulatedSeconds += delta;
        if (!verification.recorded && verification.accumulatedSeconds >= 3) {
          verification.recorded = true;
          recordDirectPlaybackEvidence("played");
        }
      }
    }
    const position = logicalVideoPosition(video);
    const previous = playbackProgress.current;
    if (!previous || Math.abs(position - previous.position) >= 0.01) {
      playbackProgress.current = { position, observedAt: performance.now() };
    }
  }

  function armPlaybackWatchdog() {
    if (playbackWatchdog.current != null) return;
    const checkProgress = () => {
      playbackWatchdog.current = null;
      const video = videoRef.current;
      if (!video || video.paused || video.ended) return;
      notePlaybackProgress(video);
      const observedAt = playbackProgress.current?.observedAt ?? performance.now();
      const stalledFor = performance.now() - observedAt;
      if (stalledFor < EMBEDDED_PLAYBACK_TIMEOUT_MS) {
        playbackWatchdog.current = window.setTimeout(
          checkProgress,
          Math.max(50, EMBEDDED_PLAYBACK_TIMEOUT_MS - stalledFor),
        );
        return;
      }
      video.pause();
      recordDirectPlaybackEvidence("failed");
      if (advanceCompatibilitySource(video, true)) return;
      setLoading(false);
      setPlaybackError(createPlaybackError(
        "embedded_playback_timeout",
        session?.liveTranscode
          ? "The available compatibility streams stalled. Try libmpv, a cached H.264 copy, or your system player."
          : "Embedded playback stalled after starting. Try an H.264 playback copy or open the original in your system player.",
      ));
    };
    playbackWatchdog.current = window.setTimeout(checkProgress, EMBEDDED_PLAYBACK_TIMEOUT_MS);
  }

  function startPlaybackWatchdog() {
    clearPlaybackWatchdog();
    const video = videoRef.current;
    if (video && Number.isFinite(video.currentTime)) {
      playbackProgress.current = { position: logicalVideoPosition(video), observedAt: performance.now() };
    }
    armPlaybackWatchdog();
  }

  function armPlaybackWatchdogWhilePlaying() {
    const video = videoRef.current;
    if (video && !video.paused && !video.ended) armPlaybackWatchdog();
  }

  useEffect(() => () => {
    clearPlaybackWatchdog();
    playbackProgress.current = null;
    playbackVerification.current = null;
  }, [item.id, session?.url]);

  function persistProgress(completed = false) {
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.currentTime)) return;
    const duration = logicalVideoDuration(video);
    const position = logicalVideoPosition(video);
    if (!Number.isFinite(duration) || duration <= 0) return;
    lastSavedPosition.current = position;
    void api.saveLibraryPlaybackProgress({
      itemId: item.id,
      positionSeconds: position,
      durationSeconds: duration,
      completed,
    }).catch(() => {});
  }

  useEffect(() => () => persistProgress(false), [item.id]);

  useEffect(() => () => {
    void api.releaseLibraryPlayback(item.id).catch(() => {});
  }, [item.id]);

  useEffect(() => () => {
    embeddedMpvStopRequested.current = true;
    void api.closeEmbeddedMpv().catch(() => {});
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
    if (embeddedMpvSession) {
      embeddedMpvStopRequested.current = true;
      void api.closeEmbeddedMpv().catch(() => {});
    }
    else persistProgress(false);
    onClose();
  }

  function changeVideo(change: (() => void) | undefined) {
    if (!change) return;
    if (embeddedMpvSession) {
      embeddedMpvStopRequested.current = true;
      void api.closeEmbeddedMpv().catch(() => {});
    }
    else persistProgress(false);
    change();
  }

  function togglePlayback() {
    if (embeddedMpvSession) {
      const action = embeddedMpvSession.paused ? "play" : "pause";
      void api.controlEmbeddedMpv({ action }).catch((error) => setError(normalizeError(error)));
      return;
    }
    const video = videoRef.current;
    if (!video) return;
    if (video.paused) void video.play().catch(() => {});
    else video.pause();
  }

  function toggleMuted() {
    const restoringVolume = playbackVolume === 0;
    const volume = restoringVolume ? 1 : playbackVolume;
    const muted = restoringVolume ? false : !playbackMuted;
    if (restoringVolume) setPlaybackVolume(volume);
    setPlaybackMuted(muted);
    if (embeddedMpvSession) {
      if (restoringVolume) void api.controlEmbeddedMpv({ action: "setVolume", value: volume * 100 }).catch((error) => setError(normalizeError(error)));
      void api.controlEmbeddedMpv({ action: "setMuted", value: muted ? 1 : 0 }).catch((error) => setError(normalizeError(error)));
    }
    const video = videoRef.current;
    if (video) {
      if (restoringVolume) video.volume = volume;
      video.muted = muted;
    }
  }

  function changeVolume(volume: number) {
    const normalized = Math.min(1, Math.max(0, Number.isFinite(volume) ? volume : 1));
    const muted = normalized === 0;
    setPlaybackVolume(normalized);
    setPlaybackMuted(muted);
    if (embeddedMpvSession) {
      void api.controlEmbeddedMpv({ action: "setVolume", value: normalized * 100 }).catch((error) => setError(normalizeError(error)));
      void api.controlEmbeddedMpv({ action: "setMuted", value: muted ? 1 : 0 }).catch((error) => setError(normalizeError(error)));
    }
    const video = videoRef.current;
    if (video) {
      video.volume = normalized;
      video.muted = muted;
    }
  }

  function seek(seconds: number) {
    if (embeddedMpvSession) {
      void api.controlEmbeddedMpv({ action: "seekRelative", value: seconds }).catch((error) => setError(normalizeError(error)));
      return;
    }
    if (session?.liveTranscode) {
      restartCompatibilityPlaybackAt(currentPosition + seconds, isPlaying);
      return;
    }
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration)) return;
    video.currentTime = Math.min(video.duration, Math.max(0, video.currentTime + seconds));
  }

  function seekTo(seconds: number) {
    if (embeddedMpvSession) {
      if (!Number.isFinite(seconds)) return;
      void api.controlEmbeddedMpv({ action: "seekAbsolute", value: Math.max(0, seconds) }).catch((error) => setError(normalizeError(error)));
      setCurrentPosition(Math.max(0, seconds));
      return;
    }
    if (session?.liveTranscode) {
      restartCompatibilityPlaybackAt(seconds, isPlaying);
      setCurrentPosition(Math.max(0, seconds));
      return;
    }
    const video = videoRef.current;
    if (!video || !Number.isFinite(seconds)) return;
    const duration = Number.isFinite(video.duration) ? video.duration : playbackDuration;
    video.currentTime = Math.min(Math.max(0, duration), Math.max(0, seconds));
    setCurrentPosition(video.currentTime);
  }

  function commitNativeSeek(seconds: number) {
    if (Number.isFinite(seconds)) seekTo(seconds);
    setPendingNativeSeek(null);
  }

  function setLoopStartAtCurrentPosition() {
    const start = Math.max(0, currentPosition);
    setLoopStart(start);
    if (loopEnd != null && loopEnd <= start + 0.05) setLoopEnd(null);
  }

  function setLoopEndAtCurrentPosition() {
    if (loopStart == null) return;
    if (currentPosition <= loopStart + 0.05) {
      setNotice({ message: "Move past point A before setting loop point B.", view: "library" });
      return;
    }
    setLoopEnd(currentPosition);
  }

  function clearLoop() {
    setLoopStart(null);
    setLoopEnd(null);
  }

  function stepFrame(direction: -1 | 1) {
    if (session?.liveTranscode) return;
    const frameSeconds = 1 / (item.fps && item.fps > 0 ? item.fps : 30);
    if (embeddedMpvSession) {
      const action = direction < 0 ? "frameBackStep" : "frameStep";
      void api.controlEmbeddedMpv({ action }).catch((error) => setError(normalizeError(error)));
      setCurrentPosition((position) => Math.max(0, position + frameSeconds * direction));
      setIsPlaying(false);
      return;
    }
    const video = videoRef.current;
    if (!video) return;
    video.pause();
    seekTo(currentPosition + frameSeconds * direction);
  }

  async function captureFrame() {
    if (screenshotBusy) return;
    const safeTitle = item.title.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_").trim().slice(0, 80) || "video";
    const selected = await save({
      title: "Save current video frame",
      defaultPath: `${safeTitle}-frame-${Math.floor(Math.max(0, currentPosition))}.png`,
      filters: [{ name: "PNG image", extensions: ["png"] }, { name: "JPEG image", extensions: ["jpg", "jpeg"] }],
    });
    if (!selected) return;
    setScreenshotBusy(true);
    try {
      await api.captureLibraryFrame(item.id, Math.max(0, currentPosition), selected);
      setNotice({ message: `Captured frame at ${formatDuration(currentPosition)}.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setScreenshotBusy(false);
    }
  }

  useEffect(() => {
    if (!isPlaying || loopStart == null || loopEnd == null || loopEnd <= loopStart) return;
    if (currentPosition >= loopEnd) seekTo(loopStart);
  }, [currentPosition, isPlaying, loopEnd, loopStart]);

  function requestPreviewStoryboard() {
    if (previewStoryboard || previewStoryboardBusy || previewStoryboardUnavailable || playbackDuration <= 0) return;
    setPreviewStoryboardBusy(true);
    void api.generateLibraryStoryboard(item.id, compact ? 8 : 16)
      .then((generated) => {
        if (generated.itemId === item.id) setPreviewStoryboard(generated);
      })
      .catch(() => setPreviewStoryboardUnavailable(true))
      .finally(() => setPreviewStoryboardBusy(false));
  }

  function applySubtitleSettings() {
    const video = videoRef.current;
    if (!video || !session) return;
    for (let index = 0; index < video.textTracks.length; index += 1) {
      const textTrack = video.textTracks[index];
      const selected = index === selectedSubtitleIndex;
      textTrack.mode = selected ? "showing" : "disabled";
      if (!selected || !textTrack.cues) continue;
      for (const cue of Array.from(textTrack.cues)) {
        let origin = subtitleCueOrigins.current.get(cue);
        if (!origin) {
          const visualCue = typeof VTTCue !== "undefined" && cue instanceof VTTCue ? cue : null;
          origin = {
            startTime: cue.startTime,
            endTime: cue.endTime,
            line: visualCue?.line ?? "auto",
            snapToLines: visualCue?.snapToLines ?? true,
          };
          subtitleCueOrigins.current.set(cue, origin);
        }
        cue.startTime = Math.max(0, origin.startTime - session.streamStartSeconds + subtitleOffsetSeconds);
        cue.endTime = Math.max(cue.startTime + 0.05, origin.endTime - session.streamStartSeconds + subtitleOffsetSeconds);
        if (typeof VTTCue !== "undefined" && cue instanceof VTTCue) {
          cue.snapToLines = false;
          cue.line = subtitlePosition === "top" ? 12 : subtitlePosition === "middle" ? 50 : 88;
        }
      }
    }
  }

  function onSubtitleTrackLoaded() {
    window.requestAnimationFrame(applySubtitleSettings);
  }

  useEffect(() => {
    if (!session) return;
    setSubtitleTracks(session.subtitles);
    setSelectedSubtitleIndex(-1);
    setEmbeddedSubtitleScanned(false);
    setEmbeddedSubtitleMessage(null);
  }, [session?.item.id]);

  function toggleSubtitleCenter() {
    const opening = !subtitleCenterOpen;
    setSubtitleCenterOpen(opening);
    if (!opening || embeddedSubtitleScanned || embeddedSubtitleBusy) return;
    setEmbeddedSubtitleBusy(true);
    setEmbeddedSubtitleMessage(null);
    void api.loadLibraryEmbeddedSubtitles(item.id)
      .then((embedded) => {
        setSubtitleTracks((current) => [...current, ...embedded.filter((track) => !current.some((candidate) => candidate.url === track.url))]);
        setEmbeddedSubtitleMessage(embedded.length ? `${embedded.length} embedded text track${embedded.length === 1 ? "" : "s"} ready.` : "No compatible embedded text subtitles were found.");
      })
      .catch((error) => setEmbeddedSubtitleMessage(normalizeError(error).message))
      .finally(() => {
        setEmbeddedSubtitleBusy(false);
        setEmbeddedSubtitleScanned(true);
      });
  }

  useEffect(() => {
    applySubtitleSettings();
  }, [selectedSubtitleIndex, session?.streamStartSeconds, session?.url, subtitleOffsetSeconds, subtitlePosition, subtitleTracks]);

  function updateDisplaySettings(patch: Partial<LibraryVideoDisplaySettings>) {
    setDisplaySettings((current) => {
      const next = { ...current, ...patch };
      if (next.zoom <= 1) {
        next.panX = 0;
        next.panY = 0;
      }
      return next;
    });
  }

  async function toggleRememberDisplaySettings(remember: boolean) {
    setDisplayRememberBusy(true);
    try {
      if (remember) {
        await api.saveLibraryVideoDisplaySettings({ itemId: item.id, settings: displaySettings });
        lastSavedDisplaySettings.current = JSON.stringify(displaySettings);
        setDisplayRemembered(true);
        setNotice({ message: "Video appearance will be restored the next time you watch it.", view: "library" });
      } else {
        await api.forgetLibraryVideoDisplaySettings(item.id);
        lastSavedDisplaySettings.current = null;
        setDisplayRemembered(false);
        setNotice({ message: "Video appearance is now session-only.", view: "library" });
      }
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setDisplayRememberBusy(false);
    }
  }

  function beginDisplayPan(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || compact || consent || effectiveDisplaySettings.zoom <= 1) return;
    displayPanGesture.current = {
      pointerId: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      panX: displaySettings.panX,
      panY: displaySettings.panY,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function updateDisplayPan(event: ReactPointerEvent<HTMLDivElement>) {
    const gesture = displayPanGesture.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return;
    updateDisplaySettings({
      panX: Math.max(-1, Math.min(1, gesture.panX + ((event.clientX - gesture.startX) * 2) / bounds.width)),
      panY: Math.max(-1, Math.min(1, gesture.panY + ((event.clientY - gesture.startY) * 2) / bounds.height)),
    });
  }

  function endDisplayPan(event: ReactPointerEvent<HTMLDivElement>) {
    if (displayPanGesture.current?.pointerId !== event.pointerId) return;
    displayPanGesture.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
  }

  const subtitleVideoStyle = {
    "--subtitle-size": `${subtitleSizePercent}%`,
    "--subtitle-color": subtitleColor === "yellow" ? "#f6df78" : "#fffdf5",
    "--subtitle-background": subtitleBackground === "none" ? "transparent" : subtitleBackground === "strong" ? "rgba(0, 0, 0, 0.88)" : "rgba(0, 0, 0, 0.62)",
    objectFit: effectiveDisplaySettings.fit === "fit" ? "contain" : effectiveDisplaySettings.fit === "fill" ? "cover" : effectiveDisplaySettings.fit === "original" ? "none" : "fill",
    transform: `translate3d(${effectiveDisplaySettings.panX * 50}%, ${effectiveDisplaySettings.panY * 50}%, 0) rotate(${effectiveDisplaySettings.rotation}deg) scale(${effectiveDisplaySettings.zoom}) scaleX(${effectiveDisplaySettings.flipHorizontal ? -1 : 1}) scaleY(${effectiveDisplaySettings.flipVertical ? -1 : 1})`,
    transformOrigin: "center",
    filter: `brightness(${Math.max(0, 1 + effectiveDisplaySettings.brightness / 100) * (2 ** (effectiveDisplaySettings.gamma / 200))}) contrast(${Math.max(0, 1 + effectiveDisplaySettings.contrast / 100) * (1 - effectiveDisplaySettings.gamma / 500)}) saturate(${Math.max(0, 1 + effectiveDisplaySettings.saturation / 100)})`,
  } as CSSProperties;

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
      if (key === "escape" && !document.fullscreenElement) { event.preventDefault(); if (leanBack) onExitLeanBack?.(); else closePlayer(); }
      else if (key === " " || key === "k") { event.preventDefault(); togglePlayback(); }
      else if (key === "j" || key === "arrowleft") { event.preventDefault(); seek(-15); }
      else if (key === "l" || key === "arrowright") { event.preventDefault(); seek(15); }
      else if (key === "m" && (embeddedMpvSession || videoRef.current)) { event.preventDefault(); toggleMuted(); }
      else if (key === "f") { event.preventDefault(); toggleFullscreen(); }
      else if (key === ",") { event.preventDefault(); stepFrame(-1); }
      else if (key === ".") { event.preventDefault(); stepFrame(1); }
      else if (key === "[") { event.preventDefault(); setLoopStartAtCurrentPosition(); }
      else if (key === "]") { event.preventDefault(); setLoopEndAtCurrentPosition(); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [compact, consent, currentPosition, embeddedMpvSession, item.fps, leanBack, loopEnd, loopStart, onExitLeanBack, playbackMuted, session?.liveTranscode]);

  useEffect(() => {
    if (!leanBack || consent || typeof navigator.getGamepads !== "function") return;
    let frame = 0;
    let previousActions = new Set<string>();
    const poll = () => {
      const actions = new Set<string>();
      for (const gamepad of navigator.getGamepads()) {
        if (!gamepad?.connected) continue;
        if (gamepad.buttons[0]?.pressed || gamepad.buttons[9]?.pressed) actions.add("toggle");
        if (gamepad.buttons[1]?.pressed) actions.add("exit");
        if (gamepad.buttons[14]?.pressed) actions.add("previous");
        if (gamepad.buttons[15]?.pressed) actions.add("next");
        if (gamepad.buttons[4]?.pressed) actions.add("seek-back");
        if (gamepad.buttons[5]?.pressed) actions.add("seek-forward");
      }
      for (const action of actions) {
        if (previousActions.has(action)) continue;
        if (action === "toggle") togglePlayback();
        else if (action === "exit") onExitLeanBack?.();
        else if (action === "previous") changeVideo(onPrevious);
        else if (action === "next") changeVideo(onNext);
        else if (action === "seek-back") seek(-15);
        else if (action === "seek-forward") seek(15);
      }
      previousActions = actions;
      frame = window.requestAnimationFrame(poll);
    };
    frame = window.requestAnimationFrame(poll);
    return () => window.cancelAnimationFrame(frame);
  }, [consent, item.id, leanBack]);

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
    setPlaybackError(createPlaybackError("embedded_playback_cancelled", embeddedMpvStatus?.available
      ? "The compatibility copy was not started. The original is unchanged; you can play it here with experimental libmpv or open it in your system player."
      : "Embedded playback was not started. The original video is unchanged and can still be opened with your system player."));
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
      if (embeddedMpvSession) {
        embeddedMpvStopRequested.current = true;
        await api.closeEmbeddedMpv();
        setEmbeddedMpvSession(null);
      } else {
        persistProgress(false);
      }
      await api.openLibraryItem(item.id);
    } catch (error) {
      setError(normalizeError(error));
    }
  }

  async function playWithNativeMpv() {
    if (nativeBusy || !nativeStatus?.available) return;
    clearPlaybackWatchdog();
    videoRef.current?.pause();
    persistProgress(false);
    setNativeBusy(true);
    try {
      const transcriptStart = seekRequest?.itemId === item.id ? seekRequest.seconds : undefined;
      if (embeddedMpvSession) embeddedMpvStopRequested.current = true;
      const opened = await api.openLibraryNativePlayback(item.id, transcriptStart);
      setEmbeddedMpvSession(null);
      setNativeSession(opened);
      setConsent(null);
      setPlaybackError(null);
      setLoading(false);
      restoreConsentFocus();
    } catch (error) {
      setPlaybackError(normalizeError(error));
      setLoading(false);
    } finally {
      setNativeBusy(false);
    }
  }

  async function playWithEmbeddedMpv() {
    if (embeddedMpvBusy || !embeddedMpvStatus?.available) return;
    const bounds = readEmbeddedMpvBounds();
    if (!bounds) {
      setPlaybackError(createPlaybackError("embedded_mpv_surface_missing", "The experimental native playback surface is not ready."));
      return;
    }
    clearPlaybackWatchdog();
    videoRef.current?.pause();
    persistProgress(false);
    setEmbeddedMpvBusy(true);
    setPlaybackRenderer("native");
    try {
      const transcriptStart = seekRequest?.itemId === item.id ? seekRequest.seconds : undefined;
      const opened = await api.openLibraryEmbeddedMpv(item.id, bounds, transcriptStart);
      embeddedMpvStopRequested.current = false;
      await api.releaseLibraryPlayback(item.id).catch(() => {});
      setSession(null);
      setNativeSession(null);
      setEmbeddedMpvSession(opened);
      void api.controlEmbeddedMpv({ action: "setVolume", value: playbackVolume * 100 }).catch((error) => setError(normalizeError(error)));
      void api.controlEmbeddedMpv({ action: "setMuted", value: playbackMuted ? 1 : 0 }).catch((error) => setError(normalizeError(error)));
      setCurrentPosition(opened.positionSeconds);
      setPlaybackDuration(opened.durationSeconds);
      setIsPlaying(!opened.paused);
      setConsent(null);
      setPlaybackError(null);
      setNativeFallbackMessage(null);
      setLoading(false);
      restoreConsentFocus();
    } catch (error) {
      setPlaybackError(normalizeError(error));
      setLoading(false);
    } finally {
      setEmbeddedMpvBusy(false);
    }
  }

  async function closeEmbeddedMpvPlayback() {
    if (embeddedMpvBusy) return;
    setEmbeddedMpvBusy(true);
    embeddedMpvStopRequested.current = true;
    const handoff = { positionSeconds: currentPosition, shouldPlay: isPlaying };
    try {
      await api.closeEmbeddedMpv();
      setEmbeddedMpvSession(null);
      await restartEmbeddedPlaybackAfterNative(handoff);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setEmbeddedMpvBusy(false);
    }
  }

  async function closeNativePlayback() {
    if (nativeBusy) return;
    setNativeBusy(true);
    try {
      await api.closeLibraryNativePlayback();
      setNativeSession(null);
      await restartEmbeddedPlaybackAfterNative();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setNativeBusy(false);
    }
  }

  async function restartEmbeddedPlaybackAfterNative(handoff: { positionSeconds: number; shouldPlay: boolean } | null = null) {
    clearPlaybackWatchdog();
    webPlaybackHandoff.current = handoff;
    setPlaybackRenderer("web");
    setPlaybackError(null);
    setSession(null);
    setLoading(true);
    try {
      await api.releaseLibraryPlayback(item.id).catch(() => {});
      const inspected = await api.inspectLibraryPlayback(item.id);
      setPreflight(inspected);
      if (inspected.compatibilityRequired) {
        await startHlsPlaybackAt(handoff?.positionSeconds || 0, handoff?.shouldPlay || false);
        return;
      }
      if (inspected.requiresConfirmation) {
        consentReturnFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
        setConsent(inspected);
        setLoading(false);
        return;
      }
      const prepared = await api.prepareLibraryPlayback(item.id, false);
      setSession(prepared);
    } catch (error) {
      webPlaybackHandoff.current = null;
      setPlaybackError(normalizeError(error));
      setLoading(false);
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
    video.volume = playbackVolume;
    video.muted = playbackMuted;
    if (session?.liveTranscode) {
      const position = logicalVideoPosition(video);
      const duration = item.durationSeconds || Math.max(playbackDuration, position);
      setPlaybackDuration(duration);
      setCurrentPosition(position);
      lastSavedPosition.current = position;
      if (session.streamStartSeconds > 0) setResumedAt(session.streamStartSeconds);
      const shouldPlay = livePlaybackShouldPlay.current || seekRequest?.itemId === item.id;
      livePlaybackShouldPlay.current = false;
      if (shouldPlay) void video.play().catch(() => {});
      return;
    }
    setPlaybackDuration(video.duration);
    setCurrentPosition(video.currentTime);
    const handoff = webPlaybackHandoff.current;
    if (handoff) {
      webPlaybackHandoff.current = null;
      const target = Math.min(Math.max(0, video.duration - 0.25), handoff.positionSeconds);
      video.currentTime = target;
      lastSavedPosition.current = target;
      setCurrentPosition(target);
      setResumedAt(target);
      if (handoff.shouldPlay) void video.play().catch(() => {});
      return;
    }
    if (seekRequest?.itemId === item.id) {
      const target = Math.min(Math.max(0, video.duration - 0.25), seekRequest.seconds);
      video.currentTime = target;
      lastSavedPosition.current = target;
      setCurrentPosition(target);
      setResumedAt(target);
      void video.play().catch(() => {});
      return;
    }
    const saved = session?.playback;
    if (saved && !saved.completed && saved.positionSeconds >= 5 && saved.positionSeconds < video.duration - 10) {
      video.currentTime = saved.positionSeconds;
      lastSavedPosition.current = saved.positionSeconds;
      setResumedAt(saved.positionSeconds);
    }
  }

  useEffect(() => {
    if (seekRequest?.itemId !== item.id) return;
    if (embeddedMpvSession) {
      void api.controlEmbeddedMpv({ action: "seekAbsolute", value: seekRequest.seconds }).catch(() => {});
      setCurrentPosition(seekRequest.seconds);
      setResumedAt(seekRequest.seconds);
      return;
    }
    if (session?.liveTranscode) {
      restartCompatibilityPlaybackAt(seekRequest.seconds, true);
      setCurrentPosition(seekRequest.seconds);
      setResumedAt(seekRequest.seconds);
      return;
    }
    const video = videoRef.current;
    if (!video || !Number.isFinite(video.duration)) return;
    const target = Math.min(Math.max(0, video.duration - 0.25), seekRequest.seconds);
    video.currentTime = target;
    lastSavedPosition.current = target;
    setCurrentPosition(target);
    setResumedAt(target);
    void video.play().catch(() => {});
  }, [embeddedMpvSession?.itemId, item.id, seekRequest?.id, session?.liveTranscode]);

  const nativeControlClass = "grid size-8 shrink-0 place-items-center rounded-[8px] text-white/65 transition-[background-color,color,transform] duration-150 hover:bg-white/10 hover:text-white active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75 disabled:pointer-events-none disabled:opacity-40";

  return <><section ref={playerRef} className={`cinematic-player panel-shadow min-w-0 overflow-hidden border border-white/10 text-white ${compact ? "rounded-[10px] shadow-[0_28px_80px_rgb(0_0_0/0.58)]" : leanBack ? "flex min-h-0 flex-1 flex-col rounded-[16px] shadow-[0_30px_120px_rgb(0_0_0/0.8)]" : `flex flex-col rounded-[12px] ${displayPanelOpen ? "" : "max-h-[calc(100dvh-96px)]"}`}`} aria-label={`${leanBack ? "Lean-back player" : compact ? "Mini player" : "Playing"} ${item.title}`}>
    <header className={`flex items-center border-b border-white/10 bg-[linear-gradient(180deg,rgba(20,18,14,0.97),rgba(10,9,7,0.96))] shadow-[inset_0_1px_0_rgba(255,255,255,0.035)] backdrop-blur-xl ${compact ? "min-h-11 gap-1.5 px-2.5 py-1.5" : leanBack ? "min-h-14 gap-3 px-4 py-2" : "min-h-16 gap-3 px-4 py-2.5"}`}>
      {compact ? <button className="flex min-w-0 flex-1 touch-none items-center gap-2 rounded-[7px] px-1.5 py-1 text-left text-white/65 transition hover:bg-white/[0.065] hover:text-white active:cursor-grabbing focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75" style={{ cursor: "grab" }} onPointerDown={onMiniPlayerMoveStart} onKeyDown={onMiniPlayerMoveKey} onDoubleClick={onMiniPlayerReset} aria-label={`Move mini player for ${item.title}. Use arrow keys to move it; hold Shift for larger steps. Double-click to reset.`} title="Drag to move · Arrow keys move · Double-click resets"><GripHorizontal className="shrink-0 text-white/35" size={14} /><span className="truncate text-[11px] font-semibold">{item.title}</span></button> : <div className="min-w-0 flex-1"><h1 className="truncate text-sm font-semibold">{item.title}</h1><p className="mt-0.5 truncate text-[11px] text-white/55">{sourceName}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}{item.width && item.height ? ` · ${item.width} × ${item.height}` : ""}</p></div>}
      {embeddedMpvStatus?.available && !embeddedMpvSession ? <Button variant="ghost" className={`${compact ? "size-8 px-0" : ""} text-accent hover:text-accent`} disabled={embeddedMpvBusy} onClick={() => void playWithEmbeddedMpv()} aria-label="Use native libmpv player"><MonitorPlay size={15} />{compact ? null : " Use native player"}</Button> : null}
      {embeddedMpvSession ? <div className={`flex shrink-0 items-center ${compact ? "gap-1" : "gap-2"}`} aria-label="Native libmpv controls">
        {!compact ? <span className="inline-flex h-8 items-center gap-1.5 rounded-[8px] border border-accent/20 bg-accent/[0.08] px-2 text-[10px] font-semibold text-accent shadow-[inset_0_1px_0_rgba(255,255,255,0.04)]"><span className="size-1.5 rounded-full bg-accent shadow-[0_0_8px_rgb(var(--accent)/0.65)]" /> Native</span> : null}
        {!compact ? <Select className="h-9 w-[72px] rounded-[9px] border-white/10 bg-white/[0.055] px-2 text-[11px] font-semibold text-white/75 shadow-[inset_0_1px_0_rgba(255,255,255,0.035)] hover:border-white/20 focus:border-accent/50 focus:bg-white/[0.08]" value={playbackRate} aria-label="Playback speed" title="Playback speed" onChange={(event) => { const rate = Number(event.target.value); setPlaybackRate(rate); void api.controlEmbeddedMpv({ action: "setRate", value: rate }).catch((error) => setError(normalizeError(error))); }}><option value={0.5}>0.5×</option><option value={0.75}>0.75×</option><option value={1}>1×</option><option value={1.25}>1.25×</option><option value={1.5}>1.5×</option><option value={2}>2×</option></Select> : null}
        <button className={`inline-flex h-9 shrink-0 items-center justify-center gap-1.5 rounded-[9px] border border-white/10 bg-white/[0.045] text-[11px] font-semibold text-white/65 transition-[background-color,border-color,color,transform] duration-150 hover:border-white/20 hover:bg-white/[0.09] hover:text-white active:scale-[0.97] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/75 ${compact ? "w-9" : "px-2.5"}`} title="Switch to the WebView player" disabled={embeddedMpvBusy} onClick={() => void closeEmbeddedMpvPlayback()} aria-label="Switch to WebView player"><MonitorPlay size={14} strokeWidth={1.9} />{compact ? null : <span>Web player</span>}</button>
      </div> : null}
      {!compact && !leanBack && (embeddedMpvSession || session) && !playbackError && !nativeSession ? <button className={`${nativeControlClass} ${displayPanelOpen ? "bg-white/10 text-accent" : ""}`} onClick={() => setDisplayPanelOpen((open) => !open)} aria-expanded={displayPanelOpen} aria-controls="video-display-panel" aria-label="Video fit and appearance" title="Video fit and appearance"><Settings2 size={16} /></button> : null}
      <Button variant="ghost" className={`${compact ? "size-8 px-0" : "h-9 rounded-[9px] border-white/10 bg-white/[0.035] px-3 text-[11px]"} shrink-0 text-white/65 hover:border-white/20 hover:bg-white/[0.08] hover:text-white`} onClick={() => void openExternal()} aria-label="Open in system player"><ExternalLink size={14} strokeWidth={1.9} />{compact ? null : " System player"}</Button>
      {compact ? <button className={`${nativeControlClass} touch-none cursor-ew-resize`} onPointerDown={onMiniPlayerResizeStart} onKeyDown={onMiniPlayerResizeKey} aria-label="Resize mini player. Drag horizontally or use arrow keys; hold Shift for larger steps." title="Drag horizontally to resize · Arrow keys resize"><Scaling size={15} /></button> : null}
      {compact ? <button className={nativeControlClass} onClick={onRestore} aria-label="Return to full watch page" title="Restore full player"><Maximize size={15} /></button> : null}
      {compact ? <button className={nativeControlClass} onClick={closePlayer} aria-label="Close mini player" title="Close player"><X size={16} /></button> : null}
    </header>
    <div ref={playbackSurfaceRef} className={`relative flex min-h-0 items-center justify-center overflow-hidden bg-black ${!compact && effectiveDisplaySettings.zoom > 1 ? "touch-none cursor-grab active:cursor-grabbing" : ""} ${compact ? "aspect-video" : leanBack ? "flex-1" : "aspect-video min-h-[240px] max-h-[calc(100dvh-250px)]"}`} onPointerDown={beginDisplayPan} onPointerMove={updateDisplayPan} onPointerUp={endDisplayPan} onPointerCancel={endDisplayPan}>
      {loading && !embeddedMpvSession ? <div className="absolute inset-0 z-[1] flex items-center justify-center gap-2 bg-black/80 text-sm text-white/60" role="status"><LoaderCircle className="animate-spin text-accent" size={18} /> {playbackRenderer === "native" ? "Starting native playback…" : "Preparing web playback…"}</div> : null}
      {embeddedMpvSession ? <div className="sr-only" role="status">Playing {item.title} with native libmpv inside Nice Downloader.</div> : null}
      {nativeSession ? <div className="max-w-lg border border-accent/25 bg-accent/5 p-6 text-center"><MonitorPlay className="mx-auto text-accent" size={36} /><h2 className="mt-4 text-lg font-semibold">Playing with native mpv</h2><p className="mt-2 text-sm leading-6 text-white/60">mpv is decoding this video in its native window without creating a compatibility copy.</p>{nativeSession.resumedAtSeconds > 0 ? <p className="mt-2 text-xs text-accent">{seekRequest?.itemId === item.id ? "Started from transcript" : "Resumed"} at {formatDuration(nativeSession.resumedAtSeconds)}</p> : null}<div className="mt-5 flex flex-wrap justify-center gap-2"><Button variant="secondary" disabled={nativeBusy} onClick={() => void closeNativePlayback()}>{nativeBusy ? <LoaderCircle className="animate-spin" size={15} /> : <Square size={15} />} Stop native player</Button><Button onClick={() => void openExternal()}><ExternalLink size={15} /> System player</Button></div></div> : null}
        {playbackError && !nativeSession && !embeddedMpvSession ? <div className="max-w-lg border border-white/10 bg-white/5 p-6 text-center"><Film className="mx-auto text-white/35" size={34} /><h2 className="mt-4 text-lg font-semibold">This video could not be played here</h2><p className="mt-2 text-sm leading-6 text-white/55">{playbackError.message}</p><div className="mt-5 flex flex-wrap justify-center gap-2">{embeddedMpvStatus?.available ? <Button disabled={embeddedMpvBusy} onClick={() => void playWithEmbeddedMpv()}>{embeddedMpvBusy ? <LoaderCircle className="animate-spin" size={15} /> : <MonitorPlay size={15} />} Play here with libmpv</Button> : null}{nativeStatus?.available ? <Button variant={embeddedMpvStatus?.available ? "secondary" : "primary"} disabled={nativeBusy} onClick={() => void playWithNativeMpv()}>{nativeBusy ? <LoaderCircle className="animate-spin" size={15} /> : <MonitorPlay size={15} />} Open native mpv window</Button> : null}{canOfferPlaybackFallback(playbackError) && !session?.compatibilityTranscoded ? <Button variant="secondary" onClick={() => void retryEmbeddedPlayback()}><RotateCcw size={15} /> Create H.264 playback copy</Button> : null}<Button variant={nativeStatus?.available || embeddedMpvStatus?.available ? "secondary" : "primary"} onClick={() => void openExternal()}><ExternalLink size={15} /> Open in system player</Button></div></div> : null}
        {session && !playbackError && !nativeSession && !embeddedMpvSession ? <video
        ref={videoRef}
        className="h-full w-full max-h-full max-w-full bg-black shadow-[0_30px_100px_rgb(0_0_0/0.65)] transition-transform duration-150 motion-reduce:transition-none"
        style={subtitleVideoStyle}
        crossOrigin="anonymous"
        src={session.url}
        controls={false}
        playsInline
        preload="metadata"
        onLoadedMetadata={onLoadedMetadata}
        onPlay={() => { setIsPlaying(true); startPlaybackWatchdog(); }}
        onPlaying={() => { setIsPlaying(true); clearPlaybackWatchdog(); }}
        onWaiting={armPlaybackWatchdogWhilePlaying}
        onStalled={armPlaybackWatchdogWhilePlaying}
        onTimeUpdate={() => { const video = videoRef.current; if (!video) return; notePlaybackProgress(video); const position = logicalVideoPosition(video); setCurrentPosition(position); if (Math.abs(position - lastSavedPosition.current) >= 5) persistProgress(false); }}
        onPause={() => { setIsPlaying(false); clearPlaybackWatchdog(); persistProgress(false); }}
        onEnded={() => { recordDirectPlaybackEvidence("played"); setIsPlaying(false); clearPlaybackWatchdog(); persistProgress(true); window.setTimeout(() => onPlaybackEnded?.(), 0); }}
        onError={() => { const video = videoRef.current; recordDirectPlaybackEvidence("failed"); clearPlaybackWatchdog(); if (advanceCompatibilitySource(video, true)) return; setLoading(false); setPlaybackError(createPlaybackError("embedded_playback_decode_failed", session.liveTranscode ? "The available H.264/AAC compatibility streams failed. Try libmpv, a cached playback copy, or your system player." : "The WebView does not support this file's container or codecs. The original file is unchanged and can still be opened with your system player.")); }}
      >
        {subtitleTracks.map((track, index) => <track key={`${track.label}:${index}:${session.streamStartSeconds}`} kind="subtitles" src={track.url} srcLang={track.language || "und"} label={track.label} default={index === selectedSubtitleIndex} onLoad={onSubtitleTrackLoaded} />)}
      </video> : null}
    </div>
    {(embeddedMpvSession || session) && !playbackError && !nativeSession ? <PlaybackPreviewTimeline currentTime={currentPosition} duration={playbackDuration} pendingTime={pendingNativeSeek} compact={compact} isPlaying={isPlaying} muted={playbackMuted} volume={playbackVolume} subtitlesAvailable={Boolean(session && !embeddedMpvSession)} subtitlesOpen={subtitleCenterOpen} storyboard={previewStoryboard} storyboardBusy={previewStoryboardBusy} storyboardUnavailable={previewStoryboardUnavailable} onPreviewRequested={requestPreviewStoryboard} onPendingTime={setPendingNativeSeek} onCommit={commitNativeSeek} onTogglePlayback={togglePlayback} onSeekRelative={seek} onToggleMuted={toggleMuted} onVolumeChange={changeVolume} onToggleSubtitles={session && !embeddedMpvSession ? toggleSubtitleCenter : undefined} onToggleFullscreen={toggleFullscreen} /> : null}
    {!compact && !leanBack && (embeddedMpvSession || session) && !playbackError && !nativeSession ? <PlaybackTools currentTime={currentPosition} loopStart={loopStart} loopEnd={loopEnd} frameStepDisabled={Boolean(session?.liveTranscode)} screenshotBusy={screenshotBusy} onSetLoopStart={setLoopStartAtCurrentPosition} onSetLoopEnd={setLoopEndAtCurrentPosition} onClearLoop={clearLoop} onFrameStep={stepFrame} onScreenshot={() => void captureFrame()} /> : null}
    {displayPanelOpen && !compact && !leanBack && (embeddedMpvSession || session) && !playbackError && !nativeSession ? <VideoDisplayPanel settings={displaySettings} remembered={displayRemembered} rememberBusy={displayRememberBusy} renderer={embeddedMpvSession ? "libmpv" : "WebView"} onChange={updateDisplaySettings} onReset={() => setDisplaySettings(DEFAULT_VIDEO_DISPLAY_SETTINGS)} onRemember={(remember) => void toggleRememberDisplaySettings(remember)} onClose={() => setDisplayPanelOpen(false)} /> : null}
    {subtitleCenterOpen && session && !embeddedMpvSession ? <section id="subtitle-center" className={`border-t border-white/10 bg-[linear-gradient(180deg,#15120e,#0d0c09)] ${compact ? "p-3" : "px-5 py-4"}`} aria-labelledby="subtitle-center-title">
      <div className="flex items-start gap-3"><div className="grid size-8 shrink-0 place-items-center rounded-[7px] bg-accent/10 text-accent"><Captions size={15} /></div><div className="min-w-0 flex-1"><h2 id="subtitle-center-title" className="text-xs font-semibold text-white/85">Subtitle Center</h2>{!compact ? <p className="mt-1 text-[10px] leading-4 text-white/40">Choose a local sidecar track and tune its timing and appearance. The original subtitle file is never modified.</p> : null}</div><button className={nativeControlClass} onClick={() => setSubtitleCenterOpen(false)} aria-label="Close Subtitle Center"><X size={14} /></button></div>
      <div className={`mt-3 grid gap-3 ${compact ? "grid-cols-2" : "sm:grid-cols-2 lg:grid-cols-5"}`}>
        <label className={compact ? "col-span-2" : "lg:col-span-2"}><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Track</span><Select className="mt-1 h-9 border-white/10 bg-white/5 text-xs text-white" value={selectedSubtitleIndex} onChange={(event) => setSelectedSubtitleIndex(Number(event.target.value))}><option value={-1}>Off</option>{subtitleTracks.map((track, index) => <option key={`${track.label}:${index}`} value={index}>{track.label}{track.language ? ` · ${track.language.toLocaleUpperCase()}` : ""}</option>)}</Select>{embeddedSubtitleBusy ? <span className="mt-1 flex items-center gap-1 text-[9px] text-white/40"><LoaderCircle className="animate-spin" size={10} /> Inspecting embedded tracks…</span> : embeddedSubtitleMessage ? <span className="mt-1 block text-[9px] leading-4 text-white/35">{embeddedSubtitleMessage}</span> : null}</label>
        <label><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Sync offset</span><div className="mt-1 flex"><Input className="h-9 rounded-r-none border-white/10 bg-white/5 text-xs text-white" type="number" min={-10} max={10} step={0.25} value={subtitleOffsetSeconds} onChange={(event) => setSubtitleOffsetSeconds(Math.min(10, Math.max(-10, Number(event.target.value) || 0)))} /><span className="grid h-9 place-items-center rounded-r-[8px] border border-l-0 border-white/10 bg-white/5 px-2 text-[9px] text-white/40">sec</span></div></label>
        <label><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Size</span><Select className="mt-1 h-9 border-white/10 bg-white/5 text-xs text-white" value={subtitleSizePercent} onChange={(event) => setSubtitleSizePercent(Number(event.target.value))}><option value={80}>Small</option><option value={100}>Standard</option><option value={125}>Large</option><option value={150}>Extra large</option></Select></label>
        <label><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Position</span><Select className="mt-1 h-9 border-white/10 bg-white/5 text-xs text-white" value={subtitlePosition} onChange={(event) => setSubtitlePosition(event.target.value as SubtitlePosition)}><option value="bottom">Bottom</option><option value="middle">Middle</option><option value="top">Top</option></Select></label>
        <label><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Text</span><Select className="mt-1 h-9 border-white/10 bg-white/5 text-xs text-white" value={subtitleColor} onChange={(event) => setSubtitleColor(event.target.value as SubtitleColor)}><option value="white">Ivory</option><option value="yellow">Warm yellow</option></Select></label>
        <label><span className="text-[9px] font-semibold uppercase tracking-[0.1em] text-white/40">Background</span><Select className="mt-1 h-9 border-white/10 bg-white/5 text-xs text-white" value={subtitleBackground} onChange={(event) => setSubtitleBackground(event.target.value as SubtitleBackground)}><option value="none">None</option><option value="soft">Soft</option><option value="strong">Strong</option></Select></label>
        <div className="flex items-end"><Button variant="ghost" className="h-9 w-full text-[10px] text-white/55" onClick={() => { setSubtitleOffsetSeconds(0); setSubtitleSizePercent(100); setSubtitlePosition("bottom"); setSubtitleColor("white"); setSubtitleBackground("soft"); }}>Reset tuning</Button></div>
      </div>
    </section> : null}
    {!compact && !leanBack ? <PlaybackDiagnostics item={item} session={session} preflight={preflight} nativeSession={nativeSession} embeddedMpvSession={embeddedMpvSession} subtitleCount={subtitleTracks.length} error={playbackError} /> : null}
    <footer className={`${compact ? "hidden" : "flex"} flex-wrap items-center gap-3 border-t border-white/10 bg-[#100e0b] ${leanBack ? "justify-center px-5 py-4" : "px-5 py-3"}`}>
      {leanBack ? <><Button variant="ghost" className="h-14 min-w-32 text-base text-white/75" disabled={!onPrevious} onClick={() => changeVideo(onPrevious)}><SkipBack size={21} /> Previous</Button><Button className="h-14 min-w-44 text-base" disabled={(!videoRef.current && !embeddedMpvSession) || Boolean(playbackError) || Boolean(nativeSession)} onClick={togglePlayback}>{isPlaying ? <Pause size={22} /> : <Play size={22} fill="currentColor" />} {isPlaying ? "Pause" : "Play"}</Button><Button variant="ghost" className="h-14 min-w-32 text-base text-white/75" disabled={!onNext} onClick={() => changeVideo(onNext)}>Next <SkipForward size={21} /></Button><Button variant="secondary" className="h-14 min-w-28 border-white/10 bg-white/10 text-white" onClick={onExitLeanBack}><Minimize2 size={19} /> Exit</Button><p className="w-full text-center text-[11px] text-white/45">Gamepad: A/Start play · B exit · D-pad browse · LB/RB seek · Keyboard: Space/K · J/L · M · F · Esc</p></> : <><Button variant="ghost" className="h-9 text-white/70" disabled={!onPrevious} onClick={() => changeVideo(onPrevious)}><ChevronRight className="rotate-180" size={15} /> Previous</Button><Button variant="ghost" className="h-9 text-white/70" disabled={!onNext} onClick={() => changeVideo(onNext)}>Next <ChevronRight size={15} /></Button><span className="h-5 w-px bg-white/10" /><label className="flex items-center gap-2 text-xs text-white/55"><span>Speed</span><Select className="h-8 w-24 border-white/10 bg-white/5 text-xs text-white" value={playbackRate} onChange={(event) => { const rate = Number(event.target.value); setPlaybackRate(rate); if (embeddedMpvSession) void api.controlEmbeddedMpv({ action: "setRate", value: rate }).catch((error) => setError(normalizeError(error))); else if (videoRef.current) videoRef.current.playbackRate = rate; }}><option value={0.5}>0.5×</option><option value={0.75}>0.75×</option><option value={1}>1×</option><option value={1.25}>1.25×</option><option value={1.5}>1.5×</option><option value={2}>2×</option></Select></label><p className="ml-auto text-[10px] text-white/40">Space/K play · J/L seek · M mute · F fullscreen · Esc close</p></>}
      {resumedAt != null ? <p className="w-full text-[10px] text-accent" role="status">{seekRequest?.itemId === item.id ? "Started from transcript" : "Resumed"} at {formatDuration(resumedAt)}</p> : null}
      {session?.compatibilityMode ? <p className="w-full text-[10px] text-accent" role="status">{session.compatibilityTranscoded ? "Using an H.264/AAC embedded-playback copy." : preflight?.compatibilityCached ? "Using an existing cached compatibility copy. AV1 timing is optimized for the embedded player." : "A lossless AV1 timing copy was created for this playback."} The original is unchanged.</p> : null}
      {session?.liveTranscode ? <p className="w-full text-[10px] text-accent" role="status">{session.segmentedStream ? "Using temporary HLS segments for resilient H.264/AAC playback." : "Streaming a fragmented H.264/AAC compatibility feed without a completed copy."} The original is unchanged.</p> : null}
      {nativeFallbackMessage && session ? <p className="w-full text-[10px] text-white/45" role="status">Native libmpv was unavailable for this playback. Using the web player. {nativeFallbackMessage}</p> : null}
    </footer>
  </section>{!compact && !leanBack ? <CreativeTimeline item={item} currentTime={currentPosition} duration={playbackDuration} playbackReady={Boolean((session || embeddedMpvSession) && !playbackError)} onSeek={seekTo} /> : null}{consent ? <div className="fixed inset-0 z-[90] grid place-items-center bg-black/75 p-5 backdrop-blur-sm" onMouseDown={dismissConsent}><section ref={consentDialogRef} className="panel-shadow w-full max-w-xl overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="compatibility-copy-title" aria-describedby="compatibility-copy-description" aria-busy={consentBusy || nativeBusy} onKeyDown={trapConsentFocus} onMouseDown={(event) => event.stopPropagation()}><header className="flex items-start gap-4 border-b border-line/70 px-5 py-5"><div className="grid size-11 shrink-0 place-items-center rounded-[10px] border border-accent/20 bg-accent/10 text-accent"><Copy size={20} /></div><div><p className="text-[10px] font-semibold uppercase tracking-[0.12em] text-accent">Embedded playback compatibility</p><h2 id="compatibility-copy-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">{consent.reencodeRequired ? "Create an H.264 playback copy?" : "Create a temporary playback copy?"}</h2><p id="compatibility-copy-description" className="mt-2 text-xs leading-5 text-muted">{consent.reason}</p></div></header><div className="space-y-4 px-5 py-5"><dl className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-5 gap-y-2.5 border-y border-line/60 py-4 text-xs"><dt className="text-muted">Estimated copy size</dt><dd className="number-font font-medium text-ink">{formatBytes(consent.estimatedCopyBytes)}</dd><dt className="text-muted">Free space available</dt><dd className="number-font font-medium text-ink">{consent.availableSpaceBytes != null ? formatBytes(consent.availableSpaceBytes) : "Unknown"}</dd><dt className="text-muted">Conversion</dt><dd className="font-medium text-ink">{consent.reencodeRequired ? "H.264/AAC · video re-encode" : "Stream-copy · no re-encoding"}</dd></dl>{consent.availableSpaceBytes != null && consent.estimatedCopyBytes > consent.availableSpaceBytes ? <p className="border-l-2 border-red-400/70 bg-red-400/5 px-3 py-2 text-xs leading-5 text-red-300" role="alert">There may not be enough free space to create this copy.</p> : null}{consent.unusuallyLarge ? <p className="border-l-2 border-amber-400/70 bg-amber-400/5 px-3 py-2 text-xs leading-5 text-amber-200">This is an unusually large copy, so Nice Downloader always asks before creating it—even when automatic copies are enabled.</p> : null}<div className="rounded-[9px] bg-elevated/55 p-3 text-[11px] leading-5 text-muted"><p><strong className="font-semibold text-ink">Your original stays untouched.</strong> The copy exists only in Nice Downloader’s private playback cache.</p><p className="mt-1">The oldest copies are removed automatically before the cache exceeds 4 files or 8 GiB. You can review or clear the cache in Settings.</p></div>{!consent.reencodeRequired && consent.preference === "never" ? <p className="text-[11px] leading-5 text-muted">Your saved preference is <strong className="text-ink">Never create automatically</strong>. “Create copy and play” is a one-time override unless you enable the option below.</p> : null}{!consent.reencodeRequired ? <label className="flex items-start gap-3 text-xs text-muted"><input className="mt-0.5" type="checkbox" checked={rememberAutomatic} onChange={(event) => setRememberAutomatic(event.target.checked)} /><span><strong className="block font-medium text-ink">Automatically create compatible copies in the future</strong>Large copies will still require confirmation.</span></label> : <p className="text-[11px] leading-5 text-muted">This fallback is never created automatically because it changes the cached copy’s video codec. The original file and its quality remain unchanged.</p>}{nativeStatus?.available ? <div className="rounded-[9px] border border-accent/20 bg-accent/5 p-3 text-[11px] leading-5 text-muted"><p className="flex items-center gap-2 font-medium text-ink"><MonitorPlay className="text-accent" size={15} /> Native mpv is available</p><p className="mt-1">Play the original directly with FFmpeg decoders and create no compatibility copy.</p></div> : null}{copyPreparationBusy ? <PlaybackPreparationStatus preparation={preparation} reencodeRequired={consent.reencodeRequired} cancelling={cancelBusy || cancelRequested.current} /> : null}</div><footer className="flex flex-wrap items-center justify-end gap-2 border-t border-line/70 bg-elevated/25 px-5 py-4"><Button data-consent-cancel variant="ghost" disabled={cancelBusy || (copyPreparationBusy && !preparation)} onClick={copyPreparationBusy ? () => void cancelCompatibilityCopy() : dismissConsent}>{copyPreparationBusy ? cancelBusy ? "Cancelling…" : "Cancel copy" : "Cancel"}</Button><Button variant="secondary" disabled={consentBusy || nativeBusy} onClick={() => void playWithNativeMpv()}>{nativeBusy ? <LoaderCircle className="animate-spin" size={15} /> : <MonitorPlay size={15} />} Play with native mpv</Button><Button variant="secondary" disabled={consentBusy || nativeBusy} onClick={() => void openOriginalFromConsent()}><ExternalLink size={15} /> Open original in system player</Button><Button disabled={consentBusy || nativeBusy} onClick={() => void approveCompatibilityCopy()}>{consentBusy ? <LoaderCircle className="animate-spin" size={15} /> : <Copy size={15} />} {consentBusy && consent.reencodeRequired ? "Encoding…" : consent.reencodeRequired ? "Create H.264 copy and play" : "Create copy and play"}</Button></footer></section></div> : null}</>;
}

function MediaDetails({ item, sourceName, onClose, onWatch, onChanged, onMetadataChanged, onBrowseCreator, onBrowseSeries, onBrowseSite, onSelectRelated, onRemoved, onOperation }: { item: LibraryItem; sourceName: string; onClose: () => void; onWatch: () => void; onChanged: (item: LibraryItem) => void; onMetadataChanged: () => void; onBrowseCreator: (creator: string) => void; onBrowseSeries: (series: string) => void; onBrowseSite: (site: string) => void; onSelectRelated: (item: LibraryItem) => void; onRemoved: () => void; onOperation: (result: LibraryFileOperationResult) => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const setView = useAppStore((state) => state.setView);
  const [editing, setEditing] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [title, setTitle] = useState(item.title);
  const [tags, setTags] = useState(item.tags.join(", "));
  const [richMetadata, setRichMetadata] = useState<LibraryRichMetadata | null>(null);
  const [relatedItems, setRelatedItems] = useState<LibraryItem[]>([]);
  const [relatedLoading, setRelatedLoading] = useState(false);
  const [playbackPreflight, setPlaybackPreflight] = useState<LibraryPlaybackPreflight | null>(null);
  const [watchLaterSaved, setWatchLaterSaved] = useState<boolean | null>(null);
  const [watchLaterBusy, setWatchLaterBusy] = useState(false);
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
    setPlaybackPreflight(null);
    setWatchLaterSaved(null);
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
    if (item.mediaKind === "video" && !item.missing) {
      void api.getLibraryWatchState(item.id)
        .then((state) => { if (!disposed) setWatchLaterSaved(state.watchLater); })
        .catch(() => { if (!disposed) setWatchLaterSaved(false); });
      void api.inspectLibraryPlayback(item.id)
        .then((preflight) => { if (!disposed) setPlaybackPreflight(preflight); })
        .catch(() => { if (!disposed) setPlaybackPreflight(null); });
    }
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

  async function toggleWatchLater() {
    if (watchLaterBusy || item.mediaKind !== "video") return;
    setWatchLaterBusy(true);
    try {
      const state = await api.setLibraryWatchLater(item.id, !watchLaterSaved);
      setWatchLaterSaved(state.watchLater);
      setNotice({ message: state.watchLater ? "Saved to Watch Later." : "Removed from Watch Later.", view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setWatchLaterBusy(false);
    }
  }

  function openContextualTool(view: "similarity" | "sharing", eventName: "nice-downloader:similarity-reference" | "nice-downloader:sharing-select") {
    const itemId = item.id;
    onClose();
    setView(view);
    window.setTimeout(() => window.dispatchEvent(new CustomEvent(eventName, { detail: { itemId } })), 0);
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

  return <aside className="panel-shadow fixed bottom-3 right-3 top-3 z-50 w-[min(680px,calc(100vw-24px))] overflow-y-auto rounded-media border border-line/70 bg-panel/98 backdrop-blur-xl" role="dialog" aria-modal="true" aria-label={`${item.title} details`} onKeyDown={trapFocus}><div className="sticky top-0 z-[1] flex items-center justify-between rounded-t-media border-b border-line/60 bg-panel/90 px-5 py-4 backdrop-blur-xl"><div><p className="text-xs font-semibold">Media details</p><p className="mt-0.5 text-[10px] text-muted">Use ← → to browse · Esc to close</p></div><button ref={closeButtonRef} className="grid size-9 place-items-center rounded-control text-muted transition hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={onClose} aria-label="Close media details"><X size={17} /></button></div><div className="p-4 sm:p-5"><div className="media-shadow aspect-video overflow-hidden rounded-media bg-elevated"><MediaArtwork item={item} eager /></div>{editing ? <div className="mt-5 space-y-3"><Field label="Title"><Input value={title} onChange={(event) => setTitle(event.target.value)} /></Field><Field label="Tags"><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="Comma-separated tags" /></Field><div className="flex gap-2"><Button onClick={() => void update()}>Save changes</Button><Button variant="secondary" onClick={() => setEditing(false)}>Cancel</Button></div></div> : <><h2 className="mt-5 text-[24px] font-semibold leading-[1.2] tracking-[-0.035em] text-balance">{item.title}</h2><p className="mt-2 text-xs text-muted">{sourceName}{item.durationSeconds != null ? ` · ${formatDuration(item.durationSeconds)}` : ""}</p>{item.mediaKind === "video" ? <details className="mt-3 rounded-control bg-elevated/35 px-3 py-2"><summary className="cursor-pointer text-[11px] font-medium text-muted">Embedded playback compatibility</summary><div className="mt-2"><EmbeddedCompatibilityLabel item={item} preflight={playbackPreflight} /></div></details> : null}<div className="mt-5 flex flex-wrap gap-2 rounded-panel bg-elevated/35 p-2">{item.mediaKind === "video" ? <Button className="h-11 min-w-40 flex-1" disabled={item.missing} onClick={onWatch}><Play size={16} fill="currentColor" /> Watch here</Button> : <Button className="h-11 min-w-40 flex-1" disabled={item.missing} onClick={onWatch}><FileImage size={16} /> View image</Button>}<Button variant="secondary" className="h-11" disabled={item.missing} onClick={() => void openItem()}><ExternalLink size={15} /> {item.mediaKind === "image" ? "System viewer" : "System player"}</Button><Button variant="ghost" className="h-11 px-3" onClick={() => setEditing(true)}><Settings2 size={14} /> Edit</Button></div></>}
      <div className="mt-3 flex flex-wrap items-center gap-2 rounded-panel border border-line/45 bg-canvas/35 p-2">{item.mediaKind === "video" ? <Button variant="secondary" disabled={watchLaterBusy || watchLaterSaved == null} onClick={() => void toggleWatchLater()} aria-pressed={watchLaterSaved ?? undefined}>{watchLaterBusy ? <LoaderCircle className="animate-spin" size={15} /> : <Bookmark size={15} fill={watchLaterSaved ? "currentColor" : "none"} />} {watchLaterSaved ? "Saved for later" : "Watch later"}</Button> : null}<Button variant="secondary" className={item.favorite ? "text-rose-300" : ""} onClick={() => void updateCuration(!item.favorite, item.rating)}><Heart size={15} fill={item.favorite ? "currentColor" : "none"} /> {item.favorite ? "Favorited" : "Favorite"}</Button><label className="flex min-w-40 flex-1 items-center gap-2 text-xs text-muted"><Star size={15} /><span className="sr-only">Rating</span><Select className="h-10" value={item.rating ?? ""} onChange={(event) => void updateCuration(item.favorite, event.target.value ? Number(event.target.value) : null)}><option value="">Not rated</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}</Select></label></div>
      <section className="mt-4 rounded-panel border border-line/45 bg-elevated/25 p-4" aria-labelledby="rich-metadata-title">
        <div className="flex items-start justify-between gap-3"><div><h3 id="rich-metadata-title" className="text-sm font-semibold">Media information</h3><p className="mt-1 text-[11px] leading-5 text-muted">Imported from yt-dlp sidecars. Your edits stay in Nice Downloader and survive rescans.</p></div><Button variant="ghost" className="h-8 text-xs" disabled={!richMetadata || metadataBusy} onClick={() => { if (richMetadata) loadMetadataForm(richMetadata); setMetadataEditing((value) => !value); }}>{metadataEditing ? "Cancel" : "Edit metadata"}</Button></div>
        {metadataEditing ? <div className="mt-4 space-y-3"><Field label="Creator or channel"><Input value={creator} maxLength={200} onChange={(event) => setCreator(event.target.value)} /></Field><Field label="Description"><Textarea className="min-h-28" value={description} maxLength={10000} onChange={(event) => setDescription(event.target.value)} /></Field><div className="grid gap-3 sm:grid-cols-2"><Field label="Upload date"><Input type="date" value={uploadDate} onChange={(event) => setUploadDate(event.target.value)} /></Field><Field label="Series"><Input value={series} maxLength={200} onChange={(event) => setSeries(event.target.value)} /></Field><Field label="Episode"><Input value={episode} maxLength={200} onChange={(event) => setEpisode(event.target.value)} /></Field><Field label="Original URL"><Input type="url" value={originalUrl} maxLength={2000} onChange={(event) => setOriginalUrl(event.target.value)} /></Field></div><div className="flex flex-wrap gap-2"><Button disabled={metadataBusy} onClick={() => void saveRichMetadata()}>{metadataBusy ? <LoaderCircle className="animate-spin" size={14} /> : <Save size={14} />} Save metadata</Button>{richMetadata && richMetadata.overriddenFields.length > 0 ? <Button variant="secondary" disabled={metadataBusy} onClick={() => void resetRichMetadata()}><RotateCcw size={14} /> Use extracted values</Button> : null}</div></div> : richMetadata ? <div className="mt-4"><dl className="grid grid-cols-[92px_1fr] gap-x-3 gap-y-2 text-xs">{richMetadata.effective.creator ? <><dt className="text-muted">Creator</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseCreator(richMetadata.effective.creator!)}>{richMetadata.effective.creator}</button></dd></> : null}{richMetadata.effective.uploadDate ? <><dt className="text-muted">Published</dt><dd>{new Date(`${richMetadata.effective.uploadDate}T00:00:00`).toLocaleDateString()}</dd></> : null}{richMetadata.effective.series ? <><dt className="text-muted">Series</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseSeries(richMetadata.effective.series!)}>{richMetadata.effective.series}</button>{richMetadata.effective.episode ? ` · ${richMetadata.effective.episode}` : ""}</dd></> : richMetadata.effective.episode ? <><dt className="text-muted">Episode</dt><dd>{richMetadata.effective.episode}</dd></> : null}{richMetadata.effective.originalUrl ? <><dt className="text-muted">Original</dt><dd className="break-all text-muted" title={richMetadata.effective.originalUrl}>{richMetadata.effective.originalUrl}</dd></> : null}</dl>{richMetadata.effective.description ? <p className="mt-4 whitespace-pre-wrap text-xs leading-5 text-muted">{richMetadata.effective.description}</p> : null}{richMetadata.overriddenFields.length > 0 ? <p className="mt-3 text-[10px] text-accent">User overrides: {richMetadata.overriddenFields.join(", ")}</p> : <p className="mt-3 text-[10px] text-muted">Extracted metadata · no user overrides</p>}</div> : <p className="mt-4 flex items-center gap-2 text-xs text-muted" role="status"><LoaderCircle className="animate-spin" size={13} /> Loading metadata…</p>}
        <div className="mt-5 flex flex-wrap gap-2 border-t border-line/50 pt-4"><Button variant="secondary" className="h-8 text-xs" onClick={() => void chooseArtwork()}><ImageIcon size={13} /> Choose artwork</Button>{item.thumbnail ? <Button variant="ghost" className="h-8 text-xs" onClick={() => void clearArtwork()}><RotateCcw size={13} /> Use generated artwork</Button> : null}</div>
      </section>
      <details className="mt-4 rounded-panel border border-line/45 bg-elevated/20 p-4"><summary className="cursor-pointer text-sm font-semibold">More ways to use this media</summary><p className="mt-2 text-[12px] leading-5 text-muted">Open a specialized local tool with this item already selected.</p><div className="mt-3 flex flex-wrap gap-2"><Button variant="secondary" className="h-9 text-xs" onClick={() => openContextualTool("similarity", "nice-downloader:similarity-reference")}><ScanSearch size={14} /> Find similar</Button><Button variant="secondary" className="h-9 text-xs" onClick={() => openContextualTool("sharing", "nice-downloader:sharing-select")}><Share2 size={14} /> Share temporarily</Button></div></details>
      {relatedLoading || relatedItems.length > 0 ? <section className="mt-4 rounded-panel border border-line/45 bg-elevated/25 p-4" aria-labelledby="related-media-title"><div className="flex items-center justify-between"><h3 id="related-media-title" className="text-sm font-semibold">Related media</h3>{relatedLoading ? <LoaderCircle className="animate-spin text-muted" size={14} /> : <span className="text-[10px] text-muted">Same series, creator, or site</span>}</div>{relatedItems.length > 0 ? <div className="mt-3 grid gap-2">{relatedItems.map((related) => <button key={related.id} className="group flex min-w-0 items-center gap-3 rounded-control bg-canvas/45 p-2 text-left transition hover:bg-elevated" onClick={() => onSelectRelated(related)}><div className="aspect-video w-24 shrink-0 overflow-hidden rounded-control bg-canvas"><MediaArtwork item={related} /></div><span className="min-w-0"><span className="line-clamp-2 block text-xs font-medium leading-4 text-ink group-hover:text-accent">{related.title}</span><span className="mt-1 block truncate text-[10px] text-muted">{related.site || related.container?.toLocaleUpperCase() || (related.mediaKind === "video" ? "Video" : "Image")}</span></span></button>)}</div> : null}</section> : null}
      <dl className="mt-4 grid grid-cols-[100px_1fr] gap-x-4 gap-y-3 rounded-panel border border-line/45 bg-canvas/35 p-4 text-xs"><dt className="text-muted">Type</dt><dd>{item.mediaKind === "image" ? "Image" : "Video"}{item.container ? ` · ${item.container.toLocaleUpperCase()}` : ""}</dd>{item.width && item.height ? <><dt className="text-muted">Dimensions</dt><dd className="number-font">{item.width} × {item.height}{item.fps ? ` · ${formatFps(item.fps)} fps` : ""}{item.dynamicRange ? ` · ${item.dynamicRange}` : ""}</dd></> : null}{item.videoCodec || item.audioCodec ? <><dt className="text-muted">Codecs</dt><dd>{[formatCodec(item.videoCodec), formatCodec(item.audioCodec)].filter(Boolean).join(" + ")}</dd></> : null}{item.fileSizeBytes != null ? <><dt className="text-muted">File size</dt><dd className="number-font">{formatBytes(item.fileSizeBytes)}</dd></> : null}<dt className="text-muted">Added</dt><dd>{new Date(item.addedAt).toLocaleString()}</dd>{item.modifiedAtMs ? <><dt className="text-muted">Modified</dt><dd>{new Date(item.modifiedAtMs).toLocaleString()}</dd></> : null}<dt className="text-muted">Location</dt><dd className="break-all text-muted" title={item.path}>{item.relativePath || item.path}</dd>{item.site ? <><dt className="text-muted">Origin</dt><dd><button className="font-medium text-accent hover:underline" onClick={() => onBrowseSite(item.site!)}>{item.site}</button></dd></> : null}</dl>
      {item.metadataState === "pending" ? <p className="mt-5 flex items-center gap-2 text-[11px] text-muted" role="status"><LoaderCircle className="animate-spin text-accent" size={13} /> Inspecting resolution, codecs, and duration…</p> : null}
      {item.metadataError ? <details className="mt-5 border-l-2 border-amber-400/50 pl-3 text-[11px] text-muted"><summary className="cursor-pointer font-medium text-amber-300">Media inspection unavailable</summary><p className="mt-2 leading-5">{item.metadataError}</p></details> : null}
      {item.tags.length > 0 ? <div className="mt-6 flex flex-wrap gap-1.5">{item.tags.map((tag) => <span key={tag} className="rounded-[6px] bg-elevated px-2 py-1 text-[10px] text-muted">#{tag}</span>)}</div> : null}
      <section className="mt-4 rounded-panel border border-line/45 bg-elevated/25 p-4" aria-labelledby="organize-media-title"><button className="flex w-full items-center justify-between text-left" onClick={() => { setOrganizeOpen((value) => !value); setOperationPlan(null); }} aria-expanded={organizeOpen}><span><span id="organize-media-title" className="block text-sm font-semibold">Organize file</span><span className="mt-1 block text-[11px] text-muted">Preview every rename, move, or copy before it changes disk.</span></span><FolderInput className="text-accent" size={18} /></button>{organizeOpen ? <div className="mt-4 space-y-3"><Field label="Action"><Select value={operationKind} onChange={(event) => { setOperationKind(event.target.value as LibraryFileOperationKind); setOperationPlan(null); }}><option value="rename">Rename in this folder</option><option value="move">Move to another folder</option><option value="copy">Copy to another folder</option></Select></Field>{operationKind !== "rename" ? <Field label="Destination folder"><div className="flex gap-2"><Input value={destinationDirectory} readOnly placeholder="Choose a folder" /><Button variant="secondary" className="shrink-0 px-3" onClick={() => void chooseOperationFolder()}><FolderOpen size={14} /> Browse</Button></div></Field> : null}<Field label="Filename"><Input value={newFileName} onChange={(event) => { setNewFileName(event.target.value); setOperationPlan(null); }} /></Field>{operationPlan ? <div className="rounded-control border border-accent/25 bg-accent/5 p-3"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Ready to confirm</p><div className="mt-3 grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-[11px]"><span className="min-w-0 break-all text-muted">{operationPlan.sourceDisplayName}</span><ArrowRight className="shrink-0 text-accent" size={14} /><span className="min-w-0 break-all text-ink">{operationPlan.destinationDisplayName}</span></div><ul className="mt-3 space-y-1 border-t border-line/60 pt-3 text-[10px] leading-4 text-muted">{operationPlan.warnings.map((warning) => <li key={warning}>• {warning}</li>)}</ul></div> : null}<div className="flex gap-2">{operationPlan ? <Button variant="secondary" disabled={operationBusy} onClick={() => setOperationPlan(null)}>Change plan</Button> : <Button variant="secondary" disabled={operationBusy || !newFileName.trim() || (operationKind !== "rename" && !destinationDirectory)} onClick={() => void previewOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : null} Preview</Button>}{operationPlan ? <Button disabled={operationBusy} onClick={() => void executeOperation()}>{operationBusy ? <LoaderCircle className="animate-spin" size={14} /> : <Check size={14} />} Confirm {operationKind}</Button> : null}</div></div> : null}</section>
      <div className="mb-1 mt-4 rounded-panel border border-line/45 bg-canvas/30 p-4"><p className="text-[11px] leading-5 text-muted">Removing this record does not delete the original file.</p><div className="mt-3 flex items-center gap-2">{confirmRemove ? <Button variant="secondary" className="h-8 text-xs" onClick={() => setConfirmRemove(false)}>Keep record</Button> : null}<Button variant={confirmRemove ? "danger" : "ghost"} className="h-8 text-xs" onClick={() => void remove()}><Trash2 size={13} /> {confirmRemove ? "Confirm removal" : "Remove record"}</Button></div></div></div></aside>;
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
  return formatMediaCodec(codec);
}

function formatFps(fps: number): string {
  return Number.isInteger(fps) ? fps.toFixed(0) : fps.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

function sourceLabel(item: LibraryItem, sourceNames: Map<string, string>): string {
  return item.sourceId ? sourceNames.get(item.sourceId) || item.site || "Local media" : item.site || "Local media";
}

function PrivacyManagerDialog({ status, rules, sources, busy, onClose, onShowPrivate, onOpenItem, onRemoveItem, onRemoveFolder, onRemoveSource }: { status: LibraryPrivacyStatus; rules: LibraryPrivacyRules; sources: LibrarySource[]; busy: boolean; onClose: () => void; onShowPrivate: () => void; onOpenItem: (item: LibraryItem) => void; onRemoveItem: (itemId: string) => Promise<void>; onRemoveFolder: (sourceId: string, relativePath: string) => Promise<void>; onRemoveSource: (source: LibrarySource) => Promise<void> }) {
  const setError = useAppStore((state) => state.setError);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [loading, setLoading] = useState(true);
  const sourceNames = useMemo(() => new Map(sources.map((source) => [source.id, source.name])), [sources]);
  const privateSources = useMemo(() => sources.filter((source) => source.private), [sources]);

  useEffect(() => {
    let disposed = false;
    void (async () => {
      setLoading(true);
      try {
        const loaded: LibraryItem[] = [];
        for (let index = 0; index < rules.itemIds.length; index += 500) {
          loaded.push(...await api.getLibraryItemsByIds(rules.itemIds.slice(index, index + 500)));
        }
        if (!disposed) setItems(loaded);
      } catch (error) {
        if (!disposed) setError(normalizeError(error));
      } finally {
        if (!disposed) setLoading(false);
      }
    })();
    return () => { disposed = true; };
  }, [rules, setError]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !busy) onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [busy, onClose]);

  const query = search.trim().toLocaleLowerCase();
  const matches = (...values: Array<string | null | undefined>) => !query || values.some((value) => value?.toLocaleLowerCase().includes(query));
  const visibleSources = privateSources.filter((source) => matches(source.name, source.path));
  const visibleFolders = rules.folders.filter((folder) => matches(sourceNames.get(folder.sourceId), folder.relativePath));
  const visibleItems = items.filter((item) => matches(item.title, item.path, sourceNames.get(item.sourceId || "")));
  const noMatches = visibleSources.length === 0 && visibleFolders.length === 0 && visibleItems.length === 0;

  return <div className="fixed inset-0 z-[100] grid place-items-center bg-black/70 p-5 backdrop-blur-sm" onMouseDown={() => { if (!busy) onClose(); }}><section className="panel-shadow flex max-h-[min(860px,calc(100dvh-40px))] w-full max-w-5xl flex-col overflow-hidden rounded-[16px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="privacy-manager-title" onMouseDown={(event) => event.stopPropagation()}><header className="flex flex-wrap items-start gap-4 border-b border-line/70 px-6 py-5"><div className="grid size-11 shrink-0 place-items-center rounded-[11px] bg-accent/10 text-accent"><Lock size={20} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-accent">Unlocked for this session</p><h2 id="privacy-manager-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Manage private media</h2><p className="mt-1 text-xs text-muted">Review every privacy rule without exposing this list while the Library is locked.</p></div><Button variant="secondary" className="h-9" onClick={onShowPrivate}><Library size={14} /> Open Private view</Button><button className="grid size-9 place-items-center rounded-[8px] text-muted hover:bg-elevated hover:text-ink" disabled={busy} onClick={onClose} aria-label="Close private-media manager"><X size={17} /></button></header><div className="border-b border-line/60 px-6 py-4"><label className="relative block"><span className="sr-only">Search private media and rules</span><Search className="absolute left-3.5 top-3 text-muted" size={15} /><Input autoFocus className="h-10 bg-elevated/55 pl-10" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="Search private items, folders, and sources" /></label><div className="mt-3 grid gap-2 text-center sm:grid-cols-3"><div className="rounded-[9px] bg-elevated/45 px-3 py-2"><strong className="number-font block text-sm">{status.privateItemCount}</strong><span className="text-[10px] text-muted">Individual items</span></div><div className="rounded-[9px] bg-elevated/45 px-3 py-2"><strong className="number-font block text-sm">{status.privateFolderCount}</strong><span className="text-[10px] text-muted">Folder rules</span></div><div className="rounded-[9px] bg-elevated/45 px-3 py-2"><strong className="number-font block text-sm">{status.privateSourceCount}</strong><span className="text-[10px] text-muted">Private sources</span></div></div></div><div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">{loading ? <div className="flex items-center justify-center gap-2 py-16 text-sm text-muted"><LoaderCircle className="animate-spin text-accent" size={17} /> Loading private rules…</div> : noMatches ? <div className="rounded-[12px] border border-dashed border-line/70 px-6 py-14 text-center"><Lock className="mx-auto text-muted/40" size={28} /><p className="mt-3 text-sm font-semibold">{query ? "No private rules match this search" : "No private rules yet"}</p><p className="mt-1 text-xs text-muted">Use Select in the Library to mark individual media private.</p></div> : <div className="space-y-7">{visibleItems.length > 0 ? <section aria-labelledby="privacy-items-heading"><div className="mb-3"><h3 id="privacy-items-heading" className="text-sm font-semibold">Individual item rules</h3><p className="mt-1 text-[11px] text-muted">Rules attached directly to a video or image.</p></div><div className="space-y-2">{visibleItems.map((item) => <article key={item.id} className="flex items-center gap-3 rounded-[10px] border border-line/55 bg-elevated/35 p-2.5"><button className="relative aspect-video w-24 shrink-0 overflow-hidden rounded-[7px] bg-elevated focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpenItem(item)}><MediaArtwork item={item} /></button><button className="min-w-0 flex-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" onClick={() => onOpenItem(item)}><strong className="block truncate text-xs font-semibold">{item.title}</strong><span className="mt-1 block truncate text-[10px] text-muted">{sourceLabel(item, sourceNames)} · {item.mediaKind || "media"}</span></button><Button variant="ghost" className="h-8 shrink-0 px-2 text-xs" disabled={busy} onClick={() => void onRemoveItem(item.id)}><X size={13} /> Remove rule</Button></article>)}</div></section> : null}{visibleFolders.length > 0 ? <section aria-labelledby="privacy-folders-heading"><div className="mb-3"><h3 id="privacy-folders-heading" className="text-sm font-semibold">Private folders</h3><p className="mt-1 text-[11px] text-muted">Every nested media item inherits the folder rule.</p></div><div className="grid gap-2 sm:grid-cols-2">{visibleFolders.map((folder) => <article key={`${folder.sourceId}:${folder.relativePath}`} className="flex items-center gap-3 rounded-[10px] border border-line/55 bg-elevated/35 p-3"><div className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-accent/10 text-accent"><FolderOpen size={16} /></div><div className="min-w-0 flex-1"><strong className="block truncate text-xs font-semibold">{folder.relativePath || "Source root"}</strong><span className="mt-1 block truncate text-[10px] text-muted">{sourceNames.get(folder.sourceId) || "Unknown source"}</span></div><button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-red-300 disabled:opacity-40" disabled={busy} onClick={() => void onRemoveFolder(folder.sourceId, folder.relativePath)} aria-label={`Remove privacy rule from ${folder.relativePath || "source root"}`}><X size={14} /></button></article>)}</div></section> : null}{visibleSources.length > 0 ? <section aria-labelledby="privacy-sources-heading"><div className="mb-3"><h3 id="privacy-sources-heading" className="text-sm font-semibold">Private sources</h3><p className="mt-1 text-[11px] text-muted">Every indexed item in the source is private.</p></div><div className="grid gap-2 sm:grid-cols-2">{visibleSources.map((source) => <article key={source.id} className="flex items-center gap-3 rounded-[10px] border border-line/55 bg-elevated/35 p-3"><div className="grid size-9 shrink-0 place-items-center rounded-[8px] bg-accent/10 text-accent"><HardDrive size={16} /></div><div className="min-w-0 flex-1"><strong className="block truncate text-xs font-semibold">{source.name}</strong><span className="mt-1 block truncate text-[10px] text-muted" title={source.path}>{source.path}</span></div><button className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-panel hover:text-red-300 disabled:opacity-40" disabled={busy} onClick={() => void onRemoveSource(source)} aria-label={`Make ${source.name} a normal source`}><X size={14} /></button></article>)}</div></section> : null}</div>}</div><footer className="flex flex-wrap items-center gap-3 border-t border-line/65 bg-elevated/20 px-6 py-4"><p className="mr-auto max-w-2xl text-[10px] leading-4 text-muted">The Private view includes media inherited from source and folder rules. Privacy hides records inside the app; it does not encrypt the original files on disk.</p><Button variant="secondary" disabled={busy} onClick={onClose}>Done</Button></footer></section></div>;
}

function BulkActionBar({ selectedIds, collections, busy, onCompare, onCompareImages, onCreateStack, onMove, onSetPrivacy, onApply, onClose }: { selectedIds: Set<string>; collections: LibraryCollection[]; busy: boolean; onCompare: () => Promise<void>; onCompareImages: () => Promise<void>; onCreateStack: () => Promise<void>; onMove: () => void; onSetPrivacy: (isPrivate: boolean) => Promise<void>; onApply: (action: LibraryBulkAction, message: string) => Promise<void>; onClose: () => void }) {
  const [tag, setTag] = useState("");
  const [metadataOpen, setMetadataOpen] = useState(false);
  const disabled = busy || selectedIds.size === 0;
  const compareDisabled = busy || selectedIds.size < 2 || selectedIds.size > 4;
  const stackDisabled = busy || selectedIds.size < 2 || selectedIds.size > 100;
  function closeMetadata() {
    setMetadataOpen(false);
    window.requestAnimationFrame(() => document.querySelector<HTMLButtonElement>("[data-bulk-metadata-trigger]")?.focus());
  }
  return <><aside className="panel-shadow fixed bottom-6 left-1/2 z-40 flex max-h-[calc(100dvh-3rem)] w-[min(1280px,calc(100vw-300px))] -translate-x-1/2 flex-wrap items-center gap-2 overflow-y-auto rounded-[14px] border border-line/80 bg-panel/95 p-3 backdrop-blur-xl" aria-label="Bulk library actions"><div className="mr-2 min-w-24"><p className="number-font text-sm font-semibold">{selectedIds.size} selected</p><button className="mt-0.5 text-[10px] font-medium text-muted hover:text-ink" onClick={onClose}>Clear selection</button></div><Button className="h-9 px-3 text-xs" disabled={compareDisabled} onClick={() => void onCompare()} title={compareDisabled ? "Select between 2 and 4 videos to compare" : "Open selected videos in the comparison grid"}><LayoutGrid size={14} /> Videos</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={compareDisabled} onClick={() => void onCompareImages()} title={compareDisabled ? "Select between 2 and 4 images to compare" : "Open selected images in the comparison studio"}><ImageIcon size={14} /> Images</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={stackDisabled} onClick={() => void onCreateStack()} title={stackDisabled ? "Select between 2 and 100 items of one media type" : "Group versions without moving files"}><Layers size={14} /> Stack</Button><Button className="h-9 px-3 text-xs" disabled={disabled} onClick={onMove}><FolderInput size={14} /> Move</Button><Button data-bulk-metadata-trigger variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => setMetadataOpen(true)}><FileImage size={14} /> Metadata</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onSetPrivacy(true)}><Lock size={14} /> Make private</Button><Button variant="ghost" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onSetPrivacy(false)}>Remove privacy rule</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: true }, `Marked ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} as favorites.`)}><Heart size={14} /> Favorite</Button><Button variant="secondary" className="h-9 px-3 text-xs" disabled={disabled} onClick={() => void onApply({ type: "setFavorite", favorite: false }, `Removed ${selectedIds.size} item${selectedIds.size === 1 ? "" : "s"} from favorites.`)}><Heart size={14} fill="none" /> Unfavorite</Button><label><span className="sr-only">Set rating</span><Select className="h-9 w-32 text-xs" disabled={disabled} defaultValue="" onChange={(event) => { const value = event.target.value; if (value !== "") void onApply({ type: "setRating", rating: value === "clear" ? null : Number(value) }, value === "clear" ? "Cleared selected ratings." : `Rated selected media ${value} stars.`); event.currentTarget.value = ""; }}><option value="" disabled>Set rating</option>{[5, 4, 3, 2, 1].map((rating) => <option key={rating} value={rating}>{rating} star{rating === 1 ? "" : "s"}</option>)}<option value="clear">Clear rating</option></Select></label><label><span className="sr-only">Add to collection</span><Select className="h-9 w-44 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "addToCollection", collectionId: event.target.value }, "Added selected media to the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Add to collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><label><span className="sr-only">Remove from collection</span><Select className="h-9 w-48 text-xs" disabled={disabled || collections.length === 0} defaultValue="" onChange={(event) => { if (event.target.value) void onApply({ type: "removeFromCollection", collectionId: event.target.value }, "Removed selected media from the collection."); event.currentTarget.value = ""; }}><option value="" disabled>{collections.length ? "Remove from collection" : "No collections"}</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></label><div className="flex min-w-48 flex-1"><Input className="h-9 rounded-r-none text-xs" value={tag} onChange={(event) => setTag(event.target.value)} placeholder="Add a tag" /><Button variant="secondary" className="h-9 rounded-l-none px-3 text-xs" disabled={disabled || !tag.trim()} onClick={() => { const value = tag.trim(); if (!value) return; void onApply({ type: "addTags", tags: [value] }, `Added #${value} to selected media.`); setTag(""); }}><Tags size={14} /> Add</Button></div></aside>{metadataOpen ? <BulkMetadataDialog selectedIds={selectedIds} onClose={closeMetadata} /> : null}</>;
}

function BulkMetadataDialog({ selectedIds, onClose }: { selectedIds: Set<string>; onClose: () => void }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [creator, setCreator] = useState("");
  const [series, setSeries] = useState("");
  const [episode, setEpisode] = useState("");
  const [tags, setTags] = useState("");
  const [busy, setBusy] = useState(false);
  const hasChanges = Boolean(creator.trim() || series.trim() || episode.trim() || tags.trim());

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

  async function applyMetadata() {
    if (!hasChanges || selectedIds.size === 0 || busy) return;
    setBusy(true);
    try {
      await api.applyLibraryBulkMetadata({
        itemIds: Array.from(selectedIds),
        creator: creator.trim() || null,
        series: series.trim() || null,
        episode: episode.trim() || null,
        tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean).slice(0, 20),
      });
      window.dispatchEvent(new Event("nice-downloader:library-items-changed"));
      setNotice({ message: `Updated metadata for ${selectedIds.size} selected item${selectedIds.size === 1 ? "" : "s"}. Untouched fields were preserved.`, view: "library" });
      onClose();
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  function trapFocus(event: React.KeyboardEvent<HTMLElement>) {
    if (event.key !== "Tab") return;
    const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled), [tabindex]:not([tabindex='-1'])"));
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

  return <div className="fixed inset-0 z-[80] grid place-items-center bg-black/70 p-5 backdrop-blur-sm" onMouseDown={() => { if (!busy) onClose(); }}><section className="panel-shadow w-full max-w-2xl overflow-hidden rounded-[14px] border border-line bg-panel" role="dialog" aria-modal="true" aria-labelledby="bulk-metadata-title" aria-describedby="bulk-metadata-description" aria-busy={busy} onKeyDown={trapFocus} onMouseDown={(event) => event.stopPropagation()}><header className="flex items-start gap-4 border-b border-line/70 px-5 py-4"><div className="grid size-10 shrink-0 place-items-center rounded-[9px] bg-accent/10 text-accent"><FileImage size={18} /></div><div className="min-w-0 flex-1"><p className="text-[10px] font-semibold uppercase tracking-[0.1em] text-accent">Catalog presentation</p><h2 id="bulk-metadata-title" className="mt-1 text-xl font-semibold tracking-[-0.03em]">Edit {selectedIds.size} selected item{selectedIds.size === 1 ? "" : "s"}</h2><p id="bulk-metadata-description" className="mt-1.5 text-xs leading-5 text-muted">Only entered values are applied. Titles, descriptions, dates, URLs, artwork, and all other metadata remain unchanged.</p></div><button autoFocus className="grid size-8 shrink-0 place-items-center rounded-[7px] text-muted hover:bg-elevated hover:text-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/70" disabled={busy} onClick={onClose} aria-label="Close bulk metadata editor"><X size={17} /></button></header><div className="grid gap-3 p-5 sm:grid-cols-2"><Field label="Creator or channel" hint="Leave blank to preserve each current creator."><Input value={creator} maxLength={200} onChange={(event) => setCreator(event.target.value)} placeholder="Apply one creator" /></Field><Field label="Series" hint="Useful for grouping episodes into a shelf."><Input value={series} maxLength={200} onChange={(event) => setSeries(event.target.value)} placeholder="Apply one series" /></Field><Field label="Episode label"><Input value={episode} maxLength={200} onChange={(event) => setEpisode(event.target.value)} placeholder="Special, S01E04, Part 2" /></Field><Field label="Add tags" hint="Comma-separated; existing tags stay in place."><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="archive, tutorial" /></Field></div><footer className="flex flex-wrap items-center justify-between gap-3 border-t border-line/70 bg-elevated/25 px-5 py-4"><p className="text-[10px] text-muted">Original files and sidecars are never modified.</p><div className="flex gap-2"><Button variant="ghost" disabled={busy} onClick={onClose}>Cancel</Button><Button disabled={!hasChanges || busy} onClick={() => void applyMetadata()}>{busy ? <LoaderCircle className="animate-spin" size={14} /> : <Save size={14} />} Apply metadata</Button></div></footer></section></div>;
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
  return <section aria-labelledby="library-sources-title"><div className="mb-3 flex items-center justify-between"><div><h2 id="library-sources-title" className="text-lg font-semibold tracking-[-0.025em]">Folders</h2><p className="mt-1 text-xs text-muted">Scans index media in place. Watched folders can organize newly discovered media automatically.</p></div><span className="text-[11px] text-muted">{sources.length} source{sources.length === 1 ? "" : "s"}</span></div><div className="grid items-start gap-3 lg:grid-cols-2 2xl:grid-cols-3">{sources.map((source) => <SourceCard key={source.id} source={source} latestJob={scanJobs.find((job) => job.sourceId === source.id)} onJob={onJob} onRemoved={() => onRemoved(source.id)} />)}</div></section>;
}

function WatchAutomationSection({ sources, collections }: { sources: LibrarySource[]; collections: LibraryCollection[] }) {
  if (sources.length === 0) return null;
  return <section className="mt-8 border-t border-line/60 pt-7" aria-labelledby="watch-automation-title">
    <div className="mb-4 flex flex-wrap items-end justify-between gap-3"><div><h2 id="watch-automation-title" className="flex items-center gap-2 text-lg font-semibold tracking-[-0.025em]"><FolderInput className="text-accent" size={19} /> Watch-folder automation</h2><p className="mt-1 max-w-3xl text-xs leading-5 text-muted">Apply typed organization rules only when a folder discovers new media. Existing catalog items and original files stay unchanged.</p></div><span className="rounded-[6px] bg-elevated px-2 py-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-muted">No scripts</span></div>
    <div className="grid items-start gap-3 lg:grid-cols-2">{sources.map((source) => <WatchAutomationRule key={source.id} source={source} collections={collections} />)}</div>
  </section>;
}

function WatchAutomationRule({ source, collections }: { source: LibrarySource; collections: LibraryCollection[] }) {
  const setError = useAppStore((state) => state.setError);
  const setNotice = useAppStore((state) => state.setNotice);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [automation, setAutomation] = useState<LibrarySourceAutomation | null>(null);
  const [tags, setTags] = useState("");
  const [collectionId, setCollectionId] = useState("");
  const [favorite, setFavorite] = useState(false);

  async function loadAutomation() {
    setLoading(true);
    try {
      const next = await api.getLibrarySourceAutomation(source.id);
      setAutomation(next);
      setTags(next.tags.join(", "));
      setCollectionId(next.collectionId || "");
      setFavorite(next.favorite);
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setLoading(false);
    }
  }

  function toggleOpen() {
    const next = !open;
    setOpen(next);
    if (next && !automation && !loading) void loadAutomation();
  }

  async function saveAutomation() {
    if (busy) return;
    setBusy(true);
    try {
      const next = await api.saveLibrarySourceAutomation({
        sourceId: source.id,
        tags: tags.split(",").map((tag) => tag.trim()).filter(Boolean).slice(0, 20),
        collectionId: collectionId || null,
        favorite,
      });
      setAutomation(next);
      setTags(next.tags.join(", "));
      setCollectionId(next.collectionId || "");
      setFavorite(next.favorite);
      setNotice({ message: next.tags.length || next.collectionId || next.favorite ? `Automation saved for ${source.name}. New media will be organized after indexing.` : `Automation cleared for ${source.name}.`, view: "library" });
    } catch (error) {
      setError(normalizeError(error));
    } finally {
      setBusy(false);
    }
  }

  const actionCount = automation ? automation.tags.length + Number(Boolean(automation.collectionId)) + Number(automation.favorite) : 0;
  return <article className="rounded-[10px] border border-line/60 bg-panel/55">
    <button className="flex w-full items-center gap-3 p-4 text-left" onClick={toggleOpen} aria-expanded={open} aria-controls={`watch-automation-${source.id}`}><span className={`grid size-9 shrink-0 place-items-center rounded-[8px] ${source.watchEnabled ? "bg-accent/12 text-accent" : "bg-elevated text-muted"}`}><FolderInput size={16} /></span><span className="min-w-0 flex-1"><strong className="block truncate text-sm font-semibold">{source.name}</strong><span className="mt-1 block text-[11px] text-muted">{automation ? actionCount > 0 ? `${actionCount} active action${actionCount === 1 ? "" : "s"}` : "No organization actions" : source.watchEnabled ? "Auto scan enabled" : "Manual scans only"}</span></span><ChevronRight className={`shrink-0 text-muted transition-transform ${open ? "rotate-90" : ""}`} size={15} /></button>
    {open ? <div id={`watch-automation-${source.id}`} className="border-t border-line/60 p-4">{loading ? <p className="flex items-center gap-2 py-3 text-xs text-muted"><LoaderCircle className="animate-spin" size={14} /> Loading automation…</p> : <><p className="mb-3 text-[11px] leading-5 text-muted">Rules run once for each newly indexed file. {source.watchEnabled ? "Folder changes are watched automatically." : "Enable Auto scan above for immediate folder monitoring."}</p><div className="grid gap-3 sm:grid-cols-2"><Field label="Add tags"><Input value={tags} onChange={(event) => setTags(event.target.value)} placeholder="incoming, archive" /></Field><Field label="Add to collection"><Select value={collectionId} onChange={(event) => setCollectionId(event.target.value)}><option value="">No collection</option>{collections.map((collection) => <option key={collection.id} value={collection.id}>{collection.name}</option>)}</Select></Field></div><label className="mt-3 flex items-center gap-2 text-xs text-muted"><input type="checkbox" checked={favorite} onChange={(event) => setFavorite(event.target.checked)} /> Mark new media as favorite</label><div className="mt-4 flex justify-end"><Button className="h-9 text-xs" disabled={busy} onClick={() => void saveAutomation()}>{busy ? <LoaderCircle className="animate-spin" size={14} /> : <Save size={14} />} Save automation</Button></div></>}</div> : null}
  </article>;
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
