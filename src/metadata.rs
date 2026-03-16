//! YouTube video metadata fetching via yt-dlp --dump-json.
//!
//! Provides a `VideoMetadata` struct and `fetch_metadata()` function
//! that invokes yt-dlp to retrieve video information as JSON.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::cmd_util::HideWindow;

/// Core metadata for a YouTube video.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    /// YouTube video ID (e.g. "dQw4w9WgXcQ").
    pub id: String,
    /// Full video title.
    pub title: String,
    /// Channel name that uploaded the video.
    pub channel: String,
    /// Duration in seconds (may include fractional seconds).
    pub duration: f64,
    /// Upload date in YYYYMMDD format (e.g. "20091025").
    pub upload_date: String,
}

/// Raw JSON shape returned by `yt-dlp --dump-json`.
///
/// Only the fields we care about are listed; serde ignores the rest.
#[derive(Deserialize)]
struct YtDlpJson {
    id: String,
    title: String,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    upload_date: Option<String>,
}

/// Resolve the path to the yt-dlp executable.
///
/// Search order:
/// 1. `tools/yt-dlp.exe` next to the executable (managed by tools_manager).
/// 2. `yt-dlp.exe` directly next to the executable (portable/legacy).
/// 3. System PATH fallback.
pub fn resolve_ytdlp_path() -> PathBuf {
    crate::tools_manager::resolve_ytdlp_path()
}

/// Fetch metadata for a YouTube video URL using yt-dlp.
///
/// Runs `yt-dlp --dump-json --no-playlist <url>` and parses the JSON output
/// into a [`VideoMetadata`] struct. Returns an error string on failure.
pub fn fetch_metadata(url: &str) -> Result<VideoMetadata, String> {
    let ytdlp_path = resolve_ytdlp_path();

    let output = Command::new(&ytdlp_path)
        .args(["--dump-json", "--no-playlist", url])
        .hide_window()
        .output()
        .map_err(|e| {
            format!(
                "Failed to run yt-dlp ({}): {}",
                ytdlp_path.display(),
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp exited with error: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_metadata_json(&stdout)
}

/// Parse the JSON output of `yt-dlp --dump-json` into [`VideoMetadata`].
pub fn parse_metadata_json(json: &str) -> Result<VideoMetadata, String> {
    let raw: YtDlpJson =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse yt-dlp JSON: {e}"))?;

    let channel = raw
        .channel
        .or(raw.uploader)
        .unwrap_or_else(|| String::from("Unknown"));

    Ok(VideoMetadata {
        id: raw.id,
        title: raw.title,
        channel,
        duration: raw.duration.unwrap_or(0.0),
        upload_date: raw.upload_date.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata_full() {
        let json = r#"{
            "id": "dQw4w9WgXcQ",
            "title": "Rick Astley - Never Gonna Give You Up (Official Music Video)",
            "channel": "Rick Astley",
            "duration": 212.0,
            "upload_date": "20091025"
        }"#;
        let meta = parse_metadata_json(json).unwrap();
        assert_eq!(meta.id, "dQw4w9WgXcQ");
        assert_eq!(meta.title, "Rick Astley - Never Gonna Give You Up (Official Music Video)");
        assert_eq!(meta.channel, "Rick Astley");
        assert!((meta.duration - 212.0).abs() < f64::EPSILON);
        assert_eq!(meta.upload_date, "20091025");
    }

    #[test]
    fn test_parse_metadata_channel_fallback_to_uploader() {
        let json = r#"{
            "id": "abc123",
            "title": "Test Video",
            "uploader": "Test Uploader",
            "duration": 60.5,
            "upload_date": "20240101"
        }"#;
        let meta = parse_metadata_json(json).unwrap();
        assert_eq!(meta.channel, "Test Uploader");
    }

    #[test]
    fn test_parse_metadata_missing_optional_fields() {
        let json = r#"{
            "id": "xyz",
            "title": "Minimal Video"
        }"#;
        let meta = parse_metadata_json(json).unwrap();
        assert_eq!(meta.id, "xyz");
        assert_eq!(meta.title, "Minimal Video");
        assert_eq!(meta.channel, "Unknown");
        assert_eq!(meta.duration, 0.0);
        assert_eq!(meta.upload_date, "");
    }

    #[test]
    fn test_parse_metadata_invalid_json() {
        let result = parse_metadata_json("not valid json");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse yt-dlp JSON"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let meta = VideoMetadata {
            id: "dQw4w9WgXcQ".to_string(),
            title: "Never Gonna Give You Up".to_string(),
            channel: "Rick Astley".to_string(),
            duration: 212.0,
            upload_date: "20091025".to_string(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let loaded: VideoMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, meta.id);
        assert_eq!(loaded.title, meta.title);
        assert_eq!(loaded.channel, meta.channel);
        assert!((loaded.duration - meta.duration).abs() < f64::EPSILON);
        assert_eq!(loaded.upload_date, meta.upload_date);
    }

    #[test]
    fn test_resolve_ytdlp_path_returns_nonempty() {
        let path = resolve_ytdlp_path();
        assert!(!path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_resolve_ytdlp_path_correct_extension() {
        let path = resolve_ytdlp_path();
        let name = path.file_name().unwrap().to_string_lossy();
        if cfg!(windows) {
            assert!(name.ends_with(".exe"), "expected .exe on Windows, got {name}");
        } else {
            assert_eq!(name, "yt-dlp");
        }
    }
}
