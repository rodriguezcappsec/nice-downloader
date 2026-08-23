#!/usr/bin/env bash
set -u
set -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_profile="${1:-bundled}"
if [[ "$build_profile" != "bundled" && "$build_profile" != "slim" ]]; then
  echo "error: AppImage profile must be 'bundled' or 'slim'" >&2
  exit 2
fi
target_dir="${CARGO_TARGET_DIR:-$repo_root/src-tauri/target}"
bundle_dir="$target_dir/release/bundle/appimage"
app_dir="$bundle_dir/Nice Downloader.AppDir"
tauri_cache="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
linuxdeploy="$tauri_cache/linuxdeploy-x86_64.AppImage"
appimage_plugin="$tauri_cache/linuxdeploy-plugin-appimage.AppImage"

if [[ "$build_profile" == "bundled" && "${NICE_DOWNLOADER_ENGINES_PREPARED:-0}" != "1" ]]; then
  npm run engines:prepare || exit $?
fi

echo "Preparing the $build_profile AppImage directory with Tauri..."
echo "Note: Tauri's first linuxdeploy pass may fail on Arch; this script will continue from the generated AppDir."
(
  cd "$repo_root"
  if [[ "$build_profile" == "slim" ]]; then
    APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 npm run tauri:build:slim -- --bundles appimage
  else
    APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 npm run tauri:build:bundled:prepared -- --bundles appimage
  fi
)
tauri_status=$?

set -e

if [[ ! -d "$app_dir" ]]; then
  echo "error: Tauri failed before creating: $app_dir" >&2
  exit "$tauri_status"
fi

if [[ "$tauri_status" -ne 0 ]]; then
  echo "Recovered the expected initial linuxdeploy failure; continuing with the Arch-safe packaging pass."
fi

if [[ ! -x "$linuxdeploy" || ! -x "$appimage_plugin" ]]; then
  echo "error: Tauri's linuxdeploy tools were not found in: $tauri_cache" >&2
  if [[ "$tauri_status" -ne 0 ]]; then
    exit "$tauri_status"
  fi
  exit 1
fi

# Tauri/linuxdeploy bundles GStreamer core through WebKitGTK but does not copy
# its dynamically discovered plugins. Stage a matching, curated playback set
# before the final dependency scan so release AppImages do not hide or mix in
# incompatible host plugins.
"$repo_root/scripts/stage-appimage-gstreamer.sh" "$app_dir"

# linuxdeploy recursively runs ldd against AppDir ELF files. The bundled media
# engines are intentionally static, and ldd exits 1 for static ELFs. Preserve
# that payload outside the AppDir while dynamic application dependencies and
# GStreamer plugins are scanned, then restore it before AppImage creation.
engine_dir="$app_dir/usr/lib/Nice Downloader/engines"
if [[ "$build_profile" == "slim" && -d "$engine_dir" ]]; then
  rm -rf -- "$engine_dir"
fi
engine_staging="$(mktemp -d "${TMPDIR:-/tmp}/nice-downloader-engines.XXXXXX")"
if [[ -d "$engine_dir" ]]; then
  mv "$engine_dir" "$engine_staging/engines"
fi

restore_engines() {
  if [[ -d "$engine_staging/engines" ]]; then
    mkdir -p "$(dirname "$engine_dir")"
    mv "$engine_staging/engines" "$engine_dir"
  fi
  rmdir "$engine_staging" 2>/dev/null || true
}
trap restore_engines EXIT

# Remove a launcher left by an earlier rerun. linuxdeploy must generate its
# native AppRun first; the project wrapper is installed after this scan.
rm -f "$app_dir/AppRun.linuxdeploy"

mask_directory=""
cleanup_mask() {
  if [[ -n "$mask_directory" ]]; then
    rmdir "$mask_directory" 2>/dev/null || true
  fi
}
trap 'restore_engines; cleanup_mask' EXIT

linuxdeploy_command=(
  /usr/bin/env
  APPIMAGE_EXTRACT_AND_RUN=1
  NO_STRIP=1
  "$linuxdeploy"
  --appimage-extract-and-run
  --verbosity 1
  --appdir "$app_dir"
)

# linuxdeploy scans /usr/bin and can descend into host-specific symlinks.
# Mask an unreadable local service directory only inside this build namespace.
if [[ -e /var/lib/searxng && ! -r /var/lib/searxng ]]; then
  if ! command -v bwrap >/dev/null 2>&1; then
    echo "error: bubblewrap (bwrap) is required to mask unreadable /var/lib/searxng" >&2
    echo "Install it with: sudo pacman -S bubblewrap" >&2
    exit 1
  fi

  mask_directory="$(mktemp -d "${TMPDIR:-/tmp}/nice-downloader-searxng-mask.XXXXXX")"
  bwrap_args=(
    --ro-bind / /
    --dev-bind /dev /dev
    --proc /proc
    --bind /tmp /tmp
    --bind "$bundle_dir" "$bundle_dir"
    --ro-bind "$mask_directory" /var/lib/searxng
  )

  echo "Running linuxdeploy with the packaged playback runtime in an isolated filesystem view..."
  (
    cd "$bundle_dir"
    bwrap "${bwrap_args[@]}" -- "${linuxdeploy_command[@]}"
  )
else
  echo "Running linuxdeploy with the packaged playback runtime..."
  (
    cd "$bundle_dir"
    "${linuxdeploy_command[@]}"
  )
fi

restore_engines

if [[ "$build_profile" == "slim" && -d "$engine_dir" ]]; then
  echo "error: Slim AppImage staging unexpectedly contains bundled engines" >&2
  exit 1
fi

# libwayland-client is part of the host graphics boundary. Bundling the Ubuntu
# runner's copy can make WebKitGTK fail to create an EGL display on newer Linux
# distributions, leaving an otherwise healthy Tauri window completely white.
# Keep WebKitGTK and the curated GStreamer runtime packaged, but resolve this
# ABI-stable client library from the host alongside its compositor and drivers.
wayland_client_library="$(
  find "$app_dir/usr/lib" -maxdepth 2 \( -type f -o -type l \) \
    -name 'libwayland-client.so*' -print -quit
)"
if [[ -n "$wayland_client_library" ]]; then
  find "$app_dir/usr/lib" -maxdepth 2 \( -type f -o -type l \) \
    -name 'libwayland-client.so*' -delete
  echo "Excluded bundled libwayland-client so AppImage uses the host graphics boundary."
fi

if [[ -n "$(
  find "$app_dir/usr/lib" -maxdepth 2 \( -type f -o -type l \) \
    -name 'libwayland-client.so*' -print -quit
)" ]]; then
  echo "error: bundled libwayland-client remained in the AppDir" >&2
  exit 1
fi

gst_runtime_directory="$app_dir/usr/lib/gstreamer-1.0"
gst_inspector="$app_dir/usr/bin/gst-inspect-1.0"
appimage_library_path="$app_dir/usr/lib:$app_dir/usr/lib/x86_64-linux-gnu:$app_dir/usr/lib64"
if [[ ! -x "$gst_inspector" ]]; then
  echo "error: matching GStreamer inspector is missing from the AppDir" >&2
  exit 1
fi

if ! LC_ALL=C \
  LD_LIBRARY_PATH="$appimage_library_path" \
  GST_PLUGIN_SYSTEM_PATH="$gst_runtime_directory" \
  GST_PLUGIN_SYSTEM_PATH_1_0="$gst_runtime_directory" \
  GST_PLUGIN_SCANNER="$gst_runtime_directory/gst-plugin-scanner" \
  GST_PLUGIN_SCANNER_1_0="$gst_runtime_directory/gst-plugin-scanner" \
  "$gst_inspector" autoaudiosink >/dev/null 2>&1; then
  echo "error: packaged GStreamer inspector could not load autoaudiosink" >&2
  exit 1
fi

if [[ ! -x "$app_dir/AppRun" ]]; then
  echo "error: linuxdeploy did not create the AppDir launcher" >&2
  exit 1
fi

mv "$app_dir/AppRun" "$app_dir/AppRun.linuxdeploy"
install -m 755 "$repo_root/packaging/appimage/AppRun" "$app_dir/AppRun"

# Ensure the final plugin creates a fresh artifact containing the staged
# runtime rather than leaving the earlier Tauri-generated AppImage in place.
find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' -delete
(
  cd "$bundle_dir"
  APPIMAGE_EXTRACT_AND_RUN=1 "$appimage_plugin" \
    --appimage-extract-and-run --appdir "$app_dir"
)

appimage="$(find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [[ -z "$appimage" ]]; then
  echo "error: linuxdeploy exited successfully but produced no AppImage" >&2
  exit 1
fi

if [[ "$build_profile" == "slim" ]]; then
  slim_appimage="${appimage%.AppImage}-slim.AppImage"
  mv "$appimage" "$slim_appimage"
  appimage="$slim_appimage"
fi

echo "AppImage created: $appimage"
