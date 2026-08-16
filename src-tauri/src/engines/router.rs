use crate::{
    engines::{
        aria2::{analyze_direct, is_direct_download_url, prepare_direct},
        gallery_dl::{is_bunkr_url, GalleryDlEngine},
        ytdlp::YtDlpEngine,
        DownloadEngine,
    },
    errors::AppError,
    models::{
        DownloadEngineKind, EnginePreferences, MediaAnalysis, OutputConflictPolicy,
        StartDownloadRequest,
    },
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedEngineKind {
    YtDlp,
    GalleryDl,
    DirectHttp,
}

#[derive(Debug, Clone)]
pub struct PreparedDownload {
    pub executable: PathBuf,
    pub engine_name: &'static str,
    pub kind: PreparedEngineKind,
    pub args: Vec<String>,
    pub output_relative_path: Option<String>,
    pub expected_item_count: Option<u32>,
    pub output_conflict_policy: Option<OutputConflictPolicy>,
}

pub async fn analyze(
    url: &str,
    preferences: &EnginePreferences,
) -> Result<MediaAnalysis, AppError> {
    if is_bunkr_url(url) {
        GalleryDlEngine::discover()?.analyze(url, preferences).await
    } else if is_direct_download_url(url) {
        analyze_direct(url).await
    } else {
        YtDlpEngine::discover()?.analyze(url, preferences).await
    }
}

pub async fn prepare_download(
    request: &StartDownloadRequest,
    preferences: &EnginePreferences,
) -> Result<PreparedDownload, AppError> {
    let bunkr_url = is_bunkr_url(&request.url);
    match request.analysis.engine {
        DownloadEngineKind::GalleryDl if bunkr_url => {
            let engine = GalleryDlEngine::discover()?;
            let prepared = engine.build_download(request, preferences)?;
            Ok(PreparedDownload {
                executable: engine.executable().to_owned(),
                engine_name: "gallery-dl",
                kind: PreparedEngineKind::GalleryDl,
                args: prepared.args,
                output_relative_path: Some(prepared.output_directory),
                expected_item_count: request
                    .options
                    .gallery_selection
                    .as_ref()
                    .and_then(|items| u32::try_from(items.len()).ok())
                    .or(request.analysis.item_count),
                output_conflict_policy: None,
            })
        }
        DownloadEngineKind::GalleryDl => Err(AppError::new(
            "engine_route_mismatch",
            "The selected gallery engine does not support this URL.",
        )),
        DownloadEngineKind::YtDlp if bunkr_url => Err(AppError::new(
            "engine_route_mismatch",
            "Analyze this Bunkr album again so it can be downloaded with gallery-dl.",
        )),
        DownloadEngineKind::YtDlp => {
            if request.options.gallery_selection.is_some() {
                return Err(AppError::new(
                    "engine_route_mismatch",
                    "Gallery item selections can only be used with a gallery download.",
                ));
            }
            let engine = YtDlpEngine::discover()?;
            let capabilities = engine.runtime_capabilities().await;
            let args = engine.build_download_args(
                &request.url,
                &request.options,
                (request.analysis.download_url != request.analysis.webpage_url)
                    .then_some(request.analysis.title.as_str()),
                preferences,
                &capabilities,
            )?;
            Ok(PreparedDownload {
                executable: engine.executable().to_owned(),
                engine_name: engine.executable_name(),
                kind: PreparedEngineKind::YtDlp,
                args,
                output_relative_path: None,
                expected_item_count: None,
                output_conflict_policy: None,
            })
        }
        DownloadEngineKind::DirectHttp if is_direct_download_url(&request.url) => {
            let prepared = prepare_direct(request, preferences)?;
            Ok(PreparedDownload {
                executable: prepared.executable,
                engine_name: prepared.engine_name,
                kind: PreparedEngineKind::DirectHttp,
                args: prepared.args,
                output_relative_path: Some(prepared.filename),
                expected_item_count: Some(1),
                output_conflict_policy: Some(request.options.output_conflict_policy),
            })
        }
        DownloadEngineKind::DirectHttp => Err(AppError::new(
            "engine_route_mismatch",
            "The direct transfer engine only accepts recognized HTTP file URLs.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_gallery_only_for_supported_bunkr_shapes() {
        assert!(is_bunkr_url("https://bunkr.cr/a/ul3n82nu"));
        assert!(is_bunkr_url("https://bunkr.cr/f/eiK5ATuIv7DIY"));
        assert!(is_bunkr_url("https://bunkr.cr/v/video_1"));
        assert!(!is_bunkr_url("https://notbunkr.cr/a/ul3n82nu"));
        assert!(!is_bunkr_url("https://bunkr.cr/u/unsupported"));
    }
}
