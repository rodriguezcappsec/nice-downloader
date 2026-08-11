use crate::{
    errors::AppError,
    models::{
        AuthenticationProfile, BrowserCookieSource, DownloadArchiveMatch, DownloadJob,
        DownloadState, EnginePreferences, LibraryItem, Preset, PresetIntent, QueueSettings,
        SaveAuthenticationProfileRequest, SavePresetRequest, SaveSitePresetRuleRequest,
        SaveSubscriptionRequest, SitePresetRule, StartDownloadRequest, Subscription,
        TransformationOperation, TransformationState, TransformationTask, UpdateLibraryItemRequest,
    },
    utils::validation::{
        normalize_engine_preferences, validate_destination, validate_engine_preferences,
        validate_filename_template, validate_media_url,
    },
};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
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
        for (version, migration) in [
            (1, MIGRATION_0001),
            (2, MIGRATION_0002),
            (3, MIGRATION_0003),
            (4, MIGRATION_0004),
            (5, MIGRATION_0005),
            (6, MIGRATION_0006),
            (7, MIGRATION_0007),
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

    pub fn upsert_library_item(&self, item: &LibraryItem) -> Result<(), AppError> {
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
                 metadata_json, missing, added_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(path) DO UPDATE SET
                title = excluded.title,
                source_url = COALESCE(excluded.source_url, media_library.source_url),
                thumbnail = COALESCE(excluded.thumbnail, media_library.thumbnail),
                duration_seconds = COALESCE(excluded.duration_seconds, media_library.duration_seconds),
                site = COALESCE(excluded.site, media_library.site),
                tags_json = excluded.tags_json,
                metadata_json = excluded.metadata_json,
                missing = excluded.missing,
                updated_at = excluded.updated_at",
            params![
                item.id, item.path, item.title, item.source_url, item.thumbnail,
                item.duration_seconds, item.site, tags, metadata, item.missing,
                item.added_at.timestamp_millis(), item.updated_at.timestamp_millis(),
            ],
        )?;
        Ok(())
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>, AppError> {
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        let mut statement = connection.prepare(
            "SELECT id, path, title, source_url, thumbnail, duration_seconds, site, tags_json,
                    metadata_json, missing, added_at, updated_at
             FROM media_library ORDER BY added_at DESC LIMIT 5000",
        )?;
        let rows = statement.query_map([], library_item_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn update_library_item(
        &self,
        request: UpdateLibraryItemRequest,
    ) -> Result<LibraryItem, AppError> {
        let tags = serde_json::to_string(&request.tags)
            .map_err(|error| AppError::internal("could not serialize media tags", error))?;
        let connection = self.connection.lock().map_err(|_| {
            AppError::new("database_unavailable", "The media library is unavailable.")
        })?;
        if connection.execute(
            "UPDATE media_library SET title = ?2, tags_json = ?3, updated_at = ?4 WHERE id = ?1",
            params![
                request.id,
                request.title,
                tags,
                Utc::now().timestamp_millis()
            ],
        )? == 0
        {
            return Err(AppError::new(
                "library_item_not_found",
                "The library item was not found.",
            ));
        }
        connection
            .query_row(
                "SELECT id, path, title, source_url, thumbnail, duration_seconds, site, tags_json,
                    metadata_json, missing, added_at, updated_at FROM media_library WHERE id = ?1",
                [&request.id],
                library_item_from_row,
            )
            .map_err(AppError::from)
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
        title: row.get(2)?,
        source_url: row.get(3)?,
        thumbnail: row.get(4)?,
        duration_seconds: row.get(5)?,
        site: row.get(6)?,
        tags,
        metadata,
        missing: row.get(9)?,
        added_at: timestamp_or_epoch(row.get(10)?),
        updated_at: timestamp_or_epoch(row.get(11)?),
    })
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
}
