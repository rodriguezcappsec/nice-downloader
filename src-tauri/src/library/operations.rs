use super::{file_fingerprint, file_identity, file_title, media_kind_for_path, LibraryManager};
use crate::{
    errors::AppError,
    models::{
        LibraryChangeKind, LibraryFileOperation, LibraryFileOperationKind,
        LibraryFileOperationPlan, LibraryFileOperationResult, LibraryFileOperationState,
        LibraryItem, LibraryMediaKind, PreviewLibraryFileOperationRequest,
    },
};
use chrono::Utc;
use std::{
    fs::{self, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;

impl LibraryManager {
    pub fn preview_file_operation(
        &self,
        request: PreviewLibraryFileOperationRequest,
    ) -> Result<LibraryFileOperationPlan, AppError> {
        let item = self.database.get_library_item(&request.item_id)?;
        let source = validate_current_item(&item)?;
        let destination = operation_destination(&source, request.kind, &request)?;
        if source == destination {
            return Err(AppError::new(
                "library_operation_same_path",
                "Choose a different name or destination folder.",
            ));
        }
        if destination.exists() {
            return Err(AppError::new(
                "library_operation_conflict",
                "A file or folder already exists at the planned destination. Nice Downloader never overwrites library files.",
            ));
        }
        let (source_size_bytes, source_modified_at_ms) =
            file_fingerprint(&source).ok_or_else(|| {
                AppError::new(
                    "library_file_not_found",
                    "This media file is no longer available.",
                )
            })?;
        if item.file_size_bytes != Some(source_size_bytes)
            || item.modified_at_ms != Some(source_modified_at_ms)
        {
            return Err(AppError::new(
                "library_operation_stale_item",
                "The file changed since its last scan. Scan the source again before organizing it.",
            ));
        }
        let now = Utc::now();
        let operation = LibraryFileOperation {
            id: Uuid::new_v4().to_string(),
            item_id: item.id,
            output_item_id: None,
            kind: request.kind,
            state: LibraryFileOperationState::Planned,
            source_path: source.to_string_lossy().into_owned(),
            destination_path: destination.to_string_lossy().into_owned(),
            source_size_bytes,
            source_modified_at_ms,
            error: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        self.database.save_library_file_operation(&operation)?;
        let destination_source =
            catalog_location(&self.database.list_library_sources()?, &destination);
        let mut warnings = vec!["Existing files are never overwritten.".to_owned()];
        if destination_source.0.is_none() {
            warnings.push(
                "The destination is outside configured Library folders, so automatic source scans will not monitor it."
                    .to_owned(),
            );
        }
        if request.kind == LibraryFileOperationKind::Copy {
            warnings.push(
                "Copying keeps the original file and creates a new catalog record.".to_owned(),
            );
        }
        Ok(LibraryFileOperationPlan {
            source_display_name: display_path(&source),
            destination_display_name: display_path(&destination),
            operation,
            warnings,
        })
    }

    pub async fn execute_file_operation(
        &self,
        operation_id: &str,
    ) -> Result<LibraryFileOperationResult, AppError> {
        let mut operation = self.database.get_library_file_operation(operation_id)?;
        if operation.state != LibraryFileOperationState::Planned {
            return Err(AppError::new(
                "library_operation_not_planned",
                "This file operation is no longer waiting for confirmation.",
            ));
        }
        let item = self.database.get_library_item(&operation.item_id)?;
        let source = validate_current_item(&item)?;
        let destination = revalidate_plan(&operation, &source)?;
        if !self.database.transition_library_file_operation(
            &operation.id,
            LibraryFileOperationState::Planned,
            LibraryFileOperationState::Executing,
        )? {
            return Err(AppError::new(
                "library_operation_not_planned",
                "This file operation is already running or was handled elsewhere.",
            ));
        }
        operation.state = LibraryFileOperationState::Executing;
        operation.updated_at = Utc::now();

        let kind = operation.kind;
        let expected_fingerprint = (operation.source_size_bytes, operation.source_modified_at_ms);
        let worker_source = source.clone();
        let worker_destination = destination.clone();
        let filesystem_result = tauri::async_runtime::spawn_blocking(move || match kind {
            LibraryFileOperationKind::Copy => {
                copy_file_no_clobber(&worker_source, &worker_destination, expected_fingerprint)
            }
            LibraryFileOperationKind::Move | LibraryFileOperationKind::Rename => {
                move_file_no_clobber(&worker_source, &worker_destination, expected_fingerprint)
            }
        })
        .await
        .unwrap_or_else(|error| Err(AppError::internal("file operation task stopped", error)));

        if let Err(error) = filesystem_result {
            operation.state = LibraryFileOperationState::Failed;
            operation.error = Some(error.message.clone());
            operation.updated_at = Utc::now();
            operation.completed_at = Some(operation.updated_at);
            let _ = self.database.save_library_file_operation(&operation);
            return Err(error);
        }

        let catalog_result = match operation.kind {
            LibraryFileOperationKind::Copy => self.catalog_copied_item(&item, &destination),
            LibraryFileOperationKind::Move | LibraryFileOperationKind::Rename => {
                self.catalog_moved_item(&item, &destination)
            }
        };
        let output_item = match catalog_result {
            Ok(item) => item,
            Err(error) => {
                operation.state = LibraryFileOperationState::Failed;
                operation.error = Some(
                    "The filesystem operation completed, but the catalog could not be updated. Run Verify files."
                        .to_owned(),
                );
                operation.updated_at = Utc::now();
                operation.completed_at = Some(operation.updated_at);
                let _ = self.database.save_library_file_operation(&operation);
                return Err(AppError::with_details(
                    "library_operation_catalog_failed",
                    "The file operation completed, but the catalog could not be updated. Run Verify files in Library management.",
                    error.details.unwrap_or(error.message),
                ));
            }
        };
        operation.output_item_id = Some(output_item.id.clone());
        operation.state = LibraryFileOperationState::Completed;
        operation.updated_at = Utc::now();
        operation.completed_at = Some(operation.updated_at);
        self.database.save_library_file_operation(&operation)?;
        let change_kind = if operation.kind == LibraryFileOperationKind::Copy {
            LibraryChangeKind::Added
        } else {
            LibraryChangeKind::Renamed
        };
        let _ = self.database.record_library_change(
            &output_item.id,
            output_item.source_id.as_deref(),
            change_kind,
            (operation.kind != LibraryFileOperationKind::Copy)
                .then_some(operation.source_path.as_str()),
        );
        Ok(LibraryFileOperationResult {
            operation,
            items: vec![output_item],
        })
    }

    pub async fn undo_file_operation(
        &self,
        operation_id: &str,
    ) -> Result<LibraryFileOperationResult, AppError> {
        let mut operation = self.database.get_library_file_operation(operation_id)?;
        if operation.state != LibraryFileOperationState::Completed {
            return Err(AppError::new(
                "library_operation_not_undoable",
                "Only completed file operations can be undone.",
            ));
        }
        let output_item_id = operation.output_item_id.clone().ok_or_else(|| {
            AppError::new(
                "library_operation_not_undoable",
                "This operation does not have a catalog output to undo.",
            )
        })?;
        let output_item = self.database.get_library_item(&output_item_id)?;
        if output_item.path != operation.destination_path {
            return Err(AppError::new(
                "library_operation_changed",
                "The operated file moved again after this action, so automatic undo is unavailable.",
            ));
        }

        let operated_path = validate_current_item(&output_item)?;
        if operated_path != Path::new(&operation.destination_path)
            || file_fingerprint(&operated_path)
                != output_item.file_size_bytes.zip(output_item.modified_at_ms)
        {
            return Err(AppError::new(
                "library_operation_changed",
                "The operated file changed after this action, so automatic undo is unavailable.",
            ));
        }
        let original_path = PathBuf::from(&operation.source_path);
        if operation.kind != LibraryFileOperationKind::Copy && original_path.exists() {
            return Err(AppError::new(
                "library_operation_undo_conflict",
                "The original path is occupied, so this operation cannot be undone safely.",
            ));
        }
        let expected_fingerprint = output_item
            .file_size_bytes
            .zip(output_item.modified_at_ms)
            .ok_or_else(|| {
                AppError::new(
                    "library_operation_changed",
                    "The operated file does not have a current catalog fingerprint.",
                )
            })?;
        if !self.database.transition_library_file_operation(
            &operation.id,
            LibraryFileOperationState::Completed,
            LibraryFileOperationState::Executing,
        )? {
            return Err(AppError::new(
                "library_operation_not_undoable",
                "This file operation is already being handled elsewhere.",
            ));
        }
        operation.state = LibraryFileOperationState::Executing;
        operation.updated_at = Utc::now();

        let kind = operation.kind;
        let worker_source = operated_path.clone();
        let worker_destination = original_path.clone();
        let filesystem_result = tauri::async_runtime::spawn_blocking(move || match kind {
            LibraryFileOperationKind::Copy => trash::delete(&worker_source).map_err(|error| {
                AppError::with_details(
                    "trash_failed",
                    "The copied file could not be moved to the system Trash.",
                    error.to_string(),
                )
            }),
            LibraryFileOperationKind::Move | LibraryFileOperationKind::Rename => {
                move_file_no_clobber(&worker_source, &worker_destination, expected_fingerprint)
            }
        })
        .await
        .unwrap_or_else(|error| {
            Err(AppError::internal(
                "file operation undo task stopped",
                error,
            ))
        });
        if let Err(error) = filesystem_result {
            operation.state = LibraryFileOperationState::Completed;
            operation.error = Some(error.message.clone());
            operation.updated_at = Utc::now();
            let _ = self.database.save_library_file_operation(&operation);
            return Err(error);
        }

        let catalog_result = if operation.kind == LibraryFileOperationKind::Copy {
            self.database
                .remove_library_item(&output_item_id)
                .map(|()| vec![])
        } else {
            self.catalog_moved_item(&output_item, &original_path)
                .map(|item| vec![item])
        };
        let items = match catalog_result {
            Ok(items) => items,
            Err(error) => {
                operation.state = LibraryFileOperationState::Failed;
                operation.error = Some(
                    "Undo changed the file, but the catalog could not be updated. Run Verify files."
                        .to_owned(),
                );
                operation.updated_at = Utc::now();
                operation.completed_at = Some(operation.updated_at);
                let _ = self.database.save_library_file_operation(&operation);
                return Err(AppError::with_details(
                    "library_operation_catalog_failed",
                    "The file was changed, but the catalog could not be updated. Run Verify files in Library management.",
                    error.details.unwrap_or(error.message),
                ));
            }
        };

        operation.state = LibraryFileOperationState::Undone;
        operation.error = None;
        operation.updated_at = Utc::now();
        operation.completed_at = Some(operation.updated_at);
        self.database.save_library_file_operation(&operation)?;
        Ok(LibraryFileOperationResult { operation, items })
    }

    fn catalog_moved_item(
        &self,
        item: &LibraryItem,
        destination: &Path,
    ) -> Result<LibraryItem, AppError> {
        let mut moved = item.clone();
        apply_catalog_location(
            &mut moved,
            &self.database.list_library_sources()?,
            destination,
        );
        moved.path = destination.to_string_lossy().into_owned();
        let (size, modified) = file_fingerprint(destination).ok_or_else(|| {
            AppError::new(
                "library_file_not_found",
                "The moved file could not be verified.",
            )
        })?;
        moved.file_size_bytes = Some(size);
        moved.modified_at_ms = Some(modified);
        moved.missing = false;
        moved.updated_at = Utc::now();
        let stored = self.database.move_library_item_record(&item.id, &moved)?;
        if let Some(identity) = file_identity(destination) {
            self.database
                .save_library_file_identity(&stored.id, identity)?;
        }
        Ok(stored)
    }

    fn catalog_copied_item(
        &self,
        item: &LibraryItem,
        destination: &Path,
    ) -> Result<LibraryItem, AppError> {
        let mut copied = item.clone();
        copied.id = Uuid::new_v4().to_string();
        copied.path = destination.to_string_lossy().into_owned();
        copied.title = file_title(destination);
        copied.favorite = false;
        copied.rating = None;
        copied.added_at = Utc::now();
        copied.updated_at = copied.added_at;
        copied.missing = false;
        apply_catalog_location(
            &mut copied,
            &self.database.list_library_sources()?,
            destination,
        );
        let (size, modified) = file_fingerprint(destination).ok_or_else(|| {
            AppError::new(
                "library_file_not_found",
                "The copied file could not be verified.",
            )
        })?;
        copied.file_size_bytes = Some(size);
        copied.modified_at_ms = Some(modified);
        if copied.media_kind == Some(LibraryMediaKind::Image) {
            copied.thumbnail = Some(copied.path.clone());
        }
        let stored = self.database.upsert_library_item(&copied)?;
        if let Some(identity) = file_identity(destination) {
            self.database
                .save_library_file_identity(&stored.id, identity)?;
        }
        Ok(stored)
    }
}

fn validate_current_item(item: &LibraryItem) -> Result<PathBuf, AppError> {
    if item.missing {
        return Err(AppError::new(
            "library_file_not_found",
            "This media file is no longer available.",
        ));
    }
    let path = PathBuf::from(&item.path);
    if !path.is_absolute() {
        return Err(AppError::new(
            "invalid_library_path",
            "The catalog path is not an absolute file path.",
        ));
    }
    let metadata = fs::symlink_metadata(&path).map_err(|_| {
        AppError::new(
            "library_file_not_found",
            "This media file is no longer available.",
        )
    })?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "invalid_library_path",
            "Only regular media files can be organized.",
        ));
    }
    Ok(path)
}

fn operation_destination(
    source: &Path,
    kind: LibraryFileOperationKind,
    request: &PreviewLibraryFileOperationRequest,
) -> Result<PathBuf, AppError> {
    let source_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            AppError::new(
                "invalid_library_filename",
                "The source filename is invalid.",
            )
        })?;
    let requested_name = request.new_file_name.as_deref().unwrap_or(source_name);
    let name = validate_operation_filename(requested_name, source)?;
    let directory = if kind == LibraryFileOperationKind::Rename {
        source
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::new("invalid_library_path", "The source folder is invalid."))?
    } else {
        let directory = request.destination_directory.as_deref().ok_or_else(|| {
            AppError::new(
                "library_destination_required",
                "Choose a destination folder for this operation.",
            )
        })?;
        validate_operation_directory(directory)?
    };
    Ok(directory.join(name))
}

fn validate_operation_directory(value: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(AppError::new(
            "invalid_library_destination",
            "Choose an absolute destination folder.",
        ));
    }
    let canonical = path.canonicalize().map_err(|_| {
        AppError::new(
            "invalid_library_destination",
            "The destination folder does not exist or cannot be accessed.",
        )
    })?;
    if !canonical.is_dir() || canonical.parent().is_none() {
        return Err(AppError::new(
            "invalid_library_destination",
            "Choose an existing destination folder, not the filesystem root.",
        ));
    }
    Ok(canonical)
}

fn validate_operation_filename(value: &str, source: &Path) -> Result<String, AppError> {
    let value = value.trim();
    let candidate = Path::new(value);
    if value.is_empty()
        || value.chars().count() > 255
        || value.chars().any(char::is_control)
        || value.contains(['/', '\\'])
        || candidate.components().count() != 1
        || !matches!(candidate.components().next(), Some(Component::Normal(_)))
    {
        return Err(AppError::new(
            "invalid_library_filename",
            "Use a filename without folders, control characters, or path separators.",
        ));
    }
    let source_extension = source.extension().and_then(|extension| extension.to_str());
    let next_extension = candidate
        .extension()
        .and_then(|extension| extension.to_str());
    if !source_extension
        .zip(next_extension)
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
    {
        return Err(AppError::new(
            "library_extension_change_not_allowed",
            "Keep the original file extension. Use Media Transform to change formats.",
        ));
    }
    Ok(value.to_owned())
}

fn revalidate_plan(operation: &LibraryFileOperation, source: &Path) -> Result<PathBuf, AppError> {
    if source != Path::new(&operation.source_path)
        || file_fingerprint(source)
            != Some((operation.source_size_bytes, operation.source_modified_at_ms))
    {
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed after preview. Create a new plan.",
        ));
    }
    let recorded = PathBuf::from(&operation.destination_path);
    let parent = recorded.parent().ok_or_else(|| {
        AppError::new(
            "invalid_library_destination",
            "The destination path is invalid.",
        )
    })?;
    let canonical_parent = validate_operation_directory(&parent.to_string_lossy())?;
    let destination = canonical_parent.join(recorded.file_name().ok_or_else(|| {
        AppError::new(
            "invalid_library_destination",
            "The destination filename is invalid.",
        )
    })?);
    if destination != recorded || destination.exists() {
        return Err(AppError::new(
            "library_operation_conflict",
            "The destination changed or is now occupied. Create a new plan.",
        ));
    }
    Ok(destination)
}

fn copy_file_no_clobber(
    source: &Path,
    destination: &Path,
    expected_fingerprint: (u64, i64),
) -> Result<(), AppError> {
    if !is_regular_file_with_fingerprint(source, expected_fingerprint) {
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed after preview. Nothing was copied.",
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        AppError::new(
            "invalid_library_destination",
            "The destination folder is invalid.",
        )
    })?;
    let temporary = parent.join(format!(".nice-downloader-copy-{}.part", Uuid::new_v4()));
    let result = (|| {
        let source_file = fs::File::open(source)
            .map_err(|error| AppError::internal("could not open the source file", error))?;
        let temporary_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| AppError::internal("could not create a temporary copy", error))?;
        let mut reader = BufReader::with_capacity(1024 * 1024, source_file);
        let mut writer = BufWriter::with_capacity(1024 * 1024, temporary_file);
        io::copy(&mut reader, &mut writer)
            .map_err(|error| AppError::internal("could not copy the media file", error))?;
        writer
            .flush()
            .map_err(|error| AppError::internal("could not flush the copied file", error))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|error| AppError::internal("could not sync the copied file", error))?;
        let permissions = fs::metadata(source)
            .map_err(|error| AppError::internal("could not inspect source permissions", error))?
            .permissions();
        fs::set_permissions(&temporary, permissions).map_err(|error| {
            AppError::internal("could not apply copied file permissions", error)
        })?;
        fs::hard_link(&temporary, destination).map_err(|error| {
            if destination.exists() {
                AppError::new(
                    "library_operation_conflict",
                    "A file appeared at the destination. Nothing was overwritten.",
                )
            } else {
                AppError::internal(
                    "could not publish the copied file without overwriting",
                    error,
                )
            }
        })?;
        if !is_regular_file_with_fingerprint(source, expected_fingerprint) {
            let _ = fs::remove_file(destination);
            return Err(AppError::new(
                "library_operation_stale_plan",
                "The source file changed while it was being copied. The incomplete result was removed.",
            ));
        }
        if let Err(error) = fs::remove_file(&temporary) {
            tracing::warn!(%error, path = %temporary.display(), "could not remove copy staging link");
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn move_file_no_clobber(
    source: &Path,
    destination: &Path,
    expected_fingerprint: (u64, i64),
) -> Result<(), AppError> {
    if !is_regular_file_with_fingerprint(source, expected_fingerprint) {
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed after preview. Nothing was moved.",
        ));
    }
    let source_identity = file_identity(source);
    fs::hard_link(source, destination).map_err(|error| {
        if destination.exists() {
            AppError::new(
                "library_operation_conflict",
                "A file appeared at the destination. Nothing was overwritten.",
            )
        } else if error.kind() == io::ErrorKind::CrossesDevices {
            AppError::new(
                "library_operation_cross_device",
                "This move crosses filesystems. Copy the file first, verify it, then move the original to Trash.",
            )
        } else {
            AppError::internal("could not create the destination file", error)
        }
    })?;
    let destination_is_valid = fs::symlink_metadata(destination)
        .is_ok_and(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
        && file_fingerprint(destination) == Some(expected_fingerprint)
        && source_identity
            .zip(file_identity(source))
            .is_none_or(|(expected, current)| expected == current);
    if !destination_is_valid {
        let _ = fs::remove_file(destination);
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed while it was being moved. Nothing was removed.",
        ));
    }
    if let Err(error) = fs::remove_file(source) {
        let rollback_error = fs::remove_file(destination).err();
        return Err(if let Some(rollback_error) = rollback_error {
            AppError::with_details(
                "library_operation_cleanup_failed",
                "The source could not be removed and the destination could not be rolled back. Run Verify files.",
                format!("source removal: {error}; destination rollback: {rollback_error}"),
            )
        } else {
            AppError::internal(
                "could not remove the original file after linking its destination",
                error,
            )
        });
    }
    Ok(())
}

fn is_regular_file_with_fingerprint(path: &Path, expected: (u64, i64)) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_file()
            && !metadata.file_type().is_symlink()
            && file_fingerprint(path) == Some(expected)
    })
}

fn catalog_location(
    sources: &[crate::models::LibrarySource],
    path: &Path,
) -> (Option<String>, Option<String>) {
    sources
        .iter()
        .filter_map(|source| {
            let root = Path::new(&source.path);
            let relative = path.strip_prefix(root).ok()?;
            Some((root.components().count(), source.id.clone(), relative))
        })
        .max_by_key(|(depth, _, _)| *depth)
        .map(|(_, source_id, relative)| {
            (
                Some(source_id),
                Some(relative.to_string_lossy().replace('\\', "/")),
            )
        })
        .unwrap_or((None, None))
}

fn apply_catalog_location(
    item: &mut LibraryItem,
    sources: &[crate::models::LibrarySource],
    path: &Path,
) {
    (item.source_id, item.relative_path) = catalog_location(sources, path);
    item.media_kind = media_kind_for_path(path);
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::Database,
        models::{LibraryMetadataState, PreviewLibraryFileOperationRequest},
    };
    use tempfile::tempdir;

    fn catalog_fixture(database: &Database, path: &Path) -> LibraryItem {
        let now = Utc::now();
        let (size, modified) = file_fingerprint(path).expect("fixture fingerprint");
        database
            .upsert_library_item(&LibraryItem {
                id: Uuid::new_v4().to_string(),
                path: path.to_string_lossy().into_owned(),
                source_id: None,
                relative_path: None,
                media_kind: Some(LibraryMediaKind::Video),
                title: "Fixture clip".to_owned(),
                source_url: None,
                thumbnail: None,
                duration_seconds: None,
                site: None,
                tags: vec!["test".to_owned()],
                metadata: serde_json::json!({}),
                file_size_bytes: Some(size),
                modified_at_ms: Some(modified),
                width: None,
                height: None,
                fps: None,
                video_codec: None,
                audio_codec: None,
                container: Some("mp4".to_owned()),
                dynamic_range: None,
                metadata_state: LibraryMetadataState::Ready,
                metadata_error: None,
                favorite: false,
                rating: None,
                missing: false,
                added_at: now,
                updated_at: now,
            })
            .expect("catalog fixture")
    }

    #[tokio::test]
    async fn previews_executes_and_undoes_a_rename() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("clip.mp4");
        fs::write(&source, b"media fixture").expect("media fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let item = catalog_fixture(&database, &source);
        let manager = LibraryManager::new(database.clone()).expect("library manager");

        let plan = manager
            .preview_file_operation(PreviewLibraryFileOperationRequest {
                item_id: item.id.clone(),
                kind: LibraryFileOperationKind::Rename,
                destination_directory: None,
                new_file_name: Some("renamed.mp4".to_owned()),
            })
            .expect("rename preview");
        let destination = directory.path().join("renamed.mp4");
        assert_eq!(plan.operation.state, LibraryFileOperationState::Planned);
        assert_eq!(Path::new(&plan.operation.destination_path), destination);

        let completed = manager
            .execute_file_operation(&plan.operation.id)
            .await
            .expect("execute rename");
        assert_eq!(
            completed.operation.state,
            LibraryFileOperationState::Completed
        );
        assert!(!source.exists());
        assert!(destination.exists());
        assert_eq!(completed.items[0].id, item.id);

        let undone = manager
            .undo_file_operation(&plan.operation.id)
            .await
            .expect("undo rename");
        assert_eq!(undone.operation.state, LibraryFileOperationState::Undone);
        assert!(source.exists());
        assert!(!destination.exists());
        assert_eq!(undone.items[0].path, source.to_string_lossy());
    }

    #[tokio::test]
    async fn copy_never_overwrites_a_destination_that_appears_after_preview() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        let destination_directory = directory.path().join("copies");
        fs::create_dir(&destination_directory).expect("copy directory");
        fs::write(&source, b"source bytes").expect("source fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let item = catalog_fixture(&database, &source);
        let manager = LibraryManager::new(database.clone()).expect("library manager");
        let plan = manager
            .preview_file_operation(PreviewLibraryFileOperationRequest {
                item_id: item.id,
                kind: LibraryFileOperationKind::Copy,
                destination_directory: Some(destination_directory.to_string_lossy().into_owned()),
                new_file_name: Some("copy.mp4".to_owned()),
            })
            .expect("copy preview");
        let destination = destination_directory.join("copy.mp4");
        fs::write(&destination, b"other bytes").expect("conflicting fixture");

        let error = manager
            .execute_file_operation(&plan.operation.id)
            .await
            .expect_err("copy conflict");
        assert_eq!(error.code, "library_operation_conflict");
        assert_eq!(
            fs::read(&destination).expect("destination bytes"),
            b"other bytes"
        );
        assert_eq!(fs::read(&source).expect("source bytes"), b"source bytes");
    }

    #[tokio::test]
    async fn rejects_a_plan_when_the_source_fingerprint_changes() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("clip.mp4");
        fs::write(&source, b"original").expect("media fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let item = catalog_fixture(&database, &source);
        let manager = LibraryManager::new(database).expect("library manager");
        let plan = manager
            .preview_file_operation(PreviewLibraryFileOperationRequest {
                item_id: item.id,
                kind: LibraryFileOperationKind::Rename,
                destination_directory: None,
                new_file_name: Some("renamed.mp4".to_owned()),
            })
            .expect("rename preview");
        fs::write(&source, b"changed and longer").expect("changed fixture");

        let error = manager
            .execute_file_operation(&plan.operation.id)
            .await
            .expect_err("stale plan");
        assert_eq!(error.code, "library_operation_stale_plan");
        assert!(source.exists());
        assert!(!directory.path().join("renamed.mp4").exists());
    }
}
