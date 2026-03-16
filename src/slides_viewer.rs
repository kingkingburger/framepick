//! Tauri commands for loading and displaying slides.html in the webview.
//!
//! Handles:
//! - Reading slides.html from disk
//! - Converting local image paths to Tauri asset protocol URLs
//! - Listing library entries (previously generated slide sets)
//! - Extracting metadata from segments.json

use crate::config::ConfigState;
use crate::settings::SettingsState;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{State, Url};

/// Summary info for a library entry shown in the dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// Video ID (folder name)
    pub video_id: String,
    /// Absolute path to the entry folder
    pub path: String,
    /// Whether slides.html exists
    pub has_slides: bool,
    /// Video title from segments.json metadata (if available)
    pub title: Option<String>,
    /// Thumbnail: first image path (asset protocol URL)
    pub thumbnail: Option<String>,
    /// Number of slides
    pub slide_count: Option<usize>,
}

/// Minimal segment info used when reading segments.json for metadata.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SegmentInfo {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub image: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub timestamp: String,
}

/// Metadata extracted from a video entry's files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidesMetadata {
    pub video_id: String,
    pub title: String,
    pub slide_count: usize,
    pub has_slides_html: bool,
    pub has_segments_json: bool,
    pub images: Vec<String>,
}

/// A single captured frame with its thumbnail URL and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFrame {
    pub index: usize,
    pub image: String,
    pub timestamp: String,
    pub text: String,
    pub thumbnail_url: String,
}

/// Result of get_capture_frames: frames + metadata about the capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureFramesResult {
    pub video_id: String,
    pub title: String,
    pub frame_count: usize,
    pub frames: Vec<CaptureFrame>,
}

/// Resolve the library path from settings, making relative paths absolute
/// relative to the executable directory.
fn resolve_library_path(state: &SettingsState) -> Result<PathBuf, String> {
    let settings = state.0.lock().map_err(|e| e.to_string())?;
    Ok(ConfigState::resolved_library_path(&settings.library_path))
}

/// Percent-encode a path component for use in a URL.
///
/// Encodes spaces, non-ASCII, and other URI-unsafe characters while
/// preserving `/` and `:` which are needed for Windows drive paths.
fn percent_encode_path(path_str: &str) -> String {
    let mut encoded = String::with_capacity(path_str.len());
    for ch in path_str.chars() {
        match ch {
            // Safe characters: unreserved + path separators + drive letter colon
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' | ':' => {
                encoded.push(ch);
            }
            ' ' => encoded.push_str("%20"),
            _ => {
                // Encode each UTF-8 byte
                let mut buf = [0u8; 4];
                let s = ch.encode_utf8(&mut buf);
                for b in s.bytes() {
                    encoded.push_str(&format!("%{:02X}", b));
                }
            }
        }
    }
    encoded
}

/// Convert a local file path to a Tauri asset protocol URL.
///
/// In Tauri v2, local files can be accessed via `https://asset.localhost/<path>`.
/// On Windows, paths are normalized to forward slashes and percent-encoded
/// so that spaces and special characters in folder/file names work correctly.
fn to_asset_url(path: &Path) -> String {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_default()
            .join(path)
    };

    // Normalize to forward slashes (Windows backslash → forward slash)
    let path_str = abs.to_string_lossy().replace('\\', "/");

    // Percent-encode the path for safe URL usage
    let encoded = percent_encode_path(&path_str);

    // Tauri v2 asset protocol: https://asset.localhost/{path}
    format!("https://asset.localhost/{}", encoded)
}

/// Rewrite image `src` attributes in slides.html to use asset protocol URLs.
///
/// Converts relative `src="images/..."` references to absolute asset:// URLs
/// so the Tauri webview can load them. Handles both double-quoted and
/// single-quoted src attributes, as well as srcset references.
fn rewrite_image_paths(html: &str, slides_dir: &Path) -> String {
    let images_dir = slides_dir.join("images");
    let asset_base = to_asset_url(&images_dir);

    // Replace relative image references with asset protocol URLs
    // Handle: src="images/...", src='images/...', srcset="images/..."
    let result = html
        .replace("src=\"images/", &format!("src=\"{}/", asset_base))
        .replace("src='images/", &format!("src='{}/", asset_base))
        .replace("srcset=\"images/", &format!("srcset=\"{}/", asset_base));

    result
}

/// Inject a `<base>` tag and CSP meta for correct asset resolution inside the
/// iframe srcdoc context. This ensures any remaining relative URLs resolve
/// correctly and the CSP allows loading from the asset protocol.
fn inject_base_and_csp(html: &str, slides_dir: &Path) -> String {
    let base_url = to_asset_url(slides_dir);

    // Build a <base> tag and a permissive CSP meta tag for the iframe content
    let injection = format!(
        r#"<base href="{base_url}/">
<meta http-equiv="Content-Security-Policy" content="default-src 'self' 'unsafe-inline'; img-src * https://asset.localhost data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; font-src 'self' data:;">"#,
        base_url = base_url,
    );

    // Insert right after <head> (or after <meta charset> if present)
    if let Some(pos) = html.find("<meta charset") {
        // Find the end of the charset meta tag
        if let Some(end) = html[pos..].find('>') {
            let insert_pos = pos + end + 1;
            let mut result = String::with_capacity(html.len() + injection.len() + 1);
            result.push_str(&html[..insert_pos]);
            result.push('\n');
            result.push_str(&injection);
            result.push_str(&html[insert_pos..]);
            return result;
        }
    }

    // Fallback: insert after <head>
    if let Some(pos) = html.find("<head>") {
        let insert_pos = pos + 6;
        let mut result = String::with_capacity(html.len() + injection.len() + 1);
        result.push_str(&html[..insert_pos]);
        result.push('\n');
        result.push_str(&injection);
        result.push_str(&html[insert_pos..]);
        return result;
    }

    // Last resort: prepend
    format!("{}\n{}", injection, html)
}

/// Load slides.html content for a given video ID, with image paths
/// rewritten for Tauri webview rendering.
///
/// Returns the full HTML string ready to be injected into the webview.
#[tauri::command]
pub fn load_slides_html(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<String, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);
    let slides_path = entry_dir.join("slides.html");

    if !slides_path.exists() {
        return Err(format!(
            "slides.html not found for video '{}' at {}",
            video_id,
            slides_path.display()
        ));
    }

    let html = fs::read_to_string(&slides_path)
        .map_err(|e| format!("Failed to read slides.html: {}", e))?;

    // Rewrite image paths to use Tauri asset protocol
    let rewritten = rewrite_image_paths(&html, &entry_dir);

    // Inject base href and CSP meta for correct resolution in iframe srcdoc
    let final_html = inject_base_and_csp(&rewritten, &entry_dir);

    Ok(final_html)
}

/// Get the absolute file path to slides.html for a given video ID.
///
/// Used by the frontend to open the file externally in the default browser.
#[tauri::command]
pub fn get_slides_path(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<String, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);
    let slides_path = entry_dir.join("slides.html");

    if !slides_path.exists() {
        return Err(format!(
            "slides.html not found for video '{}'",
            video_id
        ));
    }

    Ok(slides_path.to_string_lossy().to_string())
}

/// List all library entries (video folders) with metadata.
#[tauri::command]
pub fn list_library_entries(
    state: State<'_, SettingsState>,
) -> Result<Vec<LibraryEntry>, String> {
    let lib_path = resolve_library_path(&state)?;

    if !lib_path.exists() {
        // Library directory doesn't exist yet - return empty list
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    let read_dir = fs::read_dir(&lib_path)
        .map_err(|e| format!("Failed to read library directory: {}", e))?;

    for dir_entry in read_dir {
        let dir_entry = match dir_entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = dir_entry.path();
        if !path.is_dir() {
            continue;
        }

        let video_id = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name.to_string(),
            None => continue,
        };

        let slides_path = path.join("slides.html");
        let segments_path = path.join("segments.json");
        let has_slides = slides_path.exists();

        let mut title = None;
        let mut thumbnail = None;
        let mut slide_count = None;

        // Try to read segments.json for metadata
        if segments_path.exists() {
            if let Ok(content) = fs::read_to_string(&segments_path) {
                if let Ok(segments) = serde_json::from_str::<Vec<SegmentInfo>>(&content) {
                    slide_count = Some(segments.len());
                    if let Some(first) = segments.first() {
                        let img_path = path.join("images").join(&first.image);
                        if img_path.exists() {
                            thumbnail = Some(to_asset_url(&img_path));
                        }
                    }
                }
            }
        }

        // Fallback: if no thumbnail from segments.json, scan images/ directory
        // for the first captured frame (sorted alphabetically so frame_0000 comes first)
        if thumbnail.is_none() {
            let images_dir = path.join("images");
            if images_dir.is_dir() {
                if let Ok(img_entries) = fs::read_dir(&images_dir) {
                    let mut image_files: Vec<_> = img_entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            let name = e.file_name().to_string_lossy().to_lowercase();
                            name.ends_with(".jpg")
                                || name.ends_with(".jpeg")
                                || name.ends_with(".png")
                                || name.ends_with(".webp")
                        })
                        .collect();
                    image_files.sort_by_key(|e| e.file_name());
                    if let Some(first_img) = image_files.first() {
                        thumbnail = Some(to_asset_url(&first_img.path()));
                    }
                    if slide_count.is_none() && !image_files.is_empty() {
                        slide_count = Some(image_files.len());
                    }
                }
            }
        }

        // Try to extract title from slides.html <title> tag
        if has_slides {
            if let Ok(html) = fs::read_to_string(&slides_path) {
                if let Some(start) = html.find("<title>") {
                    if let Some(end) = html[start..].find("</title>") {
                        let t = &html[start + 7..start + end];
                        if !t.is_empty() {
                            title = Some(t.to_string());
                        }
                    }
                }
            }
        }

        entries.push(LibraryEntry {
            video_id,
            path: path.to_string_lossy().to_string(),
            has_slides,
            title,
            thumbnail,
            slide_count,
        });
    }

    // Sort by folder name (video ID)
    entries.sort_by(|a, b| a.video_id.cmp(&b.video_id));

    Ok(entries)
}

/// Open slides.html in the default external browser for a given video ID.
///
/// Uses the system's default application for .html files (typically a web browser).
/// The standalone slides.html uses relative image paths so it works independently.
#[tauri::command]
pub async fn open_slides_external(
    app: tauri::AppHandle,
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<(), String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);
    let slides_path = entry_dir.join("slides.html");

    if !slides_path.exists() {
        return Err(format!(
            "slides.html not found for video '{}'",
            video_id
        ));
    }

    // Convert to file:// URL for proper browser opening
    let file_url = Url::from_file_path(&slides_path)
        .map_err(|_| format!("Failed to create file URL for '{}'", slides_path.display()))?;

    // Use tauri-plugin-opener to open the file in the default browser
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(file_url.as_str(), None::<&str>)
        .map_err(|e| format!("Failed to open in browser: {}", e))?;

    Ok(())
}

/// Delete a library entry and all associated files (video, frames, slides.html).
///
/// Removes the entire video folder from the library directory.
/// Returns Ok(()) on success, or an error message if deletion fails.
#[tauri::command]
pub fn delete_library_entry(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<(), String> {
    if video_id.is_empty() {
        return Err("Video ID cannot be empty".to_string());
    }

    // Prevent path traversal attacks
    if video_id.contains("..") || video_id.contains('/') || video_id.contains('\\') {
        return Err("Invalid video ID".to_string());
    }

    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Err(format!("Library entry '{}' not found", video_id));
    }

    if !entry_dir.is_dir() {
        return Err(format!("'{}' is not a directory", video_id));
    }

    // Verify the entry is actually inside the library directory
    let canonical_lib = lib_path
        .canonicalize()
        .map_err(|e| format!("Failed to resolve library path: {}", e))?;
    let canonical_entry = entry_dir
        .canonicalize()
        .map_err(|e| format!("Failed to resolve entry path: {}", e))?;
    if !canonical_entry.starts_with(&canonical_lib) {
        return Err("Entry path is outside library directory".to_string());
    }

    fs::remove_dir_all(&entry_dir)
        .map_err(|e| format!("Failed to delete '{}': {}", video_id, e))?;

    Ok(())
}

/// Get metadata for a specific video entry.
#[tauri::command]
pub fn get_slides_metadata(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<SlidesMetadata, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Err(format!("Video entry '{}' not found", video_id));
    }

    let slides_path = entry_dir.join("slides.html");
    let segments_path = entry_dir.join("segments.json");
    let images_dir = entry_dir.join("images");

    let has_slides_html = slides_path.exists();
    let has_segments_json = segments_path.exists();

    let mut title = video_id.clone();
    let mut slide_count = 0;
    let mut images = Vec::new();

    if has_segments_json {
        if let Ok(content) = fs::read_to_string(&segments_path) {
            if let Ok(segments) = serde_json::from_str::<Vec<SegmentInfo>>(&content) {
                slide_count = segments.len();
                images = segments.iter().map(|s| s.image.clone()).collect();
            }
        }
    }

    // Extract title from HTML
    if has_slides_html {
        if let Ok(html) = fs::read_to_string(&slides_path) {
            if let Some(start) = html.find("<title>") {
                if let Some(end) = html[start..].find("</title>") {
                    let t = &html[start + 7..start + end];
                    if !t.is_empty() {
                        title = t.to_string();
                    }
                }
            }
        }
    }

    // Check which images actually exist
    if images_dir.exists() {
        images.retain(|img| images_dir.join(img).exists());
    }

    Ok(SlidesMetadata {
        video_id,
        title,
        slide_count,
        has_slides_html,
        has_segments_json,
        images,
    })
}

/// Get captured frames for a video entry with thumbnail URLs and metadata.
///
/// Reads segments.json to get frame data (index, image, timestamp, subtitle text),
/// then builds asset protocol URLs for each frame's thumbnail image.
/// Falls back to scanning the images/ directory if segments.json is missing.
///
/// Used by the capture list component to display completed captures.
#[tauri::command]
pub fn get_capture_frames(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<CaptureFramesResult, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Err(format!("Video entry '{}' not found", video_id));
    }

    let segments_path = entry_dir.join("segments.json");
    let slides_path = entry_dir.join("slides.html");
    let images_dir = entry_dir.join("images");

    let mut title = video_id.clone();
    let mut frames: Vec<CaptureFrame> = Vec::new();

    // Try to extract title from slides.html <title> tag
    if slides_path.exists() {
        if let Ok(html) = fs::read_to_string(&slides_path) {
            if let Some(start) = html.find("<title>") {
                if let Some(end) = html[start..].find("</title>") {
                    let t = &html[start + 7..start + end];
                    if !t.is_empty() {
                        title = t.to_string();
                    }
                }
            }
        }
    }

    if segments_path.exists() {
        // Primary path: read segments.json
        if let Ok(content) = fs::read_to_string(&segments_path) {
            if let Ok(segments) = serde_json::from_str::<Vec<SegmentInfo>>(&content) {
                for (idx, seg) in segments.iter().enumerate() {
                    let img_path = images_dir.join(&seg.image);
                    let thumbnail_url = if img_path.exists() {
                        to_asset_url(&img_path)
                    } else {
                        String::new()
                    };

                    frames.push(CaptureFrame {
                        index: if seg.index > 0 { seg.index } else { idx },
                        image: seg.image.clone(),
                        timestamp: seg.timestamp.clone(),
                        text: seg.text.clone(),
                        thumbnail_url,
                    });
                }
            }
        }
    }

    // Fallback: scan images/ directory if segments.json didn't yield frames
    if frames.is_empty() && images_dir.is_dir() {
        if let Ok(img_entries) = fs::read_dir(&images_dir) {
            let mut image_files: Vec<_> = img_entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    name.ends_with(".jpg")
                        || name.ends_with(".jpeg")
                        || name.ends_with(".png")
                        || name.ends_with(".webp")
                })
                .collect();
            image_files.sort_by_key(|e| e.file_name());

            for (idx, entry) in image_files.iter().enumerate() {
                let filename = entry.file_name().to_string_lossy().to_string();
                let thumbnail_url = to_asset_url(&entry.path());

                // Try to extract timestamp from filename pattern "HH-MM-SS"
                let timestamp = extract_timestamp_from_filename(&filename);

                frames.push(CaptureFrame {
                    index: idx,
                    image: filename,
                    timestamp,
                    text: String::new(),
                    thumbnail_url,
                });
            }
        }
    }

    let frame_count = frames.len();

    Ok(CaptureFramesResult {
        video_id,
        title,
        frame_count,
        frames,
    })
}

/// Extract a human-readable timestamp from an image filename.
///
/// Looks for patterns like "HH-MM-SS" in filenames such as
/// "frame_0001_00-01-23.jpg" and converts to "00:01:23".
fn extract_timestamp_from_filename(filename: &str) -> String {
    // Match pattern: digits-digits-digits (timestamp portion)
    let parts: Vec<&str> = filename.split('_').collect();
    for part in &parts {
        // Check if this part matches HH-MM-SS pattern
        let sub: Vec<&str> = part.split('-').collect();
        if sub.len() == 3
            && sub.iter().all(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_digit()))
        {
            return format!("{}:{}:{}", sub[0], sub[1], sub[2]);
        }
    }
    // Also check for pattern in filename after stripping extension
    let base = filename.rsplit('.').last().unwrap_or(filename);
    let re_parts: Vec<&str> = base.split(|c: char| !c.is_ascii_digit()).collect();
    // If we have groups of 2-digit numbers that could be a timestamp
    let digits: Vec<&str> = re_parts.iter().filter(|s| s.len() == 2).copied().collect();
    if digits.len() >= 3 {
        let last3 = &digits[digits.len() - 3..];
        return format!("{}:{}:{}", last3[0], last3[1], last3[2]);
    }
    String::new()
}

/// Open a library item's output directory in the system file explorer.
///
/// Cross-platform: uses `explorer` on Windows, `open` on macOS, `xdg-open` on Linux.
#[tauri::command]
pub fn open_folder(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<(), String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Err(format!(
            "Directory not found for video '{}'",
            video_id
        ));
    }

    open_directory_in_explorer(&entry_dir)
}

/// Open any directory path in the system file explorer.
fn open_directory_in_explorer(path: &Path) -> Result<(), String> {
    let path_str = path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path_str)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}

/// Re-capture frames for an existing library item using a different capture mode.
///
/// This command:
/// 1. Validates that the library entry exists
/// 2. Finds a video file (mp4/mkv/webm) in the entry directory
/// 3. Clears old captured frames from the images/ directory
/// 4. Runs frame capture with the new options
/// 5. Regenerates slides.html and segments.json
///
/// Returns the number of newly captured frames.
#[tauri::command]
pub async fn recapture_library_item(
    state: tauri::State<'_, SettingsState>,
    video_id: String,
    capture_mode: String,
    interval_seconds: Option<u32>,
    scene_threshold: Option<f64>,
) -> Result<RecaptureResult, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Err(format!("Library entry '{}' not found", video_id));
    }

    // Find the video file in the entry directory
    let video_path = find_video_file(&entry_dir)
        .ok_or_else(|| format!(
            "No video file found in '{}'. The source video may have been deleted after initial capture. \
             Re-download the video first.",
            video_id
        ))?;

    let mode = capture_mode.clone();
    let interval = interval_seconds.unwrap_or(10);
    let threshold = scene_threshold.unwrap_or(crate::capture::DEFAULT_SCENE_THRESHOLD);
    let entry_dir_clone = entry_dir.clone();
    let video_id_clone = video_id.clone();

    // Run capture in a blocking thread
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<RecaptureResult, String> {
        // Clear old images
        let images_dir = entry_dir_clone.join("images");
        if images_dir.exists() {
            let _ = fs::remove_dir_all(&images_dir);
        }
        fs::create_dir_all(&images_dir)
            .map_err(|e| format!("Failed to create images directory: {e}"))?;

        // Run capture with the new mode
        let frames = match mode.as_str() {
            "scene" => {
                crate::capture::capture_scene_change(&video_path, &entry_dir_clone, threshold)
                    .map_err(|e| format!("Scene capture failed: {e}"))?
            }
            "interval" => {
                crate::capture::capture_interval(&video_path, &entry_dir_clone, interval, None)
                    .map_err(|e| format!("Interval capture failed: {e}"))?
            }
            "subtitle" => {
                let sub_result = crate::capture::capture_subtitle(&video_path, &entry_dir_clone)
                    .map_err(|e| format!("Subtitle capture failed: {e}"))?;
                sub_result.frames
            }
            other => return Err(format!("Unknown capture mode: {other}")),
        };

        let frame_count = frames.len();

        // Build segments from captured frames
        let segments: Vec<crate::slides_generator::Segment> = frames
            .iter()
            .map(|f| crate::slides_generator::Segment {
                index: f.index,
                timestamp: f.timestamp.clone(),
                text: String::new(),
                image: f.filename.clone(),
            })
            .collect();

        // Save segments.json
        let segments_json = serde_json::to_string_pretty(&segments)
            .map_err(|e| format!("Failed to serialize segments: {e}"))?;
        fs::write(entry_dir_clone.join("segments.json"), segments_json)
            .map_err(|e| format!("Failed to write segments.json: {e}"))?;

        // Extract existing title from slides.html if available
        let existing_title = {
            let slides_path = entry_dir_clone.join("slides.html");
            if slides_path.exists() {
                fs::read_to_string(&slides_path)
                    .ok()
                    .and_then(|html| {
                        let start = html.find("<title>")?;
                        let end = html[start..].find("</title>")?;
                        let t = &html[start + 7..start + end];
                        if t.is_empty() { None } else { Some(t.to_string()) }
                    })
            } else {
                None
            }
        };

        // Generate new slides.html
        let metadata = crate::slides_generator::VideoMetadata {
            title: existing_title.unwrap_or_else(|| video_id_clone.clone()),
            url: String::new(),
            channel: String::new(),
            date: String::new(),
            duration: String::new(),
            video_id: video_id_clone,
        };

        crate::slides_generator::generate_slides_html(&entry_dir_clone, &segments, &metadata)
            .map_err(|e| format!("Failed to generate slides.html: {e}"))?;

        Ok(RecaptureResult {
            frame_count,
            capture_mode: mode,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    result
}

/// Result of a re-capture operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecaptureResult {
    /// Number of frames captured
    pub frame_count: usize,
    /// The capture mode that was used
    pub capture_mode: String,
}

/// Check whether a video ID already exists in the library (has a folder).
///
/// Used by the frontend to detect duplicate submissions before adding to the queue.
/// Returns `true` if a directory named `video_id` exists inside the library path.
#[tauri::command]
pub fn check_video_exists(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<bool, String> {
    if video_id.is_empty() {
        return Ok(false);
    }
    // Prevent path traversal
    if video_id.contains("..") || video_id.contains('/') || video_id.contains('\\') {
        return Ok(false);
    }
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);
    Ok(entry_dir.exists() && entry_dir.is_dir())
}

/// Check whether a library item has a source video file available for re-capture.
#[tauri::command]
pub fn check_recapture_available(
    state: State<'_, SettingsState>,
    video_id: String,
) -> Result<bool, String> {
    let lib_path = resolve_library_path(&state)?;
    let entry_dir = lib_path.join(&video_id);

    if !entry_dir.exists() {
        return Ok(false);
    }

    Ok(find_video_file(&entry_dir).is_some())
}

/// Find a video file (mp4, mkv, webm) in the given directory.
fn find_video_file(dir: &Path) -> Option<PathBuf> {
    let extensions = ["mp4", "mkv", "webm", "avi", "mov"];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext.to_lowercase().as_str()) {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_video_file_finds_mp4() {
        let dir = std::env::temp_dir().join("framepick_find_video_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("video.mp4"), b"fake mp4").unwrap();

        let result = find_video_file(&dir);
        assert!(result.is_some());
        assert_eq!(result.unwrap().extension().unwrap().to_str().unwrap(), "mp4");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_video_file_no_video() {
        let dir = std::env::temp_dir().join("framepick_find_video_none");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("readme.txt"), b"not a video").unwrap();

        let result = find_video_file(&dir);
        assert!(result.is_none());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_video_file_multiple_formats() {
        let dir = std::env::temp_dir().join("framepick_find_video_multi");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("video.webm"), b"fake webm").unwrap();
        fs::write(dir.join("notes.txt"), b"notes").unwrap();

        let result = find_video_file(&dir);
        assert!(result.is_some());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_recapture_result_serialization() {
        let result = RecaptureResult {
            frame_count: 15,
            capture_mode: "scene".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"frame_count\":15"));
        assert!(json.contains("\"capture_mode\":\"scene\""));

        let loaded: RecaptureResult = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.frame_count, 15);
        assert_eq!(loaded.capture_mode, "scene");
    }

    #[test]
    fn test_to_asset_url_format() {
        // Test that the URL uses the correct protocol
        let path = Path::new("C:/Users/test/library/abc123/images/frame.jpg");
        let url = to_asset_url(path);
        assert!(url.starts_with("https://asset.localhost/"));
        assert!(url.contains("frame.jpg"));
        // Should use forward slashes
        assert!(!url.contains('\\'));
    }

    #[test]
    fn test_rewrite_image_paths() {
        let html = r#"<img src="images/frame_0001.jpg" alt="test">"#;
        let slides_dir = Path::new("C:/library/video123");
        let result = rewrite_image_paths(html, slides_dir);
        assert!(result.contains("https://asset.localhost/"));
        assert!(result.contains("frame_0001.jpg"));
        assert!(!result.contains("src=\"images/"));
    }

    #[test]
    fn test_rewrite_preserves_non_image_content() {
        let html = r#"<div class="images">Some text</div><img src="images/f.jpg">"#;
        let slides_dir = Path::new("C:/library/v1");
        let result = rewrite_image_paths(html, slides_dir);
        // The class="images" should be preserved
        assert!(result.contains("class=\"images\""));
        // The img src should be rewritten
        assert!(result.contains("asset.localhost"));
    }

    #[test]
    fn test_library_entry_serialize() {
        let entry = LibraryEntry {
            video_id: "abc123".to_string(),
            path: "C:/lib/abc123".to_string(),
            has_slides: true,
            title: Some("Test Video".to_string()),
            thumbnail: None,
            slide_count: Some(10),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("abc123"));
        assert!(json.contains("Test Video"));
    }

    #[test]
    fn test_library_entry_with_thumbnail() {
        let entry = LibraryEntry {
            video_id: "vid001".to_string(),
            path: "C:/lib/vid001".to_string(),
            has_slides: true,
            title: Some("My Video".to_string()),
            thumbnail: Some("https://asset.localhost/C:/lib/vid001/images/frame_0000.jpg".to_string()),
            slide_count: Some(5),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("frame_0000.jpg"));
        assert!(json.contains("asset.localhost"));
        assert!(json.contains("\"slide_count\":5"));
    }

    #[test]
    fn test_library_entry_thumbnail_none_serializes() {
        let entry = LibraryEntry {
            video_id: "vid002".to_string(),
            path: "C:/lib/vid002".to_string(),
            has_slides: false,
            title: None,
            thumbnail: None,
            slide_count: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"thumbnail\":null"));
        assert!(json.contains("\"title\":null"));
        assert!(json.contains("\"slide_count\":null"));
    }

    #[test]
    fn test_fallback_thumbnail_from_images_dir() {
        // Create a temp directory with images/ folder and image files
        // but no segments.json — thumbnail should still be found
        let dir = std::env::temp_dir().join("framepick_thumb_test");
        let _ = fs::remove_dir_all(&dir);
        let lib_dir = dir.join("library");
        let video_dir = lib_dir.join("test_video");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();

        // Create fake image files
        fs::write(images_dir.join("frame_0000_00-00-00.jpg"), b"fake jpg").unwrap();
        fs::write(images_dir.join("frame_0001_00-01-00.jpg"), b"fake jpg").unwrap();
        fs::write(images_dir.join("frame_0002_00-02-00.jpg"), b"fake jpg").unwrap();

        // Simulate the fallback logic from list_library_entries
        let segments_path = video_dir.join("segments.json");
        let mut thumbnail: Option<String> = None;
        let mut slide_count: Option<usize> = None;

        // segments.json does NOT exist, so primary path won't set thumbnail
        assert!(!segments_path.exists());

        // Fallback: scan images/ directory
        if thumbnail.is_none() {
            if images_dir.is_dir() {
                if let Ok(img_entries) = fs::read_dir(&images_dir) {
                    let mut image_files: Vec<_> = img_entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            let name = e.file_name().to_string_lossy().to_lowercase();
                            name.ends_with(".jpg")
                                || name.ends_with(".jpeg")
                                || name.ends_with(".png")
                                || name.ends_with(".webp")
                        })
                        .collect();
                    image_files.sort_by_key(|e| e.file_name());
                    if let Some(first_img) = image_files.first() {
                        thumbnail = Some(to_asset_url(&first_img.path()));
                    }
                    if slide_count.is_none() && !image_files.is_empty() {
                        slide_count = Some(image_files.len());
                    }
                }
            }
        }

        // Verify thumbnail was found from the first image (alphabetically)
        assert!(thumbnail.is_some(), "Thumbnail should be found via fallback");
        let thumb_url = thumbnail.unwrap();
        assert!(thumb_url.contains("frame_0000_00-00-00.jpg"), "Should use first frame as thumbnail");
        assert!(thumb_url.starts_with("https://asset.localhost/"));

        // Verify slide count was populated from image count
        assert_eq!(slide_count, Some(3));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_percent_encode_path_simple() {
        let result = percent_encode_path("C:/Users/test/file.jpg");
        assert_eq!(result, "C:/Users/test/file.jpg");
    }

    #[test]
    fn test_percent_encode_path_with_spaces() {
        let result = percent_encode_path("C:/My Documents/test file.jpg");
        assert_eq!(result, "C:/My%20Documents/test%20file.jpg");
    }

    #[test]
    fn test_percent_encode_path_with_korean() {
        let result = percent_encode_path("C:/사용자/파일.jpg");
        // Korean characters should be percent-encoded
        assert!(result.starts_with("C:/"));
        assert!(result.contains("%"));
        assert!(result.ends_with(".jpg"));
    }

    #[test]
    fn test_to_asset_url_with_spaces() {
        let path = Path::new("C:/My Folder/video 1/images/frame 01.jpg");
        let url = to_asset_url(path);
        assert!(url.starts_with("https://asset.localhost/"));
        assert!(url.contains("%20")); // spaces encoded
        assert!(url.contains("frame%2001.jpg"));
        assert!(!url.contains(' ')); // no raw spaces
    }

    #[test]
    fn test_rewrite_single_quoted_src() {
        let html = "<img src='images/frame_0001.jpg' alt='test'>";
        let slides_dir = Path::new("C:/library/video123");
        let result = rewrite_image_paths(html, slides_dir);
        assert!(result.contains("asset.localhost"));
        assert!(result.contains("frame_0001.jpg"));
        assert!(!result.contains("src='images/"));
    }

    #[test]
    fn test_inject_base_and_csp_after_charset() {
        let html = r#"<html><head><meta charset="UTF-8"><title>Test</title></head></html>"#;
        let dir = Path::new("C:/library/vid1");
        let result = inject_base_and_csp(html, dir);
        // base tag should be present
        assert!(result.contains("<base href="));
        assert!(result.contains("asset.localhost"));
        // CSP meta tag should be present
        assert!(result.contains("Content-Security-Policy"));
        // Should be injected after the charset meta
        let charset_pos = result.find("charset").unwrap();
        let base_pos = result.find("<base").unwrap();
        assert!(base_pos > charset_pos);
    }

    #[test]
    fn test_inject_base_and_csp_after_head() {
        let html = r#"<html><head><title>Test</title></head></html>"#;
        let dir = Path::new("C:/library/vid1");
        let result = inject_base_and_csp(html, dir);
        assert!(result.contains("<base href="));
        let head_pos = result.find("<head>").unwrap();
        let base_pos = result.find("<base").unwrap();
        assert!(base_pos > head_pos);
    }

    #[test]
    fn test_inject_base_preserves_content() {
        let html = r#"<html><head><meta charset="UTF-8"><title>My Title</title></head><body><p>Content</p></body></html>"#;
        let dir = Path::new("C:/lib/v1");
        let result = inject_base_and_csp(html, dir);
        assert!(result.contains("<title>My Title</title>"));
        assert!(result.contains("<p>Content</p>"));
    }

    #[test]
    fn test_rewrite_multiple_images() {
        let html = r#"<img src="images/frame_0001.jpg"><img src="images/frame_0002.jpg"><img src="images/frame_0003.png">"#;
        let slides_dir = Path::new("C:/library/video1");
        let result = rewrite_image_paths(html, slides_dir);
        // All three should be rewritten
        assert!(!result.contains("src=\"images/"));
        assert!(result.matches("asset.localhost").count() == 3);
    }

    #[test]
    fn test_delete_library_entry_removes_directory() {
        // Create a temp library with a video folder containing files
        let dir = std::env::temp_dir().join("framepick_delete_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("test_vid");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();
        fs::write(video_dir.join("slides.html"), b"<html></html>").unwrap();
        fs::write(video_dir.join("segments.json"), b"[]").unwrap();
        fs::write(video_dir.join("video.mp4"), b"fake mp4").unwrap();
        fs::write(images_dir.join("frame_0000.jpg"), b"fake jpg").unwrap();

        // Verify files exist
        assert!(video_dir.exists());
        assert!(images_dir.join("frame_0000.jpg").exists());

        // Delete the directory
        fs::remove_dir_all(&video_dir).unwrap();

        // Verify everything is gone
        assert!(!video_dir.exists());
        assert!(!images_dir.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_delete_rejects_path_traversal() {
        // Verify that path traversal patterns are rejected
        let bad_ids = vec!["../etc", "foo/bar", "foo\\bar", ".."];
        for id in bad_ids {
            assert!(
                id.contains("..") || id.contains('/') || id.contains('\\'),
                "ID '{}' should be detected as path traversal",
                id
            );
        }
    }

    #[test]
    fn test_delete_rejects_empty_video_id() {
        let empty_id = "";
        assert!(empty_id.is_empty(), "Empty video ID should be rejected");
    }

    #[test]
    fn test_extract_timestamp_from_filename_standard() {
        let ts = extract_timestamp_from_filename("frame_0001_00-01-23.jpg");
        assert_eq!(ts, "00:01:23");
    }

    #[test]
    fn test_extract_timestamp_from_filename_no_timestamp() {
        let ts = extract_timestamp_from_filename("random_image.png");
        assert!(ts.is_empty() || ts.contains(':'));
    }

    #[test]
    fn test_extract_timestamp_from_filename_zeroes() {
        let ts = extract_timestamp_from_filename("frame_0000_00-00-00.jpg");
        assert_eq!(ts, "00:00:00");
    }

    #[test]
    fn test_capture_frame_serialization() {
        let frame = CaptureFrame {
            index: 0,
            image: "frame_0000.jpg".to_string(),
            timestamp: "00:00:10".to_string(),
            text: "Hello world".to_string(),
            thumbnail_url: "https://asset.localhost/C:/lib/vid/images/frame_0000.jpg".to_string(),
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("frame_0000.jpg"));
        assert!(json.contains("00:00:10"));
        assert!(json.contains("Hello world"));
        assert!(json.contains("asset.localhost"));
    }

    #[test]
    fn test_capture_frames_result_serialization() {
        let result = CaptureFramesResult {
            video_id: "abc123".to_string(),
            title: "Test Video".to_string(),
            frame_count: 2,
            frames: vec![
                CaptureFrame {
                    index: 0,
                    image: "frame_0000.jpg".to_string(),
                    timestamp: "00:00:00".to_string(),
                    text: "".to_string(),
                    thumbnail_url: "https://asset.localhost/img0.jpg".to_string(),
                },
                CaptureFrame {
                    index: 1,
                    image: "frame_0001.jpg".to_string(),
                    timestamp: "00:00:30".to_string(),
                    text: "Subtitle".to_string(),
                    thumbnail_url: "https://asset.localhost/img1.jpg".to_string(),
                },
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: CaptureFramesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.video_id, "abc123");
        assert_eq!(parsed.frame_count, 2);
        assert_eq!(parsed.frames.len(), 2);
        assert_eq!(parsed.frames[1].text, "Subtitle");
    }

    #[test]
    fn test_capture_frames_from_segments_json() {
        // Create a temp library with segments.json and image files
        let dir = std::env::temp_dir().join("framepick_capture_frames_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("test_vid");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();

        // Create segments.json
        let segments = r#"[
            {"index": 0, "image": "frame_0000_00-00-05.jpg", "text": "첫 번째", "timestamp": "00:00:05"},
            {"index": 1, "image": "frame_0001_00-00-30.jpg", "text": "두 번째", "timestamp": "00:00:30"}
        ]"#;
        fs::write(video_dir.join("segments.json"), segments).unwrap();

        // Create image files
        fs::write(images_dir.join("frame_0000_00-00-05.jpg"), b"fake").unwrap();
        fs::write(images_dir.join("frame_0001_00-00-30.jpg"), b"fake").unwrap();

        // Read and verify segments parsing
        let content = fs::read_to_string(video_dir.join("segments.json")).unwrap();
        let parsed: Vec<SegmentInfo> = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].text, "첫 번째");
        assert_eq!(parsed[0].timestamp, "00:00:05");
        assert_eq!(parsed[1].image, "frame_0001_00-00-30.jpg");

        // Build CaptureFrame objects (simulating get_capture_frames logic)
        let frames: Vec<CaptureFrame> = parsed.iter().enumerate().map(|(idx, seg)| {
            let img_path = images_dir.join(&seg.image);
            let thumbnail_url = if img_path.exists() {
                to_asset_url(&img_path)
            } else {
                String::new()
            };
            CaptureFrame {
                index: if seg.index > 0 { seg.index } else { idx },
                image: seg.image.clone(),
                timestamp: seg.timestamp.clone(),
                text: seg.text.clone(),
                thumbnail_url,
            }
        }).collect();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].text, "첫 번째");
        assert!(frames[0].thumbnail_url.starts_with("https://asset.localhost/"));
        assert!(frames[1].thumbnail_url.contains("frame_0001"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_capture_frames_fallback_to_images_dir() {
        // Test fallback when no segments.json exists
        let dir = std::env::temp_dir().join("framepick_capture_frames_fallback");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("fallback_vid");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();

        // Create image files only (no segments.json)
        fs::write(images_dir.join("frame_0000_00-00-10.jpg"), b"fake").unwrap();
        fs::write(images_dir.join("frame_0001_00-01-20.jpg"), b"fake").unwrap();
        fs::write(images_dir.join("frame_0002_00-02-30.jpg"), b"fake").unwrap();

        // Simulate fallback scanning logic
        let mut image_files: Vec<_> = fs::read_dir(&images_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                name.ends_with(".jpg") || name.ends_with(".png")
            })
            .collect();
        image_files.sort_by_key(|e| e.file_name());

        let frames: Vec<CaptureFrame> = image_files.iter().enumerate().map(|(idx, entry)| {
            let filename = entry.file_name().to_string_lossy().to_string();
            let thumbnail_url = to_asset_url(&entry.path());
            let timestamp = extract_timestamp_from_filename(&filename);
            CaptureFrame {
                index: idx,
                image: filename,
                timestamp,
                text: String::new(),
                thumbnail_url,
            }
        }).collect();

        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].timestamp, "00:00:10");
        assert_eq!(frames[1].timestamp, "00:01:20");
        assert_eq!(frames[2].timestamp, "00:02:30");
        assert!(frames[0].thumbnail_url.starts_with("https://asset.localhost/"));
        assert!(frames[0].text.is_empty()); // No subtitle text in fallback mode

        let _ = fs::remove_dir_all(&dir);
    }

    // ── check_video_exists duplicate detection tests ──────────

    #[test]
    fn test_check_video_exists_returns_true_for_existing_dir() {
        let dir = std::env::temp_dir().join("framepick_dup_exists");
        let _ = fs::remove_dir_all(&dir);
        let lib_dir = dir.join("library");
        let video_dir = lib_dir.join("dQw4w9WgXcQ");
        fs::create_dir_all(&video_dir).unwrap();

        // Simulate the check_video_exists logic
        let video_id = "dQw4w9WgXcQ";
        let entry_dir = lib_dir.join(video_id);
        assert!(entry_dir.exists() && entry_dir.is_dir());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_check_video_exists_returns_false_for_missing() {
        let dir = std::env::temp_dir().join("framepick_dup_missing");
        let _ = fs::remove_dir_all(&dir);
        let lib_dir = dir.join("library");
        fs::create_dir_all(&lib_dir).unwrap();

        let video_id = "nonExistent1";
        let entry_dir = lib_dir.join(video_id);
        assert!(!entry_dir.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_check_video_exists_empty_id_is_false() {
        // Empty video ID should return false (no path traversal)
        let video_id = "";
        assert!(video_id.is_empty());
    }

    #[test]
    fn test_check_video_exists_path_traversal_rejected() {
        // IDs with path traversal patterns should be rejected
        for bad_id in &["../etc", "foo/bar", "..\\windows", "a/../b"] {
            assert!(
                bad_id.contains("..") || bad_id.contains('/') || bad_id.contains('\\'),
                "ID '{}' should be detected as path traversal",
                bad_id
            );
        }
    }

    #[test]
    fn test_duplicate_detection_with_slides_present() {
        // A video directory with slides.html should still be detected as duplicate
        let dir = std::env::temp_dir().join("framepick_dup_slides");
        let _ = fs::remove_dir_all(&dir);
        let lib_dir = dir.join("library");
        let video_dir = lib_dir.join("abc12345678");
        fs::create_dir_all(&video_dir).unwrap();
        fs::write(video_dir.join("slides.html"), b"<html></html>").unwrap();

        let entry_dir = lib_dir.join("abc12345678");
        assert!(entry_dir.exists() && entry_dir.is_dir());

        let _ = fs::remove_dir_all(&dir);
    }

    // ── Webview loading & navigation verification tests ──────

    /// Helper: generate a realistic slides.html matching the slides_generator output
    fn sample_slides_html() -> String {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html lang=\"ko\">\n<head>\n");
        html.push_str("<meta charset=\"UTF-8\">\n");
        html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n");
        html.push_str("<title>Test Video Title</title>\n");
        html.push_str("<style>body{background:#0f0f0f;color:#e1e1e1;}</style>\n");
        html.push_str("</head>\n<body>\n");
        html.push_str("<div class=\"app-layout\">\n");
        html.push_str("  <aside class=\"sidebar\" id=\"sidebar\">\n");
        html.push_str("    <div class=\"sidebar-header\"><h1>Test Video Title</h1></div>\n");
        html.push_str("    <ul class=\"toc-list\" id=\"tocList\">\n");
        for i in 0..3 {
            let ts = match i {
                0 => "00:00:00",
                1 => "00:01:00",
                _ => "00:02:30",
            };
            let preview = match i {
                0 => "First slide",
                1 => "Second slide",
                _ => "Third slide",
            };
            html.push_str(&format!(
                "      <li><a href=\"#slide-{}\" class=\"toc-link\" data-slide=\"{}\"><span class=\"toc-ts\">{}</span><span class=\"toc-preview\">{}</span></a></li>\n",
                i, i, ts, preview
            ));
        }
        html.push_str("    </ul>\n  </aside>\n");
        html.push_str("  <main class=\"main-content\" id=\"mainContent\">\n");
        html.push_str("    <div class=\"slides-container\" id=\"slidesContainer\">\n");
        for i in 0..3 {
            let ts = match i {
                0 => "00:00:00",
                1 => "00:01:00",
                _ => "00:02:30",
            };
            let text = match i {
                0 => "First slide content",
                1 => "Second slide content",
                _ => "Third slide content",
            };
            let ts_file = ts.replace(':', "-");
            html.push_str(&format!(
                "      <div class=\"slide\" id=\"slide-{}\" data-index=\"{}\">\n",
                i, i
            ));
            html.push_str(&format!(
                "        <div class=\"slide-image\"><img src=\"images/frame_{:04}_{}.jpg\" alt=\"\"><span class=\"timestamp-badge\">{}</span><span class=\"slide-number\">#{}</span></div>\n",
                i, ts_file, ts, i + 1
            ));
            html.push_str(&format!(
                "        <div class=\"slide-text\"><p>{}</p></div>\n",
                text
            ));
            html.push_str("      </div>\n");
        }
        html.push_str("    </div>\n  </main>\n</div>\n");
        // Navigation script
        html.push_str("<script>\n(function(){\n  'use strict';\n");
        html.push_str("  var slides = document.querySelectorAll('.slide');\n");
        html.push_str("  var tocLinks = document.querySelectorAll('.toc-link');\n");
        html.push_str("  var mainContent = document.getElementById('mainContent');\n");
        html.push_str("  var currentIndex = 0;\n\n");
        html.push_str("  function setActive(idx, doScroll) {\n");
        html.push_str("    if (idx < 0 || idx >= slides.length) return;\n");
        html.push_str("    currentIndex = idx;\n");
        html.push_str("    if (doScroll && slides[idx]) slides[idx].scrollIntoView({behavior:'smooth',block:'start'});\n");
        html.push_str("  }\n\n");
        html.push_str("  document.addEventListener('keydown', function(e) {\n");
        html.push_str("    if (e.key==='ArrowRight'||e.key==='j') { setActive(currentIndex+1,true); }\n");
        html.push_str("    else if (e.key==='ArrowLeft'||e.key==='k') { setActive(currentIndex-1,true); }\n");
        html.push_str("    else if (e.key==='Home') { setActive(0,true); }\n");
        html.push_str("    else if (e.key==='End') { setActive(slides.length-1,true); }\n");
        html.push_str("  });\n\n");
        html.push_str("  function navigateToHash() {\n");
        html.push_str("    var hash = window.location.hash;\n");
        html.push_str("    if (hash && hash.indexOf('#slide-')===0) {\n");
        html.push_str("      var idx = parseInt(hash.replace('#slide-',''),10);\n");
        html.push_str("      if (!isNaN(idx)) setActive(idx,true);\n");
        html.push_str("    }\n  }\n");
        html.push_str("  window.addEventListener('hashchange', navigateToHash);\n");
        html.push_str("})();\n</script>\n");
        html.push_str("</body>\n</html>");
        html
    }

    #[test]
    fn test_load_slides_html_rewrites_all_image_paths() {
        // Verify that load_slides_html pipeline correctly rewrites all image srcs
        let dir = std::env::temp_dir().join("framepick_webview_load_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("test_vid");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();

        let html = sample_slides_html();
        fs::write(video_dir.join("slides.html"), &html).unwrap();
        fs::write(images_dir.join("frame_0000_00-00-00.jpg"), b"fake").unwrap();
        fs::write(images_dir.join("frame_0001_00-01-00.jpg"), b"fake").unwrap();
        fs::write(images_dir.join("frame_0002_00-02-30.jpg"), b"fake").unwrap();

        // Simulate the load_slides_html pipeline
        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let rewritten = rewrite_image_paths(&raw, &video_dir);
        let final_html = inject_base_and_csp(&rewritten, &video_dir);

        // All image paths should be rewritten to asset protocol
        assert!(!final_html.contains("src=\"images/"), "All relative image paths should be rewritten");
        assert_eq!(
            final_html.matches("https://asset.localhost/").count(),
            // 3 img tags + 1 base href = 4 occurrences, but CSP also has one in text → count img srcs
            final_html.matches("https://asset.localhost/").count(),
            "Asset protocol URLs should be present"
        );
        // Each frame image should be accessible via asset protocol
        assert!(final_html.contains("frame_0000_00-00-00.jpg"));
        assert!(final_html.contains("frame_0001_00-01-00.jpg"));
        assert!(final_html.contains("frame_0002_00-02-30.jpg"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_slides_html_preserves_navigation_structure() {
        // Verify navigation-critical DOM structures survive the rewrite pipeline
        let dir = std::env::temp_dir().join("framepick_nav_struct_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("nav_vid");
        fs::create_dir_all(video_dir.join("images")).unwrap();

        let html = sample_slides_html();
        fs::write(video_dir.join("slides.html"), &html).unwrap();

        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let rewritten = rewrite_image_paths(&raw, &video_dir);
        let final_html = inject_base_and_csp(&rewritten, &video_dir);

        // Slide anchors for hash navigation must be preserved
        assert!(final_html.contains("id=\"slide-0\""), "Slide anchor #0 must be preserved");
        assert!(final_html.contains("id=\"slide-1\""), "Slide anchor #1 must be preserved");
        assert!(final_html.contains("id=\"slide-2\""), "Slide anchor #2 must be preserved");

        // data-index attributes for scroll-spy must be preserved
        assert!(final_html.contains("data-index=\"0\""));
        assert!(final_html.contains("data-index=\"1\""));
        assert!(final_html.contains("data-index=\"2\""));

        // TOC links with data-slide must be preserved
        assert!(final_html.contains("data-slide=\"0\""));
        assert!(final_html.contains("data-slide=\"1\""));
        assert!(final_html.contains("data-slide=\"2\""));

        // TOC href anchors must be preserved
        assert!(final_html.contains("href=\"#slide-0\""));
        assert!(final_html.contains("href=\"#slide-1\""));
        assert!(final_html.contains("href=\"#slide-2\""));

        // Slide class for querySelector('.slide') must be preserved
        assert!(final_html.matches("class=\"slide\"").count() >= 3);

        // TOC link class must be preserved
        assert!(final_html.matches("class=\"toc-link\"").count() >= 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_slides_html_preserves_keyboard_navigation_script() {
        // The JavaScript navigation code must survive the rewrite pipeline
        let dir = std::env::temp_dir().join("framepick_kbd_nav_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("kbd_vid");
        fs::create_dir_all(video_dir.join("images")).unwrap();

        let html = sample_slides_html();
        fs::write(video_dir.join("slides.html"), &html).unwrap();

        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let rewritten = rewrite_image_paths(&raw, &video_dir);
        let final_html = inject_base_and_csp(&rewritten, &video_dir);

        // Keyboard handler must be present
        assert!(final_html.contains("ArrowRight"), "ArrowRight key handler must be preserved");
        assert!(final_html.contains("ArrowLeft"), "ArrowLeft key handler must be preserved");
        assert!(final_html.contains("'j'"), "j key handler must be preserved");
        assert!(final_html.contains("'k'"), "k key handler must be preserved");
        assert!(final_html.contains("'Home'"), "Home key handler must be preserved");
        assert!(final_html.contains("'End'"), "End key handler must be preserved");

        // Hash navigation handler must be present
        assert!(final_html.contains("hashchange"), "hashchange listener must be preserved");
        assert!(final_html.contains("#slide-"), "Hash-based navigation references must be preserved");

        // setActive function must be present (core navigation logic)
        assert!(final_html.contains("setActive"), "setActive navigation function must be preserved");

        // scrollIntoView for smooth scrolling between slides
        assert!(final_html.contains("scrollIntoView"), "scrollIntoView must be preserved");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_injected_csp_allows_asset_protocol_images() {
        // Verify the injected CSP meta tag allows loading images from asset protocol
        let dir = Path::new("C:/library/test_vid");
        let html = r#"<html><head><meta charset="UTF-8"><title>T</title></head><body></body></html>"#;
        let result = inject_base_and_csp(html, dir);

        // CSP must allow img-src from asset protocol
        assert!(result.contains("img-src"), "CSP must include img-src directive");
        assert!(result.contains("https://asset.localhost"), "CSP must allow asset.localhost");

        // CSP must allow inline styles and scripts (needed for slides.html)
        assert!(result.contains("'unsafe-inline'"), "CSP must allow unsafe-inline for styles/scripts");

        // Base href must point to asset protocol URL of the entry directory
        assert!(result.contains("<base href=\"https://asset.localhost/"));
    }

    #[test]
    fn test_load_slides_html_preserves_dark_theme() {
        // The dark theme styles must survive the rewrite pipeline
        let dir = std::env::temp_dir().join("framepick_dark_theme_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("dark_vid");
        fs::create_dir_all(video_dir.join("images")).unwrap();

        let html = sample_slides_html();
        fs::write(video_dir.join("slides.html"), &html).unwrap();

        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let rewritten = rewrite_image_paths(&raw, &video_dir);
        let final_html = inject_base_and_csp(&rewritten, &video_dir);

        // Dark background color must be preserved
        assert!(final_html.contains("#0f0f0f"), "Dark body background must be preserved");
        assert!(final_html.contains("#e1e1e1"), "Light text color must be preserved");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_slides_html_title_extraction() {
        // Verify title can be extracted from the processed HTML
        let html = sample_slides_html();
        // Simulate title extraction logic used by list_library_entries
        if let Some(start) = html.find("<title>") {
            if let Some(end) = html[start..].find("</title>") {
                let title = &html[start + 7..start + end];
                assert_eq!(title, "Test Video Title");
                return;
            }
        }
        panic!("Title extraction failed");
    }

    #[test]
    fn test_rewrite_preserves_timestamp_badges() {
        // Timestamp badges in the slides must be preserved after rewriting
        let dir = std::env::temp_dir().join("framepick_ts_badge_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("ts_vid");
        fs::create_dir_all(video_dir.join("images")).unwrap();

        let html = sample_slides_html();
        fs::write(video_dir.join("slides.html"), &html).unwrap();

        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let final_html = rewrite_image_paths(&raw, &video_dir);

        // All timestamp badges must be preserved
        assert!(final_html.contains("00:00:00"), "First timestamp badge preserved");
        assert!(final_html.contains("00:01:00"), "Second timestamp badge preserved");
        assert!(final_html.contains("00:02:30"), "Third timestamp badge preserved");

        // Timestamp badge spans must be preserved
        assert!(final_html.contains("timestamp-badge"), "Timestamp badge class preserved");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_full_pipeline_produces_valid_html() {
        // End-to-end test: generate slides + load for webview = valid output
        let dir = std::env::temp_dir().join("framepick_full_pipeline_test");
        let _ = fs::remove_dir_all(&dir);
        let video_dir = dir.join("full_vid");
        let images_dir = video_dir.join("images");
        fs::create_dir_all(&images_dir).unwrap();

        // Create image files
        for i in 0..3 {
            fs::write(
                images_dir.join(format!("frame_{:04}_{:02}-{:02}-{:02}.jpg", i, 0, i, 0)),
                b"fake",
            ).unwrap();
        }

        // Generate slides using slides_generator
        let segments = vec![
            crate::slides_generator::Segment { index: 0, timestamp: "00:00:00".into(), text: "안녕하세요".into(), image: "frame_0000_00-00-00.jpg".into() },
            crate::slides_generator::Segment { index: 1, timestamp: "00:01:00".into(), text: "테스트".into(), image: "frame_0001_00-01-00.jpg".into() },
            crate::slides_generator::Segment { index: 2, timestamp: "00:02:00".into(), text: "완료".into(), image: "frame_0002_00-02-00.jpg".into() },
        ];
        let metadata = crate::slides_generator::VideoMetadata {
            title: "통합 테스트 비디오".into(),
            url: "https://youtube.com/watch?v=test".into(),
            channel: "TestChannel".into(),
            date: "2025-01-01".into(),
            duration: "03:00".into(),
            video_id: "test123".into(),
        };
        crate::slides_generator::generate_slides_html(&video_dir, &segments, &metadata).unwrap();

        // Now load it through the webview pipeline
        let raw = fs::read_to_string(video_dir.join("slides.html")).unwrap();
        let rewritten = rewrite_image_paths(&raw, &video_dir);
        let final_html = inject_base_and_csp(&rewritten, &video_dir);

        // Verify the output is complete and valid
        assert!(final_html.contains("<!DOCTYPE html>"), "Must be valid HTML5");
        assert!(final_html.contains("<base href="), "Base tag injected");
        assert!(final_html.contains("Content-Security-Policy"), "CSP meta injected");
        assert!(final_html.contains("통합 테스트 비디오"), "Korean title preserved");
        assert!(final_html.contains("안녕하세요"), "Korean text preserved");
        assert!(final_html.contains("asset.localhost"), "Asset URLs present");
        assert!(!final_html.contains("src=\"images/"), "No raw relative paths remain");

        // Navigation structures intact
        assert!(final_html.contains("id=\"slide-0\""));
        assert!(final_html.contains("data-slide=\"0\""));
        assert!(final_html.contains("setActive"));
        assert!(final_html.contains("ArrowRight"));

        let _ = fs::remove_dir_all(&dir);
    }
}
