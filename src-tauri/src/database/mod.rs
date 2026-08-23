use crate::{
    errors::AppError,
    models::{
        ApplyLibraryBulkMetadataRequest, AuthenticationProfile, BrowserCookieSource,
        DesktopPreferences, DownloadArchiveMatch, DownloadJob, DownloadState, EnginePreferences,
        LibraryAvailabilityFilter, LibraryBulkAction, LibraryChange, LibraryChangeKind,
        LibraryCollection, LibraryCompareSession, LibraryCompareSessionItem,
        LibraryCurationDecision, LibraryCurationState, LibraryFacetRequest, LibraryFacets,
        LibraryFileIdentity, LibraryFileOperation, LibraryFileOperationKind,
        LibraryFileOperationState, LibraryFolderFacet, LibraryFolderScope, LibraryHealthIssue,
        LibraryHealthIssueKind, LibraryHealthSummary, LibraryItem, LibraryMediaKind,
        LibraryMediaStack, LibraryMediaStackMember, LibraryMediaStackRole, LibraryMetadataFields,
        LibraryMetadataState, LibraryMoment, LibraryMontage, LibraryMontageSegment,
        LibraryNearDuplicateMatch, LibraryPlaybackEntry, LibraryPlaybackState, LibraryPlaylist,
        LibraryPrivacyRules, LibraryPrivacyStatus, LibraryPrivateFolder, LibraryProbeResult,
        LibraryQueryRequest, LibraryQueryResult, LibraryRichMetadata, LibraryScanJob,
        LibraryScanState, LibrarySearchStatus, LibrarySemanticIndexStatus, LibrarySmartView,
        LibrarySmartViewQuery, LibrarySort, LibrarySource, LibrarySourceAutomation,
        LibrarySourceStatus, LibraryStorageBucket, LibraryStoragePlan, LibraryStorageSource,
        LibraryThemeAnalysis, LibraryThemeAnalysisMode, LibraryThemeAnalysisScope,
        LibraryThemeAssignment, LibraryThemeFacet, LibraryThemeIndexStatus, LibraryThemeSettings,
        LibraryTranscriptCue, LibraryTranscriptIndexStatus, LibraryTranscriptMatch,
        LibraryVisualSimilarityMatch, LibraryVisualSimilarityStatus, LibraryWatchState,
        PostDownloadRecipe, PostDownloadRecipeRun, PostDownloadRecipeRunState,
        PostDownloadRecipeStep, Preset, PresetIntent, QueueSettings,
        SaveAuthenticationProfileRequest, SaveLibraryCollectionRequest,
        SaveLibraryCompareSessionRequest, SaveLibraryMediaStackRequest, SaveLibraryMomentRequest,
        SaveLibraryMontageRequest, SaveLibraryPlaybackRequest, SaveLibraryPlaylistRequest,
        SaveLibraryRichMetadataRequest, SaveLibrarySmartViewRequest,
        SaveLibrarySourceAutomationRequest, SaveLibrarySourceRequest,
        SavePostDownloadRecipeRequest, SavePresetRequest, SaveSitePresetRuleRequest,
        SaveSubscriptionRequest, SitePresetRule, StartDownloadRequest,
        StartLibraryThemeAnalysisRequest, Subscription, TransformationOperation,
        TransformationState, TransformationTask, UpdateLibraryItemRequest,
    },
    utils::validation::{
        normalize_engine_preferences, validate_destination, validate_engine_preferences,
        validate_filename_template, validate_media_url,
    },
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
        Arc, Mutex, MutexGuard,
    },
};
use uuid::Uuid;

const MIGRATION_0001: &str = include_str!("migrations/0001_initial.sql");
const MIGRATION_0002: &str = include_str!("migrations/0002_durable_queue.sql");
const MIGRATION_0003: &str = include_str!("migrations/0003_workflow_intelligence.sql");
const MIGRATION_0004: &str = include_str!("migrations/0004_subscriptions.sql");
const MIGRATION_0005: &str = include_str!("migrations/0005_transformations.sql");
const MIGRATION_0006: &str = include_str!("migrations/0006_media_library.sql");
const MIGRATION_0007: &str = include_str!("migrations/0007_authentication_profiles.sql");
const MIGRATION_0008: &str = include_str!("migrations/0008_download_visibility.sql");
const MIGRATION_0009: &str = include_str!("migrations/0009_library_sources.sql");
const MIGRATION_0010: &str = include_str!("migrations/0010_library_metadata.sql");
const MIGRATION_0011: &str = include_str!("migrations/0011_library_curation.sql");
const MIGRATION_0012: &str = include_str!("migrations/0012_library_reconciliation.sql");
const MIGRATION_0013: &str = include_str!("migrations/0013_library_duplicate_hashes.sql");
const MIGRATION_0014: &str = include_str!("migrations/0014_library_file_operations.sql");
const MIGRATION_0015: &str = include_str!("migrations/0015_library_playback.sql");
const MIGRATION_0016: &str = include_str!("migrations/0016_library_watch_later.sql");
const MIGRATION_0017: &str = include_str!("migrations/0017_library_playlists.sql");
const MIGRATION_0018: &str = include_str!("migrations/0018_library_rich_metadata.sql");
const MIGRATION_0019: &str = include_str!("migrations/0019_library_smart_discovery.sql");
const MIGRATION_0020: &str = include_str!("migrations/0020_library_moments.sql");
const MIGRATION_0021: &str = include_str!("migrations/0021_library_compare_sessions.sql");
const MIGRATION_0022: &str = include_str!("migrations/0022_library_media_stacks.sql");
const MIGRATION_0023: &str = include_str!("migrations/0023_library_curation_decisions.sql");
const MIGRATION_0024: &str = include_str!("migrations/0024_library_transcripts.sql");
const MIGRATION_0025: &str = include_str!("migrations/0025_library_montages.sql");
const MIGRATION_0026: &str = include_str!("migrations/0026_library_visual_similarity.sql");
const MIGRATION_0027: &str = include_str!("migrations/0027_library_themes.sql");
const MIGRATION_0028: &str = include_str!("migrations/0028_gallery_file_progress.sql");
const MIGRATION_0029: &str = include_str!("migrations/0029_library_privacy.sql");
const MIGRATION_0030: &str = include_str!("migrations/0030_library_semantic_transcripts.sql");
const MIGRATION_0031: &str = include_str!("migrations/0031_post_download_recipes.sql");
const MIGRATION_0032: &str = include_str!("migrations/0032_library_source_automation.sql");
const MIGRATION_0033: &str = include_str!("migrations/0033_library_video_display.sql");
const MIGRATION_0034: &str = include_str!("migrations/0034_library_performance.sql");
const ENGINE_PREFERENCES_KEY: &str = "engine.preferences";
const QUEUE_SETTINGS_KEY: &str = "queue.settings";
const DESKTOP_PREFERENCES_KEY: &str = "desktop.preferences";
const LIBRARY_THEME_SETTINGS_KEY: &str = "library.theme.settings";
const LIBRARY_PRIVACY_SETTINGS_KEY: &str = "library.privacy.settings";
const ONBOARDING_COMPLETED_KEY: &str = "onboarding.completed";
const LOCKED_LIBRARY_FILTER_SQL: &str = "
    AND NOT EXISTS (
        SELECT 1 FROM library_sources privacy_source
        WHERE privacy_source.id = media_library.source_id AND privacy_source.private = 1
    )
    AND NOT EXISTS (
        SELECT 1 FROM library_private_items privacy_item
        WHERE privacy_item.item_id = media_library.id
    )
    AND NOT EXISTS (
        SELECT 1 FROM library_private_folders privacy_folder
        WHERE privacy_folder.source_id = media_library.source_id
          AND (privacy_folder.relative_path = ''
            OR media_library.relative_path = privacy_folder.relative_path
            OR media_library.relative_path LIKE privacy_folder.relative_path || '/%')
    )";

fn locked_library_filter_for(alias: &str) -> String {
    debug_assert!(alias
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_'));
    LOCKED_LIBRARY_FILTER_SQL.replace("media_library", alias)
}

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
    library_read_connections: Arc<Vec<Mutex<Connection>>>,
    next_library_reader: Arc<AtomicUsize>,
    library_privacy_unlocked: Arc<AtomicBool>,
    library_privacy_last_activity_ms: Arc<AtomicI64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryPrivacySettingsRecord {
    salt: String,
    verifier: String,
    auto_lock_minutes: u32,
}

#[derive(Debug, Clone)]
pub struct VisualSignatureTarget {
    pub id: String,
    pub path: String,
    pub title: String,
    pub media_kind: String,
    pub duration_seconds: Option<f64>,
    pub file_size_bytes: Option<u64>,
    pub modified_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ThemeAnalysisTarget {
    pub id: String,
    pub path: String,
    pub title: String,
    pub duration_seconds: Option<f64>,
    pub tags: Vec<String>,
    pub description: Option<String>,
    pub transcript: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub modified_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct SemanticTranscriptDocument {
    pub item_id: String,
    pub title: String,
    pub text: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct StoredTranscriptEmbedding {
    pub item_id: String,
    pub vector_json: String,
}

#[derive(Debug, Clone)]
pub struct TranscriptExcerpt {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub text: String,
    pub language: Option<String>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        let mut connection = Connection::open(path).map_err(|error| {
            AppError::internal("could not open the application database", error)
        })?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| {
                AppError::internal("could not configure the application database", error)
            })?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| {
                AppError::internal("could not enable database write-ahead logging", error)
            })?;

        let transaction = connection
            .transaction()
            .map_err(|error| AppError::internal("could not begin database migration", error))?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY NOT NULL);",
        )?;
        for (version, migration) in [
            (1, MIGRATION_0001),
            (2, MIGRATION_0002),
            (3, MIGRATION_0003),
            (4, MIGRATION_0004),
            (5, MIGRATION_0005),
            (6, MIGRATION_0006),
            (7, MIGRATION_0007),
            (8, MIGRATION_0008),
            (9, MIGRATION_0009),
            (10, MIGRATION_0010),
            (11, MIGRATION_0011),
            (12, MIGRATION_0012),
            (13, MIGRATION_0013),
            (14, MIGRATION_0014),
            (15, MIGRATION_0015),
            (16, MIGRATION_0016),
            (17, MIGRATION_0017),
            (18, MIGRATION_0018),
            (19, MIGRATION_0019),
            (20, MIGRATION_0020),
            (21, MIGRATION_0021),
            (22, MIGRATION_0022),
            (23, MIGRATION_0023),
            (24, MIGRATION_0024),
            (25, MIGRATION_0025),
            (26, MIGRATION_0026),
            (27, MIGRATION_0027),
            (28, MIGRATION_0028),
            (29, MIGRATION_0029),
            (30, MIGRATION_0030),
            (31, MIGRATION_0031),
            (32, MIGRATION_0032),
            (33, MIGRATION_0033),
            (34, MIGRATION_0034),
        ] {
            let migrated: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )?;
            if !migrated {
                transaction.execute_batch(migration)?;
                transaction.execute(
                    "INSERT INTO schema_migrations(version) VALUES (?1)",
                    [version],
                )?;
            }
        }
        transaction
            .commit()
            .map_err(|error| AppError::internal("could not commit database migration", error))?;

        let privacy_configured = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM settings WHERE key = ?1)",
            [LIBRARY_PRIVACY_SETTINGS_KEY],
            |row| row.get::<_, bool>(0),
        )?;
        let mut library_read_connections = Vec::with_capacity(2);
        for _ in 0..2 {
            let library_read_connection = Connection::open(path).map_err(|error| {
                AppError::internal("could not open Library read connection", error)
            })?;
            library_read_connection
                .pragma_update(None, "foreign_keys", "ON")
                .map_err(|error| AppError::internal("could not configure Library reads", error))?;
            library_read_connection
                .pragma_update(None, "query_only", "ON")
                .map_err(|error| AppError::internal("could not protect Library reads", error))?;
            library_read_connection
                .busy_timeout(std::time::Duration::from_secs(5))
                .map_err(|error| {
                    AppError::internal("could not configure Library read timeout", error)
                })?;
            library_read_connections.push(Mutex::new(library_read_connection));
        }
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            library_read_connections: Arc::new(library_read_connections),
            next_library_reader: Arc::new(AtomicUsize::new(0)),
            library_privacy_unlocked: Arc::new(AtomicBool::new(!privacy_configured)),
            library_privacy_last_activity_ms: Arc::new(AtomicI64::new(
                Utc::now().timestamp_millis(),
            )),
        })
    }

    fn library_reader(&self) -> Result<MutexGuard<'_, Connection>, AppError> {
        let index = self.next_library_reader.fetch_add(1, Ordering::Relaxed)
            % self.library_read_connections.len();
        self.library_read_connections[index]
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "The media library is unavailable."))
    }

    pub fn save_job(
        &self,
        job: &DownloadJob,
        request: &StartDownloadRequest,
    ) -> Result<(), AppError> {
        let options_json = serde_json::to_string(&request.options)
            .map_err(|error| AppError::internal("could not serialize download options", error))?;
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::internal("could not serialize download request", error))?;
        let diagnostics_json = serde_json::to_string(&job.diagnostics)
            .map_err(|error| AppError::internal("could not serialize job diagnostics", error))?;
        let gallery_files_json = serde_json::to_string(&job.gallery_files).map_err(|error| {
            AppError::internal("could not serialize gallery file progress", error)
        })?;
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The download history database is unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, title, thumbnail, state, progress_percent, downloaded_bytes,
                total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                output_format, destination, output_path, current_operation, error,
                created_at, updated_at, completed_at, options_json, request_json,
                diagnostics_json, attempt_count, retry_of_id, gallery_files_json,
                visible_in_downloads
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25,
                       COALESCE((SELECT visible_in_downloads FROM downloads WHERE id = ?1), 1))",
            params![
                job.id,
                job.url,
                job.title,
                job.thumbnail,
                download_state_name(job.state),
                job.progress_percent,
                job.downloaded_bytes
                    .and_then(|value| i64::try_from(value).ok()),
                job.total_bytes.and_then(|value| i64::try_from(value).ok()),
                job.speed_bytes_per_second,
                job.eta_seconds.and_then(|value| i64::try_from(value).ok()),
                job.selected_quality,
                job.output_format,
                job.destination,
                job.output_path,
                job.current_operation,
                job.error,
                job.created_at.timestamp_millis(),
                job.updated_at.timestamp_millis(),
                job.completed_at.map(|value| value.timestamp_millis()),
                options_json,
                request_json,
                diagnostics_json,
                job.attempt_count,
                job.retry_of_id,
                gallery_files_json,
            ],
        )?;
        if job.state == DownloadState::Completed {
            transaction.execute(
                "INSERT INTO download_archive (url, destination, output_path, completed_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(url, destination) DO UPDATE SET
                    output_path = excluded.output_path,
                    completed_at = excluded.completed_at",
                params![
                    job.url,
                    job.destination,
                    job.output_path,
                    job.completed_at
                        .unwrap_or(job.updated_at)
                        .timestamp_millis(),
                ],
            )?;
            if let Some(output_path) = job
                .output_path
                .as_deref()
                .filter(|path| Path::new(path).is_file())
            {
                let metadata = serde_json::json!({
                    "downloadJobId": job.id,
                    "selectedQuality": job.selected_quality,
                    "outputFormat": job.output_format,
                });
                transaction.execute(
                    "INSERT INTO media_library
                        (id, path, title, source_url, thumbnail, duration_seconds, site,
                         tags_json, metadata_json, missing, added_at, updated_at, media_kind)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '[]', ?8, 0, ?9, ?9, ?10)
                     ON CONFLICT(path) DO UPDATE SET
                        title = excluded.title, source_url = excluded.source_url,
                        thumbnail = excluded.thumbnail, duration_seconds = excluded.duration_seconds,
                        site = excluded.site, metadata_json = excluded.metadata_json,
                        missing = 0, updated_at = excluded.updated_at,
                        media_kind = COALESCE(excluded.media_kind, media_library.media_kind)",
                    params![
                        Uuid::new_v4().to_string(), output_path, job.title, job.url, job.thumbnail,
                        request.analysis.duration_seconds, request.analysis.site_name,
                        metadata.to_string(), job.completed_at.unwrap_or(job.updated_at).timestamp_millis(),
                        library_media_kind_for_path(Path::new(output_path)),
                    ],
                )?;
            }
        }
        transaction.commit().map_err(|error| {
            AppError::internal("could not commit the download state update", error)
        })?;
        Ok(())
    }

    pub fn load_runtime_jobs(&self) -> Result<Vec<(DownloadJob, StartDownloadRequest)>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The download queue database is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, url, title, thumbnail, state, progress_percent, downloaded_bytes,
                    total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                output_format, destination, output_path, current_operation, error,
                created_at, updated_at, completed_at, diagnostics_json, attempt_count,
                retry_of_id, gallery_files_json, request_json
             FROM downloads
             WHERE state != 'Completed' AND request_json IS NOT NULL AND visible_in_downloads = 1
             ORDER BY created_at ASC LIMIT 500",
        )?;
        let rows = statement.query_map([], |row| {
            let request_json: String = row.get(23)?;
            Ok((job_from_row(row)?, request_json))
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            let (mut job, request_json) = row?;
            let request: StartDownloadRequest =
                serde_json::from_str(&request_json).map_err(|error| {
                    AppError::internal("could not deserialize a persisted download request", error)
                })?;
            job.scheduled_for = request.scheduled_for;
            jobs.push((job, request));
        }
        Ok(jobs)
    }

    pub fn load_engine_preferences(&self) -> Result<EnginePreferences, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The application settings database is unavailable.",
            )
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [ENGINE_PREFERENCES_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let mut preferences = match value {
            Some(value) => serde_json::from_str(&value)
                .map_err(|error| AppError::internal("could not read engine preferences", error))?,
            None => EnginePreferences::default(),
        };
        normalize_engine_preferences(&mut preferences);
        validate_engine_preferences(&preferences)?;
        Ok(preferences)
    }

    pub fn save_engine_preferences(&self, preferences: &EnginePreferences) -> Result<(), AppError> {
        let value = serde_json::to_string(preferences)
            .map_err(|error| AppError::internal("could not serialize engine preferences", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The application settings database is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                                updated_at = excluded.updated_at",
            params![ENGINE_PREFERENCES_KEY, value, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn load_desktop_preferences(&self) -> Result<DesktopPreferences, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Desktop settings are unavailable.")
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [DESKTOP_PREFERENCES_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let mut preferences: DesktopPreferences = value
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|error| AppError::internal("could not read desktop settings", error))
            })
            .transpose()
            .map(|value| value.unwrap_or_default())?;
        if !matches!(preferences.interface_scale_percent, 90 | 100 | 110 | 125) {
            preferences.interface_scale_percent = 100;
        }
        Ok(preferences)
    }

    pub fn save_desktop_preferences(
        &self,
        preferences: &DesktopPreferences,
    ) -> Result<(), AppError> {
        let value = serde_json::to_string(preferences)
            .map_err(|error| AppError::internal("could not serialize desktop settings", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Desktop settings are unavailable.")
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![DESKTOP_PREFERENCES_KEY, value, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn load_library_theme_settings(&self) -> Result<LibraryThemeSettings, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Local AI settings are unavailable.")
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [LIBRARY_THEME_SETTINGS_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let settings = value
            .map(|value| {
                serde_json::from_str(&value)
                    .map_err(|error| AppError::internal("could not read local AI settings", error))
            })
            .transpose()?
            .unwrap_or_default();
        validate_library_theme_settings(&settings)?;
        Ok(settings)
    }

    pub fn save_library_theme_settings(
        &self,
        settings: &LibraryThemeSettings,
    ) -> Result<(), AppError> {
        validate_library_theme_settings(settings)?;
        let value = serde_json::to_string(settings)
            .map_err(|error| AppError::internal("could not serialize local AI settings", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Local AI settings are unavailable.")
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![LIBRARY_THEME_SETTINGS_KEY, value, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn library_privacy_status(&self) -> Result<LibraryPrivacyStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy settings are unavailable.",
            )
        })?;
        let settings = load_library_privacy_settings_record(&connection)?;
        let private_source_count = connection.query_row(
            "SELECT COUNT(*) FROM library_sources WHERE private = 1",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let private_folder_count =
            connection.query_row("SELECT COUNT(*) FROM library_private_folders", [], |row| {
                row.get::<_, i64>(0)
            })?;
        let private_item_count =
            connection.query_row("SELECT COUNT(*) FROM library_private_items", [], |row| {
                row.get::<_, i64>(0)
            })?;
        let configured = settings.is_some();
        Ok(LibraryPrivacyStatus {
            configured,
            locked: configured && !self.library_privacy_unlocked.load(Ordering::SeqCst),
            auto_lock_minutes: settings.map_or(15, |record| record.auto_lock_minutes),
            private_source_count: usize::try_from(private_source_count).unwrap_or(usize::MAX),
            private_folder_count: usize::try_from(private_folder_count).unwrap_or(usize::MAX),
            private_item_count: usize::try_from(private_item_count).unwrap_or(usize::MAX),
        })
    }

    pub fn configure_library_privacy(
        &self,
        passphrase: &str,
        auto_lock_minutes: u32,
    ) -> Result<LibraryPrivacyStatus, AppError> {
        validate_library_privacy_passphrase(passphrase)?;
        if !(1..=240).contains(&auto_lock_minutes) {
            return Err(AppError::new(
                "invalid_library_privacy_timeout",
                "Choose an automatic lock timeout between 1 minute and 4 hours.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy settings are unavailable.",
            )
        })?;
        if load_library_privacy_settings_record(&connection)?.is_some()
            && !self.library_privacy_unlocked.load(Ordering::SeqCst)
        {
            return Err(library_locked_error());
        }
        let salt = Uuid::new_v4().to_string();
        let record = LibraryPrivacySettingsRecord {
            verifier: derive_library_privacy_verifier(passphrase, &salt),
            salt,
            auto_lock_minutes,
        };
        let value = serde_json::to_string(&record).map_err(|error| {
            AppError::internal("could not serialize Library privacy settings", error)
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![LIBRARY_PRIVACY_SETTINGS_KEY, value, Utc::now().timestamp_millis()],
        )?;
        drop(connection);
        self.library_privacy_unlocked.store(true, Ordering::SeqCst);
        self.touch_library_privacy();
        self.library_privacy_status()
    }

    pub fn unlock_library_privacy(
        &self,
        passphrase: &str,
    ) -> Result<LibraryPrivacyStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy settings are unavailable.",
            )
        })?;
        let record = load_library_privacy_settings_record(&connection)?.ok_or_else(|| {
            AppError::new(
                "library_privacy_not_configured",
                "Set a Library privacy passphrase before using the lock.",
            )
        })?;
        drop(connection);
        let candidate = derive_library_privacy_verifier(passphrase, &record.salt);
        if !constant_time_equal(candidate.as_bytes(), record.verifier.as_bytes()) {
            return Err(AppError::new(
                "library_privacy_unlock_failed",
                "The Library privacy passphrase did not match.",
            ));
        }
        self.library_privacy_unlocked.store(true, Ordering::SeqCst);
        self.touch_library_privacy();
        self.library_privacy_status()
    }

    pub fn disable_library_privacy(
        &self,
        passphrase: &str,
    ) -> Result<LibraryPrivacyStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy settings are unavailable.",
            )
        })?;
        let record = load_library_privacy_settings_record(&connection)?.ok_or_else(|| {
            AppError::new(
                "library_privacy_not_configured",
                "Library privacy protection is not configured.",
            )
        })?;
        let candidate = derive_library_privacy_verifier(passphrase, &record.salt);
        if !constant_time_equal(candidate.as_bytes(), record.verifier.as_bytes()) {
            return Err(AppError::new(
                "library_privacy_disable_failed",
                "The Library privacy passphrase did not match.",
            ));
        }
        connection.execute(
            "DELETE FROM settings WHERE key = ?1",
            [LIBRARY_PRIVACY_SETTINGS_KEY],
        )?;
        drop(connection);
        self.library_privacy_unlocked.store(true, Ordering::SeqCst);
        self.library_privacy_last_activity_ms
            .store(0, Ordering::SeqCst);
        self.library_privacy_status()
    }

    pub fn lock_library_privacy(&self) -> Result<LibraryPrivacyStatus, AppError> {
        if self.library_privacy_status()?.configured {
            self.library_privacy_unlocked.store(false, Ordering::SeqCst);
        }
        self.library_privacy_status()
    }

    pub fn touch_library_privacy(&self) {
        if self.library_privacy_unlocked.load(Ordering::Relaxed) {
            self.library_privacy_last_activity_ms
                .store(Utc::now().timestamp_millis(), Ordering::Relaxed);
        }
    }

    pub fn expire_library_privacy_if_idle(&self) -> Result<bool, AppError> {
        let status = self.library_privacy_status()?;
        if !status.configured || status.locked {
            return Ok(false);
        }
        let idle_ms = Utc::now().timestamp_millis().saturating_sub(
            self.library_privacy_last_activity_ms
                .load(Ordering::Relaxed),
        );
        if idle_ms >= i64::from(status.auto_lock_minutes) * 60_000 {
            self.library_privacy_unlocked.store(false, Ordering::SeqCst);
            return Ok(true);
        }
        Ok(false)
    }

    pub fn list_library_privacy_rules(&self) -> Result<LibraryPrivacyRules, AppError> {
        self.require_library_privacy_unlocked()?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy rules are unavailable.",
            )
        })?;
        let mut item_statement =
            connection.prepare("SELECT item_id FROM library_private_items ORDER BY item_id")?;
        let item_ids = item_statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        let mut folder_statement = connection.prepare(
            "SELECT source_id, relative_path FROM library_private_folders
             ORDER BY source_id, relative_path COLLATE NOCASE",
        )?;
        let folders = folder_statement
            .query_map([], |row| {
                Ok(LibraryPrivateFolder {
                    source_id: row.get(0)?,
                    relative_path: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LibraryPrivacyRules { item_ids, folders })
    }

    pub fn set_library_items_private(
        &self,
        item_ids: Vec<String>,
        private: bool,
    ) -> Result<LibraryPrivacyRules, AppError> {
        self.require_library_privacy_unlocked()?;
        if item_ids.is_empty() || item_ids.len() > 500 {
            return Err(AppError::new(
                "invalid_library_privacy_selection",
                "Choose between 1 and 500 Library items.",
            ));
        }
        let mut unique = Vec::with_capacity(item_ids.len());
        let mut seen = HashSet::new();
        for item_id in item_ids {
            validate_uuid(&item_id, "library item")?;
            if seen.insert(item_id.clone()) {
                unique.push(item_id);
            }
        }
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy rules are unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        for item_id in unique {
            let exists = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1)",
                [&item_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Err(AppError::new(
                    "library_item_not_found",
                    "A selected Library item is no longer available.",
                ));
            }
            if private {
                transaction.execute(
                    "INSERT OR IGNORE INTO library_private_items (item_id, created_at) VALUES (?1, ?2)",
                    params![item_id, Utc::now().timestamp_millis()],
                )?;
            } else {
                transaction.execute(
                    "DELETE FROM library_private_items WHERE item_id = ?1",
                    [item_id],
                )?;
            }
        }
        transaction.commit()?;
        drop(connection);
        self.list_library_privacy_rules()
    }

    pub fn set_library_folder_private(
        &self,
        source_id: &str,
        relative_path: &str,
        private: bool,
    ) -> Result<LibraryPrivacyRules, AppError> {
        self.require_library_privacy_unlocked()?;
        validate_uuid(source_id, "library source")?;
        let relative_path = normalize_private_folder_path(relative_path)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy rules are unavailable.",
            )
        })?;
        let exists = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_sources WHERE id = ?1)",
            [source_id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Err(AppError::new(
                "library_source_not_found",
                "The selected Library source no longer exists.",
            ));
        }
        if private {
            connection.execute(
                "INSERT OR IGNORE INTO library_private_folders (source_id, relative_path, created_at)
                 VALUES (?1, ?2, ?3)",
                params![source_id, relative_path, Utc::now().timestamp_millis()],
            )?;
        } else {
            connection.execute(
                "DELETE FROM library_private_folders WHERE source_id = ?1 AND relative_path = ?2",
                params![source_id, relative_path],
            )?;
        }
        drop(connection);
        self.list_library_privacy_rules()
    }

    pub fn require_library_privacy_unlocked(&self) -> Result<(), AppError> {
        let status = self.library_privacy_status()?;
        if status.configured && status.locked {
            Err(library_locked_error())
        } else {
            Ok(())
        }
    }

    fn filter_visible_library_records<T>(
        &self,
        records: Vec<T>,
        item_id: impl Fn(&T) -> &str,
    ) -> Result<Vec<T>, AppError> {
        if !self.is_library_privacy_locked() {
            return Ok(records);
        }
        let mut visible = Vec::with_capacity(records.len());
        for record in records {
            if !self.is_library_item_private(item_id(&record))? {
                visible.push(record);
            }
        }
        Ok(visible)
    }

    fn filter_visible_library_item_ids(
        &self,
        item_ids: Vec<String>,
    ) -> Result<Vec<String>, AppError> {
        self.filter_visible_library_records(item_ids, |item_id| item_id)
    }

    pub fn is_library_privacy_locked(&self) -> bool {
        !self.library_privacy_unlocked.load(Ordering::SeqCst)
    }

    pub fn is_library_item_private(&self, item_id: &str) -> Result<bool, AppError> {
        validate_uuid(item_id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library privacy rules are unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM media_library media
                    LEFT JOIN library_sources source ON source.id = media.source_id
                    WHERE media.id = ?1 AND (
                        COALESCE(source.private, 0) = 1
                        OR EXISTS(SELECT 1 FROM library_private_items item_rule WHERE item_rule.item_id = media.id)
                        OR EXISTS(
                            SELECT 1 FROM library_private_folders folder_rule
                            WHERE folder_rule.source_id = media.source_id
                              AND (folder_rule.relative_path = ''
                                OR media.relative_path = folder_rule.relative_path
                                OR media.relative_path LIKE folder_rule.relative_path || '/%')
                        )
                    )
                )",
                [item_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn onboarding_completed(&self) -> Result<bool, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Onboarding settings are unavailable.",
            )
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [ONBOARDING_COMPLETED_KEY],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.as_deref() == Some("true"))
    }

    pub fn complete_onboarding(&self) -> Result<(), AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Onboarding settings are unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, 'true', ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = 'true', updated_at = excluded.updated_at",
            params![ONBOARDING_COMPLETED_KEY, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn load_queue_settings(&self) -> Result<QueueSettings, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Application settings are unavailable.",
            )
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                [QUEUE_SETTINGS_KEY],
                |row| row.get(0),
            )
            .optional()?;
        let mut settings: QueueSettings = value
            .map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(|error| AppError::internal("could not read queue settings", error))?
            .unwrap_or_default();
        settings.concurrent_downloads = settings.concurrent_downloads.clamp(1, 8);
        Ok(settings)
    }

    pub fn save_queue_settings(&self, settings: &QueueSettings) -> Result<(), AppError> {
        let value = serde_json::to_string(settings)
            .map_err(|error| AppError::internal("could not serialize queue settings", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Application settings are unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                                updated_at = excluded.updated_at",
            params![QUEUE_SETTINGS_KEY, value, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn list_history(&self) -> Result<Vec<DownloadJob>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The download history database is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, url, title, thumbnail, state, progress_percent, downloaded_bytes,
                    total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                    output_format, destination, output_path, current_operation, error,
                    created_at, updated_at, completed_at, diagnostics_json, attempt_count,
                    retry_of_id, gallery_files_json
             FROM downloads WHERE state = 'Completed'
             ORDER BY completed_at DESC LIMIT 500",
        )?;
        let rows = statement.query_map([], job_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn has_completed_duplicate(&self, url: &str, destination: &str) -> Result<bool, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download history is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM download_archive
                 WHERE url = ?1 AND destination = ?2)",
                params![url, destination],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn archive_match(
        &self,
        url: &str,
        destination: &str,
    ) -> Result<Option<DownloadArchiveMatch>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download archive is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT url, destination, output_path, completed_at
                 FROM download_archive WHERE url = ?1 AND destination = ?2",
                params![url, destination],
                |row| {
                    Ok(DownloadArchiveMatch {
                        url: row.get(0)?,
                        destination: row.get(1)?,
                        output_path: row.get(2)?,
                        completed_at: timestamp_or_epoch(row.get(3)?),
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn dismiss_download_record(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download records are unavailable.")
        })?;
        let changed = connection.execute(
            "UPDATE downloads SET visible_in_downloads = 0, updated_at = ?2
             WHERE id = ?1 AND state IN ('Completed', 'Failed', 'Cancelled')",
            params![id, Utc::now().timestamp_millis()],
        )?;
        if changed == 0 {
            return Err(AppError::new(
                "download_not_removable",
                "Only completed, failed, or cancelled downloads can be removed.",
            ));
        }
        Ok(())
    }

    pub fn delete_history_entry(&self, id: &str) -> Result<(), AppError> {
        let id = Uuid::parse_str(id)
            .map_err(|_| AppError::new("invalid_job_id", "The history entry ID is invalid."))?
            .to_string();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download history is unavailable.")
        })?;
        if connection.execute(
            "DELETE FROM downloads WHERE id = ?1 AND state = 'Completed'",
            [&id],
        )? == 0
        {
            return Err(AppError::new(
                "history_not_found",
                "The history entry was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_recent_jobs(&self) -> Result<Vec<DownloadJob>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download records are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, url, title, thumbnail, state, progress_percent, downloaded_bytes,
                    total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                    output_format, destination, output_path, current_operation, error,
                    created_at, updated_at, completed_at, diagnostics_json, attempt_count,
                retry_of_id, gallery_files_json, request_json
             FROM downloads ORDER BY updated_at DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], |row| {
            let mut job = job_from_row(row)?;
            let request_json: Option<String> = row.get(23)?;
            job.scheduled_for = request_json
                .as_deref()
                .and_then(|value| serde_json::from_str::<StartDownloadRequest>(value).ok())
                .and_then(|request| request.scheduled_for);
            Ok(job)
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn download_output_path(&self, job_id: &str) -> Result<std::path::PathBuf, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download records are unavailable.")
        })?;
        let value: Option<(Option<String>, String)> = connection
            .query_row(
                "SELECT output_path, destination FROM downloads WHERE id = ?1",
                [job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (output, destination) = value
            .ok_or_else(|| AppError::new("job_not_found", "The download record was not found."))?;
        Ok(std::path::PathBuf::from(output.unwrap_or(destination)))
    }

    pub fn load_download_request(&self, job_id: &str) -> Result<StartDownloadRequest, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Download records are unavailable.")
        })?;
        let value: Option<String> = connection
            .query_row(
                "SELECT request_json FROM downloads WHERE id = ?1 AND request_json IS NOT NULL",
                [job_id],
                |row| row.get(0),
            )
            .optional()?;
        let value = value.ok_or_else(|| {
            AppError::new(
                "repeat_unavailable",
                "The original typed download options are no longer available.",
            )
        })?;
        serde_json::from_str(&value).map_err(|error| {
            AppError::internal("could not read the original download options", error)
        })
    }

    pub fn list_post_download_recipes(&self) -> Result<Vec<PostDownloadRecipe>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipes are unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, enabled, steps_json, created_at, updated_at
             FROM post_download_recipes
             ORDER BY name COLLATE NOCASE, id
             LIMIT 100",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(id, name, enabled, steps_json, created_at, updated_at)| {
                let steps = serde_json::from_str::<Vec<PostDownloadRecipeStep>>(&steps_json)
                    .map_err(|error| {
                        AppError::internal("could not read post-download recipe", error)
                    })?;
                Ok(PostDownloadRecipe {
                    id,
                    name,
                    enabled,
                    steps,
                    created_at: timestamp_or_epoch(created_at),
                    updated_at: timestamp_or_epoch(updated_at),
                })
            })
            .collect()
    }

    pub fn save_post_download_recipe(
        &self,
        mut request: SavePostDownloadRecipeRequest,
    ) -> Result<PostDownloadRecipe, AppError> {
        request.name = request.name.trim().chars().take(80).collect();
        if request.name.is_empty() || request.name.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_post_download_recipe_name",
                "Recipe names contain between 1 and 80 visible characters.",
            ));
        }
        validate_post_download_recipe_steps(&mut request.steps)?;
        let id = match request.id {
            Some(id) => {
                validate_uuid(&id, "post-download recipe identifier")?;
                id
            }
            None => Uuid::new_v4().to_string(),
        };
        let steps_json = serde_json::to_string(&request.steps).map_err(|error| {
            AppError::internal("could not serialize post-download recipe", error)
        })?;
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipes are unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO post_download_recipes
                (id, name, enabled, steps_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                enabled = excluded.enabled,
                steps_json = excluded.steps_json,
                updated_at = excluded.updated_at",
            params![id, request.name, request.enabled, steps_json, now],
        )?;
        drop(connection);
        self.list_post_download_recipes()?
            .into_iter()
            .find(|recipe| recipe.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "post_download_recipe_not_found",
                    "The recipe could not be loaded after saving.",
                )
            })
    }

    pub fn delete_post_download_recipe(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "post-download recipe identifier")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipes are unavailable.",
            )
        })?;
        if connection.execute("DELETE FROM post_download_recipes WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "post_download_recipe_not_found",
                "The post-download recipe was not found.",
            ));
        }
        Ok(())
    }

    pub fn enabled_post_download_recipes_by_ids(
        &self,
        ids: &[String],
    ) -> Result<Vec<PostDownloadRecipe>, AppError> {
        if ids.len() > 8 {
            return Err(AppError::new(
                "too_many_post_download_recipes",
                "A download can run at most eight post-download recipes.",
            ));
        }
        let mut seen = HashSet::new();
        for id in ids {
            validate_uuid(id, "post-download recipe identifier")?;
            if !seen.insert(id) {
                return Err(AppError::new(
                    "duplicate_post_download_recipe",
                    "The same post-download recipe cannot be selected twice.",
                ));
            }
        }
        let by_id = self
            .list_post_download_recipes()?
            .into_iter()
            .filter(|recipe| recipe.enabled)
            .map(|recipe| (recipe.id.clone(), recipe))
            .collect::<HashMap<_, _>>();
        ids.iter()
            .map(|id| {
                by_id.get(id).cloned().ok_or_else(|| {
                    AppError::new(
                        "post_download_recipe_unavailable",
                        "A selected post-download recipe is disabled or no longer exists.",
                    )
                })
            })
            .collect()
    }

    pub fn post_download_recipe_ids_for_download(
        &self,
        download_id: &str,
    ) -> Result<Vec<String>, AppError> {
        validate_uuid(download_id, "download identifier")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipes are unavailable.",
            )
        })?;
        let request_json = connection
            .query_row(
                "SELECT request_json FROM downloads WHERE id = ?1",
                [download_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .ok_or_else(|| {
                AppError::new(
                    "download_request_unavailable",
                    "The completed download request is unavailable.",
                )
            })?;
        let request =
            serde_json::from_str::<StartDownloadRequest>(&request_json).map_err(|error| {
                AppError::internal("could not read completed download request", error)
            })?;
        Ok(request.post_download_recipe_ids)
    }

    pub fn claim_post_download_recipe_run(
        &self,
        recipe: &PostDownloadRecipe,
        download_id: &str,
    ) -> Result<Option<PostDownloadRecipeRun>, AppError> {
        validate_uuid(download_id, "download identifier")?;
        let now = Utc::now();
        let run = PostDownloadRecipeRun {
            id: Uuid::new_v4().to_string(),
            recipe_id: recipe.id.clone(),
            recipe_name: recipe.name.clone(),
            download_id: download_id.to_owned(),
            state: PostDownloadRecipeRunState::Running,
            current_step: 0,
            step_count: recipe.steps.len(),
            error: None,
            created_at: now,
            updated_at: now,
            completed_at: None,
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipe runs are unavailable.",
            )
        })?;
        let inserted = connection.execute(
            "INSERT OR IGNORE INTO post_download_recipe_runs
                (id, recipe_id, download_id, state, current_step, error, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, 'running', 0, NULL, ?4, ?4, NULL)",
            params![run.id, run.recipe_id, run.download_id, now.timestamp_millis()],
        )?;
        Ok((inserted == 1).then_some(run))
    }

    pub fn update_post_download_recipe_run(
        &self,
        run: &PostDownloadRecipeRun,
    ) -> Result<(), AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipe runs are unavailable.",
            )
        })?;
        connection.execute(
            "UPDATE post_download_recipe_runs
             SET state = ?2, current_step = ?3, error = ?4, updated_at = ?5, completed_at = ?6
             WHERE id = ?1",
            params![
                run.id,
                post_download_recipe_run_state_name(run.state),
                i64::try_from(run.current_step).unwrap_or(i64::MAX),
                run.error,
                run.updated_at.timestamp_millis(),
                run.completed_at.map(|value| value.timestamp_millis())
            ],
        )?;
        Ok(())
    }

    pub fn list_post_download_recipe_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<PostDownloadRecipeRun>, AppError> {
        if !(1..=200).contains(&limit) {
            return Err(AppError::new(
                "invalid_post_download_recipe_run_limit",
                "Recipe history returns between 1 and 200 runs.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Post-download recipe runs are unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT run.id, run.recipe_id, recipe.name, run.download_id, run.state,
                    run.current_step, json_array_length(recipe.steps_json), run.error,
                    run.created_at, run.updated_at, run.completed_at
             FROM post_download_recipe_runs run
             JOIN post_download_recipes recipe ON recipe.id = run.recipe_id
             ORDER BY run.updated_at DESC, run.id
             LIMIT ?1",
        )?;
        let runs = statement
            .query_map([i64::try_from(limit).unwrap_or(200)], |row| {
                Ok(PostDownloadRecipeRun {
                    id: row.get(0)?,
                    recipe_id: row.get(1)?,
                    recipe_name: row.get(2)?,
                    download_id: row.get(3)?,
                    state: post_download_recipe_run_state_from_name(&row.get::<_, String>(4)?),
                    current_step: usize::try_from(row.get::<_, i64>(5)?.max(0))
                        .unwrap_or(usize::MAX),
                    step_count: usize::try_from(row.get::<_, i64>(6)?.max(0)).unwrap_or(usize::MAX),
                    error: row.get(7)?,
                    created_at: timestamp_or_epoch(row.get(8)?),
                    updated_at: timestamp_or_epoch(row.get(9)?),
                    completed_at: row.get::<_, Option<i64>>(10)?.map(timestamp_or_epoch),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }

    pub fn list_presets(&self) -> Result<Vec<Preset>, AppError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Preset storage is unavailable."))?;
        let mut statement = connection.prepare(
            "SELECT id, name, options_json, created_at, updated_at
             FROM presets ORDER BY name COLLATE NOCASE ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let mut presets = Vec::new();
        for row in rows {
            let (id, name, intent_json, created_at, updated_at) = row?;
            let intent: PresetIntent = serde_json::from_str(&intent_json)
                .map_err(|error| AppError::internal("could not read a saved preset", error))?;
            presets.push(Preset {
                id,
                name,
                intent,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        Ok(presets)
    }

    pub fn save_preset(&self, request: SavePresetRequest) -> Result<Preset, AppError> {
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::new(
                "invalid_preset_name",
                "Preset names must contain between 1 and 80 characters.",
            ));
        }
        if !matches!(
            request.intent.quality.as_str(),
            "best" | "2160" | "1440" | "1080" | "720" | "480" | "audio"
        ) {
            return Err(AppError::new(
                "invalid_preset_quality",
                "The preset quality is not supported.",
            ));
        }
        if let Some(template) = request
            .intent
            .filename_template
            .as_deref()
            .filter(|template| !template.is_empty())
        {
            validate_filename_template(template)?;
        }
        let id = match request.id {
            Some(id) => Uuid::parse_str(&id)
                .map_err(|_| AppError::new("invalid_preset_id", "The preset ID is invalid."))?
                .to_string(),
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now();
        let intent_json = serde_json::to_string(&request.intent)
            .map_err(|error| AppError::internal("could not serialize the preset", error))?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Preset storage is unavailable."))?;
        connection.execute(
            "INSERT INTO presets (id, name, options_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name,
                                                   options_json = excluded.options_json,
                                                   updated_at = excluded.updated_at",
            params![id, name, intent_json, now.timestamp_millis()],
        )?;
        let created_at = connection.query_row(
            "SELECT created_at FROM presets WHERE id = ?1",
            [&id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(Preset {
            id,
            name: name.to_owned(),
            intent: request.intent,
            created_at: timestamp_or_epoch(created_at),
            updated_at: now,
        })
    }

    pub fn delete_preset(&self, id: &str) -> Result<(), AppError> {
        let id = Uuid::parse_str(id)
            .map_err(|_| AppError::new("invalid_preset_id", "The preset ID is invalid."))?
            .to_string();
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Preset storage is unavailable."))?;
        if connection.execute("DELETE FROM presets WHERE id = ?1", [&id])? == 0 {
            return Err(AppError::new(
                "preset_not_found",
                "The preset was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_site_preset_rules(&self) -> Result<Vec<SitePresetRule>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Site rule storage is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, host_suffix, path_prefix, preset_id, enabled, created_at, updated_at
             FROM site_preset_rules ORDER BY name COLLATE NOCASE, created_at",
        )?;
        let rows = statement.query_map([], site_preset_rule_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn save_site_preset_rule(
        &self,
        request: SaveSitePresetRuleRequest,
    ) -> Result<SitePresetRule, AppError> {
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(AppError::new(
                "invalid_site_rule_name",
                "Rule names must contain between 1 and 80 characters.",
            ));
        }
        let host_suffix = normalize_host_suffix(&request.host_suffix)?;
        let path_prefix = normalize_path_prefix(request.path_prefix.as_deref())?;
        let preset_id = Uuid::parse_str(&request.preset_id)
            .map_err(|_| AppError::new("invalid_preset_id", "The selected preset is invalid."))?
            .to_string();
        let id = match request.id {
            Some(id) => Uuid::parse_str(&id)
                .map_err(|_| AppError::new("invalid_site_rule_id", "The site rule ID is invalid."))?
                .to_string(),
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Site rule storage is unavailable.")
        })?;
        let preset_exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM presets WHERE id = ?1)",
            [&preset_id],
            |row| row.get(0),
        )?;
        if !preset_exists {
            return Err(AppError::new(
                "preset_not_found",
                "Save the selected preset before creating this rule.",
            ));
        }
        if request.enabled {
            let conflicting_rule: Option<String> = connection
                .query_row(
                    "SELECT name FROM site_preset_rules
                     WHERE enabled = 1 AND host_suffix = ?1
                       AND COALESCE(path_prefix, '') = COALESCE(?2, '') AND id != ?3
                     LIMIT 1",
                    params![host_suffix, path_prefix, id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(name) = conflicting_rule {
                return Err(AppError::new(
                    "duplicate_site_rule",
                    format!(
                        "An enabled site rule named ‘{name}’ already handles the same host and path."
                    ),
                ));
            }
        }
        connection.execute(
            "INSERT INTO site_preset_rules
                (id, name, host_suffix, path_prefix, preset_id, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                host_suffix = excluded.host_suffix,
                path_prefix = excluded.path_prefix,
                preset_id = excluded.preset_id,
                enabled = excluded.enabled,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                host_suffix,
                path_prefix,
                preset_id,
                request.enabled,
                now.timestamp_millis(),
            ],
        )?;
        let created_at = connection.query_row(
            "SELECT created_at FROM site_preset_rules WHERE id = ?1",
            [&id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(SitePresetRule {
            id,
            name: name.to_owned(),
            host_suffix,
            path_prefix,
            preset_id,
            enabled: request.enabled,
            created_at: timestamp_or_epoch(created_at),
            updated_at: now,
        })
    }

    pub fn delete_site_preset_rule(&self, id: &str) -> Result<(), AppError> {
        let id = Uuid::parse_str(id)
            .map_err(|_| AppError::new("invalid_site_rule_id", "The site rule ID is invalid."))?
            .to_string();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Site rule storage is unavailable.")
        })?;
        if connection.execute("DELETE FROM site_preset_rules WHERE id = ?1", [&id])? == 0 {
            return Err(AppError::new(
                "site_rule_not_found",
                "The site rule was not found.",
            ));
        }
        Ok(())
    }

    pub fn match_site_preset_rule(&self, url: &str) -> Result<Option<SitePresetRule>, AppError> {
        let parsed = url::Url::parse(url)
            .map_err(|_| AppError::new("invalid_url", "Enter a valid HTTP or HTTPS URL."))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| AppError::new("invalid_url", "The URL must include a host."))?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let path = parsed.path();
        let mut rules = self.list_site_preset_rules()?;
        rules.retain(|rule| {
            rule.enabled
                && (host == rule.host_suffix || host.ends_with(&format!(".{}", rule.host_suffix)))
                && rule
                    .path_prefix
                    .as_deref()
                    .is_none_or(|prefix| path.starts_with(prefix))
        });
        rules.sort_by_key(|rule| {
            std::cmp::Reverse((
                rule.host_suffix.len(),
                rule.path_prefix.as_deref().map_or(0, str::len),
            ))
        });
        if rules.len() > 1 {
            let specificity = |rule: &SitePresetRule| {
                (
                    rule.host_suffix.len(),
                    rule.path_prefix.as_deref().map_or(0, str::len),
                )
            };
            if specificity(&rules[0]) == specificity(&rules[1]) {
                return Err(AppError::new(
                    "ambiguous_site_rules",
                    "Multiple enabled site rules match this URL equally. Disable or edit one of them.",
                ));
            }
        }
        Ok(rules.into_iter().next())
    }

    pub fn list_subscriptions(&self) -> Result<Vec<Subscription>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, url, preset_id, destination, check_interval_minutes,
                    max_entries, enabled, last_checked_at, next_check_at, last_error,
                    created_at, updated_at
             FROM subscriptions ORDER BY name COLLATE NOCASE, created_at",
        )?;
        let rows = statement.query_map([], subscription_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn load_subscription(&self, id: &str) -> Result<Subscription, AppError> {
        let id = parse_uuid(
            id,
            "invalid_subscription_id",
            "The subscription ID is invalid.",
        )?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT id, name, url, preset_id, destination, check_interval_minutes,
                        max_entries, enabled, last_checked_at, next_check_at, last_error,
                        created_at, updated_at
                 FROM subscriptions WHERE id = ?1",
                [&id],
                subscription_from_row,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new("subscription_not_found", "The subscription was not found.")
            })
    }

    pub fn save_subscription(
        &self,
        request: SaveSubscriptionRequest,
    ) -> Result<Subscription, AppError> {
        let name = request.name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(AppError::new(
                "invalid_subscription_name",
                "Subscription names must contain between 1 and 100 characters.",
            ));
        }
        let url = validate_media_url(&request.url)?.to_string();
        let destination = validate_destination(&request.destination)?
            .to_string_lossy()
            .into_owned();
        if !(15..=10_080).contains(&request.check_interval_minutes) {
            return Err(AppError::new(
                "invalid_subscription_interval",
                "Check intervals must be between 15 minutes and 7 days.",
            ));
        }
        if !(1..=20).contains(&request.max_entries) {
            return Err(AppError::new(
                "invalid_subscription_limit",
                "Subscriptions can inspect between 1 and 20 recent entries per check.",
            ));
        }
        let preset_id = parse_uuid(
            &request.preset_id,
            "invalid_preset_id",
            "The selected preset is invalid.",
        )?;
        let id = match request.id {
            Some(id) => parse_uuid(
                &id,
                "invalid_subscription_id",
                "The subscription ID is invalid.",
            )?,
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        let preset_exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM presets WHERE id = ?1)",
            [&preset_id],
            |row| row.get(0),
        )?;
        if !preset_exists {
            return Err(AppError::new(
                "preset_not_found",
                "Save the selected preset before creating a subscription.",
            ));
        }
        connection.execute(
            "INSERT INTO subscriptions
                (id, name, url, preset_id, destination, check_interval_minutes, max_entries,
                 enabled, last_checked_at, next_check_at, last_error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, NULL, ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                url = excluded.url,
                preset_id = excluded.preset_id,
                destination = excluded.destination,
                check_interval_minutes = excluded.check_interval_minutes,
                max_entries = excluded.max_entries,
                enabled = excluded.enabled,
                next_check_at = CASE
                    WHEN excluded.enabled = 1 AND subscriptions.enabled = 0 THEN excluded.updated_at
                    ELSE subscriptions.next_check_at
                END,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                url,
                preset_id,
                destination,
                request.check_interval_minutes,
                request.max_entries,
                request.enabled,
                now.timestamp_millis(),
            ],
        )?;
        drop(connection);
        self.load_subscription(&id)
    }

    pub fn delete_subscription(&self, id: &str) -> Result<(), AppError> {
        let id = parse_uuid(
            id,
            "invalid_subscription_id",
            "The subscription ID is invalid.",
        )?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        if connection.execute("DELETE FROM subscriptions WHERE id = ?1", [&id])? == 0 {
            return Err(AppError::new(
                "subscription_not_found",
                "The subscription was not found.",
            ));
        }
        Ok(())
    }

    pub fn due_subscriptions(&self, limit: usize) -> Result<Vec<Subscription>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, url, preset_id, destination, check_interval_minutes,
                    max_entries, enabled, last_checked_at, next_check_at, last_error,
                    created_at, updated_at
             FROM subscriptions
             WHERE enabled = 1 AND next_check_at <= ?1
             ORDER BY next_check_at LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![
                Utc::now().timestamp_millis(),
                i64::try_from(limit).unwrap_or(20)
            ],
            subscription_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn record_subscription_check(
        &self,
        id: &str,
        error: Option<&str>,
    ) -> Result<Subscription, AppError> {
        let subscription = self.load_subscription(id)?;
        let now = Utc::now();
        let next = now + Duration::minutes(i64::from(subscription.check_interval_minutes));
        let error = error.map(|value| value.chars().take(500).collect::<String>());
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Subscription storage is unavailable.",
            )
        })?;
        connection.execute(
            "UPDATE subscriptions
             SET last_checked_at = ?2, next_check_at = ?3, last_error = ?4, updated_at = ?2
             WHERE id = ?1",
            params![id, now.timestamp_millis(), next.timestamp_millis(), error],
        )?;
        drop(connection);
        self.load_subscription(id)
    }

    pub fn save_transformation_task(&self, task: &TransformationTask) -> Result<(), AppError> {
        let operation = serde_json::to_string(&task.operation)
            .map_err(|error| AppError::internal("could not serialize transformation", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Transformation history is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO transformation_tasks
                (id, input_path, output_path, operation_json, state, progress_percent,
                 current_operation, error, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
                state = excluded.state,
                progress_percent = excluded.progress_percent,
                current_operation = excluded.current_operation,
                error = excluded.error,
                updated_at = excluded.updated_at,
                completed_at = excluded.completed_at",
            params![
                task.id,
                task.input_path,
                task.output_path,
                operation,
                transformation_state_name(task.state),
                task.progress_percent,
                task.current_operation,
                task.error,
                task.created_at.timestamp_millis(),
                task.updated_at.timestamp_millis(),
                task.completed_at.map(|value| value.timestamp_millis()),
            ],
        )?;
        Ok(())
    }

    pub fn list_transformation_tasks(&self) -> Result<Vec<TransformationTask>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Transformation history is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, input_path, output_path, operation_json, state, progress_percent,
                    current_operation, error, created_at, updated_at, completed_at
             FROM transformation_tasks ORDER BY created_at DESC LIMIT 500",
        )?;
        let rows = statement.query_map([], transformation_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn upsert_library_item(&self, item: &LibraryItem) -> Result<LibraryItem, AppError> {
        let tags = serde_json::to_string(&item.tags)
            .map_err(|error| AppError::internal("could not serialize media tags", error))?;
        let metadata = serde_json::to_string(&item.metadata)
            .map_err(|error| AppError::internal("could not serialize media metadata", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection.execute(
            "INSERT INTO media_library
                (id, path, title, source_url, thumbnail, duration_seconds, site, tags_json,
                 metadata_json, missing, added_at, updated_at, source_id, relative_path, media_kind,
                 favorite, rating)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                source_url = COALESCE(excluded.source_url, media_library.source_url),
                thumbnail = COALESCE(media_library.thumbnail, excluded.thumbnail),
                duration_seconds = COALESCE(excluded.duration_seconds, media_library.duration_seconds),
                site = COALESCE(excluded.site, media_library.site),
                tags_json = excluded.tags_json,
                metadata_json = excluded.metadata_json,
                missing = excluded.missing,
                updated_at = excluded.updated_at,
                source_id = COALESCE(excluded.source_id, media_library.source_id),
                relative_path = COALESCE(excluded.relative_path, media_library.relative_path),
                media_kind = COALESCE(excluded.media_kind, media_library.media_kind),
                favorite = excluded.favorite,
                rating = excluded.rating",
            params![
                item.id, item.path, item.title, item.source_url, item.thumbnail,
                item.duration_seconds, item.site, tags, metadata, item.missing,
                item.added_at.timestamp_millis(), item.updated_at.timestamp_millis(),
                item.source_id, item.relative_path, item.media_kind.map(library_media_kind_name),
                item.favorite, item.rating,
            ],
        )?;
        let item_id: String = connection.query_row(
            "SELECT id FROM media_library WHERE path = ?1",
            [&item.path],
            |row| row.get(0),
        )?;
        let mut stored = item.clone();
        stored.id.clone_from(&item_id);
        save_library_metadata(&connection, &stored)?;
        save_extracted_library_metadata(&connection, &item_id, &item.metadata)?;
        drop(connection);
        self.get_library_item(&item_id)
    }

    pub fn upsert_scanned_library_item(&self, item: &LibraryItem) -> Result<LibraryItem, AppError> {
        let tags = serde_json::to_string(&item.tags)
            .map_err(|error| AppError::internal("could not serialize media tags", error))?;
        let metadata = serde_json::to_string(&item.metadata)
            .map_err(|error| AppError::internal("could not serialize media metadata", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection.execute(
            "INSERT INTO media_library
                (id, path, title, source_url, thumbnail, duration_seconds, site, tags_json,
                 metadata_json, missing, added_at, updated_at, source_id, relative_path, media_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(path) DO UPDATE SET
                source_url = COALESCE(excluded.source_url, media_library.source_url),
                thumbnail = COALESCE(media_library.thumbnail, excluded.thumbnail),
                duration_seconds = COALESCE(excluded.duration_seconds, media_library.duration_seconds),
                site = COALESCE(excluded.site, media_library.site),
                metadata_json = excluded.metadata_json,
                missing = 0,
                updated_at = excluded.updated_at,
                source_id = excluded.source_id,
                relative_path = excluded.relative_path,
                media_kind = excluded.media_kind",
            params![
                item.id,
                item.path,
                item.title,
                item.source_url,
                item.thumbnail,
                item.duration_seconds,
                item.site,
                tags,
                metadata,
                item.missing,
                item.added_at.timestamp_millis(),
                item.updated_at.timestamp_millis(),
                item.source_id,
                item.relative_path,
                item.media_kind.map(library_media_kind_name),
            ],
        )?;
        let item_id: String = connection.query_row(
            "SELECT id FROM media_library WHERE path = ?1",
            [&item.path],
            |row| row.get(0),
        )?;
        connection.execute(
            "INSERT INTO library_media_metadata
                (item_id, file_size_bytes, modified_at_ms, state, updated_at)
             VALUES (?1, ?2, ?3, 'pending', ?4)
             ON CONFLICT(item_id) DO UPDATE SET
                width = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                  AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                             THEN library_media_metadata.width ELSE NULL END,
                height = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                   AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                              THEN library_media_metadata.height ELSE NULL END,
                fps = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                           THEN library_media_metadata.fps ELSE NULL END,
                video_codec = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                        AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                                   THEN library_media_metadata.video_codec ELSE NULL END,
                audio_codec = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                        AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                                   THEN library_media_metadata.audio_codec ELSE NULL END,
                container = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                      AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                                 THEN library_media_metadata.container ELSE NULL END,
                dynamic_range = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                          AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                                     THEN library_media_metadata.dynamic_range ELSE NULL END,
                error = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                  AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                             THEN library_media_metadata.error ELSE NULL END,
                state = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                  AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                             THEN library_media_metadata.state ELSE 'pending' END,
                file_size_bytes = excluded.file_size_bytes,
                modified_at_ms = excluded.modified_at_ms,
                updated_at = CASE WHEN library_media_metadata.file_size_bytes IS excluded.file_size_bytes
                                       AND library_media_metadata.modified_at_ms IS excluded.modified_at_ms
                                  THEN library_media_metadata.updated_at ELSE excluded.updated_at END",
            params![
                item_id,
                item.file_size_bytes.and_then(|value| i64::try_from(value).ok()),
                item.modified_at_ms,
                Utc::now().timestamp_millis(),
            ],
        )?;
        save_extracted_library_metadata(&connection, &item_id, &item.metadata)?;
        drop(connection);
        self.get_library_item(&item_id)
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, media_library.duration_seconds, site,
             tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
             relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
             video_codec, audio_codec, container, dynamic_range,
             COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             WHERE 1 = 1{privacy_filter}
             ORDER BY added_at DESC LIMIT 5000"
        ))?;
        let rows = statement.query_map([], library_item_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_library_transcript_targets_after(
        &self,
        after_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(String, String)>, AppError> {
        if !(1..=1_000).contains(&limit) {
            return Err(AppError::new(
                "invalid_transcript_index_limit",
                "Transcript indexing batches contain between 1 and 1,000 videos.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT id, path
             FROM media_library
             WHERE missing = 0
               AND media_kind IN ('video', 'both')
               AND (?1 IS NULL OR id > ?1)
               {privacy_filter}
             ORDER BY id
             LIMIT ?2"
        ))?;
        let rows = statement.query_map(
            params![after_id, i64::try_from(limit).unwrap_or(1_000)],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_visual_signature_targets_after(
        &self,
        after_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<VisualSignatureTarget>, AppError> {
        if !(1..=500).contains(&limit) {
            return Err(AppError::new(
                "invalid_visual_similarity_batch",
                "Visual similarity batches contain between 1 and 500 items.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The visual similarity index is unavailable.",
            )
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media.id, media.path, media.title, COALESCE(media.media_kind, ''),
                    media.duration_seconds, metadata.file_size_bytes, metadata.modified_at_ms
             FROM media_library media
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             LEFT JOIN library_visual_signatures signature ON signature.item_id = media.id
             WHERE media.missing = 0
               AND media.media_kind IN ('video', 'image', 'both')
               AND (?1 IS NULL OR media.id > ?1)
               AND (
                    signature.item_id IS NULL
                    OR NOT (
                        signature.file_size_bytes IS metadata.file_size_bytes
                        AND signature.modified_at_ms IS metadata.modified_at_ms
                    )
               )
             {privacy_filter}
             ORDER BY media.id
             LIMIT ?2"
        ))?;
        let rows = statement.query_map(
            params![after_id, i64::try_from(limit).unwrap_or(500)],
            |row| {
                Ok(VisualSignatureTarget {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    media_kind: row.get(3)?,
                    duration_seconds: row.get(4)?,
                    file_size_bytes: row.get(5)?,
                    modified_at_ms: row.get(6)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn save_visual_signature(
        &self,
        target: &VisualSignatureTarget,
        signature: u64,
    ) -> Result<(), AppError> {
        validate_uuid(&target.id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The visual similarity index is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO library_visual_signatures
                (item_id, signature, file_size_bytes, modified_at_ms, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(item_id) DO UPDATE SET
                signature = excluded.signature,
                file_size_bytes = excluded.file_size_bytes,
                modified_at_ms = excluded.modified_at_ms,
                updated_at = excluded.updated_at",
            params![
                target.id,
                format!("{signature:016x}"),
                target.file_size_bytes,
                target.modified_at_ms,
                Utc::now().timestamp()
            ],
        )?;
        Ok(())
    }

    pub fn remove_visual_signature(&self, item_id: &str) -> Result<(), AppError> {
        validate_uuid(item_id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The visual similarity index is unavailable.",
            )
        })?;
        connection.execute(
            "DELETE FROM library_visual_signatures WHERE item_id = ?1",
            [item_id],
        )?;
        Ok(())
    }

    pub fn visual_similarity_status(&self) -> Result<LibraryVisualSimilarityStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The visual similarity index is unavailable.",
            )
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let eligible_items: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*) FROM media_library
                 WHERE missing = 0 AND media_kind IN ('video', 'image', 'both'){privacy_filter}"
            ),
            [],
            |row| row.get(0),
        )?;
        let media_privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let indexed_items: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*)
             FROM media_library media
             JOIN library_visual_signatures signature ON signature.item_id = media.id
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE media.missing = 0
               AND media.media_kind IN ('video', 'image', 'both')
               AND signature.file_size_bytes IS metadata.file_size_bytes
               AND signature.modified_at_ms IS metadata.modified_at_ms{media_privacy_filter}"
            ),
            [],
            |row| row.get(0),
        )?;
        let eligible_items = usize::try_from(eligible_items).unwrap_or(usize::MAX);
        let indexed_items = usize::try_from(indexed_items).unwrap_or(usize::MAX);
        Ok(LibraryVisualSimilarityStatus {
            eligible_items,
            indexed_items,
            stale_items: eligible_items.saturating_sub(indexed_items),
        })
    }

    pub fn find_visually_similar_items(
        &self,
        item_id: &str,
        limit: usize,
        minimum_similarity_percent: f64,
    ) -> Result<Vec<LibraryVisualSimilarityMatch>, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        if !(1..=50).contains(&limit)
            || !minimum_similarity_percent.is_finite()
            || !(50.0..=100.0).contains(&minimum_similarity_percent)
        {
            return Err(AppError::new(
                "invalid_visual_similarity_query",
                "Similarity queries return 1 to 50 matches with a threshold from 50 to 100 percent.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The visual similarity index is unavailable.",
            )
        })?;
        let target: Option<String> = connection
            .query_row(
                "SELECT signature.signature
                 FROM library_visual_signatures signature
                 JOIN media_library media ON media.id = signature.item_id
                 LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
                 WHERE signature.item_id = ?1
                   AND media.missing = 0
                   AND signature.file_size_bytes IS metadata.file_size_bytes
                   AND signature.modified_at_ms IS metadata.modified_at_ms",
                [item_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(target) = target.and_then(|value| u64::from_str_radix(&value, 16).ok()) else {
            return Err(AppError::new(
                "visual_similarity_not_indexed",
                "Build or refresh the visual index before finding similar media.",
            ));
        };
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let mut statement = connection.prepare(&format!(
            "SELECT signature.item_id, signature.signature
             FROM library_visual_signatures signature
             JOIN media_library media ON media.id = signature.item_id
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE signature.item_id != ?1
               AND media.missing = 0
               AND media.media_kind IN ('video', 'image', 'both')
               AND signature.file_size_bytes IS metadata.file_size_bytes
               AND signature.modified_at_ms IS metadata.modified_at_ms
               {privacy_filter}
             LIMIT 250000"
        ))?;
        let rows = statement.query_map([item_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut scored = rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter_map(|(id, signature)| {
                let signature = u64::from_str_radix(&signature, 16).ok()?;
                let distance = (target ^ signature).count_ones();
                let similarity = (1.0 - f64::from(distance) / 64.0) * 100.0;
                (similarity >= minimum_similarity_percent).then_some((id, similarity, distance))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        });
        scored.truncate(limit);
        let ids = scored
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect::<Vec<_>>();
        drop(statement);
        drop(connection);
        let items = self
            .list_library_items_by_ids(ids)?
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(scored
            .into_iter()
            .filter_map(|(id, similarity_percent, hamming_distance)| {
                Some(LibraryVisualSimilarityMatch {
                    item: items.get(&id)?.clone(),
                    similarity_percent,
                    hamming_distance,
                })
            })
            .collect())
    }

    pub fn find_near_duplicate_videos(
        &self,
        minimum_visual_similarity_percent: f64,
        maximum_duration_delta_seconds: f64,
        limit: usize,
    ) -> Result<Vec<LibraryNearDuplicateMatch>, AppError> {
        if !minimum_visual_similarity_percent.is_finite()
            || !(70.0..=100.0).contains(&minimum_visual_similarity_percent)
            || !maximum_duration_delta_seconds.is_finite()
            || !(0.0..=600.0).contains(&maximum_duration_delta_seconds)
            || !(1..=200).contains(&limit)
        {
            return Err(AppError::new(
                "invalid_near_duplicate_query",
                "Near-duplicate review uses a 70–100% visual threshold, a 0–600 second duration tolerance, and returns 1–200 pairs.",
            ));
        }
        #[derive(Clone)]
        struct CandidateRow {
            id: String,
            signature: u64,
            duration: f64,
            size: Option<i64>,
            width: Option<i64>,
            height: Option<i64>,
            container: Option<String>,
            video_codec: Option<String>,
            audio_codec: Option<String>,
            sha256: Option<String>,
        }
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Near-duplicate review is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT media.id, signature.signature, media.duration_seconds,
                    metadata.file_size_bytes, metadata.width, metadata.height,
                    metadata.container, metadata.video_codec, metadata.audio_codec,
                    content.sha256
             FROM media_library media
             JOIN library_visual_signatures signature ON signature.item_id = media.id
             JOIN library_media_metadata metadata ON metadata.item_id = media.id
             LEFT JOIN library_content_hashes content
               ON content.item_id = media.id
              AND content.file_size_bytes IS metadata.file_size_bytes
              AND content.modified_at_ms IS metadata.modified_at_ms
             WHERE media.missing = 0
               AND media.media_kind IN ('video', 'both')
               AND media.duration_seconds IS NOT NULL
               AND media.duration_seconds >= 0
               AND signature.file_size_bytes IS metadata.file_size_bytes
               AND signature.modified_at_ms IS metadata.modified_at_ms
               {privacy_filter}
             ORDER BY media.duration_seconds, media.id
             LIMIT 20000"
        ))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, Option<i64>>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                ))
            })?
            .filter_map(|row| match row {
                Ok((
                    id,
                    signature,
                    duration,
                    size,
                    width,
                    height,
                    container,
                    video_codec,
                    audio_codec,
                    sha256,
                )) => u64::from_str_radix(&signature, 16)
                    .ok()
                    .map(|signature| CandidateRow {
                        id,
                        signature,
                        duration,
                        size,
                        width,
                        height,
                        container,
                        video_codec,
                        audio_codec,
                        sha256,
                    }),
                Err(_) => None,
            })
            .collect::<Vec<_>>();
        drop(statement);
        drop(connection);

        #[derive(Clone)]
        struct ScoredPair {
            left_id: String,
            right_id: String,
            visual: f64,
            duration_delta: f64,
            size_delta: Option<i64>,
            same_resolution: bool,
            same_container: bool,
            same_video_codec: bool,
            same_audio_codec: bool,
            exact_hash_compared: bool,
        }
        let same_optional_text = |left: &Option<String>, right: &Option<String>| matches!((left, right), (Some(left), Some(right)) if left.eq_ignore_ascii_case(right));
        let mut pairs = Vec::new();
        let mut comparisons = 0_usize;
        'outer: for (left_index, left) in rows.iter().enumerate() {
            for right in rows.iter().skip(left_index + 1) {
                let duration_delta = right.duration - left.duration;
                if duration_delta > maximum_duration_delta_seconds {
                    break;
                }
                comparisons += 1;
                if comparisons > 5_000_000 {
                    break 'outer;
                }
                let distance = (left.signature ^ right.signature).count_ones();
                let visual = (1.0 - f64::from(distance) / 64.0) * 100.0;
                if visual < minimum_visual_similarity_percent {
                    continue;
                }
                let exact_hash_compared = left.sha256.is_some() && right.sha256.is_some();
                if matches!((&left.sha256, &right.sha256), (Some(left), Some(right)) if left == right)
                {
                    continue;
                }
                let size_delta = left.size.zip(right.size).map(|(left, right)| {
                    let difference = i128::from(left) - i128::from(right);
                    i64::try_from(difference.abs()).unwrap_or(i64::MAX)
                });
                pairs.push(ScoredPair {
                    left_id: left.id.clone(),
                    right_id: right.id.clone(),
                    visual,
                    duration_delta,
                    size_delta,
                    same_resolution: left.width.is_some()
                        && left.width == right.width
                        && left.height.is_some()
                        && left.height == right.height,
                    same_container: same_optional_text(&left.container, &right.container),
                    same_video_codec: same_optional_text(&left.video_codec, &right.video_codec),
                    same_audio_codec: same_optional_text(&left.audio_codec, &right.audio_codec),
                    exact_hash_compared,
                });
            }
        }
        pairs.sort_by(|left, right| {
            right
                .visual
                .total_cmp(&left.visual)
                .then_with(|| left.duration_delta.total_cmp(&right.duration_delta))
                .then_with(|| left.left_id.cmp(&right.left_id))
                .then_with(|| left.right_id.cmp(&right.right_id))
        });
        pairs.truncate(limit);
        let ids = pairs
            .iter()
            .flat_map(|pair| [pair.left_id.clone(), pair.right_id.clone()])
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let items = self
            .list_library_items_by_ids(ids)?
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(pairs
            .into_iter()
            .filter_map(|pair| {
                Some(LibraryNearDuplicateMatch {
                    left: items.get(&pair.left_id)?.clone(),
                    right: items.get(&pair.right_id)?.clone(),
                    visual_similarity_percent: pair.visual,
                    duration_delta_seconds: pair.duration_delta,
                    size_delta_bytes: pair.size_delta,
                    same_resolution: pair.same_resolution,
                    same_container: pair.same_container,
                    same_video_codec: pair.same_video_codec,
                    same_audio_codec: pair.same_audio_codec,
                    exact_hash_compared: pair.exact_hash_compared,
                })
            })
            .collect())
    }

    pub fn list_theme_analysis_targets(
        &self,
        request: &StartLibraryThemeAnalysisRequest,
        settings: &LibraryThemeSettings,
    ) -> Result<Vec<ThemeAnalysisTarget>, AppError> {
        if !(1..=500).contains(&request.max_videos) {
            return Err(AppError::new(
                "invalid_theme_analysis_limit",
                "Choose between 1 and 500 videos for a theme-analysis job.",
            ));
        }
        validate_library_theme_settings(settings)?;
        let mode = library_theme_mode_name(settings.mode);
        let (scope, source_id, folder_path, folder_scope, item_ids) = match request.scope {
            LibraryThemeAnalysisScope::All => {
                if request.source_id.is_some()
                    || request.folder_path.is_some()
                    || !request.item_ids.is_empty()
                    || request.folder_scope != LibraryFolderScope::Tree
                {
                    return Err(invalid_theme_analysis_scope());
                }
                ("all", None, String::new(), "tree", Vec::new())
            }
            LibraryThemeAnalysisScope::Folder => {
                let (Some(source_id), Some(folder_path)) =
                    (request.source_id.as_deref(), request.folder_path.as_deref())
                else {
                    return Err(invalid_theme_analysis_scope());
                };
                if !request.item_ids.is_empty() {
                    return Err(invalid_theme_analysis_scope());
                }
                validate_uuid(source_id, "library source")?;
                (
                    "folder",
                    Some(source_id.to_owned()),
                    normalize_library_folder_path(folder_path)?,
                    match request.folder_scope {
                        LibraryFolderScope::Tree => "tree",
                        LibraryFolderScope::Direct => "direct",
                    },
                    Vec::new(),
                )
            }
            LibraryThemeAnalysisScope::Items => {
                if request.source_id.is_some()
                    || request.folder_path.is_some()
                    || request.folder_scope != LibraryFolderScope::Tree
                {
                    return Err(invalid_theme_analysis_scope());
                }
                (
                    "items",
                    None,
                    String::new(),
                    "tree",
                    validate_library_item_ids(request.item_ids.clone())?,
                )
            }
        };
        let folder_like = format!("{}/%", escape_sql_like(&folder_path));
        let folder_prefix_length =
            i64::try_from(folder_path.chars().count() + 2).unwrap_or(i64::MAX);
        let item_ids = serde_json::to_string(&item_ids)
            .map_err(|error| AppError::internal("could not serialize theme selection", error))?;
        let after_id: Option<&str> = None;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The theme index is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media.id, media.path, media.title, media.duration_seconds, media.tags_json,
                    COALESCE(NULLIF(json_extract(rich.overrides_json, '$.description'), ''),
                             json_extract(rich.extracted_json, '$.description')),
                    (SELECT GROUP_CONCAT(cue_text, ' ') FROM (
                        SELECT cue_text FROM library_transcript_search
                        WHERE item_id = media.id ORDER BY start_seconds LIMIT 80
                    )),
                    metadata.file_size_bytes, metadata.modified_at_ms
             FROM media_library media
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             LEFT JOIN library_rich_metadata rich ON rich.item_id = media.id
             LEFT JOIN library_theme_analyses analysis ON analysis.item_id = media.id
             WHERE media.missing = 0 AND media.media_kind IN ('video', 'both')
               AND metadata.state = 'ready'
               AND (?1 IS NULL OR media.id > ?1)
               AND (
                    analysis.item_id IS NULL OR analysis.model != ?2 OR analysis.mode != ?3
                    OR NOT (
                        analysis.file_size_bytes IS metadata.file_size_bytes
                        AND analysis.modified_at_ms IS metadata.modified_at_ms
                    )
               )
               AND (
                    ?4 = 'all' OR
                    (?4 = 'folder' AND media.source_id = ?5 AND (
                        (?6 = '' AND (?7 = 'tree' OR
                            INSTR(REPLACE(COALESCE(media.relative_path, ''), '\\', '/'), '/') = 0)) OR
                        (?6 != '' AND
                            REPLACE(COALESCE(media.relative_path, ''), '\\', '/') LIKE ?8 ESCAPE '\\' AND
                            (?7 = 'tree' OR INSTR(SUBSTR(
                                REPLACE(COALESCE(media.relative_path, ''), '\\', '/'), ?9
                            ), '/') = 0))
                    )) OR
                    (?4 = 'items' AND media.id IN (SELECT value FROM json_each(?10)))
               )
             {privacy_filter}
             ORDER BY media.id LIMIT ?11"
        ))?;
        let rows = statement.query_map(
            params![
                after_id,
                settings.model,
                mode,
                scope,
                source_id,
                folder_path,
                folder_scope,
                folder_like,
                folder_prefix_length,
                item_ids,
                i64::from(request.max_videos)
            ],
            |row| {
                let tags_json = row.get::<_, String>(4)?;
                Ok(ThemeAnalysisTarget {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    title: row.get(2)?,
                    duration_seconds: row.get(3)?,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    description: row.get(5)?,
                    transcript: row.get(6)?,
                    file_size_bytes: row.get(7)?,
                    modified_at_ms: row.get(8)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn save_theme_analysis(
        &self,
        target: &ThemeAnalysisTarget,
        settings: &LibraryThemeSettings,
        summary: &str,
        overall_confidence: f64,
        themes: &[LibraryThemeAssignment],
    ) -> Result<(), AppError> {
        validate_uuid(&target.id, "library item")?;
        validate_library_theme_settings(settings)?;
        if summary.chars().count() > 500
            || !overall_confidence.is_finite()
            || !(0.0..=1.0).contains(&overall_confidence)
            || themes.len() > 12
        {
            return Err(AppError::new(
                "invalid_theme_analysis",
                "The local model returned an invalid theme analysis.",
            ));
        }
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The theme index is unavailable.")
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO library_theme_analyses
                (item_id, model, mode, summary, overall_confidence, file_size_bytes, modified_at_ms, analyzed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(item_id) DO UPDATE SET
                model = excluded.model, mode = excluded.mode, summary = excluded.summary,
                overall_confidence = excluded.overall_confidence,
                file_size_bytes = excluded.file_size_bytes,
                modified_at_ms = excluded.modified_at_ms, analyzed_at = excluded.analyzed_at",
            params![
                target.id,
                settings.model,
                library_theme_mode_name(settings.mode),
                summary,
                overall_confidence,
                target.file_size_bytes,
                target.modified_at_ms,
                Utc::now().timestamp_millis(),
            ],
        )?;
        transaction.execute(
            "DELETE FROM library_theme_assignments WHERE item_id = ?1",
            [&target.id],
        )?;
        for theme in themes {
            validate_theme_assignment(theme)?;
            let evidence = serde_json::to_string(&theme.evidence)
                .map_err(|error| AppError::internal("could not serialize theme evidence", error))?;
            transaction.execute(
                "INSERT INTO library_theme_assignments
                    (item_id, theme_key, label, confidence, evidence_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    target.id,
                    theme.key,
                    theme.label,
                    theme.confidence,
                    evidence
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_theme_analysis(&self, item_id: &str) -> Result<(), AppError> {
        validate_uuid(item_id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The theme index is unavailable.")
        })?;
        connection.execute(
            "DELETE FROM library_theme_analyses WHERE item_id = ?1",
            [item_id],
        )?;
        Ok(())
    }

    pub fn library_theme_index_status(
        &self,
        settings: &LibraryThemeSettings,
    ) -> Result<LibraryThemeIndexStatus, AppError> {
        validate_library_theme_settings(settings)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The theme index is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let eligible_items: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*) FROM media_library media
             JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE media.missing = 0 AND media.media_kind IN ('video', 'both')
               AND metadata.state = 'ready'{privacy_filter}"
            ),
            [],
            |row| row.get(0),
        )?;
        let indexed_items: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*)
             FROM library_theme_analyses analysis
             JOIN media_library media ON media.id = analysis.item_id
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE media.missing = 0 AND media.media_kind IN ('video', 'both')
               AND metadata.state = 'ready'
               AND analysis.model = ?1 AND analysis.mode = ?2
               AND analysis.file_size_bytes IS metadata.file_size_bytes
               AND analysis.modified_at_ms IS metadata.modified_at_ms{privacy_filter}"
            ),
            params![settings.model, library_theme_mode_name(settings.mode)],
            |row| row.get(0),
        )?;
        let theme_count: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(DISTINCT assignment.theme_key)
             FROM library_theme_assignments assignment
             JOIN library_theme_analyses analysis ON analysis.item_id = assignment.item_id
             JOIN media_library media ON media.id = assignment.item_id
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = assignment.item_id
             WHERE media.missing = 0 AND metadata.state = 'ready'
               AND analysis.model = ?1 AND analysis.mode = ?2
               AND analysis.file_size_bytes IS metadata.file_size_bytes
               AND analysis.modified_at_ms IS metadata.modified_at_ms{privacy_filter}"
            ),
            params![settings.model, library_theme_mode_name(settings.mode)],
            |row| row.get(0),
        )?;
        let eligible_items = usize::try_from(eligible_items).unwrap_or(usize::MAX);
        let indexed_items = usize::try_from(indexed_items).unwrap_or(usize::MAX);
        Ok(LibraryThemeIndexStatus {
            eligible_items,
            indexed_items,
            stale_items: eligible_items.saturating_sub(indexed_items),
            theme_count: usize::try_from(theme_count).unwrap_or(usize::MAX),
        })
    }

    pub fn list_library_theme_analyses(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryThemeAnalysis>, AppError> {
        if !(1..=100).contains(&limit) {
            return Err(AppError::new(
                "invalid_theme_analysis_limit",
                "Theme-analysis history returns between 1 and 100 records.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The theme index is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT analysis.item_id, media.title, analysis.model, analysis.mode,
                    analysis.summary, analysis.overall_confidence, analysis.analyzed_at
             FROM library_theme_analyses analysis
             JOIN media_library media ON media.id = analysis.item_id
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = analysis.item_id
             WHERE analysis.file_size_bytes IS metadata.file_size_bytes
               AND analysis.modified_at_ms IS metadata.modified_at_ms
               AND analysis.model = COALESCE(
                   (SELECT json_extract(value_json, '$.model') FROM settings WHERE key = 'library.theme.settings'),
                   'qwen2.5vl:7b'
               )
               AND analysis.mode = COALESCE(
                   (SELECT json_extract(value_json, '$.mode') FROM settings WHERE key = 'library.theme.settings'),
                   'deep'
               )
             ORDER BY analysis.analyzed_at DESC LIMIT ?1",
        )?;
        let rows = statement
            .query_map([i64::try_from(limit).unwrap_or(100)], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut theme_statement = connection.prepare(
            "SELECT theme_key, label, confidence, evidence_json
             FROM library_theme_assignments WHERE item_id = ?1
             ORDER BY confidence DESC, label COLLATE NOCASE",
        )?;
        let analyses = rows
            .into_iter()
            .map(
                |(item_id, title, model, mode, summary, overall_confidence, analyzed_at)| {
                    let themes = theme_statement
                        .query_map([&item_id], |row| {
                            let evidence_json = row.get::<_, String>(3)?;
                            Ok(LibraryThemeAssignment {
                                key: row.get(0)?,
                                label: row.get(1)?,
                                confidence: row.get(2)?,
                                evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
                            })
                        })?
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(LibraryThemeAnalysis {
                        item_id,
                        title,
                        model,
                        mode: library_theme_mode_from_name(&mode),
                        summary,
                        overall_confidence,
                        themes,
                        analyzed_at: DateTime::from_timestamp_millis(analyzed_at)
                            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH),
                    })
                },
            )
            .collect::<Result<Vec<_>, AppError>>()?;
        drop(theme_statement);
        drop(connection);
        self.filter_visible_library_records(analyses, |analysis| &analysis.item_id)
    }

    pub fn list_library_items_for_backup(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let limit = i64::try_from(limit).map_err(|_| {
            AppError::new(
                "invalid_library_backup_limit",
                "The library backup item limit is invalid.",
            )
        })?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, media_library.duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             WHERE 1 = 1{privacy_filter}
             ORDER BY added_at DESC LIMIT ?1"
        ))?;
        let rows = statement.query_map([limit], library_item_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_library_item_id_paths(&self) -> Result<HashMap<String, String>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare("SELECT id, path FROM media_library")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<HashMap<_, _>, _>>()
            .map_err(AppError::from)
    }

    pub fn query_library_items(
        &self,
        request: LibraryQueryRequest,
    ) -> Result<LibraryQueryResult, AppError> {
        if request.page == 0 || !(1..=60).contains(&request.page_size) {
            return Err(AppError::new(
                "invalid_library_page",
                "Library pages must contain between 1 and 60 items.",
            ));
        }
        let (mut where_clause, parameters) = build_library_query(&request)?;
        if self.is_library_privacy_locked() {
            if where_clause.is_empty() {
                where_clause.push_str(" WHERE 1 = 1");
            }
            where_clause.push_str(LOCKED_LIBRARY_FILTER_SQL);
        }
        let connection = self.library_reader()?;
        let total_items: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM media_library
                 LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
                 LEFT JOIN library_rich_metadata rich ON rich.item_id = media_library.id
                 {where_clause}"
            ),
            params_from_iter(parameters.iter()),
            |row| row.get(0),
        )?;
        let total_items = usize::try_from(total_items).unwrap_or(usize::MAX);
        let total_pages = total_items.div_ceil(request.page_size as usize).max(1);
        let page = usize::try_from(request.page)
            .unwrap_or(usize::MAX)
            .min(total_pages);
        let offset = page
            .saturating_sub(1)
            .saturating_mul(request.page_size as usize);
        let mut item_parameters = parameters;
        item_parameters.push(SqlValue::Integer(i64::from(request.page_size)));
        item_parameters.push(SqlValue::Integer(i64::try_from(offset).unwrap_or(i64::MAX)));
        let order_by = library_order_by(request.sort);
        let sql = format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                    tags_json, '{{}}' AS metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
             LEFT JOIN library_rich_metadata rich ON rich.item_id = media_library.id
             {where_clause}
             ORDER BY {order_by}
             LIMIT ?{} OFFSET ?{}",
            item_parameters.len() - 1,
            item_parameters.len()
        );
        let mut statement = connection.prepare(&sql)?;
        let items = statement
            .query_map(
                params_from_iter(item_parameters.iter()),
                library_item_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(LibraryQueryResult {
            items,
            total_items,
            page: u32::try_from(page).unwrap_or(u32::MAX),
            page_size: request.page_size,
            total_pages: u32::try_from(total_pages).unwrap_or(u32::MAX),
        })
    }

    pub fn library_facets(&self, request: LibraryFacetRequest) -> Result<LibraryFacets, AppError> {
        if let Some(source_id) = request.source_id.as_deref() {
            validate_uuid(source_id, "library source")?;
        }
        let connection = self.library_reader()?;
        let privacy_condition = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let source_condition = if request.source_id.is_some() {
            " AND media_library.source_id = ?1"
        } else {
            ""
        };
        let (total_items, available_items, missing_items): (i64, i64, i64) = connection.query_row(
            &format!(
                "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN missing = 0 THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN missing = 1 THEN 1 ELSE 0 END), 0)
             FROM media_library WHERE 1 = 1{privacy_condition}"
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let containers = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => " AND media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT container
                 FROM library_media_metadata
                 JOIN media_library ON media_library.id = item_id
                 WHERE container IS NOT NULL AND container != ''{availability_condition}{source_condition}{privacy_condition}
                 ORDER BY container COLLATE NOCASE"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let tags = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => " AND media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT json_each.value
                 FROM media_library, json_each(media_library.tags_json)
                 WHERE 1 = 1{availability_condition}{source_condition}{privacy_condition}
                 ORDER BY json_each.value COLLATE NOCASE"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let creators = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => " AND media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT COALESCE(
                    NULLIF(json_extract(rich.overrides_json, '$.creator'), ''),
                    json_extract(rich.extracted_json, '$.creator')
                 ) AS creator
                 FROM library_rich_metadata rich
                 JOIN media_library ON media_library.id = rich.item_id
                 WHERE creator IS NOT NULL AND creator != '' {availability_condition}{source_condition}{privacy_condition}
                 ORDER BY creator COLLATE NOCASE
                 LIMIT 200"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let series = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => " AND media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT COALESCE(
                    NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
                    json_extract(rich.extracted_json, '$.series')
                 ) AS series_name
                 FROM library_rich_metadata rich
                 JOIN media_library ON media_library.id = rich.item_id
                 WHERE COALESCE(
                    NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
                    json_extract(rich.extracted_json, '$.series')
                 ) IS NOT NULL
                   AND COALESCE(
                    NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
                    json_extract(rich.extracted_json, '$.series')
                 ) != '' {availability_condition}{source_condition}{privacy_condition}
                 ORDER BY series_name COLLATE NOCASE
                 LIMIT 200"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let sites = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND missing = 0",
                LibraryAvailabilityFilter::Missing => " AND missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT site
                 FROM media_library
                 WHERE site IS NOT NULL AND site != '' {availability_condition}{source_condition}{privacy_condition}
                 ORDER BY site COLLATE NOCASE
                 LIMIT 200"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let themes = {
            let availability_condition = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => " AND media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => " AND media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT assignment.theme_key, MIN(assignment.label), COUNT(DISTINCT assignment.item_id)
                 FROM library_theme_assignments assignment
                 JOIN media_library ON media_library.id = assignment.item_id
                 JOIN library_theme_analyses analysis ON analysis.item_id = assignment.item_id
                 LEFT JOIN library_media_metadata metadata ON metadata.item_id = assignment.item_id
                 WHERE analysis.file_size_bytes IS metadata.file_size_bytes
                   AND analysis.modified_at_ms IS metadata.modified_at_ms
                   AND analysis.model = COALESCE(
                       (SELECT json_extract(value_json, '$.model') FROM settings WHERE key = 'library.theme.settings'),
                       'qwen2.5vl:7b'
                   )
                   AND analysis.mode = COALESCE(
                       (SELECT json_extract(value_json, '$.mode') FROM settings WHERE key = 'library.theme.settings'),
                       'deep'
                   )
                   {availability_condition}{source_condition}{privacy_condition}
                 GROUP BY assignment.theme_key
                 ORDER BY COUNT(DISTINCT assignment.item_id) DESC, MIN(assignment.label) COLLATE NOCASE
                 LIMIT 200"
            ))?;
            let values = statement
                .query_map(params_from_iter(request.source_id.iter()), |row| {
                    Ok(LibraryThemeFacet {
                        key: row.get(0)?,
                        label: row.get(1)?,
                        item_count: usize::try_from(row.get::<_, i64>(2)?).unwrap_or(usize::MAX),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            values
        };
        let mut folder_sql = String::from(
            "SELECT source_id, relative_path FROM media_library
             WHERE source_id IS NOT NULL AND relative_path IS NOT NULL",
        );
        let mut folder_parameters = Vec::new();
        match request.availability {
            LibraryAvailabilityFilter::All => {}
            LibraryAvailabilityFilter::Available => folder_sql.push_str(" AND missing = 0"),
            LibraryAvailabilityFilter::Missing => folder_sql.push_str(" AND missing = 1"),
        }
        if let Some(source_id) = request.source_id.as_deref() {
            folder_sql.push_str(" AND source_id = ?1");
            folder_parameters.push(SqlValue::Text(source_id.to_owned()));
        }
        folder_sql.push_str(privacy_condition);
        let mut statement = connection.prepare(&folder_sql)?;
        let rows = statement.query_map(params_from_iter(folder_parameters.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut folders = HashMap::<(String, String), (usize, usize)>::new();
        for row in rows {
            let (source_id, relative_path) = row?;
            let segments = relative_path
                .replace('\\', "/")
                .split('/')
                .filter(|segment| !segment.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if segments.is_empty() {
                continue;
            }
            let directory_depth = segments.len() - 1;
            for depth in 0..=directory_depth {
                let path = segments[..depth].join("/");
                let counts = folders.entry((source_id.clone(), path)).or_default();
                counts.1 = counts.1.saturating_add(1);
                if depth == directory_depth {
                    counts.0 = counts.0.saturating_add(1);
                }
            }
        }
        let mut folders = folders
            .into_iter()
            .map(
                |((source_id, path), (direct_count, tree_count))| LibraryFolderFacet {
                    source_id,
                    path,
                    direct_count,
                    tree_count,
                },
            )
            .collect::<Vec<_>>();
        folders.sort_by(|left, right| {
            left.source_id
                .cmp(&right.source_id)
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(LibraryFacets {
            total_items: usize::try_from(total_items).unwrap_or(usize::MAX),
            available_items: usize::try_from(available_items).unwrap_or(usize::MAX),
            missing_items: usize::try_from(missing_items).unwrap_or(usize::MAX),
            containers,
            tags,
            creators,
            series,
            sites,
            themes,
            folders,
        })
    }

    pub fn list_library_items_by_ids(
        &self,
        ids: Vec<String>,
    ) -> Result<Vec<LibraryItem>, AppError> {
        if ids.len() > 500 {
            return Err(AppError::new(
                "library_item_batch_too_large",
                "Choose no more than 500 library items.",
            ));
        }
        let mut unique = Vec::with_capacity(ids.len());
        let mut seen = HashSet::new();
        for id in ids {
            validate_uuid(&id, "library item")?;
            if seen.insert(id.clone()) {
                unique.push(id);
            }
        }
        if unique.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders = (1..=unique.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>()
            .join(", ");
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             WHERE media_library.id IN ({placeholders}){privacy_filter}"
        ))?;
        let items = statement
            .query_map(params_from_iter(unique.iter()), library_item_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(items)
    }

    pub fn get_library_playback(
        &self,
        item_id: &str,
    ) -> Result<Option<LibraryPlaybackState>, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Ok(None);
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT item_id, position_seconds, duration_seconds, completed, updated_at
                 FROM library_playback WHERE item_id = ?1",
                [item_id],
                |row| library_playback_from_row(row, 0),
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn save_library_playback(
        &self,
        request: SaveLibraryPlaybackRequest,
    ) -> Result<LibraryPlaybackState, AppError> {
        validate_uuid(&request.item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(&request.item_id)? {
            return Err(library_locked_error());
        }
        if !request.position_seconds.is_finite()
            || !request.duration_seconds.is_finite()
            || request.position_seconds < 0.0
            || request.duration_seconds < 0.0
            || (!request.completed && request.duration_seconds == 0.0)
            || request.duration_seconds > 365.0 * 24.0 * 60.0 * 60.0
            || request.position_seconds > request.duration_seconds + 30.0
        {
            return Err(AppError::new(
                "invalid_library_playback",
                "The playback position or duration is invalid.",
            ));
        }
        let completed = request.completed
            || request.position_seconds >= request.duration_seconds - 10.0
            || request.position_seconds / request.duration_seconds >= 0.95;
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let media_kind: Option<String> = connection
            .query_row(
                "SELECT media_kind FROM media_library WHERE id = ?1",
                [&request.item_id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if media_kind.as_deref() != Some("video") {
            return Err(AppError::new(
                "library_video_not_found",
                "The selected library video was not found.",
            ));
        }
        connection.execute(
            "INSERT INTO library_playback
                (item_id, position_seconds, duration_seconds, completed, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(item_id) DO UPDATE SET
                position_seconds = excluded.position_seconds,
                duration_seconds = excluded.duration_seconds,
                 completed = CASE
                    WHEN library_playback.completed = 1 AND excluded.completed = 0 THEN 1
                    ELSE excluded.completed
                 END,
                updated_at = excluded.updated_at",
            params![
                request.item_id,
                request.position_seconds,
                request.duration_seconds,
                completed,
                now
            ],
        )?;
        connection
            .query_row(
                "SELECT item_id, position_seconds, duration_seconds, completed, updated_at
                 FROM library_playback WHERE item_id = ?1",
                [&request.item_id],
                |row| library_playback_from_row(row, 0),
            )
            .map_err(AppError::from)
    }

    pub fn get_library_video_display_settings(
        &self,
        item_id: &str,
    ) -> Result<Option<crate::models::LibraryVideoDisplaySettings>, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Ok(None);
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved video display settings are unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT fit, zoom, pan_x, pan_y, rotation, flip_horizontal, flip_vertical,
                        brightness, contrast, saturation, gamma
                 FROM library_video_display_settings WHERE item_id = ?1",
                [item_id],
                library_video_display_settings_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn save_library_video_display_settings(
        &self,
        request: crate::models::SaveLibraryVideoDisplaySettingsRequest,
    ) -> Result<crate::models::LibraryVideoDisplaySettings, AppError> {
        validate_uuid(&request.item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(&request.item_id)? {
            return Err(library_locked_error());
        }
        validate_library_video_display_settings(request.settings)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved video display settings are unavailable.",
            )
        })?;
        let media_kind = connection
            .query_row(
                "SELECT media_kind FROM media_library WHERE id = ?1",
                [&request.item_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        if media_kind.as_deref() != Some("video") {
            return Err(AppError::new(
                "library_video_not_found",
                "The selected library video was not found.",
            ));
        }
        let settings = request.settings;
        let fit = library_video_fit_value(settings.fit);
        connection.execute(
            "INSERT INTO library_video_display_settings
                (item_id, fit, zoom, pan_x, pan_y, rotation, flip_horizontal, flip_vertical,
                 brightness, contrast, saturation, gamma, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(item_id) DO UPDATE SET
                fit = excluded.fit,
                zoom = excluded.zoom,
                pan_x = excluded.pan_x,
                pan_y = excluded.pan_y,
                rotation = excluded.rotation,
                flip_horizontal = excluded.flip_horizontal,
                flip_vertical = excluded.flip_vertical,
                brightness = excluded.brightness,
                contrast = excluded.contrast,
                saturation = excluded.saturation,
                gamma = excluded.gamma,
                updated_at = excluded.updated_at",
            params![
                request.item_id,
                fit,
                settings.zoom,
                settings.pan_x,
                settings.pan_y,
                settings.rotation,
                settings.flip_horizontal,
                settings.flip_vertical,
                settings.brightness,
                settings.contrast,
                settings.saturation,
                settings.gamma,
                Utc::now().timestamp_millis(),
            ],
        )?;
        Ok(settings)
    }

    pub fn forget_library_video_display_settings(&self, item_id: &str) -> Result<(), AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(library_locked_error());
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved video display settings are unavailable.",
            )
        })?;
        connection.execute(
            "DELETE FROM library_video_display_settings WHERE item_id = ?1",
            [item_id],
        )?;
        Ok(())
    }

    pub fn list_continue_watching(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryPlaybackEntry>, AppError> {
        validate_library_shelf_limit(limit)?;
        let connection = self.library_reader()?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, media_library.duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating,
                    library_playback.item_id, library_playback.position_seconds,
                    library_playback.duration_seconds, library_playback.completed,
                    library_playback.updated_at
             FROM library_playback
             JOIN media_library ON media_library.id = library_playback.item_id
             LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
             WHERE media_library.missing = 0 AND media_library.media_kind = 'video'
               AND library_playback.completed = 0
               AND library_playback.position_seconds >= 5
               AND library_playback.duration_seconds - library_playback.position_seconds >= 10
               {privacy_filter}
             ORDER BY library_playback.updated_at DESC
             LIMIT ?1"
        ))?;
        let entries = statement
            .query_map([i64::try_from(limit).unwrap_or(24)], |row| {
                Ok(LibraryPlaybackEntry {
                    item: library_item_from_row(row)?,
                    playback: library_playback_from_row(row, 28)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn get_library_watch_state(&self, item_id: &str) -> Result<LibraryWatchState, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(AppError::new(
                "library_video_not_found",
                "The selected library video was not found.",
            ));
        }
        let playback = self.get_library_playback(item_id)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let media_kind = connection
            .query_row(
                "SELECT media_kind FROM media_library WHERE id = ?1",
                [item_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if media_kind.as_deref() != Some("video") {
            return Err(AppError::new(
                "library_video_not_found",
                "The selected library video was not found.",
            ));
        }
        let watch_later = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_watch_later WHERE item_id = ?1)",
            [item_id],
            |row| row.get(0),
        )?;
        Ok(LibraryWatchState {
            item_id: item_id.to_owned(),
            watch_later,
            playback,
        })
    }

    pub fn set_library_watch_later(
        &self,
        item_id: &str,
        watch_later: bool,
    ) -> Result<LibraryWatchState, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(library_locked_error());
        }
        {
            let connection = self.connection.lock().map_err(|_| {
                AppError::new("database_unavailable", "The media library is unavailable.")
            })?;
            let media_kind = connection
                .query_row(
                    "SELECT media_kind FROM media_library WHERE id = ?1",
                    [item_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if media_kind.as_deref() != Some("video") {
                return Err(AppError::new(
                    "library_video_not_found",
                    "The selected library video was not found.",
                ));
            }
            if watch_later {
                connection.execute(
                    "INSERT INTO library_watch_later (item_id, added_at) VALUES (?1, ?2)
                     ON CONFLICT(item_id) DO NOTHING",
                    params![item_id, Utc::now().timestamp_millis()],
                )?;
            } else {
                connection.execute(
                    "DELETE FROM library_watch_later WHERE item_id = ?1",
                    [item_id],
                )?;
            }
        }
        self.get_library_watch_state(item_id)
    }

    pub fn set_library_watched(
        &self,
        item_id: &str,
        watched: bool,
    ) -> Result<LibraryWatchState, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(library_locked_error());
        }
        if watched {
            let duration_seconds = {
                let connection = self.connection.lock().map_err(|_| {
                    AppError::new("database_unavailable", "The media library is unavailable.")
                })?;
                connection
                    .query_row(
                        "SELECT duration_seconds FROM media_library
                         WHERE id = ?1 AND media_kind = 'video'",
                        [item_id],
                        |row| row.get::<_, Option<f64>>(0),
                    )
                    .optional()?
                    .ok_or_else(|| {
                        AppError::new(
                            "library_video_not_found",
                            "The selected library video was not found.",
                        )
                    })?
                    .unwrap_or(0.0)
            };
            self.save_library_playback(SaveLibraryPlaybackRequest {
                item_id: item_id.to_owned(),
                position_seconds: duration_seconds,
                duration_seconds,
                completed: true,
            })?;
        } else {
            let connection = self.connection.lock().map_err(|_| {
                AppError::new("database_unavailable", "The media library is unavailable.")
            })?;
            let media_kind = connection
                .query_row(
                    "SELECT media_kind FROM media_library WHERE id = ?1",
                    [item_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if media_kind.as_deref() != Some("video") {
                return Err(AppError::new(
                    "library_video_not_found",
                    "The selected library video was not found.",
                ));
            }
            connection.execute("DELETE FROM library_playback WHERE item_id = ?1", [item_id])?;
        }
        self.get_library_watch_state(item_id)
    }

    pub fn list_library_watch_later(&self, limit: usize) -> Result<Vec<LibraryItem>, AppError> {
        validate_library_shelf_limit(limit)?;
        let connection = self.library_reader()?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail,
                    media_library.duration_seconds, site, tags_json, metadata_json, missing,
                    media_library.added_at, media_library.updated_at, source_id, relative_path, media_kind,
                    file_size_bytes, modified_at_ms, width, height, fps, video_codec, audio_codec,
                    container, dynamic_range, COALESCE(state, 'pending'), error, favorite, rating
             FROM library_watch_later
             JOIN media_library ON media_library.id = library_watch_later.item_id
             LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
             WHERE media_library.missing = 0 AND media_library.media_kind = 'video'
               {privacy_filter}
             ORDER BY library_watch_later.added_at DESC, media_library.id ASC
             LIMIT ?1"
        ))?;
        let items = statement
            .query_map([i64::try_from(limit).unwrap_or(24)], library_item_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(items)
    }

    pub fn list_recently_watched(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryPlaybackEntry>, AppError> {
        validate_library_shelf_limit(limit)?;
        let connection = self.library_reader()?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT media_library.id, path, title, source_url, thumbnail,
                    media_library.duration_seconds, site, tags_json, metadata_json, missing,
                    added_at, media_library.updated_at, source_id, relative_path, media_kind,
                    file_size_bytes, modified_at_ms, width, height, fps, video_codec, audio_codec,
                    container, dynamic_range, COALESCE(state, 'pending'), error, favorite, rating,
                    library_playback.item_id, library_playback.position_seconds,
                    library_playback.duration_seconds, library_playback.completed,
                    library_playback.updated_at
             FROM library_playback
             JOIN media_library ON media_library.id = library_playback.item_id
             LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
             WHERE media_library.missing = 0 AND media_library.media_kind = 'video'
               AND (library_playback.completed = 1 OR library_playback.position_seconds >= 5)
               {privacy_filter}
             ORDER BY library_playback.updated_at DESC, media_library.id ASC
             LIMIT ?1"
        ))?;
        let entries = statement
            .query_map([i64::try_from(limit).unwrap_or(24)], |row| {
                Ok(LibraryPlaybackEntry {
                    item: library_item_from_row(row)?,
                    playback: library_playback_from_row(row, 28)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(entries)
    }

    pub fn get_library_item(&self, id: &str) -> Result<LibraryItem, AppError> {
        if self.is_library_privacy_locked() && self.is_library_item_private(id)? {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                        tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                        relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                        video_codec, audio_codec, container, dynamic_range,
                        COALESCE(state, 'pending'), error, favorite, rating
                 FROM media_library
                 LEFT JOIN library_media_metadata ON item_id = media_library.id
                 WHERE media_library.id = ?1",
                [id],
                library_item_from_row,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new("library_item_not_found", "The library item was not found.")
            })
    }

    pub fn get_library_item_by_path(&self, path: &str) -> Result<Option<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                        tags_json, metadata_json, missing, added_at, media_library.updated_at,
                        source_id, relative_path, media_kind, file_size_bytes, modified_at_ms,
                        width, height, fps, video_codec, audio_codec, container, dynamic_range,
                        COALESCE(state, 'pending'), error, favorite, rating
                 FROM media_library
                 LEFT JOIN library_media_metadata ON item_id = media_library.id
                 WHERE path = ?1",
                [path],
                library_item_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn find_library_items_by_identity(
        &self,
        source_id: &str,
        identity: LibraryFileIdentity,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let Some((device_id, file_id)) = sqlite_file_identity(identity) else {
            return Ok(vec![]);
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at,
                    source_id, relative_path, media_kind, file_size_bytes, modified_at_ms,
                    width, height, fps, video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             JOIN library_file_identity ON library_file_identity.item_id = media_library.id
             LEFT JOIN library_media_metadata ON library_media_metadata.item_id = media_library.id
             WHERE source_id = ?1 AND device_id = ?2 AND file_id = ?3",
        )?;
        let rows = statement.query_map(
            params![source_id, device_id, file_id],
            library_item_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn move_library_item_record(
        &self,
        id: &str,
        item: &LibraryItem,
    ) -> Result<LibraryItem, AppError> {
        validate_uuid(id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute(
            "UPDATE media_library SET path = ?2, relative_path = ?3, media_kind = ?4,
                    missing = 0, updated_at = ?5 WHERE id = ?1",
            params![
                id,
                item.path,
                item.relative_path,
                item.media_kind.map(library_media_kind_name),
                Utc::now().timestamp_millis()
            ],
        )? == 0
        {
            return Err(library_item_not_found());
        }
        drop(connection);
        let mut moved = item.clone();
        moved.id = id.to_owned();
        self.upsert_scanned_library_item(&moved)
    }

    pub fn save_library_file_identity(
        &self,
        item_id: &str,
        identity: LibraryFileIdentity,
    ) -> Result<(), AppError> {
        let Some((device_id, file_id)) = sqlite_file_identity(identity) else {
            return Ok(());
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection.execute(
            "INSERT INTO library_file_identity (item_id, device_id, file_id, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(item_id) DO UPDATE SET device_id = excluded.device_id,
                file_id = excluded.file_id, updated_at = excluded.updated_at",
            params![item_id, device_id, file_id, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn update_library_item(
        &self,
        mut request: UpdateLibraryItemRequest,
    ) -> Result<LibraryItem, AppError> {
        request.title = validate_library_title(&request.title)?;
        request.tags = normalize_library_tags(request.tags)?;
        validate_library_rating(request.rating)?;
        let tags = serde_json::to_string(&request.tags)
            .map_err(|error| AppError::internal("could not serialize media tags", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute(
            "UPDATE media_library SET title = ?2, tags_json = ?3, favorite = ?4,
                rating = ?5, updated_at = ?6 WHERE id = ?1",
            params![
                request.id,
                request.title,
                tags,
                request.favorite,
                request.rating,
                Utc::now().timestamp_millis()
            ],
        )? == 0
        {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        drop(connection);
        self.get_library_item(&request.id)
    }

    pub fn get_library_rich_metadata(
        &self,
        item_id: &str,
    ) -> Result<LibraryRichMetadata, AppError> {
        validate_uuid(item_id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let record = connection
            .query_row(
                "SELECT extracted_json, overrides_json, updated_at
                 FROM library_rich_metadata WHERE item_id = ?1",
                [item_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((extracted_json, overrides_json, updated_at)) = record {
            return rich_metadata_from_json(item_id, &extracted_json, &overrides_json, updated_at);
        }
        let metadata_json: Option<String> = connection
            .query_row(
                "SELECT metadata_json FROM media_library WHERE id = ?1",
                [item_id],
                |row| row.get(0),
            )
            .optional()?;
        let metadata_json = metadata_json.ok_or_else(|| {
            AppError::new("library_item_not_found", "The library item was not found.")
        })?;
        let metadata = serde_json::from_str(&metadata_json).unwrap_or_default();
        let extracted = metadata_fields_from_value(&metadata);
        rich_metadata_from_fields(
            item_id,
            extracted,
            LibraryMetadataFields::default(),
            Utc::now().timestamp_millis(),
        )
    }

    pub fn list_library_rich_metadata_for_backup(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryRichMetadata>, AppError> {
        if !(1..=100_001).contains(&limit) {
            return Err(AppError::new(
                "invalid_library_backup_limit",
                "The rich metadata backup limit is invalid.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT item_id, extracted_json, overrides_json, updated_at
             FROM library_rich_metadata ORDER BY item_id LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (item_id, extracted, overrides, updated_at) = row?;
            rich_metadata_from_json(&item_id, &extracted, &overrides, updated_at)
        })
        .collect()
    }

    pub fn save_library_rich_metadata(
        &self,
        request: SaveLibraryRichMetadataRequest,
    ) -> Result<LibraryRichMetadata, AppError> {
        validate_uuid(&request.item_id, "library item")?;
        let mut overrides = validate_library_metadata_fields(LibraryMetadataFields {
            creator: request.creator,
            description: request.description,
            upload_date: request.upload_date,
            series: request.series,
            episode: request.episode,
            original_url: request.original_url,
        })?;
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if !connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1)",
            [&request.item_id],
            |row| row.get(0),
        )? {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        let extracted_json: Option<String> = connection
            .query_row(
                "SELECT extracted_json FROM library_rich_metadata WHERE item_id = ?1",
                [&request.item_id],
                |row| row.get(0),
            )
            .optional()?;
        let extracted = extracted_json
            .as_deref()
            .and_then(|value| serde_json::from_str::<LibraryMetadataFields>(value).ok())
            .unwrap_or_default();
        clear_matching_metadata_overrides(&mut overrides, &extracted);
        let overrides_json = serde_json::to_string(&overrides)
            .map_err(|error| AppError::internal("could not serialize metadata overrides", error))?;
        connection.execute(
            "INSERT INTO library_rich_metadata (item_id, extracted_json, overrides_json, updated_at)
             VALUES (?1, '{}', ?2, ?3)
             ON CONFLICT(item_id) DO UPDATE SET
                overrides_json = excluded.overrides_json,
                updated_at = excluded.updated_at",
            params![request.item_id, overrides_json, now],
        )?;
        drop(connection);
        self.get_library_rich_metadata(&request.item_id)
    }

    pub fn apply_library_bulk_metadata(
        &self,
        request: ApplyLibraryBulkMetadataRequest,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let ids = validate_library_item_ids(request.item_ids)?;
        let fields = validate_library_metadata_fields(LibraryMetadataFields {
            creator: request.creator,
            series: request.series,
            episode: request.episode,
            ..LibraryMetadataFields::default()
        })?;
        let tags = normalize_library_tags(request.tags)?;
        if fields.creator.is_none()
            && fields.series.is_none()
            && fields.episode.is_none()
            && tags.is_empty()
        {
            return Err(AppError::new(
                "empty_library_bulk_metadata",
                "Enter at least one creator, series, episode, or tag to apply.",
            ));
        }
        for id in &ids {
            self.get_library_item(id)?;
        }
        if fields.creator.is_some() || fields.series.is_some() || fields.episode.is_some() {
            for id in &ids {
                let current = self.get_library_rich_metadata(id)?;
                self.save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                    item_id: id.clone(),
                    creator: fields.creator.clone().or(current.overrides.creator),
                    description: current.overrides.description,
                    upload_date: current.overrides.upload_date,
                    series: fields.series.clone().or(current.overrides.series),
                    episode: fields.episode.clone().or(current.overrides.episode),
                    original_url: current.overrides.original_url,
                })?;
            }
        }
        if !tags.is_empty() {
            self.apply_library_bulk_action(ids.clone(), LibraryBulkAction::AddTags { tags })?;
        }
        self.list_library_items_by_ids(ids)
    }

    pub fn reset_library_rich_metadata(
        &self,
        item_id: &str,
    ) -> Result<LibraryRichMetadata, AppError> {
        validate_uuid(item_id, "library item")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute(
            "UPDATE library_rich_metadata SET overrides_json = '{}', updated_at = ?2
             WHERE item_id = ?1",
            params![item_id, Utc::now().timestamp_millis()],
        )? == 0
        {
            drop(connection);
            return self.get_library_rich_metadata(item_id);
        }
        drop(connection);
        self.get_library_rich_metadata(item_id)
    }

    pub fn set_library_artwork(
        &self,
        item_id: &str,
        path: Option<String>,
    ) -> Result<LibraryItem, AppError> {
        validate_uuid(item_id, "library item")?;
        let path = path
            .map(|value| validate_library_artwork_path(&value))
            .transpose()?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute(
            "UPDATE media_library SET thumbnail = ?2, updated_at = ?3 WHERE id = ?1",
            params![item_id, path, Utc::now().timestamp_millis()],
        )? == 0
        {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        drop(connection);
        self.get_library_item(item_id)
    }

    pub fn library_search_status(&self) -> Result<LibrarySearchStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let catalog_items: i64 =
            connection.query_row("SELECT COUNT(*) FROM media_library", [], |row| row.get(0))?;
        let (indexed_rows, indexed_items): (i64, i64) = connection.query_row(
            "SELECT COUNT(*), COUNT(DISTINCT item_id) FROM library_search",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let catalog_items = usize::try_from(catalog_items).unwrap_or(usize::MAX);
        let indexed_items = usize::try_from(indexed_items).unwrap_or(usize::MAX);
        let same_item_ids: bool = connection.query_row(
            "SELECT NOT EXISTS(
                SELECT 1 FROM (
                    SELECT id AS item_id FROM media_library
                    EXCEPT SELECT item_id FROM library_search
                )
                UNION ALL
                SELECT 1 FROM (
                    SELECT item_id FROM library_search
                    EXCEPT SELECT id FROM media_library
                )
             )",
            [],
            |row| row.get(0),
        )?;
        Ok(LibrarySearchStatus {
            catalog_items,
            indexed_items,
            consistent: catalog_items == indexed_items
                && indexed_rows == i64::try_from(indexed_items).unwrap_or(i64::MAX)
                && same_item_ids,
        })
    }

    pub fn library_health_summary(&self) -> Result<LibraryHealthSummary, AppError> {
        const COMPATIBILITY_RISK: &str = "(
            media.media_kind = 'video' AND metadata.state = 'ready' AND (
                LOWER(COALESCE(metadata.video_codec, '')) IN ('av1', 'hevc', 'h265', 'mpeg4')
                OR LOWER(COALESCE(metadata.container, '')) IN ('avi', 'flv', 'mpeg', 'mpg', 'ts', 'm2ts', 'mts', '3gp', '3g2', 'ogg', 'ogv')
            )
        )";
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let aggregate_sql = format!(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN media.missing = 0 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN media.missing = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN EXISTS(
                    SELECT 1 FROM library_changes change
                    WHERE change.item_id = media.id AND change.reviewed_at IS NULL
                ) THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN media.missing = 0 AND (metadata.state IS NULL OR metadata.state = 'pending') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN media.missing = 0 AND metadata.state IN ('failed', 'unavailable') THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN {COMPATIBILITY_RISK} THEN 1 ELSE 0 END), 0)
             FROM media_library media
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE 1 = 1{privacy_filter}"
        );
        let counts: (i64, i64, i64, i64, i64, i64, i64) =
            connection.query_row(&aggregate_sql, [], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            })?;
        let issues_sql = format!(
            "SELECT
                media.id,
                media.title,
                media.media_kind,
                media.missing,
                COALESCE(metadata.state, 'pending'),
                EXISTS(
                    SELECT 1 FROM library_changes change
                    WHERE change.item_id = media.id AND change.reviewed_at IS NULL
                ),
                CASE WHEN {COMPATIBILITY_RISK} THEN 1 ELSE 0 END
             FROM media_library media
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE (media.missing = 1
                OR metadata.state IS NULL
                OR metadata.state IN ('pending', 'failed', 'unavailable')
                OR EXISTS(
                    SELECT 1 FROM library_changes change
                    WHERE change.item_id = media.id AND change.reviewed_at IS NULL
                )
                OR {COMPATIBILITY_RISK})
               {privacy_filter}
             ORDER BY
                media.missing DESC,
                CASE COALESCE(metadata.state, 'pending') WHEN 'failed' THEN 0 WHEN 'unavailable' THEN 0 WHEN 'pending' THEN 1 ELSE 2 END,
                media.updated_at DESC,
                media.id
             LIMIT 100"
        );
        let mut statement = connection.prepare(&issues_sql)?;
        let issues = statement
            .query_map([], |row| {
                let missing = row.get::<_, i64>(3)? != 0;
                let metadata_state = row.get::<_, String>(4)?;
                let changed = row.get::<_, i64>(5)? != 0;
                let compatibility_risk = row.get::<_, i64>(6)? != 0;
                let mut kinds = Vec::with_capacity(3);
                if missing {
                    kinds.push(LibraryHealthIssueKind::Missing);
                }
                if changed {
                    kinds.push(LibraryHealthIssueKind::Changed);
                }
                if !missing && metadata_state == "pending" {
                    kinds.push(LibraryHealthIssueKind::MetadataPending);
                }
                if !missing && matches!(metadata_state.as_str(), "failed" | "unavailable") {
                    kinds.push(LibraryHealthIssueKind::MetadataFailed);
                }
                if compatibility_risk {
                    kinds.push(LibraryHealthIssueKind::CompatibilityRisk);
                }
                Ok(LibraryHealthIssue {
                    item_id: row.get(0)?,
                    title: row.get(1)?,
                    media_kind: row
                        .get::<_, Option<String>>(2)?
                        .as_deref()
                        .and_then(library_media_kind_from_name),
                    kinds,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let count = |value: i64| usize::try_from(value).unwrap_or(usize::MAX);
        Ok(LibraryHealthSummary {
            total_items: count(counts.0),
            available_items: count(counts.1),
            missing_items: count(counts.2),
            changed_items: count(counts.3),
            metadata_pending_items: count(counts.4),
            metadata_failed_items: count(counts.5),
            compatibility_risk_items: count(counts.6),
            issues,
        })
    }

    pub fn library_storage_plan_base(&self) -> Result<LibraryStoragePlan, AppError> {
        let visible_sources = self.list_library_sources()?;
        let source_names = visible_sources
            .iter()
            .map(|source| (source.id.clone(), source.name.clone()))
            .collect::<HashMap<_, _>>();
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The storage planner is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT media.source_id, media.relative_path,
                    metadata.file_size_bytes, metadata.container
             FROM media_library media
             LEFT JOIN library_media_metadata metadata ON metadata.item_id = media.id
             WHERE media.missing = 0
               {privacy_filter}
             ORDER BY media.source_id, media.relative_path, media.id
             LIMIT 250000"
        ))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);

        let mut source_totals = HashMap::<String, (u64, usize)>::new();
        let mut folder_totals = HashMap::<(String, String), (u64, usize)>::new();
        let mut container_totals = HashMap::<String, (u64, usize)>::new();
        let mut catalog_bytes = 0_u64;
        for (source_id, relative_path, size, container) in rows {
            let size = u64::try_from(size.unwrap_or_default().max(0)).unwrap_or_default();
            catalog_bytes = catalog_bytes.saturating_add(size);
            let source_key = source_id.unwrap_or_default();
            let source_total = source_totals.entry(source_key.clone()).or_default();
            source_total.0 = source_total.0.saturating_add(size);
            source_total.1 += 1;

            let normalized = relative_path.unwrap_or_default().replace('\\', "/");
            let folder = normalized
                .rsplit_once('/')
                .map(|(parent, _)| parent)
                .filter(|parent| !parent.is_empty())
                .unwrap_or("Root")
                .to_owned();
            let folder_total = folder_totals
                .entry((source_key.clone(), folder))
                .or_default();
            folder_total.0 = folder_total.0.saturating_add(size);
            folder_total.1 += 1;

            let container = container
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "Unknown".to_owned())
                .to_ascii_lowercase();
            let container_total = container_totals.entry(container).or_default();
            container_total.0 = container_total.0.saturating_add(size);
            container_total.1 += 1;
        }

        let mut sources = visible_sources
            .into_iter()
            .map(|source| {
                let (catalog_bytes, item_count) =
                    source_totals.remove(&source.id).unwrap_or_default();
                LibraryStorageSource {
                    source_id: source.id,
                    name: source.name,
                    path: source.path,
                    catalog_bytes,
                    item_count,
                    available_bytes: None,
                }
            })
            .collect::<Vec<_>>();
        if let Some((bytes, item_count)) = source_totals.remove("") {
            sources.push(LibraryStorageSource {
                source_id: String::new(),
                name: "Downloads and unassigned".to_owned(),
                path: String::new(),
                catalog_bytes: bytes,
                item_count,
                available_bytes: None,
            });
        }
        for source in &mut sources {
            if !source.path.is_empty() {
                source.available_bytes = fs2::available_space(&source.path).ok();
            }
        }
        sources.sort_by(|left, right| {
            right.catalog_bytes.cmp(&left.catalog_bytes).then_with(|| {
                left.name
                    .to_ascii_lowercase()
                    .cmp(&right.name.to_ascii_lowercase())
            })
        });

        let mut folders = folder_totals
            .into_iter()
            .map(|((source_id, folder), (bytes, item_count))| {
                let source_name = source_names
                    .get(&source_id)
                    .cloned()
                    .unwrap_or_else(|| "Downloads and unassigned".to_owned());
                LibraryStorageBucket {
                    key: format!("{source_id}:{folder}"),
                    label: format!("{source_name} / {folder}"),
                    bytes,
                    item_count,
                }
            })
            .collect::<Vec<_>>();
        folders.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.label.cmp(&right.label))
        });
        folders.truncate(40);
        let mut containers = container_totals
            .into_iter()
            .map(|(container, (bytes, item_count))| LibraryStorageBucket {
                key: container.clone(),
                label: container.to_ascii_uppercase(),
                bytes,
                item_count,
            })
            .collect::<Vec<_>>();
        containers.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.key.cmp(&right.key))
        });

        let mut hash_groups = HashMap::<(u64, String), usize>::new();
        for (_, size, hash) in self.list_current_library_content_hashes()? {
            *hash_groups.entry((size, hash)).or_default() += 1;
        }
        let exact_duplicate_reclaimable_bytes = hash_groups
            .into_iter()
            .filter(|(_, count)| *count > 1)
            .fold(0_u64, |total, ((size, _), count)| {
                total.saturating_add(
                    size.saturating_mul(u64::try_from(count - 1).unwrap_or(u64::MAX)),
                )
            });

        Ok(LibraryStoragePlan {
            catalog_bytes,
            sources,
            folders,
            containers,
            exact_duplicate_reclaimable_bytes,
            playback_cache_bytes: 0,
            artwork_cache_bytes: 0,
            incoming_known_bytes: 0,
            incoming_unknown_items: 0,
            active_transformation_count: 0,
            warnings: Vec::new(),
        })
    }

    pub fn rebuild_library_search_index(&self) -> Result<LibrarySearchStatus, AppError> {
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM library_search", [])?;
        transaction.execute(
            "INSERT INTO library_search (item_id, title, creator, series, description, site, tags, path)
             SELECT
                media_library.id,
                COALESCE(media_library.title, ''),
                COALESCE(NULLIF(json_extract(rich.overrides_json, '$.creator'), ''), json_extract(rich.extracted_json, '$.creator'), ''),
                COALESCE(NULLIF(json_extract(rich.overrides_json, '$.series'), ''), json_extract(rich.extracted_json, '$.series'), ''),
                COALESCE(NULLIF(json_extract(rich.overrides_json, '$.description'), ''), json_extract(rich.extracted_json, '$.description'), ''),
                COALESCE(media_library.site, ''),
                COALESCE((SELECT group_concat(value, ' ') FROM json_each(media_library.tags_json)), ''),
                COALESCE(media_library.relative_path, media_library.path, '')
             FROM media_library
             LEFT JOIN library_rich_metadata rich ON rich.item_id = media_library.id",
            [],
        )?;
        transaction.commit()?;
        drop(connection);
        self.library_search_status()
    }

    pub fn replace_library_transcript_cues(
        &self,
        item_id: &str,
        cues: &[LibraryTranscriptCue],
    ) -> Result<(), AppError> {
        validate_uuid(item_id, "library item")?;
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The transcript search index is unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM library_transcript_search WHERE item_id = ?1",
            [item_id],
        )?;
        transaction.execute(
            "DELETE FROM library_transcript_embeddings WHERE item_id = ?1",
            [item_id],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO library_transcript_search
                    (item_id, subtitle_name, language, start_seconds, end_seconds, cue_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for cue in cues.iter().take(50_000) {
                if !cue.start_seconds.is_finite()
                    || !cue.end_seconds.is_finite()
                    || cue.start_seconds < 0.0
                    || cue.end_seconds <= cue.start_seconds
                    || cue.text.is_empty()
                {
                    continue;
                }
                statement.execute(params![
                    item_id,
                    cue.subtitle_name,
                    cue.language,
                    cue.start_seconds,
                    cue.end_seconds,
                    cue.text,
                ])?;
            }
        }
        transaction.execute(
            "INSERT INTO library_transcript_search
                (item_id, subtitle_name, language, start_seconds, end_seconds, cue_text)
             SELECT item_id, subtitle_name, language, start_seconds, end_seconds, cue_text
             FROM library_local_transcript_cues
             WHERE item_id = ?1
             ORDER BY start_seconds, id",
            [item_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn replace_library_local_transcript_cues(
        &self,
        item_id: &str,
        cues: &[LibraryTranscriptCue],
    ) -> Result<(), AppError> {
        validate_uuid(item_id, "library item identifier")?;
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The local transcript store is unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM library_transcript_search
             WHERE item_id = ?1
               AND subtitle_name IN (
                   SELECT DISTINCT subtitle_name FROM library_local_transcript_cues WHERE item_id = ?1
               )",
            [item_id],
        )?;
        transaction.execute(
            "DELETE FROM library_local_transcript_cues WHERE item_id = ?1",
            [item_id],
        )?;
        {
            let mut local_statement = transaction.prepare(
                "INSERT INTO library_local_transcript_cues
                    (item_id, subtitle_name, language, start_seconds, end_seconds, cue_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            let mut search_statement = transaction.prepare(
                "INSERT INTO library_transcript_search
                    (item_id, subtitle_name, language, start_seconds, end_seconds, cue_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for cue in cues.iter().take(50_000) {
                if !cue.start_seconds.is_finite()
                    || !cue.end_seconds.is_finite()
                    || cue.start_seconds < 0.0
                    || cue.end_seconds <= cue.start_seconds
                    || cue.text.trim().is_empty()
                {
                    continue;
                }
                let values = params![
                    item_id,
                    cue.subtitle_name,
                    cue.language,
                    cue.start_seconds,
                    cue.end_seconds,
                    cue.text,
                ];
                local_statement.execute(values)?;
                search_statement.execute(params![
                    item_id,
                    cue.subtitle_name,
                    cue.language,
                    cue.start_seconds,
                    cue.end_seconds,
                    cue.text,
                ])?;
            }
        }
        transaction.execute(
            "DELETE FROM library_transcript_embeddings WHERE item_id = ?1",
            [item_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn search_library_transcripts(
        &self,
        search: &str,
        limit: usize,
    ) -> Result<Vec<LibraryTranscriptMatch>, AppError> {
        let search = search.trim();
        if search.chars().count() > 200 || search.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_transcript_search",
                "Transcript search text must be 200 characters or fewer.",
            ));
        }
        if !(1..=50).contains(&limit) {
            return Err(AppError::new(
                "invalid_transcript_search_limit",
                "Transcript search returns between 1 and 50 matches.",
            ));
        }
        let query = library_fts_query(search)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The transcript search index is unavailable.",
            )
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let mut statement = connection.prepare(&format!(
            "SELECT transcript.item_id, transcript.start_seconds, transcript.end_seconds,
                    transcript.cue_text, NULLIF(transcript.language, '')
             FROM library_transcript_search transcript
             JOIN media_library ON media_library.id = transcript.item_id
             WHERE library_transcript_search MATCH ?1
               AND media_library.missing = 0
               AND media_library.media_kind IN ('video', 'both')
               {privacy_filter}
             ORDER BY bm25(library_transcript_search), transcript.start_seconds, transcript.rowid
             LIMIT ?2"
        ))?;
        let matches = statement
            .query_map(params![query, i64::try_from(limit).unwrap_or(50)], |row| {
                Ok(LibraryTranscriptMatch {
                    item_id: row.get(0)?,
                    start_seconds: row.get(1)?,
                    end_seconds: row.get(2)?,
                    text: row.get(3)?,
                    language: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(matches)
    }

    pub fn library_transcript_index_status(
        &self,
    ) -> Result<LibraryTranscriptIndexStatus, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The transcript search index is unavailable.",
            )
        })?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let (cue_count, indexed_items): (i64, i64) = connection.query_row(
            &format!(
                "SELECT COUNT(*), COUNT(DISTINCT transcript.item_id)
             FROM library_transcript_search transcript
             JOIN media_library ON media_library.id = transcript.item_id
             WHERE media_library.missing = 0
               AND media_library.media_kind IN ('video', 'both'){privacy_filter}"
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        Ok(LibraryTranscriptIndexStatus {
            indexed_items: usize::try_from(indexed_items).unwrap_or(usize::MAX),
            cue_count: usize::try_from(cue_count).unwrap_or(usize::MAX),
        })
    }

    pub fn list_semantic_transcript_documents(
        &self,
        item_ids: &[String],
        limit: usize,
    ) -> Result<Vec<SemanticTranscriptDocument>, AppError> {
        if !(1..=500).contains(&limit) || item_ids.len() > 500 {
            return Err(AppError::new(
                "invalid_semantic_transcript_batch",
                "Semantic transcript batches contain between 1 and 500 videos.",
            ));
        }
        for item_id in item_ids {
            validate_uuid(item_id, "library item identifier")?;
        }
        let item_ids_json = serde_json::to_string(item_ids)
            .map_err(|error| AppError::internal("could not serialize transcript targets", error))?;
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The semantic transcript index is unavailable.",
            )
        })?;
        let mut item_statement = connection.prepare(&format!(
            "SELECT DISTINCT media.id, media.title
             FROM media_library media
             JOIN library_transcript_search transcript ON transcript.item_id = media.id
             WHERE media.missing = 0
               AND media.media_kind IN ('video', 'both')
               AND (?1 = 0 OR media.id IN (SELECT value FROM json_each(?2)))
               {privacy_filter}
             ORDER BY media.id
             LIMIT ?3"
        ))?;
        let item_rows = item_statement
            .query_map(
                params![
                    i64::from(!item_ids.is_empty()),
                    item_ids_json,
                    i64::try_from(limit).unwrap_or(500)
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        drop(item_statement);

        let mut cue_statement = connection.prepare(
            "SELECT start_seconds, end_seconds, cue_text
             FROM library_transcript_search
             WHERE item_id = ?1
             ORDER BY start_seconds, rowid
             LIMIT 50000",
        )?;
        let mut documents = Vec::with_capacity(item_rows.len());
        for (item_id, title) in item_rows {
            let cues = cue_statement
                .query_map([&item_id], |row| {
                    Ok((
                        row.get::<_, f64>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let mut text = String::new();
            let mut hasher = Sha256::new();
            for (start, end, cue_text) in cues {
                hasher.update(start.to_le_bytes());
                hasher.update(end.to_le_bytes());
                hasher.update(cue_text.as_bytes());
                if text.chars().count() < 120_000 {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.extend(cue_text.chars().take(2_000));
                }
            }
            if !text.trim().is_empty() {
                documents.push(SemanticTranscriptDocument {
                    item_id,
                    title,
                    text,
                    fingerprint: format!("{:x}", hasher.finalize()),
                });
            }
        }
        Ok(documents)
    }

    pub fn save_semantic_transcript_embedding(
        &self,
        document: &SemanticTranscriptDocument,
        model: &str,
        vector_json: &str,
    ) -> Result<(), AppError> {
        validate_uuid(&document.item_id, "library item identifier")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The semantic transcript index is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO library_transcript_embeddings
                (item_id, model, vector_json, transcript_fingerprint, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(item_id) DO UPDATE SET
                model = excluded.model,
                vector_json = excluded.vector_json,
                transcript_fingerprint = excluded.transcript_fingerprint,
                updated_at = excluded.updated_at",
            params![
                document.item_id,
                model,
                vector_json,
                document.fingerprint,
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn semantic_transcript_embedding_is_current(
        &self,
        document: &SemanticTranscriptDocument,
        model: &str,
    ) -> Result<bool, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The semantic transcript index is unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM library_transcript_embeddings
                    WHERE item_id = ?1 AND model = ?2 AND transcript_fingerprint = ?3
                )",
                params![document.item_id, model, document.fingerprint],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn list_semantic_transcript_embeddings(
        &self,
        model: &str,
        limit: usize,
    ) -> Result<Vec<StoredTranscriptEmbedding>, AppError> {
        if !(1..=50_000).contains(&limit) {
            return Err(AppError::new(
                "invalid_semantic_transcript_limit",
                "Semantic search examines between 1 and 50,000 indexed videos.",
            ));
        }
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The semantic transcript index is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT embedding.item_id, embedding.vector_json
             FROM library_transcript_embeddings embedding
             JOIN media_library media ON media.id = embedding.item_id
             WHERE embedding.model = ?1
               AND media.missing = 0
               AND media.media_kind IN ('video', 'both')
               {privacy_filter}
             ORDER BY embedding.updated_at DESC
             LIMIT ?2"
        ))?;
        let embeddings = statement
            .query_map(
                params![model, i64::try_from(limit).unwrap_or(50_000)],
                |row| {
                    Ok(StoredTranscriptEmbedding {
                        item_id: row.get(0)?,
                        vector_json: row.get(1)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(embeddings)
    }

    pub fn semantic_transcript_index_status(
        &self,
        model: &str,
    ) -> Result<LibrarySemanticIndexStatus, AppError> {
        let privacy_filter = if self.is_library_privacy_locked() {
            locked_library_filter_for("media")
        } else {
            String::new()
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The semantic transcript index is unavailable.",
            )
        })?;
        let eligible: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(DISTINCT media.id)
                 FROM media_library media
                 JOIN library_transcript_search transcript ON transcript.item_id = media.id
                 WHERE media.missing = 0
                   AND media.media_kind IN ('video', 'both')
                   {privacy_filter}"
            ),
            [],
            |row| row.get(0),
        )?;
        let indexed: i64 = connection.query_row(
            &format!(
                "SELECT COUNT(*)
                 FROM library_transcript_embeddings embedding
                 JOIN media_library media ON media.id = embedding.item_id
                 WHERE embedding.model = ?1
                   AND media.missing = 0
                   AND media.media_kind IN ('video', 'both')
                   {privacy_filter}"
            ),
            [model],
            |row| row.get(0),
        )?;
        let eligible = usize::try_from(eligible).unwrap_or(usize::MAX);
        let indexed = usize::try_from(indexed).unwrap_or(usize::MAX);
        Ok(LibrarySemanticIndexStatus {
            model: model.to_owned(),
            indexed_items: indexed,
            stale_items: eligible.saturating_sub(indexed),
        })
    }

    pub fn library_transcript_excerpt(
        &self,
        item_id: &str,
        query: &str,
    ) -> Result<Option<TranscriptExcerpt>, AppError> {
        validate_uuid(item_id, "library item identifier")?;
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The transcript search index is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT transcript.start_seconds, transcript.end_seconds,
                    transcript.cue_text, NULLIF(transcript.language, '')
             FROM library_transcript_search transcript
             JOIN media_library ON media_library.id = transcript.item_id
             WHERE transcript.item_id = ?1
               AND media_library.missing = 0
               {privacy_filter}
             ORDER BY transcript.start_seconds, transcript.rowid
             LIMIT 500"
        ))?;
        let rows = statement
            .query_map([item_id], |row| {
                Ok(TranscriptExcerpt {
                    start_seconds: row.get(0)?,
                    end_seconds: row.get(1)?,
                    text: row.get(2)?,
                    language: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let terms = query
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .filter(|term| term.chars().count() >= 2)
            .take(12)
            .collect::<Vec<_>>();
        Ok(rows.into_iter().max_by_key(|cue| {
            let text = cue.text.to_ascii_lowercase();
            terms
                .iter()
                .filter(|term| text.contains(term.as_str()))
                .count()
        }))
    }

    pub fn library_transcript_cues_for_suggestions(
        &self,
        item_id: &str,
        limit: usize,
    ) -> Result<Vec<LibraryTranscriptCue>, AppError> {
        validate_uuid(item_id, "library item identifier")?;
        if !(1..=5_000).contains(&limit) {
            return Err(AppError::new(
                "invalid_chapter_transcript_limit",
                "Chapter analysis reads between 1 and 5,000 transcript cues.",
            ));
        }
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Transcript chapter analysis is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT transcript.subtitle_name, NULLIF(transcript.language, ''),
                    transcript.start_seconds, transcript.end_seconds, transcript.cue_text
             FROM library_transcript_search transcript
             JOIN media_library ON media_library.id = transcript.item_id
             WHERE transcript.item_id = ?1
               AND media_library.missing = 0
               AND media_library.media_kind IN ('video', 'both')
               {privacy_filter}
             ORDER BY transcript.start_seconds, transcript.rowid
             LIMIT ?2"
        ))?;
        let cues = statement
            .query_map(
                params![item_id, i64::try_from(limit).unwrap_or(5_000)],
                |row| {
                    Ok(LibraryTranscriptCue {
                        subtitle_name: row.get(0)?,
                        language: row.get(1)?,
                        start_seconds: row.get(2)?,
                        end_seconds: row.get(3)?,
                        text: row.get(4)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(cues)
    }

    pub fn list_library_smart_views(&self) -> Result<Vec<LibrarySmartView>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, query_json, created_at, updated_at
             FROM library_smart_views ORDER BY name COLLATE NOCASE, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, query_json, created_at, updated_at) = row?;
            let query =
                serde_json::from_str::<LibrarySmartViewQuery>(&query_json).map_err(|error| {
                    AppError::internal("could not parse a saved Library view", error)
                })?;
            Ok(LibrarySmartView {
                id,
                name,
                query,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            })
        })
        .collect()
    }

    pub fn library_smart_view_counts(&self) -> Result<HashMap<String, usize>, AppError> {
        let mut counts = HashMap::new();
        for view in self.list_library_smart_views()? {
            let request = library_query_from_smart_view(&view.query, 1, 1);
            if let Ok(result) = self.query_library_items(request) {
                counts.insert(view.id, result.total_items);
            }
        }
        Ok(counts)
    }

    pub fn save_library_smart_view(
        &self,
        request: SaveLibrarySmartViewRequest,
    ) -> Result<LibrarySmartView, AppError> {
        let SaveLibrarySmartViewRequest { id, name, query } = request;
        let name = validate_smart_view_name(&name)?;
        let query = normalize_smart_view_query(query)?;
        let id = if let Some(id) = id {
            validate_uuid(&id, "smart view")?;
            id
        } else {
            Uuid::new_v4().to_string()
        };
        let query_json = serde_json::to_string(&query)
            .map_err(|error| AppError::internal("could not serialize the smart view", error))?;
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let duplicate: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM library_smart_views
                WHERE name = ?1 COLLATE NOCASE AND id != ?2
             )",
            params![name, id],
            |row| row.get(0),
        )?;
        if duplicate {
            return Err(AppError::new(
                "duplicate_library_smart_view",
                "A saved Library view already uses that name.",
            ));
        }
        connection.execute(
            "INSERT INTO library_smart_views (id, name, query_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                query_json = excluded.query_json,
                updated_at = excluded.updated_at",
            params![id, name, query_json, now],
        )?;
        drop(connection);
        self.list_library_smart_views()?
            .into_iter()
            .find(|view| view.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "library_smart_view_not_found",
                    "The saved view was not found.",
                )
            })
    }

    pub fn delete_library_smart_view(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "smart view")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute("DELETE FROM library_smart_views WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_smart_view_not_found",
                "The saved view was not found.",
            ));
        }
        Ok(())
    }

    pub fn related_library_items(
        &self,
        item_id: &str,
        limit: usize,
    ) -> Result<Vec<LibraryItem>, AppError> {
        validate_uuid(item_id, "library item")?;
        if !(1..=24).contains(&limit) {
            return Err(AppError::new(
                "invalid_library_related_limit",
                "Related media requests must contain between 1 and 24 items.",
            ));
        }
        let item = self.get_library_item(item_id)?;
        let metadata = self.get_library_rich_metadata(item_id)?;
        let mut requests = Vec::with_capacity(4);
        if let Some(series) = metadata.effective.series {
            let mut request = empty_library_query_request(
                1,
                u32::try_from(limit.saturating_add(1)).unwrap_or(25),
            );
            request.series.push(series);
            requests.push(request);
        }
        if let Some(creator) = metadata.effective.creator {
            let mut request = empty_library_query_request(
                1,
                u32::try_from(limit.saturating_add(1)).unwrap_or(25),
            );
            request.creators.push(creator);
            requests.push(request);
        }
        if let Some(site) = item.site {
            let mut request = empty_library_query_request(
                1,
                u32::try_from(limit.saturating_add(1)).unwrap_or(25),
            );
            request.sites.push(site);
            requests.push(request);
        }
        if let Some(source_id) = item.source_id {
            let mut request = empty_library_query_request(
                1,
                u32::try_from(limit.saturating_add(1)).unwrap_or(25),
            );
            request.source_id = Some(source_id);
            requests.push(request);
        }

        let mut related = Vec::with_capacity(limit);
        let mut seen = HashSet::with_capacity(limit.saturating_add(1));
        seen.insert(item_id.to_owned());
        for mut request in requests {
            request.media_kind = item.media_kind;
            request.sort = LibrarySort::PublishedDesc;
            for candidate in self.query_library_items(request)?.items {
                if seen.insert(candidate.id.clone()) {
                    related.push(candidate);
                    if related.len() == limit {
                        return Ok(related);
                    }
                }
            }
        }
        Ok(related)
    }

    pub fn list_library_collections(&self) -> Result<Vec<LibraryCollection>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library collections are unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, created_at, updated_at FROM library_collections
             ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;
        let base = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut membership = connection.prepare(
            "SELECT item_id FROM library_collection_items
             WHERE collection_id = ?1 ORDER BY added_at DESC",
        )?;
        let mut collections = Vec::with_capacity(base.len());
        for (id, name, created_at, updated_at) in base {
            let item_ids = membership
                .query_map([&id], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            collections.push(LibraryCollection {
                id,
                name,
                item_ids,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        drop(membership);
        drop(connection);
        for collection in &mut collections {
            collection.item_ids =
                self.filter_visible_library_item_ids(std::mem::take(&mut collection.item_ids))?;
        }
        Ok(collections)
    }

    pub fn save_library_collection(
        &self,
        request: SaveLibraryCollectionRequest,
    ) -> Result<LibraryCollection, AppError> {
        let name = validate_collection_name(&request.name)?;
        let id = match request.id {
            Some(id) => Uuid::parse_str(&id)
                .map_err(|_| {
                    AppError::new(
                        "invalid_library_collection",
                        "The collection identifier is invalid.",
                    )
                })?
                .to_string(),
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library collections are unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO library_collections (id, name, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, updated_at = excluded.updated_at",
            params![id, name, now],
        )?;
        drop(connection);
        self.list_library_collections()?
            .into_iter()
            .find(|collection| collection.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "library_collection_not_found",
                    "The collection could not be loaded after saving.",
                )
            })
    }

    pub fn delete_library_collection(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "collection")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library collections are unavailable.",
            )
        })?;
        if connection.execute("DELETE FROM library_collections WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_collection_not_found",
                "The library collection was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_playlists(&self) -> Result<Vec<LibraryPlaylist>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, description, created_at, updated_at
             FROM library_playlists ORDER BY updated_at DESC, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        let base = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut membership = connection.prepare(
            "SELECT item_id FROM library_playlist_items
             WHERE playlist_id = ?1 ORDER BY position ASC, added_at ASC, item_id ASC",
        )?;
        let mut playlists = Vec::with_capacity(base.len());
        for (id, name, description, created_at, updated_at) in base {
            let item_ids = membership
                .query_map([&id], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            playlists.push(LibraryPlaylist {
                id,
                name,
                description,
                item_ids,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        drop(membership);
        drop(connection);
        for playlist in &mut playlists {
            playlist.item_ids =
                self.filter_visible_library_item_ids(std::mem::take(&mut playlist.item_ids))?;
        }
        Ok(playlists)
    }

    pub fn save_library_playlist(
        &self,
        request: SaveLibraryPlaylistRequest,
    ) -> Result<LibraryPlaylist, AppError> {
        let name = validate_playlist_name(&request.name)?;
        let description = validate_playlist_description(&request.description)?;
        let id = match request.id {
            Some(id) => parse_uuid(
                &id,
                "invalid_library_playlist",
                "The playlist identifier is invalid.",
            )?,
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        let duplicate: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_playlists
             WHERE name = ?1 COLLATE NOCASE AND id <> ?2)",
            params![name, id],
            |row| row.get(0),
        )?;
        if duplicate {
            return Err(AppError::new(
                "library_playlist_name_exists",
                "Choose a different playlist name.",
            ));
        }
        connection.execute(
            "INSERT INTO library_playlists (id, name, description, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                description = excluded.description,
                updated_at = excluded.updated_at",
            params![id, name, description, now],
        )?;
        drop(connection);
        self.list_library_playlists()?
            .into_iter()
            .find(|playlist| playlist.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "library_playlist_not_found",
                    "The library playlist was not found.",
                )
            })
    }

    pub fn delete_library_playlist(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "playlist")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        if connection.execute("DELETE FROM library_playlists WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_playlist_not_found",
                "The library playlist was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_moments(&self, item_id: &str) -> Result<Vec<LibraryMoment>, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library moments are unavailable.")
        })?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1)",
            [item_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(library_item_not_found());
        }
        let mut statement = connection.prepare(
            "SELECT id, item_id, label, note, start_seconds, end_seconds, color, created_at, updated_at
             FROM library_moments
             WHERE item_id = ?1
             ORDER BY start_seconds ASC, created_at ASC, id ASC
             LIMIT 500",
        )?;
        let rows = statement.query_map([item_id], library_moment_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn list_library_moments_for_backup(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryMoment>, AppError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library moments are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, item_id, label, note, start_seconds, end_seconds, color, created_at, updated_at
             FROM library_moments
             ORDER BY item_id ASC, start_seconds ASC, created_at ASC, id ASC
             LIMIT ?1",
        )?;
        let rows = statement.query_map([limit], library_moment_from_row)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn save_library_moment(
        &self,
        request: SaveLibraryMomentRequest,
    ) -> Result<LibraryMoment, AppError> {
        validate_uuid(&request.item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(&request.item_id)? {
            return Err(library_locked_error());
        }
        let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        validate_uuid(&id, "moment")?;
        let label = validate_moment_label(&request.label)?;
        let note = validate_moment_note(&request.note)?;
        let color = validate_moment_color(&request.color)?;
        validate_moment_times(request.start_seconds, request.end_seconds)?;
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library moments are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        let item: Option<(String, bool, Option<f64>)> = transaction
            .query_row(
                "SELECT COALESCE(media_kind, ''), missing, duration_seconds
                 FROM media_library WHERE id = ?1",
                [&request.item_id],
                |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0, row.get(2)?)),
            )
            .optional()?;
        let Some((media_kind, missing, duration)) = item else {
            return Err(library_item_not_found());
        };
        if media_kind != "video" || missing {
            return Err(AppError::new(
                "library_moment_video_required",
                "Moments can only be saved for available videos.",
            ));
        }
        if let Some(duration) = duration.filter(|value| value.is_finite() && *value > 0.0) {
            let last = request.end_seconds.unwrap_or(request.start_seconds);
            if last > duration + 1.0 {
                return Err(AppError::new(
                    "invalid_library_moment_time",
                    "The moment must be within the video's duration.",
                ));
            }
        }
        transaction.execute(
            "INSERT INTO library_moments
                (id, item_id, label, note, start_seconds, end_seconds, color, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET
                item_id = excluded.item_id,
                label = excluded.label,
                note = excluded.note,
                start_seconds = excluded.start_seconds,
                end_seconds = excluded.end_seconds,
                color = excluded.color,
                updated_at = excluded.updated_at",
            params![
                id,
                request.item_id,
                label,
                note,
                request.start_seconds,
                request.end_seconds,
                color,
                now,
            ],
        )?;
        let saved = transaction.query_row(
            "SELECT id, item_id, label, note, start_seconds, end_seconds, color, created_at, updated_at
             FROM library_moments WHERE id = ?1",
            [&id],
            library_moment_from_row,
        )?;
        transaction.commit()?;
        Ok(saved)
    }

    pub fn delete_library_moment(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "moment")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library moments are unavailable.")
        })?;
        if connection.execute("DELETE FROM library_moments WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_moment_not_found",
                "The saved moment was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_montages(&self) -> Result<Vec<LibraryMontage>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library montages are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, notes, fade_seconds, segment_count, created_at, updated_at
             FROM library_montages
             ORDER BY updated_at DESC, name COLLATE NOCASE ASC
             LIMIT 200",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, usize>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let headers = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut segment_statement = connection.prepare(
            "SELECT item_id, label, start_seconds, end_seconds
             FROM library_montage_segments
             WHERE montage_id = ?1
             ORDER BY position ASC",
        )?;
        let mut montages = Vec::with_capacity(headers.len());
        for (id, name, notes, fade_seconds, expected_segment_count, created_at, updated_at) in
            headers
        {
            let segments = segment_statement
                .query_map([&id], |row| {
                    Ok(LibraryMontageSegment {
                        item_id: row.get(0)?,
                        label: row.get(1)?,
                        start_seconds: row.get(2)?,
                        end_seconds: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            let incomplete = segments.len() != expected_segment_count;
            montages.push(LibraryMontage {
                id,
                name,
                notes,
                fade_seconds,
                segments,
                expected_segment_count,
                incomplete,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        drop(segment_statement);
        drop(connection);
        if self.is_library_privacy_locked() {
            let mut visible_montages = Vec::with_capacity(montages.len());
            for mut montage in montages {
                let original_count = montage.segments.len();
                let mut visible_segments = Vec::with_capacity(original_count);
                for segment in montage.segments {
                    if !self.is_library_item_private(&segment.item_id)? {
                        visible_segments.push(segment);
                    }
                }
                if visible_segments.is_empty() && original_count > 0 {
                    continue;
                }
                if visible_segments.len() != original_count {
                    montage.expected_segment_count = visible_segments.len();
                    montage.incomplete = false;
                }
                montage.segments = visible_segments;
                visible_montages.push(montage);
            }
            return Ok(visible_montages);
        }
        Ok(montages)
    }

    pub fn get_library_montage(&self, id: &str) -> Result<LibraryMontage, AppError> {
        validate_uuid(id, "montage")?;
        self.list_library_montages()?
            .into_iter()
            .find(|montage| montage.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "library_montage_not_found",
                    "The montage project was not found.",
                )
            })
    }

    pub fn save_library_montage(
        &self,
        request: SaveLibraryMontageRequest,
    ) -> Result<LibraryMontage, AppError> {
        let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        validate_uuid(&id, "montage")?;
        let name = validate_montage_name(&request.name)?;
        let notes = validate_montage_notes(&request.notes)?;
        validate_montage_fade(request.fade_seconds)?;
        validate_montage_segments(&request.segments)?;

        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library montages are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_montages WHERE id = ?1)",
            [&id],
            |row| row.get(0),
        )?;
        if !exists {
            let count: i64 =
                transaction.query_row("SELECT COUNT(*) FROM library_montages", [], |row| {
                    row.get(0)
                })?;
            if count >= 200 {
                return Err(AppError::new(
                    "library_montage_limit",
                    "The Library can store at most 200 montage projects.",
                ));
            }
        }

        let mut total_duration = 0.0;
        for segment in &request.segments {
            validate_uuid(&segment.item_id, "library item")?;
            let item: Option<(String, bool, Option<f64>)> = transaction
                .query_row(
                    "SELECT COALESCE(media_kind, ''), missing, duration_seconds
                     FROM media_library WHERE id = ?1",
                    [&segment.item_id],
                    |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0, row.get(2)?)),
                )
                .optional()?;
            let Some((media_kind, missing, duration)) = item else {
                return Err(library_item_not_found());
            };
            if !matches!(media_kind.as_str(), "video" | "both") || missing {
                return Err(AppError::new(
                    "library_montage_video_required",
                    "Montages can only use available Library videos.",
                ));
            }
            if let Some(duration) = duration.filter(|value| value.is_finite() && *value > 0.0) {
                if segment.end_seconds > duration + 1.0 {
                    return Err(AppError::new(
                        "invalid_library_montage_range",
                        "Every montage segment must fit within its source video's duration.",
                    ));
                }
            }
            total_duration += segment.end_seconds - segment.start_seconds;
        }
        if total_duration > 21_600.0 {
            return Err(AppError::new(
                "library_montage_duration_limit",
                "A montage can contain at most six hours of source video.",
            ));
        }

        let now = Utc::now().timestamp();
        transaction.execute(
            "INSERT INTO library_montages
                (id, name, notes, fade_seconds, segment_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                notes = excluded.notes,
                fade_seconds = excluded.fade_seconds,
                segment_count = excluded.segment_count,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                notes,
                request.fade_seconds,
                request.segments.len(),
                now
            ],
        )?;
        transaction.execute(
            "DELETE FROM library_montage_segments WHERE montage_id = ?1",
            [&id],
        )?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO library_montage_segments
                    (montage_id, position, item_id, label, start_seconds, end_seconds)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (position, segment) in request.segments.iter().enumerate() {
                statement.execute(params![
                    id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    segment.item_id,
                    segment.label.trim(),
                    segment.start_seconds,
                    segment.end_seconds,
                ])?;
            }
        }
        transaction.commit()?;
        drop(connection);
        self.get_library_montage(&id)
    }

    pub fn restore_library_montage(
        &self,
        request: SaveLibraryMontageRequest,
        expected_segment_count: usize,
    ) -> Result<LibraryMontage, AppError> {
        if expected_segment_count < request.segments.len() || expected_segment_count > 50 {
            return Err(AppError::new(
                "invalid_library_montage_segments",
                "The restored montage has an invalid expected segment count.",
            ));
        }
        if request.segments.is_empty() && expected_segment_count > 0 {
            let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
            validate_uuid(&id, "montage")?;
            let name = validate_montage_name(&request.name)?;
            let notes = validate_montage_notes(&request.notes)?;
            validate_montage_fade(request.fade_seconds)?;
            let mut connection = self.connection.lock().map_err(|_| {
                AppError::new("database_unavailable", "Library montages are unavailable.")
            })?;
            let transaction = connection.transaction()?;
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM library_montages WHERE id = ?1)",
                [&id],
                |row| row.get(0),
            )?;
            let count: i64 =
                transaction.query_row("SELECT COUNT(*) FROM library_montages", [], |row| {
                    row.get(0)
                })?;
            if !exists && count >= 200 {
                return Err(AppError::new(
                    "library_montage_limit",
                    "The Library can store at most 200 montage projects.",
                ));
            }
            let now = Utc::now().timestamp();
            transaction.execute(
                "INSERT INTO library_montages
                    (id, name, notes, fade_seconds, segment_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    notes = excluded.notes,
                    fade_seconds = excluded.fade_seconds,
                    segment_count = excluded.segment_count,
                    updated_at = excluded.updated_at",
                params![
                    id,
                    name,
                    notes,
                    request.fade_seconds,
                    expected_segment_count,
                    now
                ],
            )?;
            transaction.execute(
                "DELETE FROM library_montage_segments WHERE montage_id = ?1",
                [&id],
            )?;
            transaction.commit()?;
            drop(connection);
            return self.get_library_montage(&id);
        }
        let saved = self.save_library_montage(request)?;
        if expected_segment_count != saved.segments.len() {
            let connection = self.connection.lock().map_err(|_| {
                AppError::new("database_unavailable", "Library montages are unavailable.")
            })?;
            connection.execute(
                "UPDATE library_montages SET segment_count = ?2 WHERE id = ?1",
                params![saved.id, expected_segment_count],
            )?;
            drop(connection);
            return self.get_library_montage(&saved.id);
        }
        Ok(saved)
    }

    pub fn delete_library_montage(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "montage")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library montages are unavailable.")
        })?;
        if connection.execute("DELETE FROM library_montages WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_montage_not_found",
                "The montage project was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_compare_sessions(&self) -> Result<Vec<LibraryCompareSession>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved comparison sessions are unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, notes, active_audio_item_id, multiple_audio, created_at, updated_at
             FROM library_compare_sessions
             ORDER BY updated_at DESC, name COLLATE NOCASE ASC
             LIMIT 200",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, bool>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let base = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut item_statement = connection.prepare(
            "SELECT item_id, position, playback_position_seconds, playback_rate
             FROM library_compare_session_items
             WHERE session_id = ?1
             ORDER BY position ASC, item_id ASC",
        )?;
        let mut sessions = Vec::with_capacity(base.len());
        for (id, name, notes, active_audio_item_id, multiple_audio, created_at, updated_at) in base
        {
            let items = item_statement
                .query_map([&id], |row| {
                    Ok(LibraryCompareSessionItem {
                        item_id: row.get(0)?,
                        position: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(usize::MAX),
                        playback_position_seconds: row.get(2)?,
                        playback_rate: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            sessions.push(LibraryCompareSession {
                id,
                name,
                notes,
                items,
                active_audio_item_id,
                multiple_audio,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        Ok(sessions)
    }

    pub fn save_library_compare_session(
        &self,
        request: SaveLibraryCompareSessionRequest,
    ) -> Result<LibraryCompareSession, AppError> {
        let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        validate_uuid(&id, "comparison session")?;
        let name = validate_compare_session_name(&request.name)?;
        let notes = validate_compare_session_notes(&request.notes)?;
        if !(2..=4).contains(&request.items.len()) {
            return Err(AppError::new(
                "invalid_compare_session_items",
                "A saved comparison must contain between 2 and 4 videos.",
            ));
        }
        let mut items = request.items;
        items.sort_by_key(|item| item.position);
        let mut unique = HashSet::new();
        for item in &items {
            validate_uuid(&item.item_id, "comparison video")?;
            if !unique.insert(item.item_id.clone())
                || !item.playback_position_seconds.is_finite()
                || item.playback_position_seconds < 0.0
                || !item.playback_rate.is_finite()
                || !(0.25..=4.0).contains(&item.playback_rate)
            {
                return Err(AppError::new(
                    "invalid_compare_session_items",
                    "Comparison videos and playback settings must be valid and unique.",
                ));
            }
        }
        if request
            .active_audio_item_id
            .as_ref()
            .is_some_and(|active| !unique.contains(active))
        {
            return Err(AppError::new(
                "invalid_compare_session_audio",
                "The active audio video must belong to the saved comparison.",
            ));
        }
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved comparison sessions are unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        let duplicate: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_compare_sessions WHERE name = ?1 COLLATE NOCASE AND id <> ?2)",
            params![name, id],
            |row| row.get(0),
        )?;
        if duplicate {
            return Err(AppError::new(
                "compare_session_name_exists",
                "Choose a different comparison session name.",
            ));
        }
        for item in &items {
            let catalog: Option<(String, bool, Option<f64>)> = transaction
                .query_row(
                    "SELECT COALESCE(media_kind, ''), missing, duration_seconds FROM media_library WHERE id = ?1",
                    [&item.item_id],
                    |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0, row.get(2)?)),
                )
                .optional()?;
            let Some((media_kind, missing, duration)) = catalog else {
                return Err(library_item_not_found());
            };
            if media_kind != "video" || missing {
                return Err(AppError::new(
                    "compare_session_video_required",
                    "Saved comparisons require available videos.",
                ));
            }
            if duration
                .filter(|value| value.is_finite() && *value > 0.0)
                .is_some_and(|duration| item.playback_position_seconds > duration + 1.0)
            {
                return Err(AppError::new(
                    "invalid_compare_session_items",
                    "A saved playback position is outside the video's duration.",
                ));
            }
        }
        transaction.execute(
            "INSERT INTO library_compare_sessions
                (id, name, notes, active_audio_item_id, multiple_audio, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                notes = excluded.notes,
                active_audio_item_id = excluded.active_audio_item_id,
                multiple_audio = excluded.multiple_audio,
                updated_at = excluded.updated_at",
            params![
                id,
                name,
                notes,
                request.active_audio_item_id,
                request.multiple_audio,
                now,
            ],
        )?;
        transaction.execute(
            "DELETE FROM library_compare_session_items WHERE session_id = ?1",
            [&id],
        )?;
        for (position, item) in items.iter().enumerate() {
            transaction.execute(
                "INSERT INTO library_compare_session_items
                    (session_id, item_id, position, playback_position_seconds, playback_rate)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    item.item_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    item.playback_position_seconds,
                    item.playback_rate,
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.list_library_compare_sessions()?
            .into_iter()
            .find(|session| session.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "compare_session_not_found",
                    "The comparison session could not be loaded after saving.",
                )
            })
    }

    pub fn delete_library_compare_session(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "comparison session")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Saved comparison sessions are unavailable.",
            )
        })?;
        if connection.execute("DELETE FROM library_compare_sessions WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "compare_session_not_found",
                "The saved comparison session was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_media_stacks(&self) -> Result<Vec<LibraryMediaStack>, AppError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Media stacks are unavailable."))?;
        let mut statement = connection.prepare(
            "SELECT id, name, notes, cover_item_id, created_at, updated_at
             FROM library_media_stacks
             ORDER BY updated_at DESC, name COLLATE NOCASE ASC
             LIMIT 2000",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?;
        let base = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut member_statement = connection.prepare(
            "SELECT item_id, position, role
             FROM library_media_stack_items
             WHERE stack_id = ?1
             ORDER BY position ASC, item_id ASC",
        )?;
        let mut stacks = Vec::with_capacity(base.len());
        for (id, name, notes, cover_item_id, created_at, updated_at) in base {
            let members = member_statement
                .query_map([&id], |row| {
                    Ok(LibraryMediaStackMember {
                        item_id: row.get(0)?,
                        position: usize::try_from(row.get::<_, i64>(1)?).unwrap_or(usize::MAX),
                        role: parse_media_stack_role(&row.get::<_, String>(2)?),
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            stacks.push(LibraryMediaStack {
                id,
                name,
                notes,
                cover_item_id,
                members,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
            });
        }
        drop(member_statement);
        drop(connection);
        if self.is_library_privacy_locked() {
            let mut visible_stacks = Vec::with_capacity(stacks.len());
            for mut stack in stacks {
                let mut members = Vec::with_capacity(stack.members.len());
                for member in stack.members {
                    if !self.is_library_item_private(&member.item_id)? {
                        members.push(member);
                    }
                }
                if members.is_empty() {
                    continue;
                }
                if stack
                    .cover_item_id
                    .as_deref()
                    .is_some_and(|cover| !members.iter().any(|member| member.item_id == cover))
                {
                    stack.cover_item_id = None;
                }
                stack.members = members;
                visible_stacks.push(stack);
            }
            return Ok(visible_stacks);
        }
        Ok(stacks)
    }

    pub fn save_library_media_stack(
        &self,
        request: SaveLibraryMediaStackRequest,
    ) -> Result<LibraryMediaStack, AppError> {
        let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        validate_uuid(&id, "media stack")?;
        let name = validate_media_stack_name(&request.name)?;
        let notes = validate_media_stack_notes(&request.notes)?;
        if !(2..=100).contains(&request.members.len()) {
            return Err(AppError::new(
                "invalid_media_stack_size",
                "A media stack must contain between 2 and 100 items.",
            ));
        }

        let mut members = request.members;
        members.sort_by_key(|member| member.position);
        let mut item_ids = HashSet::with_capacity(members.len());
        let mut original_count = 0usize;
        for member in &members {
            validate_uuid(&member.item_id, "library item")?;
            if !item_ids.insert(member.item_id.clone()) {
                return Err(AppError::new(
                    "invalid_media_stack_members",
                    "Each item may appear only once in a media stack.",
                ));
            }
            if member.role == LibraryMediaStackRole::Original {
                original_count += 1;
            }
        }
        if original_count != 1 {
            return Err(AppError::new(
                "invalid_media_stack_original",
                "Choose exactly one original item for the media stack.",
            ));
        }
        let cover_item_id = request.cover_item_id.or_else(|| {
            members
                .iter()
                .find(|member| member.role == LibraryMediaStackRole::Original)
                .map(|member| member.item_id.clone())
        });
        if cover_item_id
            .as_ref()
            .is_some_and(|cover| !item_ids.contains(cover))
        {
            return Err(AppError::new(
                "invalid_media_stack_cover",
                "The stack cover must be one of its items.",
            ));
        }

        let now = Utc::now().timestamp_millis();
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Media stacks are unavailable."))?;
        let transaction = connection.transaction()?;
        let duplicate: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_media_stacks
             WHERE name = ?1 COLLATE NOCASE AND id <> ?2)",
            params![name, id],
            |row| row.get(0),
        )?;
        if duplicate {
            return Err(AppError::new(
                "media_stack_name_exists",
                "Choose a different media stack name.",
            ));
        }

        let mut media_kind: Option<String> = None;
        for member in &members {
            let catalog: Option<(String, bool)> = transaction
                .query_row(
                    "SELECT COALESCE(media_kind, ''), missing FROM media_library WHERE id = ?1",
                    [&member.item_id],
                    |row| Ok((row.get(0)?, row.get::<_, i64>(1)? != 0)),
                )
                .optional()?;
            let Some((kind, missing)) = catalog else {
                return Err(library_item_not_found());
            };
            if missing || !matches!(kind.as_str(), "video" | "image") {
                return Err(AppError::new(
                    "media_stack_available_media_required",
                    "Media stacks require available videos or images.",
                ));
            }
            if media_kind
                .as_ref()
                .is_some_and(|expected| expected != &kind)
            {
                return Err(AppError::new(
                    "media_stack_kind_mismatch",
                    "A media stack cannot mix videos and images.",
                ));
            }
            media_kind.get_or_insert(kind);
            let owner: Option<String> = transaction
                .query_row(
                    "SELECT stack_id FROM library_media_stack_items
                     WHERE item_id = ?1 AND stack_id <> ?2",
                    params![member.item_id, id],
                    |row| row.get(0),
                )
                .optional()?;
            if owner.is_some() {
                return Err(AppError::new(
                    "media_stack_item_in_use",
                    "One of these items already belongs to another media stack.",
                ));
            }
        }

        transaction.execute(
            "DELETE FROM library_media_stack_items WHERE stack_id = ?1",
            [&id],
        )?;
        transaction.execute(
            "INSERT INTO library_media_stacks
                (id, name, notes, cover_item_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                notes = excluded.notes,
                cover_item_id = excluded.cover_item_id,
                updated_at = excluded.updated_at",
            params![id, name, notes, cover_item_id, now],
        )?;
        for (position, member) in members.iter().enumerate() {
            transaction.execute(
                "INSERT INTO library_media_stack_items (stack_id, item_id, position, role)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    id,
                    member.item_id,
                    i64::try_from(position).unwrap_or(i64::MAX),
                    member.role.as_str(),
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.list_library_media_stacks()?
            .into_iter()
            .find(|stack| stack.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "media_stack_not_found",
                    "The media stack could not be loaded after saving.",
                )
            })
    }

    pub fn delete_library_media_stack(&self, id: &str) -> Result<(), AppError> {
        validate_uuid(id, "media stack")?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Media stacks are unavailable."))?;
        if connection.execute("DELETE FROM library_media_stacks WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "media_stack_not_found",
                "The media stack was not found.",
            ));
        }
        Ok(())
    }

    pub fn list_library_curation_queue(
        &self,
        review_later: bool,
        limit: usize,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let limit = limit.clamp(1, 500);
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The curation queue is unavailable.")
        })?;
        let sql = if review_later {
            "SELECT media_library.id FROM media_library
             INNER JOIN library_curation_decisions decisions ON decisions.item_id = media_library.id
             WHERE media_library.missing = 0 AND decisions.decision = 'reviewLater'
             ORDER BY decisions.reviewed_at ASC, media_library.id ASC LIMIT ?1"
        } else {
            "SELECT media_library.id FROM media_library
             LEFT JOIN library_curation_decisions decisions ON decisions.item_id = media_library.id
             WHERE media_library.missing = 0 AND decisions.item_id IS NULL
             ORDER BY media_library.added_at DESC, media_library.id ASC LIMIT ?1"
        };
        let mut statement = connection.prepare(sql)?;
        let ids = statement
            .query_map([i64::try_from(limit).unwrap_or(500)], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        drop(statement);
        drop(connection);
        let items = self.list_library_items_by_ids(ids.clone())?;
        let by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(ids
            .into_iter()
            .filter_map(|id| by_id.get(&id).cloned())
            .collect())
    }

    pub fn get_library_curation_state(
        &self,
        item_id: &str,
    ) -> Result<LibraryCurationState, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Curation state is unavailable."))?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1)",
            [item_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(library_item_not_found());
        }
        let state = connection
            .query_row(
                "SELECT decision, reviewed_at FROM library_curation_decisions WHERE item_id = ?1",
                [item_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        Ok(LibraryCurationState {
            item_id: item_id.to_owned(),
            decision: state
                .as_ref()
                .and_then(|(decision, _)| parse_curation_decision(decision)),
            reviewed_at: state.map(|(_, reviewed_at)| timestamp_or_epoch(reviewed_at)),
        })
    }

    pub fn list_library_curation_states_for_backup(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryCurationState>, AppError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Curation state is unavailable."))?;
        let mut statement = connection.prepare(
            "SELECT item_id, decision, reviewed_at FROM library_curation_decisions
             ORDER BY reviewed_at ASC, item_id ASC LIMIT ?1",
        )?;
        let rows = statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            let decision = row.get::<_, String>(1)?;
            Ok(LibraryCurationState {
                item_id: row.get(0)?,
                decision: parse_curation_decision(&decision),
                reviewed_at: Some(timestamp_or_epoch(row.get(2)?)),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_library_curation_decision(
        &self,
        item_id: &str,
        decision: Option<LibraryCurationDecision>,
    ) -> Result<LibraryCurationState, AppError> {
        validate_uuid(item_id, "library item")?;
        if self.is_library_privacy_locked() && self.is_library_item_private(item_id)? {
            return Err(library_locked_error());
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| AppError::new("database_unavailable", "Curation state is unavailable."))?;
        let available: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1 AND missing = 0)",
            [item_id],
            |row| row.get(0),
        )?;
        if !available {
            return Err(AppError::new(
                "library_curation_item_unavailable",
                "Only available library items can be reviewed.",
            ));
        }
        if let Some(decision) = decision {
            connection.execute(
                "INSERT INTO library_curation_decisions (item_id, decision, reviewed_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(item_id) DO UPDATE SET
                    decision = excluded.decision,
                    reviewed_at = excluded.reviewed_at",
                params![item_id, decision.as_str(), Utc::now().timestamp_millis()],
            )?;
        } else {
            connection.execute(
                "DELETE FROM library_curation_decisions WHERE item_id = ?1",
                [item_id],
            )?;
        }
        drop(connection);
        self.get_library_curation_state(item_id)
    }

    pub fn add_library_playlist_item(
        &self,
        playlist_id: &str,
        item_id: &str,
    ) -> Result<LibraryPlaylist, AppError> {
        validate_uuid(playlist_id, "playlist")?;
        validate_uuid(item_id, "library item")?;
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        ensure_playlist_exists(&transaction, playlist_id)?;
        ensure_library_video_exists(&transaction, item_id)?;
        let (count, last_position): (i64, i64) = transaction.query_row(
            "SELECT COUNT(*), COALESCE(MAX(position), -1)
             FROM library_playlist_items WHERE playlist_id = ?1",
            [playlist_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let already_added: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM library_playlist_items
             WHERE playlist_id = ?1 AND item_id = ?2)",
            params![playlist_id, item_id],
            |row| row.get(0),
        )?;
        if !already_added && count >= 500 {
            return Err(AppError::new(
                "library_playlist_full",
                "Playlists are limited to 500 videos.",
            ));
        }
        transaction.execute(
            "INSERT OR IGNORE INTO library_playlist_items
             (playlist_id, item_id, position, added_at) VALUES (?1, ?2, ?3, ?4)",
            params![playlist_id, item_id, last_position.saturating_add(1), now],
        )?;
        transaction.execute(
            "UPDATE library_playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, now],
        )?;
        transaction.commit()?;
        drop(connection);
        self.library_playlist(playlist_id)
    }

    pub fn remove_library_playlist_item(
        &self,
        playlist_id: &str,
        item_id: &str,
    ) -> Result<LibraryPlaylist, AppError> {
        validate_uuid(playlist_id, "playlist")?;
        validate_uuid(item_id, "library item")?;
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        ensure_playlist_exists(&transaction, playlist_id)?;
        transaction.execute(
            "DELETE FROM library_playlist_items WHERE playlist_id = ?1 AND item_id = ?2",
            params![playlist_id, item_id],
        )?;
        normalize_playlist_positions(&transaction, playlist_id)?;
        transaction.execute(
            "UPDATE library_playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, now],
        )?;
        transaction.commit()?;
        drop(connection);
        self.library_playlist(playlist_id)
    }

    pub fn move_library_playlist_item(
        &self,
        playlist_id: &str,
        item_id: &str,
        new_index: usize,
    ) -> Result<LibraryPlaylist, AppError> {
        validate_uuid(playlist_id, "playlist")?;
        validate_uuid(item_id, "library item")?;
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library playlists are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        ensure_playlist_exists(&transaction, playlist_id)?;
        let mut item_ids = playlist_item_ids(&transaction, playlist_id)?;
        let old_index = item_ids
            .iter()
            .position(|id| id == item_id)
            .ok_or_else(|| {
                AppError::new(
                    "library_playlist_item_not_found",
                    "This video is not in the selected playlist.",
                )
            })?;
        let moved = item_ids.remove(old_index);
        let index = new_index.min(item_ids.len());
        item_ids.insert(index, moved);
        for (position, id) in item_ids.iter().enumerate() {
            transaction.execute(
                "UPDATE library_playlist_items SET position = ?3
                 WHERE playlist_id = ?1 AND item_id = ?2",
                params![playlist_id, id, i64::try_from(position).unwrap_or(i64::MAX)],
            )?;
        }
        transaction.execute(
            "UPDATE library_playlists SET updated_at = ?2 WHERE id = ?1",
            params![playlist_id, now],
        )?;
        transaction.commit()?;
        drop(connection);
        self.library_playlist(playlist_id)
    }

    pub fn list_library_playlist_items(
        &self,
        playlist_id: &str,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let playlist = self.library_playlist(playlist_id)?;
        let items = self.list_library_items_by_ids(playlist.item_ids.clone())?;
        let mut by_id = items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        Ok(playlist
            .item_ids
            .iter()
            .filter_map(|id| by_id.remove(id))
            .collect())
    }

    fn library_playlist(&self, id: &str) -> Result<LibraryPlaylist, AppError> {
        validate_uuid(id, "playlist")?;
        self.list_library_playlists()?
            .into_iter()
            .find(|playlist| playlist.id == id)
            .ok_or_else(|| {
                AppError::new(
                    "library_playlist_not_found",
                    "The library playlist was not found.",
                )
            })
    }

    pub fn apply_library_bulk_action(
        &self,
        ids: Vec<String>,
        action: LibraryBulkAction,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let ids = validate_library_item_ids(ids)?;
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let transaction = connection.transaction()?;
        let now = Utc::now().timestamp_millis();
        match action {
            LibraryBulkAction::SetFavorite { favorite } => {
                for id in &ids {
                    ensure_library_item_updated(transaction.execute(
                        "UPDATE media_library SET favorite = ?2, updated_at = ?3 WHERE id = ?1",
                        params![id, favorite, now],
                    )?)?;
                }
            }
            LibraryBulkAction::SetRating { rating } => {
                validate_library_rating(rating)?;
                for id in &ids {
                    ensure_library_item_updated(transaction.execute(
                        "UPDATE media_library SET rating = ?2, updated_at = ?3 WHERE id = ?1",
                        params![id, rating, now],
                    )?)?;
                }
            }
            LibraryBulkAction::AddTags { tags } => {
                let additions = normalize_library_tags(tags)?;
                for id in &ids {
                    let current: Option<String> = transaction
                        .query_row(
                            "SELECT tags_json FROM media_library WHERE id = ?1",
                            [id],
                            |row| row.get(0),
                        )
                        .optional()?;
                    let Some(current) = current else {
                        return Err(library_item_not_found());
                    };
                    let mut tags: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
                    for tag in &additions {
                        if !tags.iter().any(|value| value.eq_ignore_ascii_case(tag)) {
                            tags.push(tag.clone());
                        }
                    }
                    tags.truncate(20);
                    transaction.execute(
                        "UPDATE media_library SET tags_json = ?2, updated_at = ?3 WHERE id = ?1",
                        params![
                            id,
                            serde_json::to_string(&tags).map_err(|error| AppError::internal(
                                "could not serialize media tags",
                                error
                            ))?,
                            now
                        ],
                    )?;
                }
            }
            LibraryBulkAction::AddToCollection { collection_id } => {
                ensure_collection_exists(&transaction, &collection_id)?;
                for id in &ids {
                    ensure_library_item_exists(&transaction, id)?;
                    transaction.execute(
                        "INSERT OR IGNORE INTO library_collection_items
                            (collection_id, item_id, added_at) VALUES (?1, ?2, ?3)",
                        params![collection_id, id, now],
                    )?;
                }
                transaction.execute(
                    "UPDATE library_collections SET updated_at = ?2 WHERE id = ?1",
                    params![collection_id, now],
                )?;
            }
            LibraryBulkAction::RemoveFromCollection { collection_id } => {
                ensure_collection_exists(&transaction, &collection_id)?;
                for id in &ids {
                    ensure_library_item_exists(&transaction, id)?;
                    transaction.execute(
                        "DELETE FROM library_collection_items WHERE collection_id = ?1 AND item_id = ?2",
                        params![collection_id, id],
                    )?;
                }
                transaction.execute(
                    "UPDATE library_collections SET updated_at = ?2 WHERE id = ?1",
                    params![collection_id, now],
                )?;
            }
        }
        transaction.commit()?;
        drop(connection);
        ids.iter().map(|id| self.get_library_item(id)).collect()
    }

    pub fn remove_library_item(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute("DELETE FROM media_library WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        Ok(())
    }

    pub fn set_library_missing(&self, id: &str, missing: bool) -> Result<(), AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection.execute(
            "UPDATE media_library SET missing = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, missing, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn record_library_change(
        &self,
        item_id: &str,
        source_id: Option<&str>,
        kind: LibraryChangeKind,
        previous_path: Option<&str>,
    ) -> Result<LibraryChange, AppError> {
        validate_uuid(item_id, "library item")?;
        if let Some(source_id) = source_id {
            validate_uuid(source_id, "library source")?;
        }
        let id = Uuid::new_v4().to_string();
        let detected_at = Utc::now();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library changes are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM library_changes WHERE item_id = ?1 AND reviewed_at IS NULL",
            [item_id],
        )?;
        transaction.execute(
            "INSERT INTO library_changes
                (id, item_id, source_id, kind, previous_path, detected_at, reviewed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                id,
                item_id,
                source_id,
                library_change_kind_name(kind),
                previous_path,
                detected_at.timestamp_millis()
            ],
        )?;
        transaction.commit()?;
        Ok(LibraryChange {
            id,
            item_id: item_id.to_owned(),
            source_id: source_id.map(str::to_owned),
            kind,
            previous_path: previous_path.map(str::to_owned),
            detected_at,
            reviewed_at: None,
        })
    }

    pub fn list_library_changes(&self) -> Result<Vec<LibraryChange>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library changes are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, item_id, source_id, kind, previous_path, detected_at, reviewed_at
             FROM library_changes WHERE reviewed_at IS NULL
             ORDER BY detected_at DESC LIMIT 500",
        )?;
        let rows = statement.query_map([], library_change_from_row)?;
        let changes = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        self.filter_visible_library_records(changes, |change| &change.item_id)
    }

    pub fn review_library_changes(&self, ids: Vec<String>) -> Result<usize, AppError> {
        let ids = validate_bounded_ids(ids, "library changes")?;
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library changes are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        let mut reviewed = 0usize;
        for id in ids {
            reviewed += transaction.execute(
                "UPDATE library_changes SET reviewed_at = ?2
                 WHERE id = ?1 AND reviewed_at IS NULL",
                params![id, now],
            )?;
        }
        transaction.commit()?;
        Ok(reviewed)
    }

    pub fn save_library_content_hash(
        &self,
        item_id: &str,
        file_size_bytes: u64,
        modified_at_ms: i64,
        sha256: &str,
    ) -> Result<(), AppError> {
        validate_uuid(item_id, "library item")?;
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(AppError::new(
                "invalid_content_hash",
                "The calculated content hash is invalid.",
            ));
        }
        let file_size_bytes = i64::try_from(file_size_bytes).map_err(|_| {
            AppError::new(
                "library_file_too_large",
                "This file is too large to cache duplicate-analysis metadata.",
            )
        })?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Duplicate analysis is unavailable.")
        })?;
        connection.execute(
            "INSERT INTO library_content_hashes
                (item_id, file_size_bytes, modified_at_ms, sha256, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(item_id) DO UPDATE SET
                file_size_bytes = excluded.file_size_bytes,
                modified_at_ms = excluded.modified_at_ms,
                sha256 = excluded.sha256,
                updated_at = excluded.updated_at",
            params![
                item_id,
                file_size_bytes,
                modified_at_ms,
                sha256,
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(())
    }

    pub fn list_current_library_content_hashes(
        &self,
    ) -> Result<Vec<(String, u64, String)>, AppError> {
        let privacy_filter = if self.is_library_privacy_locked() {
            LOCKED_LIBRARY_FILTER_SQL
        } else {
            ""
        };
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Duplicate analysis is unavailable.")
        })?;
        let mut statement = connection.prepare(&format!(
            "SELECT library_content_hashes.item_id, library_content_hashes.file_size_bytes, sha256
             FROM library_content_hashes
             JOIN library_media_metadata ON library_media_metadata.item_id = library_content_hashes.item_id
             JOIN media_library ON media_library.id = library_content_hashes.item_id
             WHERE media_library.missing = 0
             AND library_media_metadata.file_size_bytes = library_content_hashes.file_size_bytes
             AND library_media_metadata.modified_at_ms = library_content_hashes.modified_at_ms
             {privacy_filter}"
        ))?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?.max(0) as u64,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn save_library_file_operation(
        &self,
        operation: &LibraryFileOperation,
    ) -> Result<(), AppError> {
        validate_uuid(&operation.id, "file operation")?;
        validate_uuid(&operation.item_id, "library item")?;
        if let Some(output_item_id) = operation.output_item_id.as_deref() {
            validate_uuid(output_item_id, "output library item")?;
        }
        let source_size_bytes = i64::try_from(operation.source_size_bytes).map_err(|_| {
            AppError::new(
                "library_file_too_large",
                "This file is too large to journal safely.",
            )
        })?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "File operation history is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT INTO library_file_operations
                (id, item_id, output_item_id, kind, state, source_path, destination_path,
                 source_size_bytes, source_modified_at_ms, error, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(id) DO UPDATE SET output_item_id = excluded.output_item_id,
                state = excluded.state, error = excluded.error, updated_at = excluded.updated_at,
                completed_at = excluded.completed_at",
            params![
                operation.id,
                operation.item_id,
                operation.output_item_id,
                library_file_operation_kind_name(operation.kind),
                library_file_operation_state_name(operation.state),
                operation.source_path,
                operation.destination_path,
                source_size_bytes,
                operation.source_modified_at_ms,
                operation.error,
                operation.created_at.timestamp_millis(),
                operation.updated_at.timestamp_millis(),
                operation.completed_at.map(|value| value.timestamp_millis())
            ],
        )?;
        Ok(())
    }

    pub fn get_library_file_operation(&self, id: &str) -> Result<LibraryFileOperation, AppError> {
        validate_uuid(id, "file operation")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "File operation history is unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT id, item_id, output_item_id, kind, state, source_path, destination_path,
                        source_size_bytes, source_modified_at_ms, error, created_at, updated_at,
                        completed_at
                 FROM library_file_operations WHERE id = ?1",
                [id],
                library_file_operation_from_row,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new(
                    "library_file_operation_not_found",
                    "The file operation plan was not found.",
                )
            })
    }

    pub fn transition_library_file_operation(
        &self,
        id: &str,
        expected: LibraryFileOperationState,
        next: LibraryFileOperationState,
    ) -> Result<bool, AppError> {
        validate_uuid(id, "file operation")?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "File operation history is unavailable.",
            )
        })?;
        let changed = connection.execute(
            "UPDATE library_file_operations SET state = ?3, updated_at = ?4
             WHERE id = ?1 AND state = ?2",
            params![
                id,
                library_file_operation_state_name(expected),
                library_file_operation_state_name(next),
                Utc::now().timestamp_millis()
            ],
        )?;
        Ok(changed == 1)
    }

    pub fn list_library_file_operations(&self) -> Result<Vec<LibraryFileOperation>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "File operation history is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, item_id, output_item_id, kind, state, source_path, destination_path,
                    source_size_bytes, source_modified_at_ms, error, created_at, updated_at,
                    completed_at
             FROM library_file_operations WHERE state != 'planned'
             ORDER BY created_at DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], library_file_operation_from_row)?;
        let operations = rows.collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        self.filter_visible_library_records(operations, |operation| &operation.item_id)
    }

    pub fn reconcile_library_file_operations(&self) -> Result<usize, AppError> {
        let now = Utc::now().timestamp_millis();
        let planned_cutoff = now.saturating_sub(24 * 60 * 60 * 1000);
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "File operation history is unavailable.",
            )
        })?;
        let transaction = connection.transaction()?;
        let interrupted = transaction.execute(
            "UPDATE library_file_operations SET state = 'failed', error = ?1,
                    updated_at = ?2, completed_at = ?2 WHERE state = 'executing'",
            params![
                "The application closed during this file operation. Run Verify files before trying another change.",
                now
            ],
        )?;
        transaction.execute(
            "DELETE FROM library_file_operations WHERE state = 'planned' AND created_at < ?1",
            [planned_cutoff],
        )?;
        transaction.commit()?;
        Ok(interrupted)
    }

    pub fn save_library_probe_result(
        &self,
        item_id: &str,
        result: &LibraryProbeResult,
    ) -> Result<Option<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let current_fingerprint = connection
            .query_row(
                "SELECT file_size_bytes, modified_at_ms FROM library_media_metadata WHERE item_id = ?1",
                [item_id],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
            )
            .optional()?;
        let expected_size = i64::try_from(result.file_size_bytes).ok();
        if current_fingerprint != Some((expected_size, Some(result.modified_at_ms))) {
            return Ok(None);
        }
        connection.execute(
            "UPDATE library_media_metadata SET width = ?2, height = ?3, fps = ?4,
                video_codec = ?5, audio_codec = ?6, container = ?7, dynamic_range = ?8,
                state = ?9, error = ?10, updated_at = ?11 WHERE item_id = ?1",
            params![
                item_id,
                result.width,
                result.height,
                result.fps,
                result.video_codec,
                result.audio_codec,
                result.container,
                result.dynamic_range,
                library_metadata_state_name(result.state),
                result.error,
                Utc::now().timestamp_millis(),
            ],
        )?;
        if let Some(duration) = result
            .duration_seconds
            .filter(|value| value.is_finite() && *value >= 0.0)
        {
            connection.execute(
                "UPDATE media_library SET duration_seconds = ?2, updated_at = ?3 WHERE id = ?1",
                params![item_id, duration, Utc::now().timestamp_millis()],
            )?;
        }
        drop(connection);
        self.get_library_item(item_id).map(Some)
    }

    pub fn reset_library_probe(
        &self,
        item_id: &str,
        file_size_bytes: u64,
        modified_at_ms: i64,
    ) -> Result<LibraryItem, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        connection.execute(
            "UPDATE library_media_metadata SET file_size_bytes = ?2, modified_at_ms = ?3,
                width = NULL, height = NULL, fps = NULL, video_codec = NULL,
                audio_codec = NULL, container = NULL, dynamic_range = NULL,
                state = 'pending', error = NULL, updated_at = ?4 WHERE item_id = ?1",
            params![
                item_id,
                i64::try_from(file_size_bytes).ok(),
                modified_at_ms,
                Utc::now().timestamp_millis(),
            ],
        )?;
        drop(connection);
        self.get_library_item(item_id)
    }

    pub fn list_library_sources(&self) -> Result<Vec<LibrarySource>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library sources are unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, path, media_kind, recursive, watch_enabled, private,
                    include_patterns_json, exclude_patterns_json, status, last_error,
                    last_scanned_at, item_count, created_at, updated_at
             FROM library_sources ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], library_source_from_row)?;
        let mut sources = rows.collect::<Result<Vec<_>, _>>()?;
        if self.is_library_privacy_locked() {
            sources.retain(|source| !source.private);
        }
        Ok(sources)
    }

    pub fn get_library_source(&self, id: &str) -> Result<LibrarySource, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library sources are unavailable.")
        })?;
        connection
            .query_row(
                "SELECT id, name, path, media_kind, recursive, watch_enabled, private,
                        include_patterns_json, exclude_patterns_json, status, last_error,
                        last_scanned_at, item_count, created_at, updated_at
                 FROM library_sources WHERE id = ?1",
                [id],
                library_source_from_row,
            )
            .optional()?
            .ok_or_else(|| {
                AppError::new(
                    "library_source_not_found",
                    "The library source was not found.",
                )
            })
    }

    pub fn save_library_source(
        &self,
        request: SaveLibrarySourceRequest,
    ) -> Result<LibrarySource, AppError> {
        let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let now = Utc::now().timestamp_millis();
        let includes = serde_json::to_string(&request.include_patterns)
            .map_err(|error| AppError::internal("could not serialize include patterns", error))?;
        let excludes = serde_json::to_string(&request.exclude_patterns)
            .map_err(|error| AppError::internal("could not serialize exclude patterns", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library sources are unavailable.")
        })?;
        connection.execute(
            "INSERT INTO library_sources
                (id, name, path, media_kind, recursive, watch_enabled, private,
                 include_patterns_json, exclude_patterns_json, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'online', ?10, ?10)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name, path = excluded.path, media_kind = excluded.media_kind,
                recursive = excluded.recursive, watch_enabled = excluded.watch_enabled,
                private = excluded.private, include_patterns_json = excluded.include_patterns_json,
                exclude_patterns_json = excluded.exclude_patterns_json, updated_at = excluded.updated_at",
            params![
                id,
                request.name,
                request.path,
                library_media_kind_name(request.media_kind),
                request.recursive,
                request.watch_enabled,
                request.private,
                includes,
                excludes,
                now,
            ],
        )?;
        drop(connection);
        self.get_library_source(&id)
    }

    pub fn get_library_source_automation(
        &self,
        source_id: &str,
    ) -> Result<LibrarySourceAutomation, AppError> {
        validate_uuid(source_id, "library source")?;
        let source = self.get_library_source(source_id)?;
        if source.private && self.is_library_privacy_locked() {
            return Err(library_locked_error());
        }
        self.get_library_source_automation_internal(source_id)
    }

    fn get_library_source_automation_internal(
        &self,
        source_id: &str,
    ) -> Result<LibrarySourceAutomation, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Folder automation is unavailable.")
        })?;
        connection
            .query_row(
                "SELECT source_id, tags_json, collection_id, favorite
                 FROM library_source_automation WHERE source_id = ?1",
                [source_id],
                |row| {
                    let tags_json: String = row.get(1)?;
                    Ok(LibrarySourceAutomation {
                        source_id: row.get(0)?,
                        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                        collection_id: row.get(2)?,
                        favorite: row.get(3)?,
                    })
                },
            )
            .optional()
            .map(|automation| {
                automation.unwrap_or_else(|| LibrarySourceAutomation {
                    source_id: source_id.to_owned(),
                    ..LibrarySourceAutomation::default()
                })
            })
            .map_err(Into::into)
    }

    pub fn save_library_source_automation(
        &self,
        request: SaveLibrarySourceAutomationRequest,
    ) -> Result<LibrarySourceAutomation, AppError> {
        validate_uuid(&request.source_id, "library source")?;
        let source = self.get_library_source(&request.source_id)?;
        if source.private && self.is_library_privacy_locked() {
            return Err(library_locked_error());
        }
        self.save_library_source_automation_internal(request)
    }

    pub(crate) fn save_library_source_automation_internal(
        &self,
        request: SaveLibrarySourceAutomationRequest,
    ) -> Result<LibrarySourceAutomation, AppError> {
        validate_uuid(&request.source_id, "library source")?;
        self.get_library_source(&request.source_id)?;
        let source_id = request.source_id.clone();
        let tags = normalize_library_tags(request.tags)?;
        let collection_id = request.collection_id.filter(|id| !id.trim().is_empty());
        let now = Utc::now().timestamp_millis();
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Folder automation is unavailable.")
        })?;
        let transaction = connection.transaction()?;
        if let Some(collection_id) = collection_id.as_deref() {
            ensure_collection_exists(&transaction, collection_id)?;
        }
        if tags.is_empty() && collection_id.is_none() && !request.favorite {
            transaction.execute(
                "DELETE FROM library_source_automation WHERE source_id = ?1",
                [&request.source_id],
            )?;
        } else {
            let tags_json = serde_json::to_string(&tags).map_err(|error| {
                AppError::internal("could not serialize folder automation tags", error)
            })?;
            transaction.execute(
                "INSERT INTO library_source_automation
                    (source_id, tags_json, collection_id, favorite, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(source_id) DO UPDATE SET
                    tags_json = excluded.tags_json,
                    collection_id = excluded.collection_id,
                    favorite = excluded.favorite,
                    updated_at = excluded.updated_at",
                params![
                    request.source_id,
                    tags_json,
                    collection_id,
                    request.favorite,
                    now
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_library_source_automation_internal(&source_id)
    }

    pub fn apply_library_source_automation(
        &self,
        source_id: &str,
        item_id: &str,
    ) -> Result<(), AppError> {
        let automation = self.get_library_source_automation_internal(source_id)?;
        if automation.tags.is_empty() && automation.collection_id.is_none() && !automation.favorite
        {
            return Ok(());
        }
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Folder automation is unavailable.")
        })?;
        let transaction = connection.transaction()?;
        ensure_library_item_exists(&transaction, item_id)?;
        let now = Utc::now().timestamp_millis();
        if automation.favorite {
            transaction.execute(
                "UPDATE media_library SET favorite = 1, updated_at = ?2 WHERE id = ?1",
                params![item_id, now],
            )?;
        }
        if !automation.tags.is_empty() {
            let current: String = transaction.query_row(
                "SELECT tags_json FROM media_library WHERE id = ?1",
                [item_id],
                |row| row.get(0),
            )?;
            let mut tags: Vec<String> = serde_json::from_str(&current).unwrap_or_default();
            for tag in automation.tags {
                if !tags.iter().any(|value| value.eq_ignore_ascii_case(&tag)) {
                    tags.push(tag);
                }
            }
            tags.truncate(20);
            transaction.execute(
                "UPDATE media_library SET tags_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![
                    item_id,
                    serde_json::to_string(&tags).map_err(|error| AppError::internal(
                        "could not serialize media tags",
                        error
                    ))?,
                    now
                ],
            )?;
        }
        if let Some(collection_id) = automation.collection_id {
            ensure_collection_exists(&transaction, &collection_id)?;
            transaction.execute(
                "INSERT OR IGNORE INTO library_collection_items (collection_id, item_id, added_at)
                 VALUES (?1, ?2, ?3)",
                params![collection_id, item_id, now],
            )?;
            transaction.execute(
                "UPDATE library_collections SET updated_at = ?2 WHERE id = ?1",
                params![collection_id, now],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn remove_library_source(&self, id: &str) -> Result<(), AppError> {
        let mut connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library sources are unavailable.")
        })?;
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM media_library WHERE source_id = ?1", [id])?;
        if transaction.execute("DELETE FROM library_sources WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new(
                "library_source_not_found",
                "The library source was not found.",
            ));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn save_library_scan_job(&self, job: &LibraryScanJob) -> Result<(), AppError> {
        let warnings = serde_json::to_string(&job.warnings)
            .map_err(|error| AppError::internal("could not serialize scan warnings", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library scan state is unavailable.")
        })?;
        connection.execute(
            "INSERT INTO library_scan_jobs
                (id, source_id, state, discovered_count, indexed_count, skipped_count,
                 warning_count, current_path, warnings_json, started_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
                state = excluded.state, discovered_count = excluded.discovered_count,
                indexed_count = excluded.indexed_count, skipped_count = excluded.skipped_count,
                warning_count = excluded.warning_count, current_path = excluded.current_path,
                warnings_json = excluded.warnings_json, updated_at = excluded.updated_at,
                completed_at = excluded.completed_at",
            params![
                job.id,
                job.source_id,
                library_scan_state_name(job.state),
                job.discovered_count as i64,
                job.indexed_count as i64,
                job.skipped_count as i64,
                job.warning_count as i64,
                job.current_path,
                warnings,
                job.started_at.timestamp_millis(),
                job.updated_at.timestamp_millis(),
                job.completed_at.map(|value| value.timestamp_millis()),
            ],
        )?;
        Ok(())
    }

    pub fn list_library_scan_jobs(&self) -> Result<Vec<LibraryScanJob>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Library scan state is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, source_id, state, discovered_count, indexed_count, skipped_count,
                    warning_count, current_path, warnings_json, started_at, updated_at, completed_at
             FROM library_scan_jobs ORDER BY started_at DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], library_scan_job_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn update_library_source_scan_status(
        &self,
        id: &str,
        status: LibrarySourceStatus,
        error: Option<&str>,
        completed: bool,
    ) -> Result<(), AppError> {
        let now = Utc::now().timestamp_millis();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Library source status is unavailable.",
            )
        })?;
        connection.execute(
            "UPDATE library_sources SET status = ?2, last_error = ?3,
                last_scanned_at = CASE WHEN ?4 THEN ?5 ELSE last_scanned_at END,
                item_count = (SELECT COUNT(*) FROM media_library WHERE source_id = ?1 AND missing = 0),
                updated_at = ?5 WHERE id = ?1",
            params![id, library_source_status_name(status), error, completed, now],
        )?;
        Ok(())
    }

    pub fn list_library_items_for_source(
        &self,
        source_id: &str,
    ) -> Result<Vec<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             WHERE source_id = ?1",
        )?;
        let rows = statement.query_map([source_id], library_item_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn list_authentication_profiles(&self) -> Result<Vec<AuthenticationProfile>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Authentication profiles are unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, name, host_suffix, browser_cookie_source, browser_profile, proxy_url,
                    managed_cookie_path, enabled, created_at, updated_at
             FROM authentication_profiles ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], authentication_profile_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn save_authentication_profile(
        &self,
        request: SaveAuthenticationProfileRequest,
        managed_cookie_path: Option<String>,
    ) -> Result<AuthenticationProfile, AppError> {
        let id = match request.id {
            Some(id) => Uuid::parse_str(&id)
                .map_err(|_| {
                    AppError::new(
                        "invalid_auth_profile_id",
                        "The authentication profile ID is invalid.",
                    )
                })?
                .to_string(),
            None => Uuid::new_v4().to_string(),
        };
        let now = Utc::now();
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Authentication profiles are unavailable.",
            )
        })?;
        if request.enabled {
            let conflicting_profile: Option<String> = connection
                .query_row(
                    "SELECT name FROM authentication_profiles
                     WHERE enabled = 1 AND host_suffix = ?1 AND id != ?2 LIMIT 1",
                    params![request.host_suffix, id],
                    |row| row.get(0),
                )
                .optional()?;
            if let Some(name) = conflicting_profile {
                return Err(AppError::new(
                    "duplicate_auth_profile",
                    format!(
                        "An enabled authentication profile named ‘{name}’ already handles this hostname."
                    ),
                ));
            }
        }
        connection.execute(
            "INSERT INTO authentication_profiles
                (id, name, host_suffix, browser_cookie_source, browser_profile, proxy_url,
                 managed_cookie_path, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name, host_suffix = excluded.host_suffix,
                browser_cookie_source = excluded.browser_cookie_source,
                browser_profile = excluded.browser_profile, proxy_url = excluded.proxy_url,
                managed_cookie_path = excluded.managed_cookie_path, enabled = excluded.enabled,
                updated_at = excluded.updated_at",
            params![
                id,
                request.name,
                request.host_suffix,
                browser_source_name(request.browser_cookie_source),
                request.browser_profile,
                request.proxy_url,
                managed_cookie_path,
                request.enabled,
                now.timestamp_millis()
            ],
        )?;
        connection
            .query_row(
                "SELECT id, name, host_suffix, browser_cookie_source, browser_profile, proxy_url,
                    managed_cookie_path, enabled, created_at, updated_at
             FROM authentication_profiles WHERE id = ?1",
                [&id],
                authentication_profile_from_row,
            )
            .map_err(AppError::from)
    }

    pub fn authentication_profile(
        &self,
        id: &str,
    ) -> Result<Option<AuthenticationProfile>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Authentication profiles are unavailable.",
            )
        })?;
        connection
            .query_row(
                "SELECT id, name, host_suffix, browser_cookie_source, browser_profile, proxy_url,
                    managed_cookie_path, enabled, created_at, updated_at
             FROM authentication_profiles WHERE id = ?1",
                [id],
                authentication_profile_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn match_authentication_profile(
        &self,
        url: &str,
    ) -> Result<Option<AuthenticationProfile>, AppError> {
        let host = url::Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned));
        let Some(host) = host else {
            return Ok(None);
        };
        let profiles = self
            .list_authentication_profiles()?
            .into_iter()
            .filter(|profile| {
                profile.enabled
                    && (host == profile.host_suffix
                        || host.ends_with(&format!(".{}", profile.host_suffix)))
            })
            .collect::<Vec<_>>();
        let max_specificity = profiles
            .iter()
            .map(|profile| profile.host_suffix.len())
            .max();
        let mut matches = profiles
            .into_iter()
            .filter(|profile| Some(profile.host_suffix.len()) == max_specificity)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(AppError::new(
                "ambiguous_auth_profiles",
                "Multiple enabled authentication profiles match this hostname. Disable or edit one of them.",
            ));
        }
        Ok(matches.pop())
    }

    pub fn delete_authentication_profile(&self, id: &str) -> Result<Option<String>, AppError> {
        let profile = self.authentication_profile(id)?.ok_or_else(|| {
            AppError::new(
                "auth_profile_not_found",
                "The authentication profile was not found.",
            )
        })?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "Authentication profiles are unavailable.",
            )
        })?;
        connection.execute("DELETE FROM authentication_profiles WHERE id = ?1", [id])?;
        Ok(profile.managed_cookie_path)
    }
}

fn authentication_profile_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<AuthenticationProfile> {
    let source: String = row.get(3)?;
    let managed_cookie_path: Option<String> = row.get(6)?;
    Ok(AuthenticationProfile {
        id: row.get(0)?,
        name: row.get(1)?,
        host_suffix: row.get(2)?,
        browser_cookie_source: browser_source_from_name(&source),
        browser_profile: row.get(4)?,
        proxy_url: row.get(5)?,
        cookie_file_configured: managed_cookie_path.is_some(),
        managed_cookie_path,
        enabled: row.get(7)?,
        created_at: timestamp_or_epoch(row.get(8)?),
        updated_at: timestamp_or_epoch(row.get(9)?),
    })
}

fn browser_source_name(source: BrowserCookieSource) -> &'static str {
    match source {
        BrowserCookieSource::None => "none",
        BrowserCookieSource::Firefox => "firefox",
        BrowserCookieSource::Chromium => "chromium",
        BrowserCookieSource::Chrome => "chrome",
        BrowserCookieSource::Brave => "brave",
        BrowserCookieSource::Vivaldi => "vivaldi",
        BrowserCookieSource::Edge => "edge",
        BrowserCookieSource::Opera => "opera",
    }
}

fn browser_source_from_name(source: &str) -> BrowserCookieSource {
    match source {
        "firefox" => BrowserCookieSource::Firefox,
        "chromium" => BrowserCookieSource::Chromium,
        "chrome" => BrowserCookieSource::Chrome,
        "brave" => BrowserCookieSource::Brave,
        "vivaldi" => BrowserCookieSource::Vivaldi,
        "edge" => BrowserCookieSource::Edge,
        "opera" => BrowserCookieSource::Opera,
        _ => BrowserCookieSource::None,
    }
}

fn library_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryItem> {
    let tags_json: String = row.get(7)?;
    let metadata_json: String = row.get(8)?;
    let tags = serde_json::from_str(&tags_json).unwrap_or_default();
    let metadata = serde_json::from_str(&metadata_json).unwrap_or(serde_json::Value::Null);
    Ok(LibraryItem {
        id: row.get(0)?,
        path: row.get(1)?,
        source_id: row.get(12)?,
        relative_path: row.get(13)?,
        media_kind: row
            .get::<_, Option<String>>(14)?
            .as_deref()
            .and_then(library_media_kind_from_name),
        title: row.get(2)?,
        source_url: row.get(3)?,
        thumbnail: row.get(4)?,
        duration_seconds: row.get(5)?,
        site: row.get(6)?,
        tags,
        metadata,
        file_size_bytes: row
            .get::<_, Option<i64>>(15)?
            .and_then(|value| u64::try_from(value).ok()),
        modified_at_ms: row.get(16)?,
        width: row
            .get::<_, Option<i64>>(17)?
            .and_then(|value| u32::try_from(value).ok()),
        height: row
            .get::<_, Option<i64>>(18)?
            .and_then(|value| u32::try_from(value).ok()),
        fps: row.get(19)?,
        video_codec: row.get(20)?,
        audio_codec: row.get(21)?,
        container: row.get(22)?,
        dynamic_range: row.get(23)?,
        metadata_state: library_metadata_state_from_name(&row.get::<_, String>(24)?),
        metadata_error: row.get(25)?,
        favorite: row.get(26)?,
        rating: row
            .get::<_, Option<i64>>(27)?
            .and_then(|value| u8::try_from(value).ok()),
        missing: row.get(9)?,
        added_at: timestamp_or_epoch(row.get(10)?),
        updated_at: timestamp_or_epoch(row.get(11)?),
    })
}

fn library_playback_from_row(
    row: &rusqlite::Row<'_>,
    offset: usize,
) -> rusqlite::Result<LibraryPlaybackState> {
    Ok(LibraryPlaybackState {
        item_id: row.get(offset)?,
        position_seconds: row.get(offset + 1)?,
        duration_seconds: row.get(offset + 2)?,
        completed: row.get(offset + 3)?,
        updated_at: timestamp_or_epoch(row.get(offset + 4)?),
    })
}

fn library_video_fit_value(fit: crate::models::LibraryVideoFit) -> &'static str {
    match fit {
        crate::models::LibraryVideoFit::Fit => "fit",
        crate::models::LibraryVideoFit::Fill => "fill",
        crate::models::LibraryVideoFit::Original => "original",
        crate::models::LibraryVideoFit::Stretch => "stretch",
    }
}

fn validate_library_video_display_settings(
    settings: crate::models::LibraryVideoDisplaySettings,
) -> Result<(), AppError> {
    let values = [
        settings.zoom,
        settings.pan_x,
        settings.pan_y,
        settings.brightness,
        settings.contrast,
        settings.saturation,
        settings.gamma,
    ];
    let valid = values.iter().all(|value| value.is_finite())
        && (1.0..=3.0).contains(&settings.zoom)
        && (-1.0..=1.0).contains(&settings.pan_x)
        && (-1.0..=1.0).contains(&settings.pan_y)
        && [0, 90, 180, 270].contains(&settings.rotation)
        && [
            settings.brightness,
            settings.contrast,
            settings.saturation,
            settings.gamma,
        ]
        .iter()
        .all(|value| (-100.0..=100.0).contains(value));
    if valid {
        Ok(())
    } else {
        Err(AppError::new(
            "invalid_library_video_display",
            "The video display adjustment is outside the supported range.",
        ))
    }
}

fn library_video_display_settings_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<crate::models::LibraryVideoDisplaySettings> {
    let fit_value: String = row.get(0)?;
    let fit = match fit_value.as_str() {
        "fit" => crate::models::LibraryVideoFit::Fit,
        "fill" => crate::models::LibraryVideoFit::Fill,
        "original" => crate::models::LibraryVideoFit::Original,
        "stretch" => crate::models::LibraryVideoFit::Stretch,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "invalid saved video fit",
                )),
            ));
        }
    };
    Ok(crate::models::LibraryVideoDisplaySettings {
        fit,
        zoom: row.get(1)?,
        pan_x: row.get(2)?,
        pan_y: row.get(3)?,
        rotation: row.get(4)?,
        flip_horizontal: row.get(5)?,
        flip_vertical: row.get(6)?,
        brightness: row.get(7)?,
        contrast: row.get(8)?,
        saturation: row.get(9)?,
        gamma: row.get(10)?,
    })
}

fn library_order_by(sort: LibrarySort) -> &'static str {
    match sort {
        LibrarySort::Recent => "added_at DESC, media_library.id DESC",
        LibrarySort::Oldest => "added_at ASC, media_library.id ASC",
        LibrarySort::TitleAsc => {
            "title COLLATE NOCASE ASC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::TitleDesc => {
            "title COLLATE NOCASE DESC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::DurationDesc => {
            "duration_seconds IS NULL ASC, duration_seconds DESC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::DurationAsc => {
            "duration_seconds IS NULL ASC, duration_seconds ASC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::SizeDesc => {
            "file_size_bytes IS NULL ASC, file_size_bytes DESC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::SizeAsc => {
            "file_size_bytes IS NULL ASC, file_size_bytes ASC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::FileDateDesc => {
            "modified_at_ms IS NULL ASC, modified_at_ms DESC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::FileDateAsc => {
            "modified_at_ms IS NULL ASC, modified_at_ms ASC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::PublishedDesc => {
            "COALESCE(NULLIF(json_extract(rich.overrides_json, '$.uploadDate'), ''), json_extract(rich.extracted_json, '$.uploadDate')) IS NULL ASC, COALESCE(NULLIF(json_extract(rich.overrides_json, '$.uploadDate'), ''), json_extract(rich.extracted_json, '$.uploadDate')) DESC, added_at DESC, media_library.id DESC"
        }
        LibrarySort::PublishedAsc => {
            "COALESCE(NULLIF(json_extract(rich.overrides_json, '$.uploadDate'), ''), json_extract(rich.extracted_json, '$.uploadDate')) IS NULL ASC, COALESCE(NULLIF(json_extract(rich.overrides_json, '$.uploadDate'), ''), json_extract(rich.extracted_json, '$.uploadDate')) ASC, added_at DESC, media_library.id DESC"
        }
    }
}

fn build_library_query(request: &LibraryQueryRequest) -> Result<(String, Vec<SqlValue>), AppError> {
    let mut clauses = Vec::<String>::new();
    let mut parameters = Vec::<SqlValue>::new();

    let mut bind = |value: SqlValue| {
        parameters.push(value);
        format!("?{}", parameters.len())
    };

    if let Some(search) = request.search.as_deref() {
        let search = search.trim();
        if search.chars().count() > 200 || search.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_library_search",
                "Library searches must be at most 200 characters.",
            ));
        }
        if !search.is_empty() {
            let query = library_fts_query(search)?;
            let placeholder = bind(SqlValue::Text(query));
            clauses.push(format!(
                "media_library.id IN (
                    SELECT item_id FROM library_search WHERE library_search MATCH {placeholder}
                 )"
            ));
        }
    }

    if let Some(source_id) = request.source_id.as_deref() {
        validate_uuid(source_id, "library source")?;
        let placeholder = bind(SqlValue::Text(source_id.to_owned()));
        clauses.push(format!("media_library.source_id = {placeholder}"));
    }

    match (
        request.folder_source_id.as_deref(),
        request.folder_path.as_deref(),
    ) {
        (Some(source_id), Some(folder_path)) => {
            validate_uuid(source_id, "library folder source")?;
            let folder_path = normalize_library_folder_path(folder_path)?;
            let source_placeholder = bind(SqlValue::Text(source_id.to_owned()));
            clauses.push(format!("media_library.source_id = {source_placeholder}"));
            let relative_path = "REPLACE(COALESCE(media_library.relative_path, ''), '\\', '/')";
            if folder_path.is_empty() {
                if request.folder_scope == LibraryFolderScope::Direct {
                    clauses.push(format!("INSTR({relative_path}, '/') = 0"));
                }
            } else {
                let prefix = format!("{}/%", escape_sql_like(&folder_path));
                let path_placeholder = bind(SqlValue::Text(prefix));
                clauses.push(format!(
                    "{relative_path} LIKE {path_placeholder} ESCAPE '\\'"
                ));
                if request.folder_scope == LibraryFolderScope::Direct {
                    let prefix_length = folder_path.chars().count() + 2;
                    clauses.push(format!(
                        "INSTR(SUBSTR({relative_path}, {prefix_length}), '/') = 0"
                    ));
                }
            }
        }
        (None, None) => {}
        _ => {
            return Err(AppError::new(
                "invalid_library_folder",
                "Choose a library folder and its source together.",
            ));
        }
    }

    if let Some(media_kind) = request.media_kind {
        if media_kind == LibraryMediaKind::Both {
            return Err(AppError::new(
                "invalid_library_media_kind",
                "Choose video, image, or all media.",
            ));
        }
        let placeholder = bind(SqlValue::Text(
            library_media_kind_name(media_kind).to_owned(),
        ));
        clauses.push(format!("media_library.media_kind = {placeholder}"));
    }

    if let Some(resolution) = request.resolution.as_deref() {
        clauses.push(match resolution {
            "2160" => "height >= 2160".to_owned(),
            "1440" => "height >= 1440 AND height < 2160".to_owned(),
            "1080" => "height >= 1080 AND height < 1440".to_owned(),
            "720" => "height >= 720 AND height < 1080".to_owned(),
            "sd" => "height < 720".to_owned(),
            "unknown" => "height IS NULL".to_owned(),
            _ => return Err(invalid_library_filter("resolution")),
        });
    }

    if let Some(container) = request.container.as_deref() {
        let container = validate_library_filter_text(container, "container", 40)?;
        let placeholder = bind(SqlValue::Text(container));
        clauses.push(format!("container = {placeholder} COLLATE NOCASE"));
    }

    if let Some(duration) = request.duration.as_deref() {
        clauses.push(match duration {
            "short" => "duration_seconds < 300".to_owned(),
            "medium" => "duration_seconds >= 300 AND duration_seconds <= 1800".to_owned(),
            "long" => "duration_seconds > 1800".to_owned(),
            "unknown" => "duration_seconds IS NULL".to_owned(),
            _ => return Err(invalid_library_filter("duration")),
        });
    }

    if let Some(date_added) = request.date_added.as_deref() {
        let days = match date_added {
            "week" => 7,
            "month" => 31,
            "year" => 366,
            _ => return Err(invalid_library_filter("date")),
        };
        let cutoff = Utc::now()
            .checked_sub_signed(Duration::days(days))
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
            .timestamp_millis();
        let placeholder = bind(SqlValue::Integer(cutoff));
        clauses.push(format!("added_at >= {placeholder}"));
    }

    if let Some(date_published) = request.date_published.as_deref() {
        let days = match date_published {
            "week" => 7,
            "month" => 31,
            "year" => 366,
            _ => return Err(invalid_library_filter("published date")),
        };
        let cutoff = Utc::now()
            .checked_sub_signed(Duration::days(days))
            .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
            .date_naive()
            .format("%Y-%m-%d")
            .to_string();
        let placeholder = bind(SqlValue::Text(cutoff));
        clauses.push(format!(
            "COALESCE(
                NULLIF(json_extract(rich.overrides_json, '$.uploadDate'), ''),
                json_extract(rich.extracted_json, '$.uploadDate')
             ) >= {placeholder}"
        ));
    }

    match request.availability {
        LibraryAvailabilityFilter::All => {}
        LibraryAvailabilityFilter::Available => clauses.push("missing = 0".to_owned()),
        LibraryAvailabilityFilter::Missing => clauses.push("missing = 1".to_owned()),
    }

    let tags = validate_library_filter_values(&request.tags, "tag", 40)?;
    if !tags.is_empty() {
        let matches = tags
            .into_iter()
            .map(|tag| {
                let placeholder = bind(SqlValue::Text(tag));
                format!("EXISTS (SELECT 1 FROM json_each(media_library.tags_json) WHERE json_each.value = {placeholder} COLLATE NOCASE)")
            })
            .collect::<Vec<_>>();
        clauses.push(format!("({})", matches.join(" OR ")));
    }
    let creators = validate_library_filter_values(&request.creators, "creator", 200)?;
    if !creators.is_empty() {
        let matches = creators
            .into_iter()
            .map(|creator| {
                let placeholder = bind(SqlValue::Text(creator));
                format!(
                    "COALESCE(
                        NULLIF(json_extract(rich.overrides_json, '$.creator'), ''),
                        json_extract(rich.extracted_json, '$.creator')
                     ) = {placeholder} COLLATE NOCASE"
                )
            })
            .collect::<Vec<_>>();
        clauses.push(format!("({})", matches.join(" OR ")));
    }

    let series = validate_library_filter_values(&request.series, "series", 200)?;
    if !series.is_empty() {
        let matches = series
            .into_iter()
            .map(|series| {
                let placeholder = bind(SqlValue::Text(series));
                format!(
                    "COALESCE(
                        NULLIF(json_extract(rich.overrides_json, '$.series'), ''),
                        json_extract(rich.extracted_json, '$.series')
                     ) = {placeholder} COLLATE NOCASE"
                )
            })
            .collect::<Vec<_>>();
        clauses.push(format!("({})", matches.join(" OR ")));
    }

    let sites = validate_library_filter_values(&request.sites, "site", 200)?;
    if !sites.is_empty() {
        let matches = sites
            .into_iter()
            .map(|site| {
                let placeholder = bind(SqlValue::Text(site));
                format!("media_library.site = {placeholder} COLLATE NOCASE")
            })
            .collect::<Vec<_>>();
        clauses.push(format!("({})", matches.join(" OR ")));
    }

    if let Some(theme) = request.theme.as_deref() {
        let theme = validate_library_filter_text(theme, "theme", 80)?;
        let placeholder = bind(SqlValue::Text(theme));
        clauses.push(format!(
            "EXISTS (
                SELECT 1 FROM library_theme_assignments assignment
                JOIN library_theme_analyses analysis ON analysis.item_id = assignment.item_id
                LEFT JOIN library_media_metadata theme_metadata ON theme_metadata.item_id = assignment.item_id
                WHERE assignment.item_id = media_library.id
                  AND assignment.theme_key = {placeholder} COLLATE NOCASE
                  AND analysis.file_size_bytes IS theme_metadata.file_size_bytes
                  AND analysis.modified_at_ms IS theme_metadata.modified_at_ms
                  AND analysis.model = COALESCE(
                      (SELECT json_extract(value_json, '$.model') FROM settings WHERE key = 'library.theme.settings'),
                      'qwen2.5vl:7b'
                  )
                  AND analysis.mode = COALESCE(
                      (SELECT json_extract(value_json, '$.mode') FROM settings WHERE key = 'library.theme.settings'),
                      'deep'
                  )
            )"
        ));
    }

    if let Some(rating) = request.rating.as_deref() {
        clauses.push(match rating {
            "rated" => "rating IS NOT NULL".to_owned(),
            "unrated" => "rating IS NULL".to_owned(),
            "1" | "2" | "3" | "4" | "5" => {
                let value = rating
                    .parse::<i64>()
                    .map_err(|_| invalid_library_filter("rating"))?;
                let placeholder = bind(SqlValue::Integer(value));
                format!("rating = {placeholder}")
            }
            _ => return Err(invalid_library_filter("rating")),
        });
    }

    if let Some(collection_id) = request.collection_id.as_deref() {
        validate_uuid(collection_id, "library collection")?;
        let placeholder = bind(SqlValue::Text(collection_id.to_owned()));
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM library_collection_items WHERE item_id = media_library.id AND collection_id = {placeholder})"
        ));
    }

    if request.favorite_only {
        clauses.push("favorite = 1".to_owned());
    }

    if request.private_only {
        clauses.push(
            "(
                EXISTS (
                    SELECT 1 FROM library_sources private_source
                    WHERE private_source.id = media_library.source_id
                      AND private_source.private = 1
                )
                OR EXISTS (
                    SELECT 1 FROM library_private_items private_item
                    WHERE private_item.item_id = media_library.id
                )
                OR EXISTS (
                    SELECT 1 FROM library_private_folders private_folder
                    WHERE private_folder.source_id = media_library.source_id
                      AND (private_folder.relative_path = ''
                        OR REPLACE(COALESCE(media_library.relative_path, ''), '\\', '/') = private_folder.relative_path
                        OR REPLACE(COALESCE(media_library.relative_path, ''), '\\', '/') LIKE private_folder.relative_path || '/%')
                )
            )"
            .to_owned(),
        );
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    Ok((where_clause, parameters))
}

fn normalize_library_folder_path(value: &str) -> Result<String, AppError> {
    let normalized = value.replace('\\', "/");
    let normalized = normalized.trim_matches('/');
    if normalized.chars().count() > 1000 || normalized.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_folder",
            "The selected library folder is invalid.",
        ));
    }
    Ok(normalized.to_owned())
}

fn invalid_theme_analysis_scope() -> AppError {
    AppError::new(
        "invalid_theme_analysis_scope",
        "Choose all videos, one imported folder, or between 1 and 500 selected videos.",
    )
}

fn validate_library_theme_settings(settings: &LibraryThemeSettings) -> Result<(), AppError> {
    let model = settings.model.trim();
    if model.is_empty()
        || model.chars().count() > 100
        || !model
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".:_-/".contains(character))
    {
        return Err(AppError::new(
            "invalid_theme_model",
            "Choose a valid local Ollama model name.",
        ));
    }
    Ok(())
}

fn validate_theme_assignment(theme: &LibraryThemeAssignment) -> Result<(), AppError> {
    if theme.key.is_empty()
        || theme.key.chars().count() > 80
        || !theme.key.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
        || theme.label.trim().is_empty()
        || theme.label.chars().count() > 80
        || !theme.confidence.is_finite()
        || !(0.0..=1.0).contains(&theme.confidence)
        || theme.evidence.len() > 6
        || theme.evidence.iter().any(|evidence| {
            evidence.chars().count() > 240 || evidence.chars().any(char::is_control)
        })
    {
        return Err(AppError::new(
            "invalid_theme_analysis",
            "The local model returned an invalid theme assignment.",
        ));
    }
    Ok(())
}

fn library_theme_mode_name(mode: LibraryThemeAnalysisMode) -> &'static str {
    match mode {
        LibraryThemeAnalysisMode::Fast => "fast",
        LibraryThemeAnalysisMode::Deep => "deep",
    }
}

fn library_theme_mode_from_name(value: &str) -> LibraryThemeAnalysisMode {
    match value {
        "fast" => LibraryThemeAnalysisMode::Fast,
        _ => LibraryThemeAnalysisMode::Deep,
    }
}

fn validate_library_filter_text(
    value: &str,
    label: &str,
    max_length: usize,
) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max_length || value.chars().any(char::is_control)
    {
        return Err(invalid_library_filter(label));
    }
    Ok(value.to_owned())
}

fn validate_library_filter_values(
    values: &[String],
    label: &str,
    max_length: usize,
) -> Result<Vec<String>, AppError> {
    if values.len() > 20 {
        return Err(invalid_library_filter(label));
    }
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = validate_library_filter_text(value, label, max_length)?;
        if !normalized
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(&value))
        {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn library_fts_query(search: &str) -> Result<String, AppError> {
    let tokens = search
        .split_whitespace()
        .filter(|token| token.chars().any(char::is_alphanumeric))
        .take(20)
        .map(|token| format!("\"{}\"*", token.replace('"', "\"\"")))
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return Err(AppError::new(
            "invalid_library_search",
            "Enter at least one letter or number to search the Library.",
        ));
    }
    Ok(tokens.join(" AND "))
}

fn invalid_library_filter(label: &str) -> AppError {
    AppError::new(
        "invalid_library_filter",
        format!("The selected library {label} filter is invalid."),
    )
}

fn validate_smart_view_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_smart_view_name",
            "Use a saved-view name between 1 and 80 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn normalize_smart_view_query(
    mut query: LibrarySmartViewQuery,
) -> Result<LibrarySmartViewQuery, AppError> {
    query.search = match query.search.take() {
        Some(search) => {
            let search = search.trim();
            if search.is_empty() {
                None
            } else {
                if search.chars().count() > 200 || search.chars().any(char::is_control) {
                    return Err(AppError::new(
                        "invalid_library_search",
                        "Library search text must be 200 characters or fewer.",
                    ));
                }
                Some(search.to_owned())
            }
        }
        None => None,
    };
    query.source_id = normalize_smart_view_uuid(query.source_id, "library source")?;
    query.folder_source_id =
        normalize_smart_view_uuid(query.folder_source_id, "library folder source")?;
    query.collection_id = normalize_smart_view_uuid(query.collection_id, "collection")?;
    query.folder_path = query
        .folder_path
        .take()
        .map(|path| normalize_library_folder_path(&path))
        .transpose()?;
    query.container = query
        .container
        .take()
        .map(|container| validate_library_filter_text(&container, "container", 40))
        .transpose()?;
    query.tags = validate_library_filter_values(&query.tags, "tag", 40)?;
    query.creators = validate_library_filter_values(&query.creators, "creator", 200)?;
    query.series = validate_library_filter_values(&query.series, "series", 200)?;
    query.sites = validate_library_filter_values(&query.sites, "site", 200)?;
    query.theme = query
        .theme
        .take()
        .map(|theme| validate_library_filter_text(&theme, "theme", 80))
        .transpose()?;
    build_library_query(&library_query_from_smart_view(&query, 1, 1))?;
    Ok(query)
}

fn normalize_smart_view_uuid(
    value: Option<String>,
    label: &str,
) -> Result<Option<String>, AppError> {
    value
        .map(|value| {
            Uuid::parse_str(&value)
                .map(|id| id.to_string())
                .map_err(|_| {
                    AppError::new(
                        "invalid_library_smart_view",
                        format!("The saved view contains an invalid {label} identifier."),
                    )
                })
        })
        .transpose()
}

fn library_query_from_smart_view(
    query: &LibrarySmartViewQuery,
    page: u32,
    page_size: u32,
) -> LibraryQueryRequest {
    LibraryQueryRequest {
        page,
        page_size,
        search: query.search.clone(),
        source_id: query.source_id.clone(),
        folder_source_id: query.folder_source_id.clone(),
        folder_path: query.folder_path.clone(),
        folder_scope: query.folder_scope,
        media_kind: query.media_kind,
        resolution: query.resolution.clone(),
        container: query.container.clone(),
        duration: query.duration.clone(),
        date_added: query.date_added.clone(),
        date_published: query.date_published.clone(),
        availability: query.availability,
        tags: query.tags.clone(),
        creators: query.creators.clone(),
        series: query.series.clone(),
        sites: query.sites.clone(),
        theme: query.theme.clone(),
        rating: query.rating.clone(),
        collection_id: query.collection_id.clone(),
        favorite_only: query.favorite_only,
        private_only: query.private_only,
        sort: query.sort,
    }
}

fn empty_library_query_request(page: u32, page_size: u32) -> LibraryQueryRequest {
    LibraryQueryRequest {
        page,
        page_size,
        search: None,
        source_id: None,
        folder_source_id: None,
        folder_path: None,
        folder_scope: LibraryFolderScope::Tree,
        media_kind: None,
        resolution: None,
        container: None,
        duration: None,
        date_added: None,
        date_published: None,
        availability: LibraryAvailabilityFilter::Available,
        tags: Vec::new(),
        creators: Vec::new(),
        series: Vec::new(),
        sites: Vec::new(),
        theme: None,
        rating: None,
        collection_id: None,
        favorite_only: false,
        private_only: false,
        sort: LibrarySort::Recent,
    }
}

fn escape_sql_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn validate_library_item_ids(ids: Vec<String>) -> Result<Vec<String>, AppError> {
    if ids.is_empty() || ids.len() > 500 {
        return Err(AppError::new(
            "invalid_library_selection",
            "Select between 1 and 500 library items.",
        ));
    }
    let mut unique = Vec::with_capacity(ids.len());
    let mut seen = HashSet::new();
    for id in ids {
        validate_uuid(&id, "library item")?;
        if seen.insert(id.clone()) {
            unique.push(id);
        }
    }
    Ok(unique)
}

fn validate_bounded_ids(ids: Vec<String>, label: &str) -> Result<Vec<String>, AppError> {
    if ids.is_empty() || ids.len() > 500 {
        return Err(AppError::new(
            "invalid_library_selection",
            format!("Select between 1 and 500 {label}."),
        ));
    }
    let mut unique = Vec::with_capacity(ids.len());
    let mut seen = HashSet::new();
    for id in ids {
        validate_uuid(&id, label)?;
        if seen.insert(id.clone()) {
            unique.push(id);
        }
    }
    Ok(unique)
}

fn sqlite_file_identity(identity: LibraryFileIdentity) -> Option<(i64, i64)> {
    Some((
        i64::try_from(identity.device_id).ok()?,
        i64::try_from(identity.file_id).ok()?,
    ))
}

fn validate_uuid(value: &str, label: &str) -> Result<(), AppError> {
    Uuid::parse_str(value).map_err(|_| {
        AppError::new(
            "invalid_identifier",
            format!("The {label} identifier is invalid."),
        )
    })?;
    Ok(())
}

fn validate_library_title(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 300 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_title",
            "Use a title between 1 and 300 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_collection_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_collection_name",
            "Use a collection name between 1 and 80 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_library_shelf_limit(limit: usize) -> Result<(), AppError> {
    if !(1..=24).contains(&limit) {
        return Err(AppError::new(
            "invalid_library_playback_limit",
            "Library media shelves can contain between 1 and 24 items.",
        ));
    }
    Ok(())
}

fn validate_library_rating(rating: Option<u8>) -> Result<(), AppError> {
    if rating.is_some_and(|value| !(1..=5).contains(&value)) {
        return Err(AppError::new(
            "invalid_library_rating",
            "Choose a rating between 1 and 5, or clear the rating.",
        ));
    }
    Ok(())
}

fn normalize_library_tags(tags: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut normalized = Vec::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.chars().count() > 40 || tag.chars().any(char::is_control) {
            return Err(AppError::new(
                "invalid_library_tag",
                "Tags must be at most 40 characters and cannot contain control characters.",
            ));
        }
        if !normalized
            .iter()
            .any(|value: &String| value.eq_ignore_ascii_case(tag))
        {
            normalized.push(tag.to_owned());
        }
    }
    normalized.truncate(20);
    Ok(normalized)
}

pub(crate) fn validate_library_metadata_fields(
    mut fields: LibraryMetadataFields,
) -> Result<LibraryMetadataFields, AppError> {
    fields.creator = normalize_metadata_text(fields.creator, "creator", 200)?;
    fields.description = normalize_metadata_description(fields.description)?;
    fields.series = normalize_metadata_text(fields.series, "series", 200)?;
    fields.episode = normalize_metadata_text(fields.episode, "episode", 200)?;
    fields.upload_date = fields
        .upload_date
        .map(|value| normalize_upload_date(&value))
        .transpose()?;
    fields.original_url = normalize_original_url(fields.original_url)?;
    Ok(fields)
}

fn normalize_metadata_text(
    value: Option<String>,
    label: &str,
    max_chars: usize,
) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > max_chars || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_metadata",
            format!("The {label} contains unsupported text or is too long."),
        ));
    }
    Ok(Some(value))
}

fn normalize_metadata_description(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().replace("\r\n", "\n").replace('\r', "\n");
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 10_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(AppError::new(
            "invalid_library_metadata",
            "The description contains unsupported text or is too long.",
        ));
    }
    Ok(Some(value))
}

fn normalize_upload_date(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    let normalized = if value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("{}-{}-{}", &value[..4], &value[4..6], &value[6..8])
    } else {
        value.to_owned()
    };
    chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d").map_err(|_| {
        AppError::new(
            "invalid_library_metadata",
            "The upload date must use YYYY-MM-DD.",
        )
    })?;
    Ok(normalized)
}

fn normalize_original_url(value: Option<String>) -> Result<Option<String>, AppError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Ok(None);
    }
    if value.chars().count() > 2_000 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_metadata",
            "The original URL must be a valid HTTP or HTTPS URL.",
        ));
    }
    let parsed = url::Url::parse(&value).map_err(|_| {
        AppError::new(
            "invalid_library_metadata",
            "The original URL must be a valid HTTP or HTTPS URL.",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(AppError::new(
            "invalid_library_metadata",
            "The original URL must be a valid HTTP or HTTPS URL.",
        ));
    }
    Ok(Some(value))
}

fn metadata_fields_from_value(value: &serde_json::Value) -> LibraryMetadataFields {
    let string = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    LibraryMetadataFields {
        creator: normalize_metadata_text(
            string(&["uploader", "channel", "creator"]),
            "creator",
            200,
        )
        .unwrap_or(None),
        description: normalize_metadata_description(string(&["description"])).unwrap_or(None),
        upload_date: string(&["uploadDate", "upload_date"])
            .and_then(|value| normalize_upload_date(&value).ok()),
        series: normalize_metadata_text(
            string(&["series", "playlistTitle", "playlist_title"]),
            "series",
            200,
        )
        .unwrap_or(None),
        episode: normalize_metadata_text(
            string(&["episode", "episodeNumber", "episode_number"]),
            "episode",
            200,
        )
        .unwrap_or(None),
        original_url: normalize_original_url(string(&[
            "webpageUrl",
            "webpage_url",
            "originalUrl",
            "original_url",
        ]))
        .unwrap_or(None),
    }
}

fn save_extracted_library_metadata(
    connection: &Connection,
    item_id: &str,
    metadata: &serde_json::Value,
) -> Result<(), AppError> {
    let extracted = validate_library_metadata_fields(metadata_fields_from_value(metadata))?;
    let extracted_json = serde_json::to_string(&extracted)
        .map_err(|error| AppError::internal("could not serialize extracted metadata", error))?;
    connection.execute(
        "INSERT INTO library_rich_metadata (item_id, extracted_json, overrides_json, updated_at)
         VALUES (?1, ?2, '{}', ?3)
         ON CONFLICT(item_id) DO UPDATE SET
            extracted_json = excluded.extracted_json,
            updated_at = excluded.updated_at",
        params![item_id, extracted_json, Utc::now().timestamp_millis()],
    )?;
    Ok(())
}

fn rich_metadata_from_json(
    item_id: &str,
    extracted_json: &str,
    overrides_json: &str,
    updated_at: i64,
) -> Result<LibraryRichMetadata, AppError> {
    let extracted = serde_json::from_str(extracted_json)
        .map_err(|error| AppError::internal("could not read extracted metadata", error))?;
    let overrides = serde_json::from_str(overrides_json)
        .map_err(|error| AppError::internal("could not read metadata overrides", error))?;
    rich_metadata_from_fields(item_id, extracted, overrides, updated_at)
}

fn rich_metadata_from_fields(
    item_id: &str,
    extracted: LibraryMetadataFields,
    overrides: LibraryMetadataFields,
    updated_at: i64,
) -> Result<LibraryRichMetadata, AppError> {
    let mut overridden_fields = Vec::new();
    let effective = LibraryMetadataFields {
        creator: metadata_override(
            "creator",
            &overrides.creator,
            &extracted.creator,
            &mut overridden_fields,
        ),
        description: metadata_override(
            "description",
            &overrides.description,
            &extracted.description,
            &mut overridden_fields,
        ),
        upload_date: metadata_override(
            "uploadDate",
            &overrides.upload_date,
            &extracted.upload_date,
            &mut overridden_fields,
        ),
        series: metadata_override(
            "series",
            &overrides.series,
            &extracted.series,
            &mut overridden_fields,
        ),
        episode: metadata_override(
            "episode",
            &overrides.episode,
            &extracted.episode,
            &mut overridden_fields,
        ),
        original_url: metadata_override(
            "originalUrl",
            &overrides.original_url,
            &extracted.original_url,
            &mut overridden_fields,
        ),
    };
    Ok(LibraryRichMetadata {
        item_id: item_id.to_owned(),
        extracted,
        overrides,
        effective,
        overridden_fields,
        updated_at: timestamp_or_epoch(updated_at),
    })
}

fn metadata_override(
    field: &str,
    override_value: &Option<String>,
    extracted_value: &Option<String>,
    overridden_fields: &mut Vec<String>,
) -> Option<String> {
    if override_value.is_some() {
        overridden_fields.push(field.to_owned());
        override_value.clone()
    } else {
        extracted_value.clone()
    }
}

fn clear_matching_metadata_overrides(
    overrides: &mut LibraryMetadataFields,
    extracted: &LibraryMetadataFields,
) {
    if overrides.creator == extracted.creator {
        overrides.creator = None;
    }
    if overrides.description == extracted.description {
        overrides.description = None;
    }
    if overrides.upload_date == extracted.upload_date {
        overrides.upload_date = None;
    }
    if overrides.series == extracted.series {
        overrides.series = None;
    }
    if overrides.episode == extracted.episode {
        overrides.episode = None;
    }
    if overrides.original_url == extracted.original_url {
        overrides.original_url = None;
    }
}

fn validate_library_artwork_path(value: &str) -> Result<String, AppError> {
    let path = Path::new(value);
    if !path.is_absolute()
        || std::fs::symlink_metadata(path)
            .map(|metadata| !metadata.file_type().is_file() || metadata.file_type().is_symlink())
            .unwrap_or(true)
        || !matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "webp")
        )
    {
        return Err(AppError::new(
            "invalid_library_artwork",
            "Choose a regular JPEG, PNG, or WebP image file.",
        ));
    }
    path.canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| AppError::internal("could not validate the artwork file", error))
}

fn library_item_not_found() -> AppError {
    AppError::new(
        "library_item_not_found",
        "A selected library item was not found.",
    )
}

fn ensure_library_item_updated(changed: usize) -> Result<(), AppError> {
    if changed == 0 {
        Err(library_item_not_found())
    } else {
        Ok(())
    }
}

fn ensure_library_item_exists(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
) -> Result<(), AppError> {
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM media_library WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(library_item_not_found())
    }
}

fn ensure_collection_exists(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
) -> Result<(), AppError> {
    validate_uuid(id, "collection")?;
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM library_collections WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::new(
            "library_collection_not_found",
            "The library collection was not found.",
        ))
    }
}

fn ensure_playlist_exists(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
) -> Result<(), AppError> {
    validate_uuid(id, "playlist")?;
    let exists: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM library_playlists WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(AppError::new(
            "library_playlist_not_found",
            "The library playlist was not found.",
        ))
    }
}

fn ensure_library_video_exists(
    transaction: &rusqlite::Transaction<'_>,
    id: &str,
) -> Result<(), AppError> {
    let media_kind: Option<String> = transaction
        .query_row(
            "SELECT media_kind FROM media_library WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    match media_kind.as_deref() {
        Some("video") => Ok(()),
        Some(_) => Err(AppError::new(
            "library_playlist_video_required",
            "Only videos can be added to a playlist.",
        )),
        None => Err(library_item_not_found()),
    }
}

fn playlist_item_ids(
    transaction: &rusqlite::Transaction<'_>,
    playlist_id: &str,
) -> Result<Vec<String>, AppError> {
    let mut statement = transaction.prepare(
        "SELECT item_id FROM library_playlist_items
         WHERE playlist_id = ?1 ORDER BY position ASC, added_at ASC, item_id ASC",
    )?;
    let item_ids = statement
        .query_map([playlist_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(AppError::from)?;
    Ok(item_ids)
}

fn normalize_playlist_positions(
    transaction: &rusqlite::Transaction<'_>,
    playlist_id: &str,
) -> Result<(), AppError> {
    for (position, id) in playlist_item_ids(transaction, playlist_id)?
        .iter()
        .enumerate()
    {
        transaction.execute(
            "UPDATE library_playlist_items SET position = ?3
             WHERE playlist_id = ?1 AND item_id = ?2",
            params![playlist_id, id, i64::try_from(position).unwrap_or(i64::MAX)],
        )?;
    }
    Ok(())
}

fn validate_playlist_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_playlist_name",
            "Playlist names must contain between 1 and 80 printable characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_playlist_description(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 500
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(AppError::new(
            "invalid_library_playlist_description",
            "Playlist descriptions must contain no more than 500 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_moment_label(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_moment_label",
            "Moment labels must contain between 1 and 100 visible characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_compare_session_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_compare_session_name",
            "Comparison session names must contain between 1 and 80 visible characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_compare_session_notes(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 2_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(AppError::new(
            "invalid_compare_session_notes",
            "Comparison notes must contain no more than 2,000 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_media_stack_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_media_stack_name",
            "Media stack names must contain between 1 and 80 visible characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_media_stack_notes(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 2_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(AppError::new(
            "invalid_media_stack_notes",
            "Media stack notes must contain no more than 2,000 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn parse_media_stack_role(value: &str) -> LibraryMediaStackRole {
    match value {
        "original" => LibraryMediaStackRole::Original,
        "alternate" => LibraryMediaStackRole::Alternate,
        "edit" => LibraryMediaStackRole::Edit,
        "generated" => LibraryMediaStackRole::Generated,
        _ => LibraryMediaStackRole::Other,
    }
}

fn parse_curation_decision(value: &str) -> Option<LibraryCurationDecision> {
    match value {
        "keep" => Some(LibraryCurationDecision::Keep),
        "reviewLater" => Some(LibraryCurationDecision::ReviewLater),
        _ => None,
    }
}

fn validate_moment_note(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 2_000
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(AppError::new(
            "invalid_library_moment_note",
            "Moment notes must contain no more than 2,000 characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_moment_color(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if !matches!(value, "accent" | "blue" | "amber" | "green" | "rose") {
        return Err(AppError::new(
            "invalid_library_moment_color",
            "Choose a supported moment color.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_moment_times(start_seconds: f64, end_seconds: Option<f64>) -> Result<(), AppError> {
    if !start_seconds.is_finite() || start_seconds < 0.0 {
        return Err(AppError::new(
            "invalid_library_moment_time",
            "The moment start must be a valid non-negative time.",
        ));
    }
    if let Some(end_seconds) = end_seconds {
        if !end_seconds.is_finite() || end_seconds <= start_seconds {
            return Err(AppError::new(
                "invalid_library_moment_time",
                "The moment end must be later than its start.",
            ));
        }
    }
    Ok(())
}

fn validate_montage_name(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_montage_name",
            "Montage names must contain 1 to 100 visible characters.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_montage_notes(value: &str) -> Result<String, AppError> {
    let value = value.trim();
    if value.chars().count() > 2_000
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t')
    {
        return Err(AppError::new(
            "invalid_library_montage_notes",
            "Montage notes must be 2,000 characters or fewer.",
        ));
    }
    Ok(value.to_owned())
}

fn validate_montage_fade(value: f64) -> Result<(), AppError> {
    if !value.is_finite() || !(0.0..=5.0).contains(&value) {
        return Err(AppError::new(
            "invalid_library_montage_fade",
            "Montage fades must be between zero and five seconds.",
        ));
    }
    Ok(())
}

fn validate_montage_segments(segments: &[LibraryMontageSegment]) -> Result<(), AppError> {
    if segments.is_empty() || segments.len() > 50 {
        return Err(AppError::new(
            "invalid_library_montage_segments",
            "A montage must contain between 1 and 50 segments.",
        ));
    }
    for segment in segments {
        let label = segment.label.trim();
        if label.is_empty()
            || label.chars().count() > 100
            || label.chars().any(char::is_control)
            || !segment.start_seconds.is_finite()
            || !segment.end_seconds.is_finite()
            || segment.start_seconds < 0.0
            || segment.end_seconds - segment.start_seconds < 0.1
        {
            return Err(AppError::new(
                "invalid_library_montage_segment",
                "Each segment needs a label and a valid clip range of at least 0.1 seconds.",
            ));
        }
    }
    Ok(())
}

fn library_moment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryMoment> {
    Ok(LibraryMoment {
        id: row.get(0)?,
        item_id: row.get(1)?,
        label: row.get(2)?,
        note: row.get(3)?,
        start_seconds: row.get(4)?,
        end_seconds: row.get(5)?,
        color: row.get(6)?,
        created_at: timestamp_or_epoch(row.get(7)?),
        updated_at: timestamp_or_epoch(row.get(8)?),
    })
}

fn save_library_metadata(connection: &Connection, item: &LibraryItem) -> Result<(), AppError> {
    connection.execute(
        "INSERT INTO library_media_metadata
            (item_id, file_size_bytes, modified_at_ms, width, height, fps, video_codec,
             audio_codec, container, dynamic_range, state, error, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
         ON CONFLICT(item_id) DO UPDATE SET
            file_size_bytes = excluded.file_size_bytes, modified_at_ms = excluded.modified_at_ms,
            width = excluded.width, height = excluded.height, fps = excluded.fps,
            video_codec = excluded.video_codec, audio_codec = excluded.audio_codec,
            container = excluded.container, dynamic_range = excluded.dynamic_range,
            state = excluded.state, error = excluded.error, updated_at = excluded.updated_at",
        params![
            item.id,
            item.file_size_bytes
                .and_then(|value| i64::try_from(value).ok()),
            item.modified_at_ms,
            item.width,
            item.height,
            item.fps,
            item.video_codec,
            item.audio_codec,
            item.container,
            item.dynamic_range,
            library_metadata_state_name(item.metadata_state),
            item.metadata_error,
            Utc::now().timestamp_millis(),
        ],
    )?;
    Ok(())
}

fn library_metadata_state_name(state: LibraryMetadataState) -> &'static str {
    match state {
        LibraryMetadataState::Pending => "pending",
        LibraryMetadataState::Ready => "ready",
        LibraryMetadataState::Failed => "failed",
        LibraryMetadataState::Unavailable => "unavailable",
    }
}

fn library_metadata_state_from_name(value: &str) -> LibraryMetadataState {
    match value {
        "ready" => LibraryMetadataState::Ready,
        "failed" => LibraryMetadataState::Failed,
        "unavailable" => LibraryMetadataState::Unavailable,
        _ => LibraryMetadataState::Pending,
    }
}

fn library_change_kind_name(kind: LibraryChangeKind) -> &'static str {
    match kind {
        LibraryChangeKind::Added => "added",
        LibraryChangeKind::Modified => "modified",
        LibraryChangeKind::Renamed => "renamed",
        LibraryChangeKind::Missing => "missing",
        LibraryChangeKind::Restored => "restored",
    }
}

fn library_change_kind_from_name(value: &str) -> LibraryChangeKind {
    match value {
        "modified" => LibraryChangeKind::Modified,
        "renamed" => LibraryChangeKind::Renamed,
        "missing" => LibraryChangeKind::Missing,
        "restored" => LibraryChangeKind::Restored,
        _ => LibraryChangeKind::Added,
    }
}

fn library_change_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryChange> {
    Ok(LibraryChange {
        id: row.get(0)?,
        item_id: row.get(1)?,
        source_id: row.get(2)?,
        kind: library_change_kind_from_name(&row.get::<_, String>(3)?),
        previous_path: row.get(4)?,
        detected_at: timestamp_or_epoch(row.get(5)?),
        reviewed_at: row.get::<_, Option<i64>>(6)?.map(timestamp_or_epoch),
    })
}

fn library_file_operation_kind_name(kind: LibraryFileOperationKind) -> &'static str {
    match kind {
        LibraryFileOperationKind::Move => "move",
        LibraryFileOperationKind::Copy => "copy",
        LibraryFileOperationKind::Rename => "rename",
    }
}

fn library_file_operation_kind_from_name(value: &str) -> LibraryFileOperationKind {
    match value {
        "copy" => LibraryFileOperationKind::Copy,
        "rename" => LibraryFileOperationKind::Rename,
        _ => LibraryFileOperationKind::Move,
    }
}

fn library_file_operation_state_name(state: LibraryFileOperationState) -> &'static str {
    match state {
        LibraryFileOperationState::Planned => "planned",
        LibraryFileOperationState::Executing => "executing",
        LibraryFileOperationState::Completed => "completed",
        LibraryFileOperationState::Failed => "failed",
        LibraryFileOperationState::Undone => "undone",
    }
}

fn library_file_operation_state_from_name(value: &str) -> LibraryFileOperationState {
    match value {
        "executing" => LibraryFileOperationState::Executing,
        "completed" => LibraryFileOperationState::Completed,
        "failed" => LibraryFileOperationState::Failed,
        "undone" => LibraryFileOperationState::Undone,
        _ => LibraryFileOperationState::Planned,
    }
}

fn library_file_operation_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<LibraryFileOperation> {
    Ok(LibraryFileOperation {
        id: row.get(0)?,
        item_id: row.get(1)?,
        output_item_id: row.get(2)?,
        kind: library_file_operation_kind_from_name(&row.get::<_, String>(3)?),
        state: library_file_operation_state_from_name(&row.get::<_, String>(4)?),
        source_path: row.get(5)?,
        destination_path: row.get(6)?,
        source_size_bytes: row.get::<_, i64>(7)?.max(0) as u64,
        source_modified_at_ms: row.get(8)?,
        error: row.get(9)?,
        created_at: timestamp_or_epoch(row.get(10)?),
        updated_at: timestamp_or_epoch(row.get(11)?),
        completed_at: row.get::<_, Option<i64>>(12)?.map(timestamp_or_epoch),
    })
}

fn library_source_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibrarySource> {
    let includes: String = row.get(7)?;
    let excludes: String = row.get(8)?;
    let last_scanned_at = row.get::<_, Option<i64>>(11)?.map(timestamp_or_epoch);
    Ok(LibrarySource {
        id: row.get(0)?,
        name: row.get(1)?,
        path: row.get(2)?,
        media_kind: library_media_kind_from_name(&row.get::<_, String>(3)?)
            .unwrap_or(LibraryMediaKind::Both),
        recursive: row.get(4)?,
        watch_enabled: row.get(5)?,
        private: row.get(6)?,
        include_patterns: serde_json::from_str(&includes).unwrap_or_default(),
        exclude_patterns: serde_json::from_str(&excludes).unwrap_or_default(),
        status: library_source_status_from_name(&row.get::<_, String>(9)?),
        last_error: row.get(10)?,
        last_scanned_at,
        item_count: row.get::<_, i64>(12)?.max(0) as usize,
        created_at: timestamp_or_epoch(row.get(13)?),
        updated_at: timestamp_or_epoch(row.get(14)?),
    })
}

fn library_scan_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LibraryScanJob> {
    let warnings: String = row.get(8)?;
    Ok(LibraryScanJob {
        id: row.get(0)?,
        source_id: row.get(1)?,
        state: library_scan_state_from_name(&row.get::<_, String>(2)?),
        discovered_count: row.get::<_, i64>(3)?.max(0) as usize,
        indexed_count: row.get::<_, i64>(4)?.max(0) as usize,
        skipped_count: row.get::<_, i64>(5)?.max(0) as usize,
        warning_count: row.get::<_, i64>(6)?.max(0) as usize,
        current_path: row.get(7)?,
        warnings: serde_json::from_str(&warnings).unwrap_or_default(),
        started_at: timestamp_or_epoch(row.get(9)?),
        updated_at: timestamp_or_epoch(row.get(10)?),
        completed_at: row.get::<_, Option<i64>>(11)?.map(timestamp_or_epoch),
    })
}

fn library_media_kind_name(kind: LibraryMediaKind) -> &'static str {
    match kind {
        LibraryMediaKind::Video => "video",
        LibraryMediaKind::Image => "image",
        LibraryMediaKind::Both => "both",
    }
}

fn library_media_kind_from_name(value: &str) -> Option<LibraryMediaKind> {
    match value {
        "video" => Some(LibraryMediaKind::Video),
        "image" => Some(LibraryMediaKind::Image),
        "both" => Some(LibraryMediaKind::Both),
        _ => None,
    }
}

fn library_media_kind_for_path(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    if matches!(
        extension.as_str(),
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v"
    ) {
        Some("video")
    } else if matches!(
        extension.as_str(),
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "avif"
    ) {
        Some("image")
    } else {
        None
    }
}

fn library_source_status_name(status: LibrarySourceStatus) -> &'static str {
    match status {
        LibrarySourceStatus::Online => "online",
        LibrarySourceStatus::Offline => "offline",
        LibrarySourceStatus::Scanning => "scanning",
        LibrarySourceStatus::Error => "error",
    }
}

fn library_source_status_from_name(value: &str) -> LibrarySourceStatus {
    match value {
        "offline" => LibrarySourceStatus::Offline,
        "scanning" => LibrarySourceStatus::Scanning,
        "error" => LibrarySourceStatus::Error,
        _ => LibrarySourceStatus::Online,
    }
}

fn library_scan_state_name(state: LibraryScanState) -> &'static str {
    match state {
        LibraryScanState::Running => "running",
        LibraryScanState::Completed => "completed",
        LibraryScanState::Cancelled => "cancelled",
        LibraryScanState::Failed => "failed",
    }
}

fn library_scan_state_from_name(value: &str) -> LibraryScanState {
    match value {
        "completed" => LibraryScanState::Completed,
        "cancelled" => LibraryScanState::Cancelled,
        "failed" => LibraryScanState::Failed,
        _ => LibraryScanState::Running,
    }
}

fn transformation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TransformationTask> {
    let operation_json: String = row.get(3)?;
    let operation: TransformationOperation =
        serde_json::from_str(&operation_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                operation_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let state: String = row.get(4)?;
    Ok(TransformationTask {
        id: row.get(0)?,
        input_path: row.get(1)?,
        output_path: row.get(2)?,
        operation,
        state: match state.as_str() {
            "Queued" => TransformationState::Queued,
            "Running" => TransformationState::Running,
            "Completed" => TransformationState::Completed,
            "Cancelled" => TransformationState::Cancelled,
            _ => TransformationState::Failed,
        },
        progress_percent: row.get(5)?,
        current_operation: row.get(6)?,
        error: row.get(7)?,
        created_at: timestamp_or_epoch(row.get(8)?),
        updated_at: timestamp_or_epoch(row.get(9)?),
        completed_at: row.get::<_, Option<i64>>(10)?.map(timestamp_or_epoch),
    })
}

fn transformation_state_name(state: TransformationState) -> &'static str {
    match state {
        TransformationState::Queued => "Queued",
        TransformationState::Running => "Running",
        TransformationState::Completed => "Completed",
        TransformationState::Failed => "Failed",
        TransformationState::Cancelled => "Cancelled",
    }
}

fn subscription_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Subscription> {
    Ok(Subscription {
        id: row.get(0)?,
        name: row.get(1)?,
        url: row.get(2)?,
        preset_id: row.get(3)?,
        destination: row.get(4)?,
        check_interval_minutes: u32::try_from(row.get::<_, i64>(5)?).unwrap_or(360),
        max_entries: u32::try_from(row.get::<_, i64>(6)?).unwrap_or(10),
        enabled: row.get(7)?,
        last_checked_at: row.get::<_, Option<i64>>(8)?.map(timestamp_or_epoch),
        next_check_at: timestamp_or_epoch(row.get(9)?),
        last_error: row.get(10)?,
        created_at: timestamp_or_epoch(row.get(11)?),
        updated_at: timestamp_or_epoch(row.get(12)?),
    })
}

fn parse_uuid(value: &str, code: &'static str, message: &'static str) -> Result<String, AppError> {
    Uuid::parse_str(value)
        .map(|value| value.to_string())
        .map_err(|_| AppError::new(code, message))
}

fn site_preset_rule_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SitePresetRule> {
    Ok(SitePresetRule {
        id: row.get(0)?,
        name: row.get(1)?,
        host_suffix: row.get(2)?,
        path_prefix: row.get(3)?,
        preset_id: row.get(4)?,
        enabled: row.get(5)?,
        created_at: timestamp_or_epoch(row.get(6)?),
        updated_at: timestamp_or_epoch(row.get(7)?),
    })
}

fn normalize_host_suffix(value: &str) -> Result<String, AppError> {
    let value = value.trim().trim_start_matches("*.").trim_end_matches('.');
    if value.is_empty() || value.len() > 253 || value.contains(['/', ':', '?', '#']) {
        return Err(AppError::new(
            "invalid_host_suffix",
            "Enter a hostname such as youtube.com, without a URL path.",
        ));
    }
    let parsed = url::Url::parse(&format!("https://{value}/"))
        .map_err(|_| AppError::new("invalid_host_suffix", "Enter a valid hostname."))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AppError::new("invalid_host_suffix", "Enter a valid hostname."))?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host != value.to_ascii_lowercase() {
        return Err(AppError::new(
            "invalid_host_suffix",
            "Enter a hostname such as youtube.com.",
        ));
    }
    Ok(host)
}

fn normalize_path_prefix(value: Option<&str>) -> Result<Option<String>, AppError> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    let Some(value) = value else {
        return Ok(None);
    };
    if !value.starts_with('/')
        || value.len() > 256
        || value.contains(['?', '#'])
        || value.chars().any(char::is_control)
    {
        return Err(AppError::new(
            "invalid_path_prefix",
            "Path prefixes must start with / and cannot contain a query or fragment.",
        ));
    }
    Ok(Some(value.to_owned()))
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DownloadJob> {
    let state: String = row.get(4)?;
    let created_at: i64 = row.get(16)?;
    let updated_at: i64 = row.get(17)?;
    let completed_at: Option<i64> = row.get(18)?;
    let diagnostics_json: String = row.get(19)?;
    let attempt_count: i64 = row.get(20)?;
    let gallery_files_json: String = row.get(22)?;
    Ok(DownloadJob {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        thumbnail: row.get(3)?,
        state: parse_download_state(&state),
        progress_percent: row.get(5)?,
        downloaded_bytes: optional_u64(row.get(6)?),
        total_bytes: optional_u64(row.get(7)?),
        speed_bytes_per_second: row.get(8)?,
        eta_seconds: optional_u64(row.get(9)?),
        selected_quality: row.get(10)?,
        output_format: row.get(11)?,
        destination: row.get(12)?,
        output_path: row.get(13)?,
        current_operation: row.get(14)?,
        error: row.get(15)?,
        diagnostics: serde_json::from_str(&diagnostics_json).unwrap_or_default(),
        gallery_files: serde_json::from_str(&gallery_files_json).unwrap_or_default(),
        attempt_count: u32::try_from(attempt_count).unwrap_or(1).max(1),
        retry_of_id: row.get(21)?,
        created_at: timestamp_or_epoch(created_at),
        updated_at: timestamp_or_epoch(updated_at),
        completed_at: completed_at.map(timestamp_or_epoch),
        scheduled_for: None,
    })
}

fn download_state_name(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "Queued",
        DownloadState::Paused => "Paused",
        DownloadState::Analyzing => "Analyzing",
        DownloadState::Preparing => "Preparing",
        DownloadState::Downloading => "Downloading",
        DownloadState::PostProcessing => "PostProcessing",
        DownloadState::Merging => "Merging",
        DownloadState::Verifying => "Verifying",
        DownloadState::Completed => "Completed",
        DownloadState::Failed => "Failed",
        DownloadState::Cancelled => "Cancelled",
    }
}

fn parse_download_state(value: &str) -> DownloadState {
    match value {
        "Queued" => DownloadState::Queued,
        "Paused" => DownloadState::Paused,
        "Analyzing" => DownloadState::Analyzing,
        "Preparing" => DownloadState::Preparing,
        "Downloading" => DownloadState::Downloading,
        "PostProcessing" => DownloadState::PostProcessing,
        "Merging" => DownloadState::Merging,
        "Verifying" => DownloadState::Verifying,
        "Completed" => DownloadState::Completed,
        "Cancelled" => DownloadState::Cancelled,
        _ => DownloadState::Failed,
    }
}

fn optional_u64(value: Option<i64>) -> Option<u64> {
    value.and_then(|number| u64::try_from(number).ok())
}

fn timestamp_or_epoch(milliseconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(milliseconds).unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
}

fn validate_post_download_recipe_steps(
    steps: &mut Vec<PostDownloadRecipeStep>,
) -> Result<(), AppError> {
    if steps.is_empty() || steps.len() > 12 {
        return Err(AppError::new(
            "invalid_post_download_recipe_steps",
            "A post-download recipe contains between 1 and 12 typed steps.",
        ));
    }
    for step in steps {
        match step {
            PostDownloadRecipeStep::AddTags { tags } => {
                *tags = normalize_library_tags(std::mem::take(tags))?;
                if tags.is_empty() {
                    return Err(AppError::new(
                        "invalid_post_download_recipe_tags",
                        "The Add tags step needs at least one valid tag.",
                    ));
                }
            }
            PostDownloadRecipeStep::AddToCollection { collection_id } => {
                validate_uuid(collection_id, "collection identifier")?;
            }
            PostDownloadRecipeStep::Transcribe {
                model_path,
                language,
            } => {
                *model_path = model_path.trim().chars().take(4_096).collect();
                if model_path.is_empty() || model_path.chars().any(char::is_control) {
                    return Err(AppError::new(
                        "invalid_recipe_whisper_model",
                        "Choose a valid local whisper.cpp model path.",
                    ));
                }
                if language.as_ref().is_some_and(|value| {
                    value.chars().count() > 24
                        || !value.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || character == '-'
                                || character == '_'
                        })
                }) {
                    return Err(AppError::new(
                        "invalid_recipe_transcription_language",
                        "Use a short whisper.cpp language code.",
                    ));
                }
            }
            PostDownloadRecipeStep::SemanticIndex { model } => {
                *model = model.trim().chars().take(100).collect();
                if model.is_empty()
                    || model.chars().any(char::is_control)
                    || model.chars().any(char::is_whitespace)
                {
                    return Err(AppError::new(
                        "invalid_recipe_embedding_model",
                        "Use a valid local Ollama embedding model name.",
                    ));
                }
            }
            PostDownloadRecipeStep::ImportLibrary
            | PostDownloadRecipeStep::TranscodeMp4
            | PostDownloadRecipeStep::ExtractAudio { .. }
            | PostDownloadRecipeStep::PrivacySafeCopy => {}
        }
    }
    Ok(())
}

fn post_download_recipe_run_state_name(state: PostDownloadRecipeRunState) -> &'static str {
    match state {
        PostDownloadRecipeRunState::Running => "running",
        PostDownloadRecipeRunState::Completed => "completed",
        PostDownloadRecipeRunState::Failed => "failed",
    }
}

fn post_download_recipe_run_state_from_name(value: &str) -> PostDownloadRecipeRunState {
    match value {
        "completed" => PostDownloadRecipeRunState::Completed,
        "failed" => PostDownloadRecipeRunState::Failed,
        _ => PostDownloadRecipeRunState::Running,
    }
}

fn load_library_privacy_settings_record(
    connection: &Connection,
) -> Result<Option<LibraryPrivacySettingsRecord>, AppError> {
    let value: Option<String> = connection
        .query_row(
            "SELECT value_json FROM settings WHERE key = ?1",
            [LIBRARY_PRIVACY_SETTINGS_KEY],
            |row| row.get(0),
        )
        .optional()?;
    value
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                AppError::internal("could not read Library privacy settings", error)
            })
        })
        .transpose()
}

fn validate_library_privacy_passphrase(passphrase: &str) -> Result<(), AppError> {
    let length = passphrase.chars().count();
    if !(6..=256).contains(&length) || passphrase.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_privacy_passphrase",
            "Use a Library privacy passphrase between 6 and 256 characters without control characters.",
        ));
    }
    Ok(())
}

fn derive_library_privacy_verifier(passphrase: &str, salt: &str) -> String {
    let mut digest = Sha256::new()
        .chain_update(b"nice-downloader-library-privacy-v1")
        .chain_update(salt.as_bytes())
        .chain_update(passphrase.as_bytes())
        .finalize()
        .to_vec();
    for round in 0_u32..50_000 {
        digest = Sha256::new()
            .chain_update(b"nice-downloader-library-privacy-v1")
            .chain_update(salt.as_bytes())
            .chain_update(round.to_le_bytes())
            .chain_update(&digest)
            .chain_update(passphrase.as_bytes())
            .finalize()
            .to_vec();
    }
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

fn normalize_private_folder_path(value: &str) -> Result<String, AppError> {
    if value.chars().count() > 1_024 || value.chars().any(char::is_control) {
        return Err(AppError::new(
            "invalid_library_private_folder",
            "The private folder path is invalid.",
        ));
    }
    let normalized = value.replace('\\', "/");
    let normalized = normalized.trim_matches('/');
    if normalized
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err(AppError::new(
            "invalid_library_private_folder",
            "Private folder paths cannot contain parent-directory segments.",
        ));
    }
    Ok(normalized.to_owned())
}

fn library_locked_error() -> AppError {
    AppError::new(
        "library_locked",
        "The private Library is locked. Unlock it before accessing this media.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, DownloadOptions, MediaAnalysis, OutputConflictPolicy, OutputContainer,
        QualityKind, QualityOption, SaveLibraryVideoDisplaySettingsRequest, VideoCodecPreference,
    };
    use std::time::Instant;
    use tempfile::tempdir;

    fn media_stack_test_item(directory: &Path, title: &str, kind: LibraryMediaKind) -> LibraryItem {
        let now = Utc::now();
        let extension = if kind == LibraryMediaKind::Image {
            "jpg"
        } else {
            "mp4"
        };
        LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: directory
                .join(format!("{}-{title}.{extension}", Uuid::new_v4()))
                .to_string_lossy()
                .into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(kind),
            title: title.to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: (kind == LibraryMediaKind::Video).then_some(30.0),
            site: None,
            tags: Vec::new(),
            metadata: serde_json::json!({}),
            file_size_bytes: Some(100),
            modified_at_ms: Some(1),
            width: Some(1920),
            height: Some(1080),
            fps: (kind == LibraryMediaKind::Video).then_some(30.0),
            video_codec: (kind == LibraryMediaKind::Video).then(|| "h264".to_owned()),
            audio_codec: None,
            container: Some(extension.to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn migrates_and_round_trips_completed_history() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("test.sqlite3")).expect("database");
        let now = Utc::now();
        let job = DownloadJob {
            id: "one".to_owned(),
            url: "https://example.com/video".to_owned(),
            title: "Example".to_owned(),
            thumbnail: None,
            state: DownloadState::Completed,
            progress_percent: 100.0,
            downloaded_bytes: Some(42),
            total_bytes: Some(42),
            speed_bytes_per_second: None,
            eta_seconds: Some(0),
            selected_quality: "Best".to_owned(),
            output_format: "mp4".to_owned(),
            destination: directory.path().to_string_lossy().into_owned(),
            output_path: Some(
                directory
                    .path()
                    .join("video.mp4")
                    .to_string_lossy()
                    .into_owned(),
            ),
            current_operation: "Download completed".to_owned(),
            error: None,
            diagnostics: vec!["finished".to_owned()],
            gallery_files: vec![crate::models::GalleryFileProgress {
                index: 2,
                filename: "failed.mp4".to_owned(),
                size_bytes: Some(42),
                state: crate::models::GalleryFileState::Failed,
                progress_percent: 50.0,
            }],
            attempt_count: 2,
            retry_of_id: Some("original".to_owned()),
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
            scheduled_for: None,
        };
        std::fs::write(directory.path().join("video.mp4"), b"fixture").expect("media fixture");
        let options = DownloadOptions {
            quality: QualityOption {
                id: "best".to_owned(),
                label: "Best".to_owned(),
                description: String::new(),
                kind: QualityKind::Best,
                max_height: None,
            },
            container: OutputContainer::Mp4,
            video_codec_preference: VideoCodecPreference::Any,
            format_strategy: crate::models::FormatStrategy::Recommended,
            audio_quality: AudioQuality::Best,
            audio_only: false,
            subtitles: false,
            subtitle_languages: vec![],
            embed_thumbnail: false,
            embed_metadata: true,
            chapters: false,
            sponsorblock: false,
            sponsorblock_categories: vec![],
            destination: job.destination.clone(),
            filename_template: "%(title)s.%(ext)s".to_owned(),
            output_conflict_policy: OutputConflictPolicy::Skip,
            gallery_selection: None,
        };
        let request = StartDownloadRequest {
            url: job.url.clone(),
            analysis: MediaAnalysis {
                engine: crate::models::DownloadEngineKind::YtDlp,
                title: job.title.clone(),
                uploader: None,
                duration_seconds: None,
                thumbnail: None,
                webpage_url: job.url.clone(),
                download_url: job.url.clone(),
                extractor: "generic".to_owned(),
                site_name: "Generic".to_owned(),
                playlist_title: None,
                playlist_index: None,
                formats: vec![],
                quality_options: vec![options.quality.clone()],
                item_count: None,
                total_size_bytes: None,
            },
            options,
            scheduled_for: None,
            post_download_recipe_ids: Vec::new(),
        };
        database.save_job(&job, &request).expect("save history");
        let repeated = database
            .load_download_request(&job.id)
            .expect("load repeat request");
        assert_eq!(repeated.url, request.url);
        let history = database.list_history().expect("list history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "Example");
        assert_eq!(history[0].diagnostics, ["finished"]);
        assert_eq!(history[0].attempt_count, 2);
        assert_eq!(history[0].gallery_files, job.gallery_files);
        let library = database.list_library_items().expect("auto-indexed library");
        assert_eq!(library.len(), 1);
        assert_eq!(library[0].title, "Example");
        assert_eq!(library[0].media_kind, Some(LibraryMediaKind::Video));
        database
            .dismiss_download_record(&job.id)
            .expect("dismiss completed download");
        assert_eq!(database.list_history().expect("history remains").len(), 1);
        assert!(database
            .archive_match(&job.url, &job.destination)
            .expect("archive lookup")
            .is_some());
        database
            .connection
            .lock()
            .expect("database lock")
            .execute("DELETE FROM downloads WHERE id = ?1", [&job.id])
            .expect("delete history record");
        assert!(database
            .archive_match(&job.url, &job.destination)
            .expect("archive survives history deletion")
            .is_some());
    }

    #[test]
    fn round_trips_engine_preferences() {
        use crate::models::BrowserCookieSource;

        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("settings.sqlite3")).expect("database");
        assert_eq!(
            database
                .load_engine_preferences()
                .expect("default settings"),
            EnginePreferences::default()
        );

        let preferences = EnginePreferences {
            browser_cookie_source: BrowserCookieSource::Firefox,
            browser_profile: Some("default-release".to_owned()),
            proxy_url: Some("http://127.0.0.1:8080".to_owned()),
            enable_impersonation: false,
            bandwidth_limit_kib: Some(2048),
            managed_cookie_file: None,
        };
        database
            .save_engine_preferences(&preferences)
            .expect("save settings");
        assert_eq!(
            database.load_engine_preferences().expect("saved settings"),
            preferences
        );
    }

    #[test]
    fn persists_desktop_preferences_and_onboarding() {
        use crate::models::{CloseBehavior, DesktopPreferences, PlaybackCompatibilityPreference};
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("desktop.sqlite3")).expect("database");
        assert!(!database.onboarding_completed().expect("onboarding status"));
        let preferences = DesktopPreferences {
            close_behavior: CloseBehavior::Background,
            start_minimized: true,
            notify_when_backgrounded: false,
            reduce_motion: true,
            high_contrast: true,
            interface_scale_percent: 110,
            playback_compatibility: PlaybackCompatibilityPreference::Automatic,
        };
        database
            .save_desktop_preferences(&preferences)
            .expect("save desktop preferences");
        assert_eq!(
            database
                .load_desktop_preferences()
                .expect("load desktop preferences"),
            preferences
        );
        database.complete_onboarding().expect("complete onboarding");
        assert!(database.onboarding_completed().expect("completed status"));
    }

    #[test]
    fn saves_validates_and_deletes_typed_post_download_recipes() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("recipes.sqlite3")).expect("database");
        let recipe = database
            .save_post_download_recipe(SavePostDownloadRecipeRequest {
                id: None,
                name: "  Archive and index  ".to_owned(),
                enabled: true,
                steps: vec![
                    PostDownloadRecipeStep::AddTags {
                        tags: vec![" Archive ".to_owned(), "archive".to_owned()],
                    },
                    PostDownloadRecipeStep::PrivacySafeCopy,
                    PostDownloadRecipeStep::SemanticIndex {
                        model: "nomic-embed-text".to_owned(),
                    },
                ],
            })
            .expect("save recipe");
        assert_eq!(recipe.name, "Archive and index");
        assert_eq!(recipe.steps.len(), 3);
        assert!(matches!(
            &recipe.steps[0],
            PostDownloadRecipeStep::AddTags { tags } if tags == &vec!["Archive".to_owned()]
        ));
        assert_eq!(
            database
                .enabled_post_download_recipes_by_ids(std::slice::from_ref(&recipe.id))
                .expect("selected recipe")
                .len(),
            1
        );
        let invalid = database
            .save_post_download_recipe(SavePostDownloadRecipeRequest {
                id: None,
                name: "Invalid".to_owned(),
                enabled: true,
                steps: vec![PostDownloadRecipeStep::SemanticIndex {
                    model: "not a model".to_owned(),
                }],
            })
            .expect_err("invalid model");
        assert_eq!(invalid.code, "invalid_recipe_embedding_model");
        database
            .delete_post_download_recipe(&recipe.id)
            .expect("delete recipe");
        assert!(database
            .list_post_download_recipes()
            .expect("recipes")
            .is_empty());
    }

    #[test]
    fn creates_lists_and_deletes_presets() {
        use crate::models::{
            AudioQuality, OutputConflictPolicy, OutputContainer, PresetIntent, SavePresetRequest,
            SaveSitePresetRuleRequest, VideoCodecPreference,
        };
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("presets.sqlite3")).expect("database");
        let preset = database
            .save_preset(SavePresetRequest {
                id: None,
                name: "  Living room  ".to_owned(),
                intent: PresetIntent {
                    quality: "1080".to_owned(),
                    container: OutputContainer::Mp4,
                    video_codec_preference: VideoCodecPreference::H264,
                    audio_quality: AudioQuality::Best,
                    embed_thumbnail: false,
                    embed_metadata: true,
                    output_conflict_policy: OutputConflictPolicy::Skip,
                    sponsorblock_categories: vec![],
                    filename_template: None,
                },
            })
            .expect("save preset");
        assert_eq!(preset.name, "Living room");
        assert_eq!(database.list_presets().expect("list presets").len(), 1);
        let rule = database
            .save_site_preset_rule(SaveSitePresetRuleRequest {
                id: None,
                name: "YouTube shorts".to_owned(),
                host_suffix: "*.YouTube.com".to_owned(),
                path_prefix: Some("/shorts/".to_owned()),
                preset_id: preset.id.clone(),
                enabled: true,
            })
            .expect("save site rule");
        assert_eq!(rule.host_suffix, "youtube.com");
        assert_eq!(
            database
                .match_site_preset_rule("https://www.youtube.com/shorts/example")
                .expect("match rule")
                .map(|matched| matched.id),
            Some(rule.id.clone())
        );
        assert!(database
            .match_site_preset_rule("https://www.youtube.com/watch?v=example")
            .expect("path mismatch")
            .is_none());
        let duplicate = database
            .save_site_preset_rule(SaveSitePresetRuleRequest {
                id: None,
                name: "Duplicate YouTube shorts".to_owned(),
                host_suffix: "youtube.com".to_owned(),
                path_prefix: Some("/shorts/".to_owned()),
                preset_id: preset.id.clone(),
                enabled: true,
            })
            .expect_err("duplicate enabled site rule");
        assert_eq!(duplicate.code, "duplicate_site_rule");
        database.delete_preset(&preset.id).expect("delete preset");
        assert!(database.list_presets().expect("list presets").is_empty());
        assert!(database
            .list_site_preset_rules()
            .expect("cascade site rules")
            .is_empty());
    }

    #[test]
    fn rejects_duplicate_enabled_authentication_hosts() {
        use crate::models::{BrowserCookieSource, SaveAuthenticationProfileRequest};
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("auth.sqlite3")).expect("database");
        let request = |name: &str| SaveAuthenticationProfileRequest {
            id: None,
            name: name.to_owned(),
            host_suffix: "reddit.com".to_owned(),
            browser_cookie_source: BrowserCookieSource::Firefox,
            browser_profile: None,
            proxy_url: None,
            cookie_file_path: None,
            clear_cookie_file: false,
            enabled: true,
        };
        database
            .save_authentication_profile(request("Primary"), None)
            .expect("primary profile");
        let duplicate = database
            .save_authentication_profile(request("Duplicate"), None)
            .expect_err("duplicate enabled authentication host");
        assert_eq!(duplicate.code, "duplicate_auth_profile");
    }

    #[test]
    fn schedules_and_records_subscriptions() {
        use crate::models::{
            AudioQuality, OutputConflictPolicy, OutputContainer, PresetIntent, SavePresetRequest,
            SaveSubscriptionRequest, VideoCodecPreference,
        };
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("subscriptions.sqlite3")).expect("database");
        let preset = database
            .save_preset(SavePresetRequest {
                id: None,
                name: "Subscription default".to_owned(),
                intent: PresetIntent {
                    quality: "best".to_owned(),
                    container: OutputContainer::Mkv,
                    video_codec_preference: VideoCodecPreference::Any,
                    audio_quality: AudioQuality::Best,
                    embed_thumbnail: false,
                    embed_metadata: true,
                    output_conflict_policy: OutputConflictPolicy::Skip,
                    sponsorblock_categories: vec![],
                    filename_template: None,
                },
            })
            .expect("preset");
        let subscription = database
            .save_subscription(SaveSubscriptionRequest {
                id: None,
                name: "Example channel".to_owned(),
                url: "https://example.com/channel".to_owned(),
                preset_id: preset.id,
                destination: directory.path().to_string_lossy().into_owned(),
                check_interval_minutes: 60,
                max_entries: 5,
                enabled: true,
            })
            .expect("subscription");
        assert_eq!(database.due_subscriptions(10).expect("due").len(), 1);
        let checked = database
            .record_subscription_check(&subscription.id, Some("temporary failure"))
            .expect("record check");
        assert!(checked.last_checked_at.is_some());
        assert_eq!(checked.last_error.as_deref(), Some("temporary failure"));
        assert!(database.due_subscriptions(10).expect("not due").is_empty());
        database
            .delete_subscription(&subscription.id)
            .expect("delete subscription");
        assert!(database.list_subscriptions().expect("list").is_empty());
    }

    #[test]
    fn upgrades_an_existing_milestone_one_database() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("upgrade.sqlite3");
        {
            let connection = Connection::open(&path).expect("legacy database");
            connection
                .execute_batch(
                    "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL);
                     INSERT INTO schema_migrations(version) VALUES (1);",
                )
                .expect("legacy migration registry");
            connection
                .execute_batch(MIGRATION_0001)
                .expect("milestone one schema");
        }

        let database = Database::open(&path).expect("upgraded database");
        let connection = database.connection.lock().expect("database lock");
        let has_request_json: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM pragma_table_info('downloads') WHERE name = 'request_json'
                )",
                [],
                |row| row.get(0),
            )
            .expect("migration column");
        assert!(has_request_json);
    }

    #[test]
    fn rolls_back_a_failed_migration_and_recovers_on_the_next_open() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("migration-recovery.sqlite3");
        {
            let mut connection = Connection::open(&path).expect("legacy database");
            let transaction = connection.transaction().expect("legacy transaction");
            transaction
                .execute_batch(
                    "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY NOT NULL);",
                )
                .expect("migration registry");
            for (version, migration) in [
                (1, MIGRATION_0001),
                (2, MIGRATION_0002),
                (3, MIGRATION_0003),
                (4, MIGRATION_0004),
                (5, MIGRATION_0005),
                (6, MIGRATION_0006),
                (7, MIGRATION_0007),
                (8, MIGRATION_0008),
            ] {
                transaction
                    .execute_batch(migration)
                    .expect("legacy migration");
                transaction
                    .execute(
                        "INSERT INTO schema_migrations(version) VALUES (?1)",
                        [version],
                    )
                    .expect("legacy migration version");
            }
            transaction.commit().expect("legacy commit");
            connection
                .execute_batch("CREATE TABLE library_sources (id TEXT PRIMARY KEY NOT NULL);")
                .expect("malformed future table");
        }

        let error = match Database::open(&path) {
            Ok(_) => panic!("malformed migration unexpectedly succeeded"),
            Err(error) => error,
        };
        assert!(error.message.contains("database") || error.details.is_some());

        {
            let connection = Connection::open(&path).expect("inspect rolled-back database");
            let latest_version: i64 = connection
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("latest migration");
            assert_eq!(latest_version, 8);
            let has_scan_jobs: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_scan_jobs')",
                    [],
                    |row| row.get(0),
                )
                .expect("scan job table state");
            let has_source_id: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('media_library') WHERE name = 'source_id')",
                    [],
                    |row| row.get(0),
                )
                .expect("source column state");
            assert!(!has_scan_jobs);
            assert!(!has_source_id);
            connection
                .execute("DROP TABLE library_sources", [])
                .expect("remove malformed table");
        }

        let database = Database::open(&path).expect("migration recovery");
        let connection = database.connection.lock().expect("database lock");
        let latest_version: i64 = connection
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("recovered migration version");
        assert_eq!(latest_version, 34);
        let has_playback: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_playback')",
                [],
                |row| row.get(0),
            )
            .expect("playback table state");
        assert!(has_playback);
        let has_playlists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'library_playlists')",
                [],
                |row| row.get(0),
            )
            .expect("playlist table state");
        assert!(has_playlists);
        let has_moments: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_moments')",
                [],
                |row| row.get(0),
            )
            .expect("moments table state");
        assert!(has_moments);
        let has_compare_sessions: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_compare_sessions')",
                [],
                |row| row.get(0),
            )
            .expect("comparison session table state");
        assert!(has_compare_sessions);
        let has_media_stacks: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_media_stacks')",
                [],
                |row| row.get(0),
            )
            .expect("media stack table state");
        assert!(has_media_stacks);
        let has_rich_metadata: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'library_rich_metadata')",
                [],
                |row| row.get(0),
            )
            .expect("rich metadata table state");
        assert!(has_rich_metadata);
        let has_smart_views: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'library_smart_views')",
                [],
                |row| row.get(0),
            )
            .expect("smart view table state");
        assert!(has_smart_views);
        let has_search: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master
                 WHERE type = 'table' AND name = 'library_search')",
                [],
                |row| row.get(0),
            )
            .expect("search table state");
        assert!(has_search);
    }

    #[test]
    fn persists_library_sources_and_scan_jobs() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("library.sqlite3")).expect("database");
        let source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Archive".to_owned(),
                path: directory.path().to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: false,
                private: true,
                include_patterns: vec!["Trips/*".to_owned()],
                exclude_patterns: vec!["*/cache/*".to_owned()],
            })
            .expect("save source");
        assert_eq!(database.list_library_sources().expect("sources").len(), 1);
        assert!(source.private);

        let now = Utc::now();
        let media_path = directory.path().join("clip.mp4");
        let existing = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: media_path.to_string_lossy().into_owned(),
            source_id: Some(source.id.clone()),
            relative_path: Some("clip.mp4".to_owned()),
            media_kind: Some(LibraryMediaKind::Video),
            title: "My edited title".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: None,
            site: None,
            tags: vec!["favorite".to_owned()],
            metadata: serde_json::json!({}),
            file_size_bytes: Some(42),
            modified_at_ms: Some(1_700_000_000_000),
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
        database
            .upsert_library_item(&existing)
            .expect("library item");
        let mut rescanned = existing.clone();
        rescanned.id = Uuid::new_v4().to_string();
        rescanned.title = "clip".to_owned();
        rescanned.tags.clear();
        rescanned.metadata = serde_json::json!({ "fileSize": 42 });
        database
            .upsert_scanned_library_item(&rescanned)
            .expect("rescanned item");
        let stored = database.list_library_items().expect("library items");
        assert_eq!(stored[0].title, "My edited title");
        assert_eq!(stored[0].tags, ["favorite"]);
        assert_eq!(stored[0].metadata["fileSize"], 42);
        assert_eq!(stored[0].id, existing.id);

        database
            .save_library_probe_result(
                &stored[0].id,
                &LibraryProbeResult {
                    file_size_bytes: 42,
                    modified_at_ms: 1_700_000_000_000,
                    duration_seconds: Some(12.5),
                    width: Some(1920),
                    height: Some(1080),
                    fps: Some(30.0),
                    video_codec: Some("h264".to_owned()),
                    audio_codec: Some("aac".to_owned()),
                    container: Some("mp4".to_owned()),
                    dynamic_range: None,
                    state: LibraryMetadataState::Ready,
                    error: None,
                },
            )
            .expect("probe result");
        let unchanged = database
            .upsert_scanned_library_item(&rescanned)
            .expect("unchanged rescan");
        assert_eq!(unchanged.metadata_state, LibraryMetadataState::Ready);
        assert_eq!(unchanged.height, Some(1080));

        rescanned.file_size_bytes = Some(84);
        let changed = database
            .upsert_scanned_library_item(&rescanned)
            .expect("changed rescan");
        assert_eq!(changed.metadata_state, LibraryMetadataState::Pending);
        assert_eq!(changed.height, None);

        let collection = database
            .save_library_collection(SaveLibraryCollectionRequest {
                id: None,
                name: "Watch later".to_owned(),
            })
            .expect("collection");
        let automation = database
            .save_library_source_automation(SaveLibrarySourceAutomationRequest {
                source_id: source.id.clone(),
                tags: vec!["Incoming".to_owned(), "incoming".to_owned()],
                collection_id: Some(collection.id.clone()),
                favorite: true,
            })
            .expect("source automation");
        assert_eq!(automation.tags, ["Incoming"]);
        database
            .apply_library_source_automation(&source.id, &changed.id)
            .expect("apply source automation");
        let automated = database
            .get_library_item(&changed.id)
            .expect("automated item");
        assert!(automated.favorite);
        assert!(automated.tags.iter().any(|tag| tag == "Incoming"));
        assert!(database
            .list_library_collections()
            .expect("automated collection")
            .iter()
            .any(|entry| entry.id == collection.id && entry.item_ids.contains(&changed.id)));
        database
            .apply_library_bulk_action(
                vec![changed.id.clone()],
                LibraryBulkAction::SetFavorite { favorite: true },
            )
            .expect("favorite");
        database
            .apply_library_bulk_action(
                vec![changed.id.clone()],
                LibraryBulkAction::SetRating { rating: Some(5) },
            )
            .expect("rating");
        database
            .apply_library_bulk_action(
                vec![changed.id.clone()],
                LibraryBulkAction::AddTags {
                    tags: vec!["Later".to_owned(), "later".to_owned()],
                },
            )
            .expect("tags");
        let curated = database
            .apply_library_bulk_action(
                vec![changed.id.clone()],
                LibraryBulkAction::AddToCollection {
                    collection_id: collection.id.clone(),
                },
            )
            .expect("collection membership");
        assert!(curated[0].favorite);
        assert_eq!(curated[0].rating, Some(5));
        assert!(curated[0].tags.iter().any(|tag| tag == "Later"));
        let curated_after_rescan = database
            .upsert_scanned_library_item(&rescanned)
            .expect("rescan curated item");
        assert!(curated_after_rescan.favorite);
        assert_eq!(curated_after_rescan.rating, Some(5));
        assert!(curated_after_rescan.tags.iter().any(|tag| tag == "Later"));
        let collections = database.list_library_collections().expect("collections");
        assert_eq!(
            collections[0].item_ids.as_slice(),
            std::slice::from_ref(&changed.id)
        );

        let identity = LibraryFileIdentity {
            device_id: 22,
            file_id: 44,
        };
        database
            .save_library_file_identity(&changed.id, identity)
            .expect("file identity");
        let digest = "a".repeat(64);
        database
            .save_library_content_hash(
                &changed.id,
                changed.file_size_bytes.expect("file size"),
                changed.modified_at_ms.expect("modified time"),
                &digest,
            )
            .expect("content hash");
        assert_eq!(
            database
                .list_current_library_content_hashes()
                .expect("current content hashes"),
            [(changed.id.clone(), 84, digest)]
        );
        assert_eq!(
            database
                .find_library_items_by_identity(&source.id, identity)
                .expect("identity lookup")[0]
                .id,
            changed.id
        );
        let previous_path = changed.path.clone();
        let mut renamed = changed.clone();
        let renamed_path = directory
            .path()
            .join("renamed.mp4")
            .to_string_lossy()
            .into_owned();
        renamed.path.clone_from(&renamed_path);
        renamed.relative_path = Some("renamed.mp4".to_owned());
        let renamed = database
            .move_library_item_record(&changed.id, &renamed)
            .expect("move catalog record");
        assert_eq!(renamed.id, changed.id);
        assert_eq!(renamed.path, renamed_path);
        let change = database
            .record_library_change(
                &renamed.id,
                renamed.source_id.as_deref(),
                LibraryChangeKind::Renamed,
                Some(&previous_path),
            )
            .expect("rename change");
        let changes = database.list_library_changes().expect("library changes");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].kind, LibraryChangeKind::Renamed);
        assert_eq!(
            changes[0].previous_path.as_deref(),
            Some(previous_path.as_str())
        );
        assert_eq!(
            database
                .review_library_changes(vec![change.id])
                .expect("review change"),
            1
        );
        assert!(database
            .list_library_changes()
            .expect("reviewed changes")
            .is_empty());

        database
            .apply_library_bulk_action(
                vec![changed.id.clone()],
                LibraryBulkAction::RemoveFromCollection {
                    collection_id: collection.id,
                },
            )
            .expect("remove membership");
        assert!(database
            .list_library_collections()
            .expect("collections")
            .first()
            .is_some_and(|collection| collection.item_ids.is_empty()));

        let job = LibraryScanJob {
            id: Uuid::new_v4().to_string(),
            source_id: source.id.clone(),
            state: LibraryScanState::Completed,
            discovered_count: 4,
            indexed_count: 3,
            skipped_count: 1,
            warning_count: 0,
            current_path: None,
            warnings: vec![],
            started_at: now,
            updated_at: now,
            completed_at: Some(now),
        };
        database.save_library_scan_job(&job).expect("scan job");
        let restored = database.list_library_scan_jobs().expect("scan jobs");
        assert_eq!(restored[0].indexed_count, 3);

        database
            .remove_library_source(&source.id)
            .expect("remove source");
        assert!(database.list_library_sources().expect("sources").is_empty());
        assert!(database
            .list_library_scan_jobs()
            .expect("scan jobs")
            .is_empty());
    }

    #[test]
    fn imports_filters_and_preserves_rich_metadata_overrides() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("rich-metadata.sqlite3")).expect("database");
        let item_id = Uuid::new_v4().to_string();
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO media_library
                     (id, path, title, tags_json, metadata_json, missing, added_at, updated_at, media_kind)
                     VALUES (?1, ?2, 'Episode', '[]', '{}', 0, ?3, ?3, 'video')",
                    params![
                        item_id,
                        directory.path().join("episode.mp4").to_string_lossy(),
                        Utc::now().timestamp_millis()
                    ],
                )
                .expect("catalog item");
            save_extracted_library_metadata(
                &connection,
                &item_id,
                &serde_json::json!({
                    "uploader": "Extracted channel",
                    "description": "Sidecar description",
                    "uploadDate": "20260102",
                    "series": "Season one",
                    "episode": "Episode two",
                    "webpageUrl": "https://example.com/watch/2"
                }),
            )
            .expect("extracted metadata");
        }

        let extracted = database
            .get_library_rich_metadata(&item_id)
            .expect("rich metadata");
        assert_eq!(
            extracted.effective.creator.as_deref(),
            Some("Extracted channel")
        );
        assert_eq!(
            extracted.effective.upload_date.as_deref(),
            Some("2026-01-02")
        );

        let overridden = database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: item_id.clone(),
                creator: Some("My channel name".to_owned()),
                description: Some("Sidecar description".to_owned()),
                upload_date: Some("2026-01-02".to_owned()),
                series: Some("Season one".to_owned()),
                episode: Some("Episode two".to_owned()),
                original_url: Some("https://example.com/watch/2".to_owned()),
            })
            .expect("metadata override");
        assert_eq!(overridden.overridden_fields, vec!["creator"]);
        assert_eq!(
            overridden.effective.creator.as_deref(),
            Some("My channel name")
        );

        let mut request = library_query_request();
        request.creators = vec!["My channel name".to_owned()];
        assert_eq!(
            database
                .query_library_items(request)
                .expect("creator filter")
                .total_items,
            1
        );
        assert!(database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("creator facets")
            .creators
            .iter()
            .any(|creator| creator == "My channel name"));

        {
            let connection = database.connection.lock().expect("database lock");
            save_extracted_library_metadata(
                &connection,
                &item_id,
                &serde_json::json!({ "uploader": "Updated extracted channel" }),
            )
            .expect("refreshed extracted metadata");
        }
        let refreshed = database
            .get_library_rich_metadata(&item_id)
            .expect("refreshed metadata");
        assert_eq!(
            refreshed.extracted.creator.as_deref(),
            Some("Updated extracted channel")
        );
        assert_eq!(
            refreshed.effective.creator.as_deref(),
            Some("My channel name")
        );
    }

    #[test]
    fn bulk_metadata_updates_only_entered_fields_and_merges_tags() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("bulk-metadata.sqlite3")).expect("database");
        let mut first =
            media_stack_test_item(directory.path(), "First episode", LibraryMediaKind::Video);
        first.tags = vec!["existing".to_owned()];
        let mut second =
            media_stack_test_item(directory.path(), "Second episode", LibraryMediaKind::Video);
        second.tags = vec!["archive".to_owned()];
        database.upsert_library_item(&first).expect("first item");
        database.upsert_library_item(&second).expect("second item");

        database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: first.id.clone(),
                creator: Some("First creator".to_owned()),
                description: Some("First description".to_owned()),
                upload_date: Some("2026-02-03".to_owned()),
                series: Some("Original series".to_owned()),
                episode: Some("Episode one".to_owned()),
                original_url: Some("https://example.com/first".to_owned()),
            })
            .expect("first metadata");
        database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: second.id.clone(),
                creator: Some("Second creator".to_owned()),
                description: Some("Second description".to_owned()),
                upload_date: None,
                series: Some("Other series".to_owned()),
                episode: Some("Episode two".to_owned()),
                original_url: None,
            })
            .expect("second metadata");

        let changed = database
            .apply_library_bulk_metadata(ApplyLibraryBulkMetadataRequest {
                item_ids: vec![first.id.clone(), second.id.clone()],
                creator: None,
                series: Some("Shared series".to_owned()),
                episode: None,
                tags: vec!["curated".to_owned(), "EXISTING".to_owned()],
            })
            .expect("bulk metadata");
        assert_eq!(changed.len(), 2);

        let first_metadata = database
            .get_library_rich_metadata(&first.id)
            .expect("updated first metadata");
        assert_eq!(
            first_metadata.effective.creator.as_deref(),
            Some("First creator")
        );
        assert_eq!(
            first_metadata.effective.series.as_deref(),
            Some("Shared series")
        );
        assert_eq!(
            first_metadata.effective.episode.as_deref(),
            Some("Episode one")
        );
        assert_eq!(
            first_metadata.effective.description.as_deref(),
            Some("First description")
        );
        assert_eq!(
            first_metadata.effective.original_url.as_deref(),
            Some("https://example.com/first")
        );

        let second_metadata = database
            .get_library_rich_metadata(&second.id)
            .expect("updated second metadata");
        assert_eq!(
            second_metadata.effective.creator.as_deref(),
            Some("Second creator")
        );
        assert_eq!(
            second_metadata.effective.series.as_deref(),
            Some("Shared series")
        );
        assert_eq!(
            second_metadata.effective.episode.as_deref(),
            Some("Episode two")
        );

        let changed_by_id = changed
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let first_changed = changed_by_id.get(&first.id).expect("first changed item");
        assert!(first_changed.tags.iter().any(|tag| tag == "existing"));
        assert!(first_changed.tags.iter().any(|tag| tag == "curated"));
        assert_eq!(
            first_changed
                .tags
                .iter()
                .filter(|tag| tag.eq_ignore_ascii_case("existing"))
                .count(),
            1
        );
        let second_changed = changed_by_id.get(&second.id).expect("second changed item");
        assert!(second_changed.tags.iter().any(|tag| tag == "archive"));
        assert!(second_changed.tags.iter().any(|tag| tag == "curated"));

        let empty = database
            .apply_library_bulk_metadata(ApplyLibraryBulkMetadataRequest {
                item_ids: vec![first.id],
                creator: None,
                series: None,
                episode: None,
                tags: Vec::new(),
            })
            .expect_err("empty bulk metadata");
        assert_eq!(empty.code, "empty_library_bulk_metadata");
    }

    #[test]
    fn filters_sorts_resets_and_validates_rich_metadata() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("rich-discovery.sqlite3")).expect("database");
        let recent_id = Uuid::new_v4().to_string();
        let old_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        let current_date = Utc::now().date_naive().format("%Y-%m-%d").to_string();

        {
            let connection = database.connection.lock().expect("database lock");
            for (id, name, site) in [
                (&recent_id, "Recent episode", "YouTube"),
                (&old_id, "Archive episode", "Vimeo"),
            ] {
                connection
                    .execute(
                        "INSERT INTO media_library
                         (id, path, title, site, tags_json, metadata_json, missing, added_at, updated_at, media_kind)
                         VALUES (?1, ?2, ?3, ?4, '[]', '{}', 0, ?5, ?5, 'video')",
                        params![
                            id,
                            directory.path().join(format!("{id}.mp4")).to_string_lossy(),
                            name,
                            site,
                            now
                        ],
                    )
                    .expect("catalog item");
            }
            save_extracted_library_metadata(
                &connection,
                &recent_id,
                &serde_json::json!({
                    "uploader": "Current creator",
                    "description": "First line\nSecond line",
                    "uploadDate": current_date,
                    "series": "Current series",
                    "webpageUrl": "file:///tmp/untrusted.mp4"
                }),
            )
            .expect("recent metadata");
            save_extracted_library_metadata(
                &connection,
                &old_id,
                &serde_json::json!({
                    "uploader": "Archive creator",
                    "uploadDate": "2020-01-02",
                    "series": "Archive series"
                }),
            )
            .expect("old metadata");
        }

        let extracted = database
            .get_library_rich_metadata(&recent_id)
            .expect("sanitized extracted metadata");
        assert_eq!(
            extracted.effective.description.as_deref(),
            Some("First line\nSecond line")
        );
        assert!(extracted.effective.original_url.is_none());

        let mut request = library_query_request();
        request.series = vec!["current SERIES".to_owned()];
        request.sites = vec!["youtube".to_owned()];
        assert_eq!(
            database
                .query_library_items(request)
                .expect("series and site filter")
                .items[0]
                .id,
            recent_id
        );

        let mut published_this_year = library_query_request();
        published_this_year.date_published = Some("year".to_owned());
        assert_eq!(
            database
                .query_library_items(published_this_year)
                .expect("published date filter")
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![recent_id.as_str()]
        );

        let mut newest = library_query_request();
        newest.sort = LibrarySort::PublishedDesc;
        assert_eq!(
            database
                .query_library_items(newest)
                .expect("newest published")
                .items[0]
                .id,
            recent_id
        );
        let mut oldest = library_query_request();
        oldest.sort = LibrarySort::PublishedAsc;
        assert_eq!(
            database
                .query_library_items(oldest)
                .expect("oldest published")
                .items[0]
                .id,
            old_id
        );

        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("rich facets");
        assert_eq!(facets.series, vec!["Archive series", "Current series"]);
        assert_eq!(facets.sites, vec!["Vimeo", "YouTube"]);

        let overridden = database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: recent_id.clone(),
                creator: Some("Personal label".to_owned()),
                description: None,
                upload_date: None,
                series: None,
                episode: None,
                original_url: None,
            })
            .expect("creator override");
        assert_eq!(
            overridden.effective.creator.as_deref(),
            Some("Personal label")
        );
        let reset = database
            .reset_library_rich_metadata(&recent_id)
            .expect("reset override");
        assert_eq!(reset.effective.creator.as_deref(), Some("Current creator"));
        assert!(reset.overridden_fields.is_empty());

        let invalid_date = database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: recent_id.clone(),
                creator: None,
                description: None,
                upload_date: Some("2026-99-99".to_owned()),
                series: None,
                episode: None,
                original_url: None,
            })
            .expect_err("invalid upload date");
        assert_eq!(invalid_date.code, "invalid_library_metadata");

        let invalid_url = database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: recent_id.clone(),
                creator: None,
                description: None,
                upload_date: None,
                series: None,
                episode: None,
                original_url: Some("file:///tmp/private.mp4".to_owned()),
            })
            .expect_err("invalid original URL");
        assert_eq!(invalid_url.code, "invalid_library_metadata");

        let invalid_artwork = database
            .set_library_artwork(&recent_id, Some("relative-cover.jpg".to_owned()))
            .expect_err("relative artwork path");
        assert_eq!(invalid_artwork.code, "invalid_library_artwork");
    }

    #[test]
    fn indexes_search_saves_dynamic_views_and_recovers_drift() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("smart-discovery.sqlite3")).expect("database");
        let first_id = Uuid::new_v4().to_string();
        let second_id = Uuid::new_v4().to_string();
        let third_id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp_millis();
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, title, site, tags, relative_path) in [
                (
                    &first_id,
                    "Quiet ocean",
                    "YouTube",
                    r#"["ambient"]"#,
                    "Nature/ocean.mp4",
                ),
                (
                    &second_id,
                    "Forest walk",
                    "Vimeo",
                    r#"["nature"]"#,
                    "Nature/forest.mp4",
                ),
                (
                    &third_id,
                    "Wildlife documentary",
                    "YouTube",
                    r#"["documentary"]"#,
                    "Documentaries/wildlife.mp4",
                ),
            ] {
                connection
                    .execute(
                        "INSERT INTO media_library
                         (id, path, relative_path, title, site, tags_json, metadata_json, missing, added_at, updated_at, media_kind)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}', 0, ?7, ?7, 'video')",
                        params![
                            id,
                            directory.path().join(relative_path).to_string_lossy(),
                            relative_path,
                            title,
                            site,
                            tags,
                            now
                        ],
                    )
                    .expect("catalog item");
            }
            save_extracted_library_metadata(
                &connection,
                &first_id,
                &serde_json::json!({
                    "uploader": "Calm creator",
                    "series": "Relaxation",
                    "description": "Deep blue water"
                }),
            )
            .expect("first search metadata");
            save_extracted_library_metadata(
                &connection,
                &second_id,
                &serde_json::json!({
                    "uploader": "Trail creator",
                    "series": "Relaxation",
                    "description": "Green woodland trail"
                }),
            )
            .expect("second search metadata");
        }

        for term in ["ocean", "deep water", "ambient", "Nature"] {
            let mut request = library_query_request();
            request.search = Some(term.to_owned());
            assert!(
                database
                    .query_library_items(request)
                    .expect("indexed search")
                    .items
                    .iter()
                    .any(|item| item.id == first_id),
                "search term {term} did not find the indexed item"
            );
        }

        database
            .save_library_rich_metadata(SaveLibraryRichMetadataRequest {
                item_id: first_id.clone(),
                creator: Some("Sleep channel".to_owned()),
                description: Some("Deep blue water".to_owned()),
                upload_date: None,
                series: Some("Relaxation".to_owned()),
                episode: None,
                original_url: None,
            })
            .expect("searchable override");
        let mut override_search = library_query_request();
        override_search.search = Some("sleep channel".to_owned());
        assert_eq!(
            database
                .query_library_items(override_search)
                .expect("updated rich search index")
                .items[0]
                .id,
            first_id
        );

        let mut metacharacters = library_query_request();
        metacharacters.search = Some("ocean*".to_owned());
        assert_eq!(
            database
                .query_library_items(metacharacters)
                .expect("quoted FTS metacharacters")
                .items[0]
                .id,
            first_id
        );

        let mut multi_creator = library_query_request();
        multi_creator.creators = vec!["SLEEP CHANNEL".to_owned(), "Trail creator".to_owned()];
        assert_eq!(
            database
                .query_library_items(multi_creator)
                .expect("multi creator OR filter")
                .total_items,
            2
        );

        let smart_query = LibrarySmartViewQuery {
            search: None,
            source_id: None,
            folder_source_id: None,
            folder_path: None,
            folder_scope: LibraryFolderScope::Tree,
            media_kind: Some(LibraryMediaKind::Video),
            resolution: None,
            container: None,
            duration: None,
            date_added: None,
            date_published: None,
            availability: LibraryAvailabilityFilter::Available,
            tags: Vec::new(),
            creators: Vec::new(),
            series: vec![" Relaxation ".to_owned(), "relaxation".to_owned()],
            sites: Vec::new(),
            theme: None,
            rating: None,
            collection_id: None,
            favorite_only: false,
            private_only: false,
            sort: LibrarySort::Recent,
        };
        let smart_view = database
            .save_library_smart_view(SaveLibrarySmartViewRequest {
                id: None,
                name: "Relaxation videos".to_owned(),
                query: smart_query.clone(),
            })
            .expect("save smart view");
        assert_eq!(
            database
                .list_library_smart_views()
                .expect("smart views")
                .len(),
            1
        );
        assert_eq!(smart_view.query.series, ["Relaxation"]);
        assert_eq!(
            database
                .query_library_items(library_query_from_smart_view(&smart_view.query, 1, 60))
                .expect("dynamic smart view")
                .total_items,
            2
        );
        let duplicate = database
            .save_library_smart_view(SaveLibrarySmartViewRequest {
                id: None,
                name: "relaxation VIDEOS".to_owned(),
                query: smart_query,
            })
            .expect_err("duplicate smart view name");
        assert_eq!(duplicate.code, "duplicate_library_smart_view");
        let mut oversized_query = smart_view.query.clone();
        oversized_query.creators = (0..21).map(|index| format!("Creator {index}")).collect();
        let invalid_query = database
            .save_library_smart_view(SaveLibrarySmartViewRequest {
                id: None,
                name: "Too many creators".to_owned(),
                query: oversized_query,
            })
            .expect_err("oversized smart view filter");
        assert_eq!(invalid_query.code, "invalid_library_filter");

        let related = database
            .related_library_items(&first_id, 2)
            .expect("related media");
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].id, second_id);
        assert_eq!(related[1].id, third_id);

        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute("DELETE FROM library_search WHERE item_id = ?1", [&first_id])
                .expect("damage search index");
        }
        assert!(
            !database
                .library_search_status()
                .expect("damaged status")
                .consistent
        );
        let rebuilt = database
            .rebuild_library_search_index()
            .expect("rebuild search index");
        assert!(rebuilt.consistent);
        assert_eq!(rebuilt.catalog_items, 3);
        assert_eq!(rebuilt.indexed_items, 3);

        database
            .delete_library_smart_view(&smart_view.id)
            .expect("delete smart view");
        assert!(database
            .list_library_smart_views()
            .expect("smart views")
            .is_empty());
    }

    #[test]
    fn summarizes_library_health_without_persisting_derived_state() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("health.sqlite3")).expect("database");
        let healthy = media_stack_test_item(directory.path(), "healthy", LibraryMediaKind::Video);
        let mut risky = media_stack_test_item(directory.path(), "risky", LibraryMediaKind::Video);
        risky.video_codec = Some("av1".to_owned());
        let mut pending =
            media_stack_test_item(directory.path(), "pending", LibraryMediaKind::Image);
        pending.metadata_state = LibraryMetadataState::Pending;
        let mut failed = media_stack_test_item(directory.path(), "failed", LibraryMediaKind::Video);
        failed.metadata_state = LibraryMetadataState::Failed;
        failed.metadata_error = Some("fixture probe failure".to_owned());
        let mut missing =
            media_stack_test_item(directory.path(), "missing", LibraryMediaKind::Image);
        missing.missing = true;
        for item in [&healthy, &risky, &pending, &failed, &missing] {
            database.upsert_library_item(item).expect("library item");
        }
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO library_changes (id, item_id, kind, detected_at) VALUES (?1, ?2, 'modified', ?3)",
                    params![Uuid::new_v4().to_string(), healthy.id, Utc::now().timestamp_millis()],
                )
                .expect("pending change");
        }

        let summary = database.library_health_summary().expect("health summary");
        assert_eq!(summary.total_items, 5);
        assert_eq!(summary.available_items, 4);
        assert_eq!(summary.missing_items, 1);
        assert_eq!(summary.changed_items, 1);
        assert_eq!(summary.metadata_pending_items, 1);
        assert_eq!(summary.metadata_failed_items, 1);
        assert_eq!(summary.compatibility_risk_items, 1);
        assert!(summary.issues.iter().any(|issue| {
            issue.item_id == risky.id
                && issue
                    .kinds
                    .contains(&LibraryHealthIssueKind::CompatibilityRisk)
        }));
        assert!(summary.issues.iter().any(|issue| {
            issue.item_id == healthy.id && issue.kinds.contains(&LibraryHealthIssueKind::Changed)
        }));
    }

    #[test]
    fn disabling_library_privacy_requires_passphrase_and_preserves_rules() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("disable-privacy.sqlite3")).expect("database");
        let private_item = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Retained private video",
                LibraryMediaKind::Video,
            ))
            .expect("private item");

        database
            .configure_library_privacy("correct horse battery staple", 15)
            .expect("configure privacy");
        database
            .set_library_items_private(vec![private_item.id.clone()], true)
            .expect("private item rule");
        database.lock_library_privacy().expect("lock privacy");

        assert_eq!(
            database
                .disable_library_privacy("wrong passphrase")
                .expect_err("wrong passphrase is rejected")
                .code,
            "library_privacy_disable_failed"
        );
        let still_configured = database
            .library_privacy_status()
            .expect("configured status");
        assert!(still_configured.configured);
        assert!(still_configured.locked);

        let disabled = database
            .disable_library_privacy("correct horse battery staple")
            .expect("disable privacy");
        assert!(!disabled.configured);
        assert!(!disabled.locked);
        assert_eq!(disabled.private_item_count, 1);
        assert_eq!(
            database
                .list_library_privacy_rules()
                .expect("retained privacy rules")
                .item_ids,
            vec![private_item.id.clone()]
        );
        assert_eq!(
            database.list_library_items().expect("visible items").len(),
            1
        );

        database
            .configure_library_privacy("replacement passphrase", 30)
            .expect("enable privacy again");
        database.lock_library_privacy().expect("lock privacy again");
        assert!(database
            .list_library_items()
            .expect("locked items")
            .is_empty());
    }

    #[test]
    fn locked_library_hides_private_sources_folders_items_and_activity() {
        let directory = tempdir().expect("temporary directory");
        let database_path = directory.path().join("privacy.sqlite3");
        let database = Database::open(&database_path).expect("database");
        let public_source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Visible source".to_owned(),
                path: directory.path().to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Video,
                recursive: true,
                watch_enabled: false,
                private: false,
                include_patterns: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .expect("public source");
        let private_source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Private source".to_owned(),
                path: directory
                    .path()
                    .join("private-source")
                    .to_string_lossy()
                    .into_owned(),
                media_kind: LibraryMediaKind::Video,
                recursive: true,
                watch_enabled: false,
                private: true,
                include_patterns: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .expect("private source");

        let public_item = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Public video",
                LibraryMediaKind::Video,
            ))
            .expect("public item");
        let explicit_private = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Explicit private",
                LibraryMediaKind::Video,
            ))
            .expect("explicit private item");
        let mut folder_private =
            media_stack_test_item(directory.path(), "Folder private", LibraryMediaKind::Video);
        folder_private.source_id = Some(public_source.id.clone());
        folder_private.relative_path = Some("secret/folder-private.mp4".to_owned());
        let folder_private = database
            .upsert_library_item(&folder_private)
            .expect("folder item");
        let mut source_private =
            media_stack_test_item(directory.path(), "Source private", LibraryMediaKind::Video);
        source_private.source_id = Some(private_source.id.clone());
        source_private.relative_path = Some("source-private.mp4".to_owned());
        let source_private = database
            .upsert_library_item(&source_private)
            .expect("source item");

        database
            .configure_library_privacy("correct horse battery staple", 15)
            .expect("configure privacy");
        database
            .set_library_items_private(vec![explicit_private.id.clone()], true)
            .expect("private item rule");
        database
            .set_library_folder_private(&public_source.id, "secret", true)
            .expect("private folder rule");
        for item in [&public_item, &explicit_private] {
            database
                .save_library_playback(SaveLibraryPlaybackRequest {
                    item_id: item.id.clone(),
                    position_seconds: 12.0,
                    duration_seconds: 30.0,
                    completed: false,
                })
                .expect("playback state");
            database
                .replace_library_transcript_cues(
                    &item.id,
                    &[LibraryTranscriptCue {
                        subtitle_name: "captions.vtt".to_owned(),
                        language: Some("en".to_owned()),
                        start_seconds: 1.0,
                        end_seconds: 2.0,
                        text: "shared privacy phrase".to_owned(),
                    }],
                )
                .expect("transcript cue");
        }

        database.lock_library_privacy().expect("lock privacy");
        let visible_ids = database
            .list_library_items()
            .expect("visible items")
            .into_iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>();
        assert_eq!(visible_ids, HashSet::from([public_item.id.clone()]));
        let query = database
            .query_library_items(library_query_request())
            .expect("private query");
        assert_eq!(
            query.total_items, 1,
            "query result should count only visible items"
        );
        assert_eq!(
            query.items.iter().map(|item| &item.id).collect::<Vec<_>>(),
            vec![&public_item.id]
        );
        let mut locked_private_request = library_query_request();
        locked_private_request.private_only = true;
        let locked_private_query = database
            .query_library_items(locked_private_request)
            .expect("locked private-only query");
        assert_eq!(locked_private_query.total_items, 0);
        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("private facets");
        assert_eq!(facets.total_items, 1);
        let health = database.library_health_summary().expect("private health");
        assert_eq!(health.total_items, 1);
        let by_ids = database
            .list_library_items_by_ids(vec![
                public_item.id.clone(),
                explicit_private.id.clone(),
                folder_private.id.clone(),
                source_private.id.clone(),
            ])
            .expect("items by id");
        assert_eq!(by_ids.len(), 1);
        assert_eq!(by_ids[0].id, public_item.id);
        assert_eq!(
            database
                .get_library_item(&explicit_private.id)
                .expect_err("private item hidden")
                .code,
            "library_item_not_found"
        );
        assert_eq!(
            database
                .list_library_sources()
                .expect("visible sources")
                .iter()
                .map(|source| source.id.as_str())
                .collect::<Vec<_>>(),
            vec![public_source.id.as_str()]
        );
        let playback = database
            .list_continue_watching(24)
            .expect("visible playback");
        assert_eq!(playback.len(), 1);
        assert_eq!(playback[0].item.id, public_item.id);
        let transcripts = database
            .search_library_transcripts("shared privacy", 10)
            .expect("visible transcripts");
        assert_eq!(transcripts.len(), 1);
        assert_eq!(transcripts[0].item_id, public_item.id);
        assert_eq!(
            database
                .unlock_library_privacy("wrong passphrase")
                .expect_err("wrong passphrase")
                .code,
            "library_privacy_unlock_failed"
        );
        database
            .unlock_library_privacy("correct horse battery staple")
            .expect("unlock privacy");
        assert_eq!(
            database.list_library_items().expect("unlocked items").len(),
            4
        );
        let mut unlocked_private_request = library_query_request();
        unlocked_private_request.private_only = true;
        let private_ids = database
            .query_library_items(unlocked_private_request)
            .expect("unlocked private-only query")
            .items
            .into_iter()
            .map(|item| item.id)
            .collect::<HashSet<_>>();
        assert_eq!(
            private_ids,
            HashSet::from([
                explicit_private.id.clone(),
                folder_private.id.clone(),
                source_private.id.clone(),
            ])
        );

        database
            .library_privacy_last_activity_ms
            .store(0, Ordering::SeqCst);
        assert!(database
            .expire_library_privacy_if_idle()
            .expect("expire privacy"));
        assert!(
            database
                .library_privacy_status()
                .expect("expired status")
                .locked
        );
        drop(database);

        let reopened = Database::open(&database_path).expect("reopen database");
        assert!(
            reopened
                .library_privacy_status()
                .expect("restart status")
                .locked
        );
        assert_eq!(
            reopened.list_library_items().expect("restart items").len(),
            1
        );
    }

    #[test]
    fn targets_bounded_theme_analysis_jobs_by_folder_and_items() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("theme-targets.sqlite3")).expect("database");
        let source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Imported videos".to_owned(),
                path: directory.path().to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: false,
                private: true,
                include_patterns: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .expect("library source");
        let add_item = |title: &str, relative_path: &str, kind: LibraryMediaKind| {
            let mut item = media_stack_test_item(directory.path(), title, kind);
            item.source_id = Some(source.id.clone());
            item.relative_path = Some(relative_path.to_owned());
            database.upsert_library_item(&item).expect("library item")
        };
        let root = add_item("Root video", "root.mp4", LibraryMediaKind::Video);
        let direct = add_item("Direct show", "shows/direct.mp4", LibraryMediaKind::Video);
        let nested = add_item(
            "Nested show",
            "shows/season-one/nested.mp4",
            LibraryMediaKind::Video,
        );
        let outside = add_item("Outside show", "other/outside.mp4", LibraryMediaKind::Video);
        let image = add_item("Poster", "shows/poster.jpg", LibraryMediaKind::Image);
        let mut missing = add_item("Missing show", "shows/missing.mp4", LibraryMediaKind::Video);
        missing.missing = true;
        database
            .upsert_library_item(&missing)
            .expect("missing library item");
        let pending = add_item("Pending show", "shows/pending.mp4", LibraryMediaKind::Video);
        database
            .reset_library_probe(&pending.id, 100, 2)
            .expect("pending metadata");

        let settings = LibraryThemeSettings {
            enabled: true,
            ..LibraryThemeSettings::default()
        };
        let all_request = StartLibraryThemeAnalysisRequest {
            scope: LibraryThemeAnalysisScope::All,
            max_videos: 2,
            source_id: None,
            folder_path: None,
            folder_scope: LibraryFolderScope::Tree,
            item_ids: Vec::new(),
        };
        let first_batch = database
            .list_theme_analysis_targets(&all_request, &settings)
            .expect("bounded first batch");
        assert_eq!(first_batch.len(), 2);

        let mut invalid_limit = all_request.clone();
        invalid_limit.max_videos = 501;
        assert!(database
            .list_theme_analysis_targets(&invalid_limit, &settings)
            .is_err());
        invalid_limit.max_videos = 0;
        assert!(database
            .list_theme_analysis_targets(&invalid_limit, &settings)
            .is_err());

        let mut folder_request = StartLibraryThemeAnalysisRequest {
            scope: LibraryThemeAnalysisScope::Folder,
            max_videos: 500,
            source_id: Some(source.id.clone()),
            folder_path: Some("shows".to_owned()),
            folder_scope: LibraryFolderScope::Direct,
            item_ids: Vec::new(),
        };
        let direct_targets = database
            .list_theme_analysis_targets(&folder_request, &settings)
            .expect("direct folder targets");
        assert_eq!(
            direct_targets
                .iter()
                .map(|target| target.id.as_str())
                .collect::<Vec<_>>(),
            vec![direct.id.as_str()]
        );
        folder_request.folder_path = Some(String::new());
        let root_targets = database
            .list_theme_analysis_targets(&folder_request, &settings)
            .expect("source root direct targets");
        assert_eq!(root_targets.len(), 1);
        assert_eq!(root_targets[0].id, root.id);
        folder_request.folder_path = Some("shows".to_owned());
        folder_request.folder_scope = LibraryFolderScope::Tree;
        let tree_ids = database
            .list_theme_analysis_targets(&folder_request, &settings)
            .expect("folder tree targets")
            .into_iter()
            .map(|target| target.id)
            .collect::<HashSet<_>>();
        assert_eq!(
            tree_ids,
            HashSet::from([direct.id.clone(), nested.id.clone()])
        );

        let item_request = StartLibraryThemeAnalysisRequest {
            scope: LibraryThemeAnalysisScope::Items,
            max_videos: 500,
            source_id: None,
            folder_path: None,
            folder_scope: LibraryFolderScope::Tree,
            item_ids: vec![
                nested.id.clone(),
                outside.id.clone(),
                image.id,
                missing.id,
                pending.id,
                nested.id.clone(),
            ],
        };
        let selected_ids = database
            .list_theme_analysis_targets(&item_request, &settings)
            .expect("selected item targets")
            .into_iter()
            .map(|target| target.id)
            .collect::<HashSet<_>>();
        assert_eq!(selected_ids, HashSet::from([nested.id, outside.id]));

        for target in &first_batch {
            database
                .save_theme_analysis(target, &settings, "Indexed.", 0.9, &[])
                .expect("save current analysis");
        }
        let second_batch = database
            .list_theme_analysis_targets(&all_request, &settings)
            .expect("bounded second batch");
        assert_eq!(second_batch.len(), 2);
        assert!(second_batch
            .iter()
            .all(|target| first_batch.iter().all(|first| first.id != target.id)));

        let mut invalid_scope = all_request;
        invalid_scope.source_id = Some(source.id);
        assert!(database
            .list_theme_analysis_targets(&invalid_scope, &settings)
            .is_err());
        let mut invalid_items = item_request;
        invalid_items.item_ids.clear();
        assert!(database
            .list_theme_analysis_targets(&invalid_items, &settings)
            .is_err());
        let invalid_folder = StartLibraryThemeAnalysisRequest {
            scope: LibraryThemeAnalysisScope::Folder,
            max_videos: 10,
            source_id: None,
            folder_path: Some("shows".to_owned()),
            folder_scope: LibraryFolderScope::Tree,
            item_ids: Vec::new(),
        };
        assert!(database
            .list_theme_analysis_targets(&invalid_folder, &settings)
            .is_err());
    }

    #[test]
    fn indexes_and_filters_creator_independent_video_themes() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("themes.sqlite3")).expect("database");
        let first = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Creator A gentle session",
                LibraryMediaKind::Video,
            ))
            .expect("first video");
        let second = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Creator B binaural sounds",
                LibraryMediaKind::Video,
            ))
            .expect("second video");
        let settings = LibraryThemeSettings {
            enabled: true,
            ..LibraryThemeSettings::default()
        };
        database
            .save_library_theme_settings(&settings)
            .expect("theme settings");
        let targets = database
            .list_theme_analysis_targets(
                &StartLibraryThemeAnalysisRequest {
                    scope: LibraryThemeAnalysisScope::All,
                    max_videos: 100,
                    source_id: None,
                    folder_path: None,
                    folder_scope: LibraryFolderScope::Tree,
                    item_ids: Vec::new(),
                },
                &settings,
            )
            .expect("theme targets");
        assert_eq!(targets.len(), 2);
        let themes = vec![
            LibraryThemeAssignment {
                key: "asmr".to_owned(),
                label: "ASMR".to_owned(),
                confidence: 0.98,
                evidence: vec!["Visual: binaural microphone setup".to_owned()],
            },
            LibraryThemeAssignment {
                key: "ear-massage".to_owned(),
                label: "Ear massage".to_owned(),
                confidence: 0.93,
                evidence: vec!["Visual: hands massage an ear microphone".to_owned()],
            },
        ];
        for target in &targets {
            database
                .save_theme_analysis(
                    target,
                    &settings,
                    "An ASMR ear-massage session.",
                    0.95,
                    &themes,
                )
                .expect("theme analysis");
        }
        let status = database
            .library_theme_index_status(&settings)
            .expect("theme status");
        assert_eq!(status.eligible_items, 2);
        assert_eq!(status.indexed_items, 2);
        assert_eq!(status.theme_count, 2);

        let mut request = empty_library_query_request(1, 60);
        request.theme = Some("ear-massage".to_owned());
        let result = database.query_library_items(request).expect("theme filter");
        assert_eq!(result.total_items, 2);
        assert!(result.items.iter().any(|item| item.id == first.id));
        assert!(result.items.iter().any(|item| item.id == second.id));
        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("theme facets");
        assert!(facets
            .themes
            .iter()
            .any(|theme| theme.key == "ear-massage" && theme.item_count == 2));
        let analyses = database
            .list_library_theme_analyses(10)
            .expect("theme evidence");
        assert_eq!(analyses.len(), 2);
        assert!(analyses.iter().all(|analysis| analysis
            .themes
            .iter()
            .any(|theme| theme.key == "ear-massage")));

        let fast_settings = LibraryThemeSettings {
            mode: LibraryThemeAnalysisMode::Fast,
            ..settings.clone()
        };
        database
            .save_library_theme_settings(&fast_settings)
            .expect("changed theme settings");
        assert_eq!(
            database
                .library_theme_index_status(&fast_settings)
                .expect("mode-stale theme status")
                .stale_items,
            2
        );
        assert!(database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("mode-stale facets")
            .themes
            .is_empty());
        database
            .save_library_theme_settings(&settings)
            .expect("restore theme settings");

        let mut changed = second.clone();
        changed.modified_at_ms = Some(changed.modified_at_ms.unwrap_or_default() + 1);
        database
            .upsert_library_item(&changed)
            .expect("changed video fingerprint");
        assert_eq!(
            database
                .library_theme_index_status(&settings)
                .expect("stale theme status")
                .stale_items,
            1
        );
    }

    #[test]
    fn indexes_fingerprint_aware_approximate_visual_similarity() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("visual-similarity.sqlite3")).expect("database");
        let first = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "First image",
                LibraryMediaKind::Image,
            ))
            .expect("first image");
        let second = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Similar image",
                LibraryMediaKind::Image,
            ))
            .expect("second image");
        let third = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Different image",
                LibraryMediaKind::Image,
            ))
            .expect("third image");
        let targets = database
            .list_visual_signature_targets_after(None, 100)
            .expect("targets");
        assert_eq!(targets.len(), 3);
        let by_id = targets
            .into_iter()
            .map(|target| (target.id.clone(), target))
            .collect::<HashMap<_, _>>();
        database
            .save_visual_signature(&by_id[&first.id], 0)
            .expect("first signature");
        database
            .save_visual_signature(&by_id[&second.id], 0b11)
            .expect("similar signature");
        database
            .save_visual_signature(&by_id[&third.id], u64::MAX)
            .expect("different signature");

        let status = database.visual_similarity_status().expect("status");
        assert_eq!(status.eligible_items, 3);
        assert_eq!(status.indexed_items, 3);
        let matches = database
            .find_visually_similar_items(&first.id, 12, 90.0)
            .expect("similar items");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].item.id, second.id);
        assert_eq!(matches[0].hamming_distance, 2);

        let mut changed = second.clone();
        changed.modified_at_ms = Some(2);
        database
            .upsert_library_item(&changed)
            .expect("changed fingerprint");
        let status = database.visual_similarity_status().expect("stale status");
        assert_eq!(status.indexed_items, 2);
        assert_eq!(status.stale_items, 1);
        assert!(database
            .find_visually_similar_items(&first.id, 12, 90.0)
            .expect("stale item excluded")
            .is_empty());

        database
            .remove_library_item(&first.id)
            .expect("remove first image");
        assert_eq!(
            database
                .visual_similarity_status()
                .expect("cascade status")
                .eligible_items,
            2
        );
    }

    #[test]
    fn near_duplicate_review_combines_visual_duration_and_exact_hash_evidence() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("near-duplicates.sqlite3")).expect("database");
        let first = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Original video",
                LibraryMediaKind::Video,
            ))
            .expect("first video");
        let mut second_input = media_stack_test_item(
            directory.path(),
            "Re-encoded video",
            LibraryMediaKind::Video,
        );
        second_input.duration_seconds = Some(32.0);
        let second = database
            .upsert_library_item(&second_input)
            .expect("second video");
        let third = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Exact copy",
                LibraryMediaKind::Video,
            ))
            .expect("third video");
        let targets = database
            .list_visual_signature_targets_after(None, 100)
            .expect("signature targets")
            .into_iter()
            .map(|target| (target.id.clone(), target))
            .collect::<HashMap<_, _>>();
        database
            .save_visual_signature(&targets[&first.id], 0)
            .expect("first signature");
        database
            .save_visual_signature(&targets[&second.id], 1)
            .expect("second signature");
        database
            .save_visual_signature(&targets[&third.id], 0)
            .expect("third signature");
        database
            .save_library_content_hash(&first.id, 100, 1, &"a".repeat(64))
            .expect("first hash");
        database
            .save_library_content_hash(&second.id, 100, 1, &"b".repeat(64))
            .expect("second hash");
        database
            .save_library_content_hash(&third.id, 100, 1, &"a".repeat(64))
            .expect("third hash");

        let matches = database
            .find_near_duplicate_videos(90.0, 5.0, 20)
            .expect("near duplicate review");
        let contains_pair = |candidate: &LibraryNearDuplicateMatch, left: &str, right: &str| {
            [candidate.left.id.as_str(), candidate.right.id.as_str()].contains(&left)
                && [candidate.left.id.as_str(), candidate.right.id.as_str()].contains(&right)
        };
        assert!(matches
            .iter()
            .any(|candidate| contains_pair(candidate, &first.id, &second.id)
                && candidate.exact_hash_compared));
        assert!(!matches
            .iter()
            .any(|candidate| contains_pair(candidate, &first.id, &third.id)));
    }

    #[test]
    fn indexes_replaces_searches_and_cascades_library_transcript_cues() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("transcripts.sqlite3")).expect("database");
        let item = media_stack_test_item(
            directory.path(),
            "Transcript fixture",
            LibraryMediaKind::Video,
        );
        let item = database
            .upsert_scanned_library_item(&item)
            .expect("catalog video");

        database
            .replace_library_transcript_cues(
                &item.id,
                &[
                    LibraryTranscriptCue {
                        subtitle_name: "fixture.en.vtt".to_owned(),
                        language: Some("en".to_owned()),
                        start_seconds: 1.25,
                        end_seconds: 3.5,
                        text: "Aggressive tapping and face mask reveal".to_owned(),
                    },
                    LibraryTranscriptCue {
                        subtitle_name: "fixture.en.vtt".to_owned(),
                        language: Some("en".to_owned()),
                        start_seconds: 5.0,
                        end_seconds: 8.0,
                        text: "Whispering ocean sounds".to_owned(),
                    },
                ],
            )
            .expect("index cues");

        let status = database
            .library_transcript_index_status()
            .expect("transcript status");
        assert_eq!(status.indexed_items, 1);
        assert_eq!(status.cue_count, 2);
        let matches = database
            .search_library_transcripts("face mask", 12)
            .expect("FTS transcript search");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].item_id, item.id);
        assert_eq!(matches[0].start_seconds, 1.25);
        assert_eq!(matches[0].language.as_deref(), Some("en"));

        database
            .replace_library_transcript_cues(
                &item.id,
                &[LibraryTranscriptCue {
                    subtitle_name: "fixture.vtt".to_owned(),
                    language: None,
                    start_seconds: 9.0,
                    end_seconds: 11.0,
                    text: "Soft rain on the window".to_owned(),
                }],
            )
            .expect("replace cues");
        assert!(database
            .search_library_transcripts("aggressive", 12)
            .expect("stale search")
            .is_empty());
        assert_eq!(
            database
                .search_library_transcripts("rain", 12)
                .expect("replacement search")
                .len(),
            1
        );
        assert_eq!(
            database
                .library_transcript_index_status()
                .expect("replacement status")
                .cue_count,
            1
        );

        let invalid_query = database
            .search_library_transcripts(&"x".repeat(201), 12)
            .expect_err("oversized transcript query");
        assert_eq!(invalid_query.code, "invalid_transcript_search");
        let invalid_limit = database
            .search_library_transcripts("rain", 0)
            .expect_err("invalid transcript limit");
        assert_eq!(invalid_limit.code, "invalid_transcript_search_limit");

        database
            .remove_library_item(&item.id)
            .expect("remove catalog item");
        let status = database
            .library_transcript_index_status()
            .expect("cascade status");
        assert_eq!(status.indexed_items, 0);
        assert_eq!(status.cue_count, 0);
    }

    #[test]
    fn persists_semantic_embeddings_and_invalidates_them_when_transcripts_change() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("semantic-transcripts.sqlite3"))
            .expect("database");
        let item = database
            .upsert_scanned_library_item(&media_stack_test_item(
                directory.path(),
                "Semantic transcript fixture",
                LibraryMediaKind::Video,
            ))
            .expect("catalog video");
        database
            .replace_library_transcript_cues(
                &item.id,
                &[LibraryTranscriptCue {
                    subtitle_name: "fixture.vtt".to_owned(),
                    language: Some("en".to_owned()),
                    start_seconds: 2.0,
                    end_seconds: 6.0,
                    text: "A quiet walk through a rainy city at night".to_owned(),
                }],
            )
            .expect("transcript cues");

        let documents = database
            .list_semantic_transcript_documents(std::slice::from_ref(&item.id), 10)
            .expect("semantic documents");
        assert_eq!(documents.len(), 1);
        let document = &documents[0];
        let vector_json = serde_json::to_string(&vec![0.25_f32; 8]).expect("vector json");
        database
            .save_semantic_transcript_embedding(document, "fixture-embed", &vector_json)
            .expect("save embedding");
        assert!(database
            .semantic_transcript_embedding_is_current(document, "fixture-embed")
            .expect("current embedding"));
        let status = database
            .semantic_transcript_index_status("fixture-embed")
            .expect("semantic status");
        assert_eq!(status.indexed_items, 1);
        assert_eq!(status.stale_items, 0);

        database
            .replace_library_transcript_cues(
                &item.id,
                &[LibraryTranscriptCue {
                    subtitle_name: "fixture.vtt".to_owned(),
                    language: Some("en".to_owned()),
                    start_seconds: 8.0,
                    end_seconds: 12.0,
                    text: "The scene changed to a sunny beach".to_owned(),
                }],
            )
            .expect("replace transcript");
        assert!(database
            .list_semantic_transcript_embeddings("fixture-embed", 10)
            .expect("stored embeddings")
            .is_empty());
        let status = database
            .semantic_transcript_index_status("fixture-embed")
            .expect("stale semantic status");
        assert_eq!(status.indexed_items, 0);
        assert_eq!(status.stale_items, 1);
    }

    #[test]
    fn persists_watch_later_and_private_playback_history() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("watch-history.sqlite3")).expect("database");
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: directory
                .path()
                .join("movie.mp4")
                .to_string_lossy()
                .into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "Local movie".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: None,
            site: None,
            tags: Vec::new(),
            metadata: serde_json::json!({}),
            file_size_bytes: Some(1024),
            modified_at_ms: Some(1_700_000_000_000),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        database.upsert_library_item(&item).expect("library item");

        assert!(
            !database
                .get_library_watch_state(&item.id)
                .expect("initial watch state")
                .watch_later
        );
        assert!(
            database
                .set_library_watch_later(&item.id, true)
                .expect("save for later")
                .watch_later
        );
        assert_eq!(
            database.list_library_watch_later(12).expect("watch later")[0].id,
            item.id
        );

        let completed = database
            .set_library_watched(&item.id, true)
            .expect("mark watched");
        assert!(completed.playback.expect("playback state").completed);
        assert!(
            database
                .save_library_playback(SaveLibraryPlaybackRequest {
                    item_id: item.id.clone(),
                    position_seconds: 10.0,
                    duration_seconds: 120.0,
                    completed: false,
                })
                .expect("replayed progress")
                .completed
        );
        assert_eq!(
            database
                .list_recently_watched(12)
                .expect("recently watched")[0]
                .item
                .id,
            item.id
        );
        assert!(database
            .set_library_watched(&item.id, false)
            .expect("reset history")
            .playback
            .is_none());
        assert!(database
            .list_recently_watched(12)
            .expect("cleared recently watched")
            .is_empty());
    }

    #[test]
    fn persists_ordered_video_playlists_and_cascades_removed_items() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("playlists.sqlite3")).expect("database");
        let now = Utc::now();
        let first = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: directory
                .path()
                .join("first.mp4")
                .to_string_lossy()
                .into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "First".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(30.0),
            site: None,
            tags: Vec::new(),
            metadata: serde_json::json!({}),
            file_size_bytes: Some(100),
            modified_at_ms: Some(1),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        let mut second = first.clone();
        second.id = Uuid::new_v4().to_string();
        second.title = "Second".to_owned();
        second.path = directory
            .path()
            .join("second.mp4")
            .to_string_lossy()
            .into_owned();
        let mut image = first.clone();
        image.id = Uuid::new_v4().to_string();
        image.title = "Image".to_owned();
        image.path = directory
            .path()
            .join("image.jpg")
            .to_string_lossy()
            .into_owned();
        image.media_kind = Some(LibraryMediaKind::Image);
        database.upsert_library_item(&first).expect("first video");
        database.upsert_library_item(&second).expect("second video");
        database.upsert_library_item(&image).expect("image");

        let playlist = database
            .save_library_playlist(SaveLibraryPlaylistRequest {
                id: None,
                name: "Evening queue".to_owned(),
                description: "A deliberate order".to_owned(),
            })
            .expect("playlist");
        database
            .add_library_playlist_item(&playlist.id, &first.id)
            .expect("add first");
        database
            .add_library_playlist_item(&playlist.id, &second.id)
            .expect("add second");
        assert_eq!(
            database
                .list_library_playlist_items(&playlist.id)
                .expect("playlist items")
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![first.id.as_str(), second.id.as_str()]
        );
        assert_eq!(
            database
                .move_library_playlist_item(&playlist.id, &second.id, 0)
                .expect("move second")
                .item_ids,
            vec![second.id.clone(), first.id.clone()]
        );
        assert_eq!(
            database
                .add_library_playlist_item(&playlist.id, &image.id)
                .expect_err("images are rejected")
                .code,
            "library_playlist_video_required"
        );

        database
            .remove_library_item(&second.id)
            .expect("remove catalog item");
        assert_eq!(
            database.list_library_playlists().expect("playlists")[0].item_ids,
            vec![first.id]
        );
    }

    #[test]
    fn persists_validated_library_moments_and_cascades_with_video() {
        let directory = tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("moments.sqlite3")).expect("database");
        let now = Utc::now();
        let item = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: directory
                .path()
                .join("movie.mp4")
                .to_string_lossy()
                .into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "Movie".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(120.0),
            site: None,
            tags: Vec::new(),
            metadata: serde_json::json!({}),
            file_size_bytes: Some(100),
            modified_at_ms: Some(1),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        database.upsert_library_item(&item).expect("video item");

        let saved = database
            .save_library_moment(SaveLibraryMomentRequest {
                id: None,
                item_id: item.id.clone(),
                label: "Opening scene".to_owned(),
                note: "Compare the lighting.".to_owned(),
                start_seconds: 10.0,
                end_seconds: Some(18.5),
                color: "blue".to_owned(),
            })
            .expect("save moment");
        assert_eq!(
            database.list_library_moments(&item.id).expect("moments"),
            vec![saved.clone()]
        );

        let invalid = database
            .save_library_moment(SaveLibraryMomentRequest {
                id: None,
                item_id: item.id.clone(),
                label: "Invalid range".to_owned(),
                note: String::new(),
                start_seconds: 20.0,
                end_seconds: Some(19.0),
                color: "accent".to_owned(),
            })
            .expect_err("reversed range");
        assert_eq!(invalid.code, "invalid_library_moment_time");

        database
            .remove_library_item(&item.id)
            .expect("remove video");
        let connection = database.connection.lock().expect("database lock");
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM library_moments", [], |row| row.get(0))
            .expect("moment count");
        assert_eq!(remaining, 0);
    }

    #[test]
    fn persists_validated_montages_and_cascades_deleted_segments() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("montages.sqlite3")).expect("database");
        let item =
            media_stack_test_item(directory.path(), "Montage source", LibraryMediaKind::Video);
        let item = database.upsert_library_item(&item).expect("catalog video");
        let saved = database
            .save_library_montage(SaveLibraryMontageRequest {
                id: None,
                name: "Release cut".to_owned(),
                notes: "Opening and close".to_owned(),
                fade_seconds: 0.25,
                segments: vec![
                    LibraryMontageSegment {
                        item_id: item.id.clone(),
                        label: "Opening".to_owned(),
                        start_seconds: 1.0,
                        end_seconds: 5.0,
                    },
                    LibraryMontageSegment {
                        item_id: item.id.clone(),
                        label: "Close".to_owned(),
                        start_seconds: 20.0,
                        end_seconds: 25.0,
                    },
                ],
            })
            .expect("save montage");
        assert_eq!(saved.segments.len(), 2);
        assert_eq!(
            database.list_library_montages().expect("montages"),
            vec![saved.clone()]
        );

        let invalid = database
            .save_library_montage(SaveLibraryMontageRequest {
                id: None,
                name: "Invalid cut".to_owned(),
                notes: String::new(),
                fade_seconds: 0.0,
                segments: vec![LibraryMontageSegment {
                    item_id: item.id.clone(),
                    label: "Past duration".to_owned(),
                    start_seconds: 29.0,
                    end_seconds: 40.0,
                }],
            })
            .expect_err("reject range past duration");
        assert_eq!(invalid.code, "invalid_library_montage_range");

        database
            .remove_library_item(&item.id)
            .expect("remove video");
        let remaining = database
            .get_library_montage(&saved.id)
            .expect("project remains");
        assert!(remaining.segments.is_empty());
        assert!(remaining.incomplete);
        assert_eq!(remaining.expected_segment_count, 2);
        database
            .delete_library_montage(&saved.id)
            .expect("delete montage");
        assert!(database
            .list_library_montages()
            .expect("montages")
            .is_empty());
    }

    #[test]
    fn persists_validated_comparison_sessions_and_reconciles_deleted_videos() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("compare-sessions.sqlite3")).expect("database");
        let now = Utc::now();
        let first = LibraryItem {
            id: Uuid::new_v4().to_string(),
            path: directory
                .path()
                .join("first.mp4")
                .to_string_lossy()
                .into_owned(),
            source_id: None,
            relative_path: None,
            media_kind: Some(LibraryMediaKind::Video),
            title: "First".to_owned(),
            source_url: None,
            thumbnail: None,
            duration_seconds: Some(60.0),
            site: None,
            tags: Vec::new(),
            metadata: serde_json::json!({}),
            file_size_bytes: Some(100),
            modified_at_ms: Some(1),
            width: Some(1920),
            height: Some(1080),
            fps: Some(30.0),
            video_codec: Some("h264".to_owned()),
            audio_codec: Some("aac".to_owned()),
            container: Some("mp4".to_owned()),
            dynamic_range: None,
            metadata_state: LibraryMetadataState::Ready,
            metadata_error: None,
            favorite: false,
            rating: None,
            missing: false,
            added_at: now,
            updated_at: now,
        };
        let mut second = first.clone();
        second.id = Uuid::new_v4().to_string();
        second.title = "Second".to_owned();
        second.path = directory
            .path()
            .join("second.mp4")
            .to_string_lossy()
            .into_owned();
        database.upsert_library_item(&first).expect("first video");
        database.upsert_library_item(&second).expect("second video");

        let saved = database
            .save_library_compare_session(SaveLibraryCompareSessionRequest {
                id: None,
                name: "Lighting comparison".to_owned(),
                notes: "Compare both openings.".to_owned(),
                items: vec![
                    LibraryCompareSessionItem {
                        item_id: first.id.clone(),
                        position: 0,
                        playback_position_seconds: 4.0,
                        playback_rate: 1.0,
                    },
                    LibraryCompareSessionItem {
                        item_id: second.id.clone(),
                        position: 1,
                        playback_position_seconds: 8.0,
                        playback_rate: 0.75,
                    },
                ],
                active_audio_item_id: Some(second.id.clone()),
                multiple_audio: false,
            })
            .expect("save comparison");
        assert_eq!(
            database.list_library_compare_sessions().expect("sessions"),
            vec![saved.clone()]
        );

        database
            .remove_library_item(&second.id)
            .expect("remove second video");
        let reconciled = database
            .list_library_compare_sessions()
            .expect("reconciled sessions");
        assert_eq!(reconciled[0].items.len(), 1);
        assert!(reconciled[0].active_audio_item_id.is_none());
        database
            .delete_library_compare_session(&saved.id)
            .expect("delete session");
        assert!(database
            .list_library_compare_sessions()
            .expect("sessions")
            .is_empty());
    }

    #[test]
    fn persists_validated_media_stacks_and_cleans_incomplete_stacks() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("media-stacks.sqlite3")).expect("database");
        let first = media_stack_test_item(directory.path(), "Original", LibraryMediaKind::Image);
        let second = media_stack_test_item(directory.path(), "Edit", LibraryMediaKind::Image);
        let third = media_stack_test_item(directory.path(), "Alternate", LibraryMediaKind::Image);
        for item in [&first, &second, &third] {
            database.upsert_library_item(item).expect("library item");
        }

        let saved = database
            .save_library_media_stack(SaveLibraryMediaStackRequest {
                id: None,
                name: "Poster versions".to_owned(),
                notes: "Print revisions".to_owned(),
                cover_item_id: Some(second.id.clone()),
                members: vec![
                    LibraryMediaStackMember {
                        item_id: first.id.clone(),
                        position: 0,
                        role: LibraryMediaStackRole::Original,
                    },
                    LibraryMediaStackMember {
                        item_id: second.id.clone(),
                        position: 1,
                        role: LibraryMediaStackRole::Edit,
                    },
                ],
            })
            .expect("save media stack");
        assert_eq!(saved.cover_item_id.as_deref(), Some(second.id.as_str()));
        assert_eq!(saved.members.len(), 2);

        let conflict = database
            .save_library_media_stack(SaveLibraryMediaStackRequest {
                id: None,
                name: "Conflicting stack".to_owned(),
                notes: String::new(),
                cover_item_id: None,
                members: vec![
                    LibraryMediaStackMember {
                        item_id: second.id.clone(),
                        position: 0,
                        role: LibraryMediaStackRole::Original,
                    },
                    LibraryMediaStackMember {
                        item_id: third.id.clone(),
                        position: 1,
                        role: LibraryMediaStackRole::Alternate,
                    },
                ],
            })
            .expect_err("single stack membership");
        assert_eq!(conflict.code, "media_stack_item_in_use");

        database
            .remove_library_item(&first.id)
            .expect("remove original");
        assert!(database
            .list_library_media_stacks()
            .expect("media stacks")
            .is_empty());
    }

    #[test]
    fn persists_curation_decisions_and_builds_deterministic_queues() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("curation.sqlite3")).expect("database");
        let first = media_stack_test_item(directory.path(), "First", LibraryMediaKind::Video);
        let second = media_stack_test_item(directory.path(), "Second", LibraryMediaKind::Video);
        database.upsert_library_item(&first).expect("first item");
        database.upsert_library_item(&second).expect("second item");

        assert_eq!(
            database
                .list_library_curation_queue(false, 100)
                .expect("unreviewed")
                .len(),
            2
        );
        database
            .set_library_curation_decision(&first.id, Some(LibraryCurationDecision::Keep))
            .expect("keep decision");
        database
            .set_library_curation_decision(&second.id, Some(LibraryCurationDecision::ReviewLater))
            .expect("review later decision");
        assert!(database
            .list_library_curation_queue(false, 100)
            .expect("unreviewed")
            .is_empty());
        assert_eq!(
            database
                .list_library_curation_queue(true, 100)
                .expect("review later")
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![second.id.as_str()]
        );
        database
            .set_library_curation_decision(&second.id, None)
            .expect("clear decision");
        assert_eq!(
            database
                .list_library_curation_queue(false, 100)
                .expect("unreviewed")
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![second.id.as_str()]
        );
    }

    #[test]
    fn round_trips_library_file_operation_history() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("operations.sqlite3")).expect("database");
        let now = Utc::now();
        let operation = LibraryFileOperation {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            output_item_id: Some(Uuid::new_v4().to_string()),
            kind: LibraryFileOperationKind::Copy,
            state: LibraryFileOperationState::Completed,
            source_path: directory
                .path()
                .join("source.mp4")
                .to_string_lossy()
                .into_owned(),
            destination_path: directory
                .path()
                .join("copy.mp4")
                .to_string_lossy()
                .into_owned(),
            source_size_bytes: 2048,
            source_modified_at_ms: 1_700_000_000_000,
            error: None,
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
        };
        database
            .save_library_file_operation(&operation)
            .expect("save operation");

        let stored = database
            .get_library_file_operation(&operation.id)
            .expect("get operation");
        assert_eq!(stored.kind, LibraryFileOperationKind::Copy);
        assert_eq!(stored.state, LibraryFileOperationState::Completed);
        assert_eq!(stored.output_item_id, operation.output_item_id);
        assert_eq!(stored.source_size_bytes, 2048);
        assert_eq!(
            database
                .list_library_file_operations()
                .expect("operation history")[0]
                .id,
            operation.id
        );

        let mut interrupted = operation.clone();
        interrupted.id = Uuid::new_v4().to_string();
        interrupted.state = LibraryFileOperationState::Executing;
        interrupted.output_item_id = None;
        interrupted.completed_at = None;
        database
            .save_library_file_operation(&interrupted)
            .expect("save interrupted operation");
        assert_eq!(
            database
                .reconcile_library_file_operations()
                .expect("reconcile operations"),
            1
        );
        let recovered = database
            .get_library_file_operation(&interrupted.id)
            .expect("recovered operation");
        assert_eq!(recovered.state, LibraryFileOperationState::Failed);
        assert!(recovered.error.is_some());
    }

    fn library_query_request() -> LibraryQueryRequest {
        LibraryQueryRequest {
            page: 1,
            page_size: 60,
            search: None,
            source_id: None,
            folder_source_id: None,
            folder_path: None,
            folder_scope: LibraryFolderScope::Tree,
            media_kind: None,
            resolution: None,
            container: None,
            duration: None,
            date_added: None,
            date_published: None,
            availability: LibraryAvailabilityFilter::All,
            tags: Vec::new(),
            creators: Vec::new(),
            series: Vec::new(),
            sites: Vec::new(),
            theme: None,
            rating: None,
            collection_id: None,
            favorite_only: false,
            private_only: false,
            sort: LibrarySort::Recent,
        }
    }

    #[test]
    fn queries_and_pages_a_library_larger_than_the_legacy_limit() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("large-library.sqlite3")).expect("database");
        let source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Large archive".to_owned(),
                path: directory.path().to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Both,
                recursive: true,
                watch_enabled: false,
                private: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
            })
            .expect("source");
        let collection = database
            .save_library_collection(SaveLibraryCollectionRequest {
                id: None,
                name: "Curated".to_owned(),
            })
            .expect("collection");
        let now = Utc::now().timestamp_millis();
        let mut ids = Vec::with_capacity(5_105);
        let mut missing_count = 0usize;
        let mut root_direct_count = 0usize;
        let mut shows_direct_count = 0usize;
        let mut shows_tree_count = 0usize;
        {
            let mut connection = database.connection.lock().expect("database lock");
            let transaction = connection.transaction().expect("transaction");
            {
                let mut insert_item = transaction
                    .prepare(
                        "INSERT INTO media_library
                         (id, path, title, duration_seconds, tags_json, metadata_json, missing,
                          added_at, updated_at, source_id, relative_path, media_kind, favorite, rating)
                         VALUES (?1, ?2, ?3, ?4, ?5, '{}', ?6, ?7, ?7, ?8, ?9, ?10, ?11, ?12)",
                    )
                    .expect("item statement");
                let mut insert_metadata = transaction
                    .prepare(
                        "INSERT INTO library_media_metadata
                         (item_id, file_size_bytes, modified_at_ms, width, height, fps,
                          video_codec, audio_codec, container, state, updated_at)
                         VALUES (?1, 1024, ?2, 1920, ?3, 30, 'h264', 'aac', ?4, 'ready', ?2)",
                    )
                    .expect("metadata statement");
                let mut insert_collection = transaction
                    .prepare(
                        "INSERT INTO library_collection_items (collection_id, item_id, added_at)
                         VALUES (?1, ?2, ?3)",
                    )
                    .expect("collection statement");
                for index in 0..5_105usize {
                    let id = Uuid::new_v4().to_string();
                    let is_special = index == 42;
                    let relative_path = if index % 3 == 0 {
                        root_direct_count += 1;
                        format!("root-{index}.mp4")
                    } else if index % 3 == 1 {
                        shows_direct_count += 1;
                        shows_tree_count += 1;
                        format!("shows/video-{index}.mp4")
                    } else {
                        shows_tree_count += 1;
                        format!("shows/season-1/video-{index}.mp4")
                    };
                    let missing = index % 100 == 0;
                    if missing {
                        missing_count += 1;
                    }
                    let title = if is_special {
                        "Special curated video".to_owned()
                    } else {
                        format!("Library item {index}")
                    };
                    let tags = if is_special { "[\"curated\"]" } else { "[]" };
                    let media_kind = if index >= 5_000 { "image" } else { "video" };
                    let height = if is_special || index % 2 == 0 {
                        1080
                    } else {
                        720
                    };
                    let container = if is_special || index % 2 == 0 {
                        "mp4"
                    } else {
                        "webm"
                    };
                    insert_item
                        .execute(params![
                            &id,
                            directory.path().join(&relative_path).to_string_lossy(),
                            title,
                            if is_special { 120.0 } else { 1_900.0 },
                            tags,
                            missing,
                            now + i64::try_from(index).expect("index"),
                            &source.id,
                            relative_path,
                            media_kind,
                            is_special,
                            if is_special { Some(5i64) } else { None },
                        ])
                        .expect("insert item");
                    insert_metadata
                        .execute(params![&id, now, height, container])
                        .expect("insert metadata");
                    if is_special {
                        insert_collection
                            .execute(params![&collection.id, &id, now])
                            .expect("insert collection item");
                    }
                    ids.push(id);
                }
            }
            transaction.commit().expect("commit catalog");
        }

        let first_page = database
            .query_library_items(library_query_request())
            .expect("first page");
        assert_eq!(first_page.total_items, 5_105);
        assert_eq!(first_page.items.len(), 60);
        assert_eq!(first_page.total_pages, 86);

        let mut title_ascending = library_query_request();
        title_ascending.sort = LibrarySort::TitleAsc;
        let title_ascending = database
            .query_library_items(title_ascending)
            .expect("title ascending");
        assert!(title_ascending
            .items
            .windows(2)
            .all(|pair| { pair[0].title.to_lowercase() <= pair[1].title.to_lowercase() }));

        let mut title_descending = library_query_request();
        title_descending.sort = LibrarySort::TitleDesc;
        let title_descending = database
            .query_library_items(title_descending)
            .expect("title descending");
        assert!(title_descending
            .items
            .windows(2)
            .all(|pair| { pair[0].title.to_lowercase() >= pair[1].title.to_lowercase() }));

        let mut shortest_first = library_query_request();
        shortest_first.sort = LibrarySort::DurationAsc;
        let shortest_first = database
            .query_library_items(shortest_first)
            .expect("shortest first");
        assert_eq!(shortest_first.items[0].duration_seconds, Some(120.0));

        let mut longest_first = library_query_request();
        longest_first.sort = LibrarySort::DurationDesc;
        let longest_first = database
            .query_library_items(longest_first)
            .expect("longest first");
        assert_eq!(longest_first.items[0].duration_seconds, Some(1_900.0));

        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE library_media_metadata SET file_size_bytes = 1 WHERE item_id = ?1",
                    [&ids[0]],
                )
                .expect("smallest item");
            connection
                .execute(
                    "UPDATE library_media_metadata SET file_size_bytes = 9999 WHERE item_id = ?1",
                    [&ids[1]],
                )
                .expect("largest item");
            connection
                .execute(
                    "UPDATE library_media_metadata SET modified_at_ms = ?2 WHERE item_id = ?1",
                    params![&ids[0], now - 1],
                )
                .expect("oldest file date");
            connection
                .execute(
                    "UPDATE library_media_metadata SET modified_at_ms = ?2 WHERE item_id = ?1",
                    params![&ids[1], now + 1],
                )
                .expect("newest file date");
        }

        let mut smallest_first = library_query_request();
        smallest_first.sort = LibrarySort::SizeAsc;
        let smallest_first = database
            .query_library_items(smallest_first)
            .expect("smallest first");
        assert_eq!(smallest_first.items[0].id, ids[0]);

        let mut largest_first = library_query_request();
        largest_first.sort = LibrarySort::SizeDesc;
        let largest_first = database
            .query_library_items(largest_first)
            .expect("largest first");
        assert_eq!(largest_first.items[0].id, ids[1]);

        let mut oldest_file_first = library_query_request();
        oldest_file_first.sort = LibrarySort::FileDateAsc;
        let oldest_file_first = database
            .query_library_items(oldest_file_first)
            .expect("oldest file first");
        assert_eq!(oldest_file_first.items[0].id, ids[0]);

        let mut newest_file_first = library_query_request();
        newest_file_first.sort = LibrarySort::FileDateDesc;
        let newest_file_first = database
            .query_library_items(newest_file_first)
            .expect("newest file first");
        assert_eq!(newest_file_first.items[0].id, ids[1]);

        let mut last_request = library_query_request();
        last_request.page = 86;
        let last_page = database
            .query_library_items(last_request)
            .expect("last page");
        assert_eq!(last_page.page, 86);
        assert_eq!(last_page.items.len(), 5);

        let mut root_direct = library_query_request();
        root_direct.folder_source_id = Some(source.id.clone());
        root_direct.folder_path = Some(String::new());
        root_direct.folder_scope = LibraryFolderScope::Direct;
        assert_eq!(
            database
                .query_library_items(root_direct)
                .expect("root direct")
                .total_items,
            root_direct_count
        );

        let mut shows_direct = library_query_request();
        shows_direct.folder_source_id = Some(source.id.clone());
        shows_direct.folder_path = Some("shows".to_owned());
        shows_direct.folder_scope = LibraryFolderScope::Direct;
        assert_eq!(
            database
                .query_library_items(shows_direct.clone())
                .expect("shows direct")
                .total_items,
            shows_direct_count
        );
        shows_direct.folder_scope = LibraryFolderScope::Tree;
        assert_eq!(
            database
                .query_library_items(shows_direct)
                .expect("shows tree")
                .total_items,
            shows_tree_count
        );

        let mut missing = library_query_request();
        missing.availability = LibraryAvailabilityFilter::Missing;
        assert_eq!(
            database
                .query_library_items(missing)
                .expect("missing items")
                .total_items,
            missing_count
        );

        let mut combined = library_query_request();
        combined.search = Some("Special curated".to_owned());
        combined.media_kind = Some(LibraryMediaKind::Video);
        combined.resolution = Some("1080".to_owned());
        combined.container = Some("mp4".to_owned());
        combined.duration = Some("short".to_owned());
        combined.availability = LibraryAvailabilityFilter::Available;
        combined.tags = vec!["curated".to_owned()];
        combined.rating = Some("5".to_owned());
        combined.collection_id = Some(collection.id);
        combined.favorite_only = true;
        let combined = database
            .query_library_items(combined)
            .expect("combined filters");
        assert_eq!(combined.total_items, 1);
        assert_eq!(combined.items[0].title, "Special curated video");

        let selected = database
            .list_library_items_by_ids(vec![ids[2].clone(), ids[5_104].clone(), ids[2].clone()])
            .expect("selected item lookup");
        assert_eq!(selected.len(), 2);
        assert!(database
            .list_library_items_by_ids(ids[..501].to_vec())
            .is_err());

        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: Some(source.id.clone()),
                availability: LibraryAvailabilityFilter::All,
            })
            .expect("facets");
        assert_eq!(facets.total_items, 5_105);
        assert_eq!(facets.missing_items, missing_count);
        assert_eq!(facets.available_items, 5_105 - missing_count);
        assert_eq!(
            facets
                .folders
                .iter()
                .find(|folder| folder.source_id == source.id && folder.path == "shows")
                .map(|folder| (folder.direct_count, folder.tree_count)),
            Some((shows_direct_count, shows_tree_count))
        );
        assert!(facets.containers.iter().any(|value| value == "mp4"));
        assert!(facets.tags.iter().any(|value| value == "curated"));

        let playback = database
            .save_library_playback(SaveLibraryPlaybackRequest {
                item_id: ids[42].clone(),
                position_seconds: 30.0,
                duration_seconds: 120.0,
                completed: false,
            })
            .expect("save playback");
        assert!(!playback.completed);
        assert_eq!(
            database
                .get_library_playback(&ids[42])
                .expect("playback lookup")
                .expect("playback state")
                .position_seconds,
            30.0
        );
        let continuing = database.list_continue_watching(12).expect("continue shelf");
        assert_eq!(continuing.len(), 1);
        assert_eq!(continuing[0].item.id, ids[42]);
        let watch_state = database
            .set_library_watch_later(&ids[42], true)
            .expect("save for later");
        assert!(watch_state.watch_later);
        assert_eq!(
            database
                .list_library_watch_later(12)
                .expect("watch later shelf")[0]
                .id,
            ids[42]
        );
        let watched = database
            .set_library_watched(&ids[42], true)
            .expect("mark watched");
        assert!(watched.playback.expect("watched playback").completed);
        assert!(database
            .list_continue_watching(12)
            .expect("watched continue shelf")
            .is_empty());
        assert_eq!(
            database
                .list_recently_watched(12)
                .expect("recently watched shelf")[0]
                .item
                .id,
            ids[42]
        );
        let reset = database
            .set_library_watched(&ids[42], false)
            .expect("reset playback");
        assert!(reset.playback.is_none());
        assert!(database
            .list_recently_watched(12)
            .expect("reset recently watched shelf")
            .is_empty());
        database
            .save_library_playback(SaveLibraryPlaybackRequest {
                item_id: ids[42].clone(),
                position_seconds: 115.0,
                duration_seconds: 120.0,
                completed: false,
            })
            .expect("complete playback near end");
        assert!(database
            .list_continue_watching(12)
            .expect("completed shelf")
            .is_empty());
        assert!(database
            .save_library_playback(SaveLibraryPlaybackRequest {
                item_id: ids[42].clone(),
                position_seconds: f64::NAN,
                duration_seconds: 120.0,
                completed: false,
            })
            .is_err());
        assert!(
            !database
                .set_library_watch_later(&ids[42], false)
                .expect("remove from watch later")
                .watch_later
        );
        assert!(database
            .list_library_watch_later(12)
            .expect("empty watch later shelf")
            .is_empty());
    }

    #[test]
    #[ignore = "manual large-library profile; optionally set NICE_DOWNLOADER_LIBRARY_PROFILE_DIR"]
    fn profiles_large_library_queries_on_a_selected_filesystem() {
        let temporary_directory = tempdir().expect("temporary directory");
        let external_directory =
            std::env::var_os("NICE_DOWNLOADER_LIBRARY_PROFILE_DIR").map(std::path::PathBuf::from);
        let profile_directory = external_directory
            .as_deref()
            .unwrap_or_else(|| temporary_directory.path());
        assert!(
            profile_directory.is_dir(),
            "NICE_DOWNLOADER_LIBRARY_PROFILE_DIR must be an existing directory"
        );
        let item_count = std::env::var("NICE_DOWNLOADER_LIBRARY_PROFILE_ITEMS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(50_000);
        assert!(
            (10_000..=250_000).contains(&item_count),
            "profile item count must be between 10,000 and 250,000"
        );
        let path = profile_directory.join(format!(
            "nice-downloader-profile-{}.sqlite3",
            Uuid::new_v4()
        ));
        let opened_at = Instant::now();
        let database = Database::open(&path).expect("profile database");
        let open_elapsed = opened_at.elapsed();
        let source = database
            .save_library_source(SaveLibrarySourceRequest {
                id: None,
                name: "Profile archive".to_owned(),
                path: profile_directory.to_string_lossy().into_owned(),
                media_kind: LibraryMediaKind::Video,
                recursive: true,
                watch_enabled: false,
                private: false,
                include_patterns: vec![],
                exclude_patterns: vec![],
            })
            .expect("profile source");

        let seeded_at = Instant::now();
        {
            let mut connection = database.connection.lock().expect("database lock");
            let transaction = connection.transaction().expect("profile transaction");
            {
                let mut statement = transaction
                    .prepare(
                        "INSERT INTO media_library
                         (id, path, title, tags_json, metadata_json, missing, added_at,
                          updated_at, source_id, relative_path, media_kind, favorite)
                         VALUES (?1, ?2, ?3, ?4, '{}', 0, ?5, ?5, ?6, ?7, 'video', 0)",
                    )
                    .expect("profile insert statement");
                for index in 0..item_count {
                    let relative_path = format!(
                        "collection-{}/season-{}/video-{index}.mp4",
                        index % 100,
                        index % 12
                    );
                    let title = if index % 1_000 == 0 {
                        format!("Profile needle video {index}")
                    } else {
                        format!("Profile video {index}")
                    };
                    statement
                        .execute(params![
                            format!("00000000-0000-4000-8000-{index:012x}"),
                            profile_directory.join(&relative_path).to_string_lossy(),
                            title,
                            if index % 20 == 0 {
                                "[\"profile\"]"
                            } else {
                                "[]"
                            },
                            i64::try_from(index).expect("profile timestamp"),
                            &source.id,
                            relative_path,
                        ])
                        .expect("insert profile item");
                }
            }
            transaction.commit().expect("commit profile catalog");
        }
        let seed_elapsed = seeded_at.elapsed();
        let search_status = database
            .library_search_status()
            .expect("profile search index status");
        assert!(search_status.consistent);
        assert_eq!(search_status.catalog_items, item_count);
        assert_eq!(search_status.indexed_items, item_count);

        let first_page_at = Instant::now();
        let first_page = database
            .query_library_items(library_query_request())
            .expect("profile first page");
        let first_page_elapsed = first_page_at.elapsed();
        assert_eq!(first_page.total_items, item_count);
        assert_eq!(first_page.items.len(), 60);

        let search_at = Instant::now();
        let mut search_request = library_query_request();
        search_request.search = Some("Profile needle".to_owned());
        let search_result = database
            .query_library_items(search_request)
            .expect("profile search");
        let search_elapsed = search_at.elapsed();
        assert_eq!(search_result.total_items, item_count.div_ceil(1_000));

        let facets_at = Instant::now();
        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: Some(source.id),
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("profile facets");
        let facets_elapsed = facets_at.elapsed();
        assert_eq!(facets.total_items, item_count);
        assert!(facets.tags.iter().any(|tag| tag == "profile"));
        assert!(facets.folders.len() >= 100);

        let database_size = std::fs::metadata(&path)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        println!(
            "library-profile items={item_count} filesystem={} database={} bytes open_ms={} seed_ms={} first_page_ms={} search_ms={} facets_ms={}",
            profile_directory.display(),
            database_size,
            open_elapsed.as_millis(),
            seed_elapsed.as_millis(),
            first_page_elapsed.as_millis(),
            search_elapsed.as_millis(),
            facets_elapsed.as_millis(),
        );
        if external_directory.is_some() {
            println!("library-profile database retained at {}", path.display());
        }
    }

    #[test]
    fn returns_empty_library_facets_without_null_conversion_errors() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("empty-library.sqlite3")).expect("database");
        let facets = database
            .library_facets(LibraryFacetRequest {
                source_id: None,
                availability: LibraryAvailabilityFilter::Available,
            })
            .expect("empty facets");
        assert_eq!(facets.total_items, 0);
        assert_eq!(facets.available_items, 0);
        assert_eq!(facets.missing_items, 0);
        assert!(facets.containers.is_empty());
        assert!(facets.tags.is_empty());
        assert!(facets.folders.is_empty());
    }

    #[test]
    fn persists_validates_protects_and_cascades_video_display_settings() {
        let directory = tempdir().expect("temporary directory");
        let database =
            Database::open(&directory.path().join("video-display.sqlite3")).expect("database");
        let item = database
            .upsert_library_item(&media_stack_test_item(
                directory.path(),
                "Display settings",
                LibraryMediaKind::Video,
            ))
            .expect("library video");
        let settings = crate::models::LibraryVideoDisplaySettings {
            fit: crate::models::LibraryVideoFit::Fill,
            zoom: 1.5,
            pan_x: 0.25,
            pan_y: -0.2,
            rotation: 90,
            flip_horizontal: true,
            flip_vertical: false,
            brightness: 12.0,
            contrast: -8.0,
            saturation: 20.0,
            gamma: 5.0,
        };
        let saved = database
            .save_library_video_display_settings(SaveLibraryVideoDisplaySettingsRequest {
                item_id: item.id.clone(),
                settings,
            })
            .expect("save display settings");
        assert_eq!(saved, settings);
        assert_eq!(
            database
                .get_library_video_display_settings(&item.id)
                .expect("load display settings"),
            Some(settings)
        );

        let invalid = crate::models::LibraryVideoDisplaySettings {
            zoom: 3.01,
            ..settings
        };
        assert_eq!(
            database
                .save_library_video_display_settings(SaveLibraryVideoDisplaySettingsRequest {
                    item_id: item.id.clone(),
                    settings: invalid,
                })
                .expect_err("reject invalid zoom")
                .code,
            "invalid_library_video_display"
        );

        database
            .configure_library_privacy("correct horse battery staple", 15)
            .expect("configure privacy");
        database
            .set_library_items_private(vec![item.id.clone()], true)
            .expect("mark private");
        database.lock_library_privacy().expect("lock privacy");
        assert_eq!(
            database
                .get_library_video_display_settings(&item.id)
                .expect("locked lookup"),
            None
        );
        assert_eq!(
            database
                .forget_library_video_display_settings(&item.id)
                .expect_err("locked delete")
                .code,
            "library_locked"
        );
        database
            .unlock_library_privacy("correct horse battery staple")
            .expect("unlock privacy");
        database.remove_library_item(&item.id).expect("remove item");
        assert_eq!(
            database
                .get_library_video_display_settings(&item.id)
                .expect("cascaded lookup"),
            None
        );
    }
}
