pub mod aria2;
pub mod discovery;
pub mod gallery_dl;
pub mod ytdlp;

use crate::{
    engines::ytdlp::YtDlpRuntimeCapabilities,
    errors::AppError,
    models::{DownloadOptions, EnginePreferences},
};

pub trait DownloadEngine {
    fn executable_name(&self) -> &'static str;
    fn build_download_args(
        &self,
        url: &str,
        options: &DownloadOptions,
        metadata_title_override: Option<&str>,
        preferences: &EnginePreferences,
        capabilities: &YtDlpRuntimeCapabilities,
    ) -> Result<Vec<String>, AppError>;
}
