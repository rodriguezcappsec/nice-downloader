use crate::{
    database::Database,
    engines::managed,
    errors::AppError,
    models::{LibraryMediaKind, LibrarySubtitleTrack},
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use std::{collections::HashMap, path::Path, process::Stdio};
use tokio::{
    io::AsyncReadExt,
    process::Command,
    time::{timeout, Duration},
};

const MAX_EMBEDDED_SUBTITLE_TRACKS: usize = 8;
const MAX_WEBVTT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct SubtitleProbe {
    #[serde(default)]
    streams: Vec<SubtitleProbeStream>,
}

#[derive(Debug, Deserialize)]
struct SubtitleProbeStream {
    index: u32,
    codec_name: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

pub async fn load_embedded_subtitles(
    database: &Database,
    item_id: &str,
) -> Result<Vec<LibrarySubtitleTrack>, AppError> {
    let item = database.get_library_item(item_id)?;
    if item.missing || item.media_kind != Some(LibraryMediaKind::Video) {
        return Err(AppError::new(
            "subtitle_video_required",
            "Embedded subtitles require an available Library video.",
        ));
    }
    let source = Path::new(&item.path);
    let metadata = source.symlink_metadata().map_err(|_| {
        AppError::new(
            "subtitle_video_unavailable",
            "The video is no longer available for subtitle inspection.",
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "invalid_subtitle_video",
            "Embedded subtitles require a regular, non-symlink video file.",
        ));
    }
    let ffprobe = managed::resolve("ffprobe").ok_or_else(|| {
        AppError::new(
            "ffprobe_missing",
            "Install FFprobe in Settings to inspect embedded subtitle tracks.",
        )
    })?;
    let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
        AppError::new(
            "ffmpeg_missing",
            "Install FFmpeg in Settings to decode embedded subtitle tracks.",
        )
    })?;

    let mut probe = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut probe);
    probe
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("s")
        .arg("-show_entries")
        .arg("stream=index,codec_name:stream_tags=language,title")
        .arg("-of")
        .arg("json")
        .arg(source)
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(12), probe.output())
        .await
        .map_err(|_| AppError::new("subtitle_probe_timeout", "Subtitle inspection timed out."))?
        .map_err(|error| AppError::internal("could not start FFprobe for subtitles", error))?;
    if !output.status.success() {
        return Err(AppError::new(
            "subtitle_probe_failed",
            "FFprobe could not inspect this video's subtitle tracks.",
        ));
    }
    let streams = parse_subtitle_streams(&output.stdout)?;
    let mut tracks = Vec::with_capacity(streams.len());
    for stream in streams.into_iter().take(MAX_EMBEDDED_SUBTITLE_TRACKS) {
        if let Some(contents) = extract_webvtt(&ffmpeg, source, stream.index).await? {
            let language = normalized_tag(stream.tags.get("language"), 15);
            let title = normalized_tag(stream.tags.get("title"), 80);
            let label = title.unwrap_or_else(|| match language.as_deref() {
                Some(language) => format!("Embedded · {}", language.to_ascii_uppercase()),
                None => format!("Embedded track {}", tracks.len() + 1),
            });
            tracks.push(LibrarySubtitleTrack {
                label,
                language,
                url: format!("data:text/vtt;base64,{}", STANDARD.encode(contents)),
            });
        }
    }
    Ok(tracks)
}

fn parse_subtitle_streams(bytes: &[u8]) -> Result<Vec<SubtitleProbeStream>, AppError> {
    let mut parsed: SubtitleProbe = serde_json::from_slice(bytes)
        .map_err(|error| AppError::internal("could not parse FFprobe subtitle metadata", error))?;
    parsed.streams.retain(|stream| {
        stream.codec_name.as_deref().is_some_and(|codec| {
            matches!(
                codec.to_ascii_lowercase().as_str(),
                "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "mov_text" | "text"
            )
        })
    });
    Ok(parsed.streams)
}

async fn extract_webvtt(
    ffmpeg: &Path,
    source: &Path,
    stream_index: u32,
) -> Result<Option<Vec<u8>>, AppError> {
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg(format!("0:{stream_index}"))
        .arg("-f")
        .arg("webvtt")
        .arg("pipe:1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal("could not start FFmpeg for subtitles", error))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::new(
            "subtitle_decode_io",
            "FFmpeg subtitle output was unavailable.",
        )
    })?;
    let mut contents = Vec::new();
    let read_result = timeout(
        Duration::from_secs(30),
        stdout.take(MAX_WEBVTT_BYTES + 1).read_to_end(&mut contents),
    )
    .await;
    if read_result.is_err() || contents.len() as u64 > MAX_WEBVTT_BYTES {
        let _ = child.kill().await;
        let _ = child.wait().await;
        return Ok(None);
    }
    read_result
        .expect("checked timeout")
        .map_err(|error| AppError::internal("could not read FFmpeg subtitle output", error))?;
    let status = match timeout(Duration::from_secs(5), child.wait()).await {
        Ok(result) => result
            .map_err(|error| AppError::internal("could not wait for FFmpeg subtitles", error))?,
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Ok(None);
        }
    };
    Ok((status.success() && contents.starts_with(b"WEBVTT")).then_some(contents))
}

fn normalized_tag(value: Option<&String>, maximum: usize) -> Option<String> {
    let value = value?.trim();
    (!value.is_empty()).then(|| value.chars().take(maximum).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_probe_accepts_text_tracks_and_rejects_bitmap_tracks() {
        let streams = parse_subtitle_streams(br#"{"streams":[{"index":2,"codec_name":"subrip","tags":{"language":"eng"}},{"index":3,"codec_name":"hdmv_pgs_subtitle","tags":{}},{"index":4,"codec_name":"ass","tags":{"title":"Commentary"}}]}"#)
            .expect("subtitle probe");
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].index, 2);
        assert_eq!(streams[1].index, 4);
    }
}
