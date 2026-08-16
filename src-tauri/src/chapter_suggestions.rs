use crate::{
    database::Database,
    engines::{managed, ytdlp::sanitize_diagnostic},
    errors::AppError,
    models::{
        LibraryChapterSuggestion, LibraryChapterSuggestionKind, LibraryMediaKind,
        LibraryTranscriptCue,
    },
    utils::process::sanitize_tokio_command,
};
use std::{path::Path, process::Stdio};
use tokio::{
    io::{AsyncReadExt, BufReader},
    process::Command,
    time::{timeout, Duration},
};

pub async fn suggest(
    database: &Database,
    item_id: &str,
    maximum_suggestions: usize,
) -> Result<Vec<LibraryChapterSuggestion>, AppError> {
    if !(4..=40).contains(&maximum_suggestions) {
        return Err(AppError::new(
            "invalid_chapter_suggestion_limit",
            "Generate between 4 and 40 chapter and highlight suggestions.",
        ));
    }
    let item = database.get_library_item(item_id)?;
    if item.missing
        || !matches!(
            item.media_kind,
            Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
        )
    {
        return Err(AppError::new(
            "chapter_video_unavailable",
            "Automatic chapters require an available Library video.",
        ));
    }
    let path = Path::new(&item.path);
    let metadata = path.symlink_metadata().map_err(|_| {
        AppError::new(
            "chapter_video_unavailable",
            "The video is no longer available for chapter analysis.",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::new(
            "invalid_chapter_video",
            "Automatic chapters accept regular, non-symlink video files only.",
        ));
    }
    let cues = database.library_transcript_cues_for_suggestions(item_id, 5_000)?;
    let ffmpeg = managed::resolve("ffmpeg");
    let scene_times = match &ffmpeg {
        Some(ffmpeg) => detect_scene_times(ffmpeg, path, item.duration_seconds).await?,
        None => Vec::new(),
    };
    if cues.is_empty() && scene_times.is_empty() {
        let message = if ffmpeg.is_some() {
            "FFmpeg inspected this video but found no distinct visual changes. Generate a local transcript to add spoken-context evidence."
        } else {
            "Generate a local transcript or install FFmpeg so chapter suggestions have local evidence."
        };
        return Err(AppError::new("chapter_evidence_unavailable", message));
    }
    Ok(build_suggestions(
        &cues,
        &scene_times,
        item.duration_seconds.unwrap_or_default(),
        maximum_suggestions,
    ))
}

async fn detect_scene_times(
    ffmpeg: &Path,
    media_path: &Path,
    duration_seconds: Option<f64>,
) -> Result<Vec<f64>, AppError> {
    let strong_changes =
        detect_scene_times_at_threshold(ffmpeg, media_path, duration_seconds, 0.32).await?;
    if !strong_changes.is_empty() {
        return Ok(strong_changes);
    }

    // Static-camera videos can change topics without hard edits. Retry only
    // after an empty conservative pass; downstream spacing bounds noisy motion.
    detect_scene_times_at_threshold(ffmpeg, media_path, duration_seconds, 0.01).await
}

async fn detect_scene_times_at_threshold(
    ffmpeg: &Path,
    media_path: &Path,
    duration_seconds: Option<f64>,
    threshold: f64,
) -> Result<Vec<f64>, AppError> {
    let analysis_seconds = duration_seconds
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .unwrap_or(7_200.0)
        .min(7_200.0);
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-loglevel", "info", "-nostdin", "-i"])
        .arg(media_path)
        .args(["-t", &format!("{analysis_seconds:.3}"), "-an", "-sn", "-vf"])
        .arg(format!("select=gt(scene\\,{threshold:.2}),showinfo"))
        .args(["-fps_mode", "vfr", "-f", "null", "-"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    sanitize_tokio_command(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::internal("could not start FFmpeg scene analysis", error))?;
    let stderr = child.stderr.take().ok_or_else(|| {
        AppError::new(
            "chapter_analysis_io",
            "FFmpeg scene diagnostics are unavailable.",
        )
    })?;
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        BufReader::new(stderr)
            .take(2 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .await
            .map(|_| String::from_utf8_lossy(&bytes).into_owned())
    });
    let status = match timeout(Duration::from_secs(20 * 60), child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            return Err(AppError::internal(
                "could not wait for FFmpeg scene analysis",
                error,
            ))
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stderr_task.abort();
            return Err(AppError::new(
                "chapter_analysis_timeout",
                "Scene analysis exceeded its 20 minute limit.",
            ));
        }
    };
    let diagnostic = stderr_task
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    if !status.success() {
        return Err(AppError::with_details(
            "chapter_analysis_failed",
            "FFmpeg could not inspect scene changes for this video.",
            sanitize_diagnostic(&diagnostic),
        ));
    }
    Ok(parse_scene_times(&diagnostic, analysis_seconds))
}

fn parse_scene_times(value: &str, maximum: f64) -> Vec<f64> {
    let mut times = value
        .lines()
        .filter_map(|line| line.split_once("pts_time:").map(|(_, rest)| rest))
        .filter_map(|rest| rest.split_whitespace().next())
        .filter_map(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= maximum)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| (*left - *right).abs() < 0.5);
    if times.len() > 500 {
        let last = times.len() - 1;
        times = (0..500).map(|index| times[index * last / 499]).collect();
    }
    times
}

fn build_suggestions(
    cues: &[LibraryTranscriptCue],
    scene_times: &[f64],
    duration_seconds: f64,
    maximum: usize,
) -> Vec<LibraryChapterSuggestion> {
    let chapter_limit = maximum.saturating_sub((maximum / 4).max(1)).max(1);
    let minimum_gap = if duration_seconds.is_finite() && duration_seconds > 0.0 {
        (duration_seconds / chapter_limit as f64 / 2.0).clamp(12.0, 180.0)
    } else {
        30.0
    };
    let mut candidates = vec![(0.0_f64, 1.0_f64, "Start of video".to_owned())];
    candidates.extend(
        scene_times
            .iter()
            .copied()
            .map(|time| (time, 0.82, "Strong local scene change".to_owned())),
    );
    for window in cues.windows(2) {
        if window[1].start_seconds - window[0].end_seconds >= 10.0 {
            candidates.push((
                window[1].start_seconds,
                0.76,
                "Transcript resumes after a pause".to_owned(),
            ));
        }
    }
    if !cues.is_empty() {
        let step = (cues.len() / chapter_limit.max(1)).max(1);
        candidates.extend(cues.iter().step_by(step).map(|cue| {
            (
                cue.start_seconds,
                0.68,
                "Transcript topic checkpoint".to_owned(),
            )
        }));
    }
    candidates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| right.1.total_cmp(&left.1))
    });
    let mut chapters: Vec<LibraryChapterSuggestion> = Vec::new();
    for (time, confidence, reason) in candidates {
        if chapters
            .last()
            .is_some_and(|chapter| time - chapter.start_seconds < minimum_gap)
        {
            continue;
        }
        let label = cue_label_near(cues, time)
            .unwrap_or_else(|| format!("Scene at {}", compact_timestamp(time)));
        chapters.push(LibraryChapterSuggestion {
            kind: LibraryChapterSuggestionKind::Chapter,
            label,
            start_seconds: time.max(0.0),
            end_seconds: None,
            confidence,
            reason,
        });
        if chapters.len() >= chapter_limit {
            break;
        }
    }

    let highlight_limit = maximum.saturating_sub(chapters.len());
    let mut highlights = cues
        .iter()
        .filter_map(|cue| {
            let score = highlight_score(&cue.text);
            (score > 0).then_some((score, cue))
        })
        .collect::<Vec<_>>();
    highlights.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.start_seconds.total_cmp(&right.1.start_seconds))
    });
    let mut accepted_highlights: Vec<LibraryChapterSuggestion> = Vec::new();
    for (score, cue) in highlights {
        if accepted_highlights
            .iter()
            .any(|highlight| (highlight.start_seconds - cue.start_seconds).abs() < 20.0)
        {
            continue;
        }
        accepted_highlights.push(LibraryChapterSuggestion {
            kind: LibraryChapterSuggestionKind::Highlight,
            label: cue_label(&cue.text).unwrap_or_else(|| "Transcript highlight".to_owned()),
            start_seconds: (cue.start_seconds - 2.0).max(0.0),
            end_seconds: Some((cue.end_seconds + 4.0).max(cue.start_seconds + 1.0)),
            confidence: (0.58 + f64::from(score.min(5)) * 0.06).min(0.88),
            reason: "Transcript wording suggests an emphasized moment".to_owned(),
        });
        if accepted_highlights.len() >= highlight_limit {
            break;
        }
    }
    chapters.extend(accepted_highlights);
    chapters.sort_by(|left, right| {
        left.start_seconds
            .total_cmp(&right.start_seconds)
            .then_with(|| match (left.kind, right.kind) {
                (
                    LibraryChapterSuggestionKind::Chapter,
                    LibraryChapterSuggestionKind::Highlight,
                ) => std::cmp::Ordering::Less,
                (
                    LibraryChapterSuggestionKind::Highlight,
                    LibraryChapterSuggestionKind::Chapter,
                ) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            })
    });
    chapters.truncate(maximum);
    chapters
}

fn highlight_score(text: &str) -> u32 {
    let normalized = text.to_ascii_lowercase();
    let mut score = u32::from(text.contains('!')) + u32::from(text.contains('?'));
    for keyword in [
        "important",
        "favorite",
        "best",
        "amazing",
        "wow",
        "remember",
        "finally",
    ] {
        score += u32::from(normalized.contains(keyword));
    }
    if (40..=240).contains(&text.chars().count()) {
        score += 1;
    }
    score
}

fn cue_label_near(cues: &[LibraryTranscriptCue], time: f64) -> Option<String> {
    cues.iter()
        .find(|cue| cue.end_seconds >= time && cue.start_seconds <= time + 20.0)
        .or_else(|| cues.iter().find(|cue| cue.start_seconds >= time))
        .and_then(|cue| cue_label(&cue.text))
}

fn cue_label(text: &str) -> Option<String> {
    let label = text
        .split_whitespace()
        .take(9)
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|character: char| !character.is_alphanumeric())
        .chars()
        .take(72)
        .collect::<String>();
    (!label.is_empty()).then_some(label)
}

fn compact_timestamp(seconds: f64) -> String {
    let seconds = seconds.max(0.0).round() as u64;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> LibraryTranscriptCue {
        LibraryTranscriptCue {
            subtitle_name: "test.vtt".to_owned(),
            language: Some("en".to_owned()),
            start_seconds: start,
            end_seconds: end,
            text: text.to_owned(),
        }
    }

    #[test]
    fn parses_bounded_scene_timestamps() {
        let parsed = parse_scene_times(
            "[showinfo] n:1 pts_time:4.25 pos:1\n[showinfo] n:2 pts_time:4.40 pos:2\n[showinfo] n:3 pts_time:18.0 pos:3",
            10.0,
        );
        assert_eq!(parsed, vec![4.25]);
    }

    #[test]
    fn bounded_scene_sampling_preserves_the_full_timeline() {
        let diagnostic = (1..=1_000)
            .map(|second| format!("[showinfo] pts_time:{second}"))
            .collect::<Vec<_>>()
            .join("\n");
        let parsed = parse_scene_times(&diagnostic, 1_000.0);
        assert_eq!(parsed.len(), 500);
        assert_eq!(parsed.first(), Some(&1.0));
        assert_eq!(parsed.last(), Some(&1_000.0));
    }

    #[test]
    fn suggestions_are_bounded_reviewable_and_mix_evidence() {
        let cues = vec![
            cue(0.0, 4.0, "Welcome to this rainy city walk"),
            cue(30.0, 36.0, "This is the best view and it is amazing!"),
            cue(90.0, 96.0, "Now we enter the quiet old station"),
        ];
        let suggestions = build_suggestions(&cues, &[28.0, 88.0], 120.0, 8);
        assert!(!suggestions.is_empty());
        assert!(suggestions.len() <= 8);
        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.kind == LibraryChapterSuggestionKind::Chapter));
        assert!(suggestions
            .iter()
            .any(|suggestion| suggestion.kind == LibraryChapterSuggestionKind::Highlight));
        assert!(suggestions
            .iter()
            .all(|suggestion| !suggestion.label.is_empty()));
    }
}
