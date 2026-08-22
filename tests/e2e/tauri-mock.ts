import type { Page } from "@playwright/test";

export async function installTauriMock(page: Page) {
  await page.addInitScript(() => {
    const callbacks = new Map<number, (payload: unknown) => void>();
    let nextCallbackId = 1;
    const emptyQuery = { items: [], totalItems: 0, page: 1, pageSize: 60, totalPages: 1 };
    const emptyFacets = {
      totalItems: 0,
      availableItems: 0,
      missingItems: 0,
      containers: [],
      tags: [],
      creators: [],
      series: [],
      sites: [],
      themes: [],
      folders: [],
    };
    const invoke = async (command: string) => {
      switch (command) {
        case "plugin:event|listen": return nextCallbackId++;
        case "plugin:event|unlisten":
        case "plugin:window|on_close_requested":
        case "plugin:webview|set_webview_zoom": return null;
        case "get_desktop_preferences": return { closeBehavior: "quit", startMinimized: false, notifyWhenBackgrounded: false, reduceMotion: false, highContrast: false, interfaceScalePercent: 100, playbackCompatibility: "ask" };
        case "get_desktop_runtime_status": return { trayAvailable: false };
        case "get_onboarding_status": return { completed: true, defaultDestination: "/tmp", engines: [] };
        case "list_jobs":
        case "detect_engines":
        case "list_history":
        case "list_library_sources":
        case "list_library_scan_jobs":
        case "list_library_smart_views":
        case "list_library_collections":
        case "list_library_playlists":
        case "list_continue_watching":
        case "list_library_watch_later":
        case "list_recently_watched": return [];
        case "get_default_destination": return "/tmp";
        case "get_engine_preferences": return { browserCookieSource: "none", browserProfile: null, proxyUrl: null, enableImpersonation: true, bandwidthLimitKib: null };
        case "get_queue_snapshot": return { settings: { concurrentDownloads: 2, paused: false, notifyOnCompletion: true, completionAction: "none" }, orderedJobIds: [], activeJobIds: [], pauseSupported: true, completionNotificationsSupported: true };
        case "get_library_privacy_status": return { configured: false, locked: false, autoLockMinutes: 15, privateSourceCount: 0, privateFolderCount: 0, privateItemCount: 0 };
        case "get_library_search_status": return { catalogItems: 0, indexedItems: 0, consistent: true };
        case "get_library_transcript_index_status": return { indexedItems: 0, indexedCues: 0 };
        case "get_library_artwork_cache_status": return { fileCount: 0, totalBytes: 0 };
        case "get_library_playback_cache_status": return { fileCount: 0, totalBytes: 0, maxFiles: 4, maxBytes: 8_589_934_592 };
        case "get_library_visual_similarity_status": return { eligibleItems: 0, indexedItems: 0, staleItems: 0 };
        case "get_library_visual_similarity_job":
        case "get_library_theme_job":
        case "get_library_duplicate_analysis":
        case "get_local_share_status": return null;
        case "get_library_health_summary": return { totalItems: 0, availableItems: 0, missingItems: 0, changedItems: 0, metadataPendingItems: 0, metadataFailedItems: 0, compatibilityRiskItems: 0, issues: [] };
        case "get_library_storage_plan": return { catalogBytes: 0, sources: [], folders: [], containers: [], exactDuplicateReclaimableBytes: 0, playbackCacheBytes: 0, artworkCacheBytes: 0, incomingKnownBytes: 0, incomingUnknownItems: 0, activeTransformationCount: 0, warnings: [] };
        case "get_library_theme_settings": return { enabled: false, model: "qwen2.5vl:7b", mode: "deep" };
        case "get_library_theme_provider_status": return { available: false, installedModels: [], selectedModelAvailable: false, detail: "Test provider unavailable" };
        case "get_library_theme_index_status": return { eligibleItems: 0, indexedItems: 0, staleItems: 0, themeCount: 0 };
        case "get_native_playback_status": return { available: false, active: null, detail: "Test native player unavailable" };
        case "get_library_transcription_provider_status": return { available: false, executable: null, detail: "Test transcription provider unavailable" };
        case "get_library_semantic_transcript_index_status": return { model: "", indexedItems: 0, staleItems: 0 };
        case "get_library_local_inference_job": return null;
        case "query_library_items": return emptyQuery;
        case "get_library_facets": return emptyFacets;
        default:
          if (command.startsWith("list_")) return [];
          throw new Error(`Unhandled Playwright Tauri command: ${command}`);
      }
    };
    Object.assign(window, {
      __TAURI_INTERNALS__: {
        callbacks,
        invoke,
        transformCallback(callback: (payload: unknown) => void, once = false) {
          const id = nextCallbackId++;
          callbacks.set(id, once ? (payload) => { callbacks.delete(id); callback(payload); } : callback);
          return id;
        },
        unregisterCallback(id: number) { callbacks.delete(id); },
        runCallback(id: number, payload: unknown) { callbacks.get(id)?.(payload); },
        convertFileSrc(path: string) { return path; },
        metadata: {
          currentWindow: { label: "main" },
          currentWebview: { label: "main" },
        },
      },
    });
    window.localStorage.setItem("nice-downloader.view", "home");
  });
}
