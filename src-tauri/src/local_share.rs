use crate::{
    database::Database,
    errors::AppError,
    models::{LibraryMediaKind, LocalShareSession, StartLocalShareRequest},
};
use chrono::{Duration as ChronoDuration, Utc};
use futures_util::TryStreamExt;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Empty, Full, StreamBody};
use hyper::{
    body::{Bytes, Frame, Incoming},
    header::{ACCEPT_RANGES, CACHE_CONTROL, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE},
    server::conn::http1,
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use std::{
    collections::HashSet,
    convert::Infallible,
    error::Error,
    io::SeekFrom,
    net::{IpAddr, Ipv4Addr, UdpSocket},
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::async_runtime::JoinHandle;
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncSeekExt},
    net::TcpListener,
    sync::Mutex,
    time::{sleep_until, Instant},
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

type BoxError = Box<dyn Error + Send + Sync>;
type ShareBody = UnsyncBoxBody<Bytes, BoxError>;

struct ActiveShare {
    session: LocalShareSession,
    cancel: CancellationToken,
    _task: JoinHandle<()>,
}

#[derive(Clone)]
pub struct LocalShareManager {
    database: Database,
    active: Arc<Mutex<Option<ActiveShare>>>,
}

impl LocalShareManager {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            active: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn status(&self) -> Option<LocalShareSession> {
        self.active
            .lock()
            .await
            .as_ref()
            .map(|active| active.session.clone())
    }

    pub async fn start(
        &self,
        request: StartLocalShareRequest,
    ) -> Result<LocalShareSession, AppError> {
        if !(1..=50).contains(&request.item_ids.len())
            || !(5..=120).contains(&request.expires_minutes)
        {
            return Err(AppError::new(
                "invalid_local_share",
                "Share 1 to 50 items for between 5 and 120 minutes.",
            ));
        }
        let ids = request.item_ids.into_iter().collect::<HashSet<_>>();
        if ids.len() > 50 || ids.is_empty() {
            return Err(AppError::new(
                "invalid_local_share",
                "Shared Library item IDs must be unique.",
            ));
        }
        for id in &ids {
            let item = self.database.get_library_item(id)?;
            let path = PathBuf::from(&item.path);
            if item.missing || content_type(&path).is_none() || !regular_file(&path) {
                return Err(AppError::new(
                    "invalid_local_share_item",
                    "Only available supported Library media can be shared.",
                ));
            }
        }
        let mut active = self.active.lock().await;
        if active.is_some() {
            return Err(AppError::new(
                "local_share_running",
                "Stop the current sharing session before starting another.",
            ));
        }
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0))
            .await
            .map_err(|error| AppError::internal("could not start local sharing", error))?;
        let port = listener
            .local_addr()
            .map_err(|error| AppError::internal("could not inspect local sharing address", error))?
            .port();
        let lan_ip = local_lan_ip().unwrap_or(Ipv4Addr::LOCALHOST);
        let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let id = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + ChronoDuration::minutes(i64::from(request.expires_minutes));
        let session = LocalShareSession {
            id: id.clone(),
            url: format!("http://{lan_ip}:{port}/{token}/"),
            item_count: ids.len(),
            expires_at,
            reachable_on_lan: !lan_ip.is_loopback(),
        };
        let cancel = CancellationToken::new();
        let runtime = Arc::new(ShareRuntime {
            database: self.database.clone(),
            token,
            ids,
            cancel: cancel.clone(),
            expires_at,
        });
        let manager = self.clone();
        let task_id = id.clone();
        let task = tauri::async_runtime::spawn(async move {
            let deadline = Instant::now()
                + std::time::Duration::from_secs(u64::from(request.expires_minutes) * 60);
            loop {
                tokio::select! {
                    _ = runtime.cancel.cancelled() => break,
                    _ = sleep_until(deadline) => break,
                    accepted = listener.accept() => {
                        let Ok((stream, _)) = accepted else { break; };
                        let state = runtime.clone();
                        tauri::async_runtime::spawn(async move {
                            let service = service_fn(move |request| { let state = state.clone(); async move { Ok::<_, Infallible>(handle(request, state).await) } });
                            let _ = http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
                        });
                    }
                }
            }
            let mut current = manager.active.lock().await;
            if current
                .as_ref()
                .is_some_and(|share| share.session.id == task_id)
            {
                current.take();
            }
        });
        *active = Some(ActiveShare {
            session: session.clone(),
            cancel,
            _task: task,
        });
        Ok(session)
    }

    pub async fn stop(&self, id: &str) -> Result<(), AppError> {
        let mut active = self.active.lock().await;
        let Some(share) = active.as_ref() else {
            return Err(AppError::new(
                "local_share_not_running",
                "No local sharing session is active.",
            ));
        };
        if share.session.id != id {
            return Err(AppError::new(
                "local_share_mismatch",
                "That sharing session is no longer active.",
            ));
        }
        share.cancel.cancel();
        active.take();
        Ok(())
    }
}

struct ShareRuntime {
    database: Database,
    token: String,
    ids: HashSet<String>,
    cancel: CancellationToken,
    expires_at: chrono::DateTime<Utc>,
}

async fn handle(request: Request<Incoming>, state: Arc<ShareRuntime>) -> Response<ShareBody> {
    if state.cancel.is_cancelled()
        || Utc::now() >= state.expires_at
        || !matches!(*request.method(), Method::GET | Method::HEAD)
    {
        return simple(StatusCode::NOT_FOUND, "Not found");
    }
    let prefix = format!("/{}/", state.token);
    let path = request.uri().path();
    if path == prefix || path == prefix.trim_end_matches('/') {
        return listing(&state);
    }
    let Some(id) = path.strip_prefix(&format!("{prefix}media/")) else {
        return simple(StatusCode::NOT_FOUND, "Not found");
    };
    if id.contains('/') || !state.ids.contains(id) {
        return simple(StatusCode::NOT_FOUND, "Not found");
    }
    let Ok(item) = state.database.get_library_item(id) else {
        return simple(StatusCode::NOT_FOUND, "Not found");
    };
    let media = PathBuf::from(item.path);
    let Some(kind) = content_type(&media) else {
        return simple(StatusCode::UNSUPPORTED_MEDIA_TYPE, "Unsupported media");
    };
    if item.missing || !regular_file(&media) {
        return simple(StatusCode::NOT_FOUND, "Not found");
    }
    serve_file(&request, &media, kind)
        .await
        .unwrap_or_else(|_| simple(StatusCode::INTERNAL_SERVER_ERROR, "Could not stream media"))
}

fn listing(state: &ShareRuntime) -> Response<ShareBody> {
    let mut cards = String::new();
    let mut ids = state.ids.iter().collect::<Vec<_>>();
    ids.sort();
    for id in ids {
        if let Ok(item) = state.database.get_library_item(id) {
            let title = escape_html(&item.title);
            let url = format!("media/{id}");
            let element = match item.media_kind {
                Some(LibraryMediaKind::Image) => {
                    format!("<img loading=\"lazy\" src=\"{url}\" alt=\"{title}\">")
                }
                _ => format!("<video controls preload=\"metadata\" src=\"{url}\"></video>"),
            };
            cards.push_str(&format!(
                "<article>{element}<h2>{title}</h2><a href=\"{url}\">Open media</a></article>"
            ));
        }
    }
    let html = format!("<!doctype html><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\"><title>Shared from Nice Downloader</title><style>body{{margin:0;background:#0b1118;color:#edf5ff;font:16px system-ui;padding:32px}}main{{max-width:1100px;margin:auto}}p{{color:#9baec2}}section{{display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:18px}}article{{background:#121c27;border:1px solid #26384a;padding:14px;border-radius:12px}}video,img{{width:100%;aspect-ratio:16/9;object-fit:contain;background:#05080c}}h2{{font-size:15px}}a{{color:#72d6c9}}</style><main><h1>Shared media</h1><p>Temporary read-only session. Recipients can save bytes their browser receives.</p><section>{cards}</section></main>");
    Response::builder().status(StatusCode::OK).header(CONTENT_TYPE, "text/html; charset=utf-8").header(CACHE_CONTROL, "no-store").header("Content-Security-Policy", "default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'").header("Referrer-Policy", "no-referrer").header("X-Content-Type-Options", "nosniff").body(full_body(html)).unwrap_or_else(|_| simple(StatusCode::INTERNAL_SERVER_ERROR, "Could not build page"))
}

async fn serve_file(
    request: &Request<Incoming>,
    path: &Path,
    kind: &'static str,
) -> Result<Response<ShareBody>, BoxError> {
    let mut file = File::open(path).await?;
    let length = file.metadata().await?.len();
    if length == 0 {
        return Ok(simple(StatusCode::NOT_FOUND, "Not found"));
    }
    let requested = request
        .headers()
        .get(RANGE)
        .and_then(|value| value.to_str().ok());
    let (start, end, status) = match requested {
        Some(value) => match parse_range(value, length) {
            Some(range) => (range.0, range.1, StatusCode::PARTIAL_CONTENT),
            None => {
                return Ok(Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(CONTENT_RANGE, format!("bytes */{length}"))
                    .body(empty_body())?)
            }
        },
        None => (0, length - 1, StatusCode::OK),
    };
    let response_length = end - start + 1;
    let mut builder = Response::builder()
        .status(status)
        .header(CONTENT_TYPE, kind)
        .header(CONTENT_LENGTH, response_length)
        .header(ACCEPT_RANGES, "bytes")
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

fn parse_range(value: &str, length: u64) -> Option<(u64, u64)> {
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let count = end.parse::<u64>().ok()?.min(length);
        return Some((length - count, length - 1));
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
fn regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
}
fn content_type(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp4" | "m4v" => Some("video/mp4"),
        "webm" => Some("video/webm"),
        "mkv" => Some("video/x-matroska"),
        "mov" => Some("video/quicktime"),
        "avi" => Some("video/x-msvideo"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}
fn local_lan_ip() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 80)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) => Some(ip),
        IpAddr::V6(_) => None,
    }
}
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn simple(status: StatusCode, text: &str) -> Response<ShareBody> {
    Response::builder()
        .status(status)
        .header(CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(full_body(text.to_owned()))
        .unwrap_or_else(|_| Response::new(empty_body()))
}
fn full_body(value: String) -> ShareBody {
    Full::new(Bytes::from(value))
        .map_err(|never| -> BoxError { match never {} })
        .boxed_unsync()
}
fn empty_body() -> ShareBody {
    Empty::<Bytes>::new()
        .map_err(|never| -> BoxError { match never {} })
        .boxed_unsync()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LibraryItem, LibraryMetadataState};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn raw_request(port: u16, request: &str) -> Vec<u8> {
        let mut stream = tokio::net::TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .expect("connect to local share");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write request");
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .expect("read response");
        response
    }

    #[test]
    fn parses_only_single_bounded_byte_ranges() {
        assert_eq!(parse_range("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(parse_range("bytes=7-", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(parse_range("bytes=10-", 10), None);
        assert_eq!(parse_range("bytes=1-2,4-5", 10), None);
    }

    #[test]
    fn escapes_untrusted_titles_in_share_pages() {
        assert_eq!(
            escape_html("<script>&\"'"),
            "&lt;script&gt;&amp;&quot;&#39;"
        );
    }

    #[tokio::test]
    async fn serves_only_capability_selected_media_with_range_headers() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("shared.mp4");
        std::fs::write(&source, b"0123456789").expect("media fixture");
        let database = Database::open(&directory.path().join("share.sqlite3")).expect("database");
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: source.to_string_lossy().into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "<Shared & safe>".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(1.0),
            site: None,
            tags: vec![],
            metadata: serde_json::json!({}),
            file_size_bytes: Some(10),
            modified_at_ms: None,
            width: Some(16),
            height: Some(9),
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
        let manager = LocalShareManager::new(database);
        let session = manager
            .start(StartLocalShareRequest {
                item_ids: vec![item.id.clone()],
                expires_minutes: 5,
            })
            .await
            .expect("start local share");
        let authority_and_path = session.url.strip_prefix("http://").expect("http share URL");
        let (authority, capability) = authority_and_path.split_once('/').expect("capability path");
        let port = authority
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .expect("share port");

        let listing = raw_request(
            port,
            &format!("GET /{capability} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
        )
        .await;
        let listing = String::from_utf8_lossy(&listing);
        assert!(listing.starts_with("HTTP/1.1 200"));
        assert!(listing.contains("content-security-policy:"));
        assert!(listing.contains("&lt;Shared &amp; safe&gt;"));
        assert!(!listing.contains(&item.path));

        let media = raw_request(
            port,
            &format!(
                "GET /{capability}media/{} HTTP/1.1\r\nHost: localhost\r\nRange: bytes=2-5\r\nConnection: close\r\n\r\n",
                item.id
            ),
        )
        .await;
        let separator = media
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("response headers");
        let headers = String::from_utf8_lossy(&media[..separator]);
        assert!(headers.starts_with("HTTP/1.1 206"));
        assert!(headers.contains("content-range: bytes 2-5/10"));
        assert_eq!(&media[(separator + 4)..], b"2345");

        let unauthorized = raw_request(
            port,
            &format!(
                "GET /{capability}media/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                Uuid::new_v4()
            ),
        )
        .await;
        assert!(String::from_utf8_lossy(&unauthorized).starts_with("HTTP/1.1 404"));
        manager.stop(&session.id).await.expect("stop local share");
    }
}
