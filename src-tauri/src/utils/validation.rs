use crate::errors::AppError;
use std::path::{Path, PathBuf};
use url::Url;

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
    Ok(path.to_path_buf())
}

pub fn validate_filename_template(value: &str) -> Result<&str, AppError> {
    if value.is_empty() || value.len() > 240 || value.contains('/') || value.contains('\0') {
        return Err(AppError::new(
            "invalid_filename_template",
            "The filename template is not valid.",
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
