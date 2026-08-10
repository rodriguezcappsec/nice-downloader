use crate::{
    database::Database,
    engines::{
        ytdlp::{sanitize_diagnostic, YtDlpEngine},
        DownloadEngine,
    },
    errors::AppError,
    models::{
        DownloadJob, DownloadOptions, DownloadState, EnginePreferences, StartDownloadRequest,
    },
    utils::validation::{validate_destination, validate_media_url},
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

#[derive(Clone)]
pub struct QueueManager {
    jobs: Arc<RwLock<HashMap<String, DownloadJob>>>,
    cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    semaphore: Arc<Semaphore>,
    database: Database,
}

impl QueueManager {
    pub fn new(database: Database, concurrent_downloads: usize) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(concurrent_downloads.max(1))),
            database,
        }
    }

    pub async fn enqueue(
        &self,
        app: AppHandle,
        request: StartDownloadRequest,
        preferences: EnginePreferences,
    ) -> Result<DownloadJob, AppError> {
        validate_media_url(&request.url)?;
        let destination = validate_destination(&request.options.destination)?;
        let engine = YtDlpEngine::discover()?;
        let capabilities = engine.runtime_capabilities().await;
        let args = engine.build_download_args(
            &request.url,
            &request.options,
            &preferences,
            &capabilities,
        )?;

        let id = Uuid::new_v4().to_string();
        let job = DownloadJob::from_request(id.clone(), &request);
        self.jobs.write().await.insert(id.clone(), job.clone());
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
            options: request.options,
            destination,
            engine,
            args,
            cancellation,
        };
        tokio::spawn(async move {
            queue.run_job(context).await;
        });
        Ok(job)
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
            options,
            destination,
            engine,
            args,
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
            job.current_operation = "Preparing yt-dlp".to_owned();
        })
        .await;

        let mut command = Command::new(engine.executable());
        command
            .args(args)
            .current_dir(destination)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.as_std_mut().process_group(0);

        tracing::info!(job_id = %id, engine = engine.executable_name(), "starting download process");
        let child_result = command.spawn();
        let mut child = match child_result {
            Ok(child) => child,
            Err(error) => {
                self.transition_failed(&app, &id, format!("Could not start yt-dlp: {error}"))
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

        let mut diagnostics = VecDeque::with_capacity(40);
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
                    self.transition_failed(&app, &id, format!("Could not monitor yt-dlp: {error}"))
                        .await;
                    return;
                }
            }

            if let Ok(Some(line)) = timeout(Duration::from_millis(120), line_rx.recv()).await {
                self.process_line(&app, &id, line, &mut diagnostics).await;
            }
        };

        while let Ok(Some(line)) = timeout(Duration::from_millis(80), line_rx.recv()).await {
            self.process_line(&app, &id, line, &mut diagnostics).await;
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
                if let Err(error) = self.database.save_completed(&job, &options) {
                    tracing::error!(job_id = %id, error = %error, "could not persist completed download");
                    self.update_job(&app, &id, |job| {
                        job.error = Some(
                            "The download completed, but it could not be saved to History."
                                .to_owned(),
                        );
                    })
                    .await;
                }
            }
            self.cancellations.lock().await.remove(&id);
            tracing::info!(job_id = %id, "download completed");
        } else {
            let detail = diagnostics
                .back()
                .map(|line| sanitize_diagnostic(line))
                .filter(|line| !line.is_empty())
                .unwrap_or_else(|| "yt-dlp exited without a diagnostic message".to_owned());
            self.transition_failed(&app, &id, format!("Download failed: {detail}"))
                .await;
        }
    }

    async fn process_line(
        &self,
        app: &AppHandle,
        id: &str,
        line: EngineLine,
        diagnostics: &mut VecDeque<String>,
    ) {
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
            if diagnostics.len() == 40 {
                diagnostics.pop_front();
            }
            diagnostics.push_back(line.text.clone());
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
        Some(job)
    }
}

struct JobRunContext {
    app: AppHandle,
    id: String,
    options: DownloadOptions,
    destination: std::path::PathBuf,
    engine: YtDlpEngine,
    args: Vec<String>,
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
}
