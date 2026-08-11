# Nice Downloader Architecture

Nice Downloader is a Linux-first Tauri desktop application that presents a stable, task-oriented interface over mature media tools. React renders the desktop experience; Rust owns every privileged or authoritative operation.

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

The gallery-dl route can expand beyond Bunkr. FFmpeg remains system-managed until a portable, verifiable distribution policy is selected. Platform packaging, non-Linux pause/notification adapters, and custom arguments remain later work. Additional authenticated-site support should extend typed access preferences rather than exposing arbitrary engine flags.
