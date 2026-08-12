#!/usr/bin/env bash

set -u
set -o pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target_dir="${CARGO_TARGET_DIR:-$repo_root/src-tauri/target}"
bundle_dir="$target_dir/release/bundle/appimage"
app_dir="$bundle_dir/Nice Downloader.AppDir"
tauri_cache="${XDG_CACHE_HOME:-$HOME/.cache}/tauri"
linuxdeploy="$tauri_cache/linuxdeploy-x86_64.AppImage"

if ! command -v bwrap >/dev/null 2>&1; then
  echo "error: bubblewrap (bwrap) is required for the Arch AppImage workaround" >&2
  echo "Install it with: sudo pacman -S bubblewrap" >&2
  exit 1
fi

echo "Preparing the AppImage directory with Tauri..."
(
  cd "$repo_root"
  APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=1 npm run tauri:build -- --bundles appimage
)
tauri_status=$?

if [[ $tauri_status -eq 0 ]]; then
  echo "Tauri created the AppImage without needing the Arch workaround."
  exit 0
fi

set -e

if [[ ! -d "$app_dir" ]]; then
  echo "error: Tauri failed before creating: $app_dir" >&2
  exit "$tauri_status"
fi

if [[ ! -x "$linuxdeploy" ]]; then
  echo "error: Tauri's linuxdeploy executable was not found at: $linuxdeploy" >&2
  exit "$tauri_status"
fi

mask_dir="$(mktemp -d "${TMPDIR:-/tmp}/nice-downloader-searxng-mask.XXXXXX")"
trap 'rmdir "$mask_dir" 2>/dev/null || true' EXIT

bwrap_args=(
  --ro-bind / /
  --dev-bind /dev /dev
  --proc /proc
  --bind /tmp /tmp
  --bind "$bundle_dir" "$bundle_dir"
)

# linuxdeploy scans /usr/bin and follows this host-specific symlink. Some Arch
# systems intentionally make its /var/lib target unreadable to normal users.
if [[ -e /var/lib/searxng ]]; then
  bwrap_args+=(--ro-bind "$mask_dir" /var/lib/searxng)
fi

echo "Retrying linuxdeploy in an isolated filesystem view..."
(
  cd "$bundle_dir"
  bwrap "${bwrap_args[@]}" -- /usr/bin/env \
    APPIMAGE_EXTRACT_AND_RUN=1 \
    NO_STRIP=1 \
    "$linuxdeploy" \
    --appimage-extract-and-run \
    --verbosity 1 \
    --appdir "$app_dir" \
    --output appimage
)

appimage="$(find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' -print -quit)"
if [[ -z "$appimage" ]]; then
  echo "error: linuxdeploy exited successfully but produced no AppImage" >&2
  exit 1
fi

echo "AppImage created: $appimage"
