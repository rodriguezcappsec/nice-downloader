# Nice Downloader

Nice Downloader is a Linux-first desktop media download manager built with Tauri 2, React, TypeScript, Rust, and SQLite. It provides a friendly intent-based interface over yt-dlp and FFmpeg without reimplementing media extraction.

Milestone 1 supports structured URL analysis, simplified quality selection, real downloads, live progress/speed/ETA, cancellation, PATH-based engine discovery, and completed-download History. Settings also provides opt-in browser cookies and conditional browser impersonation for sites such as Reddit, Instagram, TikTok, and X that may reject anonymous engine requests.

## Prerequisites

- Node.js 20 or newer
- Current stable Rust toolchain
- Tauri's Linux system dependencies (GTK 3 and WebKitGTK 4.1)
- `yt-dlp`, `ffmpeg`, and `ffprobe` available in `PATH`

On Arch Linux, the desktop dependencies are provided by packages such as `webkit2gtk-4.1`, `gtk3`, and the normal Tauri build toolchain. Use the equivalent packages for your distribution.

## Development

```bash
npm install
npm run tauri:dev
```

Frontend-only checks and Rust checks can be run independently:

```bash
npm run typecheck
npm run build
cd src-tauri
cargo fmt --all -- --check
cargo test --all-targets
```

Build the desktop binary with:

```bash
npm run tauri:build
```

The application stores its SQLite database in the platform application-data directory. See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for boundaries and [docs/ROADMAP.md](docs/ROADMAP.md) for planned milestones.

For a site that requires authentication, open **Settings → Site access**, select the browser where you are signed in, save, and analyze the URL again. Close the browser first if its cookie database is locked. Nice Downloader stores only the selected browser name; yt-dlp reads the local browser profile directly.
