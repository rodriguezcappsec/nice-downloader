use crate::{
    database::Database,
    errors::AppError,
    models::{
        AuthenticationProfile, BrowserCookieSource, BrowserProfileInfo, EnginePreferences,
        SaveAuthenticationProfileRequest,
    },
};
use std::{
    fs::OpenOptions,
    io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn save_profile(
    database: &Database,
    data_directory: &Path,
    mut request: SaveAuthenticationProfileRequest,
) -> Result<AuthenticationProfile, AppError> {
    request.name = request.name.trim().chars().take(80).collect();
    if request.name.is_empty() {
        return Err(AppError::new(
            "invalid_auth_profile",
            "Enter a profile name.",
        ));
    }
    request.host_suffix = normalize_host(&request.host_suffix)?;
    request.browser_profile = request
        .browser_profile
        .map(|value| value.trim().chars().take(160).collect())
        .filter(|value: &String| !value.is_empty());
    request.proxy_url = normalize_proxy(request.proxy_url)?;
    let id = request
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    Uuid::parse_str(&id).map_err(|_| {
        AppError::new(
            "invalid_auth_profile_id",
            "The authentication profile ID is invalid.",
        )
    })?;
    request.id = Some(id.clone());
    let existing = database.authentication_profile(&id)?;
    let previous_managed_path = existing
        .as_ref()
        .and_then(|profile| profile.managed_cookie_path.clone());
    let mut managed_path = if request.clear_cookie_file {
        None
    } else {
        previous_managed_path.clone()
    };
    let mut staged_cookie = None;
    if let Some(source) = request
        .cookie_file_path
        .take()
        .filter(|value| !value.trim().is_empty())
    {
        let source = PathBuf::from(source).canonicalize().map_err(|_| {
            AppError::new(
                "invalid_cookie_file",
                "Choose an existing Netscape cookies.txt file.",
            )
        })?;
        validate_cookie_file(&source)?;
        let auth_directory = data_directory.join("authentication");
        std::fs::create_dir_all(&auth_directory).map_err(|error| {
            AppError::internal("could not create secure authentication storage", error)
        })?;
        let destination = auth_directory.join(format!("{id}-{}.cookies.txt", Uuid::new_v4()));
        copy_cookie_securely(&source, &destination)?;
        managed_path = Some(destination.to_string_lossy().into_owned());
        staged_cookie = Some(destination);
    }
    let saved = match database.save_authentication_profile(request, managed_path.clone()) {
        Ok(saved) => saved,
        Err(error) => {
            if let Some(path) = staged_cookie.as_ref() {
                let _ = std::fs::remove_file(path);
            }
            return Err(error);
        }
    };
    if previous_managed_path != managed_path {
        if let Err(error) = remove_managed_cookie(previous_managed_path.as_deref()) {
            let rollback = existing
                .as_ref()
                .ok_or_else(|| {
                    AppError::new(
                        "authentication_profile_not_found",
                        "The previous authentication profile could not be restored.",
                    )
                })
                .and_then(|profile| {
                    database
                        .save_authentication_profile(
                            request_from_profile(profile),
                            profile.managed_cookie_path.clone(),
                        )
                        .map(|_| ())
                });
            match rollback {
                Ok(()) => {
                    if let Some(path) = staged_cookie.as_ref() {
                        let _ = std::fs::remove_file(path);
                    }
                    return Err(AppError::with_details(
                        "managed_cookie_cleanup_failed",
                        "The old cookie file could not be removed, so the profile change was rolled back.",
                        error.details.unwrap_or(error.message),
                    ));
                }
                Err(rollback_error) => {
                    return Err(AppError::with_details(
                        "managed_cookie_cleanup_failed",
                        "The profile was saved, but the old cookie file could not be removed. Review the authentication storage folder.",
                        format!(
                            "cleanup: {}; rollback: {}",
                            error.details.unwrap_or(error.message),
                            rollback_error
                                .details
                                .unwrap_or(rollback_error.message)
                        ),
                    ));
                }
            }
        }
    }
    Ok(saved)
}

fn request_from_profile(profile: &AuthenticationProfile) -> SaveAuthenticationProfileRequest {
    SaveAuthenticationProfileRequest {
        id: Some(profile.id.clone()),
        name: profile.name.clone(),
        host_suffix: profile.host_suffix.clone(),
        browser_cookie_source: profile.browser_cookie_source,
        browser_profile: profile.browser_profile.clone(),
        proxy_url: profile.proxy_url.clone(),
        cookie_file_path: None,
        clear_cookie_file: false,
        enabled: profile.enabled,
    }
}

pub fn delete_profile(database: &Database, id: &str) -> Result<(), AppError> {
    let profile = database.authentication_profile(id)?.ok_or_else(|| {
        AppError::new(
            "authentication_profile_not_found",
            "The authentication profile no longer exists.",
        )
    })?;
    remove_managed_cookie(profile.managed_cookie_path.as_deref())?;
    database.delete_authentication_profile(id)?;
    Ok(())
}

pub fn apply_profile(
    database: &Database,
    url: &str,
    preferences: &mut EnginePreferences,
) -> Result<(), AppError> {
    let Some(profile) = database.match_authentication_profile(url)? else {
        return Ok(());
    };
    preferences.browser_cookie_source = profile.browser_cookie_source;
    preferences.browser_profile = profile.browser_profile;
    preferences.proxy_url = profile.proxy_url;
    preferences.managed_cookie_file = profile.managed_cookie_path;
    Ok(())
}

pub fn discover_browser_profiles() -> Vec<BrowserProfileInfo> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Vec::new();
    };
    let candidates = [
        (BrowserCookieSource::Firefox, home.join(".mozilla/firefox")),
        (BrowserCookieSource::Chromium, home.join(".config/chromium")),
        (
            BrowserCookieSource::Chrome,
            home.join(".config/google-chrome"),
        ),
        (
            BrowserCookieSource::Brave,
            home.join(".config/BraveSoftware/Brave-Browser"),
        ),
        (BrowserCookieSource::Vivaldi, home.join(".config/vivaldi")),
        (
            BrowserCookieSource::Edge,
            home.join(".config/microsoft-edge"),
        ),
        (BrowserCookieSource::Opera, home.join(".config/opera")),
    ];
    let mut profiles = Vec::new();
    for (browser, base) in candidates {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let is_profile = if browser == BrowserCookieSource::Firefox {
                path.join("cookies.sqlite").is_file()
            } else {
                name == "Default" || name.starts_with("Profile ")
            };
            if is_profile {
                profiles.push(BrowserProfileInfo {
                    browser,
                    name: name.to_owned(),
                    path: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
    profiles.sort_by(|left, right| left.name.cmp(&right.name));
    profiles
}

fn normalize_host(value: &str) -> Result<String, AppError> {
    let host = value
        .trim()
        .trim_start_matches("*.")
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host.is_empty()
        || host.len() > 253
        || host.contains('/')
        || host.contains(':')
        || !host.contains('.')
    {
        return Err(AppError::new(
            "invalid_auth_host",
            "Enter a hostname such as reddit.com.",
        ));
    }
    Ok(host)
}

fn normalize_proxy(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let parsed = url::Url::parse(&value).map_err(|_| {
        AppError::new(
            "invalid_proxy",
            "Enter a valid HTTP, HTTPS, or SOCKS proxy URL.",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https" | "socks4" | "socks5") {
        return Err(AppError::new(
            "invalid_proxy",
            "Use an HTTP, HTTPS, SOCKS4, or SOCKS5 proxy URL.",
        ));
    }
    Ok(Some(value))
}

fn validate_cookie_file(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| AppError::internal("could not inspect the cookie file", error))?;
    if !metadata.is_file() || metadata.len() > 20 * 1024 * 1024 {
        return Err(AppError::new(
            "invalid_cookie_file",
            "The cookie file must be a local text file smaller than 20 MB.",
        ));
    }
    let sample = std::fs::read(path)
        .map_err(|error| AppError::internal("could not read the cookie file", error))?;
    let sample = String::from_utf8_lossy(&sample[..sample.len().min(8192)]);
    if !sample.contains("Netscape HTTP Cookie File")
        && !sample.lines().any(|line| line.split('\t').count() >= 7)
    {
        return Err(AppError::new(
            "invalid_cookie_file",
            "Export cookies in Netscape cookies.txt format.",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> Result<(), AppError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|error| AppError::internal("could not secure the imported cookie file", error))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &Path) -> Result<(), AppError> {
    Ok(())
}

fn copy_cookie_securely(source: &Path, destination: &Path) -> Result<(), AppError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut input = std::fs::File::open(source)
        .map_err(|error| AppError::internal("could not open the cookie file", error))?;
    let mut output = options
        .open(destination)
        .map_err(|error| AppError::internal("could not create secure cookie storage", error))?;
    if let Err(error) = io::copy(&mut input, &mut output) {
        drop(output);
        let _ = std::fs::remove_file(destination);
        return Err(AppError::internal(
            "could not import the cookie file",
            error,
        ));
    }
    if let Err(error) = set_private_permissions(destination) {
        drop(output);
        let _ = std::fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}

fn remove_managed_cookie(path: Option<&str>) -> Result<(), AppError> {
    if let Some(path) = path {
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(AppError::internal(
                    "could not remove the managed cookie file",
                    error,
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn normalizes_profile_hosts() {
        assert_eq!(normalize_host("*.Reddit.com.").unwrap(), "reddit.com");
    }
    #[test]
    fn rejects_host_paths() {
        assert!(normalize_host("reddit.com/r/test").is_err());
    }

    #[test]
    fn imports_cookie_files_with_private_permissions() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("managed.txt");
        std::fs::write(&source, "# Netscape HTTP Cookie File\n").expect("cookie source");
        copy_cookie_securely(&source, &destination).expect("secure cookie copy");
        assert_eq!(
            std::fs::read_to_string(&destination).expect("managed cookie"),
            "# Netscape HTTP Cookie File\n"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(destination)
                    .expect("cookie metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
}
