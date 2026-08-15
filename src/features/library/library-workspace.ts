import type { LibrarySort } from "../../types";

export const LIBRARY_WORKSPACE_KEY = "nice-downloader.library-workspace.v1";

export interface LibraryWorkspaceSnapshot {
  search: string;
  sourceFilter: string;
  folderFilter: string;
  folderScope: "tree" | "direct";
  mediaFilter: "all" | "video" | "image";
  resolutionFilter: "all" | "2160" | "1440" | "1080" | "720" | "sd" | "unknown";
  containerFilter: string;
  durationFilter: "all" | "short" | "medium" | "long" | "unknown";
  dateFilter: "all" | "week" | "month" | "year";
  publishedFilter: "all" | "week" | "month" | "year";
  sort: LibrarySort;
  availabilityFilter: "all" | "available" | "missing";
  tagFilters: string[];
  creatorFilters: string[];
  seriesFilters: string[];
  siteFilters: string[];
  ratingFilter: string;
  collectionFilter: string;
  favoriteOnly: boolean;
  resultPage: number;
  viewMode: "grid" | "list";
  selectedItemId: string | null;
  scrollTop: number;
}

const SORTS: LibrarySort[] = ["recent", "oldest", "publishedDesc", "publishedAsc", "fileDateDesc", "fileDateAsc", "titleAsc", "titleDesc", "durationDesc", "durationAsc", "sizeDesc", "sizeAsc"];

export function defaultLibraryWorkspace(storage: Pick<Storage, "getItem"> = window.localStorage): LibraryWorkspaceSnapshot {
  return {
    search: "",
    sourceFilter: "all",
    folderFilter: "all",
    folderScope: "tree",
    mediaFilter: "all",
    resolutionFilter: "all",
    containerFilter: "all",
    durationFilter: "all",
    dateFilter: "all",
    publishedFilter: "all",
    sort: "recent",
    availabilityFilter: "available",
    tagFilters: [],
    creatorFilters: [],
    seriesFilters: [],
    siteFilters: [],
    ratingFilter: "all",
    collectionFilter: "all",
    favoriteOnly: false,
    resultPage: 1,
    viewMode: storage.getItem("nice-downloader.library-view") === "list" ? "list" : "grid",
    selectedItemId: null,
    scrollTop: 0,
  };
}

export function readLibraryWorkspace(storage: Pick<Storage, "getItem"> = window.localStorage): LibraryWorkspaceSnapshot {
  const defaults = defaultLibraryWorkspace(storage);
  try {
    const value = JSON.parse(storage.getItem(LIBRARY_WORKSPACE_KEY) || "null") as Record<string, unknown> | null;
    if (!value || typeof value !== "object") return defaults;
    return {
      search: boundedString(value.search, defaults.search, 500),
      sourceFilter: boundedString(value.sourceFilter, defaults.sourceFilter, 100),
      folderFilter: boundedString(value.folderFilter, defaults.folderFilter, 2_000),
      folderScope: oneOf(value.folderScope, ["tree", "direct"] as const, defaults.folderScope),
      mediaFilter: oneOf(value.mediaFilter, ["all", "video", "image"] as const, defaults.mediaFilter),
      resolutionFilter: oneOf(value.resolutionFilter, ["all", "2160", "1440", "1080", "720", "sd", "unknown"] as const, defaults.resolutionFilter),
      containerFilter: boundedString(value.containerFilter, defaults.containerFilter, 40),
      durationFilter: oneOf(value.durationFilter, ["all", "short", "medium", "long", "unknown"] as const, defaults.durationFilter),
      dateFilter: oneOf(value.dateFilter, ["all", "week", "month", "year"] as const, defaults.dateFilter),
      publishedFilter: oneOf(value.publishedFilter, ["all", "week", "month", "year"] as const, defaults.publishedFilter),
      sort: oneOf(value.sort, SORTS, defaults.sort),
      availabilityFilter: oneOf(value.availabilityFilter, ["all", "available", "missing"] as const, defaults.availabilityFilter),
      tagFilters: boundedStrings(value.tagFilters),
      creatorFilters: boundedStrings(value.creatorFilters),
      seriesFilters: boundedStrings(value.seriesFilters),
      siteFilters: boundedStrings(value.siteFilters),
      ratingFilter: oneOf(value.ratingFilter, ["all", "rated", "5", "4", "3", "unrated"] as const, defaults.ratingFilter),
      collectionFilter: boundedString(value.collectionFilter, defaults.collectionFilter, 100),
      favoriteOnly: typeof value.favoriteOnly === "boolean" ? value.favoriteOnly : defaults.favoriteOnly,
      resultPage: boundedNumber(value.resultPage, 1, 100_000, defaults.resultPage),
      viewMode: oneOf(value.viewMode, ["grid", "list"] as const, defaults.viewMode),
      selectedItemId: typeof value.selectedItemId === "string" && value.selectedItemId.length <= 100 ? value.selectedItemId : null,
      scrollTop: boundedNumber(value.scrollTop, 0, 10_000_000, defaults.scrollTop),
    };
  } catch {
    return defaults;
  }
}

export function writeLibraryWorkspace(snapshot: LibraryWorkspaceSnapshot, storage: Pick<Storage, "setItem"> = window.localStorage): void {
  try {
    storage.setItem(LIBRARY_WORKSPACE_KEY, JSON.stringify(snapshot));
  } catch {
    // Workspace restoration is best-effort and must never block Library use.
  }
}

function boundedString(value: unknown, fallback: string, max: number): string {
  return typeof value === "string" && value.length <= max ? value : fallback;
}

function boundedStrings(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return Array.from(new Set(value.filter((entry): entry is string => typeof entry === "string" && entry.length > 0 && entry.length <= 200))).slice(0, 20);
}

function boundedNumber(value: unknown, min: number, max: number, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? Math.min(max, Math.max(min, Math.floor(value))) : fallback;
}

function oneOf<const T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return typeof value === "string" && allowed.includes(value as T) ? value as T : fallback;
}
