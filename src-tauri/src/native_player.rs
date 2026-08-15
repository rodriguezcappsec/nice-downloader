use crate::{
    database::Database,
    engines::managed,
    errors::AppError,
    models::{
        LibraryMediaKind, NativePlaybackSession, NativePlaybackStatus, SaveLibraryPlaybackRequest,
    },
    utils::process::sanitize_std_command,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const MPV_EXECUTABLE: &str = "mpv";
const MONITOR_INTERVAL: Duration = Duration::from_millis(750);
const IPC_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Clone)]
pub struct NativePlayerManager {
    inner: Arc<NativePlayerInner>,
}

struct NativePlayerInner {
    database: Database,
    active: Mutex<Option<ActivePlayback>>,
}

struct ActivePlayback {
    token: Uuid,
    session: NativePlaybackSession,
    child: Child,
    ipc_path: PathBuf,
    last_position_seconds: f64,
    last_duration_seconds: f64,
}

impl NativePlayerManager {
    pub fn new(database: Database) -> Self {
        Self {
            inner: Arc::new(NativePlayerInner {
                database,
                active: Mutex::new(None),
            }),
        }
    }

    pub async fn status(&self) -> NativePlaybackStatus {
        let active = self.active_session();
        let Some(executable) = resolve_mpv() else {
            return NativePlaybackStatus {
                available: false,
                version: None,
                active,
                detail: native_player_unavailable_message(),
            };
        };
        let version = read_mpv_version(&executable).await;
        NativePlaybackStatus {
            available: version.is_some(),
            version,
            active,
            detail: "Native mpv playback uses FFmpeg decoders and does not create a compatibility copy. On Wayland, mpv opens a separate app-owned playback window.".to_owned(),
        }
    }

    pub fn open(
        &self,
        item_id: &str,
        start_at_seconds: Option<f64>,
    ) -> Result<NativePlaybackSession, AppError> {
        let executable = resolve_mpv().ok_or_else(|| {
            AppError::new(
                "native_player_unavailable",
                native_player_unavailable_message(),
            )
        })?;
        let item = self.inner.database.get_library_item(item_id)?;
        if item.missing {
            return Err(AppError::new(
                "library_file_not_found",
                "This video is no longer available at its indexed location.",
            ));
        }
        if !matches!(
            item.media_kind,
            Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
        ) {
            return Err(AppError::new(
                "native_player_video_required",
                "Native playback is available only for cataloged videos.",
            ));
        }
        let path = PathBuf::from(&item.path);
        if !path.is_absolute() || !path.is_file() {
            return Err(AppError::new(
                "library_file_not_found",
                "This video is no longer available at its indexed location.",
            ));
        }

        self.stop_active();
        let duration_seconds = item.duration_seconds.unwrap_or(0.0).max(0.0);
        if start_at_seconds.is_some_and(|seconds| !seconds.is_finite() || seconds < 0.0) {
            return Err(AppError::new(
                "invalid_native_playback_start",
                "The requested native playback timestamp is invalid.",
            ));
        }
        let saved = self.inner.database.get_library_playback(item_id)?;
        let resumed_at_seconds = start_at_seconds
            .map(|seconds| {
                if duration_seconds > 0.0 {
                    seconds.min(duration_seconds)
                } else {
                    seconds
                }
            })
            .or_else(|| {
                saved
                    .as_ref()
                    .filter(|state| !state.completed && state.position_seconds >= 5.0)
                    .map(|state| state.position_seconds)
            })
            .unwrap_or(0.0);
        let ipc_path = native_ipc_path();
        let _ = fs::remove_file(&ipc_path);
        let args = mpv_arguments(&path, &item.title, resumed_at_seconds, &ipc_path);
        let mut command = Command::new(executable);
        sanitize_std_command(&mut command);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "linux")]
        set_parent_death_signal(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| AppError::internal("could not start native mpv playback", error))?;
        wait_for_mpv_ready(&mut child, &ipc_path)?;
        let session = NativePlaybackSession {
            item_id: item.id,
            title: item.title,
            resumed_at_seconds,
            duration_seconds,
        };
        let token = Uuid::new_v4();
        {
            let mut active = self.inner.active.lock().map_err(|_| {
                AppError::new(
                    "native_player_unavailable",
                    "Native playback state is unavailable.",
                )
            })?;
            *active = Some(ActivePlayback {
                token,
                session: session.clone(),
                child,
                ipc_path,
                last_position_seconds: resumed_at_seconds,
                last_duration_seconds: duration_seconds,
            });
        }
        self.spawn_monitor(token);
        Ok(session)
    }

    pub fn close(&self) {
        self.stop_active();
    }

    fn active_session(&self) -> Option<NativePlaybackSession> {
        self.inner
            .active
            .lock()
            .ok()
            .and_then(|active| active.as_ref().map(|active| active.session.clone()))
    }

    fn spawn_monitor(&self, token: Uuid) {
        let manager = self.clone();
        thread::spawn(move || loop {
            thread::sleep(MONITOR_INTERVAL);
            let ipc_path = {
                let Ok(active) = manager.inner.active.lock() else {
                    return;
                };
                let Some(active) = active.as_ref().filter(|active| active.token == token) else {
                    return;
                };
                active.ipc_path.clone()
            };
            let progress = query_progress(&ipc_path);
            let finished = {
                let Ok(mut active) = manager.inner.active.lock() else {
                    return;
                };
                let Some(active) = active.as_mut().filter(|active| active.token == token) else {
                    return;
                };
                if let Some((position, duration)) = progress {
                    active.last_position_seconds = position.max(0.0);
                    if duration.is_finite() && duration > 0.0 {
                        active.last_duration_seconds = duration;
                    }
                }
                !matches!(active.child.try_wait(), Ok(None))
            };
            if finished {
                manager.finish_active(token);
                return;
            }
        });
    }

    fn finish_active(&self, token: Uuid) {
        let finished = self.inner.active.lock().ok().and_then(|mut active| {
            if active.as_ref().is_some_and(|active| active.token == token) {
                active.take()
            } else {
                None
            }
        });
        if let Some(active) = finished {
            self.persist_progress(&active);
            let _ = fs::remove_file(active.ipc_path);
        }
    }

    fn stop_active(&self) {
        let mut active = self
            .inner
            .active
            .lock()
            .ok()
            .and_then(|mut active| active.take());
        let Some(mut active) = active.take() else {
            return;
        };
        if let Some((position, duration)) = query_progress(&active.ipc_path) {
            active.last_position_seconds = position.max(0.0);
            if duration.is_finite() && duration > 0.0 {
                active.last_duration_seconds = duration;
            }
        }
        send_quit(&active.ipc_path);
        thread::sleep(Duration::from_millis(100));
        if matches!(active.child.try_wait(), Ok(None)) {
            let _ = active.child.kill();
        }
        let _ = active.child.wait();
        self.persist_progress(&active);
        let _ = fs::remove_file(active.ipc_path);
    }

    fn persist_progress(&self, active: &ActivePlayback) {
        let duration = active.last_duration_seconds;
        let position = active.last_position_seconds.min(duration.max(0.0));
        if !position.is_finite() || !duration.is_finite() || position < 0.0 || duration <= 0.0 {
            return;
        }
        let completed = duration - position <= 10.0 || position / duration >= 0.95;
        if let Err(error) = self
            .inner
            .database
            .save_library_playback(SaveLibraryPlaybackRequest {
                item_id: active.session.item_id.clone(),
                position_seconds: position,
                duration_seconds: duration,
                completed,
            })
        {
            tracing::warn!(%error, item_id = %active.session.item_id, "could not persist native playback progress");
        }
    }
}

fn resolve_mpv() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        managed::resolve(MPV_EXECUTABLE)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

async fn read_mpv_version(executable: &Path) -> Option<String> {
    let mut command = tokio::process::Command::new(executable);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command.kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(3), command.arg("--version").output())
        .await
        .ok()?
        .ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        String::from_utf8_lossy(&output.stdout)
    };
    text.lines()
        .map(str::trim)
        .find(|line| line.starts_with("mpv "))
        .map(|line| line.chars().take(120).collect())
}

fn native_player_unavailable_message() -> String {
    "Native playback requires mpv (Arch Linux: mpv; Debian/Ubuntu: mpv). The embedded WebView and H.264 compatibility-copy fallback remain available.".to_owned()
}

fn native_ipc_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "nice-downloader-mpv-{}-{}.sock",
        std::process::id(),
        Uuid::new_v4()
    ))
}

fn wait_for_mpv_ready(child: &mut Child, ipc_path: &Path) -> Result<(), AppError> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => {
                let _ = fs::remove_file(ipc_path);
                return Err(AppError::new(
                    "native_player_start_failed",
                    "mpv closed before native playback became ready. The embedded and system-player options remain available.",
                ));
            }
            Ok(None) if ipc_path.exists() => return Ok(()),
            Ok(None) => thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = fs::remove_file(ipc_path);
                return Err(AppError::internal(
                    "could not inspect native mpv startup",
                    error,
                ));
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    let _ = fs::remove_file(ipc_path);
    Err(AppError::new(
        "native_player_start_timeout",
        "mpv did not become ready in time. The embedded and system-player options remain available.",
    ))
}

fn mpv_arguments(path: &Path, title: &str, start_seconds: f64, ipc_path: &Path) -> Vec<String> {
    let title: String = title
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect();
    let mut args = vec![
        "--no-config".to_owned(),
        "--terminal=no".to_owned(),
        "--idle=no".to_owned(),
        "--keep-open=no".to_owned(),
        "--force-window=yes".to_owned(),
        "--input-default-bindings=yes".to_owned(),
        "--input-vo-keyboard=yes".to_owned(),
        "--osc=yes".to_owned(),
        "--ytdl=no".to_owned(),
        "--autofit-larger=90%x90%".to_owned(),
        format!("--title=Nice Downloader — {title}"),
        format!("--input-ipc-server={}", ipc_path.to_string_lossy()),
    ];
    if start_seconds.is_finite() && start_seconds > 0.0 {
        args.push(format!("--start={start_seconds:.3}"));
    }
    args.push("--".to_owned());
    args.push(path.to_string_lossy().into_owned());
    args
}

#[cfg(unix)]
fn query_progress(ipc_path: &Path) -> Option<(f64, f64)> {
    let position = query_number_property(ipc_path, "time-pos", 1)?;
    let duration = query_number_property(ipc_path, "duration", 2)?;
    Some((position, duration))
}

#[cfg(not(unix))]
fn query_progress(_ipc_path: &Path) -> Option<(f64, f64)> {
    None
}

#[cfg(unix)]
fn query_number_property(ipc_path: &Path, property: &str, request_id: u64) -> Option<f64> {
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(ipc_path).ok()?;
    let _ = stream.set_read_timeout(Some(IPC_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IPC_TIMEOUT));
    let request = json!({ "command": ["get_property", property], "request_id": request_id });
    writeln!(stream, "{request}").ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    let response: Value = serde_json::from_str(&line).ok()?;
    (response.get("request_id")?.as_u64()? == request_id).then(|| response.get("data")?.as_f64())?
}

#[cfg(unix)]
fn send_quit(ipc_path: &Path) {
    use std::os::unix::net::UnixStream;
    if let Ok(mut stream) = UnixStream::connect(ipc_path) {
        let _ = stream.set_write_timeout(Some(IPC_TIMEOUT));
        let _ = writeln!(stream, "{}", json!({ "command": ["quit"] }));
    }
}

#[cfg(not(unix))]
fn send_quit(_ipc_path: &Path) {}

#[cfg(target_os = "linux")]
fn set_parent_death_signal(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        command.pre_exec(|| {
            if nix::libc::prctl(nix::libc::PR_SET_PDEATHSIG, nix::libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_mpv_arguments_without_shell_interpolation() {
        let path = Path::new("/tmp/video;$(touch nope).mkv");
        let args = mpv_arguments(
            path,
            "Title\nwith controls",
            12.3456,
            Path::new("/tmp/player.sock"),
        );
        assert!(args.contains(&"--no-config".to_owned()));
        assert!(args.contains(&"--start=12.346".to_owned()));
        assert!(args.contains(&"--title=Nice Downloader — Titlewith controls".to_owned()));
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(
            args.last().map(String::as_str),
            Some("/tmp/video;$(touch nope).mkv")
        );
    }

    #[test]
    fn supports_transcript_timestamps_and_omits_invalid_positions() {
        let transcript_args = mpv_arguments(
            Path::new("/tmp/video.mp4"),
            "Video",
            1.25,
            Path::new("/tmp/player.sock"),
        );
        assert!(transcript_args.contains(&"--start=1.250".to_owned()));

        for position in [0.0, f64::NAN, f64::INFINITY] {
            let args = mpv_arguments(
                Path::new("/tmp/video.mp4"),
                "Video",
                position,
                Path::new("/tmp/player.sock"),
            );
            assert!(!args.iter().any(|argument| argument.starts_with("--start=")));
        }
    }
}
