use crate::{
    engines::{
        gallery_dl::{is_bunkr_album_url, GalleryDlEngine},
        ytdlp::YtDlpEngine,
        DownloadEngine,
    },
    errors::AppError,
    models::{DownloadEngineKind, EnginePreferences, MediaAnalysis, StartDownloadRequest},
};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparedEngineKind {
    YtDlp,
    GalleryDl,
}

#[derive(Debug, Clone)]
pub struct PreparedDownload {
    pub executable: PathBuf,
    pub engine_name: &'static str,
    pub kind: PreparedEngineKind,
    pub args: Vec<String>,
    pub output_relative_path: Option<String>,
}

pub async fn analyze(
    url: &str,
    preferences: &EnginePreferences,
) -> Result<MediaAnalysis, AppError> {
    if is_bunkr_album_url(url) {
        GalleryDlEngine::discover()?.analyze(url, preferences).await
    } else {
        YtDlpEngine::discover()?.analyze(url, preferences).await
    }
}

pub async fn prepare_download(
    request: &StartDownloadRequest,
    preferences: &EnginePreferences,
) -> Result<PreparedDownload, AppError> {
    let bunkr_url = is_bunkr_album_url(&request.url);
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
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_gallery_only_for_supported_bunkr_album_shape() {
        assert!(is_bunkr_album_url("https://bunkr.cr/a/ul3n82nu"));
        assert!(!is_bunkr_album_url("https://notbunkr.cr/a/ul3n82nu"));
    }
}
