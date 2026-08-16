use crate::{
    database::Database,
    errors::AppError,
    models::{
        AudioTransformCodec, DownloadJob, LibraryBulkAction, LibraryLocalInferenceJobState,
        PostDownloadRecipe, PostDownloadRecipeRun, PostDownloadRecipeRunState,
        PostDownloadRecipeStep, StartLibrarySemanticIndexRequest, StartLibraryTranscriptionRequest,
        StartTransformationRequest, TransformAudioCodec, TransformVideoCodec,
        TransformationOperation, TransformationState,
    },
    semantic_transcripts::SemanticTranscriptManager,
    transformations::TransformationManager,
};
use chrono::Utc;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{sleep, Duration};

pub const POST_DOWNLOAD_RECIPE_EVENT: &str = "post-download-recipe-run-updated";

pub fn schedule(app: AppHandle, job: DownloadJob) {
    let Some(database) = app
        .try_state::<Database>()
        .map(|state| state.inner().clone())
    else {
        return;
    };
    let recipe_ids = match database.post_download_recipe_ids_for_download(&job.id) {
        Ok(ids) if !ids.is_empty() => ids,
        _ => return,
    };
    let recipes = match database.enabled_post_download_recipes_by_ids(&recipe_ids) {
        Ok(recipes) => recipes,
        Err(error) => {
            tracing::warn!(job_id = %job.id, code = %error.code, "post-download recipes could not be loaded");
            return;
        }
    };
    tauri::async_runtime::spawn(async move {
        for recipe in recipes {
            let Ok(Some(mut run)) = database.claim_post_download_recipe_run(&recipe, &job.id)
            else {
                continue;
            };
            emit_run(&app, &run);
            let result = execute_recipe(&app, &database, &job, &recipe, &mut run).await;
            run.updated_at = Utc::now();
            run.completed_at = Some(run.updated_at);
            match result {
                Ok(()) => run.state = PostDownloadRecipeRunState::Completed,
                Err(error) => {
                    run.state = PostDownloadRecipeRunState::Failed;
                    run.error = Some(error.message);
                }
            }
            let _ = database.update_post_download_recipe_run(&run);
            emit_run(&app, &run);
        }
    });
}

async fn execute_recipe(
    app: &AppHandle,
    database: &Database,
    job: &DownloadJob,
    recipe: &PostDownloadRecipe,
    run: &mut PostDownloadRecipeRun,
) -> Result<(), AppError> {
    for (index, step) in recipe.steps.iter().enumerate() {
        run.current_step = index + 1;
        run.updated_at = Utc::now();
        database.update_post_download_recipe_run(run)?;
        emit_run(app, run);
        execute_step(app, database, job, step).await?;
    }
    Ok(())
}

async fn execute_step(
    app: &AppHandle,
    database: &Database,
    job: &DownloadJob,
    step: &PostDownloadRecipeStep,
) -> Result<(), AppError> {
    let item = database.get_library_item(&job.id)?;
    match step {
        PostDownloadRecipeStep::ImportLibrary => Ok(()),
        PostDownloadRecipeStep::AddTags { tags } => {
            database.apply_library_bulk_action(
                vec![item.id],
                LibraryBulkAction::AddTags { tags: tags.clone() },
            )?;
            Ok(())
        }
        PostDownloadRecipeStep::AddToCollection { collection_id } => {
            database.apply_library_bulk_action(
                vec![item.id],
                LibraryBulkAction::AddToCollection {
                    collection_id: collection_id.clone(),
                },
            )?;
            Ok(())
        }
        PostDownloadRecipeStep::TranscodeMp4 => {
            run_transformation(
                app,
                &item.path,
                derived_path(&item.path, "h264", "mp4")?,
                TransformationOperation::Transcode {
                    container: "mp4".to_owned(),
                    video_codec: TransformVideoCodec::H264,
                    audio_codec: TransformAudioCodec::Aac,
                },
            )
            .await
        }
        PostDownloadRecipeStep::ExtractAudio { codec } => {
            let extension = match codec {
                AudioTransformCodec::Mp3 => "mp3",
                AudioTransformCodec::Aac => "m4a",
                AudioTransformCodec::Opus => "opus",
            };
            run_transformation(
                app,
                &item.path,
                derived_path(&item.path, "audio", extension)?,
                TransformationOperation::ExtractAudio { codec: *codec },
            )
            .await
        }
        PostDownloadRecipeStep::PrivacySafeCopy => {
            run_transformation(
                app,
                &item.path,
                derived_path(&item.path, "privacy", "mp4")?,
                TransformationOperation::PrivacyExport {
                    strip_metadata: true,
                    blur_regions: Vec::new(),
                    mute_ranges: Vec::new(),
                },
            )
            .await
        }
        PostDownloadRecipeStep::Transcribe {
            model_path,
            language,
        } => {
            let manager = app
                .try_state::<SemanticTranscriptManager>()
                .ok_or_else(|| {
                    AppError::new(
                        "local_inference_unavailable",
                        "Local transcription is unavailable.",
                    )
                })?;
            let started = manager
                .start_transcription(
                    app.clone(),
                    StartLibraryTranscriptionRequest {
                        item_ids: vec![item.id],
                        model_path: model_path.clone(),
                        language: language.clone(),
                    },
                )
                .await?;
            wait_for_local_job(&manager, &started.id).await
        }
        PostDownloadRecipeStep::SemanticIndex { model } => {
            let manager = app
                .try_state::<SemanticTranscriptManager>()
                .ok_or_else(|| {
                    AppError::new(
                        "local_inference_unavailable",
                        "Semantic indexing is unavailable.",
                    )
                })?;
            let started = manager
                .start_semantic_index(
                    app.clone(),
                    StartLibrarySemanticIndexRequest {
                        item_ids: vec![item.id],
                        max_videos: 1,
                        model: model.clone(),
                    },
                )
                .await?;
            if started.state == LibraryLocalInferenceJobState::Completed {
                Ok(())
            } else {
                wait_for_local_job(&manager, &started.id).await
            }
        }
    }
}

async fn run_transformation(
    app: &AppHandle,
    input_path: &str,
    output_path: PathBuf,
    operation: TransformationOperation,
) -> Result<(), AppError> {
    let manager = app.try_state::<TransformationManager>().ok_or_else(|| {
        AppError::new(
            "transformations_unavailable",
            "Media transformations are unavailable.",
        )
    })?;
    let task = manager.start(
        app.clone(),
        StartTransformationRequest {
            input_path: input_path.to_owned(),
            output_path: output_path.to_string_lossy().into_owned(),
            operation,
            overwrite: false,
        },
    )?;
    loop {
        sleep(Duration::from_secs(1)).await;
        let current = manager
            .list()?
            .into_iter()
            .find(|candidate| candidate.id == task.id)
            .ok_or_else(|| {
                AppError::new(
                    "transformation_unavailable",
                    "The recipe transformation disappeared.",
                )
            })?;
        match current.state {
            TransformationState::Queued | TransformationState::Running => {}
            TransformationState::Completed => return Ok(()),
            TransformationState::Failed | TransformationState::Cancelled => {
                return Err(AppError::new(
                    "recipe_transformation_failed",
                    current
                        .error
                        .unwrap_or_else(|| "A recipe transformation did not complete.".to_owned()),
                ));
            }
        }
    }
}

async fn wait_for_local_job(
    manager: &SemanticTranscriptManager,
    job_id: &str,
) -> Result<(), AppError> {
    loop {
        sleep(Duration::from_secs(1)).await;
        let current = manager.job()?.ok_or_else(|| {
            AppError::new(
                "local_inference_unavailable",
                "The local recipe job disappeared.",
            )
        })?;
        if current.id != job_id {
            return Err(AppError::new(
                "local_inference_job_replaced",
                "Another local job replaced the recipe operation.",
            ));
        }
        match current.state {
            LibraryLocalInferenceJobState::Running => {}
            LibraryLocalInferenceJobState::Completed => return Ok(()),
            LibraryLocalInferenceJobState::Failed | LibraryLocalInferenceJobState::Cancelled => {
                return Err(AppError::new(
                    "recipe_local_inference_failed",
                    current
                        .error
                        .unwrap_or_else(|| "A local recipe operation did not complete.".to_owned()),
                ));
            }
        }
    }
}

fn derived_path(input: &str, suffix: &str, extension: &str) -> Result<PathBuf, AppError> {
    let input = Path::new(input);
    let parent = input.parent().ok_or_else(|| {
        AppError::new(
            "invalid_recipe_input",
            "The downloaded file has no output folder.",
        )
    })?;
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                "invalid_recipe_input",
                "The downloaded filename is invalid.",
            )
        })?;
    Ok(parent.join(format!("{stem}.{suffix}.{extension}")))
}

fn emit_run(app: &AppHandle, run: &PostDownloadRecipeRun) {
    let _ = app.emit(POST_DOWNLOAD_RECIPE_EVENT, run);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_outputs_are_sibling_files_with_typed_suffixes() {
        assert_eq!(
            derived_path("/media/video.mkv", "privacy", "mp4").unwrap(),
            PathBuf::from("/media/video.privacy.mp4")
        );
    }
}
