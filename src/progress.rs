//! 다운로드/캡쳐 파이프라인의 단계별 진행 보고 모듈.
//!
//! 파이프라인 단계를 정의하고 전체 단계 중 현재 단계 번호를 추적하며,
//! Tauri 이벤트를 통해 각 단계 내 진행률을 보고한다.

use crate::input_state::PipelineState;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// 각 파이프라인 단계의 식별자.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    /// yt-dlp로 YouTube 영상 다운로드 중.
    Downloading,
    /// 자막 추출 중 (자막 모드 전용).
    ExtractingSubtitles,
    /// ffmpeg으로 프레임 캡쳐 중.
    ExtractingFrames,
    /// slides.html 출력 생성 중.
    GeneratingSlides,
    /// 임시/원본 파일 정리 중.
    Cleanup,
    /// 파이프라인 완료.
    Done,
}

impl PipelineStage {
    /// 단계의 i18n 키 (프론트엔드에서 번역 조회에 사용).
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

/// Tauri 이벤트 `pipeline:progress`로 발행되는 페이로드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    /// 이 진행 정보가 속한 큐 항목 ID.
    pub queue_id: u32,
    /// 현재 파이프라인 단계.
    pub stage: PipelineStage,
    /// 현재 단계의 1-기반 인덱스.
    pub stage_number: u32,
    /// 이번 파이프라인 실행의 총 단계 수.
    pub total_stages: u32,
    /// 현재 단계 내 진행률 (0~100).
    pub percent: u32,
    /// 선택적 세부 메시지 (예: "frame 12/48").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Tauri 이벤트 `pipeline:error`로 발행되는 페이로드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorPayload {
    /// 이 오류가 속한 큐 항목 ID.
    pub queue_id: u32,
    /// 오류가 발생한 단계.
    pub stage: PipelineStage,
    /// 오류 메시지.
    pub message: String,
}

/// 주어진 캡쳐 모드에 따른 순서가 있는 단계 목록을 반환한다.
///
/// - `"subtitle"` 모드는 자막 추출 단계를 포함한다.
/// - `"scene"` 및 `"interval"` 모드는 자막 추출을 건너뛴다.
///
/// 모든 모드는 Done 전 마지막 처리 단계로 Cleanup을 포함한다.
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

/// 단일 큐 항목의 파이프라인 실행 진행을 추적하고 발행하는 헬퍼.
pub struct ProgressTracker {
    queue_id: u32,
    stages: Vec<PipelineStage>,
    current_stage_index: usize,
}

impl ProgressTracker {
    /// 주어진 큐 항목과 캡쳐 모드에 대한 새 트래커를 생성한다.
    pub fn new(queue_id: u32, capture_mode: &str) -> Self {
        Self {
            queue_id,
            stages: stages_for_mode(capture_mode),
            current_stage_index: 0,
        }
    }

    /// 총 단계 수 (Done 제외).
    pub fn total_stages(&self) -> u32 {
        self.stages.len() as u32
    }

    /// 현재 단계 (끝을 지난 경우 패닉 — 호출자가 확인해야 함).
    pub fn current_stage(&self) -> PipelineStage {
        self.stages
            .get(self.current_stage_index)
            .copied()
            .unwrap_or(PipelineStage::Done)
    }

    /// 1-기반 단계 번호.
    pub fn stage_number(&self) -> u32 {
        (self.current_stage_index + 1).min(self.stages.len()) as u32
    }

    /// 다음 단계로 진행한다. 새 단계(또는 `Done`)를 반환한다.
    pub fn advance(&mut self) -> PipelineStage {
        if self.current_stage_index < self.stages.len() {
            self.current_stage_index += 1;
        }
        self.current_stage()
    }

    /// 현재 단계의 주어진 진행률로 `ProgressPayload`를 생성한다.
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

    /// 프론트엔드로 진행 이벤트를 발행하고 단계 정보를 큐 항목에 동기화한다.
    pub fn emit(&self, app: &AppHandle, percent: u32, detail: Option<String>) {
        let payload = self.payload(percent, detail);
        // Sync to queue item state so it can be queried via get_queue
        self.sync_to_queue(app, &payload);
        let _ = app.emit("pipeline:progress", &payload);
    }

    /// 단계 완료 이벤트(100%)를 발행하고 다음 단계로 진행한다.
    pub fn complete_stage(&mut self, app: &AppHandle) {
        self.emit(app, 100, None);
        self.advance();
    }

    /// 파이프라인이 성공적으로 완료되었음을 나타내는 "done" 이벤트를 발행한다.
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

    /// 현재 진행 페이로드를 큐 항목의 상태 필드에 동기화해
    /// `get_queue`가 최신 파이프라인 단계 정보를 반환하도록 한다.
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
