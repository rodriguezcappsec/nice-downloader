use crate::{
    errors::AppError,
    models::{
        DownloadEngineKind, EnginePreferences, MediaAnalysis, OutputConflictPolicy, QualityKind,
        QualityOption, StartDownloadRequest,
    },
    utils::validation::validate_media_url,
};
use std::path::PathBuf;
use url::Url;

const DIRECT_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "m4v", "mp3", "m4a", "aac", "ogg", "opus", "flac", "wav",
    "zip", "7z", "tar", "gz", "jpg", "jpeg", "png", "gif", "webp",
];

#[derive(Debug, Clone)]
pub struct DirectHttpDownload {
    pub executable: PathBuf,
    pub engine_name: &'static str,
    pub args: Vec<String>,
    pub filename: String,
}

pub fn is_direct_download_url(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    url.path_segments()
        .and_then(Iterator::last)
        .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
        .is_some_and(|extension| {
            DIRECT_EXTENSIONS
                .iter()
                .any(|allowed| extension.eq_ignore_ascii_case(allowed))
        })
}

pub async fn analyze_direct(url: &str) -> Result<MediaAnalysis, AppError> {
    validate_media_url(url)?;
    let parsed = Url::parse(url)
        .map_err(|error| AppError::internal("could not parse the direct download URL", error))?;
    let filename = safe_filename(
        parsed
            .path_segments()
            .and_then(Iterator::last)
            .unwrap_or("download"),
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|error| AppError::internal("could not initialize HTTP analysis", error))?;
    let size = match client.head(parsed.clone()).send().await {
        Ok(response) if response.status().is_success() => response.content_length(),
        _ => None,
    };
    let host = parsed.host_str().unwrap_or("Direct HTTP").to_owned();
    Ok(MediaAnalysis {
        engine: DownloadEngineKind::DirectHttp,
        title: filename.clone(),
        uploader: None,
        duration_seconds: None,
        thumbnail: None,
        webpage_url: parsed.to_string(),
        download_url: parsed.to_string(),
        extractor: "direct-http".to_owned(),
        site_name: host,
        playlist_title: None,
        playlist_index: None,
        formats: Vec::new(),
        quality_options: vec![QualityOption {
            id: "original".to_owned(),
            label: "Original file".to_owned(),
            description: "No conversion".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        }],
        item_count: Some(1),
        total_size_bytes: size,
    })
}

pub fn prepare_direct(
    request: &StartDownloadRequest,
    preferences: &EnginePreferences,
) -> Result<DirectHttpDownload, AppError> {
    validate_media_url(&request.url)?;
    let parsed = Url::parse(&request.url)
        .map_err(|error| AppError::internal("could not parse the direct download URL", error))?;
    let filename = safe_filename(
        parsed
            .path_segments()
            .and_then(Iterator::last)
            .unwrap_or("download"),
    );
    let output_path = std::path::Path::new(&request.options.destination).join(&filename);
    if output_path.exists() && request.options.output_conflict_policy == OutputConflictPolicy::Skip
    {
        return Err(AppError::new(
            "output_exists",
            "The direct-download output already exists. Choose overwrite or a different destination.",
        ));
    }
    if let Some(executable) = crate::engines::managed::resolve("aria2c") {
        let mut args = vec![
            "--continue=true".to_owned(),
            "--max-tries=12".to_owned(),
            "--retry-wait=3".to_owned(),
            "--connect-timeout=20".to_owned(),
            "--timeout=60".to_owned(),
            "--file-allocation=none".to_owned(),
            "--auto-file-renaming=false".to_owned(),
            "--allow-overwrite=true".to_owned(),
            "--console-log-level=warn".to_owned(),
            "--summary-interval=1".to_owned(),
            format!("--out={filename}"),
        ];
        if request.options.output_conflict_policy == OutputConflictPolicy::Skip {
            args.push("--conditional-get=true".to_owned());
        }
        if let Some(proxy) = &preferences.proxy_url {
            args.push(format!("--all-proxy={proxy}"));
        }
        if let Some(limit) = preferences.bandwidth_limit_kib {
            args.push(format!("--max-download-limit={limit}K"));
        }
        args.push("--".to_owned());
        args.push(parsed.to_string());
        return Ok(DirectHttpDownload {
            executable,
            engine_name: "aria2c",
            args,
            filename,
        });
    }

    let executable = crate::engines::managed::resolve("curl").ok_or_else(|| {
        AppError::new(
            "transfer_engine_not_found",
            "A direct download requires aria2c or curl, but neither executable was found in PATH.",
        )
    })?;
    Ok(build_curl_download(
        executable,
        parsed,
        filename,
        preferences,
    ))
}

fn build_curl_download(
    executable: PathBuf,
    parsed: Url,
    filename: String,
    preferences: &EnginePreferences,
) -> DirectHttpDownload {
    let mut args = vec![
        "--location".to_owned(),
        "--fail".to_owned(),
        "--continue-at".to_owned(),
        "-".to_owned(),
        "--connect-timeout".to_owned(),
        "20".to_owned(),
        "--output".to_owned(),
        filename.clone(),
    ];
    if let Some(proxy) = &preferences.proxy_url {
        args.extend(["--proxy".to_owned(), proxy.clone()]);
    }
    if let Some(limit) = preferences.bandwidth_limit_kib {
        args.extend(["--limit-rate".to_owned(), format!("{limit}K")]);
    }
    args.push("--".to_owned());
    args.push(parsed.to_string());
    DirectHttpDownload {
        executable,
        engine_name: "curl",
        args,
        filename,
    }
}

fn safe_filename(value: &str) -> String {
    let decoded = value.replace(['/', '\\'], "_");
    let cleaned = decoded
        .chars()
        .filter(|character| !character.is_control())
        .take(180)
        .collect::<String>();
    if cleaned.is_empty() || matches!(cleaned.as_str(), "." | "..") {
        "download.bin".to_owned()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        AudioQuality, DownloadOptions, FormatStrategy, MediaAnalysis, OutputContainer, QualityKind,
        QualityOption, VideoCodecPreference,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };
    use tempfile::tempdir;

    #[test]
    fn routes_only_known_direct_file_extensions() {
        assert!(is_direct_download_url(
            "https://cdn.example/video.mp4?token=secret"
        ));
        assert!(!is_direct_download_url("https://example.com/watch?id=one"));
        assert!(!is_direct_download_url("file:///tmp/video.mp4"));
    }

    #[test]
    fn removes_path_separators_from_output_names() {
        assert_eq!(safe_filename("../video.mp4"), ".._video.mp4");
    }

    #[test]
    fn integration_retries_a_rate_limited_transfer() {
        let Some(curl) = which::which("curl").ok() else {
            return;
        };
        let server = TestServer::spawn(vec![
            Response::complete("429 Too Many Requests", b""),
            Response::complete("200 OK", b"recovered"),
        ]);
        let directory = tempdir().expect("temporary directory");
        let request = direct_request(&server.url("rate-limit.mp4"), directory.path());
        let download = build_curl_download(
            curl,
            Url::parse(&request.url).unwrap(),
            "rate-limit.mp4".to_owned(),
            &EnginePreferences::default(),
        );
        let output = run_curl_attempts(&download, directory.path(), 2);
        let requests = server.finish();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(directory.path().join("rate-limit.mp4")).unwrap(),
            b"recovered"
        );
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn integration_resumes_an_existing_partial_file_with_range() {
        let Some(curl) = which::which("curl").ok() else {
            return;
        };
        let server = TestServer::spawn(vec![Response {
            status: "206 Partial Content",
            body: b"world".to_vec(),
            declared_length: 5,
            extra_headers: vec!["Content-Range: bytes 6-10/11"],
        }]);
        let directory = tempdir().expect("temporary directory");
        std::fs::write(directory.path().join("resume.mp4"), b"hello ").unwrap();
        let request = direct_request(&server.url("resume.mp4"), directory.path());
        let download = build_curl_download(
            curl,
            Url::parse(&request.url).unwrap(),
            "resume.mp4".to_owned(),
            &EnginePreferences::default(),
        );
        let output = run_curl_attempts(&download, directory.path(), 1);
        let requests = server.finish();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(directory.path().join("resume.mp4")).unwrap(),
            b"hello world"
        );
        assert!(requests
            .iter()
            .any(|request| request.contains("Range: bytes=6-")));
    }

    #[test]
    fn integration_recovers_after_an_incomplete_response() {
        let Some(curl) = which::which("curl").ok() else {
            return;
        };
        let server = TestServer::spawn(vec![
            Response {
                status: "200 OK",
                body: b"hello ".to_vec(),
                declared_length: 11,
                extra_headers: vec![],
            },
            Response {
                status: "206 Partial Content",
                body: b"world".to_vec(),
                declared_length: 5,
                extra_headers: vec!["Content-Range: bytes 6-10/11"],
            },
        ]);
        let directory = tempdir().expect("temporary directory");
        let request = direct_request(&server.url("interrupted.mp4"), directory.path());
        let download = build_curl_download(
            curl,
            Url::parse(&request.url).unwrap(),
            "interrupted.mp4".to_owned(),
            &EnginePreferences::default(),
        );
        let output = run_curl_attempts(&download, directory.path(), 2);
        let requests = server.finish();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read(directory.path().join("interrupted.mp4")).unwrap(),
            b"hello world"
        );
        assert!(requests
            .iter()
            .any(|request| request.contains("Range: bytes=6-")));
    }

    fn run_curl_attempts(
        download: &DirectHttpDownload,
        directory: &std::path::Path,
        attempts: usize,
    ) -> std::process::Output {
        let mut output = None;
        for _ in 0..attempts {
            let result = std::process::Command::new(&download.executable)
                .args(&download.args)
                .current_dir(directory)
                .output()
                .expect("run curl");
            let succeeded = result.status.success();
            output = Some(result);
            if succeeded {
                break;
            }
        }
        output.expect("at least one curl attempt")
    }

    fn direct_request(url: &str, destination: &std::path::Path) -> StartDownloadRequest {
        let quality = QualityOption {
            id: "original".to_owned(),
            label: "Original file".to_owned(),
            description: "No conversion".to_owned(),
            kind: QualityKind::Best,
            max_height: None,
        };
        StartDownloadRequest {
            url: url.to_owned(),
            analysis: MediaAnalysis {
                engine: DownloadEngineKind::DirectHttp,
                title: "fixture".to_owned(),
                uploader: None,
                duration_seconds: None,
                thumbnail: None,
                webpage_url: url.to_owned(),
                download_url: url.to_owned(),
                extractor: "direct-http".to_owned(),
                site_name: "local fixture".to_owned(),
                playlist_title: None,
                playlist_index: None,
                formats: vec![],
                quality_options: vec![quality.clone()],
                item_count: Some(1),
                total_size_bytes: Some(11),
            },
            options: DownloadOptions {
                quality,
                container: OutputContainer::Mp4,
                video_codec_preference: VideoCodecPreference::Any,
                format_strategy: FormatStrategy::Recommended,
                audio_quality: AudioQuality::Best,
                audio_only: false,
                subtitles: false,
                subtitle_languages: vec![],
                embed_thumbnail: false,
                embed_metadata: false,
                chapters: false,
                sponsorblock: false,
                sponsorblock_categories: vec![],
                destination: destination.to_string_lossy().into_owned(),
                filename_template: "ignored".to_owned(),
                output_conflict_policy: OutputConflictPolicy::Overwrite,
                gallery_selection: None,
            },
            scheduled_for: None,
        }
    }

    struct Response {
        status: &'static str,
        body: Vec<u8>,
        declared_length: usize,
        extra_headers: Vec<&'static str>,
    }

    impl Response {
        fn complete(status: &'static str, body: &[u8]) -> Self {
            Self {
                status,
                body: body.to_vec(),
                declared_length: body.len(),
                extra_headers: vec![],
            }
        }
    }

    struct TestServer {
        address: std::net::SocketAddr,
        requests: Arc<Mutex<Vec<String>>>,
        worker: Option<thread::JoinHandle<()>>,
    }

    impl TestServer {
        fn spawn(responses: Vec<Response>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("bind fixture server");
            let address = listener.local_addr().unwrap();
            let requests = Arc::new(Mutex::new(Vec::new()));
            let recorded = requests.clone();
            let worker = thread::spawn(move || {
                for response in responses {
                    let (mut stream, _) = listener.accept().expect("accept fixture request");
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 1024];
                    loop {
                        let read = stream.read(&mut buffer).expect("read fixture request");
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request.windows(4).any(|window| window == b"\r\n\r\n") {
                            break;
                        }
                    }
                    recorded
                        .lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&request).into_owned());
                    let mut headers = format!(
                        "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                        response.status, response.declared_length
                    );
                    for header in response.extra_headers {
                        headers.push_str(header);
                        headers.push_str("\r\n");
                    }
                    headers.push_str("\r\n");
                    stream.write_all(headers.as_bytes()).unwrap();
                    stream.write_all(&response.body).unwrap();
                    let _ = stream.flush();
                }
            });
            Self {
                address,
                requests,
                worker: Some(worker),
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}/{path}", self.address)
        }
        fn finish(mut self) -> Vec<String> {
            self.worker.take().unwrap().join().expect("fixture worker");
            self.requests.lock().unwrap().clone()
        }
    }
}
