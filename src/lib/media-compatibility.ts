import type { LibraryItem, LibraryPlaybackPreflight } from "../types";

export type EmbeddedCompatibilityKind = "expected" | "copy" | "native" | "unknown";

export interface EmbeddedCompatibilityAssessment {
  kind: EmbeddedCompatibilityKind;
  badge: string;
  heading: string;
  codecSummary: string;
  detail: string;
}

type CompatibilityItem = Pick<
  LibraryItem,
  "audioCodec" | "container" | "fileSizeBytes" | "id" | "mediaKind" | "metadataState" | "modifiedAtMs" | "videoCodec"
>;

export type EmbeddedPlaybackEvidence = "played" | "failed";

const canPlayCache = new Map<string, CanPlayTypeResult>();

const VIDEO_CODEC_IDS: Record<string, string> = {
  av1: "av01.0.08M.08",
  h264: "avc1.42E01E",
  h265: "hvc1.1.6.L93.B0",
  hevc: "hvc1.1.6.L93.B0",
  mpeg4: "mp4v.20.9",
  theora: "theora",
  vp8: "vp8",
  vp9: "vp09.00.10.08",
};

const AUDIO_CODEC_IDS: Record<string, string> = {
  aac: "mp4a.40.2",
  ac3: "ac-3",
  eac3: "ec-3",
  flac: "flac",
  mp3: "mp3",
  opus: "opus",
  vorbis: "vorbis",
};

const CONTAINER_MIME_TYPES: Record<string, string> = {
  "3g2": "video/3gpp2",
  "3gp": "video/3gpp",
  avi: "video/x-msvideo",
  flv: "video/x-flv",
  m2ts: "video/mp2t",
  mkv: "video/x-matroska",
  m4v: "video/mp4",
  mov: "video/mp4",
  mp4: "video/mp4",
  mpeg: "video/mpeg",
  mpg: "video/mpeg",
  mts: "video/mp2t",
  ogg: "video/ogg",
  ogv: "video/ogg",
  ts: "video/mp2t",
  webm: "video/webm",
};

export function assessEmbeddedCompatibility(
  item: CompatibilityItem,
  preflight?: LibraryPlaybackPreflight | null,
  evidence?: EmbeddedPlaybackEvidence | null,
): EmbeddedCompatibilityAssessment {
  const videoCodec = normalizeCodec(item.videoCodec);
  const audioCodec = normalizeCodec(item.audioCodec);
  const container = item.container?.trim().toLocaleLowerCase() || null;
  const codecSummary = formatCodecSummary(videoCodec, audioCodec, container);

  if (preflight?.compatibilityRequired) {
    const ready = preflight.compatibilityCached;
    return {
      kind: "copy",
      badge: ready ? "Copy ready" : "Copy needed",
      heading: ready ? "Embedded playback copy ready" : "Playback copy required",
      codecSummary,
      detail: ready
        ? "The original needs Nice Downloader's private compatibility copy. It is already cached and the original remains unchanged."
        : preflight.reason || "The original needs a private compatibility copy before it can play reliably in the embedded WebKit player.",
    };
  }

  if (item.metadataState === "pending") {
    return {
      kind: "unknown",
      badge: "Inspecting",
      heading: "Checking embedded playback",
      codecSummary,
      detail: "Codec metadata is still being inspected.",
    };
  }

  if (item.mediaKind !== "video" || !videoCodec || !container) {
    return unknownAssessment(codecSummary, "Codec or container metadata is unavailable, so embedded playback cannot be predicted yet.");
  }

  const mimeType = buildMimeType(container, videoCodec, audioCodec);
  if (!mimeType) {
    return unknownAssessment(codecSummary, "This container or codec combination does not have a reliable WebKit capability query.");
  }

  const capability = canPlayType(mimeType);
  if (evidence === "played") {
    return {
      kind: "expected",
      badge: "Played here",
      heading: "Verified in the embedded player",
      codecSummary,
      detail: "This exact file has already made sustained playback progress in the embedded player during this app session.",
    };
  }

  if (evidence === "failed") {
    return {
      kind: "native",
      badge: "Embedded failed",
      heading: "Embedded playback failed",
      codecSummary,
      detail: "This exact file failed or stalled in the embedded player during this app session. Use native mpv or create an H.264 playback copy.",
    };
  }

  if (!capability) {
    return {
      kind: "native",
      badge: "Native advised",
      heading: "Native player recommended",
      codecSummary,
      detail: "This WebKit runtime does not report support for the detected container and codecs. When available, native mpv can play the original without creating a copy.",
    };
  }

  if (videoCodec === "av1" || videoCodec === "hevc" || videoCodec === "h265") {
    return {
      kind: "native",
      badge: "Native advised",
      heading: "Native player recommended",
      codecSummary,
      detail: `${formatMediaCodec(videoCodec)} support in WebKitGTK depends on the installed GStreamer decoder and can still fail for individual files. Native mpv is the more reliable path when available.`,
    };
  }

  if (capability === "probably") {
    return {
      kind: "unknown",
      badge: "Likely compatible",
      heading: "Format likely compatible",
      codecSummary,
      detail: "WebKit reports generic support for this container and codec combination, but this exact file has not been verified by sustained playback yet.",
    };
  }

  return {
    kind: "unknown",
    badge: "Support uncertain",
    heading: "Embedded support uncertain",
    codecSummary,
    detail: "WebKit reports possible, but not definite, support for the detected container and codecs. Native mpv remains available if playback fails.",
  };
}

export function embeddedPlaybackFingerprint(item: CompatibilityItem): string {
  return `${item.id}:${item.fileSizeBytes ?? "unknown"}:${item.modifiedAtMs ?? "unknown"}`;
}

export function formatMediaCodec(codec: string | null | undefined): string | null {
  if (!codec) return null;
  const labels: Record<string, string> = {
    aac: "AAC",
    ac3: "AC-3",
    av1: "AV1",
    eac3: "E-AC-3",
    flac: "FLAC",
    h264: "H.264",
    h265: "HEVC",
    hevc: "HEVC",
    mp3: "MP3",
    mpeg4: "MPEG-4 Visual",
    opus: "Opus",
    theora: "Theora",
    vorbis: "Vorbis",
    vp8: "VP8",
    vp9: "VP9",
  };
  return labels[codec.toLocaleLowerCase()] || codec.toLocaleUpperCase();
}

function normalizeCodec(codec: string | null | undefined): string | null {
  return codec?.trim().toLocaleLowerCase() || null;
}

function formatCodecSummary(videoCodec: string | null, audioCodec: string | null, container: string | null): string {
  const streams = [
    videoCodec ? `${formatMediaCodec(videoCodec)} video` : null,
    audioCodec ? `${formatMediaCodec(audioCodec)} audio` : null,
  ].filter(Boolean);
  const containerLabel = container?.toLocaleUpperCase();
  return [...streams, containerLabel].filter(Boolean).join(" · ") || "Media format unknown";
}

function buildMimeType(container: string, videoCodec: string, audioCodec: string | null): string | null {
  const mimeType = CONTAINER_MIME_TYPES[container];
  const videoCodecId = VIDEO_CODEC_IDS[videoCodec];
  const audioCodecId = audioCodec ? AUDIO_CODEC_IDS[audioCodec] : null;
  if (!mimeType || !videoCodecId || (audioCodec && !audioCodecId)) return null;
  const codecs = [videoCodecId, audioCodecId].filter(Boolean);
  return `${mimeType}; codecs="${codecs.join(", ")}"`;
}

function canPlayType(mimeType: string): CanPlayTypeResult {
  const cached = canPlayCache.get(mimeType);
  if (cached != null) return cached;
  if (typeof document === "undefined") return "";
  const result = document.createElement("video").canPlayType(mimeType);
  canPlayCache.set(mimeType, result);
  return result;
}

function unknownAssessment(codecSummary: string, detail: string): EmbeddedCompatibilityAssessment {
  return {
    kind: "unknown",
    badge: "Support unknown",
    heading: "Embedded compatibility unknown",
    codecSummary,
    detail,
  };
}
