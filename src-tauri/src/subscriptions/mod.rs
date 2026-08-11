use crate::{
    database::Database,
    engines::{router, ytdlp::YtDlpEngine},
    errors::AppError,
    models::{
        DownloadOptions, FormatStrategy, MediaAnalysis, PresetIntent, QualityKind,
        StartDownloadRequest, SubscriptionCheckResult,
    },
    queue::QueueManager,
};
use std::time::Duration;
use tauri::AppHandle;
use tokio::time::{interval, MissedTickBehavior};

pub fn spawn_scheduler(app: AppHandle, database: Database, queue: QueueManager) {
    tauri::async_runtime::spawn(async move {
        let mut timer = interval(Duration::from_secs(60));
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            timer.tick().await;
            let due = match database.due_subscriptions(8) {
                Ok(value) => value,
                Err(error) => {
                    tracing::warn!(%error, "could not load due subscriptions");
                    continue;
                }
            };
            for subscription in due {
                if let Err(error) =
                    check_subscription(app.clone(), &database, &queue, &subscription.id).await
                {
                    tracing::warn!(subscription_id = %subscription.id, %error, "subscription check failed");
                }
            }
        }
    });
}

pub async fn check_subscription(
    app: AppHandle,
    database: &Database,
    queue: &QueueManager,
    subscription_id: &str,
) -> Result<SubscriptionCheckResult, AppError> {
    let subscription = database.load_subscription(subscription_id)?;
    let result = check_inner(app, database, queue, &subscription).await;
    match result {
        Ok((discovered_count, queued_count, skipped_count)) => {
            let subscription = database.record_subscription_check(subscription_id, None)?;
            Ok(SubscriptionCheckResult {
                subscription,
                discovered_count,
                queued_count,
                skipped_count,
            })
        }
        Err(error) => {
            let _ = database.record_subscription_check(subscription_id, Some(&error.message));
            Err(error)
        }
    }
}

async fn check_inner(
    app: AppHandle,
    database: &Database,
    queue: &QueueManager,
    subscription: &crate::models::Subscription,
) -> Result<(usize, usize, usize), AppError> {
    if !subscription.enabled {
        return Err(AppError::new(
            "subscription_disabled",
            "Enable this subscription before checking it.",
        ));
    }
    let preset = database
        .list_presets()?
        .into_iter()
        .find(|preset| preset.id == subscription.preset_id)
        .ok_or_else(|| {
            AppError::new(
                "preset_not_found",
                "The subscription preset no longer exists.",
            )
        })?;
    let preferences = database.load_engine_preferences()?;
    let engine = YtDlpEngine::discover()?;
    let (_, urls) = engine
        .expand_playlist(&subscription.url, &preferences)
        .await?;
    let urls = urls
        .into_iter()
        .take(subscription.max_entries as usize)
        .collect::<Vec<_>>();
    let discovered_count = urls.len();
    let mut queued_count = 0;
    let mut skipped_count = 0;

    for url in urls {
        let analysis = match router::analyze(&url, &preferences).await {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!(subscription_id = %subscription.id, %error, "subscription entry analysis failed");
                skipped_count += 1;
                continue;
            }
        };
        let download_url = if analysis.download_url.is_empty() {
            analysis.webpage_url.clone()
        } else {
            analysis.download_url.clone()
        };
        if database.has_completed_duplicate(&download_url, &subscription.destination)? {
            skipped_count += 1;
            continue;
        }
        let request = StartDownloadRequest {
            url: download_url,
            options: options_from_preset(&analysis, &preset.intent, &subscription.destination)?,
            analysis,
            scheduled_for: None,
        };
        match queue
            .enqueue(app.clone(), request, preferences.clone())
            .await
        {
            Ok(_) => queued_count += 1,
            Err(error) if error.code == "duplicate_download" => skipped_count += 1,
            Err(error) => return Err(error),
        }
    }
    Ok((discovered_count, queued_count, skipped_count))
}

fn options_from_preset(
    analysis: &MediaAnalysis,
    preset: &PresetIntent,
    destination: &str,
) -> Result<DownloadOptions, AppError> {
    let quality = match preset.quality.as_str() {
        "audio" => analysis
            .quality_options
            .iter()
            .find(|option| option.kind == QualityKind::AudioOnly),
        "best" => analysis
            .quality_options
            .iter()
            .find(|option| option.kind == QualityKind::Best),
        height => height.parse::<u32>().ok().and_then(|height| {
            analysis
                .quality_options
                .iter()
                .find(|option| option.max_height == Some(height))
        }),
    }
    .or_else(|| analysis.quality_options.first())
    .cloned()
    .ok_or_else(|| {
        AppError::new(
            "subscription_format_unavailable",
            "No downloadable format was found for a subscription entry.",
        )
    })?;
    let audio_only = quality.kind == QualityKind::AudioOnly;
    Ok(DownloadOptions {
        quality,
        container: preset.container,
        video_codec_preference: preset.video_codec_preference,
        format_strategy: FormatStrategy::Recommended,
        audio_quality: preset.audio_quality,
        audio_only,
        subtitles: false,
        subtitle_languages: vec![],
        embed_thumbnail: preset.embed_thumbnail,
        embed_metadata: preset.embed_metadata,
        chapters: false,
        sponsorblock: !preset.sponsorblock_categories.is_empty(),
        sponsorblock_categories: preset.sponsorblock_categories.clone(),
        destination: destination.to_owned(),
        filename_template: preset
            .filename_template
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "%(title).180B [%(id)s].%(ext)s".to_owned()),
        output_conflict_policy: preset.output_conflict_policy,
        gallery_selection: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, OutputConflictPolicy, OutputContainer, PresetIntent, SponsorBlockCategory,
        VideoCodecPreference,
    };

    #[test]
    fn maps_subscription_preset_to_typed_options() {
        let analysis = MediaAnalysis {
            engine: crate::models::DownloadEngineKind::YtDlp,
            title: "Example".to_owned(),
            uploader: None,
            duration_seconds: None,
            thumbnail: None,
            webpage_url: "https://example.com/watch/one".to_owned(),
            download_url: "https://example.com/watch/one".to_owned(),
            extractor: "example".to_owned(),
            site_name: "Example".to_owned(),
            playlist_title: None,
            playlist_index: None,
            formats: vec![],
            quality_options: vec![crate::models::QualityOption {
                id: "best".to_owned(),
                label: "Best".to_owned(),
                description: "Best available".to_owned(),
                kind: QualityKind::Best,
                max_height: None,
            }],
            item_count: None,
            total_size_bytes: None,
        };
        let preset = PresetIntent {
            quality: "best".to_owned(),
            container: OutputContainer::Mkv,
            video_codec_preference: VideoCodecPreference::Any,
            audio_quality: AudioQuality::Best,
            embed_thumbnail: true,
            embed_metadata: true,
            output_conflict_policy: OutputConflictPolicy::Skip,
            sponsorblock_categories: vec![SponsorBlockCategory::Sponsor],
            filename_template: None,
        };
        let options = options_from_preset(&analysis, &preset, "/tmp").expect("options");
        assert!(matches!(options.container, OutputContainer::Mkv));
        assert!(options.sponsorblock);
        assert_eq!(options.destination, "/tmp");
    }
}
