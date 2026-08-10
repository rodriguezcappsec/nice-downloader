# Nice Downloader Roadmap

## Milestone 1: yt-dlp vertical slice

Deliver a runnable Linux desktop application that proves the complete path:

1. Tauri 2, React, TypeScript, Vite, Tailwind, Zustand, and Lucide foundation.
2. Desktop sidebar and dark/light theme-aware native utility styling.
3. PATH discovery and version reporting for yt-dlp, FFmpeg, and ffprobe.
4. Structured yt-dlp URL analysis with typed Rust and TypeScript models.
5. Simplified quality selection and typed download intent.
6. Rust-owned queue with two concurrent slots, normalized progress events, safe cancellation, and child reaping.
7. A real yt-dlp download with FFmpeg post-processing state reporting.
8. SQLite migrations, completed-download persistence, and basic History.
9. Formatting, frontend type checking/build, Rust formatting/checks/tests, and a Tauri build check.

Acceptance path: launch the app, analyze a supported URL, review metadata, choose quality and destination, download with visible progress/speed/ETA, cancel an active job, and see completed jobs in History.

### Compatibility hardening delivered

- Opt-in Firefox and Chromium-family cookie sources for authenticated and anti-bot-sensitive sites.
- Runtime browser-impersonation detection with safe conditional activation.
- Expandable sanitized yt-dlp diagnostics and clearer authentication/403 guidance.
- Capability status in Settings without exposing raw command arguments.

## Milestone 2: queue durability and recovery

- Persist queued/running job intent and reconcile interrupted jobs at startup.
- Retry failed/cancelled jobs.
- Queue reordering and configurable concurrency.
- Download diagnostics/log detail view.
- File reveal/open actions through narrow backend commands.
- Stronger output conflict and disk-space handling.

## Milestone 3: richer media intent

- Compatibility/quality/size recommendations per resolution.
- Audio formats and bitrate choices.
- Subtitle, metadata, thumbnail, chapters, and playlist controls.
- Reusable presets.
- ffprobe-based inspection of completed output.

## Milestone 4: additional engines

- Analyzer/router policy and URL classification.
- gallery-dl implementation for image/gallery sites.
- aria2 implementation for direct HTTP transfers.
- HLS/DASH diagnostics and engine routing improvements.

## Milestone 5: engine lifecycle and distribution

- Optional managed sidecars with signed/checksummed releases.
- Explicit update policy and rollback.
- Linux packaging and desktop integration.
- Windows and macOS platform adapters and packaging.

## Deferred by design

Browser extensions, subscriptions, standalone cookie-file importing, SponsorBlock UI, automatic binary downloads, and broad platform packaging are intentionally outside milestone 1.
