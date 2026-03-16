//! YouTube playlist detection and video list fetching.
//!
//! Detects whether a URL is a YouTube playlist (or contains a `list=` parameter),
//! then uses `yt-dlp --flat-playlist --dump-json` to extract video titles, IDs,
//! and durations without downloading any media.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::cmd_util::HideWindow;

/// A single video entry extracted from a playlist via yt-dlp --flat-playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistEntry {
    /// YouTube video ID (11 characters)
    pub video_id: String,
    /// Video title (from playlist metadata)
    pub title: String,
    /// Duration in seconds (may be 0 if unavailable)
    pub duration: f64,
    /// Full watch URL constructed from the video ID
    pub url: String,
}

/// Result of a playlist fetch operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistResult {
    /// Whether the URL was detected as a playlist
    pub is_playlist: bool,
    /// Playlist title (if available)
    pub playlist_title: String,
    /// Number of videos in the playlist
    pub video_count: usize,
    /// List of video entries
    pub entries: Vec<PlaylistEntry>,
}

/// Result of playlist URL detection (client-side check, no yt-dlp needed).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistDetectionResult {
    /// Whether the URL appears to be or contain a playlist
    pub is_playlist: bool,
    /// Extracted playlist ID (empty if not a playlist)
    pub playlist_id: String,
    /// Whether the URL also contains a video ID (e.g., watch?v=X&list=Y)
    pub has_video_id: bool,
    /// Extracted video ID if present
    pub video_id: String,
}

// ─── Playlist URL Detection ─────────────────────────────────────────

/// YouTube playlist URL patterns:
/// - `youtube.com/playlist?list=PLAYLIST_ID`
/// - `youtube.com/watch?v=VIDEO_ID&list=PLAYLIST_ID`
/// - `youtu.be/VIDEO_ID?list=PLAYLIST_ID`
///
/// Playlist IDs typically start with PL, OL, UU, RD, etc. and are 13-64 chars,
/// but we accept any non-empty value for the `list` parameter.
pub fn detect_playlist(url: &str) -> PlaylistDetectionResult {
    let url = url.trim();
    let mut result = PlaylistDetectionResult {
        is_playlist: false,
        playlist_id: String::new(),
        has_video_id: false,
        video_id: String::new(),
    };

    if url.is_empty() {
        return result;
    }

    // Must be a YouTube URL
    let is_youtube = url.contains("youtube.com") || url.contains("youtu.be");
    if !is_youtube {
        return result;
    }

    // Extract playlist ID from `list=` query parameter
    if let Some(list_id) = extract_query_param(url, "list") {
        if !list_id.is_empty() {
            result.is_playlist = true;
            result.playlist_id = list_id;
        }
    }

    // Also check if it's a /playlist? URL without list= somehow (unlikely but safe)
    if !result.is_playlist && url.contains("/playlist") {
        // Only mark as playlist if there's actually a list param
        // (already handled above, this is a no-op safety check)
    }

    // Extract video ID if present
    if let Some(vid) = crate::url_validator::extract_video_id(url) {
        result.has_video_id = true;
        result.video_id = vid;
    }

    result
}

/// Extract a query parameter value from a URL string.
fn extract_query_param(url: &str, param_name: &str) -> Option<String> {
    let query_start = url.find('?')?;
    let query = &url[query_start + 1..];
    let prefix = format!("{param_name}=");

    for part in query.split('&') {
        // Also handle fragments
        let part = part.split('#').next().unwrap_or(part);
        if let Some(value) = part.strip_prefix(&prefix) {
            let value = value.split('&').next().unwrap_or(value);
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

// ─── yt-dlp Playlist Fetching ───────────────────────────────────────

/// Resolve the path to yt-dlp executable via tools_manager.
fn resolve_ytdlp_path() -> PathBuf {
    crate::tools_manager::resolve_ytdlp_path()
}

/// Raw JSON entry from yt-dlp --flat-playlist --dump-json output.
/// Each line of output is a JSON object with these fields.
#[derive(Debug, Deserialize)]
struct YtdlpFlatEntry {
    /// Video ID
    #[serde(default)]
    id: String,
    /// Video title
    #[serde(default)]
    title: String,
    /// Duration in seconds
    #[serde(default)]
    duration: Option<f64>,
    /// Full URL (may be present)
    #[serde(default)]
    url: Option<String>,
}

/// Fetch playlist entries using `yt-dlp --flat-playlist --dump-json`.
///
/// This runs yt-dlp as a subprocess and parses the JSONL output.
/// Each line is a separate JSON object representing one video in the playlist.
///
/// Returns an error string if yt-dlp fails to execute or returns an error.
pub fn fetch_playlist_entries(url: &str) -> Result<PlaylistResult, String> {
    let ytdlp = resolve_ytdlp_path();

    let output = Command::new(&ytdlp)
        .args([
            "--flat-playlist",
            "--dump-json",
            "--no-warnings",
            "--no-check-certificates",
            "--socket-timeout",
            "30",
            url,
        ])
        .hide_window()
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute yt-dlp at '{}': {}",
                ytdlp.display(),
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "yt-dlp exited with {}: {}",
            output.status,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut playlist_title = String::new();

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse as a flat-playlist entry
        match serde_json::from_str::<YtdlpFlatEntry>(line) {
            Ok(raw) => {
                // Skip entries without a valid video ID
                if raw.id.is_empty() {
                    continue;
                }

                let video_url = raw
                    .url
                    .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", raw.id));

                entries.push(PlaylistEntry {
                    video_id: raw.id,
                    title: if raw.title.is_empty() {
                        "Untitled".to_string()
                    } else {
                        raw.title
                    },
                    duration: raw.duration.unwrap_or(0.0),
                    url: video_url,
                });
            }
            Err(e) => {
                // Try to extract playlist title from a different JSON structure
                // yt-dlp sometimes emits a playlist-level JSON first
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(t) = val.get("title").and_then(|v| v.as_str()) {
                        if playlist_title.is_empty() {
                            playlist_title = t.to_string();
                        }
                    }
                } else {
                    eprintln!("[playlist] Failed to parse yt-dlp JSON line: {e}");
                }
            }
        }
    }

    let video_count = entries.len();

    Ok(PlaylistResult {
        is_playlist: video_count > 0,
        playlist_title,
        video_count,
        entries,
    })
}

/// Format duration in seconds to a human-readable string (HH:MM:SS or MM:SS).
pub fn format_duration(seconds: f64) -> String {
    let total_secs = seconds.round() as u64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{secs:02}")
    } else {
        format!("{minutes}:{secs:02}")
    }
}

// ─── Tauri Commands ─────────────────────────────────────────────────

/// Detect whether a URL is a YouTube playlist without running yt-dlp.
///
/// This is a fast client-side check that examines URL patterns.
#[tauri::command]
pub fn detect_playlist_url(url: String) -> PlaylistDetectionResult {
    detect_playlist(&url)
}

/// Fetch the list of videos in a YouTube playlist using yt-dlp --flat-playlist.
///
/// This command runs yt-dlp as a subprocess, so it should be called from
/// an async context. The frontend should show a loading indicator while waiting.
#[tauri::command]
pub async fn fetch_playlist(url: String) -> Result<PlaylistResult, String> {
    // Run the blocking yt-dlp command on a background thread
    tauri::async_runtime::spawn_blocking(move || fetch_playlist_entries(&url))
        .await
        .map_err(|e| format!("Task join error: {e}"))?
}

// ─── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_playlist tests ───────────────────────────────────

    #[test]
    fn detect_standard_playlist_url() {
        let result = detect_playlist(
            "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
        assert!(!result.has_video_id);
        assert!(result.video_id.is_empty());
    }

    #[test]
    fn detect_watch_url_with_playlist() {
        let result = detect_playlist(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
        assert!(result.has_video_id);
        assert_eq!(result.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn detect_short_url_with_playlist() {
        let result = detect_playlist(
            "https://youtu.be/dQw4w9WgXcQ?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf");
        assert!(result.has_video_id);
        assert_eq!(result.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn detect_regular_watch_url_not_playlist() {
        let result = detect_playlist("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert!(!result.is_playlist);
        assert!(result.playlist_id.is_empty());
        assert!(result.has_video_id);
        assert_eq!(result.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn detect_empty_url() {
        let result = detect_playlist("");
        assert!(!result.is_playlist);
        assert!(result.playlist_id.is_empty());
    }

    #[test]
    fn detect_non_youtube_url() {
        let result = detect_playlist("https://vimeo.com/123456");
        assert!(!result.is_playlist);
    }

    #[test]
    fn detect_playlist_with_index_param() {
        let result = detect_playlist(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOe&index=3",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOe");
        assert!(result.has_video_id);
    }

    #[test]
    fn detect_playlist_with_fragment() {
        let result = detect_playlist(
            "https://www.youtube.com/playlist?list=PLrAXtmErZgOeiKm4#section",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOeiKm4");
    }

    #[test]
    fn detect_playlist_list_before_v() {
        // list= appears before v= in query string
        let result = detect_playlist(
            "https://www.youtube.com/watch?list=PLrAXtmErZgOe&v=dQw4w9WgXcQ",
        );
        assert!(result.is_playlist);
        assert_eq!(result.playlist_id, "PLrAXtmErZgOe");
        assert!(result.has_video_id);
        assert_eq!(result.video_id, "dQw4w9WgXcQ");
    }

    #[test]
    fn detect_mix_playlist() {
        // YouTube Mix playlists start with RD
        let result = detect_playlist(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=RDdQw4w9WgXcQ",
        );
        assert!(result.is_playlist);
        assert!(result.playlist_id.starts_with("RD"));
    }

    // ── extract_query_param tests ───────────────────────────────

    #[test]
    fn extract_list_param() {
        assert_eq!(
            extract_query_param("https://youtube.com/watch?v=abc&list=PL123", "list"),
            Some("PL123".to_string())
        );
    }

    #[test]
    fn extract_param_first_position() {
        assert_eq!(
            extract_query_param("https://youtube.com/playlist?list=PL123", "list"),
            Some("PL123".to_string())
        );
    }

    #[test]
    fn extract_param_missing() {
        assert_eq!(
            extract_query_param("https://youtube.com/watch?v=abc", "list"),
            None
        );
    }

    #[test]
    fn extract_param_no_query() {
        assert_eq!(
            extract_query_param("https://youtube.com/watch", "list"),
            None
        );
    }

    #[test]
    fn extract_param_with_fragment() {
        assert_eq!(
            extract_query_param("https://youtube.com/playlist?list=PL123#top", "list"),
            Some("PL123".to_string())
        );
    }

    // ── format_duration tests ───────────────────────────────────

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(45.0), "0:45");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(125.0), "2:05");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(3661.0), "1:01:01");
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0.0), "0:00");
    }

    #[test]
    fn format_duration_rounds() {
        assert_eq!(format_duration(59.7), "1:00");
    }

    // ── PlaylistEntry serialization tests ───────────────────────

    #[test]
    fn playlist_entry_serialization() {
        let entry = PlaylistEntry {
            video_id: "dQw4w9WgXcQ".to_string(),
            title: "Test Video".to_string(),
            duration: 212.0,
            url: "https://www.youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("dQw4w9WgXcQ"));
        assert!(json.contains("Test Video"));
        assert!(json.contains("212"));
    }

    #[test]
    fn playlist_result_empty() {
        let result = PlaylistResult {
            is_playlist: false,
            playlist_title: String::new(),
            video_count: 0,
            entries: Vec::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let loaded: PlaylistResult = serde_json::from_str(&json).unwrap();
        assert!(!loaded.is_playlist);
        assert_eq!(loaded.video_count, 0);
    }

    #[test]
    fn playlist_result_with_entries() {
        let result = PlaylistResult {
            is_playlist: true,
            playlist_title: "My Playlist".to_string(),
            video_count: 2,
            entries: vec![
                PlaylistEntry {
                    video_id: "abc12345678".to_string(),
                    title: "First".to_string(),
                    duration: 100.0,
                    url: "https://www.youtube.com/watch?v=abc12345678".to_string(),
                },
                PlaylistEntry {
                    video_id: "def12345678".to_string(),
                    title: "Second".to_string(),
                    duration: 200.0,
                    url: "https://www.youtube.com/watch?v=def12345678".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        let loaded: PlaylistResult = serde_json::from_str(&json).unwrap();
        assert!(loaded.is_playlist);
        assert_eq!(loaded.video_count, 2);
        assert_eq!(loaded.entries[0].title, "First");
        assert_eq!(loaded.entries[1].video_id, "def12345678");
    }

    // ── PlaylistDetectionResult serialization ───────────────────

    #[test]
    fn detection_result_serialization() {
        let result = PlaylistDetectionResult {
            is_playlist: true,
            playlist_id: "PL123".to_string(),
            has_video_id: true,
            video_id: "dQw4w9WgXcQ".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("is_playlist"));
        assert!(json.contains("playlist_id"));
        assert!(json.contains("PL123"));

        let loaded: PlaylistDetectionResult = serde_json::from_str(&json).unwrap();
        assert!(loaded.is_playlist);
        assert_eq!(loaded.playlist_id, "PL123");
    }

    // ── YtdlpFlatEntry deserialization ──────────────────────────

    #[test]
    fn parse_ytdlp_flat_entry_full() {
        let json = r#"{"id":"dQw4w9WgXcQ","title":"Rick Astley - Never Gonna Give You Up","duration":212.0,"url":"https://www.youtube.com/watch?v=dQw4w9WgXcQ"}"#;
        let entry: YtdlpFlatEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "dQw4w9WgXcQ");
        assert_eq!(entry.title, "Rick Astley - Never Gonna Give You Up");
        assert_eq!(entry.duration, Some(212.0));
        assert!(entry.url.is_some());
    }

    #[test]
    fn parse_ytdlp_flat_entry_minimal() {
        let json = r#"{"id":"abc12345678"}"#;
        let entry: YtdlpFlatEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "abc12345678");
        assert!(entry.title.is_empty());
        assert_eq!(entry.duration, None);
        assert!(entry.url.is_none());
    }

    #[test]
    fn parse_ytdlp_flat_entry_no_duration() {
        let json = r#"{"id":"abc12345678","title":"Test","url":null}"#;
        let entry: YtdlpFlatEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "abc12345678");
        assert_eq!(entry.title, "Test");
        assert_eq!(entry.duration, None);
    }

    #[test]
    fn parse_ytdlp_flat_entry_extra_fields() {
        // yt-dlp outputs many extra fields; our struct should ignore them
        let json = r#"{"id":"abc12345678","title":"Test","duration":100.0,"uploader":"Someone","view_count":1000,"_type":"url"}"#;
        let entry: YtdlpFlatEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.id, "abc12345678");
        assert_eq!(entry.duration, Some(100.0));
    }
}
