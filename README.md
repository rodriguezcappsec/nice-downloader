<p align="center">
  <img src="src-tauri/icons/icon.png" width="104" alt="Nice Downloader app icon">
</p>

<h1 align="center">Nice Downloader</h1>

<p align="center">
  <strong>Your media, on your disks, in a library worth browsing.</strong>
  <br>
  A private, local-first streaming library and download manager for Linux.
</p>

<p align="center">
  <a href="https://github.com/rodriguezcappsec/nice-downloader/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/rodriguezcappsec/nice-downloader/ci.yml?branch=main&amp;style=flat-square&amp;label=CI"></a>
  <a href="https://github.com/rodriguezcappsec/nice-downloader/releases"><img alt="Latest release" src="https://img.shields.io/github/v/release/rodriguezcappsec/nice-downloader?display_name=tag&amp;sort=semver&amp;style=flat-square"></a>
  <img alt="Linux only" src="https://img.shields.io/badge/platform-Linux-c99746?style=flat-square">
  <img alt="Local first" src="https://img.shields.io/badge/library-local--first-282522?style=flat-square">
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-24c8db?style=flat-square">
</p>

<p align="center">
  <a href="#installation">Install</a> ·
  <a href="#what-it-can-do">Features</a> ·
  <a href="#adaptive-linux-playback">Playback</a> ·
  <a href="#supported-sources">Supported sources</a> ·
  <a href="#development">Development</a>
</p>

> [!IMPORTANT]
> Nice Downloader is Linux-only and under active development. It does not bypass DRM or grant permission to download media. Download only content you are authorized to access and follow the terms and laws that apply to you.

## Why Nice Downloader

Downloaded media tends to disappear into folders. Nice Downloader turns those folders into a warm, artwork-driven media room without uploading the collection or asking you to adopt a new storage scheme.

| Library-first | Download-capable | Local by design |
| --- | --- | --- |
| Continue Watching, Watch Later, playlists, rich details, search, filters, and visual shelves make a local collection pleasant to use. | Analyze links, choose a simple download intention, manage active and scheduled work, and automate repeat workflows. | Rust owns filesystem and process access. Media stays where it is, originals are not silently replaced, and optional intelligence runs against local services. |

Nice Downloader is an orchestration layer over mature media tools—primarily [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg](https://ffmpeg.org/), [gallery-dl](https://github.com/mikf/gallery-dl), [aria2](https://aria2.github.io/), and curl—rather than an attempt to reimplement their extractors and codecs.

## What it can do

### Build a private streaming library

- Index multiple folders and removable drives without moving or rewriting source media.
- Browse artwork-first Home shelves for Continue Watching, Watch Later, recent activity, videos, images, playlists, and collections.
- Search ordinary metadata, rich sidecar metadata, tags, creators, series, sites, transcript cues, and saved smart views.
- Filter and sort a paginated Library by folder, availability, format, duration, resolution, rating, favorite state, publication data, and other indexed metadata.
- Watch video, inspect full-resolution images, resume playback, use a persistent movable mini-player, and enter a large-control lean-back mode.
- Manage subtitles, transcript seeking, chapter and highlight suggestions, timestamped moments, loop ranges, frame previews, screenshots, and per-video picture adjustments.
- Create ordered local playlists, maintain Watch Later, mark items watched, and optionally continue with non-looping autoplay.

### Download without losing control

- Analyze a URL before downloading and review its real title, artwork, creator, duration, formats, codecs, resolution, frame rate, HDR information, and estimated size when available.
- Start with Recommended, Best Quality, Compatibility, Smallest, or Audio Only; reveal codec, container, subtitle, authentication, SponsorBlock, filename, and other expert controls only when needed.
- Analyze batches, expand bounded playlists, select individual gallery files, and reuse validated download presets.
- Monitor Active, Queue, Scheduled, Completed, and Failed work in one Downloads workspace with progress, speed, ETA, retries, pause/resume where supported, cancellation, and startup recovery.
- Prevent accidental duplicates with destination-aware archive records and verify completed media with ffprobe when available.
- Schedule archive-aware channel or playlist subscriptions and attach typed post-download automation recipes.

### Organize, understand, and transform media

- Review Library changes, missing files, metadata status, compatibility risks, cache use, and storage pressure from focused maintenance workspaces.
- Apply tags, favorites, ratings, collections, metadata overrides, and custom artwork without changing the underlying media.
- Preview Move, Copy, and Rename operations before execution; undo supported operations and send exact duplicates to the platform Trash through a separately confirmed cleanup flow.
- Compare two to four videos or images, save video comparison sessions, and export review notes.
- Find exact duplicates, review near-duplicates, and build a local visual-similarity index.
- Use an optional local vision model to discover evidence-backed themes across selected videos, folders, or a bounded sample of the Library.
- Build montages, extract audio, create optimized H.264/AAC versions, export privacy-redacted derivatives, and run other cancellable FFmpeg transformations while preserving originals.
- Temporarily share an explicitly selected set of media over the local network with an expiring capability link and locally generated QR code.

### Stay comfortable and in control

- Use dark, light, high-contrast, and reduced-motion presentation modes with interface scaling and visible keyboard focus.
- Open global media and workspace search with `Ctrl+K`, start a download with `Ctrl+N`, open Settings with `Ctrl+,`, or press `?` for the shortcut guide.
- Keep downloads visible in the background through the application shell and optional system-tray behavior.
- Lock selected sources, folders, or individual records behind app-level privacy rules, with automatic relocking and a searchable private-media manager after unlock.
- Back up and merge supported Library metadata, organization, playlists, moments, comparisons, and derived workspace state without copying the media itself.

## Adaptive Linux playback

Nice Downloader does not claim that one HTML `<video>` element can decode every file. It uses a layered playback strategy so most local videos that FFmpeg can decode remain watchable without first waiting for a complete converted copy.

| Order | Playback path | What happens |
| --- | --- | --- |
| 1 | **Direct WebView playback** | The original file is served through a tokenized loopback byte-range endpoint and rendered by WebKitGTK/GStreamer. No media conversion occurs. |
| 2 | **HLS compatibility stream** | If preflight predicts trouble, or direct playback fails or stalls, FFmpeg creates temporary H.264/AAC segments while the video plays. |
| 3 | **Fragmented MP4 compatibility stream** | If HLS cannot become ready, FFmpeg streams a temporary fragmented H.264/AAC MP4 feed without creating a completed playback file. |
| 4 | **Recovery choices** | Embedded libmpv where available, a native mpv window, cached compatibility files, and the system player remain explicit alternatives. |

Seeking a temporary compatibility stream starts a fresh authorization at the requested source timestamp. Progress, resume state, subtitles, and transcript jumps continue to use the original video's logical timeline. Temporary streams are cancelled and cleaned up when released; the original file is unchanged.

Cached compatibility files still serve useful recovery cases:

- A lossless Matroska stream-copy can preserve AV1 and audio while avoiding WebKitGTK timing behavior seen with some MP4/M4V/MOV files.
- An explicitly approved H.264/AAC copy can recover a file that the available streaming and native paths still cannot render reliably.
- Large or re-encoded copies require confirmation. Cache use and clearing remain visible in Settings.

Release AppImages include a curated GStreamer plugin set with its matching plugin scanner. Development and native distribution builds use the host GStreamer runtime. For broad host playback support and the optional mpv fallback:

```bash
# Arch Linux
sudo pacman -S --needed gst-plugins-good gst-libav mpv

# Debian or Ubuntu
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-libav mpv
```

`gst-plugins-good` provides the required audio-output element; `gst-libav` broadens codec coverage. Corrupt files, encrypted media, DRM, unavailable decoders, or formats FFmpeg itself cannot decode can still fail.

## Installation

Published Linux artifacts are available from [GitHub Releases](https://github.com/rodriguezcappsec/nice-downloader/releases). Tagged builds are produced by the [Linux release workflow](https://github.com/rodriguezcappsec/nice-downloader/actions/workflows/release-linux.yml).

| Package | Best for |
| --- | --- |
| `Nice_Downloader-x86_64.AppImage` | Portable use on most x86-64 Linux distributions |
| `Nice Downloader_<version>_amd64.deb` | Installed desktop integration on Debian and Ubuntu |

### AppImage

```bash
chmod +x Nice_Downloader-x86_64.AppImage
./Nice_Downloader-x86_64.AppImage
```

If FUSE is unavailable:

```bash
./Nice_Downloader-x86_64.AppImage --appimage-extract-and-run
```

### Debian and Ubuntu

```bash
sudo apt install "./Nice Downloader_0.1.0_amd64.deb"
```

## Engine profiles

Nice Downloader supports two Linux package profiles:

- **Standard** is the recommended release profile. It includes verified, pinned yt-dlp, FFmpeg, ffprobe, and gallery-dl sidecars.
- **Slim** contains no bundled engine binaries. It uses compatible app-managed or system-installed executables and is intended for users and distributions that prefer to manage those tools separately.

Settings → Engines & diagnostics identifies each active executable as **Managed**, **Bundled**, or **System**, reports its version, and explains missing requirements.

| Engine | Standard | Slim | Purpose |
| --- | --- | --- | --- |
| `yt-dlp` | Bundled | Required | URL analysis and ordinary media extraction |
| `ffmpeg` | Bundled | Required | Merging, transformations, artwork, and adaptive playback |
| `ffprobe` | Bundled | Optional | Media inspection and completed-output verification |
| `gallery-dl` | Bundled | Optional | Supported gallery and Bunkr workflows |
| `aria2c` | System optional | System optional | Preferred resumable direct HTTP transfer |
| `curl` | System fallback | System fallback | Direct HTTP transfer when aria2c is unavailable |

Engine binaries generated by `npm run engines:prepare` are ignored by Git. Release CI assembles them from URLs and SHA-256 digests pinned in [`engines.lock.json`](engines.lock.json).

## Supported sources

Ordinary media URLs are routed through the installed yt-dlp version, including commonly supported sites such as YouTube, TikTok, X/Twitter, Instagram, Reddit, Twitch, and Vimeo. Extractor behavior can change independently of Nice Downloader, so a site that works today may require a newer engine tomorrow.

Nice Downloader also has typed workflows for:

- Bunkr albums and direct media pages, with bounded inspection, individual-file selection, durable per-file progress, and failed-file retry.
- Imgur albums and galleries, Flickr albums and galleries, and Pinterest boards through gallery-dl.
- Resumable direct HTTP files through aria2c, with curl as a fallback.
- Explicit or signed HLS and DASH VOD manifests through yt-dlp, with separate diagnostics for live streams, DRM, inaccessible encryption keys, malformed manifests, and unavailable media.
- Reddit share-link canonicalization and a limited public-embed fallback when Reddit blocks its API for the current IP address.

Authentication-sensitive sites can use an opt-in browser-cookie profile. Nice Downloader sends only the selected browser and validated profile intent to yt-dlp; cookie contents do not pass through the React frontend.

## Getting started

1. Launch Nice Downloader and complete engine and privacy onboarding.
2. Add one or more existing media folders under **Library**, or open **New Download** and paste an HTTP or HTTPS media URL.
3. Select **Analyze** to retrieve real metadata and available formats before downloading.
4. Choose a simple download intention, destination, and any optional advanced controls.
5. Start the download and follow it under **Downloads**.
6. Browse completed and existing media from **Home** or **Library**.

For a site that requires a signed-in session, open **Settings → Network & cookies**, select a browser in which you are already authenticated, optionally select a profile, save, and analyze the URL again. Close the browser first if its cookie database is locked.

## Privacy and security

Nice Downloader is local-first, but “local-first” does not mean “never uses the network”: downloads contact the source you provide, engine update checks contact their configured vendors, and temporary LAN sharing deliberately serves selected media to devices that possess its capability link.

Security boundaries implemented by the application include:

- React receives typed Tauri commands, not a generic shell or arbitrary filesystem API.
- URLs, filenames, metadata, proxy intent, and output paths are treated as untrusted input.
- Engine processes are started with argument arrays instead of shell-string interpolation.
- Playback, artwork, and image-viewer endpoints accept catalog IDs behind process-scoped tokens instead of frontend-supplied paths.
- Diagnostic exports sanitize URLs, headers, proxy details, cookie data, and other sensitive fields.
- Browser-cookie access is opt-in; managed cookie files and browser data stay backend-only.
- Local theme discovery and optional privacy-region suggestions use a fixed loopback Ollama service. Sampled analysis images are temporary and bounded.
- Temporary LAN shares are memory-only, time-limited, catalog-ID scoped, and stopped when the application exits. An authorized recipient can still retain bytes they receive.
- Destructive duplicate cleanup is separate from analysis, revalidates evidence, and uses recoverable platform Trash rather than exposing permanent deletion.

> [!WARNING]
> Private Library locking is application-level hiding, not disk encryption. Locked records are excluded from application search, playback activity, analysis, backups, and sharing, but the original files and local SQLite database remain readable to someone with filesystem access.

Do not include cookies, proxy credentials, private URLs, authentication headers, or unsanitized diagnostics in bug reports.

## Troubleshooting

### An engine is not detected

Open **Settings → Engines & diagnostics** and select **Detect again**. For a Slim or development build, confirm the required tools run in a terminal:

```bash
yt-dlp --version
ffmpeg -version
ffprobe -version
```

### A site returns 403, login, or rate-limit errors

Update the relevant engine, retry with an appropriate browser-cookie profile, and inspect the sanitized diagnostic details. A browser user agent does not replace valid cookies, and an IP-based block may require waiting or using a network path permitted by the site.

### Embedded playback still fails

Confirm the GStreamer packages listed under [Adaptive Linux playback](#adaptive-linux-playback) are installed when running from source or a native package. The player automatically tries temporary HLS and fragmented MP4 compatibility streams; its recovery panel can then offer libmpv, a cached H.264/AAC copy, native mpv, or the system player depending on availability.

### An AppImage built on Arch fails during packaging

Public artifacts are built by the Ubuntu release workflow. For a local Arch build, install Bubblewrap and use the repository wrapper:

```bash
sudo pacman -S --needed bubblewrap
npm run tauri:build:appimage:arch
```

See [Release and Distribution](docs/RELEASING.md) for the reason this wrapper is necessary.

## Development

### Requirements

- Node.js 22 or newer
- Current stable Rust toolchain with rustfmt and Clippy
- Git
- Python 3 with virtual-environment support for bundled gallery-dl staging
- Internet access for the first npm/Cargo install and bundled-engine preparation

Debian or Ubuntu build host:

```bash
sudo apt update
sudo apt install build-essential curl wget file libssl-dev \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libgstreamer1.0-dev \
  patchelf pkg-config \
  python3 python3-venv unzip gstreamer1.0-tools gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
  gstreamer1.0-libav gstreamer1.0-pulseaudio
```

Arch Linux build host:

```bash
sudo pacman -S --needed base-devel curl wget file openssl webkit2gtk-4.1 \
  libappindicator-gtk3 librsvg patchelf python python-pip unzip \
  gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav
```

### Run from source

```bash
npm ci
npm run tauri:dev
```

Development mode can discover compatible engines from app-managed storage, `PATH`, and standard user binary directories. Run `npm run engines:prepare` first when you need to test the exact pinned sidecars used by the Standard profile.

### Validate changes

```bash
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-fail-fast
```

Playwright shell coverage is also available:

```bash
npm run test:e2e
```

### Build Linux packages

```bash
# Standard profile with bundled engines
npm run tauri:build:bundled

# Engine-free profile
npm run tauri:build:slim

# Individual Debian packages
npm run tauri:build:bundled -- --bundles deb
npm run tauri:build:slim -- --bundles deb

# Arch-safe AppImages
sudo pacman -S --needed bubblewrap
npm run tauri:build:appimage:arch
npm run tauri:build:appimage:arch:slim
```

The base [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json) is intentionally engine-free. [`src-tauri/tauri.bundled.conf.json`](src-tauri/tauri.bundled.conf.json) is merged only for the Standard profile, preventing stale locally prepared engines from leaking into Slim artifacts. Generated binaries and build artifacts are not committed.

## Project documentation

- [`AGENTS.md`](AGENTS.md) — repository conventions, safety boundaries, and validation expectations
- [Architecture](docs/ARCHITECTURE.md) — subsystem ownership, persistence, process, playback, and security boundaries
- [Bundled engines](docs/BUNDLED_ENGINES.md) — Standard and Slim engine profiles
- [Local AI themes](docs/LOCAL_AI_THEMES.md) — optional VLM setup and theme-analysis behavior
- [Media organizer](docs/MEDIA_ORGANIZER.md) — Library design, invariants, and delivered milestones
- [Accessibility QA](docs/ACCESSIBILITY_QA.md) — keyboard, focus, contrast, motion, and assistive-technology checks
- [Library profiling](docs/LIBRARY_PROFILING.md) — repeatable large-library performance methodology
- [Release and Distribution](docs/RELEASING.md) — Linux packaging and release checks
- [UX](docs/UX.md) — interaction and progressive-disclosure principles
- [Roadmap](docs/ROADMAP.md) — delivered milestones and remaining work

Read the architecture and UX documentation before changing subsystem boundaries. Rust entry points should remain narrow, engines must receive argument arrays rather than shell strings, and the frontend must not gain direct process authority.

## License

No open-source license has been added yet. Until a `LICENSE` file is published, the source is available for inspection, but no general permission to copy, modify, or redistribute it is granted.
