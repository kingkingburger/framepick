//! YouTube URL validation — backend validation to complement frontend checks.
//!
//! Provides `validate_youtube_url` Tauri command and `extract_video_id` utility.

use serde::{Deserialize, Serialize};

/// Result of YouTube URL validation.
#[derive(Debug, Serialize, Deserialize)]
pub struct UrlValidationResult {
    /// Whether the URL is a valid YouTube video URL.
    pub valid: bool,
    /// Extracted 11-character video ID (empty if invalid).
    pub video_id: String,
    /// Human-readable error message (empty if valid).
    pub error: String,
}

/// Extract the 11-character video ID from a YouTube URL.
///
/// Supports:
/// - `https://www.youtube.com/watch?v=VIDEO_ID`
/// - `https://youtu.be/VIDEO_ID`
/// - `https://www.youtube.com/embed/VIDEO_ID`
/// - `https://www.youtube.com/shorts/VIDEO_ID`
/// - URLs with additional query parameters
/// - Both http and https
///
/// Returns `Some(video_id)` if valid, `None` otherwise.
pub fn extract_video_id(url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }

    // Try youtu.be short URLs
    if let Some(rest) = url
        .strip_prefix("https://youtu.be/")
        .or_else(|| url.strip_prefix("http://youtu.be/"))
    {
        let id = rest.split(['?', '&', '/', '#']).next().unwrap_or("");
        if is_valid_video_id(id) {
            return Some(id.to_string());
        }
    }

    // Try youtube.com URLs (with or without www, http/https)
    if url.contains("youtube.com") {
        // Shorts URL: /shorts/VIDEO_ID
        if let Some(rest) = url.split("/shorts/").nth(1) {
            let id = rest.split(['?', '&', '/', '#']).next().unwrap_or("");
            if is_valid_video_id(id) {
                return Some(id.to_string());
            }
        }

        // Embed URL: /embed/VIDEO_ID
        if let Some(rest) = url.split("/embed/").nth(1) {
            let id = rest.split(['?', '&', '/', '#']).next().unwrap_or("");
            if is_valid_video_id(id) {
                return Some(id.to_string());
            }
        }

        // Standard watch URL: ?v=VIDEO_ID
        if let Some(query_start) = url.find('?') {
            let query = &url[query_start + 1..];
            for param in query.split('&') {
                if let Some(id) = param.strip_prefix("v=") {
                    let id = id.split(['&', '#']).next().unwrap_or("");
                    if is_valid_video_id(id) {
                        return Some(id.to_string());
                    }
                }
            }
        }
    }

    None
}

/// YouTube video IDs are exactly 11 characters: [A-Za-z0-9_-]
fn is_valid_video_id(id: &str) -> bool {
    id.len() == 11 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Tauri command: validate a YouTube URL and return the video ID.
///
/// Called by the frontend for server-side validation before adding to queue.
/// Accepts both single video URLs and playlist URLs.
#[tauri::command]
pub fn validate_youtube_url(url: String) -> UrlValidationResult {
    // First try to extract a video ID
    if let Some(video_id) = extract_video_id(&url) {
        return UrlValidationResult {
            valid: true,
            video_id,
            error: String::new(),
        };
    }

    // Also accept playlist-only URLs (no video ID but has list= parameter)
    let detection = crate::playlist::detect_playlist(&url);
    if detection.is_playlist {
        return UrlValidationResult {
            valid: true,
            video_id: String::new(), // No specific video ID for playlist URLs
            error: String::new(),
        };
    }

    UrlValidationResult {
        valid: false,
        video_id: String::new(),
        error: "Invalid YouTube URL".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_watch_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_standard_without_www() {
        assert_eq!(
            extract_video_id("https://youtube.com/watch?v=dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_short_url_http() {
        assert_eq!(
            extract_video_id("http://youtu.be/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_embed_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/embed/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_shorts_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_url_with_extra_params() {
        assert_eq!(
            extract_video_id(
                "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOeiKm4"
            ),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_url_with_timestamp() {
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ?t=42"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_url_with_hash() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ#section"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_whitespace_trimmed() {
        assert_eq!(
            extract_video_id("  https://youtu.be/dQw4w9WgXcQ  "),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_invalid_empty() {
        assert_eq!(extract_video_id(""), None);
    }

    #[test]
    fn test_invalid_not_youtube() {
        assert_eq!(extract_video_id("https://google.com"), None);
    }

    #[test]
    fn test_invalid_random_text() {
        assert_eq!(extract_video_id("not a url at all"), None);
    }

    #[test]
    fn test_invalid_short_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=short"),
            None
        );
    }

    #[test]
    fn test_invalid_long_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=toolongvideoid123"),
            None
        );
    }

    #[test]
    fn test_invalid_special_chars_in_id() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=abc!@#$%^&*()"),
            None
        );
    }

    #[test]
    fn test_validate_command_valid() {
        let result = validate_youtube_url("https://youtu.be/dQw4w9WgXcQ".to_string());
        assert!(result.valid);
        assert_eq!(result.video_id, "dQw4w9WgXcQ");
        assert!(result.error.is_empty());
    }

    #[test]
    fn test_validate_command_invalid() {
        let result = validate_youtube_url("not-a-url".to_string());
        assert!(!result.valid);
        assert!(result.video_id.is_empty());
        assert!(!result.error.is_empty());
    }

    #[test]
    fn test_video_id_characters() {
        // Test IDs with hyphens and underscores
        assert_eq!(
            extract_video_id("https://youtu.be/abc-def_123"),
            Some("abc-def_123".to_string())
        );
    }
}
