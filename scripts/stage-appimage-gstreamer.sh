#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <AppDir>" >&2
  exit 2
fi

app_dir="$1"
plugin_target="$app_dir/usr/lib/gstreamer-1.0"
inspector_target="$app_dir/usr/bin/gst-inspect-1.0"

for executable in gst-inspect-1.0 pkg-config; do
  if ! command -v "$executable" >/dev/null 2>&1; then
    echo "error: $executable is required to stage AppImage playback support" >&2
    exit 1
  fi
done

if ! pkg-config --exists gstreamer-1.0; then
  echo "error: GStreamer development metadata is required to stage AppImage playback support" >&2
  echo "Install libgstreamer1.0-dev on Debian or Ubuntu." >&2
  exit 1
fi

plugin_source="$(pkg-config --variable=pluginsdir gstreamer-1.0)"
scanner_directory="$(pkg-config --variable=pluginscannerdir gstreamer-1.0)"
scanner_source="$scanner_directory/gst-plugin-scanner"

if [[ -z "$plugin_source" || -z "$scanner_directory" || ! -d "$plugin_source" || ! -x "$scanner_source" ]]; then
  echo "error: the GStreamer plugin directory or plugin scanner is unavailable" >&2
  exit 1
fi

mkdir -p "$plugin_target" "$(dirname "$inspector_target")"

declare -A staged_plugins=()

stage_element() {
  local element="$1"
  local required="${2:-required}"
  local inspection plugin_file plugin_name

  if ! inspection="$(LC_ALL=C gst-inspect-1.0 "$element" 2>/dev/null)"; then
    if [[ "$required" == "required" ]]; then
      echo "error: required GStreamer element is unavailable: $element" >&2
      exit 1
    fi
    return 1
  fi

  plugin_file="$(awk '$1 == "Filename" { print $2; exit }' <<<"$inspection")"
  if [[ -z "$plugin_file" || ! -f "$plugin_file" ]]; then
    if [[ "$required" == "required" ]]; then
      echo "error: could not locate the plugin that provides: $element" >&2
      exit 1
    fi
    return 1
  fi

  plugin_name="$(basename "$plugin_file")"
  if [[ -z "${staged_plugins[$plugin_name]:-}" ]]; then
    install -m 755 "$plugin_file" "$plugin_target/$plugin_name"
    staged_plugins[$plugin_name]=1
  fi
}

# Pipeline, conversion, demuxing, parsing, and software decoding required by
# WebKitGTK's HTML5 player for the Library formats Nice Downloader indexes.
required_elements=(
  appsrc playbin typefind typefindfunctions volume scaletempo
  audioconvert audioresample videoconvert videoscale
  autoaudiosink
  opengl subenc
  qtdemux matroskademux avidemux oggdemux tsdemux
  h264parse h265parse av1parse vp9parse
  aacparse opusparse mpegaudioparse
  avdec_h264 avdec_h265 avdec_vp9
  avdec_aac avdec_opus avdec_mp3 avdec_mpeg4
  subparse
)

for element in "${required_elements[@]}"; do
  stage_element "$element"
done

audio_sink_found=false
for element in pulsesink pipewiresink alsasink; do
  if stage_element "$element" optional; then
    audio_sink_found=true
  fi
done
if [[ "$audio_sink_found" != true ]]; then
  echo "error: no supported GStreamer audio sink is available" >&2
  exit 1
fi

av1_decoder_found=false
for element in av1dec dav1ddec avdec_av1; do
  if stage_element "$element" optional; then
    av1_decoder_found=true
  fi
done
if [[ "$av1_decoder_found" != true ]]; then
  echo "error: no GStreamer AV1 decoder is available" >&2
  exit 1
fi

install -m 755 "$scanner_source" "$plugin_target/gst-plugin-scanner"
install -m 755 "$(command -v gst-inspect-1.0)" "$inspector_target"

echo "Staged ${#staged_plugins[@]} GStreamer plugins, scanner, and inspector."
