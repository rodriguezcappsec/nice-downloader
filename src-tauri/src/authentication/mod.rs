use crate::{
    database::Database,
    errors::AppError,
    models::{
        AuthenticationProfile, BrowserCookieSource, BrowserProfileInfo, EnginePreferences,
        SaveAuthenticationProfileRequest,
    },
};
use std::path::{Path, PathBuf};
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
    let mut managed_path = existing
        .as_ref()
        .and_then(|profile| profile.managed_cookie_path.clone());
    if request.clear_cookie_file {
        remove_managed_cookie(managed_path.as_deref());
        managed_path = None;
    }
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
        let destination = auth_directory.join(format!("{id}.cookies.txt"));
        std::fs::copy(&source, &destination)
            .map_err(|error| AppError::internal("could not import the cookie file", error))?;
        set_private_permissions(&destination)?;
        managed_path = Some(destination.to_string_lossy().into_owned());
    }
    database.save_authentication_profile(request, managed_path)
}

pub fn delete_profile(database: &Database, id: &str) -> Result<(), AppError> {
    let managed = database.delete_authentication_profile(id)?;
    remove_managed_cookie(managed.as_deref());
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

fn remove_managed_cookie(path: Option<&str>) {
    if let Some(path) = path {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_profile_hosts() {
        assert_eq!(normalize_host("*.Reddit.com.").unwrap(), "reddit.com");
    }
    #[test]
    fn rejects_host_paths() {
        assert!(normalize_host("reddit.com/r/test").is_err());
    }
}
