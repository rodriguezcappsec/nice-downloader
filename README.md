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
- Local media library with hierarchical folder filters, tags, collections,
  change review, exact-duplicate analysis, recoverable Trash cleanup,
  previewable Move/Copy/Rename with Undo, and metadata backup/restore
- Scheduled playlist/channel subscriptions with archive-aware deduplication
- FFmpeg-powered media transformations with progress and cancellation
- Per-site browser-cookie, browser-profile, and proxy preferences
- Managed yt-dlp and gallery-dl versions with checksum verification, pinning,
  update checks, and rollback
- Dark/light themes, system tray behavior, diagnostics, and desktop notifications
- Keyboard workspace commands, shortcut help, interface scaling, reduced motion,
  high contrast, and assistive-technology status announcements

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

Requirements:

- Node.js 22 or newer
- Current stable Rust toolchain
- Tauri 2 platform dependencies
- GTK 3 and WebKitGTK 4.1 development packages on Linux

```bash
npm ci
npm run tauri:dev
```

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
# Linux AppImage and DEB
npm run tauri:build -- --bundles appimage,deb

# Windows PowerShell on a Windows host
npm run tauri:build -- --bundles nsis

# Windows standalone executable, without an installer
npm run tauri:build -- --no-bundle
```

Build outputs are written below `src-tauri/target/release/`. Read
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
