# Bundled engines

Release packages include `yt-dlp`, `gallery-dl`, `ffmpeg`, and `ffprobe`, so an
ordinary user does not need Python or separate media tools. Versions, URLs, and
SHA-256 values are pinned in `engines.lock.json`; generated binaries are never
committed to Git.

Run:

```sh
npm run engines:prepare
npm run tauri:build
```

The preparation script supports Linux x86_64 and Windows x86_64. It downloads
the official standalone yt-dlp executable, extracts pinned FFmpeg/ffprobe
archives, and builds gallery-dl into a one-file PyInstaller executable. A build
machine therefore needs Python, venv support, and (on Linux) `unzip`; end users
do not.

The Windows portable distribution is the executable together with its adjacent
`engines/` directory. Moving only `nice-downloader.exe` removes the bundled
baseline; the NSIS installer places both automatically.

At runtime Rust resolves engines in this order:

1. an app-managed updated version;
2. the packaged engine resource;
3. the host `PATH` and standard user executable directories.

This keeps releases immediately usable while allowing fast extractor updates.
Before changing a pinned engine, review its release, update the relevant URL and
digest together, run the preparation command, and execute every staged binary's
version check. FFmpeg builds used here enable GPL components, so release notices
and source/license obligations must be reviewed before public distribution.
