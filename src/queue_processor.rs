//! Queue processor — sequential processing of download/capture queue items.
//!
//! Runs in a background async task, processes one item at a time,
//! and emits Tauri events for each status change so the frontend
//! can update in real-time.
//!
//! Uses `crate::progress::ProgressTracker` for stage-based progress
//! reporting within each item's pipeline.

use crate::capture;
use crate::cleanup;
use crate::config::ConfigState;
use crate::downloader;
use crate::input_state::{PipelineState, QueueItem};
use crate::metadata;
use crate::progress::ProgressTracker;
use crate::settings::SettingsState;
use crate::slides_generator::{self, Segment};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Global flag indicating whether the processor loop is currently running.
/// Prevents multiple concurrent processing loops.
static PROCESSING_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Event payload sent to the frontend when a queue item's status changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatusEvent {
    /// Queue item ID
    pub id: u32,
    /// New status: "pending" | "processing" | "completed" | "failed"
    pub status: String,
    /// Optional progress percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u32>,
    /// Video title (populated when available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Error message (only when status == "failed")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Event payload for overall queue state changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueProgressEvent {
    /// Total items in queue
    pub total: usize,
    /// Number of completed items
    pub completed: usize,
    /// Number of failed items
    pub failed: usize,
    /// Whether processing is currently active
    pub is_processing: bool,
}

/// Check if the processor is currently running.
pub fn is_processing() -> bool {
    PROCESSING_ACTIVE.load(Ordering::SeqCst)
}

/// Start processing the queue sequentially in a background task.
///
/// Returns immediately. If already processing, returns Ok without
/// starting a duplicate loop.
#[tauri::command]
pub async fn start_queue_processing(app: AppHandle) -> Result<(), String> {
    // Prevent multiple concurrent processing loops
    if PROCESSING_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        // Already processing — not an error, just a no-op
        return Ok(());
    }

    // Spawn a background task for sequential processing
    tauri::async_runtime::spawn(async move {
        process_queue_loop(&app).await;
        PROCESSING_ACTIVE.store(false, Ordering::SeqCst);

        // Emit final state
        let _ = emit_queue_progress(&app);
    });

    Ok(())
}

/// Get the pipeline progress for a specific queue item.
/// Returns the current stage, stage number, total stages, percent, and detail.
#[tauri::command]
pub fn get_item_progress(
    id: u32,
    app: AppHandle,
) -> Result<Option<ItemProgressInfo>, String> {
    let pipeline = app.state::<PipelineState>();
    let queue = pipeline.queue.lock().map_err(|e| e.to_string())?;
    let item = queue.iter().find(|q| q.id == id);
    Ok(item.and_then(|q| {
        q.pipeline_stage.as_ref().map(|stage| ItemProgressInfo {
            queue_id: q.id,
            stage: stage.clone(),
            stage_number: q.pipeline_stage_number.unwrap_or(0),
            total_stages: q.pipeline_total_stages.unwrap_or(0),
            percent: q.progress.unwrap_or(0),
            detail: q.pipeline_detail.clone(),
            status: q.status.clone(),
        })
    }))
}

/// Detailed progress information for a specific queue item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProgressInfo {
    pub queue_id: u32,
    pub stage: String,
    pub stage_number: u32,
    pub total_stages: u32,
    pub percent: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub status: String,
}

/// Get the current processing status.
#[tauri::command]
pub fn get_processing_status(app: AppHandle) -> Result<QueueProgressEvent, String> {
    let pipeline = app.state::<PipelineState>();
    let queue = pipeline.queue.lock().map_err(|e| e.to_string())?;

    let total = queue.len();
    let completed = queue.iter().filter(|q| q.status == "completed").count();
    let failed = queue.iter().filter(|q| q.status == "failed").count();

    Ok(QueueProgressEvent {
        total,
        completed,
        failed,
        is_processing: is_processing(),
    })
}

/// Main processing loop — processes items one at a time.
async fn process_queue_loop(app: &AppHandle) {
    loop {
        // Find the next pending item
        let next_item = {
            let pipeline = app.state::<PipelineState>();
            let queue = pipeline.queue.lock().ok();
            queue.and_then(|q| q.iter().find(|item| item.status == "pending").cloned())
        };

        let item = match next_item {
            Some(item) => item,
            None => break, // No more pending items
        };

        // Update status to "processing"
        update_item_status(app, item.id, "processing", None, None, Some(0));
        emit_status_event(app, &QueueStatusEvent {
            id: item.id,
            status: "processing".to_string(),
            progress: Some(0),
            title: item.title.clone(),
            error: None,
        });
        let _ = emit_queue_progress(app);

        // Process the item
        let result = process_single_item(app, &item).await;

        match result {
            Ok(title) => {
                update_item_status(app, item.id, "completed", title.clone(), None, Some(100));
                emit_status_event(app, &QueueStatusEvent {
                    id: item.id,
                    status: "completed".to_string(),
                    progress: Some(100),
                    title,
                    error: None,
                });
            }
            Err(error_msg) => {
                // Don't overwrite "skipped" status (set by duplicate detection)
                let current_status = {
                    let pipeline = app.state::<PipelineState>();
                    pipeline.queue.lock().ok()
                        .and_then(|q| q.iter().find(|i| i.id == item.id).map(|i| i.status.clone()))
                        .unwrap_or_default()
                };
                if current_status != "skipped" {
                    // Emit pipeline:error with the stage context for richer frontend toast
                    let _ = app.emit("pipeline:error", crate::progress::ErrorPayload {
                        queue_id: item.id,
                        stage: crate::progress::PipelineStage::Done, // generic; stage detail is in message
                        message: error_msg.clone(),
                    });

                    update_item_status(
                        app,
                        item.id,
                        "failed",
                        item.title.clone(),
                        Some(error_msg.clone()),
                        None,
                    );
                    emit_status_event(app, &QueueStatusEvent {
                        id: item.id,
                        status: "failed".to_string(),
                        progress: None,
                        title: item.title.clone(),
                        error: Some(error_msg),
                    });
                }
            }
        }

        let _ = emit_queue_progress(app);
    }
}

/// Process a single queue item through the download → capture → slides pipeline.
///
/// **Duplicate detection**: Before processing, checks if the video ID already
/// exists in the library folder. If so, skips processing and marks as "skipped".
///
/// **Subtitle fallback**: When the capture mode is "subtitle", the pipeline
/// first checks subtitle availability via `capture_fallback::resolve_capture_mode`.
/// If no subtitles are found (or the check fails), it automatically falls back
/// to "scene" mode and emits a `capture:fallback` event to notify the frontend.
async fn process_single_item(
    app: &AppHandle,
    item: &QueueItem,
) -> Result<Option<String>, String> {
    // Stage 1: Validate URL
    let video_id = crate::url_validator::extract_video_id(&item.url)
        .ok_or_else(|| "Invalid YouTube URL".to_string())?;

    // Reconstruct a canonical, sanitized URL from the validated video ID
    let safe_url = format!("https://www.youtube.com/watch?v={}", video_id);

    // Stage 1b: Check if video already exists in library (duplicate detection)
    {
        let library_path = {
            let settings_state = app.state::<SettingsState>();
            settings_state
                .0
                .lock()
                .map(|s| s.library_path.clone())
                .unwrap_or_else(|_| "./library/".to_string())
        };
        let video_dir = ConfigState::resolved_library_path(&library_path).join(&video_id);
        if video_dir.exists() && video_dir.is_dir() {
            // Emit a duplicate-skipped event so the frontend can show a notification
            let _ = app.emit("queue:duplicate-skipped", serde_json::json!({
                "id": item.id,
                "video_id": video_id,
                "title": item.title,
            }));
            println!(
                "[queue_processor] Item {}: Skipped — video '{}' already exists in library",
                item.id, video_id
            );
            // Mark as "skipped" instead of "failed" so the user can distinguish
            // a true processing error from a benign duplicate detection
            update_item_status(app, item.id, "skipped", None, None, None);
            return Err(format!("Video '{}' already exists in library", video_id));
        }
    }

    // Capture mode will be resolved after download (when we know if subtitles exist)
    let requested_mode = item.capture_mode.clone();
    let mut tracker = ProgressTracker::new(item.id, &requested_mode);

    // ─── Stage: Fetch metadata ────────────────────────────────────
    tracker.emit(app, 0, Some("Fetching video metadata...".to_string()));

    let url_for_meta = safe_url.clone();
    let meta = tauri::async_runtime::spawn_blocking(move || metadata::fetch_metadata(&url_for_meta))
        .await
        .map_err(|e| format!("Metadata task panicked: {e}"))?
        .map_err(|e| format!("Metadata fetch failed: {e}"))?;

    let title = meta.title.clone();

    // Update the queue item with the real title
    update_item_status(app, item.id, "processing", Some(title.clone()), None, Some(10));
    tracker.emit(app, 100, Some(format!("Metadata: {}", title)));
    tracker.complete_stage(app);

    // ─── Resolve settings for download ───────────────────────────
    let (library_path, download_quality, interval_seconds, scene_threshold) = {
        let settings_state = app.state::<SettingsState>();
        let s = settings_state
            .0
            .lock()
            .map_err(|e| format!("Settings lock error: {e}"))?;
        (
            s.library_path.clone(),
            s.download_quality.clone(),
            s.default_interval_seconds,
            s.scene_change_threshold,
        )
    };

    let video_dir = ConfigState::resolved_library_path(&library_path).join(&video_id);
    std::fs::create_dir_all(&video_dir)
        .map_err(|e| format!("Failed to create video directory: {e}"))?;

    // ─── Stage: Download video ────────────────────────────────────
    tracker.emit(app, 0, Some("Downloading video...".to_string()));

    let url_for_dl = safe_url.clone();
    let video_dir_for_dl = video_dir.clone();
    let video_id_for_dl = video_id.clone();
    let quality_for_dl = download_quality.clone();

    let download_result = tauri::async_runtime::spawn_blocking(move || {
        downloader::download_video(&url_for_dl, &video_dir_for_dl, &video_id_for_dl, &quality_for_dl)
    })
    .await
    .map_err(|e| format!("Download task panicked: {e}"))?
    .map_err(|e| format!("Download failed: {e}"))?;

    tracker.emit(app, 100, Some("Download complete".to_string()));
    tracker.complete_stage(app);

    // ─── Resolve effective capture mode based on downloaded subtitle file ──
    let effective_mode = if requested_mode == "subtitle" {
        if download_result.subtitle_path.is_some() {
            println!("[queue_processor] Item {}: Subtitle file found, using subtitle mode", item.id);
            "subtitle".to_string()
        } else {
            println!("[queue_processor] Item {}: No subtitle file downloaded, falling back to scene mode", item.id);
            let _ = app.emit("capture:fallback", serde_json::json!({
                "queue_id": item.id,
                "requested_mode": "subtitle",
                "effective_mode": "scene",
                "reason": "No subtitles available — using scene change detection",
            }));
            update_item_capture_mode(app, item.id, "scene");
            "scene".to_string()
        }
    } else {
        requested_mode.clone()
    };

    // ─── Stage: Capture frames ────────────────────────────────────
    tracker.emit(app, 0, Some(format!("Capturing frames (mode: {effective_mode})...")));

    let mp4_path = download_result.mp4_path.clone();
    let video_dir_for_cap = video_dir.clone();
    let effective_mode_for_cap = effective_mode.clone();

    // Capture returns (frames, optional subtitle cues for text association)
    let (captured_frames, subtitle_cues) = tauri::async_runtime::spawn_blocking(move || {
        match effective_mode_for_cap.as_str() {
            "scene" => capture::capture_scene_change(&mp4_path, &video_dir_for_cap, scene_threshold)
                .map(|frames| (frames, Vec::new()))
                .map_err(|e| e.to_string()),
            "interval" => capture::capture_interval(
                &mp4_path,
                &video_dir_for_cap,
                interval_seconds,
                None,
            )
            .map(|frames| (frames, Vec::new()))
            .map_err(|e| e.to_string()),
            // "subtitle" (or any unknown mode — fall back to subtitle mode)
            _ => capture::capture_subtitle(&mp4_path, &video_dir_for_cap)
                .map(|r| (r.frames, r.cues))
                .map_err(|e| e.to_string()),
        }
    })
    .await
    .map_err(|e| format!("Capture task panicked: {e}"))?
    .map_err(|e| format!("Frame capture failed: {e}"))?;

    tracker.emit(
        app,
        100,
        Some(format!("Captured {} frames", captured_frames.len())),
    );
    tracker.complete_stage(app);

    // ─── Stage: Generate slides ───────────────────────────────────
    tracker.emit(app, 0, Some("Generating slides...".to_string()));

    // Convert captured frames to segments.
    // When subtitle cues are available (subtitle mode without fallback),
    // associate subtitle text with frames. Otherwise, frames display
    // timestamp only (e.g. "[00:01:30]").
    let segments: Vec<Segment> = if subtitle_cues.is_empty() {
        slides_generator::frames_to_segments(&captured_frames)
    } else {
        slides_generator::frames_to_segments_with_subtitles(&captured_frames, &subtitle_cues)
    };

    // Save segments.json to video_dir
    let segments_json_path = video_dir.join("segments.json");
    let segments_json = serde_json::to_string_pretty(&segments)
        .map_err(|e| format!("Failed to serialize segments: {e}"))?;
    std::fs::write(&segments_json_path, &segments_json)
        .map_err(|e| format!("Failed to write segments.json: {e}"))?;

    // Build slides_generator::VideoMetadata from fetched metadata
    let slides_meta = slides_generator::VideoMetadata {
        title: meta.title.clone(),
        url: item.url.clone(),
        channel: meta.channel.clone(),
        date: meta.upload_date.clone(),
        duration: format!("{:.0}s", meta.duration),
        video_id: meta.id.clone(),
    };

    let video_dir_for_slides = video_dir.clone();
    let segments_for_slides = segments.clone();

    tauri::async_runtime::spawn_blocking(move || {
        slides_generator::generate_slides_html(&video_dir_for_slides, &segments_for_slides, &slides_meta)
    })
    .await
    .map_err(|e| format!("Slides task panicked: {e}"))?
    .map_err(|e| format!("Slides generation failed: {e}"))?;

    tracker.emit(app, 100, Some("Slides generated".to_string()));
    tracker.complete_stage(app);

    // Stage: Cleanup — conditionally delete MP4 based on mp4_retention setting
    tracker.emit(app, 0, Some("Checking retention settings...".to_string()));

    let retain_mp4 = {
        let settings_state = app.state::<SettingsState>();
        settings_state
            .0
            .lock()
            .map(|s| s.mp4_retention)
            .unwrap_or(false) // Default to NOT retaining (delete) if lock fails
    };

    let cleanup_result = cleanup::cleanup_after_extraction(&video_dir, retain_mp4);

    if cleanup_result.mp4_deleted {
        let detail = format!(
            "Deleted {} file(s), freed {}",
            cleanup_result.files_deleted,
            cleanup::format_bytes(cleanup_result.bytes_freed)
        );
        println!("[queue_processor] Item {}: {}", item.id, detail);
        tracker.emit(app, 100, Some(detail));
    } else if let Some(reason) = &cleanup_result.skipped_reason {
        println!(
            "[queue_processor] Item {}: Cleanup skipped — {}",
            item.id, reason
        );
        tracker.emit(app, 100, Some("MP4 retained".to_string()));
    } else {
        tracker.emit(app, 100, Some("No MP4 files to clean up".to_string()));
    }

    tracker.complete_stage(app);

    // Done
    tracker.emit_done(app);

    Ok(Some(title))
}

/// Update a queue item's status in the shared state.
fn update_item_status(
    app: &AppHandle,
    id: u32,
    status: &str,
    title: Option<String>,
    error: Option<String>,
    progress: Option<u32>,
) {
    let pipeline = app.state::<PipelineState>();
    let queue_result = pipeline.queue.lock();
    if let Ok(mut queue) = queue_result {
        if let Some(item) = queue.iter_mut().find(|q| q.id == id) {
            item.status = status.to_string();
            if let Some(t) = title {
                item.title = Some(t);
            }
            if let Some(e) = error {
                item.error = Some(e);
            }
            if let Some(p) = progress {
                item.progress = Some(p);
            }
            // Clear pipeline stage info on terminal statuses
            if status == "completed" || status == "failed" || status == "skipped" {
                item.pipeline_stage = None;
                item.pipeline_stage_number = None;
                item.pipeline_total_stages = None;
                item.pipeline_detail = None;
            }
        }
    }
}

/// Emit a queue item status change event to the frontend.
fn emit_status_event(app: &AppHandle, event: &QueueStatusEvent) {
    let _ = app.emit("queue-item-status", event);
}

/// Update a queue item's capture mode in the shared state (used after fallback).
fn update_item_capture_mode(app: &AppHandle, id: u32, mode: &str) {
    let pipeline = app.state::<PipelineState>();
    let queue_result = pipeline.queue.lock();
    if let Ok(mut queue) = queue_result {
        if let Some(item) = queue.iter_mut().find(|q| q.id == id) {
            item.capture_mode = mode.to_string();
        }
    }
}

/// Emit overall queue progress event to the frontend.
fn emit_queue_progress(app: &AppHandle) -> Result<(), String> {
    let pipeline = app.state::<PipelineState>();
    let queue = pipeline.queue.lock().map_err(|e| e.to_string())?;

    let total = queue.len();
    let completed = queue.iter().filter(|q| q.status == "completed").count();
    let failed = queue.iter().filter(|q| q.status == "failed").count();

    let event = QueueProgressEvent {
        total,
        completed,
        failed,
        is_processing: is_processing(),
    };

    let _ = app.emit("queue-progress", &event);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processing_flag_default_false() {
        // Reset for test isolation
        PROCESSING_ACTIVE.store(false, Ordering::SeqCst);
        assert!(!is_processing());
    }

    #[test]
    fn processing_flag_set_and_clear() {
        PROCESSING_ACTIVE.store(false, Ordering::SeqCst);

        // Simulate starting processing
        let result = PROCESSING_ACTIVE.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        assert!(result.is_ok());
        assert!(is_processing());

        // Attempting to start again should fail (already processing)
        let result2 = PROCESSING_ACTIVE.compare_exchange(
            false,
            true,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        assert!(result2.is_err());

        // Clear
        PROCESSING_ACTIVE.store(false, Ordering::SeqCst);
        assert!(!is_processing());
    }

    #[test]
    fn queue_status_event_serialization() {
        let event = QueueStatusEvent {
            id: 1,
            status: "processing".to_string(),
            progress: Some(50),
            title: Some("Test Video".to_string()),
            error: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"status\":\"processing\""));
        assert!(json.contains("\"progress\":50"));
        assert!(json.contains("\"title\":\"Test Video\""));
        // error should be skipped (None)
        assert!(!json.contains("error"));
    }

    #[test]
    fn queue_status_event_with_error() {
        let event = QueueStatusEvent {
            id: 2,
            status: "failed".to_string(),
            progress: None,
            title: None,
            error: Some("Download failed".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"status\":\"failed\""));
        assert!(json.contains("\"error\":\"Download failed\""));
        // progress and title should be skipped
        assert!(!json.contains("progress"));
        assert!(!json.contains("title"));
    }

    #[test]
    fn queue_progress_event_serialization() {
        let event = QueueProgressEvent {
            total: 5,
            completed: 2,
            failed: 1,
            is_processing: true,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"total\":5"));
        assert!(json.contains("\"completed\":2"));
        assert!(json.contains("\"failed\":1"));
        assert!(json.contains("\"is_processing\":true"));
    }

    #[test]
    fn queue_status_valid_statuses() {
        // Verify all four valid status strings serialize correctly
        for status in &["pending", "processing", "completed", "failed"] {
            let event = QueueStatusEvent {
                id: 1,
                status: status.to_string(),
                progress: None,
                title: None,
                error: None,
            };
            let json = serde_json::to_string(&event).unwrap();
            assert!(json.contains(status));
        }
    }

    #[test]
    fn failed_event_carries_error_message() {
        // When a queue item fails, the event must include the error message
        // so the frontend can display it in the toast notification.
        let error_msg = "ffmpeg not found: No such file or directory";
        let event = QueueStatusEvent {
            id: 42,
            status: "failed".to_string(),
            progress: None,
            title: Some("My Video".to_string()),
            error: Some(error_msg.to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"status\":\"failed\""));
        assert!(json.contains("\"error\":\"ffmpeg not found"));
        assert!(json.contains("\"title\":\"My Video\""));

        // Deserialize round-trip
        let decoded: QueueStatusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, 42);
        assert_eq!(decoded.status, "failed");
        assert_eq!(decoded.error.as_deref(), Some(error_msg));
        assert_eq!(decoded.title.as_deref(), Some("My Video"));
    }

    #[test]
    fn failed_event_without_title() {
        // Failed items may not have a title if the error occurred before metadata fetch.
        let event = QueueStatusEvent {
            id: 5,
            status: "failed".to_string(),
            progress: None,
            title: None,
            error: Some("Invalid YouTube URL".to_string()),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"status\":\"failed\""));
        assert!(json.contains("\"error\":\"Invalid YouTube URL\""));
        assert!(!json.contains("\"title\""));
    }

    #[test]
    fn queue_progress_reflects_failures() {
        // The QueueProgressEvent should accurately count failed items
        // so the frontend badge shows the correct number of failures.
        let event = QueueProgressEvent {
            total: 10,
            completed: 5,
            failed: 3,
            is_processing: true,
        };

        let json = serde_json::to_string(&event).unwrap();
        let decoded: QueueProgressEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.total, 10);
        assert_eq!(decoded.completed, 5);
        assert_eq!(decoded.failed, 3);
        assert!(decoded.is_processing);

        // Remaining pending = total - completed - failed - processing(1)
        let remaining_pending = decoded.total - decoded.completed - decoded.failed - 1;
        assert_eq!(remaining_pending, 1);
    }

    #[test]
    fn queue_progress_done_after_all_processed() {
        // After all items are processed (some completed, some failed),
        // is_processing should be false.
        let event = QueueProgressEvent {
            total: 3,
            completed: 2,
            failed: 1,
            is_processing: false,
        };

        assert_eq!(event.total, event.completed + event.failed);
        assert!(!event.is_processing);
    }
}
