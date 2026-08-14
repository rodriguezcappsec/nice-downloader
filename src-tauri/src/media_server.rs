use crate::{
    database::Database,
    errors::AppError,
    models::{LibraryMediaKind, LibraryPlaybackSession, LibrarySubtitleTrack},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_util::TryStreamExt;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Empty, StreamBody};
use hyper::{
    body::{Bytes, Frame, Incoming},
    header::{
        ACCEPT_RANGES, ACCESS_CONTROL_ALLOW_ORIGIN, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE,
        CONTENT_TYPE, RANGE,
    },
    server::conn::http1,
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use std::{
    convert::Infallible,
    error::Error,
    ffi::OsStr,
    io::SeekFrom,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener},
    path::{Path, PathBuf},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
    net::TcpListener,
    sync::OnceCell,
    time::{timeout, Duration},
};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

type BoxError = Box<dyn Error + Send + Sync>;
type MediaBody = UnsyncBoxBody<Bytes, BoxError>;

#[derive(Clone)]
pub struct MediaServerState {
    database: Database,
    base_url: Option<String>,
    token: String,
    webview_dependency_error: std::sync::Arc<OnceCell<Option<String>>>,
}

impl MediaServerState {
    pub fn unavailable(database: Database) -> Self {
        Self {
            database,
            base_url: None,
            token: Uuid::new_v4().to_string(),
            webview_dependency_error: std::sync::Arc::new(OnceCell::new()),
        }
    }

    pub async fn prepare_webview_playback(
        &self,
        item_id: &str,
    ) -> Result<LibraryPlaybackSession, AppError> {
        let dependency_error = self
            .webview_dependency_error
            .get_or_init(detect_webview_dependency_error)
            .await;
        if let Some(message) = dependency_error {
            return Err(AppError::new(
                "embedded_playback_dependency_missing",
                message.clone(),
            ));
        }
        self.prepare_playback(item_id)
    }

    pub fn prepare_playback(&self, item_id: &str) -> Result<LibraryPlaybackSession, AppError> {
        let base_url = self.base_url.as_deref().ok_or_else(|| {
            AppError::new(
                "embedded_playback_unavailable",
                "Embedded playback is unavailable on this system. Open the video in your system player instead.",
            )
        })?;
        Uuid::parse_str(item_id).map_err(|_| {
            AppError::new(
                "invalid_identifier",
                "The library item identifier is invalid.",
            )
        })?;
        let item = self.database.get_library_item(item_id)?;
        if item.media_kind != Some(LibraryMediaKind::Video) {
            return Err(AppError::new(
                "library_video_not_found",
                "Choose a video from the media library.",
            ));
        }
        validate_regular_media_file(Path::new(&item.path))?;
        let subtitles = discover_subtitles(Path::new(&item.path))
            .into_iter()
            .map(|track| LibrarySubtitleTrack {
                label: track.label,
                language: track.language,
                url: format!(
                    "{}/subtitle/{}/{}?token={}",
                    base_url,
                    item.id,
                    URL_SAFE_NO_PAD.encode(track.file_name.as_bytes()),
                    self.token
                ),
            })
            .collect();
        Ok(LibraryPlaybackSession {
            playback: self.database.get_library_playback(item_id)?,
            url: format!("{}/media/{}?token={}", base_url, item.id, self.token),
            item,
            subtitles,
        })
    }
}

pub fn start(database: Database) -> Result<MediaServerState, AppError> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let listener = StdTcpListener::bind(address)
        .map_err(|error| AppError::internal("could not start the local media player", error))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| AppError::internal("could not configure the local media player", error))?;
    let local_address = listener.local_addr().map_err(|error| {
        AppError::internal("could not resolve the local media player address", error)
    })?;
    let state = MediaServerState {
        database,
        base_url: Some(format!("http://127.0.0.1:{}", local_address.port())),
        token: Uuid::new_v4().to_string(),
        webview_dependency_error: std::sync::Arc::new(OnceCell::new()),
    };
    let server_state = state.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::from_std(listener) {
            Ok(listener) => listener,
            Err(error) => {
                tracing::error!(%error, "could not attach the local media player listener");
                return;
            }
        };
        loop {
            let (stream, peer) = match listener.accept().await {
                Ok(connection) => connection,
                Err(error) => {
                    tracing::warn!(%error, "local media player stopped accepting connections");
                    break;
                }
            };
            if !peer.ip().is_loopback() {
                tracing::warn!(%peer, "rejected non-loopback media request");
                continue;
            }
            let request_state = server_state.clone();
            tauri::async_runtime::spawn(async move {
                let service = service_fn(move |request| {
                    let request_state = request_state.clone();
                    async move { Ok::<_, Infallible>(handle_request(request, request_state).await) }
                });
                if let Err(error) = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    tracing::debug!(%error, "local media connection ended");
                }
            });
        }
    });
    tracing::info!(address = %local_address, "local media player ready");
    Ok(state)
}

#[cfg(target_os = "linux")]
async fn detect_webview_dependency_error() -> Option<String> {
    let mut command = tokio::process::Command::new("gst-inspect-1.0");
    command.arg("autoaudiosink").kill_on_drop(true);
    match timeout(Duration::from_secs(3), command.output()).await {
        Ok(Ok(output)) if output.status.success() => None,
        Ok(Ok(_)) => Some(
            "Embedded playback needs the GStreamer audio-output plugins used by WebKitGTK. Install the package that provides autoaudiosink (Arch Linux: gst-plugins-good; Debian/Ubuntu: gstreamer1.0-plugins-good), restart Nice Downloader, or open this video in your system player."
                .to_owned(),
        ),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            // Some distributions ship the WebKitGTK GStreamer runtime without the
            // gst-inspect utility. Let WebKit try playback and rely on the UI timeout.
            None
        }
        Ok(Err(error)) => {
            tracing::debug!(%error, "could not inspect the WebKitGTK audio output plugin");
            None
        }
        Err(_) => {
            tracing::debug!("timed out while inspecting the WebKitGTK audio output plugin");
            None
        }
    }
}

#[cfg(not(target_os = "linux"))]
async fn detect_webview_dependency_error() -> Option<String> {
    None
}

async fn handle_request(
    request: Request<Incoming>,
    state: MediaServerState,
) -> Response<MediaBody> {
    if request.method() == Method::OPTIONS {
        return response(StatusCode::NO_CONTENT, empty_body());
    }
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return response(StatusCode::METHOD_NOT_ALLOWED, empty_body());
    }
    let token_matches = request
        .uri()
        .query()
        .and_then(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .find(|(key, _)| key == "token")
                .map(|(_, value)| value == state.token)
        })
        .unwrap_or(false);
    if !token_matches {
        return response(StatusCode::UNAUTHORIZED, empty_body());
    }
    let segments = request
        .uri()
        .path()
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let resolved = match segments.as_slice() {
        ["media", item_id] => resolve_media_path(&state.database, item_id),
        ["subtitle", item_id, encoded_name] => {
            resolve_subtitle_path(&state.database, item_id, encoded_name)
        }
        _ => return response(StatusCode::NOT_FOUND, empty_body()),
    };
    let (path, content_type) = match resolved {
        Ok(value) => value,
        Err(status) => return response(status, empty_body()),
    };
    match serve_file(&request, &path, content_type).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "could not stream a library media file");
            response(StatusCode::INTERNAL_SERVER_ERROR, empty_body())
        }
    }
}

fn resolve_media_path(
    database: &Database,
    item_id: &str,
) -> Result<(PathBuf, &'static str), StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let item = database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if item.missing || item.media_kind != Some(LibraryMediaKind::Video) {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = PathBuf::from(item.path);
    validate_regular_media_file(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((path.clone(), video_content_type(&path)))
}

fn resolve_subtitle_path(
    database: &Database,
    item_id: &str,
    encoded_name: &str,
) -> Result<(PathBuf, &'static str), StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let item = database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let media_path = PathBuf::from(item.path);
    validate_regular_media_file(&media_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded_name)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let file_name = std::str::from_utf8(&decoded).map_err(|_| StatusCode::NOT_FOUND)?;
    if file_name.is_empty()
        || file_name.contains('/')
        || file_name.contains('\\')
        || Path::new(file_name).file_name() != Some(OsStr::new(file_name))
        || !subtitle_name_matches(&media_path, file_name)
    {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = media_path
        .parent()
        .ok_or(StatusCode::NOT_FOUND)?
        .join(file_name);
    validate_regular_media_file(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    Ok((path, "text/vtt; charset=utf-8"))
}

async fn serve_file(
    request: &Request<Incoming>,
    path: &Path,
    content_type: &'static str,
) -> Result<Response<MediaBody>, BoxError> {
    let mut file = File::open(path).await?;
    let length = file.metadata().await?.len();
    if length == 0 {
        return Ok(response(StatusCode::NOT_FOUND, empty_body()));
    }
    let requested_range = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok());
    let (start, end, status) = match requested_range {
        Some(value) => match parse_single_range(value, length) {
            Some((start, end)) => (start, end, StatusCode::PARTIAL_CONTENT),
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(CONTENT_RANGE, format!("bytes */{length}"))
                    .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                    .body(empty_body())?)
            }
        },
        None => (0, length - 1, StatusCode::OK),
    };
    let response_length = end - start + 1;
    let mut builder = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, content_type)
        .header(CONTENT_LENGTH, response_length)
        .header(ACCEPT_RANGES, "bytes")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if status == StatusCode::PARTIAL_CONTENT {
        builder = builder.header(CONTENT_RANGE, format!("bytes {start}-{end}/{length}"));
    }
    if request.method() == Method::HEAD {
        return Ok(builder.body(empty_body())?);
    }
    file.seek(SeekFrom::Start(start)).await?;
    let stream = ReaderStream::new(file.take(response_length))
        .map_ok(Frame::data)
        .map_err(|error| -> BoxError { Box::new(error) });
    Ok(builder.body(StreamBody::new(stream).boxed_unsync())?)
}

fn parse_single_range(value: &str, length: u64) -> Option<(u64, u64)> {
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?.min(length);
        if suffix == 0 {
            return None;
        }
        return Some((length - suffix, length - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= length {
        return None;
    }
    let end = if end.is_empty() {
        length - 1
    } else {
        end.parse::<u64>().ok()?.min(length - 1)
    };
    (end >= start).then_some((start, end))
}

fn response(status: StatusCode, body: MediaBody) -> Response<MediaBody> {
    Response::builder()
        .status(status)
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap_or_else(|_| Response::new(empty_body()))
}

fn empty_body() -> MediaBody {
    Empty::<Bytes>::new()
        .map_err(|never| -> BoxError { match never {} })
        .boxed_unsync()
}

fn validate_regular_media_file(path: &Path) -> Result<(), AppError> {
    if !path.is_absolute() {
        return Err(AppError::new(
            "library_file_not_found",
            "This media file is no longer available at its saved location.",
        ));
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| {
        AppError::new(
            "library_file_not_found",
            "This media file is no longer available at its saved location.",
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "library_file_not_found",
            "This media file is not a regular file that can be played safely.",
        ));
    }
    Ok(())
}

fn video_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "m4v" => "video/x-m4v",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        _ => "application/octet-stream",
    }
}

struct DiscoveredSubtitle {
    file_name: String,
    label: String,
    language: Option<String>,
}

fn discover_subtitles(media_path: &Path) -> Vec<DiscoveredSubtitle> {
    let Some(parent) = media_path.parent() else {
        return Vec::new();
    };
    let mut tracks = std::fs::read_dir(parent)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name().to_str()?.to_owned();
            if !subtitle_name_matches(media_path, &file_name) {
                return None;
            }
            let metadata = entry.path().symlink_metadata().ok()?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return None;
            }
            let media_stem = media_path.file_stem()?.to_str()?;
            let suffix = file_name
                .strip_prefix(media_stem)?
                .strip_suffix(".vtt")?
                .trim_matches('.');
            let language = (!suffix.is_empty()
                && suffix.len() <= 15
                && suffix
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-'))
            .then(|| suffix.to_ascii_lowercase());
            Some(DiscoveredSubtitle {
                file_name,
                label: language
                    .as_deref()
                    .map(language_label)
                    .unwrap_or("Subtitles")
                    .to_owned(),
                language,
            })
        })
        .take(20)
        .collect::<Vec<_>>();
    tracks.sort_by(|left, right| left.file_name.cmp(&right.file_name));
    tracks
}

fn subtitle_name_matches(media_path: &Path, file_name: &str) -> bool {
    let Some(media_stem) = media_path.file_stem().and_then(OsStr::to_str) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".vtt")
        && (file_name == format!("{media_stem}.vtt")
            || file_name.starts_with(&format!("{media_stem}.")))
}

fn language_label(language: &str) -> &str {
    match language.split('-').next().unwrap_or(language) {
        "en" => "English",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "zh" => "Chinese",
        _ => language,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LibraryItem, LibraryMetadataState};
    use chrono::Utc;
    use tempfile::tempdir;

    #[test]
    fn parses_single_http_byte_ranges() {
        assert_eq!(parse_single_range("bytes=0-99", 1_000), Some((0, 99)));
        assert_eq!(parse_single_range("bytes=900-", 1_000), Some((900, 999)));
        assert_eq!(parse_single_range("bytes=-100", 1_000), Some((900, 999)));
        assert_eq!(
            parse_single_range("bytes=900-1200", 1_000),
            Some((900, 999))
        );
        assert_eq!(parse_single_range("bytes=1000-", 1_000), None);
        assert_eq!(parse_single_range("bytes=0-1,4-5", 1_000), None);
    }

    #[test]
    fn accepts_only_matching_webvtt_sidecars() {
        let media = Path::new("/media/movie.mp4");
        assert!(subtitle_name_matches(media, "movie.vtt"));
        assert!(subtitle_name_matches(media, "movie.en.vtt"));
        assert!(!subtitle_name_matches(media, "another.vtt"));
        assert!(!subtitle_name_matches(media, "movie.srt"));
    }

    #[tokio::test]
    async fn streams_only_the_requested_video_range() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("video.mp4");
        let bytes = (0..=255u8).cycle().take(4_096).collect::<Vec<_>>();
        std::fs::write(&path, &bytes).expect("video fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "Range fixture".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(10.0),
            site: None,
            tags: vec![],
            metadata: serde_json::json!({}),
            file_size_bytes: Some(bytes.len() as u64),
            modified_at_ms: None,
            width: Some(320),
            height: Some(180),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        database.upsert_library_item(&item).expect("catalog item");
        let server = start(database).expect("media server");
        let session = server.prepare_playback(&item.id).expect("playback session");
        let response = reqwest::Client::new()
            .get(session.url)
            .header(reqwest::header::RANGE, "bytes=100-199")
            .send()
            .await
            .expect("range response");
        assert_eq!(response.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|value| value.to_str().ok()),
            Some("bytes 100-199/4096")
        );
        assert_eq!(
            response.bytes().await.expect("range bytes"),
            bytes[100..200]
        );
    }
}
