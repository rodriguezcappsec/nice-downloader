use crate::{
    database::Database,
    engines::{
        gallery_dl::parse_human_bytes,
        router::{self, PreparedDownload, PreparedEngineKind},
        ytdlp::sanitize_diagnostic,
    },
    errors::AppError,
    models::{DownloadJob, DownloadState, EnginePreferences, StartDownloadRequest},
    utils::validation::{
        validate_destination, validate_destination_writable, validate_engine_preferences,
        validate_media_url,
    },
};
use chrono::Utc;
use std::{
    cmp::Reverse,
    collections::{HashMap, VecDeque},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, Mutex, RwLock, Semaphore},
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const JOB_EVENT: &str = "download-job-updated";
const MAX_JOB_DIAGNOSTIC_LINES: usize = 80;

#[derive(Clone)]
pub struct QueueManager {
    jobs: Arc<RwLock<HashMap<String, DownloadJob>>>,
    requests: Arc<RwLock<HashMap<String, StartDownloadRequest>>>,
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    pending_recovery: Arc<Mutex<Vec<String>>>,
    semaphore: Arc<Semaphore>,
    database: Database,
}

impl QueueManager {
    pub fn new(database: Database, concurrent_downloads: usize) -> Result<Self, AppError> {
        let persisted = database.load_runtime_jobs()?;
        let mut jobs = HashMap::new();
        let mut requests = HashMap::new();
        let mut pending_recovery = Vec::new();
        for (mut job, request) in persisted {
            if !is_terminal_state(job.state) {
                job.state = DownloadState::Queued;
                job.progress_percent = 0.0;
                job.speed_bytes_per_second = None;
                job.eta_seconds = None;
                job.error = None;
                job.current_operation = "Recovered after application restart".to_owned();
                job.updated_at = Utc::now();
                database.save_job(&job, &request)?;
                pending_recovery.push(job.id.clone());
            }
            requests.insert(job.id.clone(), request);
            jobs.insert(job.id.clone(), job);
        }
        Ok(Self {
            jobs: Arc::new(RwLock::new(jobs)),
            requests: Arc::new(RwLock::new(requests)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            pending_recovery: Arc::new(Mutex::new(pending_recovery)),
            semaphore: Arc::new(Semaphore::new(concurrent_downloads.max(1))),
            database,
        })
    }

    pub async fn enqueue(
        &self,
        app: AppHandle,
        request: StartDownloadRequest,
        preferences: EnginePreferences,
    ) -> Result<DownloadJob, AppError> {
        self.enqueue_with_lineage(app, request, preferences, 1, None)
            .await
    }

    pub async fn recover_pending(&self, app: AppHandle, preferences: EnginePreferences) {
        let ids = {
            let mut pending = self.pending_recovery.lock().await;
            std::mem::take(&mut *pending)
        };
        for id in ids {
            let request = self.requests.read().await.get(&id).cloned();
            let Some(request) = request else {
                self.transition_failed(
                    &app,
                    &id,
                    "The persisted download request could not be recovered.".to_owned(),
                )
                .await;
                continue;
            };
            if let Err(error) = self
                .start_existing(app.clone(), id.clone(), request, preferences.clone())
                .await
            {
                self.transition_failed(&app, &id, error.message).await;
            }
        }
    }

    pub async fn retry(
        &self,
        app: AppHandle,
        job_id: &str,
        preferences: EnginePreferences,
    ) -> Result<DownloadJob, AppError> {
        let previous = self
            .jobs
            .read()
            .await
            .get(job_id)
            .cloned()
            .ok_or_else(|| AppError::new("job_not_found", "The download job was not found."))?;
        if !matches!(
            previous.state,
            DownloadState::Failed | DownloadState::Cancelled
        ) {
            return Err(AppError::new(
                "job_not_retryable",
                "Only failed or cancelled downloads can be retried.",
            ));
        }
        let request = self
            .requests
            .read()
            .await
            .get(job_id)
            .cloned()
            .ok_or_else(|| {
                AppError::new(
                    "retry_unavailable",
                    "The original download options are no longer available.",
                )
            })?;
        self.enqueue_with_lineage(
            app,
            request,
            preferences,
            previous.attempt_count.saturating_add(1),
            Some(previous.id),
        )
        .await
    }

    async fn enqueue_with_lineage(
        &self,
        app: AppHandle,
        request: StartDownloadRequest,
        preferences: EnginePreferences,
        attempt_count: u32,
        retry_of_id: Option<String>,
    ) -> Result<DownloadJob, AppError> {
        validate_media_url(&request.url)?;
        validate_engine_preferences(&preferences)?;
        let destination = validate_destination(&request.options.destination)?;
        validate_destination_writable(&destination)?;
        let prepared = router::prepare_download(&request, &preferences).await?;

        let id = Uuid::new_v4().to_string();
        let mut job = DownloadJob::from_request(id.clone(), &request);
        job.attempt_count = attempt_count.max(1);
        job.retry_of_id = retry_of_id;
        self.database.save_job(&job, &request)?;
        self.jobs.write().await.insert(id.clone(), job.clone());
        self.requests
            .write()
            .await
            .insert(id.clone(), request.clone());
        let cancellation = CancellationToken::new();
        self.cancellations
            .lock()
            .await
            .insert(id.clone(), cancellation.clone());
        emit_job(&app, &job);

        let queue = self.clone();
        let context = JobRunContext {
            app,
            id,
            destination,
            prepared,
            cancellation,
        };
        tokio::spawn(async move {
            queue.run_job(context).await;
        });
        Ok(job)
    }

    async fn start_existing(
        &self,
        app: AppHandle,
        id: String,
        request: StartDownloadRequest,
        preferences: EnginePreferences,
    ) -> Result<(), AppError> {
        validate_media_url(&request.url)?;
        validate_engine_preferences(&preferences)?;
        let destination = validate_destination(&request.options.destination)?;
        validate_destination_writable(&destination)?;
        let prepared = router::prepare_download(&request, &preferences).await?;
        let cancellation = CancellationToken::new();
        self.cancellations
            .lock()
            .await
            .insert(id.clone(), cancellation.clone());
        if let Some(job) = self.jobs.read().await.get(&id).cloned() {
            emit_job(&app, &job);
        }
        let queue = self.clone();
        tokio::spawn(async move {
            queue
                .run_job(JobRunContext {
                    app,
                    id,
                    destination,
                    prepared,
                    cancellation,
                })
                .await;
        });
        Ok(())
    }

    pub async fn cancel(&self, job_id: &str) -> Result<(), AppError> {
        let token = self.cancellations.lock().await.get(job_id).cloned();
        if let Some(token) = token {
            token.cancel();
            return Ok(());
        }
        let jobs = self.jobs.read().await;
        if jobs.contains_key(job_id) {
            Err(AppError::new(
                "job_not_cancellable",
                "This download has already finished.",
            ))
        } else {
            Err(AppError::new(
                "job_not_found",
                "The download job was not found.",
            ))
        }
    }

    pub async fn list(&self) -> Vec<DownloadJob> {
        let mut jobs: Vec<_> = self.jobs.read().await.values().cloned().collect();
        jobs.sort_by_key(|job| Reverse(job.created_at));
        jobs
    }

    async fn run_job(&self, context: JobRunContext) {
        let JobRunContext {
            app,
            id,
            destination,
            prepared,
            cancellation,
        } = context;
        let permit = tokio::select! {
            permit = self.semaphore.clone().acquire_owned() => permit.ok(),
            _ = cancellation.cancelled() => None,
        };
        let Some(_permit) = permit else {
            self.transition_cancelled(&app, &id).await;
            return;
        };
        if cancellation.is_cancelled() {
            self.transition_cancelled(&app, &id).await;
            return;
        }

        self.update_job(&app, &id, |job| {
            job.state = DownloadState::Preparing;
            job.current_operation = format!("Preparing {}", prepared.engine_name);
            if let Some(relative_path) = &prepared.output_relative_path {
                job.output_path = Some(
                    destination
                        .join(relative_path)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        })
        .await;

        let mut command = Command::new(&prepared.executable);
        command
            .args(&prepared.args)
            .current_dir(destination)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.as_std_mut().process_group(0);

        tracing::info!(job_id = %id, engine = prepared.engine_name, "starting download process");
        let child_result = command.spawn();
        let mut child = match child_result {
            Ok(child) => child,
            Err(error) => {
                self.transition_failed(
                    &app,
                    &id,
                    format!("Could not start {}: {error}", prepared.engine_name),
                )
                .await;
                return;
            }
        };

        let (line_tx, mut line_rx) = mpsc::channel::<EngineLine>(256);
        if let Some(stdout) = child.stdout.take() {
            spawn_line_reader(StreamSource::Stdout, stdout, line_tx.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_line_reader(StreamSource::Stderr, stderr, line_tx.clone());
        }
        drop(line_tx);

        let mut diagnostics = VecDeque::with_capacity(MAX_JOB_DIAGNOSTIC_LINES);
        let mut gallery_progress = GalleryProgressTracker::default();
        let exit_status = loop {
            if cancellation.is_cancelled() {
                tracing::info!(job_id = %id, "cancelling download process group");
                terminate_child(&mut child).await;
                self.transition_cancelled(&app, &id).await;
                return;
            }

            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {}
                Err(error) => {
                    self.transition_failed(
                        &app,
                        &id,
                        format!("Could not monitor {}: {error}", prepared.engine_name),
                    )
                    .await;
                    return;
                }
            }

            if let Ok(Some(line)) = timeout(Duration::from_millis(120), line_rx.recv()).await {
                self.process_line(
                    &app,
                    &id,
                    prepared.kind,
                    line,
                    &mut diagnostics,
                    &mut gallery_progress,
                )
                .await;
            }
        };

        while let Ok(Some(line)) = timeout(Duration::from_millis(80), line_rx.recv()).await {
            self.process_line(
                &app,
                &id,
                prepared.kind,
                line,
                &mut diagnostics,
                &mut gallery_progress,
            )
            .await;
        }

        if exit_status.is_some_and(|status| status.success()) {
            let completed = self
                .update_job(&app, &id, |job| {
                    job.state = DownloadState::Completed;
                    job.progress_percent = 100.0;
                    job.speed_bytes_per_second = None;
                    job.eta_seconds = Some(0);
                    job.current_operation = "Download completed".to_owned();
                    job.completed_at = Some(Utc::now());
                })
                .await;
            if let Some(job) = completed {
                self.persist_job(&job).await;
            }
            self.cancellations.lock().await.remove(&id);
            tracing::info!(job_id = %id, "download completed");
        } else {
            let detail = diagnostics
                .back()
                .map(|line| sanitize_diagnostic(line))
                .filter(|line| !line.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "{} exited without a diagnostic message",
                        prepared.engine_name
                    )
                });
            self.transition_failed(&app, &id, format!("Download failed: {detail}"))
                .await;
        }
    }

    async fn process_line(
        &self,
        app: &AppHandle,
        id: &str,
        engine_kind: PreparedEngineKind,
        line: EngineLine,
        diagnostics: &mut VecDeque<String>,
        gallery_progress: &mut GalleryProgressTracker,
    ) {
        if engine_kind == PreparedEngineKind::GalleryDl
            && self
                .process_gallery_line(app, id, &line.text, gallery_progress)
                .await
        {
            return;
        }
        if let Some(progress) = line
            .text
            .strip_prefix("nicedl:progress:")
            .and_then(parse_progress)
        {
            self.update_job(app, id, |job| {
                job.state = DownloadState::Downloading;
                job.current_operation = "Downloading media".to_owned();
                job.progress_percent = progress.percent;
                job.downloaded_bytes = progress.downloaded_bytes;
                job.total_bytes = progress.total_bytes;
                job.speed_bytes_per_second = progress.speed;
                job.eta_seconds = progress.eta;
            })
            .await;
            return;
        }
        if line.text.starts_with("nicedl:postprocess:") {
            self.update_job(app, id, |job| {
                job.state = DownloadState::PostProcessing;
                job.current_operation = "Post-processing media".to_owned();
            })
            .await;
            return;
        }
        if let Some(path) = line.text.strip_prefix("nicedl:output:") {
            let path = path.trim();
            if !path.is_empty() {
                self.update_job(app, id, |job| job.output_path = Some(path.to_owned()))
                    .await;
            }
            return;
        }

        if line.source == StreamSource::Stderr {
            let diagnostic = sanitize_diagnostic(&line.text);
            if diagnostics.len() == MAX_JOB_DIAGNOSTIC_LINES {
                diagnostics.pop_front();
            }
            diagnostics.push_back(diagnostic.clone());
            self.update_job(app, id, |job| {
                if job.diagnostics.len() == MAX_JOB_DIAGNOSTIC_LINES {
                    job.diagnostics.remove(0);
                }
                job.diagnostics.push(diagnostic);
            })
            .await;
        }

        let operation = operation_from_engine_line(&line.text);
        if let Some((state, message)) = operation {
            self.update_job(app, id, |job| {
                job.state = state;
                job.current_operation = message.to_owned();
                job.speed_bytes_per_second = None;
                job.eta_seconds = None;
            })
            .await;
        }
    }

    async fn process_gallery_line(
        &self,
        app: &AppHandle,
        id: &str,
        text: &str,
        tracker: &mut GalleryProgressTracker,
    ) -> bool {
        let text = text.trim_start_matches(['\r', ' ']);
        let Some(event) = text.strip_prefix("nicedl:gallery:") else {
            return false;
        };
        if let Some(value) = event.strip_prefix("item:") {
            let mut fields = value.splitn(4, '|');
            let (Some(index), Some(count), Some(size), Some(filename)) = (
                fields.next().and_then(|value| value.parse::<u32>().ok()),
                fields.next().and_then(|value| value.parse::<u32>().ok()),
                fields.next().and_then(|value| value.parse::<u64>().ok()),
                fields.next(),
            ) else {
                return true;
            };
            tracker.current_index = index;
            tracker.item_count = count;
            tracker.current_size = size;
            tracker.current_name = filename.chars().take(180).collect();
            self.update_job(app, id, |job| {
                job.state = DownloadState::Downloading;
                job.current_operation = format!(
                    "Downloading item {index} of {count}: {}",
                    tracker.current_name
                );
                job.downloaded_bytes = Some(tracker.completed_bytes);
                job.progress_percent =
                    gallery_album_percent(tracker, 0.0, job.total_bytes, tracker.completed_bytes);
                job.speed_bytes_per_second = None;
                job.eta_seconds = None;
            })
            .await;
            return true;
        }
        if let Some(value) = event.strip_prefix("progress:") {
            let Some(progress) = parse_gallery_progress(value) else {
                return true;
            };
            let current_downloaded = progress.downloaded_bytes.unwrap_or_else(|| {
                ((tracker.current_size as f64) * progress.item_percent / 100.0) as u64
            });
            let current_downloaded = if tracker.current_size > 0 {
                current_downloaded.min(tracker.current_size)
            } else {
                current_downloaded
            };
            let downloaded = tracker.completed_bytes.saturating_add(current_downloaded);
            self.update_job(app, id, |job| {
                job.state = DownloadState::Downloading;
                job.current_operation = format!(
                    "Downloading item {} of {}: {}",
                    tracker.current_index, tracker.item_count, tracker.current_name
                );
                job.progress_percent = gallery_album_percent(
                    tracker,
                    progress.item_percent,
                    job.total_bytes,
                    downloaded,
                );
                job.downloaded_bytes = Some(downloaded);
                job.speed_bytes_per_second = progress.speed;
                job.eta_seconds = progress.speed.and_then(|speed| {
                    job.total_bytes.filter(|_| speed > 0.0).map(|total| {
                        (total.saturating_sub(downloaded) as f64 / speed).ceil() as u64
                    })
                });
            })
            .await;
            return true;
        }
        for prefix in ["complete:", "skip:"] {
            if let Some(value) = event.strip_prefix(prefix) {
                let mut fields = value.splitn(4, '|');
                let index = fields.next().and_then(|value| value.parse::<u32>().ok());
                let count = fields.next().and_then(|value| value.parse::<u32>().ok());
                let size = fields.next().and_then(|value| value.parse::<u64>().ok());
                if let (Some(index), Some(count), Some(size)) = (index, count, size) {
                    if tracker.last_completed_index != Some(index) {
                        tracker.completed_bytes = tracker.completed_bytes.saturating_add(size);
                        tracker.last_completed_index = Some(index);
                    }
                    tracker.current_index = index;
                    tracker.item_count = count;
                    self.update_job(app, id, |job| {
                        job.state = DownloadState::Downloading;
                        job.current_operation = format!("Processed item {index} of {count}");
                        job.downloaded_bytes = Some(tracker.completed_bytes);
                        job.progress_percent = gallery_album_percent(
                            tracker,
                            100.0,
                            job.total_bytes,
                            tracker.completed_bytes,
                        );
                        job.speed_bytes_per_second = None;
                        job.eta_seconds = None;
                    })
                    .await;
                }
                return true;
            }
        }
        // Custom output lines can contain paths, but never belong in diagnostics.
        true
    }

    async fn transition_cancelled(&self, app: &AppHandle, id: &str) {
        self.update_job(app, id, |job| {
            job.state = DownloadState::Cancelled;
            job.current_operation = "Download cancelled".to_owned();
            job.speed_bytes_per_second = None;
            job.eta_seconds = None;
        })
        .await;
        self.cancellations.lock().await.remove(id);
    }

    async fn transition_failed(&self, app: &AppHandle, id: &str, error: String) {
        self.update_job(app, id, |job| {
            job.state = DownloadState::Failed;
            job.current_operation = "Download failed".to_owned();
            job.error = Some(error);
            job.speed_bytes_per_second = None;
            job.eta_seconds = None;
        })
        .await;
        self.cancellations.lock().await.remove(id);
        tracing::warn!(job_id = %id, "download failed");
    }

    async fn update_job<F>(&self, app: &AppHandle, id: &str, update: F) -> Option<DownloadJob>
    where
        F: FnOnce(&mut DownloadJob),
    {
        let job = {
            let mut jobs = self.jobs.write().await;
            let job = jobs.get_mut(id)?;
            update(job);
            job.updated_at = Utc::now();
            job.clone()
        };
        emit_job(app, &job);
        self.persist_job(&job).await;
        Some(job)
    }

    async fn persist_job(&self, job: &DownloadJob) {
        let request = self.requests.read().await.get(&job.id).cloned();
        if let Some(request) = request {
            if let Err(error) = self.database.save_job(job, &request) {
                tracing::error!(job_id = %job.id, %error, "could not persist download job");
            }
        }
    }
}

struct JobRunContext {
    app: AppHandle,
    id: String,
    destination: std::path::PathBuf,
    prepared: PreparedDownload,
    cancellation: CancellationToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StreamSource {
    Stdout,
    Stderr,
}

struct EngineLine {
    source: StreamSource,
    text: String,
}

fn spawn_line_reader<R>(source: StreamSource, reader: R, sender: mpsc::Sender<EngineLine>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(text)) => {
                    if sender.send(EngineLine { source, text }).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::debug!(%error, "engine output stream ended with an error");
                    break;
                }
            }
        }
    });
}

#[derive(Debug, PartialEq)]
struct ParsedProgress {
    percent: f64,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    speed: Option<f64>,
    eta: Option<u64>,
}

#[derive(Debug, Default)]
struct GalleryProgressTracker {
    current_index: u32,
    item_count: u32,
    current_size: u64,
    current_name: String,
    completed_bytes: u64,
    last_completed_index: Option<u32>,
}

#[derive(Debug, PartialEq)]
struct ParsedGalleryProgress {
    item_percent: f64,
    downloaded_bytes: Option<u64>,
    speed: Option<f64>,
}

fn parse_gallery_progress(value: &str) -> Option<ParsedGalleryProgress> {
    let mut fields = value.split('|');
    let item_percent = fields
        .next()?
        .trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?
        .clamp(0.0, 100.0);
    Some(ParsedGalleryProgress {
        item_percent,
        downloaded_bytes: fields.next().and_then(parse_human_bytes),
        speed: fields
            .next()
            .and_then(parse_human_bytes)
            .map(|value| value as f64),
    })
}

fn gallery_album_percent(
    tracker: &GalleryProgressTracker,
    item_percent: f64,
    total_bytes: Option<u64>,
    downloaded_bytes: u64,
) -> f64 {
    if let Some(total) = total_bytes.filter(|total| *total > 0) {
        return (downloaded_bytes as f64 * 100.0 / total as f64).clamp(0.0, 100.0);
    }
    if tracker.item_count == 0 || tracker.current_index == 0 {
        return 0.0;
    }
    (((tracker.current_index - 1) as f64 + item_percent / 100.0) * 100.0
        / tracker.item_count as f64)
        .clamp(0.0, 100.0)
}

fn parse_progress(value: &str) -> Option<ParsedProgress> {
    let mut fields = value.split('|');
    let percent = fields
        .next()?
        .trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .ok()?;
    Some(ParsedProgress {
        percent: percent.clamp(0.0, 100.0),
        downloaded_bytes: parse_optional_u64(fields.next()),
        total_bytes: parse_optional_u64(fields.next()),
        speed: parse_optional_f64(fields.next()),
        eta: parse_optional_u64(fields.next()),
    })
}

fn parse_optional_u64(value: Option<&str>) -> Option<u64> {
    let parsed = parse_optional_f64(value)?;
    if parsed < 0.0 || parsed > u64::MAX as f64 {
        None
    } else {
        Some(parsed.round() as u64)
    }
}

fn parse_optional_f64(value: Option<&str>) -> Option<f64> {
    let value = value?.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("na") || value.eq_ignore_ascii_case("none") {
        return None;
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite())
}

fn operation_from_engine_line(line: &str) -> Option<(DownloadState, &'static str)> {
    if line.starts_with("[Merger]") {
        Some((DownloadState::Merging, "Merging audio and video"))
    } else if line.starts_with("[ExtractAudio]") {
        Some((DownloadState::PostProcessing, "Extracting audio"))
    } else if line.starts_with("[EmbedSubtitle]") || line.starts_with("[SubtitlesConvertor]") {
        Some((DownloadState::PostProcessing, "Embedding subtitles"))
    } else if line.starts_with("[EmbedThumbnail]") {
        Some((DownloadState::PostProcessing, "Embedding thumbnail"))
    } else if line.starts_with("[Metadata]") {
        Some((DownloadState::PostProcessing, "Embedding metadata"))
    } else if line.starts_with("[VideoConvertor]") || line.starts_with("[Fixup") {
        Some((DownloadState::PostProcessing, "Finalizing media"))
    } else {
        None
    }
}

fn emit_job(app: &AppHandle, job: &DownloadJob) {
    if let Err(error) = app.emit(JOB_EVENT, job) {
        tracing::warn!(job_id = %job.id, %error, "could not emit job update");
    }
}

fn is_terminal_state(state: DownloadState) -> bool {
    matches!(
        state,
        DownloadState::Completed | DownloadState::Failed | DownloadState::Cancelled
    )
}

async fn terminate_child(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Some(raw_pid) = child.id().and_then(|pid| i32::try_from(pid).ok()) {
            let group = nix::unistd::Pid::from_raw(raw_pid);
            let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGTERM);
            if timeout(Duration::from_secs(2), child.wait()).await.is_ok() {
                return;
            }
            let _ = nix::sys::signal::killpg(group, nix::sys::signal::Signal::SIGKILL);
        }
    }
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, MediaAnalysis, OutputConflictPolicy, OutputContainer, QualityKind,
        QualityOption, VideoCodecPreference,
    };
    use tempfile::tempdir;

    #[test]
    fn parses_structured_progress() {
        assert_eq!(
            parse_progress(" 82.1%|438000000|534000000|48200000|3"),
            Some(ParsedProgress {
                percent: 82.1,
                downloaded_bytes: Some(438000000),
                total_bytes: Some(534000000),
                speed: Some(48200000.0),
                eta: Some(3),
            })
        );
    }

    #[test]
    fn tolerates_missing_progress_values() {
        let progress = parse_progress("0.0%|0|NA|NA|NA").expect("progress");
        assert_eq!(progress.total_bytes, None);
        assert_eq!(progress.speed, None);
    }

    #[test]
    fn normalizes_gallery_item_progress_across_an_album() {
        assert_eq!(
            parse_gallery_progress("82|438.00M|48.20M|534.00M"),
            Some(ParsedGalleryProgress {
                item_percent: 82.0,
                downloaded_bytes: Some(438_000_000),
                speed: Some(48_200_000.0),
            })
        );
        let tracker = GalleryProgressTracker {
            current_index: 2,
            item_count: 4,
            ..GalleryProgressTracker::default()
        };
        assert_eq!(gallery_album_percent(&tracker, 50.0, None, 0), 37.5);
        assert_eq!(
            gallery_album_percent(&tracker, 50.0, Some(1_000), 250),
            25.0
        );
    }

    #[tokio::test]
    async fn recovers_interrupted_jobs_as_queued() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("queue.sqlite3")).expect("database");
        let quality = QualityOption {
            id: "best".to_owned(),
            label: "Best".to_owned(),
            description: "Highest available quality".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        };
        let request = StartDownloadRequest {
            url: "https://example.com/video".to_owned(),
            analysis: MediaAnalysis {
                engine: crate::models::DownloadEngineKind::YtDlp,
                title: "Example".to_owned(),
                uploader: None,
                duration_seconds: None,
                thumbnail: None,
                webpage_url: "https://example.com/video".to_owned(),
                download_url: "https://example.com/video".to_owned(),
                extractor: "generic".to_owned(),
                site_name: "Generic".to_owned(),
                playlist_title: None,
                playlist_index: None,
                formats: vec![],
                quality_options: vec![quality.clone()],
                item_count: None,
                total_size_bytes: None,
            },
            options: crate::models::DownloadOptions {
                quality,
                container: OutputContainer::Mp4,
                video_codec_preference: VideoCodecPreference::Any,
                audio_quality: AudioQuality::Best,
                audio_only: false,
                subtitles: false,
                subtitle_languages: vec![],
                embed_thumbnail: false,
                embed_metadata: true,
                chapters: false,
                sponsorblock: false,
                destination: directory.path().to_string_lossy().into_owned(),
                filename_template: "%(title)s.%(ext)s".to_owned(),
                output_conflict_policy: OutputConflictPolicy::Skip,
            },
        };
        let mut job = DownloadJob::from_request("recover-me".to_owned(), &request);
        job.state = DownloadState::Downloading;
        job.progress_percent = 61.0;
        database.save_job(&job, &request).expect("persist job");

        let queue = QueueManager::new(database, 2).expect("recover queue");
        let jobs = queue.list().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].state, DownloadState::Queued);
        assert_eq!(jobs[0].progress_percent, 0.0);
        assert_eq!(
            jobs[0].current_operation,
            "Recovered after application restart"
        );
    }
}
