# Nice Downloader Architecture

## Intelligent workflow persistence

Migration 0003 adds `site_preset_rules` and `download_archive`. Site rules associate a saved typed preset with a normalized host suffix and optional path prefix; matching never evaluates regular expressions or executable actions. Completed downloads are upserted into the archive independently of History rows, so record-only History deletion does not disable destination-scoped duplicate detection.

Filename templates remain typed intent. Rust rejects empty templates, path separators, control characters, and values longer than 240 characters before spawning an engine. SponsorBlock categories are represented by a Rust enum and mapped to a fixed yt-dlp category allowlist.

## Subscription scheduler

Subscriptions persist a channel or playlist URL, preset, destination, bounded recent-entry limit, and check interval. A Rust-owned scheduler loads due records once per minute, expands at most 20 recent entries through structured yt-dlp output, analyzes each entry, consults the durable download archive, and enqueues typed requests. The frontend can request an immediate check but never owns timers or deduplication state.

Nice Downloader is a Linux-first Tauri desktop media hub that combines a non-destructive local library with a stable, task-oriented interface over mature media tools. React renders the desktop experience; Rust owns every privileged or authoritative operation.

## Design principles

- **Intent over flags.** The UI submits a typed `DownloadOptions` value. Engine adapters translate that intent into safe argument arrays.
- **Rust is authoritative.** Analysis, process execution, validation, queue state, cancellation, progress normalization, history, and engine discovery live in the Tauri backend.
- **No shell interpolation.** Engines are started directly with `tokio::process::Command`; URLs and metadata are always treated as untrusted data.
- **Structured data first.** Analysis uses engine JSON (`yt-dlp --dump-single-json` or bounded gallery-dl events); progress uses explicit engine templates.
- **Replaceable engines.** The application models normalized media and download concepts separately from yt-dlp. Gallery-dl and aria2 adapters can be added behind the same routing boundary.
- **Portable core, Linux-first integration.** Platform-neutral models and services sit behind small filesystem, executable-discovery, and process layers.

## System boundaries

```text
React views and Zustand store
        |
        | typed Tauri commands and events
        v
Command layer (validation and DTO boundary)
        |
        +--> Media analyzer/router --> yt-dlp or gallery-dl adapter
        +--> Queue scheduler -------> engine process + signals/cancellation
        +--> Managed engines -------> verified release + pin/rollback
        +--> Engine discovery ------> PATH/version probes
        +--> History repository ----> SQLite migrations and queries
        |
        v
Normalized `DownloadJob` events back to React
```

The frontend never receives a generic command-execution primitive. Tauri capabilities are limited to the application window and the native directory picker.

## Backend modules

- `commands`: narrow Tauri endpoints for analysis, engines, queue operations, defaults, and history.
- `models`: serializable application models shared conceptually with TypeScript.
- `engines`: routing policy and isolated adapters. yt-dlp handles ordinary media URLs and explicit HLS/DASH manifests; gallery-dl handles supported Bunkr, Imgur, Flickr, and Pinterest collection URLs.
- `queue`: authoritative ordered pending queue, dynamic concurrency (default 2), scheduling, queue/job pause, child lifecycle, cancellation tokens, retry lineage, startup recovery, disk preflight, and normalized events.
- `database`: SQLite connection, embedded migrations, durable job intent/diagnostics, settings, and history repository.
- `creative`: Rust-owned, serialized, timeout-bounded storyboard generation from validated catalog videos. Clip exports reuse `transformations` for cancellable FFmpeg execution and staged output publication.
- `visual_similarity`: cancellable, bounded FFmpeg sampling and derived dHash indexing keyed only by catalog IDs and current source fingerprints.
- `theme_analysis`: opt-in sequential local-VLM classification through fixed loopback Ollama, with bounded FFmpeg contact sheets, untrusted-content prompting, validated evidence, cancellation, and fingerprint-aware derived themes.
- `local_share`: memory-only, capability-token LAN serving for a bounded set of revalidated catalog media; no arbitrary path authority crosses the command boundary.
- `errors`: typed user-facing errors with optional diagnostic details.

## Frontend modules

- `components`: reusable desktop UI primitives and shell.
- `features/new-download`: URL analysis and intent construction.
- `features/downloads`: live jobs and cancellation.
- `features/history`: completed-download records.
- `features/settings`: engine discovery, optional capability status, and typed site-access preferences.
- `stores`: Zustand state synchronized from Tauri commands/events.
- `types`: TypeScript mirrors of the command/event contracts.

Page components remain mounted and are hidden by navigation so in-progress analysis and collection selections are not lost. See `docs/UX.md` for the interaction contract, semantic status colors, recovery links, and progressive-disclosure rules.

## Download lifecycle

Jobs progress through explicit states (with `Paused` available while waiting or running):

```text
Queued -> Analyzing -> Preparing -> Downloading
                                  -> PostProcessing -> Merging
                                  -> Completed | Failed | Cancelled
```

Milestone 1 analyzes before enqueueing in the UI, but the queue retains the `Analyzing` state for future jobs created from less interactive entry points. A scheduler owns pending order and active slots. On Linux, active process groups pause/resume through `SIGSTOP`/`SIGCONT`; cancellation terminates and reaps the group. Enqueued requests and state transitions are persisted. Jobs interrupted by shutdown are reconciled to `Queued`; failed and cancelled jobs retain typed intent for explicit retry. HTTP 429/5xx, connection reset, timeout, and incomplete-read failures receive at most two automatic follow-up attempts with bounded delay. Authentication, unsupported URL, and other permanent failures never retry automatically.

After an engine exits successfully, media files enter `Verifying`. A bounded ffprobe JSON probe must find at least one audio/video stream to mark the output verified. Missing ffprobe, non-media gallery outputs, and verification warnings remain visible in History rather than causing a downloaded file to be removed.

## Analysis and format selection

The yt-dlp adapter parses JSON metadata into tolerant internal structs, then returns normalized metadata plus simplified quality choices (`Best`, common resolution caps, and `Audio only`). Selection generates a conservative selector such as `bv*[height<=1080]+ba/b[height<=1080]`; yt-dlp and FFmpeg remain responsible for compatible stream merging.

Before analysis, a narrow URL resolver canonicalizes `redd.it/<post-id>` links and Reddit `/r|user/.../s/<token>` share links that yt-dlp's Reddit extractor does not currently match. Redirects are bounded and restricted to Reddit-owned HTTPS/HTTP hosts, and tracking parameters are removed before yt-dlp runs. Native Reddit video can fall back to validated public embed metadata when Reddit blocks its JSON API.

Batch analysis accepts at most 20 unique URLs and runs three analyzers concurrently. Playlist expansion is a separate structured yt-dlp flat-playlist operation capped at 20 entries; each expanded URL then passes through the ordinary analyzer and typed download-intent path. Bulk enqueue remains a Rust command so queue authority never moves into React.

Exact `bunkr.cr/a/<album-id>` albums and `bunkr.cr/f|v|i|d/<media-id>` single-media pages route to gallery-dl. Initial album analysis is bounded to the album summary and first item; single-media analysis is bounded to one item, and signed CDN media URLs are discarded in both paths. Album browsing lazily resolves paced 12-file pages rather than signing an entire album, returning only sanitized filenames, indices, sizes, extensions, and thumbnails. Rust converts selected album indices into a validated gallery-dl range, while single-media pages reject forged album selections. Downloads preserve original files, albums use a sanitized nested directory, and gallery-dl item and byte progress is normalized into the same `DownloadJob` events as yt-dlp. Migration 0028 persists a bounded per-file gallery lifecycle without media URLs. A failed album exposes only Rust-observed failed numeric indices for retry; completed and skipped files are never included in that retry request.

Explicit Imgur `/a|gallery/<id>`, Flickr `/photos/<user>/albums|galleries/<id>`, and Pinterest `/<user>/<board>` collections use the same bounded gallery-dl workflow. Routing accepts only these tested collection shapes; arbitrary gallery-dl-supported hosts continue through the ordinary analyzer until a typed adapter is added. Bunkr retains its narrower host, route, identifier, and single-media validation.

Manifest classification runs before direct-file classification. Explicit `.m3u8` and `.mpd` paths, plus signed endpoints whose typed query values identify HLS or DASH, route through yt-dlp with their full query preserved. Manifest metadata is rejected with separate typed errors when it reports DRM or a currently live stream; engine diagnostics distinguish unavailable HLS keys/encryption from malformed or unavailable manifests. Diagnostics redact signed-query credentials independently from the URL passed to the engine.

Recognized direct HTTP file URLs route to the direct-transfer adapter. It prefers aria2c and falls back to curl, with resume, redirect following, bounded retry, optional typed proxy configuration, and filesystem-based normalized progress. These transfers bypass extraction and conversion.

Transfer reliability is covered by a loopback-only integration harness. A controlled HTTP fixture produces 429 responses, valid byte ranges, and deliberately truncated bodies while the test runs the real curl executable. Retries happen as fresh Rust-owned process attempts: this is intentional because curl's same-process retry can request the correct range yet truncate the existing output on some versions. Run the harness with `npm run test:integration`.

## Persistence

Terminal downloads have persistent Downloads-view visibility. Dismissing a completed, failed, or cancelled job hides it from Downloads without deleting its completed History row, archive identity, library item, or media file.

SQLite is stored in Tauri's application-data directory. Numbered embedded migrations create `downloads`, `presets`, and `settings`. Download rows store normalized display state, the serialized typed request, bounded sanitized diagnostics, retry lineage, and timestamps. Completed rows feed History; non-completed rows rebuild the runtime queue after restart.

Presets store reusable intent fields, not engine flags. Queue concurrency, queue pause, and notification preference are stored as typed JSON settings.

## Security and reliability

- Only HTTP(S) URLs are accepted in milestone 1.
- Output destinations must be existing, absolute directories.
- Filenames are generated by yt-dlp from a fixed safe default template unless the typed intent supplies an approved template later.
- Executable paths are discovered from PATH and passed directly to `Command`.
- Engine stderr is sanitized and returned as expandable diagnostics without logging sensitive request headers or signed URL tokens.
- Browser-cookie access is opt-in and represented by an allowlisted browser enum. Rust gives yt-dlp only the browser name; cookie contents never cross the Tauri boundary or enter application logs.
- Optional browser profile names reject path separators, and proxy configuration accepts only credential-free HTTP(S)/SOCKS URLs.
- Destination folders receive a create/remove write probe before enqueue. Existing-output behavior is explicit skip or overwrite intent mapped to fixed yt-dlp arguments.
- Known output sizes receive a free-space preflight with a 10 percent and 256 MiB temporary-file margin.
- A completed URL/destination pair is rejected under the `Skip` policy to prevent accidental duplicate downloads; choosing `Overwrite` is the explicit override.
- Browser impersonation is applied only when yt-dlp reports a compatible runtime target. Settings distinguishes executable availability from optional capability readiness.
- Production paths return typed errors rather than panicking.
- Structured tracing records engine lifecycle without logging secrets.
- Open/reveal commands accept a download ID rather than an arbitrary executable or shell command.
- Diagnostic exports are backend-generated and exclude URL paths/queries, cookie contents, proxy values, profile names, and request headers.
- Managed yt-dlp/gallery-dl installs require HTTPS vendor releases, a published SHA-256 match, and a successful version probe. The previous version remains available for rollback; pinning blocks updates.

## Near-term extension points

## Mature workflow services

- `subscriptions`: Rust-owned periodic playlist expansion, archive deduplication, and typed queue creation.
- `transformations`: FFmpeg task ownership, structured `-progress` parsing, cancellation, crash reconciliation, independent SQLite history, and Rust-resolved multi-source montage rendering.
- `library`: non-destructive local-media catalog, safe `.info.json` normalization, tagging, missing-file reconciliation, and versioned metadata backup/restore.

Library scans write catalog identity immediately, then enqueue IDs into a globally bounded metadata pipeline. ffprobe runs without a shell, emits JSON, and has a per-file timeout. Normalized duration, dimensions, FPS, codecs, container, HDR range, and the size/mtime fingerprint live in `library_media_metadata`; a changed fingerprint resets the record to pending while unchanged records retain their probe result. Artwork is generated into the application cache with a fingerprinted filename and can be cleared independently of source media.

Library curation is persisted separately from filesystem scanning. Favorites and ratings live on catalog items; collections use dedicated collection and membership tables. Bulk curation commands validate a bounded set of UUIDs and execute in a single SQLite transaction. Rescanning an existing path preserves its title, tags, favorite, rating, and collection membership.

Library browsing is also SQLite-authoritative. A typed query command composes parameterized search, source/folder depth, media metadata, availability, tag, rating, collection, and favorite filters, returning at most 60 records with an exact count and clamped page. Sort intent is a closed enum mapped to static SQL clauses with deterministic ID tie-breakers; unknown duration and size values remain after inspected media. Separate facet queries return only aggregate availability, container/tag choices, and folder counts. React keeps the current page, three bounded 12-item Home shelves, and up to 500 selected IDs; it never loads the full catalog for browsing or filtering. Indexed-item event bursts trigger one debounced refresh, while terminal scan events refresh queries, facets, shelves, and source status.

Opt-in library watching is owned by `LibraryManager`. The native watcher coalesces event bursts before requesting a normal bounded scan; it never applies filesystem mutations. A lightweight source monitor detects unavailable watched volumes, emits normalized source status events, recreates watchers when volumes return, and schedules a source-scoped reconciliation scan. React can enable or disable watching but does not own watcher tasks or scan state.

Reconciliation stores stable filesystem identity separately from user-facing metadata. On Unix, a unique device/inode match can preserve the existing item ID and curation across a rename, but only after confirming the previous path is gone. Added, modified, renamed, missing, and restored transitions become bounded review records delivered through normalized events.

Missing catalog records are retained for removable-volume and move reconciliation but excluded from normal Library browsing. Scans, artwork lookup, and failed open attempts persist the missing transition and emit the same normalized item/change events. Users can reveal these recovery records with the Missing files filter and explicitly forget one without modifying disk.

Library backup format version 6 carries explicit catalog and media-metadata schema versions plus source definitions, items, curation, collection membership, ordered video playlists, media stacks, moments, and montage projects. Restore treats JSON as bounded untrusted input, validates the complete shape before writes, remaps source and catalog identities across every dependent record, and imports older supported backups without dangling references. It is a non-destructive merge; on-disk fingerprints decide whether technical metadata can be reused or must return to Pending for reprobe.

Montage projects live in `library_montages` and ordered `library_montage_segments`. They persist only catalog UUIDs and normalized clip intent. Export revalidates every video, resolves paths in Rust, and hands a private resolved-input specification to the Transformation manager. The serialized public operation contains only segment count, duration, and fade metadata; generic frontend transformations cannot construct a montage execution. FFmpeg argument arrays normalize video/audio streams, synthesize silence when a source lacks audio, concatenate into H.264/AAC MP4, and retain the existing cancellable staged-publication lifecycle.

Generated Library artwork is an app-cache concern, never source-folder state. A Rust-owned maintenance lock coordinates status, clearing, and post-generation pruning with both FFmpeg artwork permits. Cache reads refresh filesystem recency; pruning keeps no more than 20,000 regular preview files or 1 GiB and removes oldest entries first while protecting the output serving the active request.

Embedded playback uses the platform WebView's HTML5 media engine. Rust starts an ephemeral HTTP/1 loopback server on `127.0.0.1` with a process-scoped token, resolves only UUID catalog IDs, rejects symlinks and missing/non-video records, and streams regular files with bounded request parsing and byte-range support. Each preparation issues a unique, non-authoritative session query value so WebKitGTK cannot reuse stale media-resource state when the same catalog item is reopened; authorization and path resolution still depend only on the process token and catalog ID. The server never accepts a filesystem path from React and its token is not persisted or logged. The CSP permits media only from the randomized loopback endpoint; opening through the operating-system default player remains a separate ID-based command.

An optional Rust-owned native mpv backend sits before compatibility-copy fallbacks. React supplies only a catalog UUID; Rust revalidates video kind, availability, and an absolute regular-file path before spawning a discovered `mpv` executable with a fixed argument array, `--no-config`, a random process-private IPC socket, and the media path after `--`. One native session may run at a time. The backend polls mpv time/duration through IPC and persists bounded resume state through the same SQLite playback record. Linux parent-death signaling and explicit quit handling prevent orphaned playback. Wayland uses mpv's own native window because portable foreign-surface embedding is unavailable; this still avoids remuxing or transcoding and isolates decoder failures from the WebView.

The libmpv experiment documented in `docs/experiments/libmpv-gtk.md` proves AV1 + AAC rendering in an application-owned GTK `GLArea` under native Wayland. Its opt-in phase-two path reparents the Tauri WebView into a GTK overlay and positions an input-pass-through libmpv surface from bounded React geometry while Rust retains catalog-ID path authority. WebView/copy/external-player fallbacks remain intact; packaging, license review, broader distribution coverage, and automated interaction/accessibility proof remain gates before this can become a default player.

Linux AppImages stage a curated GStreamer plugin set and matching
`gst-plugin-scanner` before the final `linuxdeploy` dependency pass. A small
AppRun wrapper pins WebKitGTK to that packaged runtime rather than mixing the
bundled GStreamer core with host plugins from another distribution. Rust checks
the required audio-output element before compatibility preflight, so a missing
runtime is reported before any copy consent. React retains the typed error code
and offers H.264 fallback only for decode or readiness failures.

Playback preparation probes ISO Base Media video timing before returning a session. Affected AV1 MP4/M4V/MOV tracks whose fractional frame rate WebKitGTK's QuickTime demuxer exposes as `0/1` are stream-copied by FFmpeg into an app-private Matroska compatibility cache. This preserves the encoded AV1, audio, frame count, and duration while routing timing through GStreamer's Matroska demuxer. Cache identity uses only the UUID catalog ID and current source fingerprint, not untrusted filenames. Rust registers resulting paths per catalog item so overlapping or stale preparation requests cannot displace the active video; the loopback route still accepts only an item ID and rejects registered paths outside the cache. The cache retains at most four files or 8 GiB, invalidates changed sources, and never modifies the original media.

Compatibility creation uses a two-step Rust-owned consent boundary. `inspect_library_playback` returns typed preflight data without running FFmpeg; `prepare_library_playback` enforces the persisted Ask/Automatic/Never policy again and requires an explicit approval bit when policy demands it. Cached targets bypass repeat prompts, while targets at or above 2 GiB always require approval. React cannot manufacture a filesystem target or weaken this check. Cache status and clear commands operate only on the app-private compatibility directory, and desktop preferences are persisted through the existing settings store.

Playback-copy preparation remains Rust-owned but is observable and cancellable. At most two FFmpeg preparations run concurrently; additional requests report a queued phase instead of waiting invisibly. React polls a typed progress record and may request cancellation by catalog UUID. Releasing a player also cancels its active preparation. The cache lock covers reservation, pruning, and atomic publication only—not the complete FFmpeg run—while active partial files are protected from pruning and removed after cancellation, failure, or interrupted-startup recovery.

If the WebView rejects a valid lossless Matroska timing copy, a distinct fallback preflight may offer an H.264/AAC MP4 derivative. This path is never authorized by the Automatic stream-copy preference: it always requires explicit consent unless the fingerprinted H.264 result already exists. Rust estimates a conservative four-times-source cache reservation, enforces free-space and 8 GiB limits, uses FFmpeg argument arrays, and registers the finished MP4 through the same catalog-ID-only loopback boundary. A successful H.264 fallback replaces the failed per-item Matroska derivative but never changes the source.

Migration `0015_library_playback.sql` stores one normalized progress record per catalog item and cascades with catalog deletion. React writes finite position/duration snapshots at bounded playback intervals, pause, close, and completion. SQLite derives the bounded Continue Watching shelf from available, incomplete video records. Only same-stem `.vtt` sidecars are exposed, and each subtitle request revalidates its catalog parent and filename boundary.

The Library player is mounted once per selected catalog item and switches between full Watch workspace and floating mini-player layouts without replacing the HTML `<video>` node. The mini player can be moved and resized within the viewport using pointer or keyboard controls, and its last frame is stored locally for the next session. Library-to-home and cross-workspace navigation therefore preserve the active media session, buffering, audio, subtitle selection, playback rate, and native control state. Restoring the mini player returns to the Library Watch workspace; closing it persists progress and releases the session. Compact mode does not register app-wide media shortcuts, preventing playback keys from interfering with other workspaces.

Duplicate analysis is an explicit, cancellable `LibraryManager` task. It narrows candidates by size, streams files through SHA-256 without following symlinks, verifies the fingerprint before and after reading, and caches only current hashes. React renders exact-content evidence and progress but cannot bypass the backend cleanup policy.

Exact duplicate cleanup is a separate narrow command. It accepts only a current duplicate-group ID and member ID, re-derives the group from current SQLite fingerprints, requires another available member, and re-hashes the selected and retained files before invoking the platform Trash API. The catalog row is removed only after that recoverable operation succeeds; no permanent-delete command is exposed.

Library organization uses persisted typed plans. React can request a Move, Copy, or Rename preview for a catalog item, but execution accepts only the resulting opaque operation ID. Bulk Move accepts at most 500 unique catalog IDs and one absolute destination directory; every item, filename collision, fingerprint, no-clobber destination, and filesystem boundary is preflighted before execution starts. `LibraryManager` revalidates the regular-file source, catalog fingerprint, destination parent, filename extension, no-clobber condition, and journal state. SQLite atomically claims each plan before filesystem work. Copy stages data inside the destination filesystem before no-clobber publication; Move and Rename are same-filesystem no-clobber transitions. Completed actions update the catalog and journal, and Undo revalidates both paths and the latest fingerprint before reversing a move/rename or sending a copied file to platform Trash. Startup marks interrupted operations for explicit verification and expires abandoned previews. Cross-filesystem Move is rejected instead of silently becoming copy-delete.

Folder browsing is presentation state derived from normalized `sourceId` plus `relativePath`. The selector exposes each source root and every ancestor folder with direct and descendant-aware counts. A depth scope chooses exact-directory or nested-path matching and combines with the source, media, curation, and metadata filters without granting React filesystem access.
- `authentication`: longest-host-suffix matching, browser-profile discovery, and backend-only managed Netscape cookie files. Managed paths use owner-only permissions on Unix and are omitted from frontend DTOs.

The tray and desktop settings form a small platform boundary. Window close policy is applied in the Tauri frontend while process survival, queue recovery, and persisted preferences remain backend-owned. Desktop preferences also carry validated interface scale, reduced-motion, and high-contrast intent. React applies the visual classes and uses Tauri's narrowly permitted webview zoom command; only 90%, 100%, 110%, and 125% are accepted by Rust.

Migration `0016_library_watch_later.sql` stores ordered video IDs with catalog-delete cascading. Watch Later and Recently Watched are separate bounded SQLite queries: the former reflects explicit user intent, while the latter derives from normalized playback timestamps. Watched/reset mutations validate the catalog media kind in Rust and update the same playback record used by Continue Watching. Autoplay remains presentation state and can request only the next catalog item already returned by the typed Library query.

Migration `0017_library_playlists.sql` stores named playlists and ordered, cascade-safe video membership. Rust owns playlist validation, the 500-item bound, add/remove/reorder transactions, and position normalization. React receives playlist IDs and catalog items through narrow Tauri commands; it never persists order or supplies paths. Playlist playback skips missing entries and stops at both boundaries. Version 2 backup/restore merges validated playlist membership without deleting current data or touching source media.

Migration `0018_library_rich_metadata.sql` stores normalized extracted sidecar fields and user overrides as separate JSON documents keyed by catalog item. Rust computes effective creator, description, upload date, series, episode, and original URL values; rescans replace only the extracted document. Parameterized SQLite queries expose source-aware creator/series/site facets, published-date filters, and static publication sort clauses. Custom artwork accepts only a catalog ID plus a native-picker path, then Rust validates an absolute regular non-symlink JPEG/PNG/WebP. Backup/restore carries bounded override records and remaps their item IDs through the validated catalog restore map.

Migration `0019_library_smart_discovery.sql` adds a trigger-maintained FTS5 table and typed saved smart views. The search table contains only normalized catalog and rich-metadata text; every media/rich insert, relevant update, or delete replaces or removes the corresponding row inside the same SQLite transaction. Rust converts bounded user text into quoted prefix terms, while multi-value facets remain parameterized SQL. Search health compares catalog/index row counts and item-ID sets; rebuild deletes and repopulates the index transactionally. Smart views serialize validated `LibrarySmartViewQuery` intent and are reapplied through the normal query command, never as stored SQL or frozen item IDs.

The comparison workspace stores only an ephemeral ordered set of at most four catalog item IDs in Zustand. It does not persist media paths or playback URLs. Each tile independently requests the existing Rust playback preflight and session commands, and Rust continues to validate media kind, availability, paths, compatibility consent, cache limits, and system-player access. Shared play/pause/seek and audio-focus behavior operates on already-authorized HTML video elements. Unmount releases every Rust playback registration; compatibility generation remains serialized by the media-server lock.

## Media organizer boundary

The Library can index multiple user-selected folders independently of download destinations. Sources, durable scan jobs, and catalog items are persisted in SQLite. A Rust-owned scanner performs bounded directory walks, ignores symlinks by default, classifies supported video and image extensions, and emits normalized progress events. Files remain in place; scanning and record removal never mutate external media.

The default Library presentation is a browse-first media hub. React may request artwork only by catalog item ID. Rust validates the stored item, resolves FFmpeg internally, limits concurrent artwork work, and returns a bounded cached JPEG data URL; the webview never receives arbitrary filesystem read access. Opening media likewise accepts an item ID and resolves the persisted path in Rust.

`docs/MEDIA_ORGANIZER.md` is the source of truth for organizer milestones, safety invariants, acceptance criteria, and deferred scope.

Full-resolution image viewing shares the tokenized loopback boundary through a separate `/image/{catalogId}` route. Rust requires an image catalog record, revalidates a regular non-symlink file on every request, and maps only JPEG, PNG, WebP, GIF, and AVIF extensions to `nosniff` response types. React receives a fresh session URL and never a source path.

Image comparison keeps only an ephemeral ordered set of two to four catalog IDs in Zustand. Every image independently receives a Rust-authorized loopback session; synchronized zoom, pan, overlay opacity, and split position remain presentation state and are never persisted. Persistent version grouping is a separate SQLite concept: `library_media_stacks` owns name, notes, and cover while normalized members own order and a typed role. Rust validates same-kind available records and single-stack membership. Version 5 Library backup/restore remaps stack members and curation decisions through the catalog restore map and never moves or copies source media.

Rapid curation stores only typed `keep` or `reviewLater` decisions in `library_curation_decisions`; missing or deleted catalog entries never remain actionable. The React workspace requests a bounded Rust-generated queue and performs favorite, rating, and tag changes through existing transactional curation commands. Its Move shortcut transfers only a catalog ID into the established previewable file-operation workflow. Automatic collections reuse validated `LibrarySmartViewQuery` persistence, so SQLite recomputes membership and React cannot store arbitrary SQL.

Migration `0024_library_transcripts.sql` stores normalized cues from trusted same-stem WebVTT sidecars in a local FTS5 table. Rust owns directory discovery, rejects symlinks and oversized or non-UTF-8 tracks, bounds tracks, cues, cue length, search text, and result count, and converts search text through the existing quoted-prefix FTS builder before parameterized execution. Normal source scans replace each video's cues; a manual rebuild walks all available videos by stable catalog-ID cursor in 500-item pages rather than using the legacy 5,000-item list bound. Catalog deletion cascades transcript rows. React receives only catalog IDs, normalized text, optional language, and finite cue timestamps, then enters the existing authorized playback flow. Embedded playback seeks after metadata loads; native mpv receives a validated optional start timestamp through its fixed argument-array launch path. No arbitrary path or process authority crosses into React. Speech-to-text remains opt-in and deferred.

Migration `0026_library_visual_similarity.sql` is a rebuildable derived index, not user data. `VisualSimilarityManager` resolves only available catalog IDs, rejects symlinks and non-regular files, bounds FFmpeg representative-frame work with cancellation and timeouts, and saves a 64-bit dHash only with the observed source size/mtime fingerprint. Queries exclude stale fingerprints and compare a bounded current set in Rust. Approximate Hamming-distance scores never participate in exact-duplicate cleanup, and signatures are intentionally absent from backup format version 6.

`LocalShareManager` owns a separate ephemeral LAN HTTP listener. One memory-only session authorizes at most 50 catalog IDs through a random capability token for at most 120 minutes. Each listing or media request checks the active session and expiry; each media request re-resolves the catalog record and validates the current regular, non-symlink path before serving a closed MIME set with GET, HEAD, or one bounded byte range. React receives only the generated capability URL and session metadata. The token is not persisted or logged, the server accepts no paths or mutations, and app shutdown stops it. This boundary prevents unintended catalog disclosure, but it cannot prevent an authorized recipient from retaining streamed bytes.

Lean-back playback is presentation state inside the existing Watch workspace. React retains the same keyed `EmbeddedVideoPlayer` and authorized loopback session while changing its surrounding layout, so current time and native media state are not serialized or recreated. Keyboard and edge-triggered Gamepad API actions invoke the same bounded previous/next, seek, play, and exit functions. Playlist ends remain non-looping, and no gamepad or lean-back preference crosses into SQLite or Library backup.

Milestone 26 presentation state remains outside Library backup. A versioned, bounded `localStorage` workspace snapshot contains only validated query/presentation values, scroll offset, and an optional catalog ID. The session-only Play Next Zustand queue stores at most 100 unique catalog IDs; every materialization returns through `get_library_items_by_ids`, removes missing or non-video records, and never persists a playlist implicitly. The command palette uses the existing typed Library query with an eight-result bound and translates actions into catalog-ID events consumed by already-mounted workspaces.

Media Health is a read-only database projection with no migration or derived health table. One aggregate query counts catalog availability, pending change review, metadata state, and a conservative closed set of codec/container compatibility risks; a second query returns at most 100 ID/title issue samples. Search consistency, similarity coverage, source status, and cache usage continue to come from their authoritative existing managers. UI wording treats playback compatibility as advisory because installed WebKitGTK/GStreamer capabilities can differ.

Temporary-share QR rendering is lazy-loaded browser code operating only on the existing capability URL. It performs no network lookup and creates no additional authority: expiry, selected-ID authorization, byte-range serving, and session teardown remain wholly owned by `LocalShareManager`.

## Remaining extension points

The gallery-dl route can expand beyond the currently typed collection sites. Release builds stage pinned,
SHA-256-verified yt-dlp, gallery-dl, FFmpeg, and ffprobe resources. Runtime
resolution prefers app-managed updates, then the bundled baseline, then host
installations. Platform packaging, non-Linux pause/notification adapters, and
custom arguments remain later work. Additional authenticated-site support should
extend typed access preferences rather than exposing arbitrary engine flags.

## Live compatibility playback

Direct byte-range playback remains the first embedded-player choice. When preflight predicts a compatibility copy, or direct WebView playback reports a decode failure or stall, React advances through two Rust-owned compatibility sources: segmented HLS first, then fragmented MP4. Random stream IDs bind the catalog UUID to a finite logical start timestamp, and the loopback server revalidates the original regular, non-symlink video before spawning FFmpeg.

At most two compatibility FFmpeg processes run concurrently across both protocols. HLS writes atomic two-second H.264/AAC transport-stream segments and an event playlist into a UUID-scoped private cache directory; Rust rewrites only media lines with the process token, serves a closed filename grammar, clears sessions on release, and removes UUID-shaped orphans at startup. Fragmented MP4 writes to stdout, and dropping its HTTP body cancels and reaps the child immediately. Seeking creates a fresh authorization and stream at the selected original-media timestamp. React offsets progress and same-stem WebVTT cues so resume, transcript jumps, and persistence remain on the original video's timeline. Cached AV1 remuxes, full H.264 copies, embedded libmpv, native mpv, and the system player remain explicit recovery paths.
