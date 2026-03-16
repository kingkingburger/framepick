//! Stage-based progress reporting for the download/capture pipeline.
//!
//! Defines pipeline stages, tracks the current stage number out of total stages,
//! and reports percentage progress within each stage via Tauri events.

use crate::input_state::PipelineState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// Identifiers for each pipeline stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    /// Downloading video from YouTube via yt-dlp
    Downloading,
    /// Extracting subtitles (subtitle mode only)
    ExtractingSubtitles,
    /// Capturing frames via ffmpeg
    ExtractingFrames,
    /// Generating slides.html output
    GeneratingSlides,
    /// Cleaning up temporary/source files
    Cleanup,
    /// Pipeline complete
    Done,
}

impl PipelineStage {
    /// Human-readable i18n key for the stage (used by the frontend to look up translations).
    pub fn i18n_key(&self) -> &'static str {
        match self {
            PipelineStage::Downloading => "progress_downloading",
            PipelineStage::ExtractingSubtitles => "progress_extracting_subtitles",
            PipelineStage::ExtractingFrames => "progress_extracting_frames",
            PipelineStage::GeneratingSlides => "progress_generating_slides",
            PipelineStage::Cleanup => "progress_cleanup",
            PipelineStage::Done => "progress_done",
        }
    }
}

/// Payload emitted via Tauri event `pipeline:progress`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    /// Queue item ID this progress belongs to
    pub queue_id: u32,
    /// Current pipeline stage
    pub stage: PipelineStage,
    /// 1-based index of the current stage
    pub stage_number: u32,
    /// Total number of stages in this pipeline run
    pub total_stages: u32,
    /// Progress percentage within the current stage (0–100)
    pub percent: u32,
    /// Optional detail message (e.g., "frame 12/48")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Payload emitted via Tauri event `pipeline:error`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// Queue item ID this error belongs to
    pub queue_id: u32,
    /// Stage where the error occurred
    pub stage: PipelineStage,
    /// Error message
    pub message: String,
}

/// Determines the ordered list of stages for a given capture mode.
///
/// - `"subtitle"` mode includes subtitle extraction.
/// - `"scene"` and `"interval"` modes skip subtitle extraction.
///
/// All modes include Cleanup as the last processing stage before Done.
pub fn stages_for_mode(capture_mode: &str) -> Vec<PipelineStage> {
    match capture_mode {
        "subtitle" => vec![
            PipelineStage::Downloading,
            PipelineStage::ExtractingSubtitles,
            PipelineStage::ExtractingFrames,
            PipelineStage::GeneratingSlides,
            PipelineStage::Cleanup,
        ],
        _ => vec![
            PipelineStage::Downloading,
            PipelineStage::ExtractingFrames,
            PipelineStage::GeneratingSlides,
            PipelineStage::Cleanup,
        ],
    }
}

/// Helper to track and emit progress for a single queue item's pipeline run.
pub struct ProgressTracker {
    queue_id: u32,
    stages: Vec<PipelineStage>,
    current_stage_index: usize,
}

impl ProgressTracker {
    /// Create a new tracker for the given queue item and capture mode.
    pub fn new(queue_id: u32, capture_mode: &str) -> Self {
        Self {
            queue_id,
            stages: stages_for_mode(capture_mode),
            current_stage_index: 0,
        }
    }

    /// Total number of stages (excluding Done).
    pub fn total_stages(&self) -> u32 {
        self.stages.len() as u32
    }

    /// The current stage (panics if past the end — callers should check).
    pub fn current_stage(&self) -> PipelineStage {
        self.stages
            .get(self.current_stage_index)
            .copied()
            .unwrap_or(PipelineStage::Done)
    }

    /// 1-based stage number.
    pub fn stage_number(&self) -> u32 {
        (self.current_stage_index + 1).min(self.stages.len()) as u32
    }

    /// Advance to the next stage.  Returns the new stage (or `Done`).
    pub fn advance(&mut self) -> PipelineStage {
        if self.current_stage_index < self.stages.len() {
            self.current_stage_index += 1;
        }
        self.current_stage()
    }

    /// Build a `ProgressPayload` for the current stage at the given percent.
    pub fn payload(&self, percent: u32, detail: Option<String>) -> ProgressPayload {
        ProgressPayload {
            queue_id: self.queue_id,
            stage: self.current_stage(),
            stage_number: self.stage_number(),
            total_stages: self.total_stages(),
            percent: percent.min(100),
            detail,
        }
    }

    /// Emit a progress event to the frontend and sync stage info to the queue item.
    pub fn emit(&self, app: &AppHandle, percent: u32, detail: Option<String>) {
        let payload = self.payload(percent, detail);
        // Sync to queue item state so it can be queried via get_queue
        self.sync_to_queue(app, &payload);
        let _ = app.emit("pipeline:progress", &payload);
    }

    /// Emit a stage-complete event (100%) and advance to the next stage.
    pub fn complete_stage(&mut self, app: &AppHandle) {
        self.emit(app, 100, None);
        self.advance();
    }

    /// Emit a "done" event indicating the pipeline finished successfully.
    pub fn emit_done(&self, app: &AppHandle) {
        let payload = ProgressPayload {
            queue_id: self.queue_id,
            stage: PipelineStage::Done,
            stage_number: self.total_stages(),
            total_stages: self.total_stages(),
            percent: 100,
            detail: None,
        };
        self.sync_to_queue(app, &payload);
        let _ = app.emit("pipeline:progress", &payload);
    }

    /// Emit an error event.
    pub fn emit_error(&self, app: &AppHandle, message: &str) {
        let payload = ErrorPayload {
            queue_id: self.queue_id,
            stage: self.current_stage(),
            message: message.to_string(),
        };
        let _ = app.emit("pipeline:error", &payload);
    }

    /// Sync the current progress payload to the queue item's state fields,
    /// so `get_queue` returns up-to-date pipeline stage info.
    fn sync_to_queue(&self, app: &AppHandle, payload: &ProgressPayload) {
        let pipeline = app.state::<PipelineState>();
        let lock_result = pipeline.queue.lock();
        if let Ok(mut queue) = lock_result {
            if let Some(item) = queue.iter_mut().find(|q| q.id == self.queue_id) {
                let stage_str = serde_json::to_value(&payload.stage)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| format!("{:?}", payload.stage).to_lowercase());
                item.pipeline_stage = Some(stage_str);
                item.pipeline_stage_number = Some(payload.stage_number);
                item.pipeline_total_stages = Some(payload.total_stages);
                item.progress = Some(payload.percent);
                item.pipeline_detail = payload.detail.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_for_subtitle_mode() {
        let stages = stages_for_mode("subtitle");
        assert_eq!(stages.len(), 5);
        assert_eq!(stages[0], PipelineStage::Downloading);
        assert_eq!(stages[1], PipelineStage::ExtractingSubtitles);
        assert_eq!(stages[2], PipelineStage::ExtractingFrames);
        assert_eq!(stages[3], PipelineStage::GeneratingSlides);
        assert_eq!(stages[4], PipelineStage::Cleanup);
    }

    #[test]
    fn stages_for_scene_mode() {
        let stages = stages_for_mode("scene");
        assert_eq!(stages.len(), 4);
        assert_eq!(stages[0], PipelineStage::Downloading);
        assert_eq!(stages[1], PipelineStage::ExtractingFrames);
        assert_eq!(stages[2], PipelineStage::GeneratingSlides);
        assert_eq!(stages[3], PipelineStage::Cleanup);
    }

    #[test]
    fn stages_for_interval_mode() {
        let stages = stages_for_mode("interval");
        assert_eq!(stages.len(), 4);
        assert_eq!(stages[0], PipelineStage::Downloading);
        assert_eq!(stages[1], PipelineStage::ExtractingFrames);
    }

    #[test]
    fn tracker_basic_flow() {
        let mut tracker = ProgressTracker::new(1, "subtitle");
        assert_eq!(tracker.total_stages(), 5);
        assert_eq!(tracker.stage_number(), 1);
        assert_eq!(tracker.current_stage(), PipelineStage::Downloading);

        // Advance through stages
        tracker.advance();
        assert_eq!(tracker.stage_number(), 2);
        assert_eq!(tracker.current_stage(), PipelineStage::ExtractingSubtitles);

        tracker.advance();
        assert_eq!(tracker.stage_number(), 3);
        assert_eq!(tracker.current_stage(), PipelineStage::ExtractingFrames);

        tracker.advance();
        assert_eq!(tracker.stage_number(), 4);
        assert_eq!(tracker.current_stage(), PipelineStage::GeneratingSlides);

        tracker.advance();
        assert_eq!(tracker.stage_number(), 5);
        assert_eq!(tracker.current_stage(), PipelineStage::Cleanup);

        // Past end → Done
        tracker.advance();
        assert_eq!(tracker.current_stage(), PipelineStage::Done);
    }

    #[test]
    fn tracker_scene_mode_flow() {
        let mut tracker = ProgressTracker::new(42, "scene");
        assert_eq!(tracker.total_stages(), 4);
        assert_eq!(tracker.current_stage(), PipelineStage::Downloading);

        tracker.advance();
        assert_eq!(tracker.current_stage(), PipelineStage::ExtractingFrames);
    }

    #[test]
    fn payload_construction() {
        let tracker = ProgressTracker::new(7, "interval");
        let p = tracker.payload(55, Some("frame 5/10".to_string()));
        assert_eq!(p.queue_id, 7);
        assert_eq!(p.stage, PipelineStage::Downloading);
        assert_eq!(p.stage_number, 1);
        assert_eq!(p.total_stages, 4);
        assert_eq!(p.percent, 55);
        assert_eq!(p.detail.as_deref(), Some("frame 5/10"));
    }

    #[test]
    fn payload_clamps_percent() {
        let tracker = ProgressTracker::new(1, "scene");
        let p = tracker.payload(150, None);
        assert_eq!(p.percent, 100);
    }

    #[test]
    fn stage_i18n_keys() {
        assert_eq!(PipelineStage::Downloading.i18n_key(), "progress_downloading");
        assert_eq!(PipelineStage::ExtractingSubtitles.i18n_key(), "progress_extracting_subtitles");
        assert_eq!(PipelineStage::ExtractingFrames.i18n_key(), "progress_extracting_frames");
        assert_eq!(PipelineStage::GeneratingSlides.i18n_key(), "progress_generating_slides");
        assert_eq!(PipelineStage::Cleanup.i18n_key(), "progress_cleanup");
        assert_eq!(PipelineStage::Done.i18n_key(), "progress_done");
    }

    #[test]
    fn progress_payload_serialization() {
        let payload = ProgressPayload {
            queue_id: 1,
            stage: PipelineStage::Downloading,
            stage_number: 1,
            total_stages: 4,
            percent: 50,
            detail: Some("50%".to_string()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"downloading\""));
        assert!(json.contains("\"stage_number\":1"));
        assert!(json.contains("\"total_stages\":4"));
        assert!(json.contains("\"percent\":50"));

        let loaded: ProgressPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.queue_id, 1);
        assert_eq!(loaded.stage, PipelineStage::Downloading);
    }

    #[test]
    fn error_payload_serialization() {
        let payload = ErrorPayload {
            queue_id: 2,
            stage: PipelineStage::ExtractingFrames,
            message: "ffmpeg not found".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"extracting_frames\""));
        assert!(json.contains("ffmpeg not found"));

        let loaded: ErrorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.queue_id, 2);
        assert_eq!(loaded.message, "ffmpeg not found");
    }

    #[test]
    fn detail_skipped_when_none() {
        let payload = ProgressPayload {
            queue_id: 1,
            stage: PipelineStage::Done,
            stage_number: 4,
            total_stages: 4,
            percent: 100,
            detail: None,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("detail"));
    }

    #[test]
    fn error_payload_at_each_stage() {
        // Verify error payloads correctly identify the failing stage,
        // which the frontend uses to show contextual error toasts.
        let stages = vec![
            (PipelineStage::Downloading, "downloading", "yt-dlp error: network timeout"),
            (PipelineStage::ExtractingSubtitles, "extracting_subtitles", "No subtitles available"),
            (PipelineStage::ExtractingFrames, "extracting_frames", "ffmpeg: codec not supported"),
            (PipelineStage::GeneratingSlides, "generating_slides", "Failed to write slides.html"),
            (PipelineStage::Cleanup, "cleanup", "Permission denied deleting mp4"),
        ];

        for (stage, expected_key, message) in stages {
            let payload = ErrorPayload {
                queue_id: 99,
                stage,
                message: message.to_string(),
            };
            let json = serde_json::to_string(&payload).unwrap();
            assert!(
                json.contains(expected_key),
                "Expected stage key '{}' in JSON: {}",
                expected_key,
                json
            );
            assert!(json.contains(message));
        }
    }

    #[test]
    fn error_payload_round_trip() {
        let payload = ErrorPayload {
            queue_id: 7,
            stage: PipelineStage::Downloading,
            message: "Connection refused".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let decoded: ErrorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.queue_id, 7);
        assert_eq!(decoded.stage, PipelineStage::Downloading);
        assert_eq!(decoded.message, "Connection refused");
    }
}
