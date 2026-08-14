use super::{file_fingerprint, file_identity, file_title, media_kind_for_path, LibraryManager};
use crate::{
    errors::AppError,
    models::{
        LibraryBulkMoveFailure, LibraryBulkMovePlan, LibraryBulkMoveResult, LibraryChangeKind,
        LibraryFileOperation, LibraryFileOperationKind, LibraryFileOperationPlan,
        LibraryFileOperationResult, LibraryFileOperationState, LibraryItem, LibraryMediaKind,
        PreviewLibraryBulkMoveRequest, PreviewLibraryFileOperationRequest,
    },
};
use chrono::Utc;
use std::{
    collections::HashSet,
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
        let plan = self.create_file_operation_plan(request)?;
        self.database.save_library_file_operation(&plan.operation)?;
        Ok(plan)
    }

    fn create_file_operation_plan(
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
        if matches!(
            request.kind,
            LibraryFileOperationKind::Move | LibraryFileOperationKind::Rename
        ) {
            validate_move_filesystem(&source, &destination)?;
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

    pub fn preview_bulk_move(
        &self,
        request: PreviewLibraryBulkMoveRequest,
    ) -> Result<LibraryBulkMovePlan, AppError> {
        let item_ids = validate_bulk_ids(request.item_ids, "library items")?;
        let destination = validate_operation_directory(&request.destination_directory)?;
        let destination_directory = destination.to_string_lossy().into_owned();
        let mut plans = Vec::with_capacity(item_ids.len());
        let mut destinations = HashSet::with_capacity(item_ids.len());
        let mut total_size_bytes = 0_u64;
        for item_id in item_ids {
            let plan = self.create_file_operation_plan(PreviewLibraryFileOperationRequest {
                item_id,
                kind: LibraryFileOperationKind::Move,
                destination_directory: Some(destination_directory.clone()),
                new_file_name: None,
            })?;
            if !destinations.insert(plan.operation.destination_path.clone()) {
                return Err(AppError::new(
                    "library_bulk_move_name_conflict",
                    "Two selected files have the same filename. Rename one of them before moving this selection.",
                ));
            }
            total_size_bytes = total_size_bytes
                .checked_add(plan.operation.source_size_bytes)
                .ok_or_else(|| {
                    AppError::new(
                        "library_bulk_move_too_large",
                        "The selected files are too large to summarize safely.",
                    )
                })?;
            plans.push(plan);
        }
        for plan in &plans {
            self.database.save_library_file_operation(&plan.operation)?;
        }
        let mut warnings = vec![
            "All destinations were checked before this preview. Existing files are never overwritten."
                .to_owned(),
            "Move is limited to the same filesystem. Cross-filesystem selections remain unchanged."
                .to_owned(),
        ];
        if plans.iter().any(|plan| {
            plan.warnings
                .iter()
                .any(|warning| warning.contains("outside configured"))
        }) {
            warnings.push(
                "The destination is outside configured Library folders, so automatic source scans will not monitor these files."
                    .to_owned(),
            );
        }
        Ok(LibraryBulkMovePlan {
            operations: plans,
            destination_directory,
            total_size_bytes,
            warnings,
        })
    }

    pub async fn execute_bulk_move(
        &self,
        operation_ids: Vec<String>,
    ) -> Result<LibraryBulkMoveResult, AppError> {
        let operation_ids = validate_bulk_ids(operation_ids, "file operations")?;
        let mut operations = Vec::with_capacity(operation_ids.len());
        let mut item_ids = HashSet::with_capacity(operation_ids.len());
        let mut destinations = HashSet::with_capacity(operation_ids.len());
        let mut destination_parent: Option<PathBuf> = None;
        for operation_id in operation_ids {
            let operation = self.preflight_move_operation(&operation_id)?;
            if !item_ids.insert(operation.item_id.clone())
                || !destinations.insert(operation.destination_path.clone())
            {
                return Err(AppError::new(
                    "invalid_library_bulk_move",
                    "Each selected file and destination must appear only once in a bulk move.",
                ));
            }
            let parent = Path::new(&operation.destination_path)
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| {
                    AppError::new(
                        "invalid_library_destination",
                        "A planned destination folder is invalid.",
                    )
                })?;
            if destination_parent
                .as_ref()
                .is_some_and(|expected| expected != &parent)
            {
                return Err(AppError::new(
                    "invalid_library_bulk_move",
                    "All files in a bulk move must use the same destination folder.",
                ));
            }
            destination_parent = Some(parent);
            operations.push(operation);
        }

        let mut results = Vec::with_capacity(operations.len());
        let mut failures = Vec::new();
        for operation in operations {
            match self.execute_file_operation(&operation.id).await {
                Ok(result) => results.push(result),
                Err(error) => failures.push(LibraryBulkMoveFailure {
                    operation_id: operation.id,
                    item_id: operation.item_id,
                    code: error.code,
                    message: error.message,
                    details: error.details,
                }),
            }
        }
        Ok(LibraryBulkMoveResult { results, failures })
    }

    fn preflight_move_operation(
        &self,
        operation_id: &str,
    ) -> Result<LibraryFileOperation, AppError> {
        let operation = self.database.get_library_file_operation(operation_id)?;
        if operation.kind != LibraryFileOperationKind::Move
            || operation.state != LibraryFileOperationState::Planned
        {
            return Err(AppError::new(
                "invalid_library_bulk_move",
                "This bulk move contains an operation that is no longer ready.",
            ));
        }
        let item = self.database.get_library_item(&operation.item_id)?;
        let source = validate_current_item(&item)?;
        let destination = revalidate_plan(&operation, &source)?;
        validate_move_filesystem(&source, &destination)?;
        Ok(operation)
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
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        move_file_no_clobber_linux(source, destination, expected_fingerprint)
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        move_file_no_clobber_with_links(source, destination, expected_fingerprint)
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn move_file_no_clobber_linux(
    source: &Path,
    destination: &Path,
    expected_fingerprint: (u64, i64),
) -> Result<(), AppError> {
    use nix::{
        errno::Errno,
        fcntl::{renameat2, RenameFlags},
    };

    if !is_regular_file_with_fingerprint(source, expected_fingerprint) {
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed after preview. Nothing was moved.",
        ));
    }
    let source_identity = file_identity(source);
    match renameat2(
        None,
        source,
        None,
        destination,
        RenameFlags::RENAME_NOREPLACE,
    ) {
        Ok(()) => {}
        Err(Errno::EEXIST) => {
            return Err(AppError::new(
                "library_operation_conflict",
                "A file appeared at the destination. Nothing was overwritten.",
            ));
        }
        Err(Errno::EXDEV) => {
            return Err(AppError::new(
                "library_operation_cross_device",
                "This move crosses filesystems. Copy the file first, verify it, then move the original to Trash.",
            ));
        }
        Err(Errno::EINVAL | Errno::ENOSYS | Errno::EOPNOTSUPP | Errno::EPERM) => {
            return move_file_by_copy_no_clobber(source, destination, expected_fingerprint);
        }
        Err(error) => {
            return Err(AppError::internal(
                "could not move the file to its destination",
                io::Error::from_raw_os_error(error as i32),
            ));
        }
    }

    let destination_is_valid = fs::symlink_metadata(destination)
        .is_ok_and(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
        && file_fingerprint(destination) == Some(expected_fingerprint)
        && source_identity
            .zip(file_identity(destination))
            .is_none_or(|(expected, current)| expected == current);
    if destination_is_valid {
        return Ok(());
    }

    let rollback = renameat2(
        None,
        destination,
        None,
        source,
        RenameFlags::RENAME_NOREPLACE,
    );
    Err(AppError::with_details(
        "library_operation_stale_plan",
        "The source file changed while it was being moved. The app attempted to restore it.",
        rollback
            .err()
            .map(|error| format!("rollback failed: {error}"))
            .unwrap_or_else(|| "rollback succeeded".to_owned()),
    ))
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn move_file_by_copy_no_clobber(
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

    let mut destination_created = false;
    let result = (|| {
        let source_file = fs::File::open(source)
            .map_err(|error| AppError::internal("could not open the source file", error))?;
        let destination_file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|error| {
                if destination.exists() {
                    AppError::new(
                        "library_operation_conflict",
                        "A file appeared at the destination. Nothing was overwritten.",
                    )
                } else {
                    AppError::internal("could not create the destination file", error)
                }
            })?;
        destination_created = true;
        let mut reader = BufReader::with_capacity(1024 * 1024, source_file);
        let mut writer = BufWriter::with_capacity(1024 * 1024, destination_file);
        let copied = io::copy(&mut reader, &mut writer)
            .map_err(|error| AppError::internal("could not copy the media file", error))?;
        writer
            .flush()
            .map_err(|error| AppError::internal("could not flush the moved file", error))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|error| AppError::internal("could not sync the moved file", error))?;
        drop(writer);

        if copied != expected_fingerprint.0
            || !is_regular_file_with_fingerprint(source, expected_fingerprint)
            || fs::symlink_metadata(destination)
                .map(|metadata| {
                    !metadata.file_type().is_file()
                        || metadata.file_type().is_symlink()
                        || metadata.len() != expected_fingerprint.0
                })
                .unwrap_or(true)
        {
            return Err(AppError::new(
                "library_operation_stale_plan",
                "The source file changed while it was being moved. The incomplete destination was removed.",
            ));
        }

        if let Err(error) = fs::remove_file(source) {
            let rollback_error = fs::remove_file(destination).err();
            return Err(if let Some(rollback_error) = rollback_error {
                AppError::with_details(
                    "library_operation_cleanup_failed",
                    "The original could not be removed and the destination could not be rolled back. Run Verify files.",
                    format!("source removal: {error}; destination rollback: {rollback_error}"),
                )
            } else {
                AppError::internal("could not remove the original after copying it", error)
            });
        }
        Ok(())
    })();

    if result.is_err() && destination_created && source.exists() {
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn move_file_no_clobber_with_links(
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
            .zip(file_identity(destination))
            .is_none_or(|(expected, current)| expected == current);
    if !destination_is_valid {
        let _ = fs::remove_file(destination);
        return Err(AppError::new(
            "library_operation_stale_plan",
            "The source file changed while it was being moved. Nothing was removed.",
        ));
    }
    let source_parent = source.parent().ok_or_else(|| {
        AppError::new(
            "invalid_library_source",
            "The source file does not have a valid parent folder.",
        )
    })?;
    let claim = source_parent.join(format!(".nice-downloader-move-{}.claim", Uuid::new_v4()));
    if let Err(error) = fs::rename(source, &claim) {
        let rollback_error = fs::remove_file(destination).err();
        return Err(if let Some(rollback_error) = rollback_error {
            AppError::with_details(
                "library_operation_cleanup_failed",
                "The source could not be claimed and the destination could not be rolled back. Run Verify files.",
                format!("source claim: {error}; destination rollback: {rollback_error}"),
            )
        } else {
            AppError::internal(
                "could not remove the original file after linking its destination",
                error,
            )
        });
    }
    let claim_is_valid = is_regular_file_with_fingerprint(&claim, expected_fingerprint)
        && source_identity
            .zip(file_identity(&claim))
            .is_none_or(|(expected, current)| expected == current);
    if !claim_is_valid {
        let destination_rollback = fs::remove_file(destination).err();
        let restore_error = restore_claim_no_clobber(&claim, source).err();
        return Err(AppError::with_details(
            "library_operation_stale_plan",
            "The source path changed during the move. No unverified file was deleted.",
            format!(
                "claim: {}; destination rollback: {}; source restore: {}",
                claim.display(),
                destination_rollback
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "ok".to_owned()),
                restore_error
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "ok".to_owned())
            ),
        ));
    }
    if let Err(error) = fs::remove_file(&claim) {
        let restore_error = restore_claim_no_clobber(&claim, source).err();
        let destination_rollback = if restore_error.is_none() {
            fs::remove_file(destination).err()
        } else {
            None
        };
        return Err(AppError::with_details(
            "library_operation_cleanup_failed",
            "The verified source claim could not be removed, so Nice Downloader attempted to roll back the move.",
            format!(
                "claim {}: {error}; source restore: {}; destination rollback: {}",
                claim.display(),
                restore_error
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "ok".to_owned()),
                destination_rollback
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "ok".to_owned())
            ),
        ));
    }
    Ok(())
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn restore_claim_no_clobber(claim: &Path, source: &Path) -> io::Result<()> {
    fs::hard_link(claim, source)?;
    fs::remove_file(claim)
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

fn validate_move_filesystem(source: &Path, destination: &Path) -> Result<(), AppError> {
    let parent = destination.parent().ok_or_else(|| {
        AppError::new(
            "invalid_library_destination",
            "The destination folder is invalid.",
        )
    })?;
    if file_identity(source)
        .zip(file_identity(parent))
        .is_some_and(|(source, destination)| source.device_id != destination.device_id)
    {
        return Err(AppError::new(
            "library_operation_cross_device",
            "This move crosses filesystems. Copy the files first, verify them, then move the originals to Trash.",
        ));
    }
    Ok(())
}

fn validate_bulk_ids(ids: Vec<String>, label: &str) -> Result<Vec<String>, AppError> {
    if ids.is_empty() {
        return Err(AppError::new(
            "empty_library_selection",
            format!("Choose at least one {label} before continuing."),
        ));
    }
    if ids.len() > 500 {
        return Err(AppError::new(
            "library_selection_too_large",
            "Bulk file moves are limited to 500 items at a time.",
        ));
    }
    let mut unique = HashSet::with_capacity(ids.len());
    let mut normalized = Vec::with_capacity(ids.len());
    for id in ids {
        let id = Uuid::parse_str(&id)
            .map_err(|_| {
                AppError::new(
                    "invalid_library_selection",
                    format!("A selected {label} identifier is invalid."),
                )
            })?
            .to_string();
        if unique.insert(id.clone()) {
            normalized.push(id);
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::Database,
        models::{
            LibraryMetadataState, PreviewLibraryBulkMoveRequest, PreviewLibraryFileOperationRequest,
        },
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

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[test]
    fn copy_fallback_moves_without_hard_links() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        let destination = directory.path().join("destination.mp4");
        fs::write(&source, b"media fixture").expect("media fixture");
        let fingerprint = file_fingerprint(&source).expect("source fingerprint");

        move_file_by_copy_no_clobber(&source, &destination, fingerprint).expect("copy-backed move");

        assert!(!source.exists());
        assert_eq!(
            fs::read(&destination).expect("destination contents"),
            b"media fixture"
        );
    }

    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[test]
    fn copy_fallback_never_removes_an_existing_destination() {
        let directory = tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        let destination = directory.path().join("destination.mp4");
        fs::write(&source, b"source fixture").expect("source fixture");
        fs::write(&destination, b"existing fixture").expect("destination fixture");
        let fingerprint = file_fingerprint(&source).expect("source fingerprint");

        let error = move_file_by_copy_no_clobber(&source, &destination, fingerprint)
            .expect_err("destination conflict");

        assert_eq!(error.code, "library_operation_conflict");
        assert_eq!(
            fs::read(&source).expect("source remains"),
            b"source fixture"
        );
        assert_eq!(
            fs::read(&destination).expect("destination remains"),
            b"existing fixture"
        );
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

    #[tokio::test]
    async fn previews_and_executes_a_multi_file_move() {
        let directory = tempdir().expect("temporary directory");
        let source_directory = directory.path().join("source");
        let destination_directory = directory.path().join("destination");
        fs::create_dir(&source_directory).expect("source directory");
        fs::create_dir(&destination_directory).expect("destination directory");
        let first_path = source_directory.join("first.mp4");
        let second_path = source_directory.join("second.mp4");
        fs::write(&first_path, b"first fixture").expect("first fixture");
        fs::write(&second_path, b"second fixture").expect("second fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let first = catalog_fixture(&database, &first_path);
        let second = catalog_fixture(&database, &second_path);
        let manager = LibraryManager::new(database).expect("library manager");

        let plan = manager
            .preview_bulk_move(PreviewLibraryBulkMoveRequest {
                item_ids: vec![first.id, second.id],
                destination_directory: destination_directory.to_string_lossy().into_owned(),
            })
            .expect("bulk move preview");
        assert_eq!(plan.operations.len(), 2);
        assert_eq!(plan.total_size_bytes, 27);
        let result = manager
            .execute_bulk_move(
                plan.operations
                    .iter()
                    .map(|plan| plan.operation.id.clone())
                    .collect(),
            )
            .await
            .expect("bulk move");

        assert_eq!(result.results.len(), 2);
        assert!(result.failures.is_empty());
        assert!(!first_path.exists());
        assert!(!second_path.exists());
        assert!(destination_directory.join("first.mp4").exists());
        assert!(destination_directory.join("second.mp4").exists());
    }

    #[tokio::test]
    async fn bulk_move_preflight_prevents_partial_work_on_a_conflict() {
        let directory = tempdir().expect("temporary directory");
        let source_directory = directory.path().join("source");
        let destination_directory = directory.path().join("destination");
        fs::create_dir(&source_directory).expect("source directory");
        fs::create_dir(&destination_directory).expect("destination directory");
        let first_path = source_directory.join("first.mp4");
        let second_path = source_directory.join("second.mp4");
        fs::write(&first_path, b"first fixture").expect("first fixture");
        fs::write(&second_path, b"second fixture").expect("second fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let first = catalog_fixture(&database, &first_path);
        let second = catalog_fixture(&database, &second_path);
        let manager = LibraryManager::new(database).expect("library manager");
        let plan = manager
            .preview_bulk_move(PreviewLibraryBulkMoveRequest {
                item_ids: vec![first.id, second.id],
                destination_directory: destination_directory.to_string_lossy().into_owned(),
            })
            .expect("bulk move preview");
        fs::write(destination_directory.join("second.mp4"), b"conflict")
            .expect("destination conflict");

        let error = manager
            .execute_bulk_move(
                plan.operations
                    .iter()
                    .map(|plan| plan.operation.id.clone())
                    .collect(),
            )
            .await
            .expect_err("preflight conflict");
        assert_eq!(error.code, "library_operation_conflict");
        assert!(first_path.exists());
        assert!(second_path.exists());
        assert!(!destination_directory.join("first.mp4").exists());
    }

    #[test]
    fn bulk_move_rejects_duplicate_destination_filenames() {
        let directory = tempdir().expect("temporary directory");
        let first_directory = directory.path().join("one");
        let second_directory = directory.path().join("two");
        let destination_directory = directory.path().join("destination");
        fs::create_dir(&first_directory).expect("first directory");
        fs::create_dir(&second_directory).expect("second directory");
        fs::create_dir(&destination_directory).expect("destination directory");
        let first_path = first_directory.join("clip.mp4");
        let second_path = second_directory.join("clip.mp4");
        fs::write(&first_path, b"first").expect("first fixture");
        fs::write(&second_path, b"second").expect("second fixture");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let first = catalog_fixture(&database, &first_path);
        let second = catalog_fixture(&database, &second_path);
        let manager = LibraryManager::new(database).expect("library manager");

        let error = manager
            .preview_bulk_move(PreviewLibraryBulkMoveRequest {
                item_ids: vec![first.id, second.id],
                destination_directory: destination_directory.to_string_lossy().into_owned(),
            })
            .expect_err("duplicate destination");
        assert_eq!(error.code, "library_bulk_move_name_conflict");
    }
}
