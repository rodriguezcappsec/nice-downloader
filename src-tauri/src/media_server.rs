use crate::{
    database::Database,
    engines::managed,
    errors::AppError,
    models::{
        LibraryCacheResult, LibraryImageSession, LibraryItem, LibraryMediaKind,
        LibraryPlaybackCacheStatus, LibraryPlaybackPreflight, LibraryPlaybackPreparationPhase,
        LibraryPlaybackPreparationProgress, LibraryPlaybackSession, LibrarySubtitleTrack,
        PlaybackCompatibilityPreference,
    },
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use futures_util::{Stream, TryStreamExt};
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Empty, Full, StreamBody};
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
use serde::Deserialize;
use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    ffi::OsStr,
    fs::OpenOptions,
    io::SeekFrom,
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdTcpListener},
    path::{Path, PathBuf},
    pin::Pin,
    process::Stdio,
    sync::{Arc, RwLock},
    task::{Context, Poll},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader},
    net::TcpListener,
    process::Command,
    sync::{Mutex as AsyncMutex, OnceCell, Semaphore, SemaphorePermit},
    time::{timeout, Duration},
};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

type BoxError = Box<dyn Error + Send + Sync>;
type MediaBody = UnsyncBoxBody<Bytes, BoxError>;
const MAX_COMPATIBILITY_CACHE_FILES: usize = 4;
const MAX_COMPATIBILITY_CACHE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const COMPATIBILITY_CACHE_VERSION: u8 = 2;
const TRANSCODE_CACHE_VERSION: u8 = 1;
const LARGE_COMPATIBILITY_COPY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_LIVE_TRANSCODES: usize = 2;
const LIVE_TRANSCODE_AUTHORIZATION_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_HLS_MANIFEST_BYTES: u64 = 1024 * 1024;
const HLS_START_TIMEOUT: Duration = Duration::from_secs(10);

struct CompatibilityTarget {
    source: PathBuf,
    output: PathBuf,
    estimated_bytes: u64,
}

#[derive(Clone)]
struct ActivePlaybackPreparation {
    progress: LibraryPlaybackPreparationProgress,
    cancellation: CancellationToken,
    reserved_bytes: u64,
}

#[derive(Clone)]
struct LiveTranscodeAuthorization {
    item_id: String,
    start_seconds: f64,
    cancellation: CancellationToken,
    started: bool,
    created_at: Instant,
}

#[derive(Clone)]
struct HlsTranscodeAuthorization {
    item_id: String,
    start_seconds: f64,
    directory: PathBuf,
    cancellation: CancellationToken,
    started: bool,
    created_at: Instant,
    last_access: Instant,
}

struct LiveTranscodeBodyStream {
    inner: Pin<Box<dyn Stream<Item = Result<Frame<Bytes>, BoxError>> + Send>>,
    cancellation: CancellationToken,
}

impl Stream for LiveTranscodeBodyStream {
    type Item = Result<Frame<Bytes>, BoxError>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(context)
    }
}

impl Drop for LiveTranscodeBodyStream {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct PlaybackPreparationGuard {
    item_id: String,
    preparations: Arc<RwLock<HashMap<String, ActivePlaybackPreparation>>>,
}

impl Drop for PlaybackPreparationGuard {
    fn drop(&mut self) {
        self.preparations
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&self.item_id);
    }
}

#[derive(Clone)]
struct PlaybackProgressReporter {
    item_id: String,
    preparations: Arc<RwLock<HashMap<String, ActivePlaybackPreparation>>>,
}

impl PlaybackProgressReporter {
    fn update(&self, phase: LibraryPlaybackPreparationPhase, processed_seconds: Option<f64>) {
        let mut preparations = self
            .preparations
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(active) = preparations.get_mut(&self.item_id) else {
            return;
        };
        active.progress.phase = phase;
        if let Some(processed_seconds) = processed_seconds.filter(|value| value.is_finite()) {
            let processed_seconds = processed_seconds.max(0.0);
            active.progress.processed_seconds = Some(processed_seconds);
            active.progress.progress_percent = active
                .progress
                .duration_seconds
                .filter(|duration| *duration > 0.0)
                .map(|duration| (processed_seconds / duration * 100.0).clamp(0.0, 99.5));
        }
    }

    fn update_progress(&self, processed_seconds: f64) {
        let mut preparations = self
            .preparations
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(active) = preparations.get_mut(&self.item_id) else {
            return;
        };
        let processed_seconds = processed_seconds.max(0.0);
        active.progress.processed_seconds = Some(processed_seconds);
        active.progress.progress_percent = active
            .progress
            .duration_seconds
            .filter(|duration| *duration > 0.0)
            .map(|duration| (processed_seconds / duration * 100.0).clamp(0.0, 99.5));
    }
}

#[derive(Clone)]
pub struct MediaServerState {
    database: Database,
    base_url: Option<String>,
    token: String,
    cache_directory: PathBuf,
    playback_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    artwork_paths: Arc<RwLock<HashMap<String, PathBuf>>>,
    compatibility_lock: Arc<AsyncMutex<()>>,
    playback_preparations: Arc<RwLock<HashMap<String, ActivePlaybackPreparation>>>,
    preparation_slots: Arc<Semaphore>,
    live_transcodes: Arc<RwLock<HashMap<String, LiveTranscodeAuthorization>>>,
    hls_transcodes: Arc<RwLock<HashMap<String, HlsTranscodeAuthorization>>>,
    live_transcode_slots: Arc<Semaphore>,
    webview_dependency_error: Arc<OnceCell<Option<String>>>,
}

impl MediaServerState {
    pub fn unavailable(database: Database) -> Self {
        Self {
            database,
            base_url: None,
            token: Uuid::new_v4().to_string(),
            cache_directory: PathBuf::new(),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new()),
        }
    }

    pub fn register_artwork(&self, item_id: &str, path: PathBuf) -> Result<String, AppError> {
        validate_item_id(item_id)?;
        validate_regular_media_file(&path)?;
        if path.extension().and_then(OsStr::to_str) != Some("jpg") {
            return Err(AppError::new(
                "library_artwork_invalid",
                "The generated Library artwork has an unexpected format.",
            ));
        }
        let base_url = self.base_url.as_deref().ok_or_else(|| {
            AppError::new(
                "embedded_player_unavailable",
                "The local media service is unavailable.",
            )
        })?;
        let version = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| {
                AppError::new(
                    "library_artwork_invalid",
                    "The generated Library artwork has an invalid cache name.",
                )
            })?
            .to_owned();
        self.artwork_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(item_id.to_owned(), path);
        Ok(format!(
            "{base_url}/artwork/{item_id}?token={}&v={version}",
            self.token
        ))
    }

    async fn ensure_webview_dependencies(&self) -> Result<(), AppError> {
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
        Ok(())
    }

    fn begin_playback_preparation(
        &self,
        item: &LibraryItem,
        reencode_required: bool,
        reserved_bytes: u64,
    ) -> Result<
        (
            PlaybackPreparationGuard,
            PlaybackProgressReporter,
            CancellationToken,
        ),
        AppError,
    > {
        let mut preparations = self
            .playback_preparations
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if preparations.contains_key(&item.id) {
            return Err(AppError::new(
                "embedded_playback_preparation_busy",
                "A playback copy is already being prepared for this video.",
            ));
        }
        let cancellation = CancellationToken::new();
        preparations.insert(
            item.id.clone(),
            ActivePlaybackPreparation {
                progress: LibraryPlaybackPreparationProgress {
                    item_id: item.id.clone(),
                    phase: LibraryPlaybackPreparationPhase::Queued,
                    progress_percent: Some(0.0),
                    processed_seconds: Some(0.0),
                    duration_seconds: item.duration_seconds.filter(|value| *value > 0.0),
                    reencode_required,
                },
                cancellation: cancellation.clone(),
                reserved_bytes,
            },
        );
        let reporter = PlaybackProgressReporter {
            item_id: item.id.clone(),
            preparations: self.playback_preparations.clone(),
        };
        let guard = PlaybackPreparationGuard {
            item_id: item.id.clone(),
            preparations: self.playback_preparations.clone(),
        };
        Ok((guard, reporter, cancellation))
    }

    fn playback_preparation_reservations(&self) -> (usize, u64) {
        let preparations = self
            .playback_preparations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        (
            preparations.len(),
            preparations.values().fold(0u64, |total, active| {
                total.saturating_add(active.reserved_bytes)
            }),
        )
    }

    async fn wait_for_preparation_slot<'a>(
        &'a self,
        cancellation: &CancellationToken,
    ) -> Result<SemaphorePermit<'a>, AppError> {
        tokio::select! {
            _ = cancellation.cancelled() => Err(playback_preparation_cancelled()),
            permit = self.preparation_slots.acquire() => permit.map_err(|_| {
                AppError::new(
                    "embedded_playback_preparation_unavailable",
                    "Playback-copy preparation is shutting down. Try again after restarting Nice Downloader.",
                )
            }),
        }
    }

    pub fn playback_preparation(
        &self,
        item_id: &str,
    ) -> Result<Option<LibraryPlaybackPreparationProgress>, AppError> {
        validate_item_id(item_id)?;
        Ok(self
            .playback_preparations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(item_id)
            .map(|active| active.progress.clone()))
    }

    pub fn cancel_playback_preparation(&self, item_id: &str) -> Result<(), AppError> {
        validate_item_id(item_id)?;
        let cancellation = self
            .playback_preparations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(item_id)
            .map(|active| active.cancellation.clone())
            .ok_or_else(|| {
                AppError::new(
                    "embedded_playback_preparation_not_running",
                    "No playback copy is currently being prepared for this video.",
                )
            })?;
        cancellation.cancel();
        Ok(())
    }

    fn cancel_playback_preparation_if_active(&self, item_id: &str) {
        if let Some(cancellation) = self
            .playback_preparations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(item_id)
            .map(|active| active.cancellation.clone())
        {
            cancellation.cancel();
        }
    }

    pub async fn prepare_webview_playback(
        &self,
        item_id: &str,
        user_approved: bool,
    ) -> Result<LibraryPlaybackSession, AppError> {
        validate_item_id(item_id)?;
        self.ensure_webview_dependencies().await?;
        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let compatibility_path = self
            .prepare_compatibility_playback(&item, user_approved)
            .await?;
        self.set_playback_path(item_id, compatibility_path);
        self.prepare_playback(item_id)
    }

    pub async fn inspect_webview_playback(
        &self,
        item_id: &str,
    ) -> Result<LibraryPlaybackPreflight, AppError> {
        validate_item_id(item_id)?;
        self.ensure_webview_dependencies().await?;
        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let preference = self
            .database
            .load_desktop_preferences()?
            .playback_compatibility;
        let Some(target) = self.compatibility_target(&item).await? else {
            return Ok(LibraryPlaybackPreflight {
                compatibility_required: false,
                compatibility_cached: false,
                unusually_large: false,
                reencode_required: false,
                estimated_copy_bytes: 0,
                available_space_bytes: None,
                reason: None,
                preference,
                requires_confirmation: false,
            });
        };
        let cached = valid_cached_media(&target.output);
        let available_path = if self.cache_directory.is_dir() {
            self.cache_directory.as_path()
        } else {
            self.cache_directory.parent().unwrap_or(Path::new("."))
        };
        let unusually_large = target.estimated_bytes >= LARGE_COMPATIBILITY_COPY_BYTES;
        let requires_confirmation =
            compatibility_requires_confirmation(preference, target.estimated_bytes, cached);
        Ok(LibraryPlaybackPreflight {
            compatibility_required: true,
            compatibility_cached: cached,
            unusually_large,
            reencode_required: false,
            estimated_copy_bytes: target.estimated_bytes,
            available_space_bytes: fs2::available_space(available_path).ok(),
            reason: Some(
                "The Linux embedded player cannot reliably interpret this AV1 MP4 track's timing. A cached Matroska stream-copy avoids video running ahead of audio without changing quality."
                    .to_owned(),
            ),
            preference,
            requires_confirmation,
        })
    }

    pub async fn inspect_transcoded_webview_playback(
        &self,
        item_id: &str,
    ) -> Result<LibraryPlaybackPreflight, AppError> {
        validate_item_id(item_id)?;
        self.ensure_webview_dependencies().await?;
        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let target = self.transcode_target(&item)?;
        let cached = valid_cached_media(&target.output);
        let available_path = if self.cache_directory.is_dir() {
            self.cache_directory.as_path()
        } else {
            self.cache_directory.parent().unwrap_or(Path::new("."))
        };
        Ok(LibraryPlaybackPreflight {
            compatibility_required: true,
            compatibility_cached: cached,
            unusually_large: target.estimated_bytes >= LARGE_COMPATIBILITY_COPY_BYTES,
            reencode_required: true,
            estimated_copy_bytes: target.estimated_bytes,
            available_space_bytes: fs2::available_space(available_path).ok(),
            reason: Some(
                "The WebView rejected the lossless timing copy. A separate H.264/AAC MP4 copy provides the broadest embedded-player compatibility, but it must re-encode the video and may take several minutes."
                    .to_owned(),
            ),
            preference: self
                .database
                .load_desktop_preferences()?
                .playback_compatibility,
            requires_confirmation: !cached,
        })
    }

    pub async fn prepare_transcoded_webview_playback(
        &self,
        item_id: &str,
        user_approved: bool,
    ) -> Result<LibraryPlaybackSession, AppError> {
        validate_item_id(item_id)?;
        self.ensure_webview_dependencies().await?;
        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let compatibility_path = self
            .prepare_transcoded_playback(&item, user_approved)
            .await?;
        self.set_playback_path(item_id, Some(compatibility_path));
        self.prepare_playback(item_id)
    }

    pub async fn compatibility_cache_status(&self) -> Result<LibraryPlaybackCacheStatus, AppError> {
        let _guard = self.compatibility_lock.lock().await;
        playback_cache_status(&self.cache_directory)
    }

    pub async fn clear_compatibility_cache(&self) -> Result<LibraryCacheResult, AppError> {
        let _guard = self.compatibility_lock.lock().await;
        if !self
            .playback_preparations
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
        {
            return Err(AppError::new(
                "playback_cache_in_use",
                "Cancel active playback-copy preparation before clearing the compatibility cache.",
            ));
        }
        if !self
            .playback_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
        {
            return Err(AppError::new(
                "playback_cache_in_use",
                "Close embedded videos before clearing the compatibility cache.",
            ));
        }
        if !self
            .hls_transcodes
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty()
        {
            return Err(AppError::new(
                "playback_cache_in_use",
                "Close segmented playback sessions before clearing the compatibility cache.",
            ));
        }
        let result = clear_playback_cache(&self.cache_directory)?;
        self.playback_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        Ok(result)
    }

    pub fn release_playback(&self, item_id: &str) -> Result<(), AppError> {
        validate_item_id(item_id)?;
        self.cancel_playback_preparation_if_active(item_id);
        self.cancel_live_transcodes_for_item(item_id);
        self.cancel_hls_transcodes_for_item(item_id);
        self.set_playback_path(item_id, None);
        Ok(())
    }

    pub fn prepare_live_transcode_playback(
        &self,
        item_id: &str,
        start_seconds: f64,
    ) -> Result<LibraryPlaybackSession, AppError> {
        let base_url = self.base_url.as_deref().ok_or_else(|| {
            AppError::new(
                "embedded_playback_unavailable",
                "Embedded playback is unavailable on this system. Open the video in your system player instead.",
            )
        })?;
        validate_item_id(item_id)?;
        if managed::resolve("ffmpeg").is_none() {
            return Err(AppError::new(
                "live_transcode_unavailable",
                "Live compatibility playback needs FFmpeg. Install FFmpeg in Settings > Engines or use a native player.",
            ));
        }

        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let playback = self.database.get_library_playback(item_id)?;
        let duration = item
            .duration_seconds
            .filter(|value| value.is_finite() && *value > 0.0);
        let maximum_start = duration
            .map(|value| (value - 0.25).max(0.0))
            .unwrap_or(f64::MAX);
        let start_seconds = if start_seconds.is_finite() {
            start_seconds.clamp(0.0, maximum_start)
        } else {
            return Err(AppError::new(
                "invalid_playback_timestamp",
                "The requested playback timestamp is invalid.",
            ));
        };

        self.cancel_live_transcodes_for_item(item_id);
        self.cancel_hls_transcodes_for_item(item_id);
        self.set_playback_path(item_id, None);
        let stream_id = Uuid::new_v4().to_string();
        self.live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                stream_id.clone(),
                LiveTranscodeAuthorization {
                    item_id: item_id.to_owned(),
                    start_seconds,
                    cancellation: CancellationToken::new(),
                    started: false,
                    created_at: Instant::now(),
                },
            );

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
                    self.token,
                ),
            })
            .collect();

        Ok(LibraryPlaybackSession {
            playback,
            url: format!(
                "{}/transcode/{}/{}.mp4?token={}",
                base_url, item.id, stream_id, self.token,
            ),
            item,
            subtitles,
            compatibility_mode: false,
            compatibility_transcoded: false,
            live_transcode: true,
            segmented_stream: false,
            stream_start_seconds: start_seconds,
        })
    }

    pub fn prepare_hls_playback(
        &self,
        item_id: &str,
        start_seconds: f64,
    ) -> Result<LibraryPlaybackSession, AppError> {
        let base_url = self.base_url.as_deref().ok_or_else(|| {
            AppError::new(
                "embedded_playback_unavailable",
                "Embedded playback is unavailable on this system. Open the video in your system player instead.",
            )
        })?;
        validate_item_id(item_id)?;
        if managed::resolve("ffmpeg").is_none() {
            return Err(AppError::new(
                "live_transcode_unavailable",
                "Segmented compatibility playback needs FFmpeg. Install FFmpeg in Settings > Engines or use a native player.",
            ));
        }

        let item = self.database.get_library_item(item_id)?;
        validate_playable_item(&item)?;
        let playback = self.database.get_library_playback(item_id)?;
        let duration = item
            .duration_seconds
            .filter(|value| value.is_finite() && *value > 0.0);
        let maximum_start = duration
            .map(|value| (value - 0.25).max(0.0))
            .unwrap_or(f64::MAX);
        let start_seconds = if start_seconds.is_finite() {
            start_seconds.clamp(0.0, maximum_start)
        } else {
            return Err(AppError::new(
                "invalid_playback_timestamp",
                "The requested playback timestamp is invalid.",
            ));
        };

        self.cancel_live_transcodes_for_item(item_id);
        self.cancel_hls_transcodes_for_item(item_id);
        self.set_playback_path(item_id, None);
        let stream_id = Uuid::new_v4().to_string();
        let directory = create_hls_session_directory(&self.cache_directory, &stream_id)?;
        let now = Instant::now();
        self.hls_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                stream_id.clone(),
                HlsTranscodeAuthorization {
                    item_id: item_id.to_owned(),
                    start_seconds,
                    directory,
                    cancellation: CancellationToken::new(),
                    started: false,
                    created_at: now,
                    last_access: now,
                },
            );

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
                    self.token,
                ),
            })
            .collect();

        Ok(LibraryPlaybackSession {
            playback,
            url: format!(
                "{}/hls/{}/{}/index.m3u8?token={}",
                base_url, item.id, stream_id, self.token,
            ),
            item,
            subtitles,
            compatibility_mode: false,
            compatibility_transcoded: false,
            live_transcode: true,
            segmented_stream: true,
            stream_start_seconds: start_seconds,
        })
    }

    fn cancel_live_transcodes_for_item(&self, item_id: &str) {
        let mut streams = self
            .live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let matching = streams
            .iter()
            .filter(|(_, stream)| stream.item_id == item_id)
            .map(|(stream_id, _)| stream_id.clone())
            .collect::<Vec<_>>();
        for stream_id in matching {
            if let Some(stream) = streams.remove(&stream_id) {
                stream.cancellation.cancel();
            }
        }
    }

    fn cancel_hls_transcodes_for_item(&self, item_id: &str) {
        let mut streams = self
            .hls_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let matching = streams
            .iter()
            .filter(|(_, stream)| stream.item_id == item_id)
            .map(|(stream_id, _)| stream_id.clone())
            .collect::<Vec<_>>();
        for stream_id in matching {
            if let Some(stream) = streams.remove(&stream_id) {
                stream.cancellation.cancel();
                let _ = std::fs::remove_dir_all(stream.directory);
            }
        }
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
        validate_playable_item(&item)?;
        let registered_path = self.registered_playback_path(item_id);
        let compatibility_mode = registered_path.is_some();
        let compatibility_transcoded = registered_path
            .as_deref()
            .is_some_and(is_transcoded_compatibility_path);
        let playback_session_id = Uuid::new_v4();
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
            url: format!(
                "{}/media/{}?token={}&session={}",
                base_url, item.id, self.token, playback_session_id
            ),
            item,
            subtitles,
            compatibility_mode,
            compatibility_transcoded,
            live_transcode: false,
            segmented_stream: false,
            stream_start_seconds: 0.0,
        })
    }

    pub fn prepare_image_view(&self, item_id: &str) -> Result<LibraryImageSession, AppError> {
        let base_url = self.base_url.as_deref().ok_or_else(|| {
            AppError::new(
                "embedded_image_unavailable",
                "The embedded image viewer is unavailable. Open the image in your system viewer instead.",
            )
        })?;
        validate_item_id(item_id)?;
        let item = self.database.get_library_item(item_id)?;
        validate_viewable_image(&item)?;
        Ok(LibraryImageSession {
            item_id: item.id.clone(),
            url: format!(
                "{}/image/{}?token={}&session={}",
                base_url,
                item.id,
                self.token,
                Uuid::new_v4()
            ),
        })
    }

    fn set_playback_path(&self, item_id: &str, path: Option<PathBuf>) {
        let mut paths = self
            .playback_paths
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(path) = path {
            paths.insert(item_id.to_owned(), path);
        } else {
            paths.remove(item_id);
        }
    }

    fn registered_playback_path(&self, item_id: &str) -> Option<PathBuf> {
        self.playback_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(item_id)
            .cloned()
    }

    async fn prepare_compatibility_playback(
        &self,
        item: &LibraryItem,
        user_approved: bool,
    ) -> Result<Option<PathBuf>, AppError> {
        let Some(target) = self.compatibility_target(item).await? else {
            return Ok(None);
        };
        let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
            AppError::new(
                "embedded_playback_compatibility_unavailable",
                "This AV1 video needs FFmpeg preparation for reliable embedded playback. Install FFmpeg in Settings > Engines or open it with the system player.",
            )
        })?;
        let (temporary, timeout_seconds, _preparation_guard, reporter, cancellation) = {
            let _cache_guard = self.compatibility_lock.lock().await;
            std::fs::create_dir_all(&self.cache_directory).map_err(|error| {
                AppError::internal("could not create the embedded playback cache", error)
            })?;
            if valid_cached_media(&target.output) {
                touch_cache_entry(&target.output);
                return Ok(Some(target.output));
            }
            let preference = self
                .database
                .load_desktop_preferences()?
                .playback_compatibility;
            if !user_approved
                && compatibility_requires_confirmation(preference, target.estimated_bytes, false)
            {
                return Err(AppError::new(
                    "compatibility_copy_consent_required",
                    "Confirm the compatibility copy before preparing this video for embedded playback.",
                ));
            }
            remove_stale_compatibility_files(&self.cache_directory, &item.id, &target.output);
            enforce_compatibility_cache_limits(
                &self.cache_directory,
                None,
                1,
                target.estimated_bytes,
            )?;
            let required_space = target.estimated_bytes.saturating_add(64 * 1024 * 1024);
            if fs2::available_space(&self.cache_directory).unwrap_or(u64::MAX) < required_space {
                return Err(AppError::new(
                    "embedded_playback_cache_full",
                    "There is not enough free space to prepare this AV1 video for embedded playback. Open it with the system player or free space in the application cache.",
                ));
            }
            let temporary =
                self.cache_directory
                    .join(format!(".{}-{}.part.mkv", item.id, Uuid::new_v4()));
            let timeout_seconds = (target.estimated_bytes / (5 * 1024 * 1024)).clamp(60, 600);
            let (guard, reporter, cancellation) =
                self.begin_playback_preparation(item, false, target.estimated_bytes)?;
            let (reserved_files, reserved_bytes) = self.playback_preparation_reservations();
            enforce_compatibility_cache_limits(
                &self.cache_directory,
                None,
                reserved_files,
                reserved_bytes,
            )?;
            if fs2::available_space(&self.cache_directory).unwrap_or(u64::MAX)
                < reserved_bytes.saturating_add(64 * 1024 * 1024)
            {
                return Err(AppError::new(
                    "embedded_playback_cache_full",
                    "There is not enough free space for the active playback-copy preparations.",
                ));
            }
            (temporary, timeout_seconds, guard, reporter, cancellation)
        };
        let _slot = self.wait_for_preparation_slot(&cancellation).await?;
        reporter.update(LibraryPlaybackPreparationPhase::Copying, Some(0.0));
        let result = remux_for_webview(
            &ffmpeg,
            &target.source,
            &temporary,
            timeout_seconds,
            &cancellation,
            reporter.clone(),
        )
        .await;
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        if cancellation.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err(playback_preparation_cancelled());
        }
        reporter.update(LibraryPlaybackPreparationPhase::Finalizing, None);
        let _cache_guard = self.compatibility_lock.lock().await;
        if cancellation.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err(playback_preparation_cancelled());
        }
        if !valid_cached_media(&temporary) {
            let _ = std::fs::remove_file(&temporary);
            return Err(AppError::new(
                "embedded_playback_preparation_failed",
                "FFmpeg did not produce a usable embedded-playback copy. The original video is unchanged and can still be opened with the system player.",
            ));
        }
        std::fs::rename(&temporary, &target.output).map_err(|error| {
            let _ = std::fs::remove_file(&temporary);
            AppError::internal("could not finalize the embedded playback cache", error)
        })?;
        touch_cache_entry(&target.output);
        enforce_compatibility_cache_limits(&self.cache_directory, Some(&target.output), 0, 0)?;
        tracing::info!(item_id = %item.id, "prepared AV1 timing compatibility copy for embedded playback");
        Ok(Some(target.output))
    }

    async fn prepare_transcoded_playback(
        &self,
        item: &LibraryItem,
        user_approved: bool,
    ) -> Result<PathBuf, AppError> {
        let target = self.transcode_target(item)?;
        let ffmpeg = managed::resolve("ffmpeg").ok_or_else(|| {
            AppError::new(
                "embedded_playback_compatibility_unavailable",
                "This fallback needs FFmpeg. Install FFmpeg in Settings > Engines or open the original with the system player.",
            )
        })?;
        let (temporary, timeout_seconds, _preparation_guard, reporter, cancellation) = {
            let _cache_guard = self.compatibility_lock.lock().await;
            std::fs::create_dir_all(&self.cache_directory).map_err(|error| {
                AppError::internal("could not create the embedded playback cache", error)
            })?;
            if valid_cached_media(&target.output) {
                touch_cache_entry(&target.output);
                return Ok(target.output);
            }
            if !user_approved {
                return Err(AppError::new(
                    "compatibility_transcode_consent_required",
                    "Confirm the H.264 compatibility transcode before preparing this video.",
                ));
            }
            if target.estimated_bytes > MAX_COMPATIBILITY_CACHE_BYTES {
                return Err(AppError::new(
                    "embedded_playback_cache_limit",
                    "The estimated H.264 copy is larger than the compatibility cache limit. Open the original with the system player instead.",
                ));
            }
            enforce_compatibility_cache_limits(
                &self.cache_directory,
                None,
                1,
                target.estimated_bytes,
            )?;
            let required_space = target.estimated_bytes.saturating_add(128 * 1024 * 1024);
            if fs2::available_space(&self.cache_directory).unwrap_or(u64::MAX) < required_space {
                return Err(AppError::new(
                    "embedded_playback_cache_full",
                    "There is not enough free space for the H.264 playback copy. Open the original with the system player or clear space in the compatibility cache.",
                ));
            }
            let temporary =
                self.cache_directory
                    .join(format!(".{}-{}.part.mp4", item.id, Uuid::new_v4()));
            let duration_seconds = item.duration_seconds.unwrap_or(300.0).max(1.0);
            let timeout_seconds = ((duration_seconds * 3.0) as u64).clamp(300, 7_200);
            let (guard, reporter, cancellation) =
                self.begin_playback_preparation(item, true, target.estimated_bytes)?;
            let (reserved_files, reserved_bytes) = self.playback_preparation_reservations();
            enforce_compatibility_cache_limits(
                &self.cache_directory,
                None,
                reserved_files,
                reserved_bytes,
            )?;
            if fs2::available_space(&self.cache_directory).unwrap_or(u64::MAX)
                < reserved_bytes.saturating_add(128 * 1024 * 1024)
            {
                return Err(AppError::new(
                    "embedded_playback_cache_full",
                    "There is not enough free space for the active playback-copy preparations.",
                ));
            }
            (temporary, timeout_seconds, guard, reporter, cancellation)
        };
        let _slot = self.wait_for_preparation_slot(&cancellation).await?;
        reporter.update(LibraryPlaybackPreparationPhase::Encoding, Some(0.0));
        let result = transcode_for_webview(
            &ffmpeg,
            &target.source,
            &temporary,
            timeout_seconds,
            &cancellation,
            reporter.clone(),
        )
        .await;
        if let Err(error) = result {
            let _ = std::fs::remove_file(&temporary);
            return Err(error);
        }
        if cancellation.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err(playback_preparation_cancelled());
        }
        reporter.update(LibraryPlaybackPreparationPhase::Finalizing, None);
        let _cache_guard = self.compatibility_lock.lock().await;
        if cancellation.is_cancelled() {
            let _ = std::fs::remove_file(&temporary);
            return Err(playback_preparation_cancelled());
        }
        let output_size = std::fs::metadata(&temporary)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if !valid_cached_media(&temporary) || output_size > MAX_COMPATIBILITY_CACHE_BYTES {
            let _ = std::fs::remove_file(&temporary);
            return Err(AppError::new(
                "embedded_playback_preparation_failed",
                "FFmpeg did not produce a usable H.264 playback copy within the cache limit. The original is unchanged.",
            ));
        }
        std::fs::rename(&temporary, &target.output).map_err(|error| {
            let _ = std::fs::remove_file(&temporary);
            AppError::internal("could not finalize the H.264 playback copy", error)
        })?;
        remove_stale_compatibility_files(&self.cache_directory, &item.id, &target.output);
        touch_cache_entry(&target.output);
        enforce_compatibility_cache_limits(&self.cache_directory, Some(&target.output), 0, 0)?;
        tracing::info!(item_id = %item.id, "prepared H.264 fallback copy for embedded playback");
        Ok(target.output)
    }

    async fn compatibility_target(
        &self,
        item: &LibraryItem,
    ) -> Result<Option<CompatibilityTarget>, AppError> {
        let source = PathBuf::from(&item.path);
        if !is_iso_base_media(&source) {
            return Ok(None);
        }
        let known_av1 = item
            .video_codec
            .as_deref()
            .is_some_and(|codec| codec.eq_ignore_ascii_case("av1"));
        let needs_remux = match managed::resolve("ffprobe") {
            Some(ffprobe) => inspect_av1_timing(&ffprobe, &source)
                .await
                .unwrap_or(known_av1),
            None => known_av1,
        };
        if !needs_remux {
            return Ok(None);
        }
        let metadata = std::fs::metadata(&source).map_err(|_| {
            AppError::new(
                "library_file_not_found",
                "This media file is no longer available at its saved location.",
            )
        })?;
        let fallback_output = transcode_cache_path(
            &self.cache_directory,
            &item.id,
            metadata.len(),
            modified_millis(&metadata),
        );
        let output = if valid_cached_media(&fallback_output) {
            fallback_output
        } else {
            compatibility_cache_path(
                &self.cache_directory,
                &item.id,
                metadata.len(),
                modified_millis(&metadata),
            )
        };
        Ok(Some(CompatibilityTarget {
            source,
            output,
            estimated_bytes: metadata.len(),
        }))
    }

    fn transcode_target(&self, item: &LibraryItem) -> Result<CompatibilityTarget, AppError> {
        let source = PathBuf::from(&item.path);
        let metadata = std::fs::metadata(&source).map_err(|_| {
            AppError::new(
                "library_file_not_found",
                "This media file is no longer available at its saved location.",
            )
        })?;
        Ok(CompatibilityTarget {
            source,
            output: transcode_cache_path(
                &self.cache_directory,
                &item.id,
                metadata.len(),
                modified_millis(&metadata),
            ),
            estimated_bytes: metadata.len().saturating_mul(4),
        })
    }
}

fn compatibility_requires_confirmation(
    preference: PlaybackCompatibilityPreference,
    estimated_bytes: u64,
    cached: bool,
) -> bool {
    !cached
        && (preference != PlaybackCompatibilityPreference::Automatic
            || estimated_bytes >= LARGE_COMPATIBILITY_COPY_BYTES)
}

fn validate_item_id(item_id: &str) -> Result<(), AppError> {
    Uuid::parse_str(item_id).map(|_| ()).map_err(|_| {
        AppError::new(
            "invalid_identifier",
            "The library item identifier is invalid.",
        )
    })
}

fn validate_playable_item(item: &LibraryItem) -> Result<(), AppError> {
    if item.media_kind != Some(LibraryMediaKind::Video) {
        return Err(AppError::new(
            "library_video_not_found",
            "Choose a video from the media library.",
        ));
    }
    validate_regular_media_file(Path::new(&item.path))
}

fn validate_viewable_image(item: &LibraryItem) -> Result<(), AppError> {
    if item.media_kind != Some(LibraryMediaKind::Image) {
        return Err(AppError::new(
            "library_image_not_found",
            "Choose an image from your media library.",
        ));
    }
    let path = Path::new(&item.path);
    validate_regular_media_file(path)?;
    if image_content_type(path).is_none() {
        return Err(AppError::new(
            "unsupported_library_image",
            "This image format is not supported by the embedded viewer. Open it in your system viewer instead.",
        ));
    }
    Ok(())
}

fn is_iso_base_media(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "m4v" | "mov"
            )
        })
}

#[derive(Debug, Deserialize)]
struct CompatibilityProbe {
    #[serde(default)]
    streams: Vec<CompatibilityVideoStream>,
}

#[derive(Debug, Deserialize)]
struct CompatibilityVideoStream {
    codec_name: Option<String>,
    time_base: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
}

async fn inspect_av1_timing(ffprobe: &Path, source: &Path) -> Result<bool, AppError> {
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=codec_name,time_base,avg_frame_rate,r_frame_rate")
        .arg("-of")
        .arg("json")
        .arg(source)
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(10), command.output())
        .await
        .map_err(|_| {
            AppError::new(
                "embedded_playback_probe_timeout",
                "Checking this video for embedded-playback compatibility took too long.",
            )
        })?
        .map_err(|error| {
            AppError::internal("could not start ffprobe for embedded playback", error)
        })?;
    if !output.status.success() {
        return Err(AppError::new(
            "embedded_playback_probe_failed",
            "FFprobe could not check this video's embedded-playback timing.",
        ));
    }
    let probe: CompatibilityProbe = serde_json::from_slice(&output.stdout).map_err(|error| {
        AppError::internal("ffprobe returned invalid embedded-playback metadata", error)
    })?;
    Ok(probe
        .streams
        .first()
        .is_some_and(has_problematic_av1_timing))
}

fn has_problematic_av1_timing(stream: &CompatibilityVideoStream) -> bool {
    if !stream
        .codec_name
        .as_deref()
        .is_some_and(|codec| codec.eq_ignore_ascii_case("av1"))
    {
        return false;
    }
    let Some((time_base_numerator, time_base_denominator)) =
        stream.time_base.as_deref().and_then(parse_rational)
    else {
        return false;
    };
    let frame_rate = stream
        .avg_frame_rate
        .as_deref()
        .and_then(parse_rational)
        .or_else(|| stream.r_frame_rate.as_deref().and_then(parse_rational));
    let Some((frame_rate_numerator, frame_rate_denominator)) = frame_rate else {
        return false;
    };
    time_base_numerator == 1
        && frame_rate_denominator > 1
        && time_base_denominator == frame_rate_numerator
        && (1..=240).contains(&(frame_rate_numerator / frame_rate_denominator))
}

fn parse_rational(value: &str) -> Option<(u64, u64)> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<u64>().ok()?;
    let denominator = denominator.parse::<u64>().ok()?;
    (numerator > 0 && denominator > 0).then_some((numerator, denominator))
}

fn compatibility_cache_path(
    directory: &Path,
    item_id: &str,
    source_size: u64,
    source_modified_ms: u128,
) -> PathBuf {
    directory.join(format!(
        "{item_id}-v{COMPATIBILITY_CACHE_VERSION}-{source_size}-{source_modified_ms}.mkv"
    ))
}

fn transcode_cache_path(
    directory: &Path,
    item_id: &str,
    source_size: u64,
    source_modified_ms: u128,
) -> PathBuf {
    directory.join(format!(
        "{item_id}-h264-v{TRANSCODE_CACHE_VERSION}-{source_size}-{source_modified_ms}.mp4"
    ))
}

fn is_transcoded_compatibility_path(path: &Path) -> bool {
    path.file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| name.contains("-h264-v") && name.ends_with(".mp4"))
}

fn modified_millis(metadata: &std::fs::Metadata) -> u128 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_millis())
}

fn valid_cached_media(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0
    })
}

fn touch_cache_entry(path: &Path) {
    if let Ok(file) = OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn remove_stale_compatibility_files(directory: &Path, item_id: &str, current: &Path) {
    let prefix = format!("{item_id}-");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != current
            && path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(&prefix) && (name.ends_with(".mp4") || name.ends_with(".mkv"))
                })
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn remove_orphaned_playback_partials(directory: &Path) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if name.starts_with('.')
            && name.contains(".part.")
            && (name.ends_with(".mp4") || name.ends_with(".mkv"))
            && std::fs::symlink_metadata(&path)
                .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn create_hls_session_directory(
    cache_directory: &Path,
    stream_id: &str,
) -> Result<PathBuf, AppError> {
    let root = cache_directory.join("live-hls");
    match std::fs::symlink_metadata(&root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(AppError::new(
                "embedded_playback_cache_unsafe",
                "The temporary playback cache is not a safe directory. Clear or repair the application cache before retrying.",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(&root).map_err(|error| {
                AppError::internal("could not create the temporary HLS playback root", error)
            })?;
        }
        Err(error) => {
            return Err(AppError::internal(
                "could not inspect the temporary HLS playback root",
                error,
            ));
        }
    }
    let directory = root.join(stream_id);
    std::fs::create_dir(&directory).map_err(|error| {
        AppError::internal(
            "could not create the temporary HLS playback directory",
            error,
        )
    })?;
    Ok(directory)
}

fn remove_orphaned_hls_sessions(cache_directory: &Path) {
    let root = cache_directory.join("live-hls");
    if !std::fs::symlink_metadata(&root)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_session_directory = entry
            .file_name()
            .to_str()
            .is_some_and(|name| Uuid::parse_str(name).is_ok())
            && std::fs::symlink_metadata(&path)
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());
        if is_session_directory {
            let _ = std::fs::remove_dir_all(path);
        }
    }
}

fn enforce_compatibility_cache_limits(
    directory: &Path,
    preserve: Option<&Path>,
    reserved_files: usize,
    reserved_bytes: u64,
) -> Result<(), AppError> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| AppError::internal("could not inspect the playback cache", error))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with('.'))
            {
                return None;
            }
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then_some((
                entry.path(),
                metadata.len(),
                metadata.modified().unwrap_or(UNIX_EPOCH),
            ))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.2);
    let mut file_count = entries.len();
    let mut total_bytes = entries
        .iter()
        .fold(0u64, |total, entry| total.saturating_add(entry.1));
    for (path, size, _) in entries {
        if file_count.saturating_add(reserved_files) <= MAX_COMPATIBILITY_CACHE_FILES
            && total_bytes.saturating_add(reserved_bytes) <= MAX_COMPATIBILITY_CACHE_BYTES
        {
            break;
        }
        if preserve.is_some_and(|preserve| preserve == path) {
            continue;
        }
        if std::fs::remove_file(path).is_ok() {
            file_count = file_count.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(size);
        }
    }
    Ok(())
}

fn playback_cache_status(directory: &Path) -> Result<LibraryPlaybackCacheStatus, AppError> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LibraryPlaybackCacheStatus {
                file_count: 0,
                total_bytes: 0,
                max_files: MAX_COMPATIBILITY_CACHE_FILES,
                max_bytes: MAX_COMPATIBILITY_CACHE_BYTES,
            });
        }
        Err(error) => {
            return Err(AppError::internal(
                "could not inspect the playback cache",
                error,
            ));
        }
    };
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;
    for entry in entries.filter_map(Result::ok) {
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() {
            file_count = file_count.saturating_add(1);
            total_bytes = total_bytes.saturating_add(metadata.len());
        }
    }
    Ok(LibraryPlaybackCacheStatus {
        file_count,
        total_bytes,
        max_files: MAX_COMPATIBILITY_CACHE_FILES,
        max_bytes: MAX_COMPATIBILITY_CACHE_BYTES,
    })
}

fn clear_playback_cache(directory: &Path) -> Result<LibraryCacheResult, AppError> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LibraryCacheResult {
                removed_files: 0,
                freed_bytes: 0,
            });
        }
        Err(error) => {
            return Err(AppError::internal(
                "could not inspect the playback cache",
                error,
            ));
        }
    };
    let mut removed_files = 0usize;
    let mut freed_bytes = 0u64;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.is_file() && std::fs::remove_file(&path).is_ok() {
            removed_files = removed_files.saturating_add(1);
            freed_bytes = freed_bytes.saturating_add(metadata.len());
        }
    }
    Ok(LibraryCacheResult {
        removed_files,
        freed_bytes,
    })
}

async fn remux_for_webview(
    ffmpeg: &Path,
    source: &Path,
    output: &Path,
    timeout_seconds: u64,
    cancellation: &CancellationToken,
    reporter: PlaybackProgressReporter,
) -> Result<(), AppError> {
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-fflags")
        .arg("+genpts")
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a?")
        .arg("-map")
        .arg("0:s?")
        .arg("-dn")
        .arg("-c")
        .arg("copy")
        .arg("-map_metadata")
        .arg("0")
        .arg("-map_chapters")
        .arg("0")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg("-y")
        .arg(output);
    let status = run_playback_ffmpeg(
        &mut command,
        timeout_seconds,
        cancellation,
        reporter,
        "embedded_playback_preparation_timeout",
        "Preparing this AV1 video for embedded playback took too long. The original is unchanged; open it with the system player instead.",
        "could not start FFmpeg for embedded playback",
    )
    .await?;
    if !status.success() {
        tracing::warn!(status = ?status.code(), "FFmpeg could not prepare AV1 embedded playback");
        return Err(AppError::new(
            "embedded_playback_preparation_failed",
            "FFmpeg could not prepare this AV1 video for embedded playback. The original is unchanged and can still be opened with the system player.",
        ));
    }
    Ok(())
}

async fn transcode_for_webview(
    ffmpeg: &Path,
    source: &Path,
    output: &Path,
    timeout_seconds: u64,
    cancellation: &CancellationToken,
    reporter: PlaybackProgressReporter,
) -> Result<(), AppError> {
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-fflags")
        .arg("+genpts")
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-sn")
        .arg("-dn")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("22")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-fps_mode")
        .arg("vfr")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("160k")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-map_metadata")
        .arg("0")
        .arg("-progress")
        .arg("pipe:1")
        .arg("-nostats")
        .arg("-y")
        .arg(output);
    let status = run_playback_ffmpeg(
        &mut command,
        timeout_seconds,
        cancellation,
        reporter,
        "embedded_playback_transcode_timeout",
        "Creating the H.264 playback copy took too long. The original is unchanged; open it with the system player instead.",
        "could not start FFmpeg fallback transcode",
    )
    .await?;
    if !status.success() {
        tracing::warn!(status = ?status.code(), "FFmpeg could not create H.264 embedded playback fallback");
        return Err(AppError::new(
            "embedded_playback_transcode_failed",
            "FFmpeg could not create the H.264 playback copy. The original is unchanged and can still be opened with the system player.",
        ));
    }
    Ok(())
}

async fn run_playback_ffmpeg(
    command: &mut Command,
    timeout_seconds: u64,
    cancellation: &CancellationToken,
    reporter: PlaybackProgressReporter,
    timeout_code: &'static str,
    timeout_message: &'static str,
    start_context: &'static str,
) -> Result<std::process::ExitStatus, AppError> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal(start_context, error))?;
    let stdout = child.stdout.take().ok_or_else(|| {
        AppError::new(
            "embedded_playback_preparation_io",
            "FFmpeg progress output was unavailable.",
        )
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        AppError::new(
            "embedded_playback_preparation_io",
            "FFmpeg diagnostic output was unavailable.",
        )
    })?;
    let progress_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(processed_seconds) = parse_ffmpeg_progress_seconds(&line) {
                reporter.update_progress(processed_seconds);
            }
        }
    });
    let diagnostic_task = tokio::spawn(async move {
        let mut diagnostic = String::new();
        let _ = stderr.read_to_string(&mut diagnostic).await;
        diagnostic
    });

    enum ProcessOutcome {
        Completed(std::io::Result<std::process::ExitStatus>),
        Cancelled,
        TimedOut,
    }

    let outcome = tokio::select! {
        result = child.wait() => ProcessOutcome::Completed(result),
        _ = cancellation.cancelled() => ProcessOutcome::Cancelled,
        _ = tokio::time::sleep(Duration::from_secs(timeout_seconds)) => ProcessOutcome::TimedOut,
    };
    let status = match outcome {
        ProcessOutcome::Completed(result) => {
            result.map_err(|error| AppError::internal("could not wait for FFmpeg", error))?
        }
        ProcessOutcome::Cancelled => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = progress_task.await;
            let _ = diagnostic_task.await;
            return Err(playback_preparation_cancelled());
        }
        ProcessOutcome::TimedOut => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            let _ = progress_task.await;
            let _ = diagnostic_task.await;
            return Err(AppError::new(timeout_code, timeout_message));
        }
    };
    let _ = progress_task.await;
    let diagnostic = diagnostic_task.await.unwrap_or_default();
    if !status.success() && !diagnostic.is_empty() {
        tracing::debug!(diagnostic = %diagnostic.chars().take(2_000).collect::<String>(), "FFmpeg playback-copy diagnostic");
    }
    Ok(status)
}

fn parse_ffmpeg_progress_seconds(line: &str) -> Option<f64> {
    line.strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
        .and_then(|value| value.parse::<f64>().ok())
        .map(|value| value / 1_000_000.0)
}

fn playback_preparation_cancelled() -> AppError {
    AppError::new(
        "embedded_playback_preparation_cancelled",
        "Playback-copy preparation was cancelled. The original video is unchanged.",
    )
}

pub fn start(database: Database, cache_directory: PathBuf) -> Result<MediaServerState, AppError> {
    remove_orphaned_playback_partials(&cache_directory);
    remove_orphaned_hls_sessions(&cache_directory);
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
        cache_directory,
        playback_paths: Arc::new(RwLock::new(HashMap::new())),
        artwork_paths: Arc::new(RwLock::new(HashMap::new())),
        compatibility_lock: Arc::new(AsyncMutex::new(())),
        playback_preparations: Arc::new(RwLock::new(HashMap::new())),
        preparation_slots: Arc::new(Semaphore::new(2)),
        live_transcodes: Arc::new(RwLock::new(HashMap::new())),
        hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
        live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
        webview_dependency_error: Arc::new(OnceCell::new()),
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
    let mut command = tokio::process::Command::new(webview_gst_inspector());
    command
        .arg("autoaudiosink")
        .env("LC_ALL", "C")
        .kill_on_drop(true);
    match timeout(Duration::from_secs(3), command.output()).await {
        Ok(Ok(output)) => {
            let dependency_error = classify_webview_dependency_probe(
                output.status.success(),
                &output.stdout,
                &output.stderr,
            );
            if dependency_error.is_none() && !output.status.success() {
                tracing::debug!(
                    stderr = %String::from_utf8_lossy(&output.stderr),
                    "GStreamer inspection failed without proving autoaudiosink is missing"
                );
            }
            dependency_error
        }
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

#[cfg(target_os = "linux")]
fn webview_gst_inspector() -> PathBuf {
    std::env::var_os("APPDIR")
        .map(PathBuf::from)
        .map(|app_dir| app_dir.join("usr/bin/gst-inspect-1.0"))
        .filter(|inspector| inspector.is_file())
        .unwrap_or_else(|| PathBuf::from("gst-inspect-1.0"))
}

#[cfg(target_os = "linux")]
fn classify_webview_dependency_probe(
    succeeded: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Option<String> {
    if succeeded {
        return None;
    }

    let diagnostic = format!(
        "{}\n{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    )
    .to_ascii_lowercase();
    if diagnostic.contains("no such element or plugin") && diagnostic.contains("autoaudiosink") {
        return Some(
            "Embedded playback needs the GStreamer audio-output plugins used by WebKitGTK. Install the package that provides autoaudiosink (Arch Linux: gst-plugins-good; Debian/Ubuntu: gstreamer1.0-plugins-good), restart Nice Downloader, or open this video in your system player."
                .to_owned(),
        );
    }

    None
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
    if let ["transcode", item_id, stream_file] = segments.as_slice() {
        let Some(stream_id) = stream_file.strip_suffix(".mp4") else {
            return response(StatusCode::NOT_FOUND, empty_body());
        };
        return match serve_live_transcode(&request, &state, item_id, stream_id).await {
            Ok(response) => response,
            Err(status) => response(status, empty_body()),
        };
    }
    if let ["hls", item_id, stream_id, file_name] = segments.as_slice() {
        return match serve_hls(&request, &state, item_id, stream_id, file_name).await {
            Ok(response) => response,
            Err(status) => response(status, empty_body()),
        };
    }
    let artwork_response = matches!(segments.as_slice(), ["artwork", _]);
    let resolved = match segments.as_slice() {
        ["media", item_id] => resolve_media_path(&state, item_id),
        ["image", item_id] => resolve_image_path(&state.database, item_id),
        ["artwork", item_id] => resolve_artwork_path(&state, item_id),
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
        Ok(mut response) => {
            if artwork_response {
                response.headers_mut().insert(
                    CACHE_CONTROL,
                    hyper::header::HeaderValue::from_static("private, max-age=31536000, immutable"),
                );
            }
            response
        }
        Err(error) => {
            tracing::warn!(%error, "could not stream a library media file");
            response(StatusCode::INTERNAL_SERVER_ERROR, empty_body())
        }
    }
}

fn resolve_artwork_path(
    state: &MediaServerState,
    item_id: &str,
) -> Result<(PathBuf, &'static str), StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let item = state
        .database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if item.missing {
        return Err(StatusCode::NOT_FOUND);
    }
    let path = state
        .artwork_paths
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(item_id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    validate_regular_media_file(&path).map_err(|_| StatusCode::NOT_FOUND)?;
    if path.extension().and_then(OsStr::to_str) != Some("jpg") {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok((path, "image/jpeg"))
}

fn resolve_media_path(
    state: &MediaServerState,
    item_id: &str,
) -> Result<(PathBuf, &'static str), StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let item = state
        .database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if item.missing || item.media_kind != Some(LibraryMediaKind::Video) {
        return Err(StatusCode::NOT_FOUND);
    }
    let source = PathBuf::from(item.path);
    validate_regular_media_file(&source).map_err(|_| StatusCode::NOT_FOUND)?;
    let compatibility = state
        .playback_paths
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(item_id)
        .cloned();
    if let Some(path) = compatibility {
        let is_internal_cache_path = path.parent() == Some(state.cache_directory.as_path())
            && path
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(item_id) && (name.ends_with(".mkv") || name.ends_with(".mp4"))
                });
        if is_internal_cache_path && valid_cached_media(&path) {
            let content_type = if is_transcoded_compatibility_path(&path) {
                "video/mp4"
            } else {
                "video/x-matroska"
            };
            return Ok((path, content_type));
        }
    }
    Ok((source.clone(), video_content_type(&source)))
}

fn resolve_image_path(
    database: &Database,
    item_id: &str,
) -> Result<(PathBuf, &'static str), StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let item = database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if item.missing || item.media_kind != Some(LibraryMediaKind::Image) {
        return Err(StatusCode::NOT_FOUND);
    }
    let source = PathBuf::from(item.path);
    validate_regular_media_file(&source).map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = image_content_type(&source).ok_or(StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    Ok((source, content_type))
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

async fn serve_hls(
    request: &Request<Incoming>,
    state: &MediaServerState,
    item_id: &str,
    stream_id: &str,
    file_name: &str,
) -> Result<Response<MediaBody>, StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    Uuid::parse_str(stream_id).map_err(|_| StatusCode::NOT_FOUND)?;
    let content_type = hls_content_type(file_name).ok_or(StatusCode::NOT_FOUND)?;
    let item = state
        .database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    validate_playable_item(&item).map_err(|_| StatusCode::NOT_FOUND)?;
    let source = PathBuf::from(&item.path);

    let authorization = {
        let mut streams = state
            .hls_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stream = streams
            .get_mut(stream_id)
            .filter(|stream| {
                stream.item_id == item_id
                    && (stream.started
                        || stream.created_at.elapsed() <= LIVE_TRANSCODE_AUTHORIZATION_TTL)
            })
            .ok_or(StatusCode::NOT_FOUND)?;
        stream.last_access = Instant::now();
        stream.clone()
    };

    if request.method() == Method::HEAD && file_name == "index.m3u8" {
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, content_type)
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(CACHE_CONTROL, "no-store")
            .body(empty_body())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    if file_name == "index.m3u8" {
        ensure_hls_started(state, item_id, stream_id, &source).await?;
        wait_for_hls_manifest(&authorization.directory, &authorization.cancellation).await?;
        serve_hls_manifest(
            request,
            &authorization.directory.join(file_name),
            &state.token,
        )
        .await
    } else {
        let path = authorization.directory.join(file_name);
        validate_regular_media_file(&path).map_err(|_| StatusCode::NOT_FOUND)?;
        serve_file(request, &path, content_type)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

fn hls_content_type(file_name: &str) -> Option<&'static str> {
    if file_name == "index.m3u8" {
        return Some("application/vnd.apple.mpegurl");
    }
    let number = file_name.strip_prefix("segment_")?.strip_suffix(".ts")?;
    (!number.is_empty() && number.len() <= 8 && number.bytes().all(|byte| byte.is_ascii_digit()))
        .then_some("video/mp2t")
}

async fn ensure_hls_started(
    state: &MediaServerState,
    item_id: &str,
    stream_id: &str,
    source: &Path,
) -> Result<(), StatusCode> {
    let authorization = {
        let mut streams = state
            .hls_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stream = streams.get_mut(stream_id).ok_or(StatusCode::NOT_FOUND)?;
        if stream.item_id != item_id {
            return Err(StatusCode::NOT_FOUND);
        }
        if stream.started {
            return Ok(());
        }
        stream.started = true;
        stream.clone()
    };

    let permit = match timeout(
        Duration::from_secs(3),
        state.live_transcode_slots.clone().acquire_owned(),
    )
    .await
    {
        Ok(Ok(permit)) => permit,
        _ => {
            if let Some(stream) = state
                .hls_transcodes
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get_mut(stream_id)
            {
                stream.started = false;
            }
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let Some(ffmpeg) = managed::resolve("ffmpeg") else {
        if let Some(stream) = state
            .hls_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_mut(stream_id)
        {
            stream.started = false;
        }
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut command = hls_transcode_command(
        &ffmpeg,
        source,
        authorization.start_seconds,
        &authorization.directory,
    );
    command
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(%error, item_id, "could not start segmented compatibility playback");
            if let Some(stream) = state
                .hls_transcodes
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .get_mut(stream_id)
            {
                stream.started = false;
            }
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let Some(mut stderr) = child.stderr.take() else {
        authorization.cancellation.cancel();
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let cancellation = authorization.cancellation.clone();
    let process_cancellation = cancellation.clone();
    let completed_item_id = item_id.to_owned();
    let completed_directory = authorization.directory.clone();
    let diagnostic_task = tauri::async_runtime::spawn(async move {
        let mut buffer = [0u8; 1024];
        loop {
            match stderr.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        let should_remove = tokio::select! {
            result = child.wait() => {
                match result {
                    Ok(status) if status.success() => false,
                    Ok(status) => {
                        tracing::debug!(item_id = %completed_item_id, ?status, "segmented compatibility playback ended with an error");
                        process_cancellation.cancel();
                        true
                    }
                    Err(error) => {
                        tracing::debug!(item_id = %completed_item_id, %error, "could not wait for segmented compatibility playback");
                        process_cancellation.cancel();
                        true
                    }
                }
            }
            _ = process_cancellation.cancelled() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                true
            }
        };
        let _ = diagnostic_task.await;
        if should_remove {
            let _ = std::fs::remove_dir_all(completed_directory);
        }
        drop(permit);
    });
    Ok(())
}

async fn wait_for_hls_manifest(
    directory: &Path,
    cancellation: &CancellationToken,
) -> Result<(), StatusCode> {
    timeout(HLS_START_TIMEOUT, async {
        loop {
            if cancellation.is_cancelled() {
                return Err(StatusCode::GONE);
            }
            let manifest = directory.join("index.m3u8");
            if std::fs::metadata(manifest)
                .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .map_err(|_| StatusCode::GATEWAY_TIMEOUT)?
}

async fn serve_hls_manifest(
    request: &Request<Incoming>,
    path: &Path,
    token: &str,
) -> Result<Response<MediaBody>, StatusCode> {
    validate_regular_media_file(path).map_err(|_| StatusCode::NOT_FOUND)?;
    let metadata = std::fs::metadata(path).map_err(|_| StatusCode::NOT_FOUND)?;
    if metadata.len() == 0 || metadata.len() > MAX_HLS_MANIFEST_BYTES {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let contents = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let contents =
        authorize_hls_manifest(&contents, token).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    let builder = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/vnd.apple.mpegurl")
        .header(CONTENT_LENGTH, contents.len())
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if request.method() == Method::HEAD {
        return builder
            .body(empty_body())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    builder
        .body(
            Full::new(Bytes::from(contents))
                .map_err(|never| -> BoxError { match never {} })
                .boxed_unsync(),
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn authorize_hls_manifest(contents: &str, token: &str) -> Option<String> {
    let mut authorized = String::with_capacity(contents.len().saturating_add(256));
    for line in contents.lines() {
        if line.is_empty() || line.starts_with('#') {
            authorized.push_str(line);
        } else {
            if hls_content_type(line) != Some("video/mp2t") {
                return None;
            }
            authorized.push_str(line);
            authorized.push_str(if line.contains('?') {
                "&token="
            } else {
                "?token="
            });
            authorized.push_str(token);
        }
        authorized.push('\n');
    }
    Some(authorized)
}

fn hls_transcode_command(
    ffmpeg: &Path,
    source: &Path,
    start_seconds: f64,
    directory: &Path,
) -> Command {
    let mut command = Command::new(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin");
    if start_seconds > 0.0 {
        command.arg("-ss").arg(format!("{start_seconds:.3}"));
    }
    command
        .arg("-re")
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-sn")
        .arg("-dn")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("23")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-vf")
        .arg("scale=trunc(iw/2)*2:trunc(ih/2)*2")
        .arg("-fps_mode")
        .arg("vfr")
        .arg("-force_key_frames")
        .arg("expr:gte(t,n_forced*2)")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("160k")
        .arg("-ac")
        .arg("2")
        .arg("-f")
        .arg("hls")
        .arg("-hls_time")
        .arg("2")
        .arg("-hls_list_size")
        .arg("0")
        .arg("-hls_playlist_type")
        .arg("event")
        .arg("-hls_flags")
        .arg("independent_segments+temp_file")
        .arg("-start_number")
        .arg("0")
        .arg("-hls_segment_filename")
        .arg(directory.join("segment_%06d.ts"))
        .arg(directory.join("index.m3u8"));
    command
}

async fn serve_live_transcode(
    request: &Request<Incoming>,
    state: &MediaServerState,
    item_id: &str,
    stream_id: &str,
) -> Result<Response<MediaBody>, StatusCode> {
    Uuid::parse_str(item_id).map_err(|_| StatusCode::NOT_FOUND)?;
    Uuid::parse_str(stream_id).map_err(|_| StatusCode::NOT_FOUND)?;

    let item = state
        .database
        .get_library_item(item_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    validate_playable_item(&item).map_err(|_| StatusCode::NOT_FOUND)?;
    let source = PathBuf::from(&item.path);

    let authorization = state
        .live_transcodes
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(stream_id)
        .filter(|authorization| {
            authorization.item_id == item_id
                && (authorization.started
                    || authorization.created_at.elapsed() <= LIVE_TRANSCODE_AUTHORIZATION_TTL)
        })
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;

    if request.method() == Method::HEAD {
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "video/mp4")
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(CACHE_CONTROL, "no-store")
            .body(empty_body())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    let permit = timeout(
        Duration::from_secs(3),
        state.live_transcode_slots.clone().acquire_owned(),
    )
    .await
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    {
        let mut streams = state
            .live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stream = streams.get_mut(stream_id).ok_or(StatusCode::NOT_FOUND)?;
        if stream.item_id != item_id {
            return Err(StatusCode::NOT_FOUND);
        }
        if stream.started {
            return Err(StatusCode::CONFLICT);
        }
        stream.started = true;
    }

    let Some(ffmpeg) = managed::resolve("ffmpeg") else {
        state
            .live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(stream_id);
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    };
    let mut command = live_transcode_command(&ffmpeg, &source, authorization.start_seconds);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!(%error, item_id, "could not start live compatibility playback");
            state
                .live_transcodes
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(stream_id);
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
    };
    let Some(stdout) = child.stdout.take() else {
        authorization.cancellation.cancel();
        state
            .live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(stream_id);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let Some(mut stderr) = child.stderr.take() else {
        authorization.cancellation.cancel();
        state
            .live_transcodes
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(stream_id);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let cancellation = authorization.cancellation.clone();
    let process_cancellation = cancellation.clone();
    let streams = state.live_transcodes.clone();
    let completed_stream_id = stream_id.to_owned();
    let completed_item_id = item_id.to_owned();

    let diagnostic_task = tauri::async_runtime::spawn(async move {
        let mut buffer = [0u8; 1024];
        loop {
            match stderr.read(&mut buffer).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });

    tauri::async_runtime::spawn(async move {
        tokio::select! {
            result = child.wait() => {
                match result {
                    Ok(status) if status.success() => {},
                    Ok(status) => {
                        tracing::debug!(item_id = %completed_item_id, ?status, "live compatibility playback ended");
                    }
                    Err(error) => {
                        tracing::debug!(item_id = %completed_item_id, %error, "could not wait for live compatibility playback");
                    }
                }
            }
            _ = process_cancellation.cancelled() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
            }
        };
        let _ = diagnostic_task.await;
        streams
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&completed_stream_id);
        drop(permit);
    });

    let stream = ReaderStream::new(stdout)
        .map_ok(Frame::data)
        .map_err(|error| -> BoxError { Box::new(error) });
    let body = LiveTranscodeBodyStream {
        inner: Box::pin(stream),
        cancellation,
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "video/mp4")
        .header(ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(CACHE_CONTROL, "no-store")
        .body(StreamBody::new(body).boxed_unsync())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn live_transcode_command(ffmpeg: &Path, source: &Path, start_seconds: f64) -> Command {
    let mut command = Command::new(ffmpeg);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin");
    if start_seconds > 0.0 {
        command.arg("-ss").arg(format!("{start_seconds:.3}"));
    }
    command
        .arg("-i")
        .arg(source)
        .arg("-map")
        .arg("0:v:0")
        .arg("-map")
        .arg("0:a:0?")
        .arg("-sn")
        .arg("-dn")
        .arg("-map_metadata")
        .arg("-1")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-tune")
        .arg("zerolatency")
        .arg("-crf")
        .arg("23")
        .arg("-pix_fmt")
        .arg("yuv420p")
        .arg("-vf")
        .arg("scale=trunc(iw/2)*2:trunc(ih/2)*2")
        .arg("-fps_mode")
        .arg("vfr")
        .arg("-g")
        .arg("60")
        .arg("-keyint_min")
        .arg("60")
        .arg("-sc_threshold")
        .arg("0")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("160k")
        .arg("-ac")
        .arg("2")
        .arg("-avoid_negative_ts")
        .arg("make_zero")
        .arg("-movflags")
        .arg("+frag_keyframe+empty_moov+default_base_moof")
        .arg("-frag_duration")
        .arg("1000000")
        .arg("-flush_packets")
        .arg("1")
        .arg("-f")
        .arg("mp4")
        .arg("pipe:1");
    command
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

fn image_content_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "avif" => Some("image/avif"),
        _ => None,
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

    #[cfg(target_os = "linux")]
    #[test]
    fn webview_dependency_probe_reports_missing_auto_audio_sink() {
        let error = classify_webview_dependency_probe(
            false,
            b"",
            b"No such element or plugin 'autoaudiosink'",
        )
        .expect("missing audio sink should be actionable");

        assert!(error.contains("autoaudiosink"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn webview_dependency_probe_ignores_inspector_loader_mismatch() {
        assert_eq!(
            classify_webview_dependency_probe(
                false,
                b"",
                b"gst-inspect-1.0: symbol lookup error: undefined symbol: gst_id_str_as_str",
            ),
            None
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn webview_dependency_probe_accepts_success() {
        assert_eq!(
            classify_webview_dependency_probe(true, b"Factory Details", b""),
            None
        );
    }

    #[test]
    fn parses_ffmpeg_progress_timestamps() {
        assert_eq!(
            parse_ffmpeg_progress_seconds("out_time_us=12500000"),
            Some(12.5)
        );
        assert_eq!(
            parse_ffmpeg_progress_seconds("out_time_ms=2500000"),
            Some(2.5)
        );
        assert_eq!(parse_ffmpeg_progress_seconds("progress=continue"), None);
    }

    #[test]
    fn playback_preparation_reports_progress_and_can_be_cancelled() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let state = MediaServerState::unavailable(database);
        let item = video_item(&directory.path().join("video.mp4"), 128);
        let (guard, reporter, cancellation) = state
            .begin_playback_preparation(&item, true, 256)
            .expect("register preparation");

        reporter.update(LibraryPlaybackPreparationPhase::Encoding, Some(5.0));
        let progress = state
            .playback_preparation(&item.id)
            .expect("preparation status")
            .expect("active preparation");
        assert_eq!(progress.phase, LibraryPlaybackPreparationPhase::Encoding);
        assert_eq!(progress.progress_percent, Some(50.0));

        state
            .cancel_playback_preparation(&item.id)
            .expect("cancel preparation");
        assert!(cancellation.is_cancelled());
        drop(guard);
        assert!(state
            .playback_preparation(&item.id)
            .expect("preparation status")
            .is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn running_playback_process_stops_after_cancellation() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let state = MediaServerState::unavailable(database);
        let item = video_item(&directory.path().join("video.mp4"), 128);
        let (_guard, reporter, cancellation) = state
            .begin_playback_preparation(&item, false, 128)
            .expect("register preparation");
        reporter.update(LibraryPlaybackPreparationPhase::Copying, Some(0.0));
        let cancellation_request = cancellation.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            cancellation_request.cancel();
        });
        let mut command = Command::new("tail");
        command.args(["-f", "/dev/null"]);

        let error = run_playback_ffmpeg(
            &mut command,
            10,
            &cancellation,
            reporter,
            "fixture_timeout",
            "fixture timeout",
            "could not start fixture process",
        )
        .await
        .expect_err("cancelled process must stop");

        assert_eq!(error.code, "embedded_playback_preparation_cancelled");
    }

    #[test]
    fn cache_pruning_never_removes_active_partial_files() {
        let directory = tempdir().expect("temporary directory");
        let partial = directory.path().join(".active.part.mp4");
        std::fs::write(&partial, [1u8; 8]).expect("partial cache file");
        for index in 0..5 {
            std::fs::write(
                directory.path().join(format!("cached-{index}.mkv")),
                [1u8; 8],
            )
            .expect("completed cache file");
        }

        enforce_compatibility_cache_limits(directory.path(), None, 0, 0).expect("prune cache");

        assert!(partial.exists());
        let completed = std::fs::read_dir(directory.path())
            .expect("cache entries")
            .filter_map(Result::ok)
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
            .count();
        assert_eq!(completed, MAX_COMPATIBILITY_CACHE_FILES);
    }

    fn video_item(path: &Path, size: u64) -> LibraryItem {
        let now = Utc::now();
        LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "Playback fixture".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(10.0),
            site: None,
            tags: vec![],
            metadata: serde_json::json!({}),
            file_size_bytes: Some(size),
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
        }
    }

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

    #[test]
    fn detects_fractional_av1_mp4_timing_that_needs_normalization() {
        let affected = CompatibilityVideoStream {
            codec_name: Some("av1".to_owned()),
            time_base: Some("1/29719".to_owned()),
            avg_frame_rate: Some("29719/1000".to_owned()),
            r_frame_rate: Some("29719/1000".to_owned()),
        };
        let conventional = CompatibilityVideoStream {
            codec_name: Some("av1".to_owned()),
            time_base: Some("1/15360".to_owned()),
            avg_frame_rate: Some("30/1".to_owned()),
            r_frame_rate: Some("30/1".to_owned()),
        };
        let other_codec = CompatibilityVideoStream {
            codec_name: Some("h264".to_owned()),
            time_base: Some("1/30000".to_owned()),
            avg_frame_rate: Some("30000/1001".to_owned()),
            r_frame_rate: Some("30000/1001".to_owned()),
        };

        assert!(has_problematic_av1_timing(&affected));
        assert!(!has_problematic_av1_timing(&conventional));
        assert!(!has_problematic_av1_timing(&other_codec));
    }

    #[test]
    fn compatibility_cache_identity_uses_only_catalog_and_fingerprint_data() {
        let directory = Path::new("/cache");
        let item_id = "f9d945f0-907e-4b28-998a-87f89b388ba0";
        let path = compatibility_cache_path(directory, item_id, 42_000, 1_234_567);

        assert_eq!(
            path.file_name().and_then(OsStr::to_str),
            Some("f9d945f0-907e-4b28-998a-87f89b388ba0-v2-42000-1234567.mkv")
        );
        let transcoded = transcode_cache_path(directory, item_id, 42_000, 1_234_567);
        assert_eq!(
            transcoded.file_name().and_then(OsStr::to_str),
            Some("f9d945f0-907e-4b28-998a-87f89b388ba0-h264-v1-42000-1234567.mp4")
        );
        assert!(is_transcoded_compatibility_path(&transcoded));
    }

    #[test]
    fn compatibility_consent_respects_preferences_cache_and_large_files() {
        let ordinary = 512 * 1024 * 1024;
        let large = LARGE_COMPATIBILITY_COPY_BYTES;

        assert!(compatibility_requires_confirmation(
            PlaybackCompatibilityPreference::Ask,
            ordinary,
            false
        ));
        assert!(compatibility_requires_confirmation(
            PlaybackCompatibilityPreference::Never,
            ordinary,
            false
        ));
        assert!(!compatibility_requires_confirmation(
            PlaybackCompatibilityPreference::Automatic,
            ordinary,
            false
        ));
        assert!(compatibility_requires_confirmation(
            PlaybackCompatibilityPreference::Automatic,
            large,
            false
        ));
        assert!(!compatibility_requires_confirmation(
            PlaybackCompatibilityPreference::Ask,
            large,
            true
        ));
    }

    #[tokio::test]
    async fn reports_and_clears_compatibility_cache_and_registered_paths() {
        let directory = tempdir().expect("temporary directory");
        let cache_directory = directory.path().join("playback-cache");
        std::fs::create_dir_all(&cache_directory).expect("cache directory");
        let first = cache_directory.join("first.mkv");
        let second = cache_directory.join("second.mkv");
        std::fs::write(&first, vec![0u8; 32]).expect("first cache file");
        std::fs::write(&second, vec![0u8; 48]).expect("second cache file");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let active_item_id = Uuid::new_v4().to_string();
        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory,
            playback_paths: Arc::new(RwLock::new(HashMap::from([(
                active_item_id.clone(),
                first,
            )]))),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new()),
        };

        let before = state
            .compatibility_cache_status()
            .await
            .expect("cache status");
        assert_eq!(before.file_count, 2);
        assert_eq!(before.total_bytes, 80);

        let error = state
            .clear_compatibility_cache()
            .await
            .expect_err("active playback blocks cache clearing");
        assert_eq!(error.code, "playback_cache_in_use");
        state
            .release_playback(&active_item_id)
            .expect("release playback");
        let cleared = state
            .clear_compatibility_cache()
            .await
            .expect("clear released cache");
        assert_eq!(cleared.removed_files, 2);
        assert_eq!(cleared.freed_bytes, 80);
        assert!(state
            .playback_paths
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_empty());
        assert_eq!(
            state
                .compatibility_cache_status()
                .await
                .expect("empty cache")
                .file_count,
            0
        );
    }

    #[tokio::test]
    async fn playback_preflight_reports_missing_runtime_before_copy_consent() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory: directory.path().join("playback-cache"),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new_with(Some(Some(
                "GStreamer fixture is missing".to_owned(),
            )))),
        };

        let error = state
            .inspect_webview_playback(&Uuid::new_v4().to_string())
            .await
            .expect_err("dependency failure must stop preflight");

        assert_eq!(error.code, "embedded_playback_dependency_missing");
        assert_eq!(error.message, "GStreamer fixture is missing");
    }

    #[tokio::test]
    async fn h264_fallback_preflight_always_requires_explicit_consent_until_cached() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("video.mp4");
        std::fs::write(&source, vec![0u8; 128]).expect("source fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let item = video_item(&source, 128);
        database.upsert_library_item(&item).expect("catalog item");
        let cache_directory = directory.path().join("playback-cache");
        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory: cache_directory.clone(),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new_with(Some(None))),
        };

        let first = state
            .inspect_transcoded_webview_playback(&item.id)
            .await
            .expect("fallback preflight");
        assert!(first.reencode_required);
        assert!(first.requires_confirmation);
        assert_eq!(first.estimated_copy_bytes, 512);

        std::fs::create_dir_all(&cache_directory).expect("cache directory");
        let metadata = std::fs::metadata(&source).expect("source metadata");
        let cached = transcode_cache_path(
            &cache_directory,
            &item.id,
            metadata.len(),
            modified_millis(&metadata),
        );
        std::fs::write(cached, b"cached h264 fixture").expect("cached fallback");
        let second = state
            .inspect_transcoded_webview_playback(&item.id)
            .await
            .expect("cached fallback preflight");
        assert!(second.compatibility_cached);
        assert!(!second.requires_confirmation);
    }

    #[test]
    fn resolves_only_registered_compatibility_files_inside_the_private_cache() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("video.mp4");
        std::fs::write(&source, b"original").expect("source fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let item = video_item(&source, 8);
        database.upsert_library_item(&item).expect("catalog item");
        let cache_directory = directory.path().join("playback-cache");
        std::fs::create_dir_all(&cache_directory).expect("cache directory");
        let compatible = compatibility_cache_path(&cache_directory, &item.id, 8, 1);
        std::fs::write(&compatible, b"compatible").expect("compatibility fixture");
        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory: cache_directory.clone(),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new()),
        };

        state.set_playback_path(&item.id, Some(compatible.clone()));
        assert_eq!(
            resolve_media_path(&state, &item.id),
            Ok((compatible.clone(), "video/x-matroska"))
        );

        let second_item_id = Uuid::new_v4().to_string();
        let second_compatible = compatibility_cache_path(&cache_directory, &second_item_id, 9, 2);
        std::fs::write(&second_compatible, b"second compatibility fixture")
            .expect("second compatibility fixture");
        state.set_playback_path(&second_item_id, Some(second_compatible));
        assert!(state.registered_playback_path(&item.id).is_some());
        assert!(state.registered_playback_path(&second_item_id).is_some());
        assert_eq!(
            resolve_media_path(&state, &item.id),
            Ok((compatible, "video/x-matroska"))
        );

        let outside_cache = directory.path().join(format!("{}-fake.mp4", item.id));
        std::fs::write(&outside_cache, b"unsafe").expect("outside fixture");
        state.set_playback_path(&item.id, Some(outside_cache));
        assert_eq!(
            resolve_media_path(&state, &item.id),
            Ok((source, "video/mp4"))
        );

        let transcoded = transcode_cache_path(&cache_directory, &item.id, 8, 3);
        std::fs::write(&transcoded, b"h264 compatibility fixture").expect("transcoded fixture");
        state.set_playback_path(&item.id, Some(transcoded.clone()));
        assert_eq!(
            resolve_media_path(&state, &item.id),
            Ok((transcoded, "video/mp4"))
        );
        assert!(
            state
                .prepare_playback(&item.id)
                .expect("transcoded playback")
                .compatibility_transcoded
        );
    }

    #[test]
    fn creates_a_fresh_media_url_each_time_an_item_is_reopened() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("video.mp4");
        std::fs::write(&source, b"original").expect("source fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let item = video_item(&source, 8);
        database.upsert_library_item(&item).expect("catalog item");
        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory: directory.path().join("playback-cache"),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new()),
        };

        let first = state.prepare_playback(&item.id).expect("first session");
        let second = state.prepare_playback(&item.id).expect("second session");

        assert_ne!(first.url, second.url);
        assert!(first.url.contains("token=fixture-token&session="));
        assert!(second.url.contains("token=fixture-token&session="));
    }

    #[test]
    fn creates_fresh_image_sessions_and_rejects_non_images() {
        let directory = tempdir().expect("temporary directory");
        let image_path = directory.path().join("poster.png");
        std::fs::write(&image_path, b"image fixture").expect("image fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let mut image = video_item(&image_path, 13);
        image.media_kind = Some(LibraryMediaKind::Image);
        image.duration_seconds = None;
        image.video_codec = None;
        image.audio_codec = None;
        image.container = Some("png".to_owned());
        database.upsert_library_item(&image).expect("catalog image");

        let video_path = directory.path().join("video.mp4");
        std::fs::write(&video_path, b"video fixture").expect("video fixture");
        let video = video_item(&video_path, 13);
        database.upsert_library_item(&video).expect("catalog video");

        let state = MediaServerState {
            database,
            base_url: Some("http://127.0.0.1:1".to_owned()),
            token: "fixture-token".to_owned(),
            cache_directory: directory.path().join("playback-cache"),
            playback_paths: Arc::new(RwLock::new(HashMap::new())),
            artwork_paths: Arc::new(RwLock::new(HashMap::new())),
            compatibility_lock: Arc::new(AsyncMutex::new(())),
            playback_preparations: Arc::new(RwLock::new(HashMap::new())),
            preparation_slots: Arc::new(Semaphore::new(2)),
            live_transcodes: Arc::new(RwLock::new(HashMap::new())),
            hls_transcodes: Arc::new(RwLock::new(HashMap::new())),
            live_transcode_slots: Arc::new(Semaphore::new(MAX_LIVE_TRANSCODES)),
            webview_dependency_error: Arc::new(OnceCell::new()),
        };

        let first = state
            .prepare_image_view(&image.id)
            .expect("first image session");
        let second = state
            .prepare_image_view(&image.id)
            .expect("second image session");
        assert_ne!(first.url, second.url);
        assert!(first
            .url
            .contains(&format!("/image/{}?token=fixture-token&session=", image.id)));
        assert_eq!(
            resolve_image_path(&state.database, &image.id),
            Ok((image_path, "image/png"))
        );
        assert_eq!(
            resolve_image_path(&state.database, &video.id),
            Err(StatusCode::NOT_FOUND)
        );
        assert_eq!(
            state
                .prepare_image_view(&video.id)
                .expect_err("video rejected")
                .code,
            "library_image_not_found"
        );
    }

    #[test]
    fn hls_manifest_authorizes_only_segment_lines() {
        let manifest = "#EXTM3U\n#EXTINF:2.0,\nsegment_000000.ts\n#EXT-X-ENDLIST\n";
        assert_eq!(
            authorize_hls_manifest(manifest, "fixture-token"),
            Some(
                "#EXTM3U\n#EXTINF:2.0,\nsegment_000000.ts?token=fixture-token\n#EXT-X-ENDLIST\n"
                    .to_owned(),
            ),
        );
        assert_eq!(authorize_hls_manifest("/private/video.ts\n", "token"), None);
        assert_eq!(
            hls_content_type("index.m3u8"),
            Some("application/vnd.apple.mpegurl")
        );
        assert_eq!(hls_content_type("segment_000042.ts"), Some("video/mp2t"));
        assert_eq!(hls_content_type("../video.ts"), None);
        assert_eq!(hls_content_type("segment_bad.ts"), None);
    }

    #[test]
    fn hls_transcode_command_uses_fixed_bounded_segment_paths() {
        let directory = Path::new("/private/cache/session-id");
        let command = hls_transcode_command(
            Path::new("/usr/bin/ffmpeg"),
            Path::new("/media/video with spaces.avi"),
            8.25,
            directory,
        );
        let arguments = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(arguments.windows(2).any(|pair| pair == ["-ss", "8.250"]));
        assert!(arguments
            .windows(2)
            .any(|pair| pair == ["-i", "/media/video with spaces.avi"]));
        assert!(arguments.windows(2).any(|pair| pair == ["-f", "hls"]));
        assert!(arguments.windows(2).any(|pair| {
            pair == [
                "-hls_segment_filename",
                "/private/cache/session-id/segment_%06d.ts",
            ]
        }));
        assert_eq!(
            arguments.last().map(String::as_str),
            Some("/private/cache/session-id/index.m3u8"),
        );
    }

    #[test]
    fn releasing_playback_cancels_and_removes_hls_session_files() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let state = MediaServerState::unavailable(database);
        let item_id = Uuid::new_v4().to_string();
        let stream_id = Uuid::new_v4().to_string();
        let stream_directory = directory.path().join(&stream_id);
        std::fs::create_dir_all(&stream_directory).expect("stream directory");
        std::fs::write(stream_directory.join("segment_000000.ts"), b"segment")
            .expect("segment fixture");
        let cancellation = CancellationToken::new();
        let now = Instant::now();
        state.hls_transcodes.write().expect("HLS lock").insert(
            stream_id,
            HlsTranscodeAuthorization {
                item_id: item_id.clone(),
                start_seconds: 0.0,
                directory: stream_directory.clone(),
                cancellation: cancellation.clone(),
                started: false,
                created_at: now,
                last_access: now,
            },
        );

        state.release_playback(&item_id).expect("release playback");

        assert!(cancellation.is_cancelled());
        assert!(!stream_directory.exists());
        assert!(state.hls_transcodes.read().expect("HLS lock").is_empty());
    }

    #[test]
    fn startup_cleanup_removes_only_uuid_hls_session_directories() {
        let directory = tempdir().expect("temporary directory");
        let root = directory.path().join("live-hls");
        let orphan = root.join(Uuid::new_v4().to_string());
        let unrelated = root.join("keep-me");
        std::fs::create_dir_all(&orphan).expect("orphan directory");
        std::fs::create_dir_all(&unrelated).expect("unrelated directory");

        remove_orphaned_hls_sessions(directory.path());

        assert!(!orphan.exists());
        assert!(unrelated.exists());
    }

    #[test]
    fn live_transcode_command_uses_a_fixed_fragmented_mp4_argument_array() {
        let source = Path::new("/media/video with spaces.mkv");
        let command = live_transcode_command(Path::new("/usr/bin/ffmpeg"), source, 12.5);
        let arguments = command
            .as_std()
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert!(arguments.windows(2).any(|pair| pair == ["-ss", "12.500"]));
        assert!(arguments
            .windows(2)
            .any(|pair| pair == ["-i", "/media/video with spaces.mkv"]));
        assert!(arguments.windows(2).any(|pair| pair == ["-c:v", "libx264"]));
        assert!(arguments.windows(2).any(|pair| pair == ["-c:a", "aac"]));
        assert!(arguments
            .windows(2)
            .any(|pair| { pair == ["-movflags", "+frag_keyframe+empty_moov+default_base_moof"] }));
        assert_eq!(arguments.last().map(String::as_str), Some("pipe:1"));
    }

    #[tokio::test]
    async fn live_transcode_head_is_authorized_without_starting_ffmpeg() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("video.avi");
        std::fs::write(&source, b"video fixture").expect("video fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let item = video_item(&source, 13);
        database.upsert_library_item(&item).expect("catalog video");
        let state = start(database, directory.path().join("playback-cache")).expect("media server");
        let stream_id = Uuid::new_v4().to_string();
        let cancellation = CancellationToken::new();
        state
            .live_transcodes
            .write()
            .expect("live transcode lock")
            .insert(
                stream_id.clone(),
                LiveTranscodeAuthorization {
                    item_id: item.id.clone(),
                    start_seconds: 4.0,
                    cancellation: cancellation.clone(),
                    started: false,
                    created_at: Instant::now(),
                },
            );
        let url = format!(
            "{}/transcode/{}/{}.mp4?token={}",
            state.base_url.as_deref().expect("server URL"),
            item.id,
            stream_id,
            state.token,
        );

        let response = reqwest::Client::new()
            .head(url)
            .send()
            .await
            .expect("HEAD response");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.headers().get(reqwest::header::CONTENT_TYPE),
            Some(&reqwest::header::HeaderValue::from_static("video/mp4")),
        );
        assert!(
            !state
                .live_transcodes
                .read()
                .expect("live transcode lock")
                .get(&stream_id)
                .expect("authorized stream")
                .started
        );

        state.release_playback(&item.id).expect("release playback");
        assert!(cancellation.is_cancelled());
        assert!(!state
            .live_transcodes
            .read()
            .expect("live transcode lock")
            .contains_key(&stream_id));
    }

    #[tokio::test]
    async fn streams_catalog_images_only_with_the_process_token() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("poster.png");
        let bytes = b"bounded image fixture";
        std::fs::write(&source, bytes).expect("image fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let mut image = video_item(
            &source,
            u64::try_from(bytes.len()).expect("fixture size fits u64"),
        );
        image.media_kind = Some(LibraryMediaKind::Image);
        image.duration_seconds = None;
        image.container = Some("png".to_owned());
        database.upsert_library_item(&image).expect("catalog image");

        let state = start(database, directory.path().join("playback-cache")).expect("media server");
        let session = state.prepare_image_view(&image.id).expect("image session");
        let client = reqwest::Client::new();

        let unauthorized = client
            .get(session.url.split('?').next().expect("image path"))
            .send()
            .await
            .expect("unauthorized response");
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

        let response = client
            .get(session.url)
            .send()
            .await
            .expect("image response");
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.headers().get(reqwest::header::CONTENT_TYPE),
            Some(&reqwest::header::HeaderValue::from_static("image/png"))
        );
        assert_eq!(response.bytes().await.expect("image body").as_ref(), bytes);
    }

    #[tokio::test]
    async fn streams_only_the_requested_video_range() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("video.mp4");
        let bytes = (0..=255u8).cycle().take(4_096).collect::<Vec<_>>();
        std::fs::write(&path, &bytes).expect("video fixture");
        let database = Database::open(&directory.path().join("media.sqlite3")).expect("database");
        let item = video_item(&path, bytes.len() as u64);
        database.upsert_library_item(&item).expect("catalog item");
        let server =
            start(database, directory.path().join("playback-cache")).expect("media server");
        let session = server.prepare_playback(&item.id).expect("playback session");
        assert!(!session.compatibility_mode);
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
