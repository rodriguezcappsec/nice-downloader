use crate::{errors::AppError, models::EnginePreferences};
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use url::Url;
use uuid::Uuid;

pub fn validate_media_url(value: &str) -> Result<Url, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("invalid_url", "Enter a media URL first."));
    }
    let url = Url::parse(trimmed)
        .map_err(|_| AppError::new("invalid_url", "Enter a valid HTTP or HTTPS URL."))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(AppError::new(
            "invalid_url",
            "Only HTTP and HTTPS media URLs are supported.",
        ));
    }
    Ok(url)
}

pub fn validate_destination(value: &str) -> Result<PathBuf, AppError> {
    let path = Path::new(value);
    if !path.is_absolute() {
        return Err(AppError::new(
            "invalid_destination",
            "Choose an absolute destination folder.",
        ));
    }
    if !path.exists() {
        return Err(AppError::new(
            "invalid_destination",
            "The destination folder does not exist.",
        ));
    }
    if !path.is_dir() {
        return Err(AppError::new(
            "invalid_destination",
            "The selected destination is not a folder.",
        ));
    }
    std::fs::canonicalize(path).map_err(|error| {
        AppError::internal("could not resolve the selected destination folder", error)
    })
}

pub fn validate_destination_writable(path: &Path) -> Result<(), AppError> {
    let probe = path.join(format!(".nice-downloader-write-test-{}", Uuid::new_v4()));
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|_| {
            AppError::new(
                "destination_not_writable",
                "Nice Downloader cannot write to the selected destination folder.",
            )
        })?;
    std::fs::remove_file(&probe).map_err(|error| {
        AppError::internal("could not remove the destination write-test file", error)
    })?;
    Ok(())
}

pub fn normalize_engine_preferences(preferences: &mut EnginePreferences) {
    preferences.browser_profile = preferences
        .browser_profile
        .take()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    preferences.proxy_url = preferences
        .proxy_url
        .take()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
}

pub fn validate_engine_preferences(preferences: &EnginePreferences) -> Result<(), AppError> {
    if preferences
        .bandwidth_limit_kib
        .is_some_and(|limit| !(64..=10_000_000).contains(&limit))
    {
        return Err(AppError::new(
            "invalid_bandwidth_limit",
            "The bandwidth limit must be between 64 KiB/s and 10,000,000 KiB/s.",
        ));
    }
    if let Some(profile) = &preferences.browser_profile {
        if preferences.browser_cookie_source.yt_dlp_name().is_none()
            || profile.len() > 80
            || !profile.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '_' | '-')
            })
        {
            return Err(AppError::new(
                "invalid_browser_profile",
                "Choose a browser before entering a profile name, and use only letters, numbers, spaces, dots, underscores, or dashes.",
            ));
        }
    }
    if let Some(proxy) = &preferences.proxy_url {
        let proxy = Url::parse(proxy).map_err(|_| {
            AppError::new(
                "invalid_proxy",
                "Enter a valid HTTP, HTTPS, SOCKS4, or SOCKS5 proxy URL.",
            )
        })?;
        if !matches!(
            proxy.scheme(),
            "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
        ) || proxy.host_str().is_none()
        {
            return Err(AppError::new(
                "invalid_proxy",
                "Enter a valid HTTP, HTTPS, SOCKS4, or SOCKS5 proxy URL.",
            ));
        }
        if !proxy.username().is_empty() || proxy.password().is_some() {
            return Err(AppError::new(
                "proxy_credentials_not_supported",
                "Proxy credentials are not stored yet. Use a proxy URL without a username or password.",
            ));
        }
    }
    Ok(())
}

pub fn validate_filename_template(value: &str) -> Result<&str, AppError> {
    if value.is_empty()
        || value.len() > 240
        || value.contains(['/', '\\', '\0'])
        || value.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "invalid_filename_template",
            "Filename templates must contain 1 to 240 characters and cannot contain path separators.",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn accepts_http_urls_only() {
        assert!(validate_media_url("https://example.com/watch?v=1").is_ok());
        assert!(validate_media_url("file:///etc/passwd").is_err());
        assert!(validate_media_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn rejects_filename_path_separators() {
        assert!(validate_filename_template("%(title)s.%(ext)s").is_ok());
        assert!(validate_filename_template("../%(title)s").is_err());
    }

    #[test]
    fn canonicalizes_equivalent_destination_paths() {
        let directory = tempdir().expect("temporary directory");
        let ordinary = validate_destination(&directory.path().to_string_lossy())
            .expect("ordinary destination");
        let dotted = validate_destination(&directory.path().join(".").to_string_lossy())
            .expect("dotted destination");

        assert_eq!(ordinary, dotted);
    }

    #[test]
    fn validates_typed_proxy_and_browser_profile_preferences() {
        use crate::models::BrowserCookieSource;

        let preferences = EnginePreferences {
            browser_cookie_source: BrowserCookieSource::Firefox,
            browser_profile: Some("work.default-release".to_owned()),
            proxy_url: Some("socks5://127.0.0.1:1080".to_owned()),
            enable_impersonation: true,
            bandwidth_limit_kib: None,
            managed_cookie_file: None,
        };
        assert!(validate_engine_preferences(&preferences).is_ok());
        assert!(validate_engine_preferences(&EnginePreferences {
            proxy_url: Some("http://user:secret@example.com".to_owned()),
            ..EnginePreferences::default()
        })
        .is_err());
    }
}
