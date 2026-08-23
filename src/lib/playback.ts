import type { AppError } from "../types";

const FALLBACK_ELIGIBLE_ERRORS = new Set([
  "embedded_playback_decode_failed",
  "embedded_playback_network_failed",
  "embedded_playback_source_failed",
  "embedded_playback_unknown_failed",
  "embedded_playback_timeout",
  "live_transcode_unavailable",
  "segmented_playback_unavailable",
]);

export const EMBEDDED_PLAYBACK_TIMEOUT_MS = 8_000;

export interface PlaybackMediaFailure {
  code: string;
  message: string;
  shouldAdvanceFallback: boolean;
  shouldRecordCompatibilityFailure: boolean;
}

interface PlaybackSourceIdentity {
  activeSessionUrl: string | null;
  eventSessionUrl: string;
  currentSrc: string;
  declaredSrc: string;
  baseUrl?: string;
}

export function createPlaybackError(code: string, message: string): AppError {
  return { code, message };
}

export function canOfferPlaybackFallback(error: AppError | null): boolean {
  return error != null && FALLBACK_ELIGIBLE_ERRORS.has(error.code);
}

export function isCurrentPlaybackSource({
  activeSessionUrl,
  eventSessionUrl,
  currentSrc,
  declaredSrc,
  baseUrl,
}: PlaybackSourceIdentity): boolean {
  if (!activeSessionUrl || activeSessionUrl !== eventSessionUrl) return false;

  const elementUrl = currentSrc || declaredSrc;
  if (!elementUrl) return false;

  return normalizePlaybackUrl(elementUrl, baseUrl) === normalizePlaybackUrl(eventSessionUrl, baseUrl);
}

export function classifyPlaybackMediaError(
  mediaErrorCode: number | null | undefined,
  compatibilityStream: boolean,
): PlaybackMediaFailure {
  const streamLabel = compatibilityStream ? "compatibility stream" : "video";
  switch (mediaErrorCode) {
    case 1:
      return {
        code: "embedded_playback_interrupted",
        message: `The current ${streamLabel} was interrupted before playback could continue. Try playing it again or use the native player.`,
        shouldAdvanceFallback: false,
        shouldRecordCompatibilityFailure: false,
      };
    case 2:
      return {
        code: "embedded_playback_network_failed",
        message: `The current ${streamLabel} stopped because its local playback connection was interrupted. This does not mean the codecs are unsupported.`,
        shouldAdvanceFallback: true,
        shouldRecordCompatibilityFailure: false,
      };
    case 3:
      return {
        code: "embedded_playback_decode_failed",
        message: compatibilityStream
          ? "The WebView stopped decoding the current H.264/AAC compatibility stream. Try libmpv, a cached playback copy, or your system player."
          : "The WebView stopped decoding this file's container or codecs. The original file is unchanged and can still be opened with your system player.",
        shouldAdvanceFallback: true,
        shouldRecordCompatibilityFailure: true,
      };
    case 4:
      return {
        code: "embedded_playback_source_failed",
        message: compatibilityStream
          ? "The WebView rejected the current compatibility stream. It may have ended unexpectedly or become unavailable."
          : "The WebView rejected this media source. Its container or codecs may be unsupported, or the local playback resource may no longer be available.",
        shouldAdvanceFallback: true,
        shouldRecordCompatibilityFailure: true,
      };
    default:
      return {
        code: "embedded_playback_unknown_failed",
        message: `The current ${streamLabel} stopped for an unknown media-engine reason. Nice Downloader did not classify this as a codec failure.`,
        shouldAdvanceFallback: true,
        shouldRecordCompatibilityFailure: false,
      };
  }
}

function normalizePlaybackUrl(url: string, baseUrl?: string): string {
  try {
    return new URL(url, baseUrl || (typeof document === "undefined" ? undefined : document.baseURI)).href;
  } catch {
    return url;
  }
}
