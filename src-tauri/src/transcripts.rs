use crate::models::LibraryTranscriptCue;
use std::{ffi::OsStr, fs, path::Path};

const MAX_SUBTITLE_TRACKS: usize = 20;
const MAX_SUBTITLE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CUES_PER_ITEM: usize = 50_000;
const MAX_CUE_CHARACTERS: usize = 2_000;

pub fn read_transcript_cues(media_path: &Path) -> Vec<LibraryTranscriptCue> {
    let Some(parent) = media_path.parent() else {
        return Vec::new();
    };
    let mut subtitle_paths = fs::read_dir(parent)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name().to_str()?.to_owned();
            if !subtitle_name_matches(media_path, &file_name) {
                return None;
            }
            let metadata = entry.path().symlink_metadata().ok()?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.len() > MAX_SUBTITLE_BYTES
            {
                return None;
            }
            Some((file_name, entry.path()))
        })
        .collect::<Vec<_>>();
    subtitle_paths.sort_by(|left, right| left.0.cmp(&right.0));
    subtitle_paths.truncate(MAX_SUBTITLE_TRACKS);

    let mut cues = Vec::new();
    for (file_name, path) in subtitle_paths {
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Ok(contents) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let language = subtitle_language(media_path, &file_name);
        for (start_seconds, end_seconds, text) in parse_webvtt(contents) {
            cues.push(LibraryTranscriptCue {
                subtitle_name: file_name.clone(),
                language: language.clone(),
                start_seconds,
                end_seconds,
                text,
            });
            if cues.len() >= MAX_CUES_PER_ITEM {
                return cues;
            }
        }
    }
    cues
}

fn parse_webvtt(contents: &str) -> Vec<(f64, f64, String)> {
    let lines = contents
        .trim_start_matches('\u{feff}')
        .lines()
        .collect::<Vec<_>>();
    let mut cues = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let timing = lines[index].trim();
        let Some((start, remainder)) = timing.split_once("-->") else {
            index += 1;
            continue;
        };
        let Some(end) = remainder.split_whitespace().next() else {
            index += 1;
            continue;
        };
        let (Some(start_seconds), Some(end_seconds)) =
            (parse_timestamp(start.trim()), parse_timestamp(end))
        else {
            index += 1;
            continue;
        };
        index += 1;
        let mut cue_lines = Vec::new();
        while index < lines.len() && !lines[index].trim().is_empty() {
            cue_lines.push(lines[index].trim());
            index += 1;
        }
        let text = normalize_cue_text(&cue_lines.join(" "));
        if !text.is_empty() && end_seconds > start_seconds {
            cues.push((start_seconds, end_seconds, text));
        }
    }
    cues
}

pub fn parse_webvtt_cues(
    contents: &str,
    subtitle_name: &str,
    language: Option<&str>,
) -> Vec<LibraryTranscriptCue> {
    parse_webvtt(contents)
        .into_iter()
        .map(|(start_seconds, end_seconds, text)| LibraryTranscriptCue {
            subtitle_name: subtitle_name.chars().take(240).collect(),
            language: language.map(|value| value.chars().take(24).collect()),
            start_seconds,
            end_seconds,
            text,
        })
        .collect()
}

fn parse_timestamp(value: &str) -> Option<f64> {
    let normalized = value.replace(',', ".");
    let parts = normalized.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (
            0.0,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        [hours, minutes, seconds] => (
            hours.parse::<f64>().ok()?,
            minutes.parse::<f64>().ok()?,
            seconds.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    let total = hours * 3_600.0 + minutes * 60.0 + seconds;
    (total.is_finite() && total >= 0.0 && minutes < 60.0 && seconds < 60.0).then_some(total)
}

fn normalize_cue_text(value: &str) -> String {
    let mut plain = String::with_capacity(value.len());
    let mut inside_tag = false;
    for character in value.chars() {
        match character {
            '<' => inside_tag = true,
            '>' if inside_tag => inside_tag = false,
            _ if !inside_tag => plain.push(character),
            _ => {}
        }
    }
    let decoded = plain
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");
    decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_CUE_CHARACTERS)
        .collect()
}

fn subtitle_name_matches(media_path: &Path, file_name: &str) -> bool {
    let Some(media_stem) = media_path.file_stem().and_then(OsStr::to_str) else {
        return false;
    };
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".vtt")
        && (file_name == format!("{media_stem}.vtt")
            || file_name.starts_with(&format!("{media_stem}.")))
}

fn subtitle_language(media_path: &Path, file_name: &str) -> Option<String> {
    let media_stem = media_path.file_stem()?.to_str()?;
    let suffix = file_name
        .strip_prefix(media_stem)?
        .strip_suffix(".vtt")?
        .trim_matches('.');
    (!suffix.is_empty()
        && suffix.len() <= 15
        && suffix
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-'))
    .then(|| suffix.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn discovers_only_same_stem_webvtt_tracks_and_extracts_language() {
        let directory = tempdir().expect("temporary directory");
        let media_path = directory.path().join("movie.mp4");
        fs::write(
            directory.path().join("movie.en.vtt"),
            "WEBVTT\n\n00:01.000 --> 00:02.000\nMatched cue\n",
        )
        .expect("matching subtitle");
        fs::write(
            directory.path().join("other.vtt"),
            "WEBVTT\n\n00:03.000 --> 00:04.000\nWrong video\n",
        )
        .expect("other subtitle");

        let cues = read_transcript_cues(&media_path);
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].subtitle_name, "movie.en.vtt");
        assert_eq!(cues[0].language.as_deref(), Some("en"));
        assert_eq!(cues[0].text, "Matched cue");
    }

    #[test]
    fn parses_bounded_webvtt_cues_and_removes_markup() {
        let cues = parse_webvtt(
            "\u{feff}WEBVTT\n\nfirst\n00:00:01.250 --> 00:00:03.500 align:start\n<v Alex>Hello &amp; <b>welcome</b></v>\n\n00:04.000 --> 00:05.000\nSecond line\n",
        );
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0], (1.25, 3.5, "Hello & welcome".to_owned()));
        assert_eq!(cues[1], (4.0, 5.0, "Second line".to_owned()));
    }

    #[test]
    fn rejects_invalid_or_reverse_timestamps() {
        let cues = parse_webvtt(
            "WEBVTT\n\nnot-a-time --> 00:00:02.000\nBad\n\n00:00:05.000 --> 00:00:04.000\nReverse\n",
        );
        assert!(cues.is_empty());
    }
}
