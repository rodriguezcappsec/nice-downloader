use crate::{
    errors::AppError,
    models::{
        DownloadJob, DownloadState, EnginePreferences, Preset, PresetIntent, QueueSettings,
        SavePresetRequest, StartDownloadRequest,
    },
    utils::validation::{normalize_engine_preferences, validate_engine_preferences},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

const MIGRATION_0001: &str = include_str!("migrations/0001_initial.sql");
const MIGRATION_0002: &str = include_str!("migrations/0002_durable_queue.sql");
const ENGINE_PREFERENCES_KEY: &str = "engine.preferences";
const QUEUE_SETTINGS_KEY: &str = "queue.settings";

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
        for (version, migration) in [(1, MIGRATION_0001), (2, MIGRATION_0002)] {
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
                diagnostics_json, attempt_count, retry_of_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
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
             WHERE state != 'Completed' AND request_json IS NOT NULL
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
            audio_quality: AudioQuality::Best,
            audio_only: false,
            subtitles: false,
            subtitle_languages: vec![],
            embed_thumbnail: false,
            embed_metadata: true,
            chapters: false,
            sponsorblock: false,
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
        let history = database.list_history().expect("list history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "Example");
        assert_eq!(history[0].diagnostics, ["finished"]);
        assert_eq!(history[0].attempt_count, 2);
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
    fn creates_lists_and_deletes_presets() {
        use crate::models::{
            AudioQuality, OutputConflictPolicy, OutputContainer, PresetIntent, SavePresetRequest,
            VideoCodecPreference,
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
                },
            })
            .expect("save preset");
        assert_eq!(preset.name, "Living room");
        assert_eq!(database.list_presets().expect("list presets").len(), 1);
        database.delete_preset(&preset.id).expect("delete preset");
        assert!(database.list_presets().expect("list presets").is_empty());
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
}
