//! Input state management for the download/capture pipeline.
//!
//! Stores the current URL, capture mode, and interval settings from the
//! frontend, plus a queue of items submitted for processing.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

/// Current input state from the frontend (URL + capture options).
/// Synced on every change from the UI so the backend always has the latest values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputState {
    /// YouTube URL entered by the user
    pub url: String,
    /// Capture mode: "subtitle" | "scene" | "interval"
    pub capture_mode: String,
    /// Interval in seconds (only relevant when capture_mode == "interval")
    pub interval_seconds: u32,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            url: String::new(),
            capture_mode: "subtitle".to_string(),
            interval_seconds: 10,
        }
    }
}

/// A queue item submitted for download and frame capture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    /// Unique ID (assigned by the frontend)
    pub id: u32,
    /// YouTube URL
    pub url: String,
    /// Capture mode snapshot at time of submission
    pub capture_mode: String,
    /// Interval seconds snapshot at time of submission
    pub interval_seconds: u32,
    /// Processing status: "pending" | "processing" | "completed" | "failed" | "skipped"
    #[serde(default = "default_status")]
    pub status: String,
    /// Video title (populated after metadata fetch)
    #[serde(default)]
    pub title: Option<String>,
    /// Error message if status == "failed"
    #[serde(default)]
    pub error: Option<String>,
    /// Progress percentage (0-100) during processing
    #[serde(default)]
    pub progress: Option<u32>,
    /// Current pipeline stage identifier (e.g. "downloading", "extracting_frames")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_stage: Option<String>,
    /// 1-based index of the current pipeline stage
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_stage_number: Option<u32>,
    /// Total number of pipeline stages for this item
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_total_stages: Option<u32>,
    /// Optional detail message for the current stage (e.g. "frame 12/48")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline_detail: Option<String>,
}

fn default_status() -> String {
    "pending".to_string()
}

/// Thread-safe Tauri state for the input and queue.
pub struct PipelineState {
    pub input: Mutex<InputState>,
    pub queue: Mutex<Vec<QueueItem>>,
}

impl PipelineState {
    pub fn new() -> Self {
        Self {
            input: Mutex::new(InputState::default()),
            queue: Mutex::new(Vec::new()),
        }
    }
}

// ─── Tauri Commands ──────────────────────────────────────────────

/// Store the latest input field values from the frontend.
/// Called on every keystroke / mode change to keep backend in sync.
#[tauri::command]
pub fn set_input_state(
    state: InputState,
    pipeline: State<'_, PipelineState>,
) -> Result<(), String> {
    let mut current = pipeline.input.lock().map_err(|e| e.to_string())?;
    *current = state;
    Ok(())
}

/// Get the current input state (for restoring UI after navigation).
#[tauri::command]
pub fn get_input_state(
    pipeline: State<'_, PipelineState>,
) -> Result<InputState, String> {
    let current = pipeline.input.lock().map_err(|e| e.to_string())?;
    Ok(current.clone())
}

/// Add an item to the processing queue.
///
/// Duplicate detection checks both URL string equality and video ID
/// extraction. Two different URL formats pointing to the same video
/// (e.g. `youtube.com/watch?v=X` and `youtu.be/X`) are treated as
/// duplicates if their extracted video IDs match.
#[tauri::command]
pub fn add_queue_item(
    item: QueueItem,
    pipeline: State<'_, PipelineState>,
) -> Result<QueueItem, String> {
    let mut queue = pipeline.queue.lock().map_err(|e| e.to_string())?;

    // Extract video ID from the new item's URL for cross-format comparison
    let new_video_id = crate::url_validator::extract_video_id(&item.url);

    // Check for duplicate in active (non-terminal) items by URL or video ID
    let duplicate = queue.iter().any(|q| {
        if q.status != "pending" && q.status != "processing" {
            return false;
        }
        // Exact URL match
        if q.url == item.url {
            return true;
        }
        // Video ID match (handles different URL formats for same video)
        if let Some(ref new_id) = new_video_id {
            if let Some(existing_id) = crate::url_validator::extract_video_id(&q.url) {
                return existing_id == *new_id;
            }
        }
        false
    });
    if duplicate {
        return Err("URL is already in the queue".to_string());
    }

    let stored = item.clone();
    queue.push(item);
    Ok(stored)
}

/// Get all queue items.
#[tauri::command]
pub fn get_queue(
    pipeline: State<'_, PipelineState>,
) -> Result<Vec<QueueItem>, String> {
    let queue = pipeline.queue.lock().map_err(|e| e.to_string())?;
    Ok(queue.clone())
}

/// Update a queue item by ID (partial merge for status, title, error, progress).
#[tauri::command]
pub fn update_queue_item(
    id: u32,
    status: Option<String>,
    title: Option<String>,
    error: Option<String>,
    progress: Option<u32>,
    pipeline: State<'_, PipelineState>,
) -> Result<(), String> {
    let mut queue = pipeline.queue.lock().map_err(|e| e.to_string())?;
    let item = queue
        .iter_mut()
        .find(|q| q.id == id)
        .ok_or_else(|| format!("Queue item {} not found", id))?;

    if let Some(s) = status {
        item.status = s;
    }
    if let Some(t) = title {
        item.title = Some(t);
    }
    if let Some(e) = error {
        item.error = Some(e);
    }
    if let Some(p) = progress {
        item.progress = Some(p);
    }

    Ok(())
}

/// Remove a queue item by ID.
#[tauri::command]
pub fn remove_queue_item(
    id: u32,
    pipeline: State<'_, PipelineState>,
) -> Result<(), String> {
    let mut queue = pipeline.queue.lock().map_err(|e| e.to_string())?;
    let len_before = queue.len();
    queue.retain(|q| q.id != id);
    if queue.len() == len_before {
        return Err(format!("Queue item {} not found", id));
    }
    Ok(())
}

/// Retry a failed queue item by resetting its status to "pending".
#[tauri::command]
pub fn retry_queue_item(
    id: u32,
    pipeline: State<'_, PipelineState>,
) -> Result<(), String> {
    let mut queue = pipeline.queue.lock().map_err(|e| e.to_string())?;
    let item = queue
        .iter_mut()
        .find(|q| q.id == id)
        .ok_or_else(|| format!("Queue item {} not found", id))?;

    if item.status != "failed" {
        return Err(format!(
            "Queue item {} cannot be retried (status: {})",
            id, item.status
        ));
    }

    item.status = "pending".to_string();
    item.error = None;
    item.progress = None;
    Ok(())
}

/// Set the language in settings and persist.
/// Convenience command so the frontend can update language without
/// sending the full settings object.
#[tauri::command]
pub fn set_language(
    language: String,
    settings_state: State<'_, crate::settings::SettingsState>,
) -> Result<(), String> {
    use crate::settings::Language;

    let mut settings = settings_state.0.lock().map_err(|e| e.to_string())?;
    match language.as_str() {
        "ko" => settings.language = Language::Ko,
        "en" => settings.language = Language::En,
        other => return Err(format!("Unsupported language: {other}")),
    }
    crate::settings::save_settings(&settings)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_state_defaults() {
        let s = InputState::default();
        assert_eq!(s.url, "");
        assert_eq!(s.capture_mode, "subtitle");
        assert_eq!(s.interval_seconds, 10);
    }

    #[test]
    fn input_state_serialization() {
        let s = InputState {
            url: "https://youtube.com/watch?v=abc".to_string(),
            capture_mode: "scene".to_string(),
            interval_seconds: 30,
        };
        let json = serde_json::to_string(&s).unwrap();
        let loaded: InputState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.url, s.url);
        assert_eq!(loaded.capture_mode, s.capture_mode);
        assert_eq!(loaded.interval_seconds, s.interval_seconds);
    }

    #[test]
    fn queue_item_serialization() {
        let item = QueueItem {
            id: 1,
            url: "https://youtube.com/watch?v=test123".to_string(),
            capture_mode: "subtitle".to_string(),
            interval_seconds: 10,
            status: "pending".to_string(),
            title: None,
            error: None,
            progress: None,
            pipeline_stage: None,
            pipeline_stage_number: None,
            pipeline_total_stages: None,
            pipeline_detail: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("test123"));
        assert!(json.contains("subtitle"));
        assert!(json.contains("pending"));

        let loaded: QueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, 1);
    }

    #[test]
    fn queue_item_status_default() {
        let json = r#"{"id":1,"url":"https://youtube.com/watch?v=x","capture_mode":"subtitle","interval_seconds":10}"#;
        let item: QueueItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.status, "pending");
        assert!(item.title.is_none());
        assert!(item.error.is_none());
    }

    #[test]
    fn retry_resets_failed_item() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            q.push(QueueItem {
                id: 1,
                url: "https://youtube.com/watch?v=abc".to_string(),
                capture_mode: "subtitle".to_string(),
                interval_seconds: 10,
                status: "failed".to_string(),
                title: Some("Test".to_string()),
                error: Some("Download error".to_string()),
                progress: Some(25),
                pipeline_stage: None,
                pipeline_stage_number: None,
                pipeline_total_stages: None,
                pipeline_detail: None,
            });
        }
        {
            let mut q = state.queue.lock().unwrap();
            let item = q.iter_mut().find(|i| i.id == 1).unwrap();
            assert_eq!(item.status, "failed");
            // Simulate retry logic
            item.status = "pending".to_string();
            item.error = None;
            item.progress = None;
            assert_eq!(item.status, "pending");
            assert!(item.error.is_none());
            assert!(item.progress.is_none());
            // Title should be preserved
            assert_eq!(item.title.as_deref(), Some("Test"));
        }
    }

    #[test]
    fn pipeline_state_thread_safety() {
        let state = PipelineState::new();

        // Set input
        {
            let mut input = state.input.lock().unwrap();
            input.url = "https://youtube.com/watch?v=abc".to_string();
            input.capture_mode = "interval".to_string();
            input.interval_seconds = 60;
        }
        {
            let input = state.input.lock().unwrap();
            assert_eq!(input.capture_mode, "interval");
            assert_eq!(input.interval_seconds, 60);
        }

        // Queue operations
        {
            let mut q = state.queue.lock().unwrap();
            q.push(QueueItem {
                id: 1,
                url: "https://youtube.com/watch?v=a".to_string(),
                capture_mode: "subtitle".to_string(),
                interval_seconds: 10,
                status: "pending".to_string(),
                title: None,
                error: None,
                progress: None,
                pipeline_stage: None,
                pipeline_stage_number: None,
                pipeline_total_stages: None,
                pipeline_detail: None,
            });
            q.push(QueueItem {
                id: 2,
                url: "https://youtube.com/watch?v=b".to_string(),
                capture_mode: "scene".to_string(),
                interval_seconds: 10,
                status: "pending".to_string(),
                title: None,
                error: None,
                progress: None,
                pipeline_stage: None,
                pipeline_stage_number: None,
                pipeline_total_stages: None,
                pipeline_detail: None,
            });
            assert_eq!(q.len(), 2);
        }
        {
            let mut q = state.queue.lock().unwrap();
            if let Some(item) = q.iter_mut().find(|i| i.id == 1) {
                item.status = "processing".to_string();
                item.title = Some("Test Video".to_string());
            }
            assert_eq!(q[0].status, "processing");
            assert_eq!(q[0].title.as_deref(), Some("Test Video"));
        }
    }

    // ── Duplicate detection tests ──────────────────────────────

    /// Helper: create a QueueItem for testing
    fn make_queue_item(id: u32, url: &str, status: &str) -> QueueItem {
        QueueItem {
            id,
            url: url.to_string(),
            capture_mode: "subtitle".to_string(),
            interval_seconds: 10,
            status: status.to_string(),
            title: None,
            error: None,
            progress: None,
            pipeline_stage: None,
            pipeline_stage_number: None,
            pipeline_total_stages: None,
            pipeline_detail: None,
        }
    }

    #[test]
    fn duplicate_detection_same_url_pending() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            q.push(make_queue_item(1, "https://youtube.com/watch?v=dQw4w9WgXcQ", "pending"));
        }
        // Adding same URL should be detected as duplicate
        let q = state.queue.lock().unwrap();
        let new_url = "https://youtube.com/watch?v=dQw4w9WgXcQ";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url {
                return true;
            }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(is_dup, "Same URL should be detected as duplicate");
    }

    #[test]
    fn duplicate_detection_different_url_same_video_id() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            // youtube.com/watch?v=X format
            q.push(make_queue_item(1, "https://www.youtube.com/watch?v=dQw4w9WgXcQ", "pending"));
        }
        // Adding youtu.be/X format for same video should be detected as duplicate
        let q = state.queue.lock().unwrap();
        let new_url = "https://youtu.be/dQw4w9WgXcQ";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        assert_eq!(new_video_id, Some("dQw4w9WgXcQ".to_string()));

        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url {
                return true;
            }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(is_dup, "Different URL formats with same video ID should be detected as duplicate");
    }

    #[test]
    fn duplicate_detection_shorts_url_same_video_id() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            q.push(make_queue_item(1, "https://youtu.be/dQw4w9WgXcQ", "processing"));
        }
        // shorts URL format for same video should be detected
        let q = state.queue.lock().unwrap();
        let new_url = "https://www.youtube.com/shorts/dQw4w9WgXcQ";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url {
                return true;
            }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(is_dup, "Shorts URL with same video ID should be detected as duplicate");
    }

    #[test]
    fn duplicate_detection_completed_item_not_duplicate() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            // Completed item should NOT block re-adding
            q.push(make_queue_item(1, "https://youtube.com/watch?v=dQw4w9WgXcQ", "completed"));
        }
        let q = state.queue.lock().unwrap();
        let new_url = "https://youtube.com/watch?v=dQw4w9WgXcQ";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url { return true; }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(!is_dup, "Completed item should not trigger duplicate detection");
    }

    #[test]
    fn duplicate_detection_failed_item_not_duplicate() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            q.push(make_queue_item(1, "https://youtube.com/watch?v=dQw4w9WgXcQ", "failed"));
        }
        let q = state.queue.lock().unwrap();
        let new_url = "https://youtu.be/dQw4w9WgXcQ";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url { return true; }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(!is_dup, "Failed item should not trigger duplicate detection");
    }

    #[test]
    fn duplicate_detection_different_videos_not_duplicate() {
        let state = PipelineState::new();
        {
            let mut q = state.queue.lock().unwrap();
            q.push(make_queue_item(1, "https://youtube.com/watch?v=dQw4w9WgXcQ", "pending"));
        }
        let q = state.queue.lock().unwrap();
        let new_url = "https://youtube.com/watch?v=jNQXAC9IVRw";
        let new_video_id = crate::url_validator::extract_video_id(new_url);
        let is_dup = q.iter().any(|existing| {
            if existing.status != "pending" && existing.status != "processing" {
                return false;
            }
            if existing.url == new_url { return true; }
            if let Some(ref new_id) = new_video_id {
                if let Some(existing_id) = crate::url_validator::extract_video_id(&existing.url) {
                    return existing_id == *new_id;
                }
            }
            false
        });
        assert!(!is_dup, "Different videos should not be detected as duplicates");
    }
}
