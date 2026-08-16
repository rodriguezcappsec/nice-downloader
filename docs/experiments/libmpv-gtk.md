# Embedded libmpv GTK experiment

Status: phase-one feasibility passed on Linux Wayland. An opt-in phase-two Watch overlay prototype now lives on the experiment branch; it is still development evidence, not the default production playback path.

## Question

Can Nice Downloader decode and render video through libmpv inside an application-owned GTK OpenGL surface, without using WebKitGTK/GStreamer, creating a compatibility copy, or spawning the external `mpv` player?

## Prototype

`src-tauri/examples/libmpv_gtk.rs` is a Linux-only standalone experiment. It:

- dynamically loads `libmpv.so.2`, so normal Nice Downloader builds do not acquire a hard libmpv link requirement;
- creates an mpv client with `vo=libmpv` and `hwdec=auto-safe`;
- resolves OpenGL through EGL first and GLX second;
- renders new libmpv frames into a GTK 3 `GLArea` default framebuffer;
- drains the mpv event queue without blocking GTK;
- tears down the render context before the mpv client;
- accepts only an existing absolute regular file for the development-only command-line experiment;
- supports a bounded smoke interval and reports the number of actual `MPV_RENDER_UPDATE_FRAME` updates rendered.

The example is deliberately outside Tauri commands and the production binary. It does not weaken the catalog-ID boundary or expose a file-path command to React.

## Reproduction

The development host needs GTK 3 and a runtime libmpv installation. No libmpv Rust crate or compile-time libmpv linker dependency is used.

Generate a small AV1 + AAC fixture:

```sh
ffmpeg -hide_banner -loglevel error \
  -f lavfi -i testsrc2=size=640x360:rate=30 \
  -f lavfi -i sine=frequency=440:sample_rate=48000 \
  -t 8 -c:v libsvtav1 -preset 12 -crf 38 -pix_fmt yuv420p \
  -c:a aac -b:a 128k -y /tmp/nice-downloader-libmpv-av1-aac.mkv
```

Run the bounded smoke test:

```sh
cd src-tauri
cargo run --quiet --example libmpv_gtk -- \
  /tmp/nice-downloader-libmpv-av1-aac.mkv --smoke-seconds 4
```

## Observed result — 2026-08-15

Environment:

- `XDG_SESSION_TYPE=wayland`
- GTK backend `GdkWaylandDisplay`
- libmpv client API package version `2.5.0`
- mpv `0.41.0`
- fixture streams: AV1 video and AAC audio

Result:

```text
LIBMPV_GTK_RESULT frames=111 session=wayland display=wayland-0 gdk_backend=GdkWaylandDisplay media=/tmp/nice-downloader-libmpv-av1-aac.mkv
```

The process exited successfully after four seconds. The frame count measures libmpv new-frame update flags, not merely GTK repaint attempts. Missing CUDA and non-conformant RADV warnings were non-fatal; mpv selected an available fallback path.

## What this proves

- libmpv can decode the AV1 + AAC fixture that motivated the playback investigation.
- GTK 3 can host the libmpv OpenGL render API directly under native Wayland on this machine.
- Playback does not require WebKit media support, a remux/transcode copy, X11 `--wid`, or an external mpv process.
- Dynamic loading keeps the ordinary build working when libmpv development headers are absent.

## What remains before an in-app player

The current Tauri WebView already occupies the main GTK window's default box. Phase two must test reparenting that widget into a `gtk::Overlay` and positioning a `GLArea` over only the Watch video rectangle. That work must prove:

1. WebView focus, scrolling, dialogs, accessibility, and input continue working before, during, and after native video display.
2. Device-pixel coordinates remain correct under browser zoom, GTK scale factors, window resize, fullscreen, and the mini-player layout.
3. React sends only presentation bounds and catalog UUIDs; Rust still resolves and revalidates the media path.
4. libmpv events, pause/seek/volume/subtitle controls, resume persistence, end-of-file behavior, and shutdown are integrated without blocking GTK.
5. AppImage packaging includes a compatible libmpv dependency closure and license notices. libmpv's build configuration can make GPL obligations materially different from the current optional external executable.
6. Failure to load or initialize libmpv restores the WebView surface and falls back to the existing external mpv, compatibility-copy, and system-player paths.

## Decision

Proceed to a Linux-only phase-two Tauri overlay prototype. Do not replace the existing embedded or native-player paths yet. The decoder/render feasibility risk is cleared; GTK/WebView composition, lifecycle, packaging size, and licensing are now the gating risks.

## Phase-two Watch overlay prototype

The experiment branch installs a hidden GTK `GLArea` above the existing Tauri WebView and exposes it only after the user chooses **Embedded libmpv** in the Watch player. React sends a catalog UUID plus presentation geometry; Rust resolves and revalidates the source path. No filesystem path crosses the WebView command boundary.

Implemented prototype behavior:

- dynamically loaded libmpv renders in the Watch video rectangle without an external window or playback copy;
- `ResizeObserver`, scroll, fullscreen, browser viewport, device-pixel-ratio, and visibility changes resynchronize the native surface;
- the GTK overlay is input-pass-through, preserving WebView focus and keyboard handling;
- header controls provide play/pause, relative seek, playback rate, mute shortcut, and stop;
- position, duration, pause state, end-of-file, and failures flow back through bounded status polling;
- Rust persists resume progress and hides the native surface on stop, EOF, render failure, initialization failure, or component cleanup;
- opening the existing external mpv path closes embedded libmpv, and opening embedded libmpv closes external mpv;
- missing libmpv/OpenGL support leaves the WebView, compatibility-copy, external-mpv, and system-player paths available.

Run the full app with `npm run tauri:dev`, open a catalog video in Watch, and choose **Embedded libmpv**. For a video that triggers the compatibility dialog, cancel the copy first and select **Play here with libmpv** from the unchanged-video fallback panel. A successful run keeps video inside the Watch rectangle and shows the experimental controls in its header.

Still gated before making this a default player:

1. AppImage dependency closure and license review for the selected libmpv build.
2. Automated interaction coverage for modal overlap, accessibility tooling, non-default browser zoom, fractional HiDPI, mini-player movement, and fullscreen transitions.
3. A persisted user preference and direct compatibility-dialog action, rather than the current explicit experiment entry points.
4. Linux distribution coverage beyond the validated native Wayland host.
