use crate::{
    errors::AppError,
    models::{
        AuthenticationProfile, BrowserCookieSource, DesktopPreferences, DownloadArchiveMatch,
        DownloadJob, DownloadState, EnginePreferences, LibraryAvailabilityFilter,
        LibraryBulkAction, LibraryChange, LibraryChangeKind, LibraryCollection,
        LibraryFacetRequest, LibraryFacets, LibraryFileIdentity, LibraryFileOperation,
        LibraryFileOperationKind, LibraryFileOperationState, LibraryFolderFacet,
        LibraryFolderScope, LibraryItem, LibraryMediaKind, LibraryMetadataState,
        LibraryPlaybackEntry, LibraryPlaybackState, LibraryProbeResult, LibraryQueryRequest,
        LibraryQueryResult, LibraryScanJob, LibraryScanState, LibrarySource, LibrarySourceStatus,
        Preset, PresetIntent, QueueSettings, SaveAuthenticationProfileRequest,
        SaveLibraryCollectionRequest, SaveLibraryPlaybackRequest, SaveLibrarySourceRequest,
        SavePresetRequest, SaveSitePresetRuleRequest, SaveSubscriptionRequest, SitePresetRule,
        StartDownloadRequest, Subscription, TransformationOperation, TransformationState,
        TransformationTask, UpdateLibraryItemRequest,
    },
    utils::validation::{
        normalize_engine_preferences, validate_destination, validate_engine_preferences,
        validate_filename_template, validate_media_url,
    },
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
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
const ENGINE_PREFERENCES_KEY: &str = "engine.preferences";
const QUEUE_SETTINGS_KEY: &str = "queue.settings";
const DESKTOP_PREFERENCES_KEY: &str = "desktop.preferences";
const ONBOARDING_COMPLETED_KEY: &str = "onboarding.completed";

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
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

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
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
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The download history database is unavailable.",
            )
        })?;
        connection.execute(
            "INSERT OR REPLACE INTO downloads (
                id, url, title, thumbnail, state, progress_percent, downloaded_bytes,
                total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                output_format, destination, output_path, current_operation, error,
                created_at, updated_at, completed_at, options_json, request_json,
                diagnostics_json, attempt_count, retry_of_id, visible_in_downloads
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24,
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
            ],
        )?;
        if job.state == DownloadState::Completed {
            connection.execute(
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
                connection.execute(
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
                    retry_of_id, request_json
             FROM downloads
             WHERE state != 'Completed' AND request_json IS NOT NULL AND visible_in_downloads = 1
             ORDER BY created_at ASC LIMIT 500",
        )?;
        let rows = statement.query_map([], |row| {
            let request_json: String = row.get(22)?;
            Ok((job_from_row(row)?, request_json))
        })?;
        let mut jobs = Vec::new();
        for row in rows {
            let (job, request_json) = row?;
            let request = serde_json::from_str(&request_json).map_err(|error| {
                AppError::internal("could not deserialize a persisted download request", error)
            })?;
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
                    retry_of_id
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
                    retry_of_id FROM downloads ORDER BY updated_at DESC LIMIT 100",
        )?;
        let rows = statement.query_map([], job_from_row)?;
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
                thumbnail = COALESCE(excluded.thumbnail, media_library.thumbnail),
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
                thumbnail = COALESCE(excluded.thumbnail, media_library.thumbnail),
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
        drop(connection);
        self.get_library_item(&item_id)
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT media_library.id, path, title, source_url, thumbnail, media_library.duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             ORDER BY added_at DESC LIMIT 5000",
        )?;
        let rows = statement.query_map([], library_item_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        let mut statement = connection.prepare(
            "SELECT media_library.id, path, title, source_url, thumbnail, media_library.duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             ORDER BY added_at DESC LIMIT ?1",
        )?;
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
        let (where_clause, parameters) = build_library_query(&request)?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let total_items: i64 = connection.query_row(
            &format!("SELECT COUNT(*) FROM media_library LEFT JOIN library_media_metadata ON item_id = media_library.id {where_clause}"),
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
        let sql = format!(
            "SELECT media_library.id, path, title, source_url, thumbnail, duration_seconds, site,
                    tags_json, metadata_json, missing, added_at, media_library.updated_at, source_id,
                    relative_path, media_kind, file_size_bytes, modified_at_ms, width, height, fps,
                    video_codec, audio_codec, container, dynamic_range,
                    COALESCE(state, 'pending'), error, favorite, rating
             FROM media_library
             LEFT JOIN library_media_metadata ON item_id = media_library.id
             {where_clause}
             ORDER BY added_at DESC, media_library.id DESC
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
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let (total_items, available_items, missing_items): (i64, i64, i64) = connection.query_row(
            "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN missing = 0 THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN missing = 1 THEN 1 ELSE 0 END), 0)
                 FROM media_library",
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
                 WHERE container IS NOT NULL AND container != ''{availability_condition}
                 ORDER BY container COLLATE NOCASE"
            ))?;
            let values = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
            values
        };
        let tags = {
            let tag_where = match request.availability {
                LibraryAvailabilityFilter::All => "",
                LibraryAvailabilityFilter::Available => "WHERE media_library.missing = 0",
                LibraryAvailabilityFilter::Missing => "WHERE media_library.missing = 1",
            };
            let mut statement = connection.prepare(&format!(
                "SELECT DISTINCT json_each.value
                 FROM media_library, json_each(media_library.tags_json)
                 {tag_where}
                 ORDER BY json_each.value COLLATE NOCASE"
            ))?;
            let values = statement
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<String>, _>>()?;
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
        if let Some(source_id) = request.source_id {
            folder_sql.push_str(" AND source_id = ?1");
            folder_parameters.push(SqlValue::Text(source_id));
        }
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
             WHERE media_library.id IN ({placeholders})"
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
        if !request.position_seconds.is_finite()
            || !request.duration_seconds.is_finite()
            || request.position_seconds < 0.0
            || request.duration_seconds <= 0.0
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
                completed = excluded.completed,
                updated_at = excluded.updated_at",
            params![
                request.item_id,
                request.position_seconds,
                request.duration_seconds,
                completed,
                now
            ],
        )?;
        Ok(LibraryPlaybackState {
            item_id: request.item_id,
            position_seconds: request.position_seconds,
            duration_seconds: request.duration_seconds,
            completed,
            updated_at: timestamp_or_epoch(now),
        })
    }

    pub fn list_continue_watching(
        &self,
        limit: usize,
    ) -> Result<Vec<LibraryPlaybackEntry>, AppError> {
        if !(1..=24).contains(&limit) {
            return Err(AppError::new(
                "invalid_library_playback_limit",
                "Continue Watching can contain between 1 and 24 items.",
            ));
        }
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
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
             ORDER BY library_playback.updated_at DESC
             LIMIT ?1",
        )?;
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

    pub fn get_library_item(&self, id: &str) -> Result<LibraryItem, AppError> {
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
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "Duplicate analysis is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT library_content_hashes.item_id, library_content_hashes.file_size_bytes, sha256
             FROM library_content_hashes
             JOIN library_media_metadata ON library_media_metadata.item_id = library_content_hashes.item_id
             JOIN media_library ON media_library.id = library_content_hashes.item_id
             WHERE media_library.missing = 0
               AND library_media_metadata.file_size_bytes = library_content_hashes.file_size_bytes
               AND library_media_metadata.modified_at_ms = library_content_hashes.modified_at_ms",
        )?;
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
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        Ok(self
            .list_authentication_profiles()?
            .into_iter()
            .filter(|profile| {
                profile.enabled
                    && (host == profile.host_suffix
                        || host.ends_with(&format!(".{}", profile.host_suffix)))
            })
            .max_by_key(|profile| profile.host_suffix.len()))
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
            let placeholder = bind(SqlValue::Text(format!(
                "%{}%",
                escape_sql_like(&search.to_lowercase())
            )));
            clauses.push(format!(
                "LOWER(COALESCE(title, '') || ' ' || COALESCE(site, '') || ' ' || path || ' ' || tags_json || ' ' || COALESCE(container, '') || ' ' || COALESCE(video_codec, '') || ' ' || COALESCE(audio_codec, '')) LIKE {placeholder} ESCAPE '\\'"
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

    match request.availability {
        LibraryAvailabilityFilter::All => {}
        LibraryAvailabilityFilter::Available => clauses.push("missing = 0".to_owned()),
        LibraryAvailabilityFilter::Missing => clauses.push("missing = 1".to_owned()),
    }

    if let Some(tag) = request.tag.as_deref() {
        let tag = validate_library_filter_text(tag, "tag", 40)?;
        let placeholder = bind(SqlValue::Text(tag));
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM json_each(media_library.tags_json) WHERE json_each.value = {placeholder} COLLATE NOCASE)"
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

fn invalid_library_filter(label: &str) -> AppError {
    AppError::new(
        "invalid_library_filter",
        format!("The selected library {label} filter is invalid."),
    )
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
        attempt_count: u32::try_from(attempt_count).unwrap_or(1).max(1),
        retry_of_id: row.get(21)?,
        created_at: timestamp_or_epoch(created_at),
        updated_at: timestamp_or_epoch(updated_at),
        completed_at: completed_at.map(timestamp_or_epoch),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, DownloadOptions, MediaAnalysis, OutputConflictPolicy, OutputContainer,
        QualityKind, QualityOption, VideoCodecPreference,
    };
    use std::time::Instant;
    use tempfile::tempdir;

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
            attempt_count: 2,
            retry_of_id: Some("original".to_owned()),
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
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
        database.delete_preset(&preset.id).expect("delete preset");
        assert!(database.list_presets().expect("list presets").is_empty());
        assert!(database
            .list_site_preset_rules()
            .expect("cascade site rules")
            .is_empty());
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
        assert_eq!(latest_version, 15);
        let has_playback: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'library_playback')",
                [],
                |row| row.get(0),
            )
            .expect("playback table state");
        assert!(has_playback);
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
            availability: LibraryAvailabilityFilter::All,
            tag: None,
            rating: None,
            collection_id: None,
            favorite_only: false,
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
        combined.tag = Some("curated".to_owned());
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
}
