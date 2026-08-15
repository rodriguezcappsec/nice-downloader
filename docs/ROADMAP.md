# Nice Downloader Roadmap

## Active: local media organizer

Implementation is tracked in `docs/MEDIA_ORGANIZER.md`. Milestones 1–6 are implemented on `feat/media-organizer`: external folders, hierarchical folder filters with direct-only depth control, cancellable scans, structured metadata, curation, watching, removable-volume recovery, change review, exact SHA-256 duplicate cleanup, previewable Move/Copy/Rename operations, and preflighted bulk moves into a chosen subfolder. Milestone 7 includes SQLite-backed Library filtering and pagination, bounded Home shelves and facets, accessibility foundations, versioned recovery, and bounded artwork caching. Milestone 8 adds secure embedded HTML5 playback, byte-range streaming, resume state, Continue Watching, WebVTT sidecars, explicit system-player fallback, and informed consent plus cache management for real compatibility copies. Milestone 9 adds bounded assistive-technology announcements, dialog focus trapping and restoration, transactional migration-recovery proof, and repeatable 10,000–250,000-item profiling; removable exFAT measurements are recorded while manual Orca and network-filesystem verification remain outstanding. Milestone 10 adds typed, deterministic SQLite-backed sorting across paginated Library results. Milestone 11 adds secure full-resolution image viewing, zoom/pan/rotation, keyboard navigation, slideshows, and system-viewer fallback while preserving active mini-player video.

Milestone 12 adds persistent Watch Later, local playback history, watched/reset controls, and bounded non-looping Autoplay to complete the first YouTube-like repeat-viewing workflow. Milestone 13 adds persistent ordered video playlists, Library and Watch workspace management, non-looping ordered playback, cascade-safe membership, and backup/restore support.

The creative roadmap now includes persisted, backup-safe video montage projects with Rust-owned multi-source FFmpeg export, a rebuildable approximate visual-similarity index, temporary capability-link LAN sharing, and session-only lean-back playback.

## Milestone 14: rich metadata and creator browsing

Deliver typed yt-dlp `.info.json` metadata import, separate extracted values from user overrides, preserve edits across rescans, add creator/series/site and published-date discovery, validate custom artwork in Rust, and include rich overrides in versioned Library backup/restore.

Status: implemented on `feat/library-rich-metadata`.

## Milestone 15: smart discovery and saved views

Deliver trigger-maintained FTS5 search, multi-value discovery filters, dedicated creator/series workspaces, dynamic saved views, bounded related media, and transactional search-index recovery.

Status: implemented on `feat/library-smart-discovery`.

## Milestone 16: multi-video comparison workspace

Deliver a dedicated two-to-four-video grid, Library selection entry point, independent native controls, one-audio-at-a-time focus, optional audio mixing, bounded shared playback controls, compatibility-copy consent, and per-tile system-player fallback.

Status: implemented on `feat/library-video-compare`.

## Milestone 17: clips, moments, and storyboards

Deliver durable timestamp/range moments, Rust-owned safe clip export through the FFmpeg transformation lifecycle, bounded cached storyboards, player seeking, and version 3 backup/restore.

Status: implemented on `feat/library-creative-workspaces`.

## Next organizer milestones

18: saved comparison sessions and review-note export — implemented on `feat/library-creative-workspaces`.
19: synchronized image comparison and explicit media-version stacks — implemented on `feat/library-visual-curation`.
20: keyboard-first rapid curation and dynamic rule collections — implemented on `feat/library-visual-curation`.
21: local subtitle/transcript FTS with timestamp seeking — implemented.
22: persisted video montage builder with cancellable Rust-owned FFmpeg export — implemented.

23: approximate visual similarity explorer with fingerprint-aware dHash indexing — implemented.

24: temporary read-only LAN sharing for explicitly selected catalog media — implemented.

25: accessible lean-back playback with large keyboard and gamepad controls — implemented.

26: restored Library workspaces, universal media commands, session-only Play Next, local share QR codes, and a bounded media-health dashboard — implemented.

## Maturity roadmap delivered

- First-run engine/privacy onboarding and Linux AppImage/DEB plus Windows NSIS release workflows.
- Rust-owned scheduled subscriptions and searchable, archive-aware playlist selection.
- Typed FFmpeg transformations with progress, cancellation, recovery, and persistence.
- Local media library with tags, filters, missing-file reconciliation, sidecar imports, and backup/restore.
- Per-site authentication profiles, browser discovery, and backend-only managed cookie files.
- System-tray lifecycle, close/background behavior, startup reconciliation, and persisted desktop preferences.
- CI, packaging foundations, strict CSP/capabilities, migration tests, and documented signing blockers.

Remaining release gates are operational: signing credentials, clean-machine smoke tests, Flatpak dependency vendoring, SBOM publication, and broader real-site compatibility fixtures.

## Intelligent repeat workflows delivered

- Opt-in clipboard URL detection that never analyzes or downloads automatically.
- SQLite-backed host/path rules that apply saved presets and remain manually overridable.
- Safe filename templates with local previews and backend validation.
- Typed SponsorBlock category controls.
- Durable archive records independent of deletable History rows, with explicit Download again overrides.
- Backend-owned channel and playlist subscriptions with bounded checks, preset reuse, archive-aware deduplication, pause/resume, and Check now controls.

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
- Canonical resolution for Reddit share-token and `redd.it` links before extractor routing.

## Milestone 2: reliability and batch workflow

Delivered:

- Persist all non-completed job intent and reconcile interrupted jobs back into the queue at startup.
- Retry failed/cancelled jobs as traceable attempts.
- Persist bounded, sanitized per-job diagnostics.
- Preflight destination write access and expose explicit skip/overwrite intent.
- Select a named browser profile and an allowlisted credential-free HTTP/SOCKS proxy.
- Analyze up to 20 URLs with bounded concurrency and bulk enqueue selected results.
- Expand playlists through structured yt-dlp flat-playlist JSON before batch analysis.

Additional reliability delivered:

- Authoritative queue reordering, persisted dynamic concurrency, whole-queue pause, per-job pause/resume, and scheduled starts.
- Disk-space estimation and backend preflight with temporary-file margin.
- Narrow file open/reveal actions and configurable Linux completion notifications.
- Sanitized diagnostic export for support.
- Classified automatic recovery for rate limits, 5xx responses, timeouts, resets, and incomplete reads, capped at three total attempts.
- ffprobe output verification with verified/warning status in History.
- Global bandwidth limiting, safe duplicate prevention, and searchable History with record-only deletion.
- Deterministic loopback integration coverage for 429 recovery, existing partial-file Range resume, and interrupted-body recovery using the real curl engine.

## Milestone 3: richer media intent

- Compatibility/quality/size recommendations per resolution.
- Audio formats and bitrate choices.
- Subtitle, metadata, thumbnail, chapters, and playlist controls.
- Reusable SQLite-backed presets with New Download integration.
- ffprobe-based inspection of completed output.

Delivered foundations include Recommended/Compatibility/Quality/Smallest selection strategies, subtitle language patterns, chapter embedding, codec/audio controls, and completed-output inspection.

## Milestone 4: additional engines

Delivered foundation:

- Analyzer/router policy and strict URL classification.
- gallery-dl analysis, lazy paced album browsing, per-file selection, resumable original-file download, cancellation, progress, and history for Bunkr albums.

Still planned:

- Extend gallery-dl routing to additional tested image/gallery sites.
- HLS/DASH diagnostics and engine routing improvements.

Delivered:

- Direct HTTP analysis and resumable transfers, preferring aria2c with a curl fallback.

## Milestone 5: engine lifecycle and distribution

- Managed yt-dlp/gallery-dl storage with vendor SHA-256 verification, version probing, pinning, and rollback.
- Linux packaging and desktop integration.
- Windows and macOS platform adapters and packaging.

Linux `.deb` and AppImage bundle targets plus first-run required-engine guidance are configured. Flatpak and cross-platform packages remain distribution work.

## Deferred by design

Browser extensions, subscriptions, standalone cookie-file importing, and broad platform packaging remain outside milestone 1.
