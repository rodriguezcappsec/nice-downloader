# Engine build profiles

Nice Downloader supports two package profiles:

- **Bundled:** includes `yt-dlp`, `gallery-dl`, `ffmpeg`, and `ffprobe`, so an ordinary user does not need Python or separate media tools. This remains the explicit self-contained build option and the default `npm run tauri:build` profile.
- **Slim (release default):** contains only Nice Downloader. It discovers compatible tools installed through the operating system or the app-managed engine directory. Settings > Engines & diagnostics identifies missing required tools and shows the active source of every discovered executable. This is the profile published by the Linux release workflow.

Versions, URLs, and SHA-256 values for the Bundled profile are pinned in `engines.lock.json`; generated binaries are never committed to Git. Build either profile with:

```sh
npm run tauri:build:bundled
npm run tauri:build:slim
```

Bundle arguments can be forwarded normally, for example:

```sh
npm run tauri:build:bundled -- --bundles deb
npm run tauri:build:slim -- --bundles appimage
```

On Arch Linux, use `npm run tauri:build:appimage:arch` for the Bundled AppImage or `npm run tauri:build:appimage:arch:slim` for the engine-free AppImage. Both use the repository's Bubblewrap-based `linuxdeploy` workaround and stage the curated playback runtime; the Slim variant removes and verifies the absence of engine resources before generating its distinctly named artifact.

The base `src-tauri/tauri.conf.json` intentionally contains no engine resources. `src-tauri/tauri.bundled.conf.json` is merged only for the Bundled profile. This prevents a stale local `resources/engines/` directory from accidentally entering a Slim package.

The preparation script supports Linux x86_64 only. It downloads the official standalone yt-dlp executable, extracts pinned FFmpeg/ffprobe archives, and builds gallery-dl into a one-file PyInstaller executable. The build machine therefore needs Python, venv support, and `unzip`; end users do not.

Both profiles use the same runtime and resolve engines in this order:

1. app-managed updated version;
2. packaged engine resource, when present;
3. host `PATH` and standard user executable directories.

This keeps Bundled packages immediately usable while allowing fast extractor updates and distribution-managed tools in Slim releases. The Settings engine list reports **Managed**, **Bundled**, or **System** for each active executable.

Before changing a pinned engine, review its release, update the relevant URL and digest together, run the preparation command, and execute every staged binary's version check. FFmpeg builds used here enable GPL components, so release notices and source/license obligations must be reviewed before public distribution.
