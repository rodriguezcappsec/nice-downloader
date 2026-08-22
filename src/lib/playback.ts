import type { AppError } from "../types";

const FALLBACK_ELIGIBLE_ERRORS = new Set([
  "embedded_playback_decode_failed",
  "embedded_playback_timeout",
  "live_transcode_unavailable",
  "segmented_playback_unavailable",
]);

export const EMBEDDED_PLAYBACK_TIMEOUT_MS = 8_000;

export function createPlaybackError(code: string, message: string): AppError {
  return { code, message };
}

export function canOfferPlaybackFallback(error: AppError | null): boolean {
  return error != null && FALLBACK_ELIGIBLE_ERRORS.has(error.code);
}
