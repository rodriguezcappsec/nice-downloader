# Nice Downloader

Nice Downloader is a private desktop media hub and download manager for Linux
and Windows. It brings downloaded media and existing local folders into a
browse-first library, while providing a task-focused interface over established
download tools such as
[yt-dlp](https://github.com/yt-dlp/yt-dlp),
[FFmpeg](https://ffmpeg.org/),
[gallery-dl](https://github.com/mikf/gallery-dl), and
[aria2](https://aria2.github.io/) instead of reimplementing media extraction.

The app opens into Media home, where videos and images are searchable and ready
to open in the system player or viewer. Downloading has its own workspace for
URL analysis, quality selection, active transfers, queue management, history,
and subscriptions. Rust owns engine execution, scanning, queue state,
cancellation, filesystem access, and SQLite persistence; the React frontend
never executes shell commands.

> Nice Downloader is an independent project. Download only media you are
> authorized to access, and follow the terms and laws that apply to you.

## Download

Builds are available from [GitHub Releases](https://github.com/rodriguezcappsec/nice-downloader/releases)
and from successful release workflows under
[GitHub Actions](https://github.com/rodriguezcappsec/nice-downloader/actions).

| Platform | Package | Use case |
| --- | --- | --- |
| Linux x86-64 | `Nice_Downloader-x86_64.AppImage` | Portable build for most Linux distributions |
| Debian/Ubuntu x86-64 | `Nice Downloader_0.1.0_amd64.deb` | Installed application-menu integration |
| Windows x86-64 | `*-setup.exe` | Standard NSIS installer |
| Windows x86-64 | `nice-downloader.exe` | Portable executable; no installation required |

### Linux AppImage

```bash
chmod +x Nice_Downloader-x86_64.AppImage
./Nice_Downloader-x86_64.AppImage
```

If FUSE is unavailable, use:

```bash
./Nice_Downloader-x86_64.AppImage --appimage-extract-and-run
```

### Debian and Ubuntu

```bash
sudo apt install "./Nice Downloader_0.1.0_amd64.deb"
```

### Windows

Run the NSIS `*-setup.exe` for a normal installation. For portable use, extract
the `nice-downloader-windows-portable-*` workflow artifact and launch
`nice-downloader.exe` directly. Settings and history are still stored in the
Windows application-data directory. Windows may display an Unknown Publisher
warning because current builds are not code-signed.

## Features

- Media-first home with visual shelves, source shortcuts, search, local artwork,
  and video/image browsing
- Non-destructive indexing for multiple local or external-drive folders
- Structured media analysis with title, thumbnail, uploader, duration, site,
  formats, codecs, resolution, FPS, HDR, and estimated size when available
- Simplified Best, 2160p, 1440p, 1080p, 720p, 480p, and audio-only choices
- Quality, compatibility, and size-oriented format recommendations
- Live progress, transferred bytes, speed, ETA, and current operation
- Rust-owned persistent queue with concurrency controls, reordering,
  cancellation, pause/resume where supported, retries, and startup recovery
- Batch URL analysis and bounded playlist expansion
- Subtitles, metadata, thumbnails, chapters, audio quality, codec preferences,
  output containers, filename templates, and reusable presets
- Searchable download history, duplicate prevention, and output verification
- Local media library with hierarchical folder filters, stable paginated sorting,
  FTS search, creator/series workspaces, related media, dynamic saved views,
  multi-value creator/series/site/tag and published-date discovery, sidecar
  metadata overrides, custom artwork, collections, change review, exact-duplicate analysis,
  recoverable Trash cleanup, previewable Move/Copy/Rename with Undo, and metadata
  backup/restore
- Integrated Watch workspace with embedded playback, resume positions,
  Continue Watching, subtitle tracks, searchable paginated browsing, and an
  persistent in-app mini player; compatibility copies require informed consent
  and have visible cache controls, with an explicit system-player fallback
- Dedicated two-to-four-video comparison grid with independent controls,
  single-audio focus, optional audio mixing, shared synchronization controls,
  compatibility consent per tile, and system-player fallbacks
- Timestamped moments and clip ranges with notes, color markers, one-click
  seeking, cancellable FFmpeg clip exports, and cached storyboard navigation
- Persistent named comparison sessions restore video order, positions, speeds,
  audio focus, multi-audio intent, and review notes; moment notes export as Markdown or CSV
- Two-to-four-image comparison studio with synchronized zoom/pan plus two-image
  overlay and split reveal modes
- Persistent media-version stacks for originals, edits, alternates, and generated
  copies, including version 4 backup/restore
- Integrated full-resolution image viewer with zoom, pan, rotation, keyboard
  browsing, slideshows, and explicit system-viewer fallback
- Scheduled playlist/channel subscriptions with archive-aware deduplication
- FFmpeg-powered media transformations with progress and cancellation
- Per-site browser-cookie, browser-profile, and proxy preferences
- Managed yt-dlp and gallery-dl versions with checksum verification, pinning,
  update checks, and rollback
- Dark/light themes, system tray behavior, diagnostics, and desktop notifications
- Keyboard workspace commands, shortcut help, interface scaling, reduced motion,
  high contrast, and assistive-technology status announcements

- Persistent Watch Later, private Recently Watched history, watched/reset
  controls, and optional non-looping Autoplay
- Ordered local video playlists with Play all, add/remove/reorder controls,
  non-looping Up next, and backup/restore

## Site and engine support

Ordinary media URLs are routed to yt-dlp, including YouTube, TikTok, X/Twitter,
Instagram, Reddit, Twitch, Vimeo, and other sites supported by the installed
yt-dlp version. Site behavior can change without an application update.

Nice Downloader also provides:

- Bunkr album inspection and individual-file selection through gallery-dl
- Resumable direct HTTP downloads through aria2c, with curl as a fallback
- HLS/DASH handling when exposed through a supported extraction engine
- Reddit share-link canonicalization and a limited public-embed fallback when
  Reddit blocks its API for the current IP address

## Media engines

Release builds ship verified, pinned copies of the core engines and show their versions under
**Settings → Engines & diagnostics**. Development builds also detect managed
engines, `PATH`, and standard user bin folders.

| Engine | Requirement | Purpose |
| --- | --- | --- |
| `yt-dlp` | Bundled | Analysis and media extraction |
| `ffmpeg` | Bundled | Audio/video merging and post-processing |
| `ffprobe` | Bundled | Output verification and media inspection |
| `gallery-dl` | Bundled | Bunkr and future gallery integrations |
| `aria2c` | Optional | Preferred direct HTTP transfer engine |
| `curl` | Optional fallback | Direct HTTP transfers when aria2c is unavailable |

`aria2c` remains an optional system accelerator; curl is used when available.
Developers can stage the pinned sidecars with `npm run engines:prepare`. The
generated binaries are ignored by Git and assembled by release CI from the
checksummed sources in `engines.lock.json`.

### Linux embedded playback

The integrated player uses WebKitGTK's GStreamer runtime. Nice Downloader
checks for `autoaudiosink` before opening a video and keeps **Open in system
player** available when the embedded runtime or a codec is missing. AV1 MP4
tracks with fractional timing that WebKitGTK can misinterpret are prepared as
a bounded, cached FFmpeg Matroska stream-copy. This routes the unchanged AV1
and audio streams through GStreamer's reliable Matroska timing path without
re-encoding or modifying the original file; the first embedded play may take
a moment while the compatibility copy is created. Before creating an uncached
copy, the player explains why it is needed, shows its estimated size and free
space, and offers **Create copy and play**, **Open original in system player**,
or **Cancel**. Settings can keep this behavior on Ask, make ordinary copies
automatic, or prevent automatic creation; copies of 2 GiB or larger are always
confirmed. Compatibility cache usage and clearing are available under
Settings > Desktop and accessibility.

If WebKitGTK still rejects the lossless AV1 timing copy, the player offers a
separate H.264/AAC MP4 fallback. This second-stage copy requires explicit
confirmation because it re-encodes video, may take several minutes, and may be
larger. It is never created by the Automatic stream-copy preference.

```bash
# Arch Linux
sudo pacman -S --needed gst-plugins-good gst-libav

# Debian or Ubuntu
sudo apt install gstreamer1.0-plugins-good gstreamer1.0-libav
```

`gst-plugins-good` supplies the required audio-output element. `gst-libav` is
recommended for broader codec support but is not required to build the app.

## Quick start

1. Launch Nice Downloader and complete the engine/privacy onboarding.
2. Open **New Download** and paste an HTTP or HTTPS media URL.
3. Select **Analyze** to inspect the media without downloading it.
4. Choose quality, container, destination, and any advanced options.
5. Start the download and monitor it from **Downloads** or **Queue**.
6. Open completed items from **History** or **Library**.

## Authentication and cookies

Some sites reject anonymous requests or require an authenticated session. Open
**Settings → Network & cookies**, select the browser in which you are already
signed in, optionally choose a profile, save, and analyze the URL again.

Close the browser first if its cookie database is locked. Nice Downloader does
not send cookie contents to the frontend and does not log them. Browser-cookie
access is opt-in, and managed cookie files remain backend-only.

## Troubleshooting

### An engine is not detected

Confirm it runs from a terminal, then select **Detect again** under
**Settings → Engines & diagnostics**:

```bash
yt-dlp --version
ffmpeg -version
ffprobe -version
```

### A site returns 403, login, or rate-limit errors

Update yt-dlp, retry with the appropriate browser-cookie profile, and review the
sanitized diagnostic details. A browser user agent alone does not replace valid
cookies, and IP-based blocks may require waiting or using a permitted network.

### Windows portable build does not start

Current Windows 10/11 systems normally include the Microsoft Edge WebView2
Runtime. Install WebView2 from Microsoft if it is missing. The portable
executable still needs access to the selected external media engines.

### AppImage fails on Arch Linux

Public release artifacts should be built by the Ubuntu GitHub Actions runner.
For a local Arch build, install Bubblewrap and use the repository workaround:

```bash
sudo pacman -S bubblewrap
npm run tauri:build:appimage:arch
```

See [Release and Distribution](docs/RELEASING.md) for details.

## Development

### Common requirements

- Node.js 22 or newer
- Current stable Rust toolchain
- Git
- Python 3 with virtual-environment support
- Internet access for the first npm/Cargo build and bundled-engine staging

Python is only needed to turn the pinned gallery-dl wheel into a standalone
sidecar. End users of release packages do not need Python.

### Linux build tools

Debian/Ubuntu build host:

```bash
sudo apt update
sudo apt install build-essential curl wget file libssl-dev \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf \
  python3 python3-venv unzip gstreamer1.0-plugins-good gstreamer1.0-libav
```

Arch Linux build host:

```bash
sudo pacman -S --needed base-devel curl wget file openssl webkit2gtk-4.1 \
  libappindicator-gtk3 librsvg patchelf python python-pip unzip \
  gstreamer gst-plugins-good gst-libav
```

Building an AppImage locally on Arch additionally requires `bubblewrap`; use
the documented `tauri:build:appimage:arch` command below.

### Windows build tools

Build Windows packages on Windows x86-64. Install:

- Node.js 22 or newer and Python 3, both available on `PATH`
- Rust with the `stable-x86_64-pc-windows-msvc` toolchain
- Visual Studio 2022 Build Tools with **Desktop development with C++** and a
  Windows 10/11 SDK
- Microsoft Edge WebView2 Runtime for local execution

The Tauri build downloads its NSIS tooling when needed. Cross-compiling the
complete Windows desktop package from Linux is not currently supported.

### Run from source

```bash
npm ci
npm run tauri:dev
```

Development mode can discover engines from `PATH`. To test the same verified
sidecars shipped in releases, run `npm run engines:prepare` first.

### Bundled engines and network downloads

Release builds must stage their engine payload before Tauri bundles the app:

```bash
npm run engines:prepare
```

This downloads the pinned Linux or Windows x86-64 yt-dlp, FFmpeg, ffprobe, and
gallery-dl inputs, verifies every SHA-256 digest from `engines.lock.json`, uses
`unzip` on Linux or PowerShell `Expand-Archive` on Windows, and builds
gallery-dl with the pinned PyInstaller version. Downloads are cached under
`.cache/bundled-engines/`; generated executables under
`src-tauri/resources/engines/` are intentionally not committed.

The first Tauri package build may also download platform bundlers such as
`linuxdeploy` or NSIS. The build does not install missing operating-system
packages automatically.

Validation commands:

```bash
npm run typecheck
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --all-targets --no-fail-fast
```

Build packages:

```bash
# Linux x86-64: stage verified engines, then build AppImage and DEB
npm run engines:prepare
npm run tauri:build -- --bundles appimage,deb

# Arch Linux AppImage workaround (also stages engines)
sudo pacman -S --needed bubblewrap
npm run tauri:build:appimage:arch

# Windows PowerShell on a Windows x86-64 host
npm run engines:prepare
npm run tauri:build -- --bundles nsis

# Windows portable executable and adjacent engines/ directory
npm run tauri:build -- --no-bundle
```

Do not distribute the Windows portable `.exe` by itself; include the adjacent
`src-tauri/target/release/engines/` directory. Build outputs are written below
`src-tauri/target/release/`, with installers under `bundle/`. Read
[Architecture](docs/ARCHITECTURE.md), [UX](docs/UX.md), and the
[Roadmap](docs/ROADMAP.md) before changing subsystem boundaries.

## Security

URLs, filenames, metadata, and output paths are treated as untrusted input.
Engine processes are started with argument arrays rather than shell command
strings. Please do not include cookies, proxy credentials, private URLs, or
authentication headers in bug reports. Sanitized diagnostic exports are the
preferred starting point for support requests.

## License

No open-source license has been added yet. Until a `LICENSE` file is published,
the source is available for inspection but no general permission to copy,
modify, or redistribute it is granted.
