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
- Filters for source, hierarchical source subfolder, collection, media type, duration, resolution, container, date, missing status, tags, favorites, and rating. Selecting a parent subfolder includes its descendants.
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
- Publication is no-clobber: Copy uses a hidden staging file and atomic hard-link publication; Move/Rename uses a no-clobber hard-link transition on the same filesystem. Existing destinations are never overwritten.
- Migration `0014_library_file_operations.sql` journals plans and the executing, completed, failed, or undone outcome. Library management shows the latest 100 confirmed operations.
- Completed Move/Rename operations can be undone when the original path is free and the operated file fingerprint still matches. Copy undo moves the verified copy to system Trash.
- Concurrent execution is rejected through an atomic SQLite state transition. Catalog changes happen only after filesystem success; partial catalog failures direct the user to Verify files.
- On startup, an interrupted executing journal entry becomes Failed with Verify files guidance; abandoned preview plans expire after 24 hours.

Cross-filesystem Move is intentionally rejected for now. Use Copy, verify the destination, then handle the original separately. This keeps the first implementation recoverable and prevents an implicit copy-delete operation from being presented as an atomic move.

## Milestone 7 - Maturity and accessibility

Status: accessibility foundation implemented; large-library and recovery work remains.

- `Ctrl+K` opens a keyboard-first workspace palette; `?` opens a discoverable shortcut guide. Both dialogs trap focus, restore it when dismissed, and support Escape.
- Persisted Desktop preferences provide 90%, 100%, 110%, and 125% native webview scaling, reduced motion, and a stronger high-contrast palette.
- CSS also honors operating-system reduced-motion and increased-contrast preferences without requiring an app override.
- Global error and success surfaces use assertive/polite live regions, and the existing skip link provides direct keyboard access to main content.
- Filtered, searched, and bulk-selection Library results use reachable 60-item pages with result ranges, numbered navigation, Previous/Next controls, and polite page announcements. SQLite-backed query pagination remains the later large-library optimization.

Remaining:

- Library backup/restore includes sources, collections, and metadata schema versions.
- Accessible status/progress announcements with keyboard-only and Orca verification.
- Large-library profiling, paginated queries, thumbnail cache limits, and migration recovery tests.

## Explicitly deferred

Face recognition, cloud sync, server mode, automatic file deletion, and Stash-compatible scraping are not prerequisites for a mature local organizer. Revisit them only after the source scanner, metadata cache, browsing, and recovery behavior are dependable.
