import {
  Clapperboard,
  Download,
  FileCog,
  HeartPulse,
  House,
  Images,
  LayoutGrid,
  Library,
  ListChecks,
  Plus,
  Rss,
  ScanSearch,
  Settings,
  Share2,
  SlidersHorizontal,
  Sparkles,
  WandSparkles,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { ViewId } from "../types";

export interface NavigationItem {
  id: ViewId;
  label: string;
  icon: LucideIcon;
  description?: string;
  usage?: string;
  selection?: "required" | "optional" | "notRequired";
}

export const primaryNavigation: NavigationItem[] = [
  { id: "home", label: "Home", icon: House },
  { id: "new", label: "New Download", icon: Plus },
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "library", label: "Library", icon: Library },
  { id: "subscriptions", label: "Subscriptions", icon: Rss },
  { id: "tools", label: "Tools", icon: Wrench },
  { id: "settings", label: "Settings", icon: Settings },
];

export const toolNavigation: NavigationItem[] = [
  {
    id: "curation",
    label: "Review Library",
    icon: ListChecks,
    description: "Move through a focused review queue and make quick keep, rating, tag, and organization decisions.",
    usage: "Use when a growing library needs a fast cleanup pass.",
    selection: "notRequired",
  },
  {
    id: "compare",
    label: "Compare Videos",
    icon: LayoutGrid,
    description: "Play two to four videos side by side with shared playback controls and audio focus.",
    usage: "Use to choose between edits, encodes, or near-duplicate versions.",
    selection: "required",
  },
  {
    id: "imageCompare",
    label: "Compare Images",
    icon: Images,
    description: "Inspect two to four images with synchronized zoom, pan, overlay, and split views.",
    usage: "Use to inspect alternate exports or closely related artwork.",
    selection: "required",
  },
  {
    id: "similarity",
    label: "Find Similar Media",
    icon: ScanSearch,
    description: "Build a local visual index and find media with closely matching frames or composition.",
    usage: "Use to surface visual duplicates and related captures.",
    selection: "optional",
  },
  {
    id: "themes",
    label: "Discover Themes",
    icon: Sparkles,
    description: "Use an optional local vision model to group videos by evidence-backed themes.",
    usage: "Use when titles and folders do not describe what is actually in a video.",
    selection: "optional",
  },
  {
    id: "montages",
    label: "Build Montages",
    icon: Clapperboard,
    description: "Arrange selected clips into a local, cancellable montage with controlled transitions.",
    usage: "Use to create compilations without changing the source files.",
    selection: "required",
  },
  {
    id: "transformations",
    label: "Optimize & Transform",
    icon: WandSparkles,
    description: "Batch-optimize Library videos for broad playback or create focused conversions, extracts, and privacy-safe derivatives.",
    usage: "Use when several videos need reliable H.264/AAC playback or one file needs a specialized output.",
    selection: "optional",
  },
  {
    id: "recipes",
    label: "Automation Recipes",
    icon: FileCog,
    description: "Chain typed post-download steps such as import, tagging, transcoding, transcription, and indexing.",
    usage: "Use to make repeatable acquisition workflows automatic.",
    selection: "notRequired",
  },
  {
    id: "sharing",
    label: "Temporary Sharing",
    icon: Share2,
    description: "Serve a time-limited, capability-protected set of local media on your network.",
    usage: "Use for short-lived viewing access without publishing a permanent link.",
    selection: "required",
  },
  {
    id: "health",
    label: "Media Health",
    icon: HeartPulse,
    description: "Review missing files, pending metadata, compatibility risks, search consistency, and cache status.",
    usage: "Use for maintenance and playback-readiness checks.",
    selection: "notRequired",
  },
  {
    id: "presets",
    label: "Download Presets",
    icon: SlidersHorizontal,
    description: "Save reusable download intentions and apply them automatically to matching sites.",
    usage: "Use when the same quality and processing choices recur.",
    selection: "notRequired",
  },
];

const downloadViews = new Set<ViewId>(["downloads", "queue", "history"]);
const toolViews = new Set<ViewId>(toolNavigation.map((item) => item.id));

export function primaryViewFor(view: ViewId): ViewId {
  if (downloadViews.has(view)) return "downloads";
  if (toolViews.has(view)) return "tools";
  return view;
}

export function allCommandNavigation(): NavigationItem[] {
  return [...primaryNavigation, ...toolNavigation];
}
