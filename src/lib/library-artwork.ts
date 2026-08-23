import { api } from "./tauri";

const MAX_ARTWORK_ENTRIES = 1_000;
const MAX_CONCURRENT_ARTWORK_REQUESTS = 6;

const resolvedArtwork = new Map<string, string | null>();
const pendingArtwork = new Map<string, Promise<string | null>>();
const artworkQueue: Array<() => void> = [];
let activeArtworkRequests = 0;
let artworkGeneration = 0;

function rememberArtwork(itemId: string, value: string | null) {
  resolvedArtwork.delete(itemId);
  resolvedArtwork.set(itemId, value);
  while (resolvedArtwork.size > MAX_ARTWORK_ENTRIES) {
    const oldest = resolvedArtwork.keys().next().value;
    if (!oldest) break;
    resolvedArtwork.delete(oldest);
  }
}

function runNextArtworkRequest() {
  while (
    activeArtworkRequests < MAX_CONCURRENT_ARTWORK_REQUESTS
    && artworkQueue.length > 0
  ) {
    activeArtworkRequests += 1;
    artworkQueue.shift()?.();
  }
}

function scheduleArtworkRequest(itemId: string): Promise<string | null> {
  return new Promise((resolve, reject) => {
    artworkQueue.push(() => {
      void api.getLibraryArtwork(itemId)
        .then((value) => {
          const versioned = value
            ? `${value}&clientRevision=${artworkGeneration}`
            : null;
          rememberArtwork(itemId, versioned);
          resolve(versioned);
        })
        .catch(reject)
        .finally(() => {
          activeArtworkRequests = Math.max(0, activeArtworkRequests - 1);
          pendingArtwork.delete(itemId);
          runNextArtworkRequest();
        });
    });
    runNextArtworkRequest();
  });
}

export function loadLibraryArtwork(itemId: string): Promise<string | null> {
  if (resolvedArtwork.has(itemId)) {
    const value = resolvedArtwork.get(itemId) ?? null;
    resolvedArtwork.delete(itemId);
    resolvedArtwork.set(itemId, value);
    return Promise.resolve(value);
  }
  const pending = pendingArtwork.get(itemId);
  if (pending) return pending;
  const request = scheduleArtworkRequest(itemId);
  pendingArtwork.set(itemId, request);
  return request;
}

export function invalidateLibraryArtwork(itemId?: string) {
  if (itemId) {
    resolvedArtwork.delete(itemId);
    pendingArtwork.delete(itemId);
    return;
  }
  artworkGeneration += 1;
  resolvedArtwork.clear();
  pendingArtwork.clear();
}
