#!/usr/bin/env bash
set -u
set -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/src-tauri/target}"
bundle_dir="$target_dir/release/bundle/appimage"
app_dir="$bundle_dir/Nice Downloader.AppDir"
tauri_cache="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
linuxdeploy="$tauri_cache/linuxdeploy-x86_64.AppImage"
appimage_plugin="$tauri_cache/linuxdeploy-plugin-appimage.AppImage"

npm run engines:prepare || exit $?

if ! command -v bwrap >/dev/null 2>&1; then
  echo "error: bubblewrap (bwrap) is required for the AppImage build" >&2
  echo "Install it with: sudo pacman -S bubblewrap" >&2
  exit 1
fi

echo "Preparing the AppImage directory with Tauri..."
(
  cd "$repo_root"
  APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 npm run tauri:build -- --bundles appimage
)
tauri_status=$?

set -e

if [[ ! -d "$app_dir" ]]; then
  echo "error: Tauri failed before creating: $app_dir" >&2
  exit "$tauri_status"
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

mask_directory="$(mktemp -d "${TMPDIR:-/tmp}/nice-downloader-searxng-mask.XXXXXX")"
cleanup_mask() {
  rmdir "$mask_directory" 2>/dev/null || true
}
trap 'restore_engines; cleanup_mask' EXIT

bwrap_args=(
  --ro-bind / /
  --dev-bind /dev /dev
  --proc /proc
  --bind /tmp /tmp
  --bind "$bundle_dir" "$bundle_dir"
)

# linuxdeploy scans /usr/bin and can descend into host-specific symlinks.
# Mask an unreadable local service directory only inside this build namespace.
if [[ -e /var/lib/searxng && ! -r /var/lib/searxng ]]; then
  bwrap_args+=(--ro-bind "$mask_directory" /var/lib/searxng)
fi

echo "Running linuxdeploy with the packaged playback runtime..."
(
  cd "$bundle_dir"
  bwrap "${bwrap_args[@]}" -- /usr/bin/env \
    APPIMAGE_EXTRACT_AND_RUN=1 \
    NO_STRIP=1 \
    "$linuxdeploy" \
    --appimage-extract-and-run \
    --verbosity 1 \
    --appdir "$app_dir"
)

restore_engines

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

echo "AppImage created: $appimage"
