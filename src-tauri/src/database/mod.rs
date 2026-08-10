use crate::{
    errors::AppError,
    models::{DownloadJob, DownloadOptions, DownloadState, EnginePreferences},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

const MIGRATION_0001: &str = include_str!("migrations/0001_initial.sql");
const ENGINE_PREFERENCES_KEY: &str = "engine.preferences";

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
        let migrated: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 1)",
            [],
            |row| row.get(0),
        )?;
        if !migrated {
            transaction.execute_batch(MIGRATION_0001)?;
            transaction.execute("INSERT INTO schema_migrations(version) VALUES (1)", [])?;
        }
        transaction
            .commit()
            .map_err(|error| AppError::internal("could not commit database migration", error))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn save_completed(
        &self,
        job: &DownloadJob,
        options: &DownloadOptions,
    ) -> Result<(), AppError> {
        let options_json = serde_json::to_string(options)
            .map_err(|error| AppError::internal("could not serialize download options", error))?;
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
                created_at, updated_at, completed_at, options_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                job.id,
                job.url,
                job.title,
                job.thumbnail,
                "Completed",
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
            ],
        )?;
        Ok(())
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
        match value {
            Some(value) => serde_json::from_str(&value)
                .map_err(|error| AppError::internal("could not read engine preferences", error)),
            None => Ok(EnginePreferences::default()),
        }
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

    pub fn list_history(&self) -> Result<Vec<DownloadJob>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new(
                "database_unavailable",
                "The download history database is unavailable.",
            )
        })?;
        let mut statement = connection.prepare(
            "SELECT id, url, title, thumbnail, progress_percent, downloaded_bytes,
                    total_bytes, speed_bytes_per_second, eta_seconds, selected_quality,
                    output_format, destination, output_path, current_operation, error,
                    created_at, updated_at, completed_at
             FROM downloads WHERE state = 'Completed'
             ORDER BY completed_at DESC LIMIT 500",
        )?;
        let rows = statement.query_map([], |row| {
            let created_at: i64 = row.get(15)?;
            let updated_at: i64 = row.get(16)?;
            let completed_at: Option<i64> = row.get(17)?;
            Ok(DownloadJob {
                id: row.get(0)?,
                url: row.get(1)?,
                title: row.get(2)?,
                thumbnail: row.get(3)?,
                state: DownloadState::Completed,
                progress_percent: row.get(4)?,
                downloaded_bytes: optional_u64(row.get(5)?),
                total_bytes: optional_u64(row.get(6)?),
                speed_bytes_per_second: row.get(7)?,
                eta_seconds: optional_u64(row.get(8)?),
                selected_quality: row.get(9)?,
                output_format: row.get(10)?,
                destination: row.get(11)?,
                output_path: row.get(12)?,
                current_operation: row.get(13)?,
                error: row.get(14)?,
                created_at: timestamp_or_epoch(created_at),
                updated_at: timestamp_or_epoch(updated_at),
                completed_at: completed_at.map(timestamp_or_epoch),
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
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
        AudioQuality, OutputContainer, QualityKind, QualityOption, VideoCodecPreference,
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
        };
        database
            .save_completed(&job, &options)
            .expect("save history");
        let history = database.list_history().expect("list history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "Example");
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
}
