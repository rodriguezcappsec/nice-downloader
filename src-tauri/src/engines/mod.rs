pub mod aria2;
pub mod discovery;
pub mod gallery_dl;
pub mod ytdlp;

use crate::{errors::AppError, models::DownloadOptions};

pub trait DownloadEngine {
    fn executable_name(&self) -> &'static str;
    fn build_download_args(
        &self,
        url: &str,
        options: &DownloadOptions,
    ) -> Result<Vec<String>, AppError>;
}
