# Media Organizer Plan

This document is the implementation reference for evolving Nice Downloader into a local-first video and image organizer. Downloads and Library remain separate workflows: Downloads manages transfers, while Library indexes media wherever it already lives.

## Product and safety contract

- Index files in place by default. Scans must never move, rename, copy, or delete media.
- Support multiple folders, including removable and temporarily offline volumes.
- Rust owns filesystem access, scan state, metadata extraction, thumbnails, persistence, cancellation, and events. React only expresses user intent and renders state.
- Treat paths and embedded metadata as untrusted. Do not follow symlinks by default.
- Removing a source or catalog record must be clearly distinguished from deleting a file. Cleanup uses the system Trash, while organization requires a separate preview and confirmation; permanent deletion remains out of scope.
- A corrupt file, unreadable directory, or missing drive is a warning or source status, not a reason to discard the rest of the catalog.

## Experience contract

The Library is a media destination, not a database administration page. Its default view is browse-first: prominent search, visual media shelves, source shortcuts, large 16:9 artwork, and a focused item detail panel with a clear watch/view action. Folder scans, backup tools, and record maintenance live in a separate management mode so they do not compete with the media.

The interaction model should feel familiar to users of YouTube and local media centers while remaining private and desktop-native. Never invent engagement counts, recommendations, or viewing history. Add those surfaces only when the application has real local data to support them.

Media home is the product entry point. Downloading remains a first-class capability, but lives in a separate Downloads navigation group with New download, active transfers, Queue, History, and Subscriptions. Media Transform and download Presets live under Tools. The app opens maximized for a comfortable editor-like workspace; its restored fallback is 1440×900 and prioritizes readable artwork and four-column browsing.

## Scan pipeline

```text
Configured source
-> bounded directory walk
-> media classification
-> immediate catalog record
-> background ffprobe/image metadata
-> cached thumbnail
-> duplicate/reconciliation pass
-> normalized Library events
```

Fast rescans compare source, relative path, size, and modification time. Scan jobs are persisted so interrupted work is visible after restart. Progress events are incremental and bounded; the frontend is never the authoritative scan queue.

## Milestone 1 - External folders and indexing

Status: implemented on `feat/media-organizer`.

Deliver the first useful vertical slice:

1. Add append-only `library_sources` and `library_scan_jobs` migrations.
2. Associate catalog items with a source and relative path.
3. Add typed source CRUD commands and TypeScript contracts.
4. Implement a Rust-owned, cancellable, concurrency-safe scanner.
5. Index a conservative video/image extension allowlist without following symlinks.
6. Persist scan counts, warnings, source status, and last-scan time.
7. Add a Library Sources UI, scan/cancel controls, and live progress.
8. Preserve records when a volume is offline and reconcile missing files after successful scans.

Initial video formats: MP4, MKV, WebM, MOV, AVI, and M4V. Initial image formats: JPEG, PNG, WebP, GIF, and AVIF.

Acceptance: add an arbitrary folder, see supported files appear incrementally, cancel and restart a scan, relaunch without losing the source, and disconnect a volume without losing its catalog.

## Milestone 2 - Metadata and thumbnails

Status: implemented on `feat/media-organizer`. The hub creates bounded, versioned FFmpeg artwork on demand and inspects video and image streams through structured ffprobe JSON.

- Probe video streams with ffprobe using bounded concurrency and timeouts.
- Read image dimensions without decoding unbounded data.
- Store duration, dimensions, codecs, container, file size, and modification time.
- Generate versioned thumbnails into application cache; never write beside external media.
- Reuse cached results when the file fingerprint is unchanged.

Implementation notes: metadata runs in a three-slot background queue with a 15-second per-file timeout. Size and modification time form the fast fingerprint. Unchanged ready or failed records are not reprobed; changed files return to pending. Results are stored in `library_media_metadata` and delivered through normalized item events. Users can clear generated artwork from Library management without touching original media.

## Milestone 3 - Library browsing

Status: implemented on `feat/media-organizer`.

- Responsive persisted grid/list views with lazy artwork and useful empty/loading states.
- Desktop video/image opening, a focused details panel, and left/right/Escape keyboard navigation.
- Filters for source, hierarchical source folder, collection, media type, duration, resolution, container, date, missing status, tags, favorites, and rating. Each source root and subfolder supports either descendant-inclusive or direct-files-only scope.
- Normal browsing shows available media only. Missing records remain recoverable through the Missing files filter, where they can be restored by a later scan or explicitly removed without deleting anything from disk.
- Per-item favorites and ratings plus bounded bulk selection for favorites, tags, ratings, and collection membership.
- User collections that organize catalog records without moving or modifying source files.

Implementation notes: migration `0011_library_curation.sql` adds favorites, ratings, collections, and collection membership. Rust validates identifiers and values, caps each bulk request at 500 unique items, and applies the entire action transactionally. React persists only the grid/list presentation preference; SQLite remains authoritative for all curation data.

## Milestone 4 - Watching and reconciliation

Status: implemented on `feat/media-organizer`.

- Optional per-source native filesystem watching, disabled by default, with two-second event-burst debouncing.
- Ten-second online/offline source health checks, automatic watcher recovery, and a source-scoped rescan when a watched volume returns.
- Normalized source, scan, and item events keep React informed while Rust owns watcher and scan lifecycle.
- Watcher settings persist in SQLite and can still be disabled while a removable source is offline.
- Linux device/inode identity safely preserves a catalog record across a rename only when the old path is gone and the identity match is unique.
- Migration `0012_library_reconciliation.sql` stores file identity and reviewable Added, Modified, Renamed, Missing, and Restored events.
- Library management provides individual and bulk review actions; initial source imports establish a baseline instead of flooding the review list.

Watching never edits source media. A watched change schedules the same bounded, non-symlink-following source scan used by the manual Scan now action; concurrent scans remain capped at two.

## Milestone 5 - Duplicate intelligence

Status: exact-content vertical slice implemented on `feat/media-organizer`.

- Group available files by size to avoid unnecessary reads.
- Verify candidates byte-for-byte with cancellable, streaming SHA-256 analysis owned by Rust.
- Cache hashes in migration `0013_library_duplicate_hashes.sql`; reuse them only while the size/mtime fingerprint still matches.
- Reject symlinks and files that change during analysis.
- Present exact groups, paths, per-file size, and an informational potential-reclaim estimate.
- Let the user choose an individual copy through a two-step Move to Trash action. Rust re-derives the group and re-hashes both the selected file and a retained copy immediately before cleanup.
- Remove the catalog record only after the operating system confirms the Trash move. Never expose permanent deletion or allow cleanup when no verified retained copy is available.

Optional perceptual similarity remains future work. It must be clearly labeled approximate and must never be mixed with exact SHA-256 evidence.

## Milestone 6 - Safe organization tools

Status: implemented on `feat/media-organizer`.

- The media details panel exposes typed Rename, Move, and Copy intent rather than raw filesystem access.
- Rust validates the current catalog fingerprint, same-extension filename, absolute destination directory, and non-symlink source before creating a persisted preview plan.
- Every preview shows the exact source and destination plus warnings. Execution accepts only the opaque persisted plan ID and revalidates it immediately before changing disk.
- Publication is no-clobber: Copy uses a hidden staging file. Linux Move/Rename uses atomic `renameat2(RENAME_NOREPLACE)` and falls back to a verified create-new copy/remove transition only when the same-filesystem mount does not support that operation. Other platforms retain their no-clobber adapter. Existing destinations are never overwritten.
- Migration `0014_library_file_operations.sql` journals plans and the executing, completed, failed, or undone outcome. Library management shows the latest 100 confirmed operations.
- Completed Move/Rename operations can be undone when the original path is free and the operated file fingerprint still matches. Copy undo moves the verified copy to system Trash.
- Selection mode can preview and move up to 500 media files into one chosen subfolder. Rust validates the entire batch before moving the first file, rejects duplicate destination filenames, and leaves failed items selected if a race causes a partial result.
- Concurrent execution is rejected through an atomic SQLite state transition. Catalog changes happen only after filesystem success; partial catalog failures direct the user to Verify files.
- On startup, an interrupted executing journal entry becomes Failed with Verify files guidance; abandoned preview plans expire after 24 hours.

Cross-filesystem Move is intentionally rejected for now. Use Copy, verify the destination, then handle the original separately. This keeps the first implementation recoverable and prevents an implicit copy-delete operation from being presented as an atomic move.

## Milestone 7 - Maturity and accessibility

Status: accessibility, backup/recovery, and SQLite-backed large-library foundations implemented.

- `Ctrl+K` opens a keyboard-first workspace palette; `?` opens a discoverable shortcut guide. Both dialogs trap focus, restore it when dismissed, and support Escape.
- Persisted Desktop preferences provide 90%, 100%, 110%, and 125% native webview scaling, reduced motion, and a stronger high-contrast palette.
- CSS also honors operating-system reduced-motion and increased-contrast preferences without requiring an app override.
- Global error and success surfaces use assertive/polite live regions, and the existing skip link provides direct keyboard access to main content.
- Filtered, searched, and bulk-selection Library results use SQLite-backed 60-item pages with exact totals, numbered navigation, Previous/Next controls, and polite page announcements. Search is debounced, scan event bursts are coalesced, and cross-page selection retains only bounded IDs in React.
- Home uses three independent 12-item queries for recent media, videos, and images. Rust/SQLite owns filter composition and collection membership; lightweight facets supply global availability counts plus tag, container, and hierarchical folder choices without loading full catalog records.
- Version 2 Library backups include source definitions, collections and membership, curation, and structured media metadata with explicit catalog/metadata schema versions. Restore accepts legacy version 1 files, validates bounded untrusted input before writes, safely remaps path/ID conflicts, preserves offline source definitions, and invalidates metadata whose on-disk fingerprint changed. Restore merges and never deletes current records.
- Generated artwork is bounded to 20,000 previews or 1 GiB. Rust updates last-use timestamps, serializes maintenance against FFmpeg generation and manual clearing, and evicts the least-recently-used entries without removing the preview currently being returned. Library management reports live file and byte usage.

Remaining:

- Accessible status/progress announcements with keyboard-only and Orca verification.
- Large-library profiling on representative network and removable filesystems, plus migration recovery tests.

## Milestone 8 - Embedded playback

Status: implemented on `feat/media-organizer`.

- Videos open in a focused watch surface backed by the WebView's native HTML5 media engine. Nice Downloader does not implement codecs or decoding.
- A Rust-owned loopback server binds only to a random `127.0.0.1` port and requires a process-scoped unguessable token. Playback URLs contain catalog IDs, never filesystem paths.
- The server validates current SQLite records, rejects missing/non-video/symlink paths, streams files without buffering them in memory, and supports single HTTP byte ranges plus `HEAD` for seeking.
- Playback position is saved to migration `0015_library_playback.sql`. In-progress videos appear in a bounded Continue Watching shelf and resume after five seconds while near-complete videos leave the shelf.
- The player provides native accessible controls, speed selection, fullscreen, previous/next navigation, and Space/K, J/L, arrow, M, F, and Escape shortcuts.
- Matching WebVTT sidecars (`video.vtt` or `video.en.vtt`) are exposed as subtitle tracks through the same token and catalog boundary.
- Open in system player remains visible in media details and the watch surface. It is the supported fallback when the platform WebView cannot decode a container or codec.
- Reopening or manually retrying a video receives a fresh loopback session URL. This prevents WebKitGTK from reusing stale media-resource failures while preserving the same token and catalog-ID security boundary; retrying an existing compatibility copy does not duplicate it.
- When WebKitGTK rejects the valid lossless timing copy, the error surface offers an explicit H.264/AAC fallback instead of repeating the same request. Its preflight clearly states that video will be re-encoded, shows a conservative size/free-space estimate, and always requires consent unless that exact fingerprinted fallback is already cached. The source remains unchanged.
- Playback opens as an integrated Library watch workspace rather than a floating dialog. The main player and metadata occupy the primary column while a sticky, independently scrollable video rail supports debounced search, paginated browsing, and in-place video switching. An eight-second readiness timeout prevents a stalled WebKit media pipeline from trapping the UI.
- Returning to the Library home or navigating to another workspace collapses the same live video element into an in-app bottom-right mini player. Playback, buffering, audio, native control state, and progress continue without reopening the media session. Accessible controls restore the full Watch workspace, open the original in the system player, or close and persist progress.
- On Linux, Nice Downloader checks an available `gst-inspect-1.0` for WebKitGTK's `autoaudiosink` before mounting the player. Missing GStreamer audio-output plugins produce an actionable distribution-specific message and leave system-player playback available.
- AV1-in-MP4 tracks with fractional time-base metadata that WebKitGTK/GStreamer can play ahead of audio are detected with structured ffprobe output. FFmpeg creates a fingerprinted, bounded Matroska stream-copy so GStreamer receives a concrete frame rate instead of the QuickTime demuxer's `0/1`; the UI identifies compatibility mode and the original remains untouched.
- Preparation is split into a read-only preflight and an explicit execution command. For uncached copies the preflight returns the reason, estimated bytes, available space, saved Ask/Automatic/Never preference, and whether confirmation is mandatory. Ordinary copies may follow the Automatic preference, but copies of 2 GiB or larger always require one-time approval. The accessible confirmation dialog traps focus, restores it when dismissed, and keeps system-player fallback visible.
- Settings reports compatibility-cache file count and bytes, exposes an explicit clear action, and documents the four-file/8 GiB oldest-first limit. Clearing removes only app-private derivatives, never cataloged originals.

## Milestone 9 - Accessibility and large-library hardening

Status: implementation in progress on `feat/library-accessibility-performance`.

- Library scan announcements are intentionally bounded: start, each 100 newly discovered files, and completion/cancellation/failure. Rapid per-file events remain visual and never flood a screen reader.
- Exact-duplicate analysis announces candidate discovery, 20% progress boundaries, and its terminal result. The visible meter exposes native progressbar semantics without becoming a second noisy live region.
- Filter and pagination changes announce the exact visible result range. The Watch workspace separately announces its debounced search result page.
- A keyboard-only and Orca checklist lives in [Library Accessibility QA](ACCESSIBILITY_QA.md), including scan, duplicate, Watch, fallback, scaling, contrast, and reduced-motion expectations.
- Media details and Bulk Move now trap focus, preserve their invoking control, and use Library search/main content as a fallback when an operation removes that control. Add folder focuses its first action and restores focus when dismissed.
- SQLite migration recovery has an automated rollback test that forces a mid-chain migration failure, proves schema and version changes are atomic, repairs the obstruction, and completes the same database through migration 19.
- An ignored, repeatable 10,000–250,000-item Rust profiling harness measures database open, transactional seeding, first-page query, text search, and facet generation. `NICE_DOWNLOADER_LIBRARY_PROFILE_DIR` places the synthetic database on a chosen local, removable, or network filesystem; see [Library Performance Profiling](LIBRARY_PROFILING.md).

Remaining before completion:

- Perform and record keyboard-only plus Orca verification on a Linux desktop; implementation-level focus handling is complete, but the current development environment does not include Orca.
- Capture a comparable 50,000-item result from a representative network mount. The three-run removable exFAT baseline is recorded and did not expose a user-facing query bottleneck.

## Milestone 10 - Library discovery controls

Status: stable SQLite-backed sorting implemented on `feat/library-accessibility-performance`.

- Library results can be sorted by newest or oldest addition, title in either direction, duration, or file size.
- Rust maps the typed sort intent to static SQL clauses. React never supplies SQL or arbitrary field names.
- Every order includes deterministic tie-breakers so pagination cannot duplicate or skip equal-valued records.
- Media without inspected duration or file size stays after known values in both directions.
- Choosing a non-default sort enters the paginated all-media view; Home resets to the browse-first shelves and newest order.
- Database tests cover title, duration, and file-size ordering across the large-library fixture.

## Milestone 11 - Integrated image viewing

Status: implemented on `feat/library-accessibility-performance`.

- Full-resolution JPEG, PNG, WebP, GIF, and AVIF files open inside a dedicated Library image workspace; system-viewer fallback remains available.
- Rust issues fresh process-token-protected loopback URLs containing catalog IDs rather than filesystem paths. Every request revalidates the current SQLite record, media kind, regular-file status, symlink policy, and allowlisted content type.
- The viewer supports fit, actual size, bounded 10%–800% zoom, drag-to-pan, quarter-turn rotation, previous/next navigation, and a 3/5/10-second slideshow without modifying originals.
- Browser-native image orientation is honored, and decoded dimensions are shown separately from indexed metadata.
- Images at or above 100 MiB or 100 megapixels require a one-time in-viewer confirmation before WebView decoding, with system-viewer fallback kept visible.
- Keyboard controls cover previous/next, zoom, fit, rotation, slideshow, and close. Viewer shortcuts pause while Media details owns focus.
- Image navigation follows the current visible Library order. Stable sorting, filters, source/folder scope, and current pagination therefore remain authoritative.
- An active video keeps playing in the existing mini player while images are viewed. Restoring full Watch closes the image workspace so two full media surfaces cannot overlap.
- Missing, moved, unsupported, or WebView-undecodable images fail with retry and explicit system-viewer actions.

## Milestone 12 - Watch Later and playback history

Status: implemented on `feat/library-accessibility-performance`.

- Migration `0016_library_watch_later.sql` persists an ordered, cascade-safe Watch Later list without modifying source files.
- Video cards expose a direct Watch Later action. The Watch workspace shows the current saved state and supports removing it again.
- Home derives bounded Watch Later and Recently Watched shelves from SQLite. Playback history is private to the local application database and excludes missing media.
- Users can mark a video watched, mark it unwatched, or reset incomplete progress. These actions update Continue Watching and Recently Watched from the same authoritative playback record.
- Optional Autoplay advances only to a real next video in the current watch browser. It never wraps the final item into an endless loop and its preference persists locally.
- Rust validates catalog UUIDs and video media kind for every mutation. React never supplies filesystem paths or authoritative playback state.

Acceptance: save a video before watching it, relaunch and find it in Watch Later, remove it, inspect private recently watched activity, reset progress, and optionally advance to the next available video when playback completes.

## Milestone 13 - Persistent ordered playlists

Status: implemented on `feat/library-accessibility-performance`.

- Migration `0017_library_playlists.sql` stores named playlists and ordered membership with catalog-delete cascading.
- Playlists are video-only, case-insensitively named, limited to 500 unique items, and validated by Rust before every mutation.
- Library Home supports create, open, and confirmed delete. Playlist workspaces support Play all, individual playback, details, remove, and deterministic move up/down controls.
- The Watch workspace can add the current video to any playlist and clearly identifies existing membership.
- Playback uses persisted playlist order for Up next, Previous, Next, and Autoplay. Playlist playback never wraps at either boundary.
- Missing files remain visible for organization but are skipped for playback. Removing a catalog item automatically removes dangling membership.
- Version 2 Library backup/restore includes playlists and ordered membership. Restore validates untrusted data, remaps conflicting IDs, merges without deleting current data, and never modifies original media.
- Async playlist loads and mutations are guarded so stale responses and rapid reorder/remove input cannot overwrite newer UI state.

Acceptance: create a playlist, add videos from Watch, reorder and remove entries, relaunch with the same order, play continuously without looping, restore the playlist from backup, and delete the playlist without changing any source video.

## Milestone 14 - Rich metadata and creator browsing

Status: implemented on `feat/library-rich-metadata`.

- Migration `0018_library_rich_metadata.sql` stores extracted sidecar metadata separately from user overrides. Rescans refresh creator/channel, description, upload date, series, episode, and original URL without replacing intentional edits.
- `.info.json` import remains bounded and typed. Dates normalize to `YYYY-MM-DD`, original URLs must be HTTP(S), and text limits are enforced before persistence.
- Library discovery supports creator, series, site, and published-date filters plus deterministic newest/oldest published sorting. Facets remain SQLite-backed and source-aware.
- Media details show metadata provenance, reset overrides to extracted values, and open filtered creator, series, or site views.
- Custom JPEG, PNG, or WebP artwork is selected through the native picker and validated by Rust as an absolute regular non-symlink file. Original media and sidecars remain untouched.
- Version 2 Library backup/restore includes bounded rich-metadata overrides and remaps referenced catalog IDs safely.

Acceptance: scan media with a yt-dlp `.info.json` sidecar, browse its creator/series/site, sort by publication date, edit and reset metadata, choose artwork, rescan without losing edits, and restore overrides from a Library backup.

## Milestone 15 - Smart discovery and saved views

Status: implemented on `feat/library-smart-discovery`.

- Migration `0019_library_smart_discovery.sql` adds a local FTS5 index across title, creator, series, description, site, tags, and relative path. SQLite triggers update exactly one index row for every catalog insert, relevant update, rich-metadata refresh, and deletion.
- Search input is converted into bounded quoted prefix terms before `MATCH`; React never supplies FTS syntax or SQL. A typed status command detects missing, extra, or duplicate index rows, and rebuilding runs as one Rust-owned SQLite transaction.
- Creator and series selections open dedicated, paginated discovery headers with representative artwork and live result counts.
- Tags, creators, series, and sites accept multiple values. Values use OR within their group and AND across groups, with visible removable filter chips and a 20-value backend limit per group.
- Saved smart views persist validated filter and sort intent rather than item membership. Applying one restores the full query, and newly scanned matching media appears automatically.
- Media details request a bounded related-media list using series first, then creator, site, or source. Responses are guarded when users navigate quickly, and the current item is never returned as its own recommendation.
- Existing 5,105-item paging coverage and the manual 10,000–250,000-item profiling harness now exercise trigger-maintained FTS search.

Acceptance: combine several filter values, save and reopen the view, scan matching media and see it appear automatically, browse a creator or series workspace, open related media, deliberately damage the search index in a test, and recover it without modifying source files.

## Milestone 16 - Multi-video comparison workspace

Status: implemented on `feat/library-video-compare`.

- Library selection opens a comparison only for two to four existing, available video records. Images and missing records are rejected before navigation.
- Compare is a dedicated application workspace. Its ephemeral ordered item IDs remain available while navigating, but are intentionally not restored after restarting the application.
- Two videos use a side-by-side layout; three or four use a responsive 2x2 grid. Every tile contains unusual and vertical aspect ratios without overflowing.
- Tiles reuse Rust-authorized playback preflight, URL, compatibility-copy, progress, release, and system-player commands. React never receives or supplies filesystem paths.
- Only the selected tile emits audio by default. Users can explicitly allow multiple audio streams, while shared controls play, pause, restart, seek, or align all ready players to the active-audio video's absolute time.
- Compatibility consent and failures remain isolated per tile. Compatibility preparation is serialized by the existing Rust media-server lock, preventing concurrent fallback transcodes.
- Leaving Compare unmounts all players, saves progress snapshots, and releases every playback session. The comparison selection remains available for a later return.

Acceptance: select two to four videos across paginated Library results, open Compare, transfer audio focus, use shared playback controls, handle an incompatible tile without blocking the others, remove or replace a tile, leave and return to the preserved comparison, and open any playable tile in the system player.

## Explicitly deferred

Face recognition, cloud sync, server mode, automatic file deletion, and Stash-compatible scraping are not prerequisites for a mature local organizer. Revisit them only after the source scanner, metadata cache, browsing, and recovery behavior are dependable.
