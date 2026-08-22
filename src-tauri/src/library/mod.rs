mod operations;
use crate::models::{
    LibraryCurationState, LibraryMediaStack, LibraryMediaStackRole, LibraryMoment, LibraryMontage,
    LibraryPlaylist, SaveLibraryMediaStackRequest, SaveLibraryMomentRequest,
    SaveLibraryMontageRequest, SaveLibraryPlaylistRequest,
};

use crate::{
    database::{validate_library_metadata_fields, Database},
    engines::managed,
    errors::AppError,
    models::{
        LibraryArtworkCacheStatus, LibraryBulkAction, LibraryCacheResult, LibraryChangeKind,
        LibraryCollection, LibraryDuplicateAnalysisJob, LibraryDuplicateAnalysisState,
        LibraryDuplicateGroup, LibraryDuplicateTrashResult, LibraryFileIdentity, LibraryItem,
        LibraryMediaKind, LibraryMetadataFields, LibraryMetadataState, LibraryProbeResult,
        LibraryRichMetadata, LibraryScanJob, LibraryScanResult, LibraryScanState, LibrarySource,
        LibrarySourceAutomation, LibrarySourceStatus, LibraryTranscriptIndexStatus,
        SaveLibraryCollectionRequest, SaveLibraryRichMetadataRequest,
        SaveLibrarySourceAutomationRequest, SaveLibrarySourceRequest,
    },
    transcripts,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use notify::{EventKind, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs::OpenOptions,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    process::Command,
    sync::{mpsc, Mutex as AsyncMutex, Semaphore},
    task::JoinSet,
    time::{sleep, timeout, Duration},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const MAX_IMPORT_FILES: usize = 200;
const MAX_SCAN_ENTRIES: usize = 100_000;
const MAX_SCAN_WARNINGS: usize = 50;
const MAX_CONCURRENT_SCANS: usize = 2;
const MAX_CONCURRENT_METADATA_PROBES: usize = 3;
const WATCH_DEBOUNCE: Duration = Duration::from_secs(2);
const WATCH_SCAN_RETRY: Duration = Duration::from_secs(1);
const SOURCE_HEALTH_INTERVAL: Duration = Duration::from_secs(10);
const LIBRARY_BACKUP_VERSION: u32 = 7;
const LIBRARY_CATALOG_SCHEMA_VERSION: u32 = 1;
const LIBRARY_METADATA_SCHEMA_VERSION: u32 = 1;
const MAX_LIBRARY_BACKUP_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LIBRARY_BACKUP_ITEMS: usize = 100_000;
const MAX_LIBRARY_BACKUP_SOURCES: usize = 1_000;
const MAX_LIBRARY_BACKUP_COLLECTIONS: usize = 2_000;
const MAX_LIBRARY_BACKUP_PLAYLISTS: usize = 2_000;
const MAX_LIBRARY_BACKUP_STACKS: usize = 2_000;
const MAX_LIBRARY_BACKUP_MEMBERSHIPS: usize = 500_000;
const MAX_LIBRARY_BACKUP_MOMENTS: usize = 500_000;
const MAX_LIBRARY_BACKUP_MONTAGES: usize = 200;
const MAX_LIBRARY_BACKUP_MONTAGE_SEGMENTS: usize = 10_000;
const MAX_ARTWORK_CACHE_FILES: usize = 20_000;
const MAX_ARTWORK_CACHE_BYTES: u64 = 1024 * 1024 * 1024;
const SCAN_EVENT: &str = "library-scan-updated";
const ITEM_EVENT: &str = "library-item-indexed";
const SOURCE_EVENT: &str = "library-source-updated";
const CHANGE_EVENT: &str = "library-change-updated";
const DUPLICATE_EVENT: &str = "library-duplicate-analysis-updated";

#[derive(Clone)]
pub struct LibraryManager {
    database: Database,
    active_scans: Arc<Mutex<HashMap<String, (String, CancellationToken)>>>,
    active_watchers: Arc<Mutex<HashMap<String, (String, CancellationToken)>>>,
    active_duplicate_analysis: Arc<Mutex<Option<(String, CancellationToken)>>>,
    duplicate_job: Arc<Mutex<Option<LibraryDuplicateAnalysisJob>>>,
    duplicate_groups: Arc<Mutex<Vec<LibraryDuplicateGroup>>>,
    active_metadata: Arc<Mutex<HashSet<String>>>,
    artwork_slots: Arc<Semaphore>,
    metadata_slots: Arc<Semaphore>,
    artwork_locks: Arc<AsyncMutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
    artwork_cache_maintenance: Arc<AsyncMutex<()>>,
}

impl LibraryManager {
    pub fn mark_item_missing(
        &self,
        app: &AppHandle,
        item_id: &str,
    ) -> Result<LibraryItem, AppError> {
        let item = self.database.get_library_item(item_id)?;
        if item.missing {
            return Ok(item);
        }
        self.database.set_library_missing(item_id, true)?;
        let updated = self.database.get_library_item(item_id)?;
        self.record_change(app, &updated, LibraryChangeKind::Missing, None);
        let _ = app.emit(ITEM_EVENT, &updated);
        Ok(updated)
    }

    pub fn new(database: Database) -> Result<Self, AppError> {
        let interrupted_operations = database.reconcile_library_file_operations()?;
        if interrupted_operations > 0 {
            tracing::warn!(
                interrupted_operations,
                "marked interrupted library file operations for verification"
            );
        }
        for mut job in database.list_library_scan_jobs()? {
            if job.state == LibraryScanState::Running {
                job.state = LibraryScanState::Failed;
                job.warnings.push(
                    "The scan was interrupted when Nice Downloader closed. Start it again."
                        .to_owned(),
                );
                job.warning_count = job.warning_count.saturating_add(1);
                job.current_path = None;
                job.updated_at = Utc::now();
                job.completed_at = Some(job.updated_at);
                database.save_library_scan_job(&job)?;
                database.update_library_source_scan_status(
                    &job.source_id,
                    LibrarySourceStatus::Error,
                    Some("The previous scan was interrupted."),
                    false,
                )?;
            }
        }
        let duplicate_groups =
            duplicate_groups_from_hash_rows(database.list_current_library_content_hashes()?);
        Ok(Self {
            database,
            active_scans: Arc::new(Mutex::new(HashMap::new())),
            active_watchers: Arc::new(Mutex::new(HashMap::new())),
            active_duplicate_analysis: Arc::new(Mutex::new(None)),
            duplicate_job: Arc::new(Mutex::new(None)),
            duplicate_groups: Arc::new(Mutex::new(duplicate_groups)),
            active_metadata: Arc::new(Mutex::new(HashSet::new())),
            artwork_slots: Arc::new(Semaphore::new(2)),
            metadata_slots: Arc::new(Semaphore::new(3)),
            artwork_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            artwork_cache_maintenance: Arc::new(AsyncMutex::new(())),
        })
    }

    pub fn list_sources(&self) -> Result<Vec<LibrarySource>, AppError> {
        self.database.list_library_sources()
    }

    pub fn source_automation(&self, source_id: &str) -> Result<LibrarySourceAutomation, AppError> {
        self.database.get_library_source_automation(source_id)
    }

    pub fn save_source_automation(
        &self,
        request: SaveLibrarySourceAutomationRequest,
    ) -> Result<LibrarySourceAutomation, AppError> {
        self.database.save_library_source_automation(request)
    }

    pub fn list_items(&self, app: AppHandle) -> Result<Vec<LibraryItem>, AppError> {
        let items = self.database.list_library_items()?;
        self.schedule_metadata_batch(
            app,
            items
                .iter()
                .filter(|item| item.metadata_state == LibraryMetadataState::Pending)
                .map(|item| item.id.clone())
                .collect(),
            CancellationToken::new(),
        );
        Ok(items)
    }

    pub fn rebuild_transcript_index(&self) -> Result<LibraryTranscriptIndexStatus, AppError> {
        const BATCH_SIZE: usize = 500;
        let mut after_id = None;
        loop {
            let targets = self
                .database
                .list_library_transcript_targets_after(after_id.as_deref(), BATCH_SIZE)?;
            if targets.is_empty() {
                break;
            }
            for (item_id, path) in &targets {
                let cues = transcripts::read_transcript_cues(Path::new(path));
                self.database
                    .replace_library_transcript_cues(item_id, &cues)?;
            }
            after_id = targets.last().map(|(item_id, _)| item_id.clone());
        }
        self.database.library_transcript_index_status()
    }

    pub fn save_source(
        &self,
        mut request: SaveLibrarySourceRequest,
    ) -> Result<LibrarySource, AppError> {
        let existing_source = if let Some(id) = request.id.as_deref() {
            Uuid::parse_str(id).map_err(|_| {
                AppError::new(
                    "invalid_library_source",
                    "The library source identifier is invalid.",
                )
            })?;
            Some(self.database.get_library_source(id)?)
        } else {
            None
        };
        request.name = validate_source_name(&request.name)?;
        request.path = if existing_source
            .as_ref()
            .is_some_and(|source| source.path == request.path)
            && !Path::new(&request.path).exists()
        {
            request.path
        } else {
            validate_source_path(&request.path)?
        };
        request.include_patterns = normalize_patterns(request.include_patterns)?;
        request.exclude_patterns = normalize_patterns(request.exclude_patterns)?;
        let new_path = Path::new(&request.path);
        for source in self.database.list_library_sources()? {
            if Some(&source.id) == request.id.as_ref() {
                continue;
            }
            let existing_path = Path::new(&source.path);
            if new_path.starts_with(existing_path) || existing_path.starts_with(new_path) {
                return Err(AppError::new(
                    "library_source_overlaps",
                    "Choose a folder that is not already inside, or a parent of, another library source.",
                ));
            }
        }
        self.database.save_library_source(request)
    }

    pub fn save_source_and_sync_watcher(
        &self,
        app: AppHandle,
        request: SaveLibrarySourceRequest,
    ) -> Result<LibrarySource, AppError> {
        let source = self.save_source(request)?;
        self.restart_source_watcher(app, &source)?;
        Ok(source)
    }

    pub fn start_enabled_watchers(&self, app: AppHandle) {
        let sources = match self.database.list_library_sources() {
            Ok(sources) => sources,
            Err(error) => {
                tracing::warn!(%error, "could not load enabled library watchers");
                return;
            }
        };
        for source in sources.into_iter().filter(|source| source.watch_enabled) {
            if let Err(error) = self.start_source_watcher(app.clone(), source.clone()) {
                tracing::warn!(%error, source_id = %source.id, "could not start library watcher");
            }
        }
    }

    pub fn start_source_monitor(&self, app: AppHandle) {
        self.start_enabled_watchers(app.clone());
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                sleep(SOURCE_HEALTH_INTERVAL).await;
                manager.reconcile_source_watchers(&app);
            }
        });
    }

    fn reconcile_source_watchers(&self, app: &AppHandle) {
        let sources = match self.database.list_library_sources() {
            Ok(sources) => sources,
            Err(error) => {
                tracing::warn!(%error, "could not check library source availability");
                return;
            }
        };
        for source in sources {
            let watching = self
                .active_watchers
                .lock()
                .is_ok_and(|watchers| watchers.contains_key(&source.id));
            if !source.watch_enabled {
                if watching {
                    let _ = self.stop_source_watcher(&source.id);
                }
                continue;
            }
            if !Path::new(&source.path).is_dir() {
                if watching {
                    let _ = self.stop_source_watcher(&source.id);
                }
                if source.status != LibrarySourceStatus::Offline {
                    let _ = self.database.update_library_source_scan_status(
                        &source.id,
                        LibrarySourceStatus::Offline,
                        Some("The watched source folder is not currently available."),
                        false,
                    );
                    self.emit_source(app, &source.id);
                }
                continue;
            }
            if !watching {
                if let Err(error) = self.start_source_watcher(app.clone(), source.clone()) {
                    tracing::warn!(%error, source_id = %source.id, "could not restart library watcher");
                    continue;
                }
                let _ = self.database.update_library_source_scan_status(
                    &source.id,
                    LibrarySourceStatus::Online,
                    None,
                    false,
                );
                self.emit_source(app, &source.id);
                let manager = self.clone();
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    manager
                        .start_watched_scan(&app, &source.id, &CancellationToken::new())
                        .await;
                });
            }
        }
    }

    pub fn remove_source(&self, id: &str) -> Result<(), AppError> {
        if self
            .active_scans
            .lock()
            .map_err(|_| library_unavailable())?
            .values()
            .any(|(source_id, _)| source_id == id)
        {
            return Err(AppError::new(
                "library_source_scanning",
                "Cancel the active scan before removing this source.",
            ));
        }
        self.database.remove_library_source(id)?;
        self.stop_source_watcher(id)?;
        Ok(())
    }

    pub fn list_scan_jobs(&self) -> Result<Vec<LibraryScanJob>, AppError> {
        self.database.list_library_scan_jobs()
    }

    pub fn import_paths(
        &self,
        app: AppHandle,
        paths: Vec<String>,
        tags: Vec<String>,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let imported = import_paths(&self.database, paths, tags)?;
        self.schedule_metadata_batch(
            app,
            imported.iter().map(|item| item.id.clone()).collect(),
            CancellationToken::new(),
        );
        Ok(imported)
    }

    pub fn start_scan(&self, app: AppHandle, source_id: &str) -> Result<LibraryScanJob, AppError> {
        let source = self.database.get_library_source(source_id)?;
        let root = Path::new(&source.path);
        if !root.is_dir() {
            self.database.update_library_source_scan_status(
                source_id,
                LibrarySourceStatus::Offline,
                Some("The source folder is not currently available."),
                false,
            )?;
            return Err(AppError::new(
                "library_source_offline",
                "This source folder is not currently available. Existing catalog records were kept.",
            ));
        }
        let mut active = self
            .active_scans
            .lock()
            .map_err(|_| library_unavailable())?;
        if active.values().any(|(id, _)| id == source_id) {
            return Err(AppError::new(
                "library_scan_running",
                "This source is already being scanned.",
            ));
        }
        if active.len() >= MAX_CONCURRENT_SCANS {
            return Err(AppError::new(
                "library_scan_limit",
                "Two folders are already being scanned. Wait for one to finish or cancel it.",
            ));
        }
        let now = Utc::now();
        let job = LibraryScanJob {
            id: Uuid::new_v4().to_string(),
            source_id: source_id.to_owned(),
            state: LibraryScanState::Running,
            discovered_count: 0,
            indexed_count: 0,
            skipped_count: 0,
            warning_count: 0,
            current_path: None,
            warnings: vec![],
            started_at: now,
            updated_at: now,
            completed_at: None,
        };
        self.database.save_library_scan_job(&job)?;
        self.database.update_library_source_scan_status(
            source_id,
            LibrarySourceStatus::Scanning,
            None,
            false,
        )?;
        let token = CancellationToken::new();
        active.insert(job.id.clone(), (source_id.to_owned(), token.clone()));
        drop(active);

        let worker = self.clone();
        let cleanup = self.clone();
        let worker_job = job.clone();
        let job_id = job.id.clone();
        let worker_app = app.clone();
        tauri::async_runtime::spawn(async move {
            let result = tauri::async_runtime::spawn_blocking(move || {
                worker.run_scan(worker_app, source, worker_job, token)
            })
            .await;
            if let Err(error) = result {
                tracing::error!(%error, scan_id = %job_id, "library scan worker stopped unexpectedly");
                let _ =
                    cleanup.fail_crashed_scan(&app, &job_id, source_id_owned(&cleanup, &job_id));
            }
            if let Ok(mut active) = cleanup.active_scans.lock() {
                active.remove(&job_id);
            }
        });
        Ok(job)
    }

    pub fn cancel_scan(&self, job_id: &str) -> Result<(), AppError> {
        let token = self
            .active_scans
            .lock()
            .map_err(|_| library_unavailable())?
            .get(job_id)
            .map(|(_, token)| token.clone())
            .ok_or_else(|| {
                AppError::new(
                    "library_scan_not_running",
                    "This library scan is not running.",
                )
            })?;
        token.cancel();
        Ok(())
    }

    pub fn duplicate_analysis_job(&self) -> Result<Option<LibraryDuplicateAnalysisJob>, AppError> {
        self.duplicate_job
            .lock()
            .map(|job| job.clone())
            .map_err(|_| library_unavailable())
    }

    pub fn duplicate_groups(&self) -> Result<Vec<LibraryDuplicateGroup>, AppError> {
        self.duplicate_groups
            .lock()
            .map(|groups| groups.clone())
            .map_err(|_| library_unavailable())
    }

    pub async fn trash_duplicate_item(
        &self,
        group_id: &str,
        item_id: &str,
    ) -> Result<LibraryDuplicateTrashResult, AppError> {
        if group_id.len() != 64 || !group_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "invalid_duplicate_group",
                "The duplicate group is invalid. Analyze the library again.",
            ));
        }
        let groups =
            duplicate_groups_from_hash_rows(self.database.list_current_library_content_hashes()?);
        let group = groups
            .into_iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| {
                AppError::new(
                    "duplicate_group_stale",
                    "This duplicate group is no longer current. Analyze the library again.",
                )
            })?;
        if group.item_ids.len() < 2 || !group.item_ids.iter().any(|id| id == item_id) {
            return Err(AppError::new(
                "duplicate_item_not_found",
                "This file is no longer part of the duplicate group.",
            ));
        }
        let target = self.database.get_library_item(item_id)?;
        let sibling = group
            .item_ids
            .iter()
            .filter(|id| id.as_str() != item_id)
            .find_map(|id| {
                self.database
                    .get_library_item(id)
                    .ok()
                    .filter(|item| !item.missing && is_regular_non_symlink(Path::new(&item.path)))
            })
            .ok_or_else(|| {
                AppError::new(
                    "duplicate_copy_unavailable",
                    "No verified duplicate copy is currently available. Nothing was moved.",
                )
            })?;
        if target.missing {
            return Err(AppError::new(
                "library_file_not_found",
                "This duplicate file is no longer available.",
            ));
        }
        let target_path = PathBuf::from(&target.path);
        let sibling_path = PathBuf::from(&sibling.path);
        let target_fingerprint = target
            .file_size_bytes
            .zip(target.modified_at_ms)
            .ok_or_else(|| {
                AppError::new(
                    "duplicate_group_stale",
                    "The selected file no longer has a current fingerprint. Analyze again.",
                )
            })?;
        let sibling_fingerprint = sibling
            .file_size_bytes
            .zip(sibling.modified_at_ms)
            .ok_or_else(|| {
                AppError::new(
                    "duplicate_group_stale",
                    "The retained copy no longer has a current fingerprint. Analyze again.",
                )
            })?;
        let expected_hash = group.id.clone();
        let target_size = target.file_size_bytes.unwrap_or_default();
        tauri::async_runtime::spawn_blocking(move || {
            let token = CancellationToken::new();
            let target_identity = file_identity(&target_path);
            if file_fingerprint(&target_path) != Some(target_fingerprint)
                || file_fingerprint(&sibling_path) != Some(sibling_fingerprint)
            {
                return Err(duplicate_group_stale_error());
            }
            let target_hash = hash_regular_file(&target_path, &token)?;
            if file_fingerprint(&target_path) != Some(target_fingerprint) {
                return Err(duplicate_group_stale_error());
            }
            let sibling_hash = hash_regular_file(&sibling_path, &token)?;
            if target_hash != expected_hash
                || sibling_hash != expected_hash
                || file_fingerprint(&target_path) != Some(target_fingerprint)
                || file_fingerprint(&sibling_path) != Some(sibling_fingerprint)
            {
                return Err(duplicate_group_stale_error());
            }

            let parent = target_path.parent().ok_or_else(duplicate_group_stale_error)?;
            let claim_directory = parent.join(format!(
                ".nice-downloader-trash-{}",
                Uuid::new_v4()
            ));
            std::fs::create_dir(&claim_directory).map_err(|error| {
                AppError::internal("could not create duplicate trash claim", error)
            })?;
            let claim_path = claim_directory.join(
                target_path
                    .file_name()
                    .ok_or_else(duplicate_group_stale_error)?,
            );
            if let Err(error) = std::fs::rename(&target_path, &claim_path) {
                let _ = std::fs::remove_dir(&claim_directory);
                return Err(AppError::internal(
                    "could not claim duplicate before moving it to Trash",
                    error,
                ));
            }

            let claim_is_valid = file_fingerprint(&claim_path) == Some(target_fingerprint)
                && target_identity
                    .zip(file_identity(&claim_path))
                    .is_none_or(|(expected, current)| expected == current)
                && hash_regular_file(&claim_path, &token).is_ok_and(|hash| hash == expected_hash);
            if !claim_is_valid {
                let restore_error = restore_claimed_file(&claim_path, &target_path).err();
                let _ = std::fs::remove_dir(&claim_directory);
                return Err(AppError::with_details(
                    "duplicate_group_stale",
                    "The selected file changed before it could be moved to Trash. No unverified file was deleted.",
                    restore_error
                        .map(|error| error.to_string())
                        .unwrap_or_else(|| "source restored".to_owned()),
                ));
            }

            if let Err(error) = trash::delete(&claim_path) {
                let restore_error = restore_claimed_file(&claim_path, &target_path).err();
                let _ = std::fs::remove_dir(&claim_directory);
                return Err(AppError::with_details(
                    "trash_failed",
                    "The file could not be moved to the system Trash.",
                    format!(
                        "trash: {error}; restore: {}",
                        restore_error
                            .map(|error| error.to_string())
                            .unwrap_or_else(|| "ok".to_owned())
                    ),
                ));
            }
            if let Err(error) = std::fs::remove_dir(&claim_directory) {
                tracing::warn!(
                    %error,
                    path = %claim_directory.display(),
                    "could not remove duplicate trash claim directory"
                );
            }
            Ok(())
        })
        .await
        .map_err(|error| AppError::internal("duplicate trash task stopped", error))??;

        if let Err(error) = self.database.remove_library_item(item_id) {
            return Err(AppError::with_details(
                "trash_catalog_cleanup_failed",
                "The file was moved to Trash, but its catalog record could not be removed. Run Verify files in Library management.",
                error.details.unwrap_or(error.message),
            ));
        }
        let remaining_item_ids = group
            .item_ids
            .into_iter()
            .filter(|id| id != item_id)
            .collect::<Vec<_>>();
        let current_groups =
            duplicate_groups_from_hash_rows(self.database.list_current_library_content_hashes()?);
        *self
            .duplicate_groups
            .lock()
            .map_err(|_| library_unavailable())? = current_groups;
        Ok(LibraryDuplicateTrashResult {
            item_id: item_id.to_owned(),
            group_id: group_id.to_owned(),
            remaining_item_ids,
            moved_bytes: target_size,
        })
    }

    pub fn start_duplicate_analysis(
        &self,
        app: AppHandle,
    ) -> Result<LibraryDuplicateAnalysisJob, AppError> {
        let mut active = self
            .active_duplicate_analysis
            .lock()
            .map_err(|_| library_unavailable())?;
        if active.is_some() {
            return Err(AppError::new(
                "duplicate_analysis_running",
                "Duplicate analysis is already running.",
            ));
        }
        let now = Utc::now();
        let job = LibraryDuplicateAnalysisJob {
            id: Uuid::new_v4().to_string(),
            state: LibraryDuplicateAnalysisState::Running,
            examined_items: 0,
            candidate_items: 0,
            hashed_items: 0,
            skipped_items: 0,
            total_candidate_bytes: 0,
            hashed_bytes: 0,
            groups_found: 0,
            current_title: None,
            error: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
        };
        let token = CancellationToken::new();
        *active = Some((job.id.clone(), token.clone()));
        drop(active);
        *self
            .duplicate_job
            .lock()
            .map_err(|_| library_unavailable())? = Some(job.clone());

        let manager = self.clone();
        let worker_job = job.clone();
        tauri::async_runtime::spawn(async move {
            let worker = manager.clone();
            let worker_app = app.clone();
            let result = tauri::async_runtime::spawn_blocking(move || {
                worker.run_duplicate_analysis(&worker_app, worker_job, &token)
            })
            .await;
            if let Err(error) = result {
                tracing::error!(%error, "duplicate analysis worker stopped unexpectedly");
                let failed_job = manager
                    .duplicate_job
                    .lock()
                    .ok()
                    .and_then(|job| job.clone());
                if let Some(mut failed_job) = failed_job {
                    manager.finish_duplicate_analysis(
                        &app,
                        &mut failed_job,
                        Err(AppError::new(
                            "duplicate_analysis_failed",
                            "Duplicate analysis stopped unexpectedly.",
                        )),
                    );
                }
            }
            if let Ok(mut active) = manager.active_duplicate_analysis.lock() {
                *active = None;
            }
        });
        Ok(job)
    }

    pub fn cancel_duplicate_analysis(&self, job_id: &str) -> Result<(), AppError> {
        let token = self
            .active_duplicate_analysis
            .lock()
            .map_err(|_| library_unavailable())?
            .as_ref()
            .filter(|(id, _)| id == job_id)
            .map(|(_, token)| token.clone())
            .ok_or_else(|| {
                AppError::new(
                    "duplicate_analysis_not_running",
                    "Duplicate analysis is not running.",
                )
            })?;
        token.cancel();
        Ok(())
    }

    fn run_duplicate_analysis(
        &self,
        app: &AppHandle,
        mut job: LibraryDuplicateAnalysisJob,
        token: &CancellationToken,
    ) {
        let items = match self.database.list_library_items() {
            Ok(items) => items,
            Err(error) => {
                self.finish_duplicate_analysis(app, &mut job, Err(error));
                return;
            }
        };
        job.examined_items = items.len();
        let mut by_size: HashMap<u64, Vec<LibraryItem>> = HashMap::new();
        for item in items.into_iter().filter(|item| !item.missing) {
            if let (Some(size), Some(_)) = (item.file_size_bytes, item.modified_at_ms) {
                if size > 0 {
                    by_size.entry(size).or_default().push(item);
                }
            }
        }
        let candidates = by_size
            .into_values()
            .filter(|items| items.len() > 1)
            .flatten()
            .collect::<Vec<_>>();
        job.candidate_items = candidates.len();
        job.total_candidate_bytes = candidates
            .iter()
            .filter_map(|item| item.file_size_bytes)
            .fold(0u64, u64::saturating_add);
        self.emit_duplicate_job(app, &mut job);

        let cached = self
            .database
            .list_current_library_content_hashes()
            .unwrap_or_default()
            .into_iter()
            .map(|(item_id, _, hash)| (item_id, hash))
            .collect::<HashMap<_, _>>();
        let mut hashes: HashMap<String, Vec<(String, u64)>> = HashMap::new();
        for item in candidates {
            if token.is_cancelled() {
                self.finish_duplicate_analysis(app, &mut job, Ok(None));
                return;
            }
            job.current_title = Some(item.title.clone());
            let size = item.file_size_bytes.unwrap_or_default();
            let modified_at_ms = item.modified_at_ms.unwrap_or_default();
            let path = Path::new(&item.path);
            let expected_fingerprint = Some((size, modified_at_ms));
            let regular_file = std::fs::symlink_metadata(path).is_ok_and(|metadata| {
                metadata.file_type().is_file() && !metadata.file_type().is_symlink()
            });
            let hash = if !regular_file || file_fingerprint(path) != expected_fingerprint {
                None
            } else if let Some(hash) = cached.get(&item.id) {
                Some(hash.clone())
            } else {
                let before = file_fingerprint(path);
                match hash_regular_file(path, token) {
                    Ok(hash)
                        if before == expected_fingerprint && file_fingerprint(path) == before =>
                    {
                        if self
                            .database
                            .save_library_content_hash(&item.id, size, modified_at_ms, &hash)
                            .is_ok()
                        {
                            Some(hash)
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };
            if token.is_cancelled() {
                self.finish_duplicate_analysis(app, &mut job, Ok(None));
                return;
            }
            if let Some(hash) = hash {
                hashes.entry(hash).or_default().push((item.id, size));
                job.hashed_items += 1;
                job.hashed_bytes = job.hashed_bytes.saturating_add(size);
            } else {
                job.skipped_items += 1;
            }
            self.emit_duplicate_job(app, &mut job);
        }

        let rows = hashes
            .into_iter()
            .flat_map(|(hash, items)| {
                items
                    .into_iter()
                    .map(move |(item_id, size)| (item_id, size, hash.clone()))
            })
            .collect();
        let groups = duplicate_groups_from_hash_rows(rows);
        self.finish_duplicate_analysis(app, &mut job, Ok(Some(groups)));
    }

    fn finish_duplicate_analysis(
        &self,
        app: &AppHandle,
        job: &mut LibraryDuplicateAnalysisJob,
        result: Result<Option<Vec<LibraryDuplicateGroup>>, AppError>,
    ) {
        match result {
            Ok(Some(groups)) => {
                job.state = LibraryDuplicateAnalysisState::Completed;
                job.groups_found = groups.len();
                if let Ok(mut current) = self.duplicate_groups.lock() {
                    *current = groups;
                }
            }
            Ok(None) => job.state = LibraryDuplicateAnalysisState::Cancelled,
            Err(error) => {
                job.state = LibraryDuplicateAnalysisState::Failed;
                job.error = Some(error.message);
            }
        }
        job.current_title = None;
        job.updated_at = Utc::now();
        job.completed_at = Some(job.updated_at);
        self.emit_duplicate_job(app, job);
    }

    fn emit_duplicate_job(&self, app: &AppHandle, job: &mut LibraryDuplicateAnalysisJob) {
        job.updated_at = Utc::now();
        if let Ok(mut current) = self.duplicate_job.lock() {
            *current = Some(job.clone());
        }
        let _ = app.emit(DUPLICATE_EVENT, job.clone());
    }

    fn restart_source_watcher(
        &self,
        app: AppHandle,
        source: &LibrarySource,
    ) -> Result<(), AppError> {
        self.stop_source_watcher(&source.id)?;
        if source.watch_enabled {
            self.start_source_watcher(app, source.clone())?;
        }
        Ok(())
    }

    fn stop_source_watcher(&self, source_id: &str) -> Result<(), AppError> {
        let watcher = self
            .active_watchers
            .lock()
            .map_err(|_| library_unavailable())?
            .remove(source_id);
        if let Some((_, token)) = watcher {
            token.cancel();
        }
        Ok(())
    }

    fn start_source_watcher(&self, app: AppHandle, source: LibrarySource) -> Result<(), AppError> {
        if !Path::new(&source.path).is_dir() {
            self.database.update_library_source_scan_status(
                &source.id,
                LibrarySourceStatus::Offline,
                Some("The watched source folder is not currently available."),
                false,
            )?;
            self.emit_source(&app, &source.id);
            return Ok(());
        }
        let watcher_id = Uuid::new_v4().to_string();
        let token = CancellationToken::new();
        self.active_watchers
            .lock()
            .map_err(|_| library_unavailable())?
            .insert(source.id.clone(), (watcher_id.clone(), token.clone()));
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let source_id = source.id.clone();
            if let Err(error) = manager
                .run_source_watcher(app.clone(), source, token.clone())
                .await
            {
                tracing::warn!(%error, %source_id, "library source watcher stopped");
                let _ = manager.database.update_library_source_scan_status(
                    &source_id,
                    LibrarySourceStatus::Error,
                    Some("Automatic folder watching stopped. Re-enable it to try again."),
                    false,
                );
                manager.emit_source(&app, &source_id);
            }
            if let Ok(mut watchers) = manager.active_watchers.lock() {
                if watchers
                    .get(&source_id)
                    .is_some_and(|(active_id, _)| active_id == &watcher_id)
                {
                    watchers.remove(&source_id);
                }
            }
        });
        Ok(())
    }

    fn emit_source(&self, app: &AppHandle, source_id: &str) {
        if let Ok(source) = self.database.get_library_source(source_id) {
            if let Err(error) = app.emit(SOURCE_EVENT, source) {
                tracing::warn!(%error, source_id, "could not emit library source update");
            }
        }
    }

    async fn run_source_watcher(
        &self,
        app: AppHandle,
        source: LibrarySource,
        token: CancellationToken,
    ) -> Result<(), AppError> {
        let (events, mut receiver) = mpsc::unbounded_channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = events.send(event);
        })
        .map_err(|error| AppError::internal("could not initialize the folder watcher", error))?;
        watcher
            .watch(
                Path::new(&source.path),
                if source.recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                },
            )
            .map_err(|error| AppError::internal("could not watch the media folder", error))?;
        tracing::info!(source_id = %source.id, path = %source.path, "library source watcher started");

        loop {
            let first = tokio::select! {
                _ = token.cancelled() => break,
                event = receiver.recv() => event.ok_or_else(|| AppError::new(
                    "library_watcher_stopped",
                    "The folder watcher stopped unexpectedly.",
                ))?,
            };
            match first {
                Ok(event) if watch_event_should_rescan(&event.kind) => {}
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!(%error, source_id = %source.id, "library watcher reported an error");
                    continue;
                }
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = sleep(WATCH_DEBOUNCE) => {}
            }
            while let Ok(event) = receiver.try_recv() {
                if let Err(error) = event {
                    tracing::warn!(%error, source_id = %source.id, "library watcher reported an error");
                }
            }
            self.start_watched_scan(&app, &source.id, &token).await;
        }
        tracing::info!(source_id = %source.id, "library source watcher stopped");
        Ok(())
    }

    async fn start_watched_scan(
        &self,
        app: &AppHandle,
        source_id: &str,
        token: &CancellationToken,
    ) {
        loop {
            if token.is_cancelled() {
                return;
            }
            match self.start_scan(app.clone(), source_id) {
                Ok(_) => return,
                Err(error)
                    if error.code == "library_scan_running"
                        || error.code == "library_scan_limit" =>
                {
                    tokio::select! {
                        _ = token.cancelled() => return,
                        _ = sleep(WATCH_SCAN_RETRY) => {}
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, source_id, "could not start watched library scan");
                    return;
                }
            }
        }
    }

    pub async fn artwork_data_url(
        &self,
        app: &AppHandle,
        item_id: &str,
    ) -> Result<Option<String>, AppError> {
        Uuid::parse_str(item_id).map_err(|_| {
            AppError::new(
                "invalid_library_item",
                "The library item identifier is invalid.",
            )
        })?;
        let item = self.database.get_library_item(item_id)?;
        if item.missing {
            return Ok(None);
        }
        let media_path = PathBuf::from(&item.path);
        if !media_path.is_file() {
            self.mark_item_missing(app, item_id)?;
            return Ok(None);
        }
        let input = item
            .thumbnail
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.is_file())
            .unwrap_or_else(|| media_path.clone());
        if !input.is_file() {
            return Ok(None);
        }
        let cache_directory = artwork_cache_directory(app)?;
        std::fs::create_dir_all(&cache_directory)
            .map_err(|error| AppError::internal("could not create the artwork cache", error))?;
        let Some((input_size, input_modified_at)) = file_fingerprint(&input) else {
            return Ok(None);
        };
        // Keep the cache generation stable so an artwork-quality improvement does not
        // force a large existing library to synchronously regenerate every thumbnail.
        // Clearing the derived artwork cache remains the explicit way to refresh it.
        let output =
            cache_directory.join(format!("{item_id}-v1-{input_size}-{input_modified_at}.jpg"));
        let mut generated = false;
        if !cache_is_current(&input, &output) {
            let item_lock = {
                let mut locks = self.artwork_locks.lock().await;
                locks
                    .entry(item_id.to_owned())
                    .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                    .clone()
            };
            let _item_guard = item_lock.lock().await;
            let Some(ffmpeg) = managed::resolve("ffmpeg") else {
                return Ok(None);
            };
            let _permit = self
                .artwork_slots
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| library_unavailable())?;
            if !cache_is_current(&input, &output) {
                let source_kind = if input == media_path
                    && media_kind_for_path(&media_path) == Some(LibraryMediaKind::Video)
                {
                    ArtworkSourceKind::Video {
                        duration_seconds: item
                            .duration_seconds
                            .filter(|duration| duration.is_finite() && *duration > 0.1),
                    }
                } else {
                    ArtworkSourceKind::StillImage
                };
                generate_artwork(&ffmpeg, &input, &output, source_kind).await?;
                remove_stale_artwork(&cache_directory, item_id, &output);
                generated = true;
            }
        }
        if generated {
            self.enforce_artwork_cache(&cache_directory, Some(output.clone()))
                .await?;
        }
        touch_cache_entry(&output);
        let bytes = std::fs::read(&output)
            .map_err(|error| AppError::internal("could not read cached artwork", error))?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(AppError::new(
                "library_artwork_too_large",
                "The generated media artwork was unexpectedly large.",
            ));
        }
        Ok(Some(format!(
            "data:image/jpeg;base64,{}",
            BASE64.encode(bytes)
        )))
    }

    pub async fn clear_artwork_cache(
        &self,
        app: &AppHandle,
    ) -> Result<LibraryCacheResult, AppError> {
        let _maintenance = self.artwork_cache_maintenance.lock().await;
        let _permits = self
            .artwork_slots
            .clone()
            .acquire_many_owned(2)
            .await
            .map_err(|_| library_unavailable())?;
        let directory = artwork_cache_directory(app)?;
        tokio::task::spawn_blocking(move || clear_artwork_cache_directory(&directory))
            .await
            .map_err(|error| AppError::internal("artwork cache cleanup stopped", error))
    }

    pub async fn artwork_cache_status(
        &self,
        app: &AppHandle,
    ) -> Result<LibraryArtworkCacheStatus, AppError> {
        let directory = artwork_cache_directory(app)?;
        std::fs::create_dir_all(&directory)
            .map_err(|error| AppError::internal("could not create the artwork cache", error))?;
        self.enforce_artwork_cache(&directory, None).await
    }

    async fn enforce_artwork_cache(
        &self,
        directory: &Path,
        preserve: Option<PathBuf>,
    ) -> Result<LibraryArtworkCacheStatus, AppError> {
        let _maintenance = self.artwork_cache_maintenance.lock().await;
        let _permits = self
            .artwork_slots
            .clone()
            .acquire_many_owned(2)
            .await
            .map_err(|_| library_unavailable())?;
        let directory = directory.to_owned();
        tokio::task::spawn_blocking(move || {
            enforce_artwork_cache_limits(
                &directory,
                preserve.as_deref(),
                MAX_ARTWORK_CACHE_FILES,
                MAX_ARTWORK_CACHE_BYTES,
            )
        })
        .await
        .map_err(|error| AppError::internal("artwork cache maintenance stopped", error))?
    }

    fn run_scan(
        &self,
        app: AppHandle,
        source: LibrarySource,
        mut job: LibraryScanJob,
        token: CancellationToken,
    ) {
        let root = PathBuf::from(&source.path);
        let mut pending = VecDeque::from([root.clone()]);
        let mut seen = HashSet::new();
        let mut visited_entries = 0usize;
        let mut reconciliation_safe = true;
        let mut pending_metadata = Vec::new();

        while let Some(directory) = pending.pop_front() {
            if token.is_cancelled() {
                self.finish_scan(
                    &app,
                    &source,
                    &mut job,
                    LibraryScanState::Cancelled,
                    &seen,
                    false,
                );
                return;
            }
            let entries = match std::fs::read_dir(&directory) {
                Ok(entries) => entries,
                Err(error) => {
                    reconciliation_safe = false;
                    add_scan_warning(
                        &mut job,
                        format!(
                            "Could not read {}: {error}",
                            relative_display(&root, &directory)
                        ),
                    );
                    if directory == root {
                        self.finish_scan(
                            &app,
                            &source,
                            &mut job,
                            LibraryScanState::Failed,
                            &seen,
                            false,
                        );
                        return;
                    }
                    continue;
                }
            };
            for entry in entries {
                if token.is_cancelled() {
                    self.finish_scan(
                        &app,
                        &source,
                        &mut job,
                        LibraryScanState::Cancelled,
                        &seen,
                        false,
                    );
                    return;
                }
                visited_entries += 1;
                if visited_entries > MAX_SCAN_ENTRIES {
                    reconciliation_safe = false;
                    add_scan_warning(
                        &mut job,
                        format!("Stopped after {MAX_SCAN_ENTRIES} entries. Narrow this source or add exclude patterns."),
                    );
                    self.finish_scan(
                        &app,
                        &source,
                        &mut job,
                        LibraryScanState::Completed,
                        &seen,
                        reconciliation_safe,
                    );
                    self.schedule_metadata_batch(app.clone(), pending_metadata, token.clone());
                    return;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        reconciliation_safe = false;
                        add_scan_warning(&mut job, format!("Could not inspect an entry: {error}"));
                        continue;
                    }
                };
                let path = entry.path();
                let relative = path.strip_prefix(&root).unwrap_or(&path);
                if is_hidden(relative) {
                    job.skipped_count += 1;
                    continue;
                }
                let file_type = match entry.file_type() {
                    Ok(value) => value,
                    Err(error) => {
                        reconciliation_safe = false;
                        add_scan_warning(
                            &mut job,
                            format!("Could not inspect {}: {error}", relative.display()),
                        );
                        continue;
                    }
                };
                if file_type.is_symlink() {
                    job.skipped_count += 1;
                    continue;
                }
                if file_type.is_dir() {
                    if source.recursive {
                        pending.push_back(path);
                    }
                    continue;
                }
                if !file_type.is_file() {
                    job.skipped_count += 1;
                    continue;
                }
                job.discovered_count += 1;
                let relative_text = relative.to_string_lossy().replace('\\', "/");
                job.current_path = Some(relative_text.clone());
                let Some(kind) = media_kind_for_path(&path) else {
                    job.skipped_count += 1;
                    continue;
                };
                if !source_accepts_kind(source.media_kind, kind)
                    || !matches_patterns(
                        &relative_text,
                        &source.include_patterns,
                        &source.exclude_patterns,
                    )
                {
                    job.skipped_count += 1;
                    continue;
                }
                let item = scanned_item(&source, &path, relative_text, kind);
                match self.index_scanned_item(&app, &source, &item) {
                    Ok(stored) => {
                        seen.insert(stored.path.clone());
                        job.indexed_count += 1;
                        if stored.metadata_state == LibraryMetadataState::Pending {
                            pending_metadata.push(stored.id.clone());
                        }
                        let _ = app.emit(ITEM_EVENT, &stored);
                    }
                    Err(error) => add_scan_warning(
                        &mut job,
                        format!("Could not index {}: {}", relative.display(), error.message),
                    ),
                }
                if job.discovered_count.is_multiple_of(25) {
                    self.persist_and_emit(&app, &mut job);
                }
            }
        }
        self.finish_scan(
            &app,
            &source,
            &mut job,
            LibraryScanState::Completed,
            &seen,
            reconciliation_safe,
        );
        self.schedule_metadata_batch(app, pending_metadata, token);
    }

    fn index_scanned_item(
        &self,
        app: &AppHandle,
        source: &LibrarySource,
        item: &LibraryItem,
    ) -> Result<LibraryItem, AppError> {
        let identity = file_identity(Path::new(&item.path));
        let previous = self.database.get_library_item_by_path(&item.path)?;
        let is_new = previous.is_none();
        if previous.is_none() {
            if let Some(identity) = identity {
                let missing_candidates = self
                    .database
                    .find_library_items_by_identity(&source.id, identity)?
                    .into_iter()
                    .filter(|candidate| {
                        candidate.path != item.path && !Path::new(&candidate.path).exists()
                    })
                    .collect::<Vec<_>>();
                if let [candidate] = missing_candidates.as_slice() {
                    let previous_path = candidate.path.clone();
                    let stored = self
                        .database
                        .move_library_item_record(&candidate.id, item)?;
                    self.database
                        .save_library_file_identity(&stored.id, identity)?;
                    self.record_change(
                        app,
                        &stored,
                        LibraryChangeKind::Renamed,
                        Some(&previous_path),
                    );
                    self.refresh_transcript_index(&stored)?;
                    return Ok(stored);
                }
            }
        }

        let stored = self.database.upsert_scanned_library_item(item)?;
        if let Some(identity) = identity {
            self.database
                .save_library_file_identity(&stored.id, identity)?;
        }
        let change = match previous {
            None if source.last_scanned_at.is_some() => Some(LibraryChangeKind::Added),
            Some(previous) if previous.missing => Some(LibraryChangeKind::Restored),
            Some(previous)
                if previous.file_size_bytes != stored.file_size_bytes
                    || previous.modified_at_ms != stored.modified_at_ms =>
            {
                Some(LibraryChangeKind::Modified)
            }
            _ => None,
        };
        if let Some(change) = change {
            self.record_change(app, &stored, change, None);
        }
        let stored = if is_new {
            self.database
                .apply_library_source_automation(&source.id, &stored.id)?;
            self.database.get_library_item(&stored.id)?
        } else {
            stored
        };
        self.refresh_transcript_index(&stored)?;
        Ok(stored)
    }

    fn refresh_transcript_index(&self, item: &LibraryItem) -> Result<(), AppError> {
        if !matches!(
            item.media_kind,
            Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
        ) {
            return Ok(());
        }
        let cues = transcripts::read_transcript_cues(Path::new(&item.path));
        self.database
            .replace_library_transcript_cues(&item.id, &cues)
    }

    fn record_change(
        &self,
        app: &AppHandle,
        item: &LibraryItem,
        kind: LibraryChangeKind,
        previous_path: Option<&str>,
    ) {
        match self.database.record_library_change(
            &item.id,
            item.source_id.as_deref(),
            kind,
            previous_path,
        ) {
            Ok(change) => {
                let _ = app.emit(CHANGE_EVENT, change);
            }
            Err(error) => {
                tracing::warn!(%error, item_id = %item.id, "could not record library change")
            }
        }
    }

    fn schedule_metadata_batch(
        &self,
        app: AppHandle,
        item_ids: Vec<String>,
        token: CancellationToken,
    ) {
        if item_ids.is_empty() || token.is_cancelled() {
            return;
        }
        let item_ids = if let Ok(mut active) = self.active_metadata.lock() {
            item_ids
                .into_iter()
                .filter(|item_id| active.insert(item_id.clone()))
                .collect::<Vec<_>>()
        } else {
            return;
        };
        if item_ids.is_empty() {
            return;
        }
        let manager = self.clone();
        tauri::async_runtime::spawn(async move {
            let batch_ids = item_ids.clone();
            let mut item_ids = item_ids.into_iter();
            let mut tasks = JoinSet::new();
            for _ in 0..MAX_CONCURRENT_METADATA_PROBES {
                let Some(item_id) = item_ids.next() else {
                    break;
                };
                let worker = manager.clone();
                let worker_app = app.clone();
                let worker_token = token.clone();
                tasks.spawn(async move {
                    worker
                        .probe_and_release(&worker_app, &item_id, &worker_token)
                        .await;
                });
            }
            while tasks.join_next().await.is_some() {
                if token.is_cancelled() {
                    tasks.abort_all();
                    break;
                }
                let Some(item_id) = item_ids.next() else {
                    continue;
                };
                let worker = manager.clone();
                let worker_app = app.clone();
                let worker_token = token.clone();
                tasks.spawn(async move {
                    worker
                        .probe_and_release(&worker_app, &item_id, &worker_token)
                        .await;
                });
            }
            if let Ok(mut active) = manager.active_metadata.lock() {
                for item_id in batch_ids {
                    active.remove(&item_id);
                }
            }
        });
    }

    async fn probe_and_release(&self, app: &AppHandle, item_id: &str, token: &CancellationToken) {
        self.probe_library_item(app, item_id, token).await;
        if let Ok(mut active) = self.active_metadata.lock() {
            active.remove(item_id);
        }
    }

    async fn probe_library_item(&self, app: &AppHandle, item_id: &str, token: &CancellationToken) {
        if token.is_cancelled() {
            return;
        }
        let _permit = match self.metadata_slots.clone().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        let item = match self.database.get_library_item(item_id) {
            Ok(item) if !item.missing && item.metadata_state == LibraryMetadataState::Pending => {
                item
            }
            _ => return,
        };
        let path = PathBuf::from(&item.path);
        let ffprobe = managed::resolve("ffprobe");
        for attempt in 0..2 {
            let Some((file_size_bytes, modified_at_ms)) = file_fingerprint(&path) else {
                return;
            };
            if attempt == 0
                && (item.file_size_bytes != Some(file_size_bytes)
                    || item.modified_at_ms != Some(modified_at_ms))
            {
                match self
                    .database
                    .reset_library_probe(item_id, file_size_bytes, modified_at_ms)
                {
                    Ok(updated) => {
                        let _ = app.emit(ITEM_EVENT, updated);
                    }
                    Err(error) => {
                        tracing::warn!(%error, item_id, "could not refresh library fingerprint");
                        return;
                    }
                }
            }
            let result = if let Some(ffprobe) = ffprobe.as_deref() {
                probe_media(ffprobe, &path, file_size_bytes, modified_at_ms, token).await
            } else {
                LibraryProbeResult {
                    file_size_bytes,
                    modified_at_ms,
                    duration_seconds: None,
                    width: None,
                    height: None,
                    fps: None,
                    video_codec: None,
                    audio_codec: None,
                    container: None,
                    dynamic_range: None,
                    state: LibraryMetadataState::Unavailable,
                    error: Some(
                        "Install ffprobe in Settings > Engines to inspect this file.".to_owned(),
                    ),
                }
            };
            if token.is_cancelled() {
                return;
            }
            if file_fingerprint(&path) != Some((file_size_bytes, modified_at_ms)) {
                if let Some((next_size, next_modified)) = file_fingerprint(&path) {
                    if let Ok(updated) =
                        self.database
                            .reset_library_probe(item_id, next_size, next_modified)
                    {
                        let _ = app.emit(ITEM_EVENT, updated);
                    }
                }
                continue;
            }
            match self.database.save_library_probe_result(item_id, &result) {
                Ok(Some(updated)) => {
                    let _ = app.emit(ITEM_EVENT, updated);
                }
                Ok(None) => tracing::debug!(item_id, "discarded stale library metadata result"),
                Err(error) => {
                    tracing::warn!(%error, item_id, "could not persist library metadata")
                }
            }
            return;
        }
    }

    fn finish_scan(
        &self,
        app: &AppHandle,
        source: &LibrarySource,
        job: &mut LibraryScanJob,
        state: LibraryScanState,
        seen: &HashSet<String>,
        reconciliation_safe: bool,
    ) {
        if state == LibraryScanState::Completed && reconciliation_safe {
            if let Ok(items) = self.database.list_library_items_for_source(&source.id) {
                for item in items {
                    let missing = !seen.contains(&item.path);
                    if missing != item.missing
                        && self.database.set_library_missing(&item.id, missing).is_ok()
                    {
                        if let Ok(updated) = self.database.get_library_item(&item.id) {
                            if missing {
                                self.record_change(app, &updated, LibraryChangeKind::Missing, None);
                            }
                            let _ = app.emit(ITEM_EVENT, updated);
                        }
                    }
                }
            }
        }
        job.state = state;
        job.current_path = None;
        job.updated_at = Utc::now();
        job.completed_at = Some(job.updated_at);
        let completed = state == LibraryScanState::Completed;
        let status = if state == LibraryScanState::Failed {
            LibrarySourceStatus::Error
        } else {
            LibrarySourceStatus::Online
        };
        let error = (state == LibraryScanState::Failed)
            .then(|| job.warnings.last().cloned())
            .flatten();
        let _ = self.database.update_library_source_scan_status(
            &source.id,
            status,
            error.as_deref(),
            completed,
        );
        self.persist_and_emit(app, job);
    }

    fn persist_and_emit(&self, app: &AppHandle, job: &mut LibraryScanJob) {
        job.updated_at = Utc::now();
        if let Err(error) = self.database.save_library_scan_job(job) {
            tracing::warn!(%error, scan_id = %job.id, "could not persist library scan progress");
        }
        if let Err(error) = app.emit(SCAN_EVENT, job.clone()) {
            tracing::warn!(%error, scan_id = %job.id, "could not emit library scan progress");
        }
    }

    fn fail_crashed_scan(
        &self,
        app: &AppHandle,
        job_id: &str,
        source_id: Option<String>,
    ) -> Result<(), AppError> {
        let Some(mut job) = self
            .database
            .list_library_scan_jobs()?
            .into_iter()
            .find(|job| job.id == job_id)
        else {
            return Ok(());
        };
        job.state = LibraryScanState::Failed;
        add_scan_warning(&mut job, "The scan worker stopped unexpectedly.".to_owned());
        job.current_path = None;
        job.updated_at = Utc::now();
        job.completed_at = Some(job.updated_at);
        self.database.save_library_scan_job(&job)?;
        if let Some(source_id) = source_id {
            self.database.update_library_source_scan_status(
                &source_id,
                LibrarySourceStatus::Error,
                Some("The scan worker stopped unexpectedly."),
                false,
            )?;
        }
        let _ = app.emit(SCAN_EVENT, job);
        Ok(())
    }
}

fn source_id_owned(manager: &LibraryManager, job_id: &str) -> Option<String> {
    manager
        .active_scans
        .lock()
        .ok()?
        .get(job_id)
        .map(|(source_id, _)| source_id.clone())
}

fn watch_event_should_rescan(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any
    )
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
    format: Option<FfprobeFormat>,
}

#[derive(Debug, Deserialize)]
struct FfprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    duration: Option<String>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    color_transfer: Option<String>,
    #[serde(default)]
    side_data_list: Vec<Value>,
}

async fn probe_media(
    ffprobe: &Path,
    path: &Path,
    file_size_bytes: u64,
    modified_at_ms: i64,
    token: &CancellationToken,
) -> LibraryProbeResult {
    let failed = |message: String| LibraryProbeResult {
        file_size_bytes,
        modified_at_ms,
        duration_seconds: None,
        width: None,
        height: None,
        fps: None,
        video_codec: None,
        audio_codec: None,
        container: None,
        dynamic_range: None,
        state: LibraryMetadataState::Failed,
        error: Some(message),
    };
    let mut command = Command::new(ffprobe);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-v")
        .arg("error")
        .arg("-print_format")
        .arg("json")
        .arg("-show_format")
        .arg("-show_streams")
        .arg(path)
        .kill_on_drop(true);
    let output = tokio::select! {
        _ = token.cancelled() => return failed("Media inspection was cancelled.".to_owned()),
        result = timeout(Duration::from_secs(15), command.output()) => match result {
            Err(_) => return failed("ffprobe took longer than 15 seconds to inspect this file.".to_owned()),
            Ok(Err(error)) => return failed(format!("Could not start ffprobe: {error}")),
            Ok(Ok(output)) => output,
        }
    };
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr)
            .trim()
            .chars()
            .take(320)
            .collect::<String>();
        return failed(if diagnostic.is_empty() {
            "ffprobe could not inspect this media file.".to_owned()
        } else {
            format!("ffprobe could not inspect this file: {diagnostic}")
        });
    }
    let parsed: FfprobeOutput = match serde_json::from_slice(&output.stdout) {
        Ok(value) => value,
        Err(error) => return failed(format!("ffprobe returned invalid metadata: {error}")),
    };
    let video = parsed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"));
    let audio = parsed
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("audio"));
    let duration_seconds = parsed
        .format
        .as_ref()
        .and_then(|format| parse_positive_number(format.duration.as_deref()))
        .or_else(|| video.and_then(|stream| parse_positive_number(stream.duration.as_deref())))
        .or_else(|| audio.and_then(|stream| parse_positive_number(stream.duration.as_deref())));
    let fps = video.and_then(|stream| {
        parse_frame_rate(stream.avg_frame_rate.as_deref())
            .or_else(|| parse_frame_rate(stream.r_frame_rate.as_deref()))
    });
    let container = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .or_else(|| {
            parsed
                .format
                .as_ref()
                .and_then(|format| format.format_name.as_deref())
                .and_then(|value| value.split(',').next())
                .map(str::to_owned)
        });
    LibraryProbeResult {
        file_size_bytes,
        modified_at_ms,
        duration_seconds,
        width: video.and_then(|stream| stream.width),
        height: video.and_then(|stream| stream.height),
        fps,
        video_codec: video.and_then(|stream| stream.codec_name.clone()),
        audio_codec: audio.and_then(|stream| stream.codec_name.clone()),
        container,
        dynamic_range: video.and_then(dynamic_range_for_stream),
        state: LibraryMetadataState::Ready,
        error: None,
    }
}

fn parse_positive_number(value: Option<&str>) -> Option<f64> {
    value?
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && *number >= 0.0)
}

fn parse_frame_rate(value: Option<&str>) -> Option<f64> {
    let value = value?;
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<f64>().ok()?;
    let denominator = denominator.parse::<f64>().ok()?;
    (denominator > 0.0)
        .then_some(numerator / denominator)
        .filter(|fps| fps.is_finite() && *fps > 0.0 && *fps < 1000.0)
}

fn dynamic_range_for_stream(stream: &FfprobeStream) -> Option<String> {
    let side_data = serde_json::to_string(&stream.side_data_list)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if side_data.contains("dovi") || side_data.contains("dolby vision") {
        return Some("Dolby Vision".to_owned());
    }
    match stream.color_transfer.as_deref() {
        Some("smpte2084") => Some("HDR10 / PQ".to_owned()),
        Some("arib-std-b67") => Some("HLG".to_owned()),
        _ => None,
    }
}

fn file_fingerprint(path: &Path) -> Option<(u64, i64)> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_at_ms = metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())?;
    Some((metadata.len(), modified_at_ms))
}

fn is_regular_non_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
}

fn restore_claimed_file(claim: &Path, original: &Path) -> std::io::Result<()> {
    std::fs::hard_link(claim, original)?;
    std::fs::remove_file(claim)
}

#[cfg(unix)]
fn file_identity(path: &Path) -> Option<LibraryFileIdentity> {
    use std::os::unix::fs::MetadataExt;

    let metadata = std::fs::metadata(path).ok()?;
    Some(LibraryFileIdentity {
        device_id: metadata.dev(),
        file_id: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn file_identity(_path: &Path) -> Option<LibraryFileIdentity> {
    None
}

fn hash_regular_file(path: &Path, token: &CancellationToken) -> Result<String, AppError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| AppError::internal("could not inspect a duplicate candidate", error))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(AppError::new(
            "invalid_duplicate_candidate",
            "A duplicate candidate is not a regular file.",
        ));
    }
    let file = std::fs::File::open(path)
        .map_err(|error| AppError::internal("could not open a duplicate candidate", error))?;
    let mut reader = BufReader::with_capacity(1024 * 1024, file);
    let mut buffer = vec![0u8; 1024 * 1024];
    let mut hasher = Sha256::new();
    loop {
        if token.is_cancelled() {
            return Err(AppError::new(
                "duplicate_analysis_cancelled",
                "Duplicate analysis was cancelled.",
            ));
        }
        let read = reader
            .read(&mut buffer)
            .map_err(|error| AppError::internal("could not read a duplicate candidate", error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn duplicate_group_stale_error() -> AppError {
    AppError::new(
        "duplicate_group_stale",
        "A file changed after duplicate analysis. Nothing was moved; analyze again.",
    )
}

fn duplicate_groups_from_hash_rows(rows: Vec<(String, u64, String)>) -> Vec<LibraryDuplicateGroup> {
    let mut by_hash: HashMap<String, Vec<(String, u64)>> = HashMap::new();
    for (item_id, size, hash) in rows {
        by_hash.entry(hash).or_default().push((item_id, size));
    }
    let mut groups = by_hash
        .into_iter()
        .filter_map(|(hash, items)| {
            if items.len() < 2 {
                return None;
            }
            let file_size_bytes = items[0].1;
            let item_ids = items
                .into_iter()
                .map(|(item_id, _)| item_id)
                .collect::<Vec<_>>();
            let potential_reclaim_bytes =
                file_size_bytes.saturating_mul(item_ids.len().saturating_sub(1) as u64);
            Some(LibraryDuplicateGroup {
                id: hash,
                item_ids,
                file_size_bytes,
                potential_reclaim_bytes,
            })
        })
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| {
        right
            .potential_reclaim_bytes
            .cmp(&left.potential_reclaim_bytes)
    });
    groups
}

fn artwork_cache_directory(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .app_cache_dir()
        .map(|path| path.join("library-artwork"))
        .map_err(|error| AppError::internal("could not resolve the artwork cache", error))
}

#[derive(Debug)]
struct ArtworkCacheEntry {
    path: PathBuf,
    size: u64,
    last_used: SystemTime,
}

fn enforce_artwork_cache_limits(
    directory: &Path,
    preserve: Option<&Path>,
    max_files: usize,
    max_bytes: u64,
) -> Result<LibraryArtworkCacheStatus, AppError> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LibraryArtworkCacheStatus {
                file_count: 0,
                total_bytes: 0,
                max_files,
                max_bytes,
            });
        }
        Err(error) => {
            return Err(AppError::internal(
                "could not inspect the artwork cache",
                error,
            ));
        }
    };
    let mut cached = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            Some(ArtworkCacheEntry {
                path: entry.path(),
                size: metadata.len(),
                last_used: metadata.modified().unwrap_or(UNIX_EPOCH),
            })
        })
        .collect::<Vec<_>>();
    cached.sort_by_key(|entry| entry.last_used);
    let mut file_count = cached.len();
    let mut total_bytes = cached
        .iter()
        .fold(0u64, |total, entry| total.saturating_add(entry.size));
    for entry in cached {
        if file_count <= max_files && total_bytes <= max_bytes {
            break;
        }
        if preserve.is_some_and(|preserve| preserve == entry.path) {
            continue;
        }
        if std::fs::remove_file(&entry.path).is_ok() {
            file_count = file_count.saturating_sub(1);
            total_bytes = total_bytes.saturating_sub(entry.size);
        }
    }
    Ok(LibraryArtworkCacheStatus {
        file_count,
        total_bytes,
        max_files,
        max_bytes,
    })
}

fn clear_artwork_cache_directory(directory: &Path) -> LibraryCacheResult {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return LibraryCacheResult {
            removed_files: 0,
            freed_bytes: 0,
        };
    };
    let mut removed_files = 0usize;
    let mut freed_bytes = 0u64;
    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        if std::fs::remove_file(entry.path()).is_ok() {
            removed_files += 1;
            freed_bytes = freed_bytes.saturating_add(size);
        }
    }
    LibraryCacheResult {
        removed_files,
        freed_bytes,
    }
}

fn touch_cache_entry(path: &Path) {
    if let Ok(file) = OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn remove_stale_artwork(directory: &Path, item_id: &str, current: &Path) {
    let prefix = format!("{item_id}-");
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path != current
            && path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".jpg"))
        {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn cache_is_current(input: &Path, output: &Path) -> bool {
    let Ok(output_metadata) = std::fs::metadata(output) else {
        return false;
    };
    if output_metadata.len() == 0 {
        return false;
    }
    let Ok(input_modified) = std::fs::metadata(input).and_then(|value| value.modified()) else {
        return true;
    };
    output_metadata
        .modified()
        .is_ok_and(|output_modified| output_modified >= input_modified)
}

const ARTWORK_FRAME_CANDIDATES: u8 = 8;
const ARTWORK_SAMPLE_WINDOW_SECONDS: f64 = 8.0;

#[derive(Debug, Clone, Copy)]
enum ArtworkSourceKind {
    StillImage,
    Video { duration_seconds: Option<f64> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ArtworkSamplingWindow {
    start_seconds: f64,
    duration_seconds: f64,
    frames_per_second: f64,
}

fn artwork_sampling_window(duration_seconds: f64) -> Option<ArtworkSamplingWindow> {
    if !duration_seconds.is_finite() || duration_seconds <= 0.1 {
        return None;
    }
    let start_seconds = duration_seconds * 0.08;
    let end_seconds = duration_seconds * 0.94;
    let sample_duration = (end_seconds - start_seconds).min(ARTWORK_SAMPLE_WINDOW_SECONDS);
    if sample_duration <= 0.05 {
        return None;
    }
    Some(ArtworkSamplingWindow {
        start_seconds,
        duration_seconds: sample_duration,
        frames_per_second: f64::from(ARTWORK_FRAME_CANDIDATES) / sample_duration,
    })
}

async fn generate_artwork(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    source_kind: ArtworkSourceKind,
) -> Result<(), AppError> {
    let output_parent = output.parent().ok_or_else(|| {
        AppError::new(
            "library_artwork_cache_unavailable",
            "The artwork cache location is unavailable.",
        )
    })?;
    let staging = output_parent.join(format!(".artwork-{}.part.jpg", Uuid::new_v4()));
    let mut command = Command::new(ffmpeg);
    crate::utils::process::sanitize_tokio_command(&mut command);
    command
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin");
    let sampling = match source_kind {
        ArtworkSourceKind::StillImage => None,
        ArtworkSourceKind::Video {
            duration_seconds: Some(duration_seconds),
        } => artwork_sampling_window(duration_seconds),
        ArtworkSourceKind::Video {
            duration_seconds: None,
        } => {
            command.arg("-ss").arg("1");
            None
        }
    };
    if let Some(sampling) = sampling {
        command
            .arg("-ss")
            .arg(format!("{:.6}", sampling.start_seconds));
    }
    command.arg("-i").arg(input);
    if let Some(sampling) = sampling {
        command
            .arg("-t")
            .arg(format!("{:.6}", sampling.duration_seconds));
    }
    let video_filter = if let Some(sampling) = sampling {
        format!(
            "fps={:.8},thumbnail={},scale=640:-2:force_original_aspect_ratio=decrease",
            sampling.frames_per_second, ARTWORK_FRAME_CANDIDATES
        )
    } else {
        "scale=640:-2:force_original_aspect_ratio=decrease".to_owned()
    };
    command
        .arg("-frames:v")
        .arg("1")
        .arg("-vf")
        .arg(video_filter)
        .arg("-q:v")
        .arg("4")
        .arg("-y")
        .arg(&staging)
        .kill_on_drop(true);
    let result = timeout(Duration::from_secs(20), command.output())
        .await
        .map_err(|_| {
            let _ = std::fs::remove_file(&staging);
            AppError::new(
                "library_artwork_timeout",
                "Creating artwork for this media item took too long.",
            )
        })?
        .map_err(|error| {
            let _ = std::fs::remove_file(&staging);
            AppError::internal("could not start FFmpeg for library artwork", error)
        })?;
    if !result.status.success() || !staging.is_file() {
        let _ = std::fs::remove_file(&staging);
        return Err(AppError::new(
            "library_artwork_failed",
            "FFmpeg could not create artwork for this media item.",
        ));
    }
    if let Err(first_error) = std::fs::rename(&staging, output) {
        if !output.exists() {
            let _ = std::fs::remove_file(&staging);
            return Err(AppError::internal(
                "could not publish cached artwork",
                first_error,
            ));
        }
        std::fs::remove_file(output).map_err(|error| {
            let _ = std::fs::remove_file(&staging);
            AppError::internal("could not replace cached artwork", error)
        })?;
        std::fs::rename(&staging, output).map_err(|error| {
            let _ = std::fs::remove_file(&staging);
            AppError::internal("could not publish cached artwork", error)
        })?;
    }
    Ok(())
}

fn validate_source_name(value: &str) -> Result<String, AppError> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_source_name",
            "Use a source name between 1 and 80 characters.",
        ));
    }
    Ok(name.to_owned())
}

fn validate_source_path(value: &str) -> Result<String, AppError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(AppError::new(
            "invalid_library_source_path",
            "Choose an absolute folder path for this source.",
        ));
    }
    let canonical = path.canonicalize().map_err(|_| {
        AppError::new(
            "invalid_library_source_path",
            "The selected source folder does not exist or cannot be accessed.",
        )
    })?;
    if !canonical.is_dir() {
        return Err(AppError::new(
            "invalid_library_source_path",
            "Only folders can be added as library sources.",
        ));
    }
    if canonical.parent().is_none() {
        return Err(AppError::new(
            "invalid_library_source_path",
            "Choose a media folder instead of an entire filesystem root.",
        ));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

fn normalize_patterns(patterns: Vec<String>) -> Result<Vec<String>, AppError> {
    if patterns.len() > 20 {
        return Err(AppError::new(
            "invalid_library_patterns",
            "Use no more than 20 include or exclude patterns.",
        ));
    }
    let mut normalized = Vec::new();
    for pattern in patterns {
        let pattern = pattern.trim().replace('\\', "/");
        if pattern.is_empty() {
            continue;
        }
        if pattern.chars().count() > 120 || pattern.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_library_patterns",
                "Patterns must be at most 120 characters and cannot contain control characters.",
            ));
        }
        if !normalized.contains(&pattern) {
            normalized.push(pattern);
        }
    }
    Ok(normalized)
}

fn scanned_item(
    source: &LibrarySource,
    path: &Path,
    relative_path: String,
    kind: LibraryMediaKind,
) -> LibraryItem {
    let sidecar = read_sidecar(path);
    let file_metadata = std::fs::metadata(path).ok();
    let modified_at_ms = file_metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|value| i64::try_from(value.as_millis()).ok());
    let mut metadata = sidecar
        .as_ref()
        .map(safe_sidecar_metadata)
        .unwrap_or_else(|| json!({}));
    if let Some(object) = metadata.as_object_mut() {
        object.insert(
            "fileSize".to_owned(),
            file_metadata
                .as_ref()
                .map(|value| json!(value.len()))
                .unwrap_or(Value::Null),
        );
        object.insert(
            "modifiedAtMs".to_owned(),
            modified_at_ms
                .map(|value| json!(value))
                .unwrap_or(Value::Null),
        );
    }
    let now = Utc::now();
    LibraryItem {
        id: Uuid::new_v4().to_string(),
        path: path.to_string_lossy().into_owned(),
        source_id: Some(source.id.clone()),
        relative_path: Some(relative_path),
        media_kind: Some(kind),
        title: sidecar
            .as_ref()
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| file_title(path)),
        source_url: sidecar
            .as_ref()
            .and_then(|value| value.get("webpage_url"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        thumbnail: if kind == LibraryMediaKind::Image {
            Some(path.to_string_lossy().into_owned())
        } else {
            find_local_thumbnail(path)
        },
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
        tags: vec![],
        metadata,
        file_size_bytes: file_metadata.as_ref().map(std::fs::Metadata::len),
        modified_at_ms,
        width: None,
        height: None,
        fps: None,
        video_codec: None,
        audio_codec: None,
        container: None,
        dynamic_range: None,
        metadata_state: LibraryMetadataState::Pending,
        metadata_error: None,
        favorite: false,
        rating: None,
        missing: false,
        added_at: now,
        updated_at: now,
    }
}

fn media_kind_for_path(path: &Path) -> Option<LibraryMediaKind> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v"
    ) {
        Some(LibraryMediaKind::Video)
    } else if matches!(
        extension.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif"
    ) {
        Some(LibraryMediaKind::Image)
    } else {
        None
    }
}

fn source_accepts_kind(configured: LibraryMediaKind, actual: LibraryMediaKind) -> bool {
    configured == LibraryMediaKind::Both || configured == actual
}

fn is_hidden(relative: &Path) -> bool {
    relative.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with('.') && name != "." && name != "..")
    })
}

fn matches_patterns(path: &str, includes: &[String], excludes: &[String]) -> bool {
    let path = path.to_ascii_lowercase();
    let included = includes.is_empty()
        || includes
            .iter()
            .any(|pattern| wildcard_matches(&pattern.to_ascii_lowercase(), &path));
    included
        && !excludes
            .iter()
            .any(|pattern| wildcard_matches(&pattern.to_ascii_lowercase(), &path))
}

fn wildcard_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let (mut pattern_index, mut text_index) = (0usize, 0usize);
    let (mut star, mut star_text) = (None, 0usize);
    while text_index < text.len() {
        if pattern_index < pattern.len()
            && (pattern[pattern_index] == b'?' || pattern[pattern_index] == text[text_index])
        {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
            star = Some(pattern_index);
            pattern_index += 1;
            star_text = text_index;
        } else if let Some(star_index) = star {
            pattern_index = star_index + 1;
            star_text += 1;
            text_index = star_text;
        } else {
            return false;
        }
    }
    while pattern_index < pattern.len() && pattern[pattern_index] == b'*' {
        pattern_index += 1;
    }
    pattern_index == pattern.len()
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn add_scan_warning(job: &mut LibraryScanJob, warning: String) {
    job.warning_count = job.warning_count.saturating_add(1);
    if job.warnings.len() < MAX_SCAN_WARNINGS {
        job.warnings.push(warning);
    }
}

fn library_unavailable() -> AppError {
    AppError::new(
        "library_unavailable",
        "The media library is temporarily unavailable.",
    )
}

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
        let (file_size_bytes, modified_at_ms) = file_fingerprint(&path).unwrap_or((0, 0));
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: media_kind_for_path(&path),
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
            file_size_bytes: Some(file_size_bytes),
            modified_at_ms: Some(modified_at_ms),
            width: None,
            height: None,
            fps: None,
            video_codec: None,
            audio_codec: None,
            container: None,
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Pending,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        imported.push(database.upsert_library_item(&item)?);
    }
    Ok(imported)
}

pub fn reconcile(database: &Database) -> Result<LibraryScanResult, AppError> {
    let items = database.list_library_items()?;
    let mut missing_count = 0;
    for item in &items {
        let missing = !is_regular_non_symlink(Path::new(&item.path));
        if missing {
            missing_count += 1;
        }
        if missing != item.missing {
            database.set_library_missing(&item.id, missing)?;
            database.record_library_change(
                &item.id,
                item.source_id.as_deref(),
                if missing {
                    LibraryChangeKind::Missing
                } else {
                    LibraryChangeKind::Restored
                },
                None,
            )?;
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
    #[serde(default)]
    catalog_schema_version: u32,
    #[serde(default)]
    metadata_schema_version: u32,
    exported_at: chrono::DateTime<Utc>,
    #[serde(default)]
    sources: Vec<LibrarySource>,
    #[serde(default)]
    source_automations: Vec<LibrarySourceAutomation>,
    items: Vec<LibraryItem>,
    #[serde(default)]
    collections: Vec<LibraryCollection>,
    #[serde(default)]
    playlists: Vec<LibraryPlaylist>,
    #[serde(default)]
    media_stacks: Vec<LibraryMediaStack>,
    #[serde(default)]
    curation_decisions: Vec<LibraryCurationState>,
    #[serde(default)]
    moments: Vec<LibraryMoment>,
    #[serde(default)]
    montages: Vec<LibraryMontage>,
    #[serde(default)]
    rich_metadata: Vec<LibraryRichMetadata>,
}

pub fn export_backup(database: &Database, path: &str) -> Result<(), AppError> {
    let path = validate_json_output(path)?;
    let sources = database.list_library_sources()?;
    let source_automations = sources
        .iter()
        .map(|source| database.get_library_source_automation(&source.id))
        .filter_map(|result| match result {
            Ok(automation)
                if !automation.tags.is_empty()
                    || automation.collection_id.is_some()
                    || automation.favorite =>
            {
                Some(Ok(automation))
            }
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let backup = LibraryBackup {
        version: LIBRARY_BACKUP_VERSION,
        catalog_schema_version: LIBRARY_CATALOG_SCHEMA_VERSION,
        metadata_schema_version: LIBRARY_METADATA_SCHEMA_VERSION,
        exported_at: Utc::now(),
        sources,
        source_automations,
        items: database.list_library_items_for_backup(MAX_LIBRARY_BACKUP_ITEMS + 1)?,
        collections: database.list_library_collections()?,
        playlists: database.list_library_playlists()?,
        media_stacks: database.list_library_media_stacks()?,
        curation_decisions: database
            .list_library_curation_states_for_backup(MAX_LIBRARY_BACKUP_ITEMS + 1)?,
        moments: database.list_library_moments_for_backup(MAX_LIBRARY_BACKUP_MOMENTS + 1)?,
        montages: database.list_library_montages()?,
        rich_metadata: database
            .list_library_rich_metadata_for_backup(MAX_LIBRARY_BACKUP_ITEMS + 1)?,
    };
    validate_backup_shape(&backup)?;
    let bytes = serde_json::to_vec_pretty(&backup).map_err(|error| {
        AppError::internal("could not serialize the media library backup", error)
    })?;
    if bytes.len() as u64 > MAX_LIBRARY_BACKUP_BYTES {
        return Err(AppError::new(
            "library_backup_too_large",
            "This library is too large for a single backup file.",
        ));
    }
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
    let backup_size = std::fs::metadata(path)
        .map_err(|error| AppError::internal("could not inspect the media library backup", error))?
        .len();
    if backup_size > MAX_LIBRARY_BACKUP_BYTES {
        return Err(AppError::new(
            "library_backup_too_large",
            "Choose a library backup smaller than 128 MB.",
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| AppError::internal("could not read the media library backup", error))?;
    let backup: LibraryBackup = serde_json::from_slice(&bytes).map_err(|error| {
        AppError::with_details(
            "invalid_library_backup",
            "This file is not a valid Nice Downloader library backup.",
            error.to_string(),
        )
    })?;
    validate_backup_shape(&backup)?;

    let (source_requests, source_id_map) = prepare_backup_sources(database, &backup)?;
    let prepared_source_automations =
        prepare_backup_source_automations(backup.source_automations, &source_id_map)?;
    let mut prepared_items =
        prepare_backup_items(database, backup.items, &source_id_map, backup.version)?;
    let prepared_collections = prepare_backup_collections(backup.collections)?;
    let backup_item_kinds = prepared_items
        .iter()
        .map(|(backup_id, item)| (backup_id.clone(), item.media_kind))
        .collect::<HashMap<_, _>>();
    let backup_item_ids = backup_item_kinds.keys().cloned().collect::<HashSet<_>>();
    let prepared_media_stacks =
        prepare_backup_media_stacks(database, backup.media_stacks, &backup_item_kinds)?;
    let mut curation_item_ids = HashSet::new();
    let prepared_curation_decisions = backup
        .curation_decisions
        .into_iter()
        .map(|state| {
            let Some(decision) = state.decision else {
                return Err(invalid_backup(
                    "Curation decisions must contain a supported decision.",
                ));
            };
            if !backup_item_ids.contains(&state.item_id)
                || !curation_item_ids.insert(state.item_id.clone())
            {
                return Err(invalid_backup(
                    "Curation decisions must reference unique backed-up items.",
                ));
            }
            Ok((state.item_id, decision))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let video_backup_ids = prepared_items
        .iter()
        .filter(|(_, item)| {
            matches!(
                item.media_kind,
                Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
            )
        })
        .map(|(backup_id, _)| backup_id.clone())
        .collect::<HashSet<_>>();
    let video_backup_durations = prepared_items
        .iter()
        .filter(|(_, item)| {
            matches!(
                item.media_kind,
                Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
            )
        })
        .map(|(backup_id, item)| (backup_id.clone(), item.duration_seconds))
        .collect::<HashMap<_, _>>();
    let prepared_playlists =
        prepare_backup_playlists(database, backup.playlists, &video_backup_ids)?;
    let prepared_moments = prepare_backup_moments(backup.moments, &video_backup_durations)?;
    let prepared_montages = prepare_backup_montages(backup.montages, &video_backup_durations)?;
    let mut rich_metadata_ids = HashSet::new();
    let mut prepared_rich_metadata = Vec::with_capacity(backup.rich_metadata.len());
    for metadata in &backup.rich_metadata {
        if !backup_item_ids.contains(&metadata.item_id)
            || !rich_metadata_ids.insert(metadata.item_id.clone())
        {
            return Err(invalid_backup(
                "Rich metadata must reference one unique backup item.",
            ));
        }
        let overrides = validate_library_metadata_fields(LibraryMetadataFields {
            creator: metadata.overrides.creator.clone(),
            description: metadata.overrides.description.clone(),
            upload_date: metadata.overrides.upload_date.clone(),
            series: metadata.overrides.series.clone(),
            episode: metadata.overrides.episode.clone(),
            original_url: metadata.overrides.original_url.clone(),
        })
        .map_err(|_| invalid_backup("Rich metadata contains an invalid override."))?;
        prepared_rich_metadata.push((metadata.item_id.clone(), overrides));
    }

    for request in source_requests {
        database.save_library_source(request)?;
    }

    let mut restored_items = Vec::with_capacity(prepared_items.len());
    let mut item_id_map = HashMap::with_capacity(prepared_items.len());
    for (backup_id, item) in &mut prepared_items {
        let stored = database.upsert_library_item(item)?;
        item_id_map.insert(backup_id.clone(), stored.id.clone());
        restored_items.push(stored);
    }

    for (backup_item_id, overrides) in prepared_rich_metadata {
        if let Some(item_id) = item_id_map.get(&backup_item_id) {
            database.save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: item_id.clone(),
                creator: overrides.creator,
                description: overrides.description,
                upload_date: overrides.upload_date,
                series: overrides.series,
                episode: overrides.episode,
                original_url: overrides.original_url,
            })?;
        }
    }

    for (backup_item_id, decision) in prepared_curation_decisions {
        if let Some(item_id) = item_id_map.get(&backup_item_id) {
            database.set_library_curation_decision(item_id, Some(decision))?;
        }
    }

    for collection in prepared_collections {
        let stored = database.save_library_collection(SaveLibraryCollectionRequest {
            id: Some(collection.id),
            name: collection.name,
        })?;
        let item_ids = collection
            .item_ids
            .into_iter()
            .filter_map(|id| item_id_map.get(&id).cloned())
            .collect::<Vec<_>>();
        for item_ids in item_ids.chunks(500) {
            database.apply_library_bulk_action(
                item_ids.to_vec(),
                LibraryBulkAction::AddToCollection {
                    collection_id: stored.id.clone(),
                },
            )?;
        }
    }
    for automation in prepared_source_automations {
        database.save_library_source_automation_internal(automation)?;
    }
    for playlist in prepared_playlists {
        let stored = database.save_library_playlist(SaveLibraryPlaylistRequest {
            id: Some(playlist.id),
            name: playlist.name,
            description: playlist.description,
        })?;
        for backup_item_id in playlist.item_ids {
            if let Some(item_id) = item_id_map.get(&backup_item_id) {
                database.add_library_playlist_item(&stored.id, item_id)?;
            }
        }
    }
    let existing_stack_owners = database
        .list_library_media_stacks()?
        .into_iter()
        .flat_map(|stack| {
            let stack_id = stack.id;
            stack
                .members
                .into_iter()
                .map(move |member| (member.item_id, stack_id.clone()))
        })
        .collect::<HashMap<_, _>>();
    for stack in prepared_media_stacks {
        let stack_id = stack.id.clone();
        let mut members = stack
            .members
            .into_iter()
            .filter_map(|member| {
                item_id_map
                    .get(&member.item_id)
                    .cloned()
                    .filter(|item_id| {
                        existing_stack_owners
                            .get(item_id)
                            .is_none_or(|owner| owner == &stack_id)
                    })
                    .map(|item_id| crate::models::LibraryMediaStackMember {
                        item_id,
                        position: member.position,
                        role: member.role,
                    })
            })
            .collect::<Vec<_>>();
        if members.len() < 2 {
            continue;
        }
        if !members
            .iter()
            .any(|member| member.role == LibraryMediaStackRole::Original)
        {
            members[0].role = LibraryMediaStackRole::Original;
        }
        let cover_item_id = stack
            .cover_item_id
            .as_ref()
            .and_then(|item_id| item_id_map.get(item_id))
            .filter(|item_id| members.iter().any(|member| &member.item_id == *item_id))
            .cloned();
        database.save_library_media_stack(SaveLibraryMediaStackRequest {
            id: Some(stack.id),
            name: stack.name,
            notes: stack.notes,
            cover_item_id,
            members,
        })?;
    }

    for moment in prepared_moments {
        if let Some(item_id) = item_id_map.get(&moment.item_id) {
            database.save_library_moment(SaveLibraryMomentRequest {
                id: Some(moment.id),
                item_id: item_id.clone(),
                label: moment.label,
                note: moment.note,
                start_seconds: moment.start_seconds,
                end_seconds: moment.end_seconds,
                color: moment.color,
            })?;
        }
    }
    for mut montage in prepared_montages {
        let expected_segment_count = montage.expected_segment_count;
        for segment in &mut montage.segments {
            segment.item_id = item_id_map
                .get(&segment.item_id)
                .cloned()
                .ok_or_else(|| invalid_backup("A montage segment could not be restored."))?;
        }
        database.restore_library_montage(
            SaveLibraryMontageRequest {
                id: Some(montage.id),
                name: montage.name,
                notes: montage.notes,
                fade_seconds: montage.fade_seconds,
                segments: montage.segments,
            },
            expected_segment_count,
        )?;
    }
    Ok(restored_items)
}

fn validate_backup_shape(backup: &LibraryBackup) -> Result<(), AppError> {
    let supported_version = backup.version == 1
        || ((2..=LIBRARY_BACKUP_VERSION).contains(&backup.version)
            && backup.catalog_schema_version == LIBRARY_CATALOG_SCHEMA_VERSION
            && backup.metadata_schema_version == LIBRARY_METADATA_SCHEMA_VERSION);
    let membership_count = backup
        .collections
        .iter()
        .map(|collection| collection.item_ids.len())
        .chain(
            backup
                .playlists
                .iter()
                .map(|playlist| playlist.item_ids.len()),
        )
        .chain(backup.media_stacks.iter().map(|stack| stack.members.len()))
        .try_fold(0usize, usize::checked_add);
    let montage_segment_count = backup
        .montages
        .iter()
        .map(|montage| montage.segments.len())
        .try_fold(0usize, usize::checked_add);
    if !supported_version
        || backup.items.len() > MAX_LIBRARY_BACKUP_ITEMS
        || backup.sources.len() > MAX_LIBRARY_BACKUP_SOURCES
        || backup.source_automations.len() > MAX_LIBRARY_BACKUP_SOURCES
        || backup.collections.len() > MAX_LIBRARY_BACKUP_COLLECTIONS
        || backup.playlists.len() > MAX_LIBRARY_BACKUP_PLAYLISTS
        || backup.media_stacks.len() > MAX_LIBRARY_BACKUP_STACKS
        || backup.moments.len() > MAX_LIBRARY_BACKUP_MOMENTS
        || backup.montages.len() > MAX_LIBRARY_BACKUP_MONTAGES
        || montage_segment_count.is_none_or(|count| count > MAX_LIBRARY_BACKUP_MONTAGE_SEGMENTS)
        || backup.rich_metadata.len() > MAX_LIBRARY_BACKUP_ITEMS
        || backup.curation_decisions.len() > MAX_LIBRARY_BACKUP_ITEMS
        || membership_count.is_none_or(|count| count > MAX_LIBRARY_BACKUP_MEMBERSHIPS)
    {
        return Err(AppError::new(
            "unsupported_library_backup",
            "This library backup uses an unsupported version or exceeds the safe restore limits.",
        ));
    }
    Ok(())
}

fn prepare_backup_sources(
    database: &Database,
    backup: &LibraryBackup,
) -> Result<(Vec<SaveLibrarySourceRequest>, HashMap<String, String>), AppError> {
    if backup.version == 1 {
        return Ok((Vec::new(), HashMap::new()));
    }
    let existing = database.list_library_sources()?;
    let mut occupied_paths = existing
        .iter()
        .map(|source| (PathBuf::from(&source.path), source.id.clone()))
        .collect::<Vec<_>>();
    let existing_ids = existing
        .iter()
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    let mut backup_ids = HashSet::new();
    let mut backup_paths = HashSet::new();
    let mut requests = Vec::with_capacity(backup.sources.len());
    let mut id_map = HashMap::with_capacity(backup.sources.len());
    for source in &backup.sources {
        if !backup_ids.insert(source.id.as_str()) {
            return Err(invalid_backup("Source identifiers must be unique."));
        }
        let path = normalize_backup_source_path(&source.path)?;
        if !backup_paths.insert(path.clone()) {
            return Err(invalid_backup("Source paths must be unique."));
        }
        let exact_existing = occupied_paths
            .iter()
            .find(|(occupied, _)| occupied == &path)
            .map(|(_, id)| id.clone());
        if exact_existing.is_none()
            && occupied_paths
                .iter()
                .any(|(occupied, _)| path.starts_with(occupied) || occupied.starts_with(&path))
        {
            return Err(AppError::new(
                "library_source_overlaps",
                "A backed-up folder overlaps an existing library source.",
            ));
        }
        let id = exact_existing.unwrap_or_else(|| {
            if Uuid::parse_str(&source.id).is_ok() && !existing_ids.contains(source.id.as_str()) {
                source.id.clone()
            } else {
                Uuid::new_v4().to_string()
            }
        });
        let name = validate_source_name(&source.name)?;
        let include_patterns = normalize_patterns(source.include_patterns.clone())?;
        let exclude_patterns = normalize_patterns(source.exclude_patterns.clone())?;
        occupied_paths.push((path.clone(), id.clone()));
        id_map.insert(source.id.clone(), id.clone());
        requests.push(SaveLibrarySourceRequest {
            id: Some(id),
            name,
            path: path.to_string_lossy().into_owned(),
            media_kind: source.media_kind,
            recursive: source.recursive,
            watch_enabled: source.watch_enabled,
            private: source.private,
            include_patterns,
            exclude_patterns,
        });
    }
    Ok((requests, id_map))
}

fn prepare_backup_source_automations(
    automations: Vec<LibrarySourceAutomation>,
    source_id_map: &HashMap<String, String>,
) -> Result<Vec<SaveLibrarySourceAutomationRequest>, AppError> {
    let mut source_ids = HashSet::new();
    automations
        .into_iter()
        .map(|automation| {
            if !source_ids.insert(automation.source_id.clone()) {
                return Err(invalid_backup(
                    "Folder automation must reference each source at most once.",
                ));
            }
            let source_id = source_id_map.get(&automation.source_id).ok_or_else(|| {
                invalid_backup("Folder automation must reference a backed-up source.")
            })?;
            if automation.tags.len() > 20 {
                return Err(invalid_backup(
                    "Folder automation can contain at most 20 tags.",
                ));
            }
            let mut tags = Vec::with_capacity(automation.tags.len());
            for tag in automation.tags {
                let tag = tag.trim();
                if tag.is_empty() || tag.chars().count() > 40 || tag.chars().any(char::is_control) {
                    return Err(invalid_backup(
                        "Folder automation contains an invalid media tag.",
                    ));
                }
                if !tags
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(tag))
                {
                    tags.push(tag.to_owned());
                }
            }
            if automation
                .collection_id
                .as_ref()
                .is_some_and(|id| Uuid::parse_str(id).is_err())
            {
                return Err(invalid_backup(
                    "Folder automation contains an invalid collection identifier.",
                ));
            }
            Ok(SaveLibrarySourceAutomationRequest {
                source_id: source_id.clone(),
                tags,
                collection_id: automation.collection_id,
                favorite: automation.favorite,
            })
        })
        .collect()
}

fn prepare_backup_items(
    database: &Database,
    items: Vec<LibraryItem>,
    source_id_map: &HashMap<String, String>,
    backup_version: u32,
) -> Result<Vec<(String, LibraryItem)>, AppError> {
    let existing_id_paths = database.list_library_item_id_paths()?;
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut prepared = Vec::with_capacity(items.len());
    for mut item in items {
        let backup_id = item.id.clone();
        if !ids.insert(backup_id.clone()) {
            return Err(invalid_backup("Media identifiers must be unique."));
        }
        let path = PathBuf::from(&item.path);
        if !path.is_absolute() || item.path.chars().count() > 32_768 {
            return Err(invalid_backup(
                "Every media item must use an absolute path.",
            ));
        }
        if !paths.insert(item.path.clone()) {
            return Err(invalid_backup("Media paths must be unique."));
        }
        let title = item.title.trim();
        if title.is_empty() || title.chars().count() > 500 || title.chars().any(char::is_control) {
            return Err(invalid_backup("Media titles must be valid text."));
        }
        item.title = title.to_owned();
        item.id = if Uuid::parse_str(&item.id).is_ok()
            && existing_id_paths
                .get(&item.id)
                .is_none_or(|existing_path| existing_path == &item.path)
        {
            item.id
        } else {
            Uuid::new_v4().to_string()
        };
        item.tags = normalize_tags(std::mem::take(&mut item.tags))?;
        if item.rating.is_some_and(|rating| !(1..=5).contains(&rating)) {
            return Err(invalid_backup(
                "Media ratings must be between one and five.",
            ));
        }
        item.source_id = if backup_version == 1 {
            None
        } else {
            item.source_id
                .as_ref()
                .and_then(|source_id| source_id_map.get(source_id).cloned())
        };
        item.relative_path = if item.source_id.is_some() {
            normalize_backup_relative_path(item.relative_path.take())?
        } else {
            None
        };
        item.thumbnail = item.thumbnail.filter(|thumbnail| {
            Path::new(thumbnail).is_absolute() && Path::new(thumbnail).is_file()
        });
        match file_fingerprint(&path) {
            Some((size, modified_at)) => {
                item.missing = false;
                if item.file_size_bytes != Some(size) || item.modified_at_ms != Some(modified_at) {
                    reset_restored_metadata(&mut item, size, modified_at);
                }
            }
            None => item.missing = true,
        }
        item.updated_at = Utc::now();
        prepared.push((backup_id, item));
    }
    Ok(prepared)
}

fn prepare_backup_collections(
    collections: Vec<LibraryCollection>,
) -> Result<Vec<LibraryCollection>, AppError> {
    let mut ids = HashSet::new();
    collections
        .into_iter()
        .map(|mut collection| {
            if !ids.insert(collection.id.clone()) {
                return Err(invalid_backup("Collection identifiers must be unique."));
            }
            collection.id = if Uuid::parse_str(&collection.id).is_ok() {
                collection.id
            } else {
                Uuid::new_v4().to_string()
            };
            let name = collection.name.trim();
            if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
                return Err(invalid_backup("Collection names must be valid text."));
            }
            collection.name = name.to_owned();
            collection.item_ids.sort();
            collection.item_ids.dedup();
            Ok(collection)
        })
        .collect()
}

fn prepare_backup_media_stacks(
    database: &Database,
    stacks: Vec<LibraryMediaStack>,
    item_kinds: &HashMap<String, Option<LibraryMediaKind>>,
) -> Result<Vec<LibraryMediaStack>, AppError> {
    let existing = database.list_library_media_stacks()?;
    let existing_ids = existing
        .iter()
        .map(|stack| stack.id.as_str())
        .collect::<HashSet<_>>();
    let mut backup_ids = HashSet::new();
    let mut names = HashSet::new();
    let mut claimed_items = HashSet::new();
    let mut prepared = Vec::with_capacity(stacks.len());

    for mut stack in stacks {
        if !backup_ids.insert(stack.id.clone()) {
            return Err(invalid_backup("Media stack identifiers must be unique."));
        }
        let name = stack.name.trim();
        if name.is_empty()
            || name.chars().count() > 80
            || name.chars().any(char::is_control)
            || !names.insert(name.to_ascii_lowercase())
        {
            return Err(invalid_backup(
                "Media stack names must be unique valid text.",
            ));
        }
        let notes = stack.notes.trim();
        if notes.chars().count() > 2_000
            || notes
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        {
            return Err(invalid_backup("Media stack notes are invalid."));
        }
        if !(2..=100).contains(&stack.members.len()) {
            return Err(invalid_backup(
                "Media stacks must contain between 2 and 100 items.",
            ));
        }
        stack.members.sort_by_key(|member| member.position);
        let mut member_ids = HashSet::new();
        let mut stack_kind = None;
        let mut original_count = 0usize;
        for (position, member) in stack.members.iter_mut().enumerate() {
            if !member_ids.insert(member.item_id.clone())
                || !claimed_items.insert(member.item_id.clone())
            {
                return Err(invalid_backup(
                    "Each backed-up item may belong to only one media stack.",
                ));
            }
            let Some(kind) = item_kinds.get(&member.item_id).copied().flatten() else {
                return Err(invalid_backup(
                    "Media stacks must reference backed-up videos or images.",
                ));
            };
            if !matches!(kind, LibraryMediaKind::Video | LibraryMediaKind::Image) {
                return Err(invalid_backup(
                    "Media stacks may contain only videos or images.",
                ));
            }
            if stack_kind.is_some_and(|expected| expected != kind) {
                return Err(invalid_backup(
                    "A backed-up media stack cannot mix videos and images.",
                ));
            }
            stack_kind.get_or_insert(kind);
            member.position = position;
            if member.role == LibraryMediaStackRole::Original {
                original_count += 1;
            }
        }
        if original_count != 1 {
            return Err(invalid_backup(
                "Each media stack must identify exactly one original.",
            ));
        }
        if stack
            .cover_item_id
            .as_ref()
            .is_some_and(|cover| !member_ids.contains(cover))
        {
            return Err(invalid_backup(
                "A media stack cover must belong to that stack.",
            ));
        }
        stack.id = existing
            .iter()
            .find(|existing| existing.name.eq_ignore_ascii_case(name))
            .map(|existing| existing.id.clone())
            .unwrap_or_else(|| {
                if Uuid::parse_str(&stack.id).is_ok() && !existing_ids.contains(stack.id.as_str()) {
                    stack.id.clone()
                } else {
                    Uuid::new_v4().to_string()
                }
            });
        stack.name = name.to_owned();
        stack.notes = notes.to_owned();
        prepared.push(stack);
    }
    Ok(prepared)
}

fn prepare_backup_playlists(
    database: &Database,
    playlists: Vec<LibraryPlaylist>,
    video_item_ids: &HashSet<String>,
) -> Result<Vec<LibraryPlaylist>, AppError> {
    let existing = database.list_library_playlists()?;
    let existing_ids = existing
        .iter()
        .map(|playlist| playlist.id.as_str())
        .collect::<HashSet<_>>();
    let mut backup_ids = HashSet::new();
    let mut names = HashSet::new();
    let mut prepared = Vec::with_capacity(playlists.len());
    for mut playlist in playlists {
        if !backup_ids.insert(playlist.id.clone()) {
            return Err(invalid_backup("Playlist identifiers must be unique."));
        }
        let name = playlist.name.trim();
        if name.is_empty()
            || name.chars().count() > 80
            || name.chars().any(char::is_control)
            || !names.insert(name.to_ascii_lowercase())
        {
            return Err(invalid_backup("Playlist names must be unique valid text."));
        }
        let description = playlist.description.trim();
        if description.chars().count() > 500
            || description
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        {
            return Err(invalid_backup("Playlist descriptions are invalid."));
        }
        let mut seen_items = HashSet::new();
        playlist
            .item_ids
            .retain(|item_id| seen_items.insert(item_id.clone()));
        if playlist.item_ids.len() > 500
            || playlist
                .item_ids
                .iter()
                .any(|item_id| !video_item_ids.contains(item_id))
        {
            return Err(invalid_backup(
                "Playlists may contain no more than 500 backed-up videos.",
            ));
        }
        playlist.id = existing
            .iter()
            .find(|existing| existing.name.eq_ignore_ascii_case(name))
            .map(|existing| existing.id.clone())
            .unwrap_or_else(|| {
                if Uuid::parse_str(&playlist.id).is_ok()
                    && !existing_ids.contains(playlist.id.as_str())
                {
                    playlist.id.clone()
                } else {
                    Uuid::new_v4().to_string()
                }
            });
        playlist.name = name.to_owned();
        playlist.description = description.to_owned();
        prepared.push(playlist);
    }
    Ok(prepared)
}

fn prepare_backup_moments(
    moments: Vec<LibraryMoment>,
    video_durations: &HashMap<String, Option<f64>>,
) -> Result<Vec<LibraryMoment>, AppError> {
    let mut backup_ids = HashSet::new();
    let mut per_item = HashMap::<String, usize>::new();
    let mut prepared = Vec::with_capacity(moments.len());
    for mut moment in moments {
        if !backup_ids.insert(moment.id.clone()) {
            return Err(invalid_backup("Moment identifiers must be unique."));
        }
        let Some(duration) = video_durations.get(&moment.item_id) else {
            return Err(invalid_backup("Moments must reference backed-up videos."));
        };
        let count = per_item.entry(moment.item_id.clone()).or_default();
        *count += 1;
        if *count > 500 {
            return Err(invalid_backup(
                "A video may contain no more than 500 moments.",
            ));
        }
        let label = moment.label.trim();
        if label.is_empty() || label.chars().count() > 100 || label.chars().any(char::is_control) {
            return Err(invalid_backup("Moment labels contain invalid text."));
        }
        let note = moment.note.trim();
        if note.chars().count() > 2_000
            || note
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        {
            return Err(invalid_backup("Moment notes contain invalid text."));
        }
        if !matches!(
            moment.color.as_str(),
            "accent" | "blue" | "amber" | "green" | "rose"
        ) || !moment.start_seconds.is_finite()
            || moment.start_seconds < 0.0
            || moment
                .end_seconds
                .is_some_and(|end| !end.is_finite() || end <= moment.start_seconds)
        {
            return Err(invalid_backup("Moment timing or color is invalid."));
        }
        if let Some(duration) = duration.filter(|value| value.is_finite() && *value > 0.0) {
            let last = moment.end_seconds.unwrap_or(moment.start_seconds);
            if last > duration + 1.0 {
                return Err(invalid_backup("A moment is outside its video's duration."));
            }
        }
        moment.id = Uuid::new_v4().to_string();
        moment.label = label.to_owned();
        moment.note = note.to_owned();
        prepared.push(moment);
    }
    Ok(prepared)
}

fn prepare_backup_montages(
    montages: Vec<LibraryMontage>,
    video_durations: &HashMap<String, Option<f64>>,
) -> Result<Vec<LibraryMontage>, AppError> {
    let mut ids = HashSet::new();
    let mut prepared = Vec::with_capacity(montages.len());
    for mut montage in montages {
        if Uuid::parse_str(&montage.id).is_err() || !ids.insert(montage.id.clone()) {
            return Err(invalid_backup(
                "Montage identifiers must be valid and unique.",
            ));
        }
        let name = montage.name.trim();
        let notes = montage.notes.trim();
        if name.is_empty()
            || name.chars().count() > 100
            || name.chars().any(char::is_control)
            || notes.chars().count() > 2_000
            || notes
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            || !montage.fade_seconds.is_finite()
            || !(0.0..=5.0).contains(&montage.fade_seconds)
            || (montage.segments.is_empty() && !montage.incomplete)
            || montage.segments.len() > 50
            || montage.expected_segment_count < montage.segments.len()
            || montage.expected_segment_count > 50
            || montage.incomplete != (montage.expected_segment_count != montage.segments.len())
        {
            return Err(invalid_backup("A montage contains invalid project data."));
        }
        let mut total_duration = 0.0;
        for segment in &mut montage.segments {
            let Some(duration) = video_durations.get(&segment.item_id) else {
                return Err(invalid_backup(
                    "Montage segments must reference backed-up videos.",
                ));
            };
            let label = segment.label.trim();
            if label.is_empty()
                || label.chars().count() > 100
                || label.chars().any(char::is_control)
                || !segment.start_seconds.is_finite()
                || !segment.end_seconds.is_finite()
                || segment.start_seconds < 0.0
                || segment.end_seconds - segment.start_seconds < 0.1
                || duration
                    .filter(|value| value.is_finite() && *value > 0.0)
                    .is_some_and(|duration| segment.end_seconds > duration + 1.0)
            {
                return Err(invalid_backup("A montage contains an invalid clip range."));
            }
            segment.label = label.to_owned();
            total_duration += segment.end_seconds - segment.start_seconds;
        }
        if total_duration > 21_600.0 {
            return Err(invalid_backup("A montage exceeds the six-hour limit."));
        }
        montage.name = name.to_owned();
        montage.notes = notes.to_owned();
        prepared.push(montage);
    }
    Ok(prepared)
}

fn normalize_backup_source_path(value: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.parent().is_none() || value.chars().count() > 32_768 {
        return Err(invalid_backup(
            "Library sources must use absolute folder paths.",
        ));
    }
    if path.exists() {
        let canonical = path.canonicalize().map_err(|error| {
            AppError::with_details(
                "invalid_library_backup",
                "A backed-up source path cannot be accessed.",
                error.to_string(),
            )
        })?;
        if !canonical.is_dir() {
            return Err(invalid_backup("A backed-up source path is not a folder."));
        }
        Ok(canonical)
    } else {
        Ok(path)
    }
}

fn normalize_backup_relative_path(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let path = Path::new(&value);
    if value.chars().count() > 32_768
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(invalid_backup(
            "A backed-up relative media path is invalid.",
        ));
    }
    Ok(Some(value))
}

fn reset_restored_metadata(item: &mut LibraryItem, size: u64, modified_at: i64) {
    item.file_size_bytes = Some(size);
    item.modified_at_ms = Some(modified_at);
    item.duration_seconds = None;
    item.width = None;
    item.height = None;
    item.fps = None;
    item.video_codec = None;
    item.audio_codec = None;
    item.container = None;
    item.dynamic_range = None;
    item.metadata_state = LibraryMetadataState::Pending;
    item.metadata_error = None;
}

fn invalid_backup(details: &str) -> AppError {
    AppError::with_details(
        "invalid_library_backup",
        "This library backup contains invalid data.",
        details,
    )
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
    let uploader = value
        .get("uploader")
        .and_then(Value::as_str)
        .or_else(|| value.get("channel").and_then(Value::as_str));
    let series = value
        .get("series")
        .and_then(Value::as_str)
        .or_else(|| value.get("playlist_title").and_then(Value::as_str));
    let episode = value
        .get("episode")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            value
                .get("episode_number")
                .and_then(Value::as_i64)
                .map(|number| number.to_string())
        });
    let webpage_url = value
        .get("webpage_url")
        .and_then(Value::as_str)
        .or_else(|| value.get("original_url").and_then(Value::as_str));
    json!({
        "id": value.get("id"), "title": value.get("title"),
        "uploader": uploader,
        "uploadDate": value.get("upload_date"), "description": value.get("description").and_then(Value::as_str).map(|text| text.chars().take(2000).collect::<String>()),
        "series": series,
        "episode": episode,
        "webpageUrl": webpage_url,
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
    use tempfile::tempdir;

    fn backup_test_item(path: &Path, source_id: Option<String>) -> LibraryItem {
        let now = Utc::now();
        let (size, modified_at) = file_fingerprint(path).unwrap_or((0, 0));
        LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: path.to_string_lossy().into_owned(),
            source_id,
            relative_path: Some("clip.mp4".to_owned()),
            media_kind: Some(LibraryMediaKind::Video),
            title: "Edited clip".to_owned(),
            source_url: Some("https://example.invalid/watch/1".to_owned()),
            thumbnail: None,
            duration_seconds: Some(12.5),
            site: Some("Example".to_owned()),
            tags: vec!["Favorite".to_owned()],
            metadata: json!({
                "safe": true,
                "uploader": "Extracted creator",
                "description": "Extracted description",
                "uploadDate": "2026-02-03"
            }),
            file_size_bytes: Some(size),
            modified_at_ms: Some(modified_at),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: Some("SDR".to_owned()),
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: true,
            rating: Some(5),
            missing: !path.is_file(),
            added_at: now,
            updated_at: now,
        }
    }

    #[cfg(unix)]
    #[test]
    fn reconcile_marks_symlinked_catalog_paths_missing() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().expect("temporary directory");
        let target = directory.path().join("target.mp4");
        let link = directory.path().join("catalog.mp4");
        std::fs::write(&target, b"media").expect("target media");
        symlink(&target, &link).expect("catalog symlink");
        let database =
            Database::open(&directory.path().join("library.sqlite3")).expect("library database");
        let item = database
            .upsert_library_item(&backup_test_item(&link, None))
            .expect("catalog item");

        reconcile(&database).expect("reconcile library");

        assert!(
            database
                .get_library_item(&item.id)
                .expect("reconciled item")
                .missing
        );
    }

    #[test]
    fn current_backup_restores_sources_collections_metadata_moments_and_curation() {
        let directory = tempdir().expect("temporary directory");
        let media_directory = directory.path().join("media");
        std::fs::create_dir(&media_directory).expect("media directory");
        let media_path = media_directory.join("clip.mp4");
        std::fs::write(&media_path, b"stable media fixture").expect("media fixture");
        let source_database =
            Database::open(&directory.path().join("source.sqlite3")).expect("source database");
        let source = source_database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Archive".to_owned(),
                path: media_directory
                    .canonicalize()
                    .expect("canonical media directory")
                    .to_string_lossy()
                    .into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: false,
                private: true,
                include_patterns: vec!["*.mp4".to_owned()],
                exclude_patterns: vec!["private/*".to_owned()],
            })
            .expect("source");
        let item = source_database
            .upsert_library_item(&backup_test_item(&media_path, Some(source.id.clone())))
            .expect("item");
        source_database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: item.id.clone(),
                creator: Some("Personal creator label".to_owned()),
                description: Some("Extracted description".to_owned()),
                upload_date: Some("2026-02-03".to_owned()),
                series: Some("My series".to_owned()),
                episode: None,
                original_url: Some("https://example.invalid/watch/1".to_owned()),
            })
            .expect("metadata overrides");
        let collection = source_database
            .save_library_collection(SaveLibraryCollectionRequest {
                id: None,
                name: "Watch later".to_owned(),
            })
            .expect("collection");
        source_database
            .apply_library_bulk_action(
                vec![item.id.clone()],
                LibraryBulkAction::AddToCollection {
                    collection_id: collection.id.clone(),
                },
            )
            .expect("collection membership");
        source_database
            .save_library_source_automation_internal(SaveLibrarySourceAutomationRequest {
                source_id: source.id.clone(),
                tags: vec!["Incoming".to_owned()],
                collection_id: Some(collection.id.clone()),
                favorite: true,
            })
            .expect("source automation");
        let playlist = source_database
            .save_library_playlist(SaveLibraryPlaylistRequest {
                id: None,
                name: "Evening".to_owned(),
                description: "Ordered viewing".to_owned(),
            })
            .expect("playlist");
        source_database
            .add_library_playlist_item(&playlist.id, &item.id)
            .expect("playlist membership");
        source_database
            .save_library_moment(SaveLibraryMomentRequest {
                id: None,
                item_id: item.id.clone(),
                label: "Favorite scene".to_owned(),
                note: "A timestamped note".to_owned(),
                start_seconds: 2.0,
                end_seconds: Some(5.0),
                color: "amber".to_owned(),
            })
            .expect("moment");
        source_database
            .save_library_montage(SaveLibraryMontageRequest {
                id: None,
                name: "Highlights".to_owned(),
                notes: "Backup montage".to_owned(),
                fade_seconds: 0.25,
                segments: vec![crate::models::LibraryMontageSegment {
                    item_id: item.id.clone(),
                    label: "Favorite scene".to_owned(),
                    start_seconds: 2.0,
                    end_seconds: 5.0,
                }],
            })
            .expect("montage");
        source_database
            .set_library_curation_decision(
                &item.id,
                Some(crate::models::LibraryCurationDecision::Keep),
            )
            .expect("curation decision");

        let backup_path = directory.path().join("library.json");
        export_backup(&source_database, &backup_path.to_string_lossy()).expect("export");
        let exported: Value =
            serde_json::from_slice(&std::fs::read(&backup_path).expect("exported backup"))
                .expect("backup json");
        assert_eq!(exported["version"], LIBRARY_BACKUP_VERSION);
        assert_eq!(
            exported["metadataSchemaVersion"],
            LIBRARY_METADATA_SCHEMA_VERSION
        );

        let restored_database =
            Database::open(&directory.path().join("restored.sqlite3")).expect("restored database");
        let restored =
            restore_backup(&restored_database, &backup_path.to_string_lossy()).expect("restore");
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].source_id.as_deref(), Some(source.id.as_str()));
        assert_eq!(restored[0].relative_path.as_deref(), Some("clip.mp4"));
        assert_eq!(restored[0].metadata_state, LibraryMetadataState::Ready);
        assert_eq!(restored[0].height, Some(1080));
        assert!(restored[0].favorite);
        assert_eq!(restored[0].rating, Some(5));
        let restored_sources = restored_database.list_library_sources().expect("sources");
        assert_eq!(restored_sources.len(), 1);
        assert!(restored_sources[0].private);
        assert_eq!(restored_sources[0].include_patterns, ["*.mp4"]);
        let restored_automation = restored_database
            .get_library_source_automation(&restored_sources[0].id)
            .expect("source automation");
        assert_eq!(restored_automation.tags, ["Incoming"]);
        assert!(restored_automation.favorite);
        let restored_collections = restored_database
            .list_library_collections()
            .expect("collections");
        assert_eq!(restored_collections.len(), 1);
        assert_eq!(restored_collections[0].name, "Watch later");
        assert_eq!(restored_collections[0].item_ids, [restored[0].id.clone()]);
        assert_eq!(
            restored_automation.collection_id.as_deref(),
            Some(restored_collections[0].id.as_str())
        );
        let restored_playlists = restored_database
            .list_library_playlists()
            .expect("playlists");
        assert_eq!(restored_playlists.len(), 1);
        assert_eq!(restored_playlists[0].name, "Evening");
        assert_eq!(restored_playlists[0].description, "Ordered viewing");
        assert_eq!(restored_playlists[0].item_ids, [restored[0].id.clone()]);
        let restored_moments = restored_database
            .list_library_moments(&restored[0].id)
            .expect("moments");
        assert_eq!(restored_moments.len(), 1);
        assert_eq!(restored_moments[0].label, "Favorite scene");
        assert_eq!(restored_moments[0].start_seconds, 2.0);
        let restored_montages = restored_database.list_library_montages().expect("montages");
        assert_eq!(restored_montages.len(), 1);
        assert_eq!(restored_montages[0].name, "Highlights");
        assert_eq!(restored_montages[0].segments[0].item_id, restored[0].id);
        assert_eq!(restored_moments[0].end_seconds, Some(5.0));
        assert_eq!(
            restored_database
                .get_library_curation_state(&restored[0].id)
                .expect("restored curation")
                .decision,
            Some(crate::models::LibraryCurationDecision::Keep)
        );
        let restored_metadata = restored_database
            .get_library_rich_metadata(&restored[0].id)
            .expect("restored rich metadata");
        assert_eq!(
            restored_metadata.effective.creator.as_deref(),
            Some("Personal creator label")
        );
        assert_eq!(
            restored_metadata.effective.series.as_deref(),
            Some("My series")
        );
        assert_eq!(
            restored_metadata.extracted.description.as_deref(),
            Some("Extracted description")
        );
    }

    #[test]
    fn version_one_backup_remains_portable_without_source_definitions() {
        let directory = tempdir().expect("temporary directory");
        let media_path = directory.path().join("missing.mp4");
        let mut item = backup_test_item(&media_path, Some(Uuid::new_v4().to_string()));
        item.relative_path = Some("nested/missing.mp4".to_owned());
        let backup_path = directory.path().join("legacy.json");
        std::fs::write(
            &backup_path,
            serde_json::to_vec(&json!({
                "version": 1,
                "exportedAt": Utc::now(),
                "items": [item]
            }))
            .expect("legacy backup json"),
        )
        .expect("legacy backup");
        let database = Database::open(&directory.path().join("legacy.sqlite3")).expect("database");
        let restored =
            restore_backup(&database, &backup_path.to_string_lossy()).expect("legacy restore");
        assert_eq!(restored.len(), 1);
        assert!(restored[0].source_id.is_none());
        assert!(restored[0].relative_path.is_none());
        assert!(restored[0].missing);
    }

    #[test]
    fn restore_invalidates_metadata_when_the_file_fingerprint_changed() {
        let directory = tempdir().expect("temporary directory");
        let media_path = directory.path().join("changed.mp4");
        std::fs::write(&media_path, b"original fixture").expect("original fixture");
        let source_database = Database::open(&directory.path().join("metadata-source.sqlite3"))
            .expect("source database");
        source_database
            .upsert_library_item(&backup_test_item(&media_path, None))
            .expect("item");
        let backup_path = directory.path().join("metadata.json");
        export_backup(&source_database, &backup_path.to_string_lossy()).expect("export");
        std::fs::write(&media_path, b"a different and longer fixture").expect("changed fixture");

        let restored_database = Database::open(&directory.path().join("metadata-restored.sqlite3"))
            .expect("restored database");
        let restored =
            restore_backup(&restored_database, &backup_path.to_string_lossy()).expect("restore");
        assert_eq!(restored[0].metadata_state, LibraryMetadataState::Pending);
        assert_eq!(restored[0].duration_seconds, None);
        assert_eq!(restored[0].height, None);
        assert_eq!(
            restored[0].file_size_bytes,
            Some(b"a different and longer fixture".len() as u64)
        );
    }

    #[test]
    fn future_backup_schema_is_rejected_before_writing_records() {
        let directory = tempdir().expect("temporary directory");
        let backup_path = directory.path().join("future.json");
        let backup = LibraryBackup {
            version: LIBRARY_BACKUP_VERSION,
            catalog_schema_version: LIBRARY_CATALOG_SCHEMA_VERSION + 1,
            metadata_schema_version: LIBRARY_METADATA_SCHEMA_VERSION,
            exported_at: Utc::now(),
            sources: Vec::new(),
            source_automations: Vec::new(),
            items: Vec::new(),
            collections: Vec::new(),
            playlists: Vec::new(),
            media_stacks: Vec::new(),
            curation_decisions: Vec::new(),
            moments: Vec::new(),
            montages: Vec::new(),
            rich_metadata: Vec::new(),
        };
        std::fs::write(
            &backup_path,
            serde_json::to_vec(&backup).expect("future backup json"),
        )
        .expect("future backup");
        let database = Database::open(&directory.path().join("future.sqlite3")).expect("database");
        let error =
            restore_backup(&database, &backup_path.to_string_lossy()).expect_err("future schema");
        assert_eq!(error.code, "unsupported_library_backup");
        assert!(database.list_library_items().expect("items").is_empty());
    }

    fn create_cache_fixture(directory: &Path, name: &str, size: u64, age: u64) -> PathBuf {
        let path = directory.join(name);
        let file = std::fs::File::create(&path).expect("cache fixture");
        file.set_len(size).expect("cache fixture size");
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(age))
            .expect("cache fixture age");
        path
    }

    #[test]
    fn artwork_cache_evicts_oldest_entries_to_meet_both_limits() {
        let directory = tempdir().expect("temporary directory");
        let oldest = create_cache_fixture(directory.path(), "oldest.jpg", 8, 1);
        let middle = create_cache_fixture(directory.path(), "middle.jpg", 8, 2);
        let newest = create_cache_fixture(directory.path(), "newest.jpg", 8, 3);

        let status =
            enforce_artwork_cache_limits(directory.path(), None, 2, 16).expect("cache maintenance");

        assert_eq!(status.file_count, 2);
        assert_eq!(status.total_bytes, 16);
        assert!(!oldest.exists());
        assert!(middle.exists());
        assert!(newest.exists());
    }

    #[test]
    fn artwork_cache_never_evicts_the_entry_being_returned() {
        let directory = tempdir().expect("temporary directory");
        let preserved = create_cache_fixture(directory.path(), "preserved.jpg", 8, 1);
        let removable = create_cache_fixture(directory.path(), "removable.jpg", 8, 2);

        let status = enforce_artwork_cache_limits(directory.path(), Some(&preserved), 1, 8)
            .expect("cache maintenance");

        assert_eq!(status.file_count, 1);
        assert_eq!(status.total_bytes, 8);
        assert!(preserved.exists());
        assert!(!removable.exists());
    }

    #[test]
    fn tags_are_trimmed_and_deduplicated() {
        assert_eq!(
            normalize_tags(vec![" Music ".into(), "music".into(), "live".into()]).unwrap(),
            ["Music", "live"]
        );
    }

    #[test]
    fn classifies_supported_media_without_accepting_other_files() {
        assert_eq!(
            media_kind_for_path(Path::new("movie.MKV")),
            Some(LibraryMediaKind::Video)
        );
        assert_eq!(
            media_kind_for_path(Path::new("cover.avif")),
            Some(LibraryMediaKind::Image)
        );
        assert_eq!(media_kind_for_path(Path::new("notes.txt")), None);
    }

    #[test]
    fn wildcard_filters_are_case_insensitive_and_exclusions_win() {
        assert!(matches_patterns(
            "Trips/Clip.MP4",
            &["trips/*".to_owned()],
            &[]
        ));
        assert!(!matches_patterns(
            "Trips/private/Clip.MP4",
            &["trips/*".to_owned()],
            &["*/private/*".to_owned()]
        ));
    }

    #[test]
    fn hidden_path_components_are_excluded() {
        assert!(is_hidden(Path::new("archive/.cache/preview.jpg")));
        assert!(!is_hidden(Path::new("archive/photos/preview.jpg")));
    }

    #[test]
    fn rejects_overlapping_library_sources() {
        let directory = tempdir().expect("temporary directory");
        let source_path = directory.path().join("media");
        let nested_path = source_path.join("nested");
        std::fs::create_dir_all(&nested_path).expect("source folders");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let manager = LibraryManager::new(database).expect("library manager");
        manager
            .save_source(SaveLibrarySourceRequest {
                id: None,
                name: "Media".to_owned(),
                path: source_path.to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: false,
                private: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
            })
            .expect("first source");
        let error = manager
            .save_source(SaveLibrarySourceRequest {
                id: None,
                name: "Nested".to_owned(),
                path: nested_path.to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Video,
                recursive: true,
                watch_enabled: false,
                private: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
            })
            .expect_err("overlapping source");
        assert_eq!(error.code, "library_source_overlaps");
    }

    #[test]
    fn recognizes_only_mutating_watcher_events() {
        use notify::event::{AccessKind, CreateKind, ModifyKind, RemoveKind};

        assert!(watch_event_should_rescan(&EventKind::Create(
            CreateKind::File
        )));
        assert!(watch_event_should_rescan(&EventKind::Modify(
            ModifyKind::Any
        )));
        assert!(watch_event_should_rescan(&EventKind::Remove(
            RemoveKind::File
        )));
        assert!(!watch_event_should_rescan(&EventKind::Access(
            AccessKind::Read
        )));
    }

    #[test]
    fn permits_disabling_an_offline_existing_source() {
        let directory = tempdir().expect("temporary directory");
        let source_path = directory.path().join("removable-media");
        std::fs::create_dir(&source_path).expect("source directory");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let manager = LibraryManager::new(database).expect("library manager");
        let source = manager
            .save_source(SaveLibrarySourceRequest {
                id: None,
                name: "Removable".to_owned(),
                path: source_path.to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: true,
                private: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
            })
            .expect("save source");
        std::fs::remove_dir(&source_path).expect("disconnect source");

        let updated = manager
            .save_source(SaveLibrarySourceRequest {
                id: Some(source.id),
                name: source.name,
                path: source.path,
                media_kind: source.media_kind,
                recursive: source.recursive,
                watch_enabled: false,
                private: source.private,
                include_patterns: source.include_patterns,
                exclude_patterns: source.exclude_patterns,
            })
            .expect("disable watcher while offline");

        assert!(!updated.watch_enabled);
    }

    #[test]
    fn hashes_regular_files_and_honors_cancellation() {
        let directory = tempdir().expect("temporary directory");
        let first = directory.path().join("first.bin");
        let second = directory.path().join("second.bin");
        std::fs::write(&first, b"same content").expect("first file");
        std::fs::write(&second, b"same content").expect("second file");
        let token = CancellationToken::new();
        assert_eq!(
            hash_regular_file(&first, &token).expect("first hash"),
            hash_regular_file(&second, &token).expect("second hash")
        );

        token.cancel();
        let error = hash_regular_file(&first, &token).expect_err("cancelled hash");
        assert_eq!(error.code, "duplicate_analysis_cancelled");
    }

    #[test]
    fn builds_only_exact_duplicate_groups() {
        let groups = duplicate_groups_from_hash_rows(vec![
            ("one".to_owned(), 10, "same".to_owned()),
            ("two".to_owned(), 10, "same".to_owned()),
            ("three".to_owned(), 20, "unique".to_owned()),
        ]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].item_ids, ["one", "two"]);
        assert_eq!(groups[0].potential_reclaim_bytes, 10);
    }

    #[tokio::test]
    async fn duplicate_cleanup_rejects_unverified_groups_before_trash() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let manager = LibraryManager::new(database).expect("library manager");
        let error = manager
            .trash_duplicate_item(&"a".repeat(64), &Uuid::new_v4().to_string())
            .await
            .expect_err("unverified duplicate group");
        assert_eq!(error.code, "duplicate_group_stale");
    }

    #[test]
    fn representative_artwork_sampling_skips_video_edges() {
        let sampling = artwork_sampling_window(100.0).expect("sampling window");
        assert!((sampling.start_seconds - 8.0).abs() < f64::EPSILON);
        assert!((sampling.duration_seconds - 8.0).abs() < f64::EPSILON);
        assert!((sampling.frames_per_second - 1.0).abs() < f64::EPSILON);
        let short = artwork_sampling_window(2.0).expect("short sampling window");
        assert!(short.duration_seconds < ARTWORK_SAMPLE_WINDOW_SECONDS);
        assert!(short.start_seconds > 0.0);
        assert!(artwork_sampling_window(0.0).is_none());
        assert!(artwork_sampling_window(f64::NAN).is_none());
    }

    #[tokio::test]
    async fn generates_bounded_jpeg_artwork_when_ffmpeg_is_available() {
        let Some(ffmpeg) = managed::resolve("ffmpeg") else {
            return;
        };
        let directory = tempdir().expect("temporary directory");
        let input = directory.path().join("source.ppm");
        let output = directory.path().join("artwork.jpg");
        let mut ppm = b"P6\n2 2\n255\n".to_vec();
        ppm.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        std::fs::write(&input, ppm).expect("test image");

        generate_artwork(&ffmpeg, &input, &output, ArtworkSourceKind::StillImage)
            .await
            .expect("generated artwork");

        assert!(output.is_file());
        assert!(std::fs::metadata(output).expect("artwork metadata").len() > 0);
    }

    #[tokio::test]
    async fn generates_representative_video_artwork_when_ffmpeg_is_available() {
        let Some(ffmpeg) = managed::resolve("ffmpeg") else {
            return;
        };
        let directory = tempdir().expect("temporary directory");
        let input = directory.path().join("source.mkv");
        let output = directory.path().join("artwork.jpg");
        let generated = Command::new(&ffmpeg)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("testsrc2=duration=2:size=64x64:rate=10")
            .arg("-c:v")
            .arg("ffv1")
            .arg("-y")
            .arg(&input)
            .output()
            .await
            .expect("create test video");
        assert!(
            generated.status.success(),
            "FFmpeg could not create the test video"
        );

        generate_artwork(
            &ffmpeg,
            &input,
            &output,
            ArtworkSourceKind::Video {
                duration_seconds: Some(2.0),
            },
        )
        .await
        .expect("generated representative artwork");

        assert!(output.is_file());
        assert!(std::fs::metadata(output).expect("artwork metadata").len() > 0);
    }

    #[tokio::test]
    async fn probes_structured_image_metadata_when_ffprobe_is_available() {
        let Some(ffprobe) = managed::resolve("ffprobe") else {
            return;
        };
        let directory = tempdir().expect("temporary directory");
        let input = directory.path().join("source.ppm");
        let mut ppm = b"P6\n2 2\n255\n".to_vec();
        ppm.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
        std::fs::write(&input, ppm).expect("test image");
        let (size, modified_at) = file_fingerprint(&input).expect("fingerprint");

        let metadata = probe_media(
            &ffprobe,
            &input,
            size,
            modified_at,
            &CancellationToken::new(),
        )
        .await;

        assert_eq!(metadata.state, LibraryMetadataState::Ready);
        assert_eq!((metadata.width, metadata.height), (Some(2), Some(2)));
        assert_eq!(metadata.video_codec.as_deref(), Some("ppm"));
        assert_eq!(metadata.container.as_deref(), Some("ppm"));
    }

    #[test]
    fn parses_fractional_frame_rates_safely() {
        let ntsc = parse_frame_rate(Some("30000/1001")).expect("valid frame rate");
        assert!((ntsc - 29.970_029_970_029_97).abs() < f64::EPSILON);
        assert_eq!(parse_frame_rate(Some("0/0")), None);
        assert_eq!(parse_frame_rate(Some("not-a-rate")), None);
    }
}
