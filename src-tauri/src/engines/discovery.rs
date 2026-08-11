use crate::{
    engines::ytdlp::detect_runtime_capabilities,
    models::{EngineCapability, EngineId, EngineInfo},
};
use std::path::Path;
use tokio::process::Command;

struct EngineDefinition {
    id: EngineId,
    executable: &'static str,
    display_name: &'static str,
    version_arg: &'static str,
    required: bool,
}

const ENGINES: [EngineDefinition; 6] = [
    EngineDefinition {
        id: EngineId::YtDlp,
        executable: "yt-dlp",
        display_name: "yt-dlp",
        version_arg: "--version",
        required: true,
    },
    EngineDefinition {
        id: EngineId::Ffmpeg,
        executable: "ffmpeg",
        display_name: "FFmpeg",
        version_arg: "-version",
        required: true,
    },
    EngineDefinition {
        id: EngineId::Ffprobe,
        executable: "ffprobe",
        display_name: "ffprobe",
        version_arg: "-version",
        required: false,
    },
    EngineDefinition {
        id: EngineId::Aria2,
        executable: "aria2c",
        display_name: "aria2",
        version_arg: "--version",
        required: false,
    },
    EngineDefinition {
        id: EngineId::Curl,
        executable: "curl",
        display_name: "curl",
        version_arg: "--version",
        required: false,
    },
    EngineDefinition {
        id: EngineId::GalleryDl,
        executable: "gallery-dl",
        display_name: "gallery-dl",
        version_arg: "--version",
        required: false,
    },
];

pub async fn detect_all() -> Vec<EngineInfo> {
    let mut results = Vec::with_capacity(ENGINES.len());
    for definition in ENGINES {
        results.push(detect(definition).await);
    }
    results
}

async fn detect(definition: EngineDefinition) -> EngineInfo {
    let path = match crate::engines::managed::resolve(definition.executable) {
        Some(path) => path,
        None => {
            return EngineInfo {
                id: definition.id,
                display_name: definition.display_name.to_owned(),
                installed: false,
                executable_path: None,
                version: None,
                required: definition.required,
                status_message: match definition.id {
                    EngineId::GalleryDl => "gallery-dl was not found in PATH. Install it to enable Bunkr album downloads.".to_owned(),
                    EngineId::Aria2 => "aria2c was not found; direct downloads will use curl when available.".to_owned(),
                    EngineId::Curl => "curl was not found; install aria2c or curl for direct downloads.".to_owned(),
                    _ => format!("{} was not found in PATH", definition.executable),
                },
                capabilities: match definition.id {
                    EngineId::GalleryDl => vec![EngineCapability {
                        id: "bunkrAlbums".to_owned(),
                        display_name: "Bunkr albums".to_owned(),
                        available: false,
                        status_message: "Install gallery-dl to enable this capability".to_owned(),
                    }],
                    EngineId::Aria2 | EngineId::Curl => vec![EngineCapability {
                        id: "directTransfers".to_owned(),
                        display_name: "Resumable direct transfers".to_owned(),
                        available: false,
                        status_message: "Install this engine to enable the transfer fallback".to_owned(),
                    }],
                    _ => Vec::new(),
                },
            };
        }
    };

    let version = read_version(&path, definition.version_arg).await;
    let capabilities = match definition.id {
        EngineId::YtDlp => {
            let runtime = detect_runtime_capabilities(&path).await;
            vec![
                EngineCapability {
                    id: "browserCookies".to_owned(),
                    display_name: "Browser cookies".to_owned(),
                    available: true,
                    status_message: "Supported when a browser source is selected below".to_owned(),
                },
                EngineCapability {
                    id: "browserImpersonation".to_owned(),
                    display_name: "Browser impersonation".to_owned(),
                    available: runtime.impersonation_available(),
                    status_message: runtime
                        .preferred_impersonation_target
                        .map(|target| format!("Available through {target}"))
                        .unwrap_or_else(|| {
                            "No compatible curl_cffi target is available".to_owned()
                        }),
                },
            ]
        }
        EngineId::GalleryDl => vec![EngineCapability {
            id: "bunkrAlbums".to_owned(),
            display_name: "Bunkr albums".to_owned(),
            available: true,
            status_message: "Album analysis and original-file downloads are available".to_owned(),
        }],
        EngineId::Aria2 | EngineId::Curl => vec![EngineCapability {
            id: "directTransfers".to_owned(),
            display_name: "Resumable direct transfers".to_owned(),
            available: true,
            status_message: if matches!(definition.id, EngineId::Aria2) {
                "Preferred direct HTTP transfer engine".to_owned()
            } else {
                "Automatic fallback when aria2c is unavailable".to_owned()
            },
        }],
        EngineId::Ffmpeg | EngineId::Ffprobe => Vec::new(),
    };
    EngineInfo {
        id: definition.id,
        display_name: definition.display_name.to_owned(),
        installed: true,
        executable_path: Some(path.to_string_lossy().into_owned()),
        version: version.clone(),
        required: definition.required,
        status_message: if version.is_some() {
            "Ready".to_owned()
        } else {
            "Executable found, but its version could not be read".to_owned()
        },
        capabilities,
    }
}

async fn read_version(path: &Path, version_arg: &str) -> Option<String> {
    let output = Command::new(path).arg(version_arg).output().await.ok()?;
    let text = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr)
    } else {
        String::from_utf8_lossy(&output.stdout)
    };
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(180).collect())
}
