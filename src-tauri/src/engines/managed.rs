use crate::{errors::AppError, models::EngineId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tokio::{process::Command, time::timeout};
use url::Url;

const MAX_ENGINE_BYTES: usize = 150 * 1024 * 1024;
static MANAGED_ROOT: OnceLock<PathBuf> = OnceLock::new();
static BUNDLED_ROOT: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EngineState {
    active_version: Option<String>,
    previous_version: Option<String>,
    pinned: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedEngineStatus {
    pub engine: EngineId,
    pub supported: bool,
    pub active_version: Option<String>,
    pub previous_version: Option<String>,
    pub pinned: bool,
    pub installed_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineUpdateCandidate {
    pub engine: EngineId,
    pub version: String,
    pub download_url: String,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn initialize(app_data: &Path, resource_dir: &Path) -> Result<(), AppError> {
    let root = app_data.join("engines");
    fs::create_dir_all(&root)
        .map_err(|error| AppError::internal("could not create managed engine storage", error))?;
    let _ = MANAGED_ROOT.set(root);
    let _ = BUNDLED_ROOT.set(resource_dir.join("engines"));
    Ok(())
}

pub fn resolve(executable: &str) -> Option<PathBuf> {
    managed_engine(executable)
        .and_then(|engine| active_path(engine).ok().flatten())
        .or_else(|| bundled_engine(executable))
        .or_else(|| resolve_host_executable(executable))
}

fn bundled_engine(executable: &str) -> Option<PathBuf> {
    find_bundled_executable(BUNDLED_ROOT.get()?, executable)
}

fn find_bundled_executable(root: &Path, executable: &str) -> Option<PathBuf> {
    let candidate = root.join(platform_executable_name(executable));
    candidate.is_file().then_some(candidate)
}

fn resolve_host_executable(executable: &str) -> Option<PathBuf> {
    which::which(executable)
        .ok()
        .or_else(|| find_executable_in_directories(executable, user_executable_directories()))
}

fn find_executable_in_directories(
    executable: &str,
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let executable = platform_executable_name(executable);
    directories
        .into_iter()
        .map(|directory| directory.join(&executable))
        .find(|candidate| candidate.is_file())
}

fn platform_executable_name(executable: &str) -> String {
    if cfg!(windows) && std::path::Path::new(executable).extension().is_none() {
        format!("{executable}.exe")
    } else {
        executable.to_owned()
    }
}

fn user_executable_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        directories.push(home.join(".local/bin"));
        directories.push(home.join("bin"));
        directories.push(home.join(".cargo/bin"));
    }
    #[cfg(target_os = "macos")]
    {
        directories.push(PathBuf::from("/opt/homebrew/bin"));
        directories.push(PathBuf::from("/usr/local/bin"));
    }
    #[cfg(windows)]
    {
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            directories.push(profile.join(".local/bin"));
        }
        if let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) {
            directories.push(app_data.join("Python/Scripts"));
        }
    }
    directories
}

pub fn list_statuses() -> Result<Vec<ManagedEngineStatus>, AppError> {
    [EngineId::YtDlp, EngineId::GalleryDl]
        .into_iter()
        .map(status)
        .collect()
}

pub fn set_pinned(engine: EngineId, pinned: bool) -> Result<ManagedEngineStatus, AppError> {
    ensure_supported(engine)?;
    let mut state = read_state(engine)?;
    state.pinned = pinned;
    write_state(engine, &state)?;
    status(engine)
}

pub fn rollback(engine: EngineId) -> Result<ManagedEngineStatus, AppError> {
    ensure_supported(engine)?;
    let mut state = read_state(engine)?;
    let previous = state.previous_version.clone().ok_or_else(|| {
        AppError::new(
            "rollback_unavailable",
            "No previous managed version is available.",
        )
    })?;
    let previous_path = binary_path(engine, &previous)?;
    if !previous_path.is_file() {
        return Err(AppError::new(
            "rollback_unavailable",
            "The previous managed engine file is no longer available.",
        ));
    }
    state.previous_version = state.active_version.replace(previous);
    write_state(engine, &state)?;
    status(engine)
}

pub async fn check_update(engine: EngineId) -> Result<EngineUpdateCandidate, AppError> {
    ensure_supported(engine)?;
    let (repository, binary_name) = match engine {
        EngineId::YtDlp => ("yt-dlp/yt-dlp", "yt-dlp"),
        EngineId::GalleryDl => ("mikf/gallery-dl", "gallery-dl.bin"),
        _ => unreachable!(),
    };
    let client = reqwest::Client::builder()
        .user_agent("Nice-Downloader/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| AppError::internal("could not initialize update check", error))?;
    let release = client
        .get(format!(
            "https://api.github.com/repos/{repository}/releases/latest"
        ))
        .send()
        .await
        .map_err(|error| AppError::internal("could not check the engine release", error))?
        .error_for_status()
        .map_err(|error| AppError::internal("the engine release service returned an error", error))?
        .json::<GithubRelease>()
        .await
        .map_err(|error| AppError::internal("could not read the engine release", error))?;
    let binary = release
        .assets
        .iter()
        .find(|asset| asset.name == binary_name)
        .ok_or_else(|| {
            AppError::new(
                "update_unavailable",
                "No compatible Linux release asset was found.",
            )
        })?;
    let checksum_asset = release
        .assets
        .iter()
        .find(|asset| {
            let name = asset.name.to_ascii_lowercase();
            name.contains("sha256") || name.contains("sha2-256")
        })
        .ok_or_else(|| {
            AppError::new(
                "checksum_unavailable",
                "The release does not publish a SHA-256 manifest, so Nice Downloader will not install it.",
            )
        })?;
    let checksum_text = client
        .get(&checksum_asset.browser_download_url)
        .send()
        .await
        .map_err(|error| AppError::internal("could not download the checksum manifest", error))?
        .error_for_status()
        .map_err(|error| AppError::internal("the checksum download failed", error))?
        .text()
        .await
        .map_err(|error| AppError::internal("could not read the checksum manifest", error))?;
    let sha256 = checksum_for(&checksum_text, binary_name).ok_or_else(|| {
        AppError::new(
            "checksum_unavailable",
            "The release checksum manifest does not cover the compatible Linux asset.",
        )
    })?;
    Ok(EngineUpdateCandidate {
        engine,
        version: clean_version(&release.tag_name)?,
        download_url: binary.browser_download_url.clone(),
        sha256,
    })
}

pub async fn install(candidate: EngineUpdateCandidate) -> Result<ManagedEngineStatus, AppError> {
    ensure_supported(candidate.engine)?;
    let version = clean_version(&candidate.version)?;
    let expected = clean_sha256(&candidate.sha256)?;
    validate_download_url(&candidate.download_url)?;
    let state = read_state(candidate.engine)?;
    if state.pinned {
        return Err(AppError::new(
            "engine_pinned",
            "Unpin this engine before installing an update.",
        ));
    }
    let response = reqwest::Client::builder()
        .user_agent("Nice-Downloader/0.1")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|error| AppError::internal("could not initialize engine download", error))?
        .get(&candidate.download_url)
        .send()
        .await
        .map_err(|error| AppError::internal("could not download the managed engine", error))?
        .error_for_status()
        .map_err(|error| AppError::internal("the managed engine download failed", error))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_ENGINE_BYTES as u64)
    {
        return Err(AppError::new(
            "engine_too_large",
            "The engine download is unexpectedly large.",
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::internal("could not read the managed engine download", error))?;
    if bytes.len() > MAX_ENGINE_BYTES {
        return Err(AppError::new(
            "engine_too_large",
            "The engine download is unexpectedly large.",
        ));
    }
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != expected {
        return Err(AppError::new(
            "checksum_mismatch",
            "The downloaded engine did not match the published SHA-256 digest and was discarded.",
        ));
    }
    let path = binary_path(candidate.engine, &version)?;
    let directory = path.parent().ok_or_else(|| {
        AppError::new(
            "managed_engine_path",
            "Could not prepare the managed engine path.",
        )
    })?;
    fs::create_dir_all(directory).map_err(|error| {
        AppError::internal("could not create the engine version directory", error)
    })?;
    let temporary = directory.join("download.tmp");
    fs::write(&temporary, &bytes)
        .map_err(|error| AppError::internal("could not store the managed engine", error))?;
    set_executable(&temporary)?;
    let mut version_command = Command::new(&temporary);
    crate::utils::process::sanitize_tokio_command(&mut version_command);
    let version_output = timeout(
        std::time::Duration::from_secs(10),
        version_command.arg("--version").output(),
    )
    .await
    .map_err(|_| {
        AppError::new(
            "engine_validation_failed",
            "The downloaded engine did not start in time.",
        )
    })?
    .map_err(|error| AppError::internal("could not validate the managed engine", error))?;
    if !version_output.status.success() {
        let _ = fs::remove_file(&temporary);
        return Err(AppError::new(
            "engine_validation_failed",
            "The downloaded engine failed its version check and was discarded.",
        ));
    }
    fs::rename(&temporary, &path)
        .map_err(|error| AppError::internal("could not activate the managed engine file", error))?;
    let mut state = state;
    if state.active_version.as_deref() != Some(&version) {
        state.previous_version = state.active_version.replace(version);
    }
    write_state(candidate.engine, &state)?;
    status(candidate.engine)
}

fn status(engine: EngineId) -> Result<ManagedEngineStatus, AppError> {
    ensure_known_engine(engine)?;
    let state = read_state(engine)?;
    let directory = engine_directory(engine)?;
    let mut installed_versions = if directory.is_dir() {
        fs::read_dir(directory)
            .map_err(|error| AppError::internal("could not list managed engine versions", error))?
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    installed_versions.sort();
    installed_versions.reverse();
    Ok(ManagedEngineStatus {
        engine,
        supported: cfg!(target_os = "linux"),
        active_version: state.active_version,
        previous_version: state.previous_version,
        pinned: state.pinned,
        installed_versions,
    })
}

fn active_path(engine: EngineId) -> Result<Option<PathBuf>, AppError> {
    let state = read_state(engine)?;
    Ok(state
        .active_version
        .and_then(|version| binary_path(engine, &version).ok())
        .filter(|path| path.is_file()))
}

fn managed_engine(executable: &str) -> Option<EngineId> {
    match executable {
        "yt-dlp" => Some(EngineId::YtDlp),
        "gallery-dl" => Some(EngineId::GalleryDl),
        _ => None,
    }
}

fn ensure_supported(engine: EngineId) -> Result<(), AppError> {
    ensure_known_engine(engine)?;
    if !cfg!(target_os = "linux") {
        return Err(AppError::new(
            "managed_engine_unsupported_platform",
            "Managed engine installation is currently available on Linux only. Install this engine through the operating system on this platform.",
        ));
    }
    Ok(())
}

fn ensure_known_engine(engine: EngineId) -> Result<(), AppError> {
    if !matches!(engine, EngineId::YtDlp | EngineId::GalleryDl) {
        Err(AppError::new(
            "managed_engine_unsupported",
            "Only yt-dlp and gallery-dl support managed installations right now.",
        ))
    } else {
        Ok(())
    }
}

fn root() -> Result<&'static Path, AppError> {
    MANAGED_ROOT.get().map(PathBuf::as_path).ok_or_else(|| {
        AppError::new(
            "managed_engines_unavailable",
            "Managed engine storage is not initialized.",
        )
    })
}

fn engine_name(engine: EngineId) -> Result<&'static str, AppError> {
    match engine {
        EngineId::YtDlp => Ok("yt-dlp"),
        EngineId::GalleryDl => Ok("gallery-dl"),
        _ => Err(AppError::new(
            "managed_engine_unsupported",
            "This engine cannot be managed.",
        )),
    }
}

fn engine_directory(engine: EngineId) -> Result<PathBuf, AppError> {
    Ok(root()?.join(engine_name(engine)?))
}

fn binary_path(engine: EngineId, version: &str) -> Result<PathBuf, AppError> {
    Ok(engine_directory(engine)?
        .join(version)
        .join(engine_name(engine)?))
}

fn state_path(engine: EngineId) -> Result<PathBuf, AppError> {
    Ok(engine_directory(engine)?.join("state.json"))
}

fn read_state(engine: EngineId) -> Result<EngineState, AppError> {
    let path = state_path(engine)?;
    if !path.exists() {
        return Ok(EngineState::default());
    }
    let value = fs::read(&path)
        .map_err(|error| AppError::internal("could not read managed engine state", error))?;
    serde_json::from_slice(&value)
        .map_err(|error| AppError::internal("managed engine state is invalid", error))
}

fn write_state(engine: EngineId, state: &EngineState) -> Result<(), AppError> {
    let path = state_path(engine)?;
    let directory = path.parent().ok_or_else(|| {
        AppError::new(
            "managed_engine_path",
            "Could not prepare managed engine state.",
        )
    })?;
    fs::create_dir_all(directory)
        .map_err(|error| AppError::internal("could not create managed engine state", error))?;
    let temporary = directory.join("state.tmp");
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| AppError::internal("could not serialize managed engine state", error))?;
    fs::write(&temporary, bytes)
        .map_err(|error| AppError::internal("could not write managed engine state", error))?;
    fs::rename(temporary, path)
        .map_err(|error| AppError::internal("could not activate managed engine state", error))
}

fn clean_version(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches('v');
    if value.is_empty()
        || value.len() > 64
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(AppError::new(
            "invalid_engine_version",
            "The engine version is invalid.",
        ));
    }
    Ok(value.to_owned())
}

fn clean_sha256(value: &str) -> Result<String, AppError> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(AppError::new(
            "invalid_checksum",
            "A valid SHA-256 digest is required.",
        ));
    }
    Ok(value)
}

fn validate_download_url(value: &str) -> Result<(), AppError> {
    let url = Url::parse(value)
        .map_err(|error| AppError::internal("could not parse the engine download URL", error))?;
    if url.scheme() != "https" || url.host_str() != Some("github.com") {
        return Err(AppError::new(
            "untrusted_engine_source",
            "Managed engines can only be installed from verified GitHub release URLs.",
        ));
    }
    Ok(())
}

fn checksum_for(manifest: &str, filename: &str) -> Option<String> {
    manifest
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let digest = fields.next()?;
            let name = fields.next()?.trim_start_matches('*');
            (name == filename).then(|| digest.to_ascii_lowercase())
        })
        .filter(|digest| clean_sha256(digest).is_ok())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| AppError::internal("could not inspect the managed engine", error))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| AppError::internal("could not make the managed engine executable", error))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_executable_in_explicit_user_directory() {
        let root = std::env::temp_dir().join(format!(
            "nice-downloader-engine-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let user_bin = root.join(".local/bin");
        fs::create_dir_all(&user_bin).expect("creates test user bin");
        let executable = user_bin.join(platform_executable_name("gallery-dl"));
        fs::write(&executable, b"test").expect("creates test executable");

        assert_eq!(
            find_executable_in_directories("gallery-dl", [user_bin]),
            Some(executable)
        );

        fs::remove_dir_all(root).expect("removes test directory");
    }

    #[test]
    fn ignores_missing_executable_candidates() {
        let root = std::env::temp_dir().join(format!(
            "nice-downloader-missing-engine-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));

        assert_eq!(find_executable_in_directories("gallery-dl", [root]), None);
    }

    #[test]
    fn resolves_packaged_engine_from_resource_root() {
        let root = std::env::temp_dir().join(format!(
            "nice-downloader-bundled-engine-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        fs::create_dir_all(&root).expect("creates bundled engine directory");
        let executable = root.join(platform_executable_name("yt-dlp"));
        fs::write(&executable, b"test").expect("creates bundled engine");

        assert_eq!(find_bundled_executable(&root, "yt-dlp"), Some(executable));

        fs::remove_dir_all(root).expect("removes bundled engine directory");
    }

    #[test]
    fn reads_exact_checksum_entries() {
        let digest = "a".repeat(64);
        let manifest = format!("{digest}  yt-dlp\n{}  other", "b".repeat(64));
        assert_eq!(checksum_for(&manifest, "yt-dlp"), Some(digest));
        assert_eq!(checksum_for(&manifest, "yt-dlp.exe"), None);
    }

    #[test]
    fn rejects_unsafe_versions_and_sources() {
        assert!(clean_version("../../latest").is_err());
        assert!(validate_download_url("http://github.com/file").is_err());
        assert!(validate_download_url("https://evil.example/file").is_err());
    }
}
