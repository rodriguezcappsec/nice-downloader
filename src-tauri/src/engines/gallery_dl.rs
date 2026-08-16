use crate::{
    engines::ytdlp::sanitize_diagnostic,
    errors::AppError,
    models::{
        DownloadEngineKind, EnginePreferences, GalleryItem, GalleryItemList, MediaAnalysis,
        OutputConflictPolicy, QualityKind, QualityOption, StartDownloadRequest,
    },
    utils::validation::validate_media_url,
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashSet, path::PathBuf, time::Duration};
use tokio::{process::Command, time::timeout};
use url::Url;

const MAX_ANALYSIS_BYTES: usize = 1_000_000;
const MAX_ITEM_LIST_BYTES: usize = 5_000_000;
const MAX_SELECTABLE_ITEMS: u32 = 1_000;
const MAX_GALLERY_PAGE_SIZE: u32 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
enum BunkrUrlKind {
    Album { id: String },
    Media { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GallerySite {
    Bunkr,
    Imgur,
    Flickr,
    Pinterest,
}

impl GallerySite {
    fn label(self) -> &'static str {
        match self {
            Self::Bunkr => "Bunkr",
            Self::Imgur => "Imgur",
            Self::Flickr => "Flickr",
            Self::Pinterest => "Pinterest",
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Bunkr => "bunkr",
            Self::Imgur => "imgur",
            Self::Flickr => "flickr",
            Self::Pinterest => "pinterest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GalleryUrlKind {
    Bunkr(BunkrUrlKind),
    Collection { site: GallerySite, id: String },
}

impl GalleryUrlKind {
    fn site(&self) -> GallerySite {
        match self {
            Self::Bunkr(_) => GallerySite::Bunkr,
            Self::Collection { site, .. } => *site,
        }
    }

    fn is_collection(&self) -> bool {
        match self {
            Self::Bunkr(kind) => kind.is_album(),
            Self::Collection { .. } => true,
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Bunkr(kind) => kind.id(),
            Self::Collection { id, .. } => id,
        }
    }
}

impl BunkrUrlKind {
    fn is_album(&self) -> bool {
        matches!(self, Self::Album { .. })
    }

    fn id(&self) -> &str {
        match self {
            Self::Album { id } | Self::Media { id } => id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GalleryDlEngine {
    executable: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GalleryDownload {
    pub args: Vec<String>,
    pub output_directory: String,
}

impl GalleryDlEngine {
    pub fn discover() -> Result<Self, AppError> {
        let executable = crate::engines::managed::resolve("gallery-dl").ok_or_else(|| {
            AppError::new(
                "engine_not_found",
                "No bundled or managed gallery-dl was available, and it was not found on this system. Reinstall Nice Downloader or install gallery-dl, then refresh Settings > Engines.",
            )
        })?;
        Ok(Self { executable })
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    pub async fn analyze(
        &self,
        url: &str,
        preferences: &EnginePreferences,
    ) -> Result<MediaAnalysis, AppError> {
        let (validated, kind) = validate_gallery_url(url)?;
        let mut command = Command::new(&self.executable);
        crate::utils::process::sanitize_tokio_command(&mut command);
        command.kill_on_drop(true).args([
            "--config-ignore",
            "--no-colors",
            "--no-input",
            "--range",
            "1",
            "--dump-json",
        ]);
        command.args(access_args(preferences));
        command.arg("--").arg(validated.as_str());

        tracing::info!(
            engine = "gallery-dl",
            operation = "analyze",
            site = kind.site().slug(),
            "starting engine operation"
        );
        let output = timeout(Duration::from_secs(60), command.output())
            .await
            .map_err(|_| {
                AppError::new(
                    "analysis_timeout",
                    "Bunkr analysis timed out after 60 seconds.",
                )
            })?
            .map_err(|error| AppError::internal("could not start gallery-dl", error))?;

        if !output.status.success() {
            let details = sanitize_diagnostic(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::with_details(
                "analysis_failed",
                friendly_error(&details),
                details,
            ));
        }
        if output.stdout.len() > MAX_ANALYSIS_BYTES {
            return Err(AppError::new(
                "analysis_too_large",
                "gallery-dl returned more metadata than the application can safely process.",
            ));
        }
        let value: Value = serde_json::from_slice(&output.stdout)
            .map_err(|error| AppError::internal("gallery-dl returned invalid JSON", error))?;
        match &kind {
            GalleryUrlKind::Bunkr(BunkrUrlKind::Album { .. }) => {
                parse_album_metadata(value, validated.as_str())
            }
            GalleryUrlKind::Bunkr(BunkrUrlKind::Media { .. }) => {
                parse_media_metadata(value, validated.as_str())
            }
            GalleryUrlKind::Collection { site, .. } => {
                parse_collection_metadata(value, validated.as_str(), *site)
            }
        }
    }

    pub async fn list_items(
        &self,
        url: &str,
        preferences: &EnginePreferences,
        offset: u32,
        limit: u32,
    ) -> Result<GalleryItemList, AppError> {
        let (validated, kind) = validate_gallery_collection_url(url)?;
        if offset >= MAX_SELECTABLE_ITEMS || !(1..=MAX_GALLERY_PAGE_SIZE).contains(&limit) {
            return Err(AppError::new(
                "invalid_gallery_page",
                "Album pages must contain between 1 and 24 files and stay within the first 1000 entries.",
            ));
        }
        let end = offset.saturating_add(limit).min(MAX_SELECTABLE_ITEMS);
        let range = format!("{}-{end}", offset + 1);
        let mut command = Command::new(&self.executable);
        crate::utils::process::sanitize_tokio_command(&mut command);
        command.kill_on_drop(true).args([
            "--config-ignore",
            "--no-colors",
            "--no-input",
            "--no-download",
            "--range",
        ]);
        command.arg(range).args([
            "--sleep-request",
            "0.15-0.35",
            "--option",
            r#"output.mode="null""#,
            "--Print",
            r#"prepare:{{"index":{num!j},"count":{count!j},"id":{id!j},"filename":{filename!j},"extension":{extension!j},"size":{size!j},"thumbnail":{thumbnail!j}}}"#,
        ]);
        command.args(access_args(preferences));
        command.arg("--").arg(validated.as_str());

        tracing::info!(
            engine = "gallery-dl",
            operation = "list_items",
            site = kind.site().slug(),
            "starting engine operation"
        );
        let output = timeout(Duration::from_secs(120), command.output())
            .await
            .map_err(|_| {
                AppError::new(
                    "gallery_items_timeout",
                    "Loading gallery contents timed out after 120 seconds.",
                )
            })?
            .map_err(|error| AppError::internal("could not start gallery-dl", error))?;
        if !output.status.success() {
            let details = sanitize_diagnostic(&String::from_utf8_lossy(&output.stderr));
            return Err(AppError::with_details(
                "gallery_items_failed",
                friendly_error(&details),
                details,
            ));
        }
        if output.stdout.len() > MAX_ITEM_LIST_BYTES {
            return Err(AppError::new(
                "gallery_items_too_large",
                "This album contains more item metadata than the application can safely display.",
            ));
        }
        parse_item_list(&String::from_utf8_lossy(&output.stdout), offset)
    }

    pub fn build_download(
        &self,
        request: &StartDownloadRequest,
        preferences: &EnginePreferences,
    ) -> Result<GalleryDownload, AppError> {
        let (validated, kind) = validate_gallery_url(&request.url)?;
        if !kind.is_collection() && request.options.gallery_selection.is_some() {
            return Err(AppError::new(
                "invalid_gallery_selection",
                "Individual media links do not accept a gallery file selection.",
            ));
        }
        let output_directory = if kind.is_collection() {
            safe_album_directory(&request.analysis.title, kind.id())
        } else {
            ".".to_owned()
        };
        let mut args = vec![
            "--config-ignore".to_owned(),
            "--no-colors".to_owned(),
            "--no-input".to_owned(),
            "--sleep-request".to_owned(),
            "0.15-0.35".to_owned(),
            "--restrict-filenames".to_owned(),
            "unix".to_owned(),
            "--directory".to_owned(),
            output_directory.clone(),
            "--Print".to_owned(),
            "prepare:nicedl:gallery:item:{num}|{count}|{size}|{filename}.{extension}".to_owned(),
            "--Print".to_owned(),
            "after:nicedl:gallery:complete:{num}|{count}|{size}|{filename}.{extension}"
                .to_owned(),
            "--Print".to_owned(),
            "skip:nicedl:gallery:skip:{num}|{count}|{size}|{filename}.{extension}"
                .to_owned(),
            "--Print".to_owned(),
            "error:nicedl:gallery:error:{num}|{count}|{size}|{filename}.{extension}"
                .to_owned(),
            "--option".to_owned(),
            r#"output.mode={"start":"nicedl:gallery:output:{}\n","skip":"nicedl:gallery:output:{}\n","success":"nicedl:gallery:output:{}\n","progress":"\nnicedl:gallery:progress:NA|{0}|{1}|NA","progress-total":"\nnicedl:gallery:progress:{3}|{0}|{1}|{2}"}"#.to_owned(),
            "--option".to_owned(),
            "output.shorten=false".to_owned(),
            "--option".to_owned(),
            "downloader.http.retries=12".to_owned(),
            "--option".to_owned(),
            "downloader.http.sleep-retries=1-3".to_owned(),
            "--option".to_owned(),
            "downloader.http.sleep-429=exp:2:0:120=15".to_owned(),
            "--option".to_owned(),
            "downloader.http.retry-codes=[429]".to_owned(),
        ];
        if kind.is_collection() {
            if let Some(selection) = &request.options.gallery_selection {
                let range = build_range(
                    selection.iter().map(|item| item.index),
                    request.analysis.item_count,
                )?;
                args.extend(["--range".to_owned(), range]);
            }
        }
        if request.options.output_conflict_policy == OutputConflictPolicy::Overwrite {
            args.push("--no-skip".to_owned());
        }
        args.extend(access_args(preferences));
        args.push("--".to_owned());
        args.push(validated.into());
        Ok(GalleryDownload {
            args,
            output_directory,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawGalleryItem {
    index: u32,
    count: u32,
    id: Option<String>,
    filename: Option<String>,
    extension: Option<String>,
    size: Option<u64>,
    thumbnail: Option<String>,
}

fn parse_item_list(value: &str, offset: u32) -> Result<GalleryItemList, AppError> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();
    let mut total_count = 0;
    for line in value.lines().filter(|line| !line.trim().is_empty()) {
        if line.len() > 32_000 {
            return Err(AppError::new(
                "invalid_gallery_items",
                "gallery-dl returned an oversized album item.",
            ));
        }
        let raw: RawGalleryItem = serde_json::from_str(line).map_err(|error| {
            AppError::internal("gallery-dl returned invalid album item JSON", error)
        })?;
        total_count = total_count.max(raw.count);
        let Some(index) = offset.checked_add(raw.index) else {
            continue;
        };
        if raw.index == 0 || !seen.insert(index) {
            continue;
        }
        let extension = raw.extension.and_then(clean_extension);
        let filename = raw
            .filename
            .as_deref()
            .map(clean_title)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("Album item {index}"));
        let thumbnail = raw.thumbnail.filter(|url| validate_media_url(url).is_ok());
        items.push(GalleryItem {
            index,
            id: raw.id.as_deref().map(clean_title).unwrap_or_default(),
            filename,
            is_video: extension.as_deref().is_some_and(is_video_extension),
            extension,
            size_bytes: raw.size,
            thumbnail,
        });
    }
    if items.is_empty() {
        return Err(AppError::new(
            "empty_gallery",
            "No selectable files were found in this Bunkr album.",
        ));
    }
    items.sort_by_key(|item| item.index);
    if total_count == 0 {
        total_count = u32::try_from(items.len()).unwrap_or(u32::MAX);
    }
    let loaded_through = offset.saturating_add(u32::try_from(items.len()).unwrap_or(u32::MAX));
    let next_offset = (loaded_through < total_count && loaded_through < MAX_SELECTABLE_ITEMS)
        .then_some(loaded_through);
    Ok(GalleryItemList {
        truncated: next_offset.is_some(),
        total_count,
        items,
        offset,
        next_offset,
    })
}

fn build_range(
    indices: impl Iterator<Item = u32>,
    album_count: Option<u32>,
) -> Result<String, AppError> {
    let mut indices = indices.collect::<Vec<_>>();
    let submitted_count = indices.len();
    indices.sort_unstable();
    indices.dedup();
    if indices.is_empty()
        || indices.len() != submitted_count
        || indices.len() > MAX_SELECTABLE_ITEMS as usize
    {
        return Err(AppError::new(
            "invalid_gallery_selection",
            "Select between 1 and 1000 album files.",
        ));
    }
    if indices
        .iter()
        .any(|index| *index == 0 || album_count.is_some_and(|count| *index > count))
    {
        return Err(AppError::new(
            "invalid_gallery_selection",
            "The selected album files are outside this album's item range.",
        ));
    }
    Ok(indices
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(","))
}

fn clean_extension(value: String) -> Option<String> {
    let value = value.trim().trim_start_matches('.').to_ascii_lowercase();
    (!value.is_empty()
        && value.len() <= 12
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric()))
    .then_some(value)
}

fn is_video_extension(value: &str) -> bool {
    matches!(
        value,
        "mp4"
            | "mkv"
            | "webm"
            | "mov"
            | "m4v"
            | "avi"
            | "flv"
            | "wmv"
            | "mpg"
            | "mpeg"
            | "ts"
            | "m2ts"
            | "3gp"
    )
}

#[cfg(test)]
fn is_bunkr_url(value: &str) -> bool {
    validate_bunkr_url(value).is_ok()
}

pub fn is_supported_gallery_url(value: &str) -> bool {
    validate_gallery_url(value).is_ok()
}

fn validate_gallery_url(value: &str) -> Result<(Url, GalleryUrlKind), AppError> {
    let url = validate_media_url(value)?;
    let host = normalized_host(&url);
    if host == "bunkr.cr" || host.ends_with(".bunkr.cr") {
        let (url, kind) = validate_bunkr_url(value)?;
        return Ok((url, GalleryUrlKind::Bunkr(kind)));
    }

    let segments = url
        .path_segments()
        .into_iter()
        .flatten()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let collection = if matches!(host.as_str(), "imgur.com" | "www.imgur.com")
        && segments.len() == 2
        && matches!(segments[0], "a" | "gallery")
        && is_safe_gallery_segment(segments[1], 120)
    {
        Some((GallerySite::Imgur, segments[1].to_owned()))
    } else if matches!(host.as_str(), "flickr.com" | "www.flickr.com")
        && segments.len() == 4
        && segments[0] == "photos"
        && matches!(segments[2], "albums" | "galleries")
        && is_safe_gallery_segment(segments[1], 120)
        && is_safe_gallery_segment(segments[3], 120)
    {
        Some((
            GallerySite::Flickr,
            format!("{}-{}", segments[1], segments[3]),
        ))
    } else if is_pinterest_host(&host)
        && segments.len() == 2
        && !matches!(
            segments[0].to_ascii_lowercase().as_str(),
            "pin" | "search" | "ideas" | "today" | "business"
        )
        && is_safe_gallery_segment(segments[0], 120)
        && is_safe_gallery_segment(segments[1], 120)
    {
        Some((
            GallerySite::Pinterest,
            format!("{}-{}", segments[0], segments[1]),
        ))
    } else {
        None
    };

    let Some((site, id)) = collection else {
        return Err(unsupported_gallery_url());
    };
    Ok((url, GalleryUrlKind::Collection { site, id }))
}

fn validate_gallery_collection_url(value: &str) -> Result<(Url, GalleryUrlKind), AppError> {
    let (url, kind) = validate_gallery_url(value)?;
    if !kind.is_collection() {
        return Err(AppError::new(
            "unsupported_gallery_url",
            "This link identifies one media file, not a browsable gallery.",
        ));
    }
    Ok((url, kind))
}

fn normalized_host(url: &Url) -> String {
    url.host_str()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn is_pinterest_host(host: &str) -> bool {
    matches!(
        host,
        "pinterest.com"
            | "www.pinterest.com"
            | "www.pinterest.ca"
            | "www.pinterest.co.uk"
            | "www.pinterest.com.au"
            | "www.pinterest.de"
            | "www.pinterest.fr"
            | "www.pinterest.es"
            | "www.pinterest.it"
            | "www.pinterest.jp"
            | "www.pinterest.com.mx"
            | "www.pinterest.pt"
            | "www.pinterest.se"
            | "www.pinterest.dk"
            | "www.pinterest.nz"
            | "www.pinterest.ie"
            | "www.pinterest.ch"
            | "www.pinterest.at"
            | "www.pinterest.nl"
            | "www.pinterest.be"
    )
}

fn is_safe_gallery_segment(value: &str, max_length: usize) -> bool {
    if value.is_empty() || value.len() > max_length {
        return false;
    }
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !(bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'-' | b'_' | b'.')) {
            return false;
        }
        index += 1;
    }
    true
}

fn unsupported_gallery_url() -> AppError {
    AppError::new(
        "unsupported_gallery_url",
        "Supported gallery links include Bunkr albums/media, Imgur albums, Flickr albums/galleries, and Pinterest boards.",
    )
}

fn validate_bunkr_url(value: &str) -> Result<(Url, BunkrUrlKind), AppError> {
    let url = validate_media_url(value)?;
    let host = url
        .host_str()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if host != "bunkr.cr" && !host.ends_with(".bunkr.cr") {
        return Err(unsupported_bunkr_url());
    }

    let segments = url.path().trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() != 2 || segments.iter().any(|segment| segment.is_empty()) {
        return Err(unsupported_bunkr_url());
    }
    let route = segments[0];
    let id = segments[1];
    let max_id_length = if route == "a" { 80 } else { 240 };
    if !is_safe_bunkr_id(id, max_id_length) {
        return Err(unsupported_bunkr_url());
    }

    let kind = match route {
        "a" => BunkrUrlKind::Album { id: id.to_owned() },
        "f" | "v" | "i" | "d" => BunkrUrlKind::Media { id: id.to_owned() },
        _ => return Err(unsupported_bunkr_url()),
    };
    Ok((url, kind))
}

fn is_safe_bunkr_id(id: &str, max_length: usize) -> bool {
    if id.is_empty() || id.len() > max_length {
        return false;
    }
    let bytes = id.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1].is_ascii_hexdigit()
                || !bytes[index + 2].is_ascii_hexdigit()
            {
                return false;
            }
            index += 3;
            continue;
        }
        if !bytes[index].is_ascii_alphanumeric() && !matches!(bytes[index], b'-' | b'_' | b'.') {
            return false;
        }
        index += 1;
    }
    true
}

fn unsupported_bunkr_url() -> AppError {
    AppError::new(
        "unsupported_gallery_url",
        "Supported Bunkr links use /a/ for albums or /f/, /v/, /i/, and /d/ for individual media.",
    )
}

#[cfg(test)]
fn validate_bunkr_album_url(value: &str) -> Result<Url, AppError> {
    let (url, kind) = validate_bunkr_url(value)?;
    if !kind.is_album() {
        return Err(AppError::new(
            "unsupported_gallery_url",
            "Only Bunkr album URLs in the form https://bunkr.cr/a/album-id are supported by this gallery adapter.",
        ));
    }
    Ok(url)
}

fn parse_album_metadata(value: Value, requested_url: &str) -> Result<MediaAnalysis, AppError> {
    let events = value.as_array().ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not return structured Bunkr album metadata.",
        )
    })?;
    let mut album = None;
    let mut first_item = None;
    for event in events {
        let Some(parts) = event.as_array() else {
            continue;
        };
        match parts.first().and_then(Value::as_u64) {
            Some(2) => album = parts.get(1).and_then(Value::as_object),
            Some(3) => first_item = parts.get(2).and_then(Value::as_object),
            _ => {}
        }
    }
    let metadata = album.or(first_item).ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not identify a Bunkr album.",
        )
    })?;
    let title = metadata
        .get("album_name")
        .and_then(Value::as_str)
        .map(clean_title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Bunkr album".to_owned());
    let item_count = metadata
        .get("count")
        .and_then(Value::as_u64)
        .and_then(|count| u32::try_from(count).ok());
    let total_size_bytes = metadata
        .get("album_size")
        .and_then(Value::as_str)
        .and_then(parse_human_bytes);
    let thumbnail = first_item
        .and_then(|item| item.get("thumbnail"))
        .and_then(Value::as_str)
        .filter(|url| validate_media_url(url).is_ok())
        .map(str::to_owned);

    Ok(MediaAnalysis {
        engine: DownloadEngineKind::GalleryDl,
        title,
        uploader: None,
        duration_seconds: None,
        thumbnail,
        webpage_url: requested_url.to_owned(),
        download_url: requested_url.to_owned(),
        extractor: "gallery-dl:bunkr".to_owned(),
        site_name: "Bunkr".to_owned(),
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original files".to_owned(),
            description: "Preserve every file in the album".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count,
        total_size_bytes,
    })
}

fn parse_collection_metadata(
    value: Value,
    requested_url: &str,
    site: GallerySite,
) -> Result<MediaAnalysis, AppError> {
    let events = value.as_array().ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not return structured gallery metadata.",
        )
    })?;
    let metadata_objects = events
        .iter()
        .filter_map(Value::as_array)
        .filter_map(|parts| match parts.first().and_then(Value::as_u64) {
            Some(2) => parts.get(1).and_then(Value::as_object),
            Some(3) => parts.get(2).and_then(Value::as_object),
            _ => None,
        })
        .collect::<Vec<_>>();
    if metadata_objects.is_empty() {
        return Err(AppError::new(
            "invalid_metadata",
            format!("gallery-dl did not identify a {} gallery.", site.label()),
        ));
    }

    let title = metadata_objects
        .iter()
        .find_map(|metadata| {
            [
                "album_name",
                "album",
                "gallery_name",
                "gallery",
                "set_title",
                "board",
                "collection",
                "title",
                "category",
            ]
            .iter()
            .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        })
        .map(clean_title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| format!("{} gallery", site.label()));
    let uploader = metadata_objects.iter().find_map(|metadata| {
        ["username", "user", "owner", "author", "account"]
            .iter()
            .find_map(|key| metadata.get(*key).and_then(Value::as_str))
            .map(clean_title)
            .filter(|value| !value.is_empty())
    });
    let item_count = metadata_objects.iter().find_map(|metadata| {
        ["count", "total", "num", "image_count", "pin_count"]
            .iter()
            .find_map(|key| metadata.get(*key).and_then(Value::as_u64))
            .and_then(|count| u32::try_from(count).ok())
    });
    let total_size_bytes = metadata_objects.iter().find_map(|metadata| {
        ["total_size", "total_size_bytes"]
            .iter()
            .find_map(|key| metadata.get(*key).and_then(Value::as_u64))
            .or_else(|| {
                metadata
                    .get("album_size")
                    .and_then(Value::as_str)
                    .and_then(parse_human_bytes)
            })
    });
    let thumbnail = metadata_objects
        .iter()
        .find_map(|metadata| {
            ["thumbnail", "thumb", "preview"]
                .iter()
                .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        })
        .filter(|url| validate_media_url(url).is_ok())
        .map(str::to_owned);

    Ok(MediaAnalysis {
        engine: DownloadEngineKind::GalleryDl,
        title,
        uploader,
        duration_seconds: None,
        thumbnail,
        webpage_url: requested_url.to_owned(),
        download_url: requested_url.to_owned(),
        extractor: format!("gallery-dl:{}", site.slug()),
        site_name: site.label().to_owned(),
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original files".to_owned(),
            description: "Preserve the original gallery files".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count,
        total_size_bytes,
    })
}

fn parse_media_metadata(value: Value, requested_url: &str) -> Result<MediaAnalysis, AppError> {
    let events = value.as_array().ok_or_else(|| {
        AppError::new(
            "invalid_metadata",
            "gallery-dl did not return structured Bunkr media metadata.",
        )
    })?;
    let metadata = events
        .iter()
        .filter_map(Value::as_array)
        .find_map(|parts| {
            (parts.first().and_then(Value::as_u64) == Some(3))
                .then(|| parts.get(2).and_then(Value::as_object))
                .flatten()
        })
        .ok_or_else(|| {
            AppError::new(
                "invalid_metadata",
                "gallery-dl did not identify an individual Bunkr file.",
            )
        })?;

    let mut title = ["filename", "name", "title", "slug", "id"]
        .iter()
        .find_map(|key| metadata.get(*key).and_then(Value::as_str))
        .map(clean_title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "Bunkr file".to_owned());
    if let Some(extension) = metadata
        .get("extension")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .and_then(clean_extension)
    {
        let suffix = format!(".{extension}");
        if !title.to_ascii_lowercase().ends_with(&suffix) {
            title.push_str(&suffix);
        }
    }
    let total_size_bytes = metadata.get("size").and_then(Value::as_u64);
    let thumbnail = metadata
        .get("thumbnail")
        .and_then(Value::as_str)
        .filter(|url| validate_media_url(url).is_ok())
        .map(str::to_owned);

    Ok(MediaAnalysis {
        engine: DownloadEngineKind::GalleryDl,
        title,
        uploader: None,
        duration_seconds: None,
        thumbnail,
        webpage_url: requested_url.to_owned(),
        download_url: requested_url.to_owned(),
        extractor: "gallery-dl:bunkr:media".to_owned(),
        site_name: "Bunkr".to_owned(),
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original file".to_owned(),
            description: "Preserve the original Bunkr file".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count: Some(1),
        total_size_bytes,
    })
}

pub(crate) fn parse_human_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    let number_end = value
        .char_indices()
        .find_map(|(index, character)| {
            (!character.is_ascii_digit() && character != '.').then_some(index)
        })
        .unwrap_or(value.len());
    let amount = value[..number_end].trim().parse::<f64>().ok()?;
    let unit = value[number_end..]
        .trim()
        .trim_end_matches("/s")
        .to_ascii_uppercase();
    if !amount.is_finite() || amount < 0.0 {
        return None;
    }
    let multiplier = match unit.as_str() {
        "" | "B" => 1.0,
        "KB" | "K" => 1_000.0,
        "MB" | "M" => 1_000_000.0,
        "GB" | "G" => 1_000_000_000.0,
        "TB" | "T" => 1_000_000_000_000.0,
        "KIB" => 1_024.0,
        "MIB" => 1_048_576.0,
        "GIB" => 1_073_741_824.0,
        _ => return None,
    };
    let bytes = amount * multiplier;
    (bytes <= u64::MAX as f64).then_some(bytes.round() as u64)
}

fn safe_album_directory(title: &str, album_id: &str) -> String {
    let mut clean = title
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | '<' | '>' | ':' | '"' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .take(120)
        .collect::<String>();
    clean = clean.trim().trim_matches('.').to_owned();
    if clean.is_empty() {
        clean = "Gallery".to_owned();
    }
    format!("{clean} [{album_id}]")
}

fn clean_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(200)
        .collect::<String>()
        .trim()
        .to_owned()
}

fn access_args(preferences: &EnginePreferences) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(cookie_file) = &preferences.managed_cookie_file {
        args.extend(["--cookies".to_owned(), cookie_file.clone()]);
    } else if let Some(browser) = preferences.browser_cookie_source.yt_dlp_name() {
        let browser = preferences
            .browser_profile
            .as_deref()
            .map(|profile| format!("{browser}:{profile}"))
            .unwrap_or_else(|| browser.to_owned());
        args.extend(["--cookies-from-browser".to_owned(), browser]);
    }
    if let Some(proxy) = &preferences.proxy_url {
        args.extend(["--proxy".to_owned(), proxy.clone()]);
    }
    if let Some(limit) = preferences.bandwidth_limit_kib {
        args.extend(["--limit-rate".to_owned(), format!("{limit}K")]);
    }
    args
}

fn friendly_error(details: &str) -> String {
    let lowercase = details.to_ascii_lowercase();
    if lowercase.contains("cookies") || lowercase.contains("login") {
        "Bunkr requires a browser session. Choose a browser cookie source in Settings, then try again.".to_owned()
    } else if lowercase.contains("403") || lowercase.contains("429") {
        "Bunkr blocked or rate-limited the request. Try again later or configure a proxy in Settings.".to_owned()
    } else {
        "gallery-dl could not analyze this Bunkr link.".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, DownloadOptions, GalleryItemSelection, OutputContainer, VideoCodecPreference,
    };
    use serde_json::json;

    fn request_for_analysis(
        analysis: MediaAnalysis,
        gallery_selection: Option<Vec<GalleryItemSelection>>,
    ) -> StartDownloadRequest {
        StartDownloadRequest {
            url: analysis.download_url.clone(),
            analysis,
            options: DownloadOptions {
                quality: QualityOption {
                    id: "original".into(),
                    label: "Original file".into(),
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
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                sponsorblock_categories: vec![],
                destination: "/tmp".into(),
                filename_template: "ignored".into(),
                output_conflict_policy: OutputConflictPolicy::Skip,
                gallery_selection,
            },
            scheduled_for: None,
            post_download_recipe_ids: Vec::new(),
        }
    }

    #[test]
    fn routes_only_supported_bunkr_urls() {
        assert!(is_bunkr_url("https://bunkr.cr/a/ul3n82nu"));
        assert!(is_bunkr_url("https://app.bunkr.cr/a/album_1/"));
        assert!(is_bunkr_url("https://bunkr.cr/f/eiK5ATuIv7DIY"));
        assert!(is_bunkr_url("https://bunkr.cr/v/video-1"));
        assert!(is_bunkr_url("https://bunkr.cr/i/image.name_1"));
        assert!(is_bunkr_url("https://bunkr.cr/d/download%20name"));
        assert!(!is_bunkr_url("https://bunkr.cr.example/a/ul3n82nu"));
        assert!(!is_bunkr_url("https://bunkr.cr/u/unsupported"));
        assert!(!is_bunkr_url("https://bunkr.cr/a/one/extra"));
        assert!(!is_bunkr_url("https://bunkr.cr/f/bad!id"));
        assert!(!is_bunkr_url("https://bunkr.cr/f/bad%ZZid"));
        assert!(!is_bunkr_url(&format!(
            "https://bunkr.cr/f/{}",
            "x".repeat(241)
        )));
        assert!(validate_bunkr_album_url("https://bunkr.cr/a/ul3n82nu").is_ok());
        assert!(validate_bunkr_album_url("https://bunkr.cr/f/eiK5ATuIv7DIY").is_err());
    }

    #[test]
    fn routes_only_explicit_tested_gallery_collection_shapes() {
        assert!(is_supported_gallery_url("https://imgur.com/a/album123"));
        assert!(is_supported_gallery_url(
            "https://imgur.com/gallery/album_123/"
        ));
        assert!(is_supported_gallery_url(
            "https://www.flickr.com/photos/example/albums/721777"
        ));
        assert!(is_supported_gallery_url(
            "https://www.flickr.com/photos/example/galleries/721778/"
        ));
        assert!(is_supported_gallery_url(
            "https://www.pinterest.com/example/board-name/"
        ));
        assert!(!is_supported_gallery_url("https://imgur.com/example"));
        assert!(!is_supported_gallery_url(
            "https://www.flickr.com/photos/example/721777"
        ));
        assert!(!is_supported_gallery_url(
            "https://www.pinterest.com/pin/12345/"
        ));
    }

    #[test]
    fn normalizes_generic_gallery_json_without_exposing_media_urls() {
        let fixture: Value =
            serde_json::from_str(include_str!("fixtures/imgur_gallery.json")).unwrap();
        let analysis =
            parse_collection_metadata(fixture, "https://imgur.com/a/album123", GallerySite::Imgur)
                .unwrap();
        assert_eq!(analysis.site_name, "Imgur");
        assert_eq!(analysis.extractor, "gallery-dl:imgur");
        assert_eq!(analysis.title, "Night sky");
        assert_eq!(analysis.uploader.as_deref(), Some("example"));
        assert_eq!(analysis.item_count, Some(2));
        assert!(!serde_json::to_string(&analysis).unwrap().contains("secret"));
    }

    #[test]
    fn parses_bounded_album_summary_without_preserving_media_url() {
        let signed = "https://cdn.example/file.mkv?token=secret";
        let analysis = parse_album_metadata(
            json!([
                [2, {"album_id":"ul3n82nu","album_name":"Album","album_size":"12.59 GB","count":67}],
                [3, signed, {"thumbnail":"https://i.example/thumb.png","size":215914671}]
            ]),
            "https://bunkr.cr/a/ul3n82nu",
        )
        .unwrap();
        assert_eq!(analysis.engine, DownloadEngineKind::GalleryDl);
        assert_eq!(analysis.item_count, Some(67));
        assert_eq!(analysis.total_size_bytes, Some(12_590_000_000));
        assert_eq!(analysis.download_url, "https://bunkr.cr/a/ul3n82nu");
        assert!(!serde_json::to_string(&analysis).unwrap().contains("secret"));
    }

    #[test]
    fn parses_single_media_without_preserving_signed_download_url() {
        let signed = "https://cdn.example/video.mp4?token=secret";
        let analysis = parse_media_metadata(
            json!([
                [2, {"category":"bunkr", "subcategory":"media"}],
                [3, signed, {
                    "id":"eiK5ATuIv7DIY",
                    "filename":"Example video\n",
                    "extension":"mp4",
                    "size":215914671,
                    "thumbnail":"https://i.example/thumb.png"
                }]
            ]),
            "https://bunkr.cr/f/eiK5ATuIv7DIY",
        )
        .unwrap();

        assert_eq!(analysis.engine, DownloadEngineKind::GalleryDl);
        assert_eq!(analysis.extractor, "gallery-dl:bunkr:media");
        assert_eq!(analysis.title, "Example video.mp4");
        assert_eq!(analysis.item_count, Some(1));
        assert_eq!(analysis.total_size_bytes, Some(215_914_671));
        assert_eq!(analysis.webpage_url, "https://bunkr.cr/f/eiK5ATuIv7DIY");
        assert_eq!(analysis.download_url, "https://bunkr.cr/f/eiK5ATuIv7DIY");
        assert!(!analysis.download_url.contains("token="));
        assert!(!analysis.webpage_url.contains("token="));
    }

    #[test]
    fn parses_sanitized_item_lines_and_classifies_videos() {
        let items = parse_item_list(
            r#"{"index":1,"count":2,"id":"one","filename":"First video","extension":"MP4","size":42,"thumbnail":"https://i.example/one.png"}
{"index":2,"count":2,"id":"two","filename":"Poster","extension":"jpg","size":12,"thumbnail":null}"#,
            3,
        )
        .unwrap();
        assert_eq!(items.total_count, 2);
        assert!(!items.truncated);
        assert_eq!(items.items[0].index, 4);
        assert!(items.items[0].is_video);
        assert!(!items.items[1].is_video);
        assert_eq!(items.items[0].extension.as_deref(), Some("mp4"));
    }

    #[test]
    fn creates_safe_numeric_gallery_ranges() {
        assert_eq!(build_range([3, 1].into_iter(), Some(4)).unwrap(), "1,3");
        assert!(build_range([3, 3].into_iter(), Some(4)).is_err());
        assert!(build_range([0].into_iter(), Some(4)).is_err());
        assert!(build_range([5].into_iter(), Some(4)).is_err());
        assert!(build_range([].into_iter(), Some(4)).is_err());
    }

    #[test]
    fn builds_safe_gallery_arguments() {
        let engine = GalleryDlEngine {
            executable: PathBuf::from("gallery-dl"),
        };
        let analysis = parse_album_metadata(
            json!([[2, {"album_id":"safe","album_name":"../Album","count":1}]]),
            "https://bunkr.cr/a/safe",
        )
        .unwrap();
        let request = StartDownloadRequest {
            url: analysis.download_url.clone(),
            analysis,
            options: DownloadOptions {
                quality: QualityOption {
                    id: "original".into(),
                    label: "Original files".into(),
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
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                sponsorblock_categories: vec![],
                destination: "/tmp".into(),
                filename_template: "ignored".into(),
                output_conflict_policy: OutputConflictPolicy::Skip,
                gallery_selection: None,
            },
            scheduled_for: None,
            post_download_recipe_ids: Vec::new(),
        };
        let prepared = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap();
        assert_eq!(prepared.output_directory, "_Album [safe]");
        assert_eq!(prepared.args.last().unwrap(), "https://bunkr.cr/a/safe");
        assert!(!prepared.args.iter().any(|arg| arg == "--no-skip"));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg.starts_with("output.mode={") && arg.contains("progress-total")));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg.starts_with("error:nicedl:gallery:error:")));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg == "downloader.http.sleep-429=exp:2:0:120=15"));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg == "downloader.http.retries=12"));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg == "downloader.http.sleep-retries=1-3"));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg == "downloader.http.retry-codes=[429]"));
    }

    #[test]
    fn builds_single_media_download_without_album_range() {
        let engine = GalleryDlEngine {
            executable: PathBuf::from("gallery-dl"),
        };
        let analysis = parse_media_metadata(
            json!([[3, "https://cdn.example/file.mp4?token=secret", {
                "filename":"Example",
                "extension":"mp4",
                "size":42
            }]]),
            "https://bunkr.cr/f/eiK5ATuIv7DIY",
        )
        .unwrap();
        let request = request_for_analysis(analysis, None);

        let prepared = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap();

        assert_eq!(prepared.output_directory, ".");
        assert!(!prepared.args.iter().any(|arg| arg == "--range"));
        assert!(prepared
            .args
            .windows(2)
            .any(|args| args == ["--", "https://bunkr.cr/f/eiK5ATuIv7DIY"]));
        assert!(prepared
            .args
            .iter()
            .any(|arg| arg == "downloader.http.retries=12"));
        assert!(!prepared.args.iter().any(|arg| arg.contains("token=secret")));
    }

    #[test]
    fn rejects_album_selection_for_single_media() {
        let engine = GalleryDlEngine {
            executable: PathBuf::from("gallery-dl"),
        };
        let analysis = parse_media_metadata(
            json!([[3, "https://cdn.example/file.mp4", {"filename":"Example"}]]),
            "https://bunkr.cr/v/example",
        )
        .unwrap();
        let request = request_for_analysis(
            analysis,
            Some(vec![GalleryItemSelection {
                index: 1,
                size_bytes: Some(42),
            }]),
        );

        let error = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap_err();
        assert_eq!(error.code, "invalid_gallery_selection");
    }

    #[tokio::test]
    #[ignore = "requires gallery-dl and public Bunkr network access"]
    async fn analyzes_public_bunkr_album() {
        let analysis = GalleryDlEngine::discover()
            .unwrap()
            .analyze("https://bunkr.cr/a/ul3n82nu", &EnginePreferences::default())
            .await
            .unwrap();
        assert_eq!(analysis.engine, DownloadEngineKind::GalleryDl);
        assert!(analysis.item_count.is_some_and(|count| count > 1));
        assert!(analysis.total_size_bytes.is_some());
    }

    #[tokio::test]
    #[ignore = "requires gallery-dl and public Bunkr network access"]
    async fn lists_public_bunkr_album_items_without_media_urls() {
        let items = GalleryDlEngine::discover()
            .unwrap()
            .list_items(
                "https://bunkr.cr/a/ul3n82nu",
                &EnginePreferences::default(),
                0,
                12,
            )
            .await
            .unwrap();
        assert!(items.items.len() > 1);
        assert!(items
            .items
            .iter()
            .all(|item| !item.filename.contains("token=")));
        assert!(items.items.iter().any(|item| item.is_video));
        let second_page = GalleryDlEngine::discover()
            .unwrap()
            .list_items(
                "https://bunkr.cr/a/ul3n82nu",
                &EnginePreferences::default(),
                3,
                2,
            )
            .await
            .unwrap();
        assert_eq!(
            second_page
                .items
                .iter()
                .map(|item| item.index)
                .collect::<Vec<_>>(),
            [4, 5]
        );
    }

    #[tokio::test]
    #[ignore = "requires gallery-dl and public Bunkr network access"]
    async fn validates_public_bunkr_download_templates_without_downloading() {
        let engine = GalleryDlEngine::discover().unwrap();
        let analysis = engine
            .analyze("https://bunkr.cr/a/ul3n82nu", &EnginePreferences::default())
            .await
            .unwrap();
        let request = StartDownloadRequest {
            url: analysis.download_url.clone(),
            options: DownloadOptions {
                quality: analysis.quality_options[0].clone(),
                container: OutputContainer::Mp4,
                video_codec_preference: VideoCodecPreference::Any,
                format_strategy: crate::models::FormatStrategy::Recommended,
                audio_quality: AudioQuality::Best,
                audio_only: false,
                subtitles: false,
                subtitle_languages: vec![],
                embed_thumbnail: false,
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                sponsorblock_categories: vec![],
                destination: "/tmp".into(),
                filename_template: "ignored".into(),
                output_conflict_policy: OutputConflictPolicy::Skip,
                gallery_selection: Some(vec![
                    GalleryItemSelection {
                        index: 1,
                        size_bytes: Some(215_914_671),
                    },
                    GalleryItemSelection {
                        index: 3,
                        size_bytes: Some(200_188_175),
                    },
                ]),
            },
            analysis,
            scheduled_for: None,
            post_download_recipe_ids: Vec::new(),
        };
        let mut prepared = engine
            .build_download(&request, &EnginePreferences::default())
            .unwrap();
        let separator = prepared.args.iter().position(|arg| arg == "--").unwrap();
        prepared
            .args
            .splice(separator..separator, ["--no-download".to_owned()]);
        let output = Command::new(engine.executable())
            .args(prepared.args)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("nicedl:gallery:item:1|"));
        assert!(stdout.contains("nicedl:gallery:item:3|"));
        assert!(!stdout.contains("nicedl:gallery:item:2|"));
    }
}
