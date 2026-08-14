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
- `engines`: routing policy and isolated adapters. yt-dlp handles ordinary media URLs; gallery-dl handles supported gallery URLs, initially Bunkr albums.
- `queue`: authoritative ordered pending queue, dynamic concurrency (default 2), scheduling, queue/job pause, child lifecycle, cancellation tokens, retry lineage, startup recovery, disk preflight, and normalized events.
- `database`: SQLite connection, embedded migrations, durable job intent/diagnostics, settings, and history repository.
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

Exact `bunkr.cr/a/<album-id>` URLs route to gallery-dl. Initial analysis is bounded to the album summary and first item, and signed CDN media URLs are discarded. Item browsing lazily resolves paced 12-file pages rather than signing an entire album, returning only sanitized filenames, indices, sizes, extensions, and thumbnails. Rust converts selected numeric indices into a validated gallery-dl range. Downloads preserve original files in a sanitized album directory, and gallery-dl item and byte progress is normalized into the same `DownloadJob` events as yt-dlp.

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
- `transformations`: FFmpeg task ownership, structured `-progress` parsing, cancellation, crash reconciliation, and independent SQLite history.
- `library`: non-destructive local-media catalog, safe `.info.json` normalization, tagging, missing-file reconciliation, and versioned metadata backup/restore.

Library scans write catalog identity immediately, then enqueue IDs into a globally bounded metadata pipeline. ffprobe runs without a shell, emits JSON, and has a per-file timeout. Normalized duration, dimensions, FPS, codecs, container, HDR range, and the size/mtime fingerprint live in `library_media_metadata`; a changed fingerprint resets the record to pending while unchanged records retain their probe result. Artwork is generated into the application cache with a fingerprinted filename and can be cleared independently of source media.

Library curation is persisted separately from filesystem scanning. Favorites and ratings live on catalog items; collections use dedicated collection and membership tables. Bulk curation commands validate a bounded set of UUIDs and execute in a single SQLite transaction. Rescanning an existing path preserves its title, tags, favorite, rating, and collection membership.

Opt-in library watching is owned by `LibraryManager`. The native watcher coalesces event bursts before requesting a normal bounded scan; it never applies filesystem mutations. A lightweight source monitor detects unavailable watched volumes, emits normalized source status events, recreates watchers when volumes return, and schedules a source-scoped reconciliation scan. React can enable or disable watching but does not own watcher tasks or scan state.

Reconciliation stores stable filesystem identity separately from user-facing metadata. On Unix, a unique device/inode match can preserve the existing item ID and curation across a rename, but only after confirming the previous path is gone. Added, modified, renamed, missing, and restored transitions become bounded review records delivered through normalized events.

Duplicate analysis is an explicit, cancellable `LibraryManager` task. It narrows candidates by size, streams files through SHA-256 without following symlinks, verifies the fingerprint before and after reading, and caches only current hashes. React renders exact-content evidence and progress but cannot bypass the backend cleanup policy.

Exact duplicate cleanup is a separate narrow command. It accepts only a current duplicate-group ID and member ID, re-derives the group from current SQLite fingerprints, requires another available member, and re-hashes the selected and retained files before invoking the platform Trash API. The catalog row is removed only after that recoverable operation succeeds; no permanent-delete command is exposed.

Library organization uses persisted typed plans. React can request a Move, Copy, or Rename preview for a catalog item, but execution accepts only the resulting opaque operation ID. Bulk Move accepts at most 500 unique catalog IDs and one absolute destination directory; every item, filename collision, fingerprint, no-clobber destination, and filesystem boundary is preflighted before execution starts. `LibraryManager` revalidates the regular-file source, catalog fingerprint, destination parent, filename extension, no-clobber condition, and journal state. SQLite atomically claims each plan before filesystem work. Copy stages data inside the destination filesystem before no-clobber publication; Move and Rename are same-filesystem no-clobber transitions. Completed actions update the catalog and journal, and Undo revalidates both paths and the latest fingerprint before reversing a move/rename or sending a copied file to platform Trash. Startup marks interrupted operations for explicit verification and expires abandoned previews. Cross-filesystem Move is rejected instead of silently becoming copy-delete.

Subfolder browsing is presentation state derived from normalized `sourceId` plus `relativePath`. The selector exposes every ancestor folder with descendant-aware counts. A folder filter always includes nested paths and is combined with the source, media, curation, and metadata filters without granting React filesystem access.
- `authentication`: longest-host-suffix matching, browser-profile discovery, and backend-only managed Netscape cookie files. Managed paths use owner-only permissions on Unix and are omitted from frontend DTOs.

The tray and desktop settings form a small platform boundary. Window close policy is applied in the Tauri frontend while process survival, queue recovery, and persisted preferences remain backend-owned. Desktop preferences also carry validated interface scale, reduced-motion, and high-contrast intent. React applies the visual classes and uses Tauri's narrowly permitted webview zoom command; only 90%, 100%, 110%, and 125% are accepted by Rust.

## Media organizer boundary

The Library can index multiple user-selected folders independently of download destinations. Sources, durable scan jobs, and catalog items are persisted in SQLite. A Rust-owned scanner performs bounded directory walks, ignores symlinks by default, classifies supported video and image extensions, and emits normalized progress events. Files remain in place; scanning and record removal never mutate external media.

The default Library presentation is a browse-first media hub. React may request artwork only by catalog item ID. Rust validates the stored item, resolves FFmpeg internally, limits concurrent artwork work, and returns a bounded cached JPEG data URL; the webview never receives arbitrary filesystem read access. Opening media likewise accepts an item ID and resolves the persisted path in Rust.

`docs/MEDIA_ORGANIZER.md` is the source of truth for organizer milestones, safety invariants, acceptance criteria, and deferred scope.

## Remaining extension points

The gallery-dl route can expand beyond Bunkr. Release builds stage pinned,
SHA-256-verified yt-dlp, gallery-dl, FFmpeg, and ffprobe resources. Runtime
resolution prefers app-managed updates, then the bundled baseline, then host
installations. Platform packaging, non-Linux pause/notification adapters, and
custom arguments remain later work. Additional authenticated-site support should
extend typed access preferences rather than exposing arbitrary engine flags.
