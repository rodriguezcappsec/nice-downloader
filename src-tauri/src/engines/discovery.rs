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

const ENGINES: [EngineDefinition; 3] = [
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
];

pub async fn detect_all() -> Vec<EngineInfo> {
    let mut results = Vec::with_capacity(ENGINES.len());
    for definition in ENGINES {
        results.push(detect(definition).await);
    }
    results
}

async fn detect(definition: EngineDefinition) -> EngineInfo {
    let path = match which::which(definition.executable) {
        Ok(path) => path,
        Err(_) => {
            return EngineInfo {
                id: definition.id,
                display_name: definition.display_name.to_owned(),
                installed: false,
                executable_path: None,
                version: None,
                required: definition.required,
                status_message: format!("{} was not found in PATH", definition.executable),
                capabilities: Vec::new(),
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
