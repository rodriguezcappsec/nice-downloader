use crate::{
    database::Database,
    errors::AppError,
    models::{LibraryItem, LibraryScanResult},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const MAX_IMPORT_FILES: usize = 200;

pub fn import_paths(
    database: &Database,
    paths: Vec<String>,
    tags: Vec<String>,
) -> Result<Vec<LibraryItem>, AppError> {
    if paths.is_empty() || paths.len() > MAX_IMPORT_FILES {
        return Err(AppError::new(
            "invalid_library_import",
            format!("Choose between 1 and {MAX_IMPORT_FILES} media files."),
        ));
    }
    let tags = normalize_tags(tags)?;
    let mut imported = Vec::with_capacity(paths.len());
    for value in paths {
        let path = PathBuf::from(value).canonicalize().map_err(|_| {
            AppError::new(
                "invalid_library_path",
                "One of the selected media files no longer exists.",
            )
        })?;
        if !path.is_file() {
            return Err(AppError::new(
                "invalid_library_path",
                "Only local files can be added to the media library.",
            ));
        }
        let sidecar = read_sidecar(&path);
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into_owned(),
            title: sidecar
                .as_ref()
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| file_title(&path)),
            source_url: sidecar
                .as_ref()
                .and_then(|value| value.get("webpage_url"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            thumbnail: find_local_thumbnail(&path),
            duration_seconds: sidecar
                .as_ref()
                .and_then(|value| value.get("duration"))
                .and_then(Value::as_f64),
            site: sidecar
                .as_ref()
                .and_then(|value| {
                    value
                        .get("extractor_key")
                        .or_else(|| value.get("extractor"))
                })
                .and_then(Value::as_str)
                .map(str::to_owned),
            tags: tags.clone(),
            metadata: sidecar
                .as_ref()
                .map(safe_sidecar_metadata)
                .unwrap_or_else(|| json!({})),
            missing: false,
            added_at: now,
            updated_at: now,
        };
        database.upsert_library_item(&item)?;
        imported.push(item);
    }
    Ok(imported)
}

pub fn reconcile(database: &Database) -> Result<LibraryScanResult, AppError> {
    let items = database.list_library_items()?;
    let mut missing_count = 0;
    for item in &items {
        let missing = !Path::new(&item.path).is_file();
        if missing {
            missing_count += 1;
        }
        if missing != item.missing {
            database.set_library_missing(&item.id, missing)?;
        }
    }
    Ok(LibraryScanResult {
        checked_count: items.len(),
        missing_count,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryBackup {
    version: u32,
    exported_at: chrono::DateTime<Utc>,
    items: Vec<LibraryItem>,
}

pub fn export_backup(database: &Database, path: &str) -> Result<(), AppError> {
    let path = validate_json_output(path)?;
    let backup = LibraryBackup {
        version: 1,
        exported_at: Utc::now(),
        items: database.list_library_items()?,
    };
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|error| {
        AppError::internal("could not serialize the media library backup", error)
    })?;
    std::fs::write(path, bytes)
        .map_err(|error| AppError::internal("could not write the media library backup", error))
}

pub fn restore_backup(database: &Database, path: &str) -> Result<Vec<LibraryItem>, AppError> {
    let path = Path::new(path);
    if !path.is_absolute() || path.extension().and_then(|value| value.to_str()) != Some("json") {
        return Err(AppError::new(
            "invalid_library_backup",
            "Choose an absolute .json library backup file.",
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| AppError::internal("could not read the media library backup", error))?;
    let mut backup: LibraryBackup = serde_json::from_slice(&bytes)
        .map_err(|error| AppError::internal("could not parse the media library backup", error))?;
    if backup.version != 1 || backup.items.len() > 5000 {
        return Err(AppError::new(
            "unsupported_library_backup",
            "This library backup is unsupported or too large.",
        ));
    }
    for item in &mut backup.items {
        if Uuid::parse_str(&item.id).is_err() {
            item.id = Uuid::new_v4().to_string();
        }
        item.tags = normalize_tags(std::mem::take(&mut item.tags))?;
        item.missing = !Path::new(&item.path).is_file();
        item.updated_at = Utc::now();
        database.upsert_library_item(item)?;
    }
    Ok(backup.items)
}

fn normalize_tags(tags: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim().chars().take(40).collect::<String>();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_library_tag",
                "Tags cannot contain control characters.",
            ));
        }
        if !normalized
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&tag))
        {
            normalized.push(tag);
        }
    }
    normalized.truncate(20);
    Ok(normalized)
}

fn read_sidecar(path: &Path) -> Option<Value> {
    let sidecar = path.with_extension("info.json");
    let bytes = std::fs::read(sidecar).ok()?;
    if bytes.len() > 8 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn safe_sidecar_metadata(value: &Value) -> Value {
    json!({
        "id": value.get("id"), "title": value.get("title"), "uploader": value.get("uploader"),
        "uploadDate": value.get("upload_date"), "description": value.get("description").and_then(Value::as_str).map(|text| text.chars().take(2000).collect::<String>()),
        "extractor": value.get("extractor"), "format": value.get("format"),
    })
}

fn file_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled media")
        .to_owned()
}

fn find_local_thumbnail(path: &Path) -> Option<String> {
    ["jpg", "jpeg", "png", "webp"]
        .into_iter()
        .map(|extension| path.with_extension(extension))
        .find(|candidate| candidate.is_file())
        .map(|candidate| candidate.to_string_lossy().into_owned())
}

fn validate_json_output(value: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(value);
    if !path.is_absolute()
        || path.extension().and_then(|value| value.to_str()) != Some("json")
        || !path.parent().is_some_and(Path::is_dir)
    {
        return Err(AppError::new(
            "invalid_library_backup",
            "Choose an existing folder and a filename ending in .json.",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_trimmed_and_deduplicated() {
        assert_eq!(
            normalize_tags(vec![" Music ".into(), "music".into(), "live".into()]).unwrap(),
            ["Music", "live"]
        );
    }
}
