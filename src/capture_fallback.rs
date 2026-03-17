//! 캡쳐 모드 자동 전환(폴백) 로직.
//!
//! 사용자가 "subtitle" 캡쳐 모드를 선택했을 때, 대상 영상에 실제로
//! 다운로드 가능한 자막이 있는지 확인한다. 없으면 자동으로 "scene"
//! (장면 전환 감지) 모드로 전환하고, Tauri 이벤트로 프론트엔드에 알린다.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::subtitle_detector::{check_subtitles, SubtitleCheckResult};
use crate::subtitle_extractor::{select_best_subtitle_language, SubtitleLanguageSelection};

/// 폴백 로직 적용 후 결정된 실제 캡쳐 모드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCaptureMode {
    /// 실제로 사용될 캡쳐 모드 ("subtitle", "scene", "interval" 중 하나).
    pub effective_mode: String,
    /// 사용자가 원래 요청한 모드.
    pub requested_mode: String,
    /// 폴백이 발생했는지 여부 (요청 모드 != 실제 모드).
    pub did_fallback: bool,
    /// 폴백 사유 (폴백 없으면 빈 문자열).
    pub fallback_reason: String,
    /// 폴백 사유 i18n 키 (폴백 없으면 빈 문자열).
    /// 프론트엔드가 현지화된 알림을 표시할 때 사용한다.
    pub fallback_reason_key: String,
    /// 자막 확인 결과 (subtitle 모드가 요청된 경우에만 채워짐).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_check: Option<SubtitleCheckResult>,
    /// 선택된 자막 언어 (subtitle 모드가 확정된 경우에만 채워짐).
    /// 한국어 → 영어 → 기타 순으로 우선순위를 적용한다.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_language: Option<SubtitleLanguageSelection>,
}

impl ResolvedCaptureMode {
    /// 폴백 없음 — 요청된 모드를 그대로 사용한다.
    fn no_fallback(mode: &str) -> Self {
        Self {
            effective_mode: mode.to_string(),
            requested_mode: mode.to_string(),
            did_fallback: false,
            fallback_reason: String::new(),
            fallback_reason_key: String::new(),
            subtitle_check: None,
            selected_language: None,
        }
    }

    /// 자막 가용성이 확인되고 언어가 선택된 subtitle 모드.
    fn subtitle_confirmed(check: SubtitleCheckResult, lang: SubtitleLanguageSelection) -> Self {
        Self {
            effective_mode: "subtitle".to_string(),
            requested_mode: "subtitle".to_string(),
            did_fallback: false,
            fallback_reason: String::new(),
            fallback_reason_key: String::new(),
            subtitle_check: Some(check),
            selected_language: Some(lang),
        }
    }

    /// subtitle에서 장면 전환 모드로 폴백한다.
    fn fallback_to_scene(reason: &str, reason_key: &str, check: SubtitleCheckResult) -> Self {
        Self {
            effective_mode: "scene".to_string(),
            requested_mode: "subtitle".to_string(),
            did_fallback: true,
            fallback_reason: reason.to_string(),
            fallback_reason_key: reason_key.to_string(),
            subtitle_check: Some(check),
            selected_language: None,
        }
    }
}

/// 캡쳐 모드 폴백이 발생했을 때 발송되는 Tauri 이벤트 페이로드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackEvent {
    /// 큐 항목 ID (큐 처리 전 해결 시에는 0).
    pub queue_id: u32,
    /// 처리 중인 영상 URL.
    pub url: String,
    /// 사용자가 원래 요청한 모드.
    pub requested_mode: String,
    /// 실제로 사용될 모드.
    pub effective_mode: String,
    /// 폴백 사유 i18n 키.
    pub reason_key: String,
    /// 로깅용 폴백 사유 (영어).
    pub reason: String,
}

/// 영상의 실제 캡쳐 모드를 결정한다.
///
/// "subtitle" 모드의 경우 자막 가용성을 확인하고, 자막이 없으면
/// "scene" 모드로 폴백한다. "scene"과 "interval" 모드는 그대로 반환된다.
///
/// 이 함수는 블로킹 — yt-dlp를 호출한다. 호출자는 백그라운드 스레드에서
/// 실행해야 한다 (예: `tokio::task::spawn_blocking`).
pub fn resolve_capture_mode(video_url: &str, requested_mode: &str) -> ResolvedCaptureMode {
    match requested_mode {
        "subtitle" => resolve_subtitle_mode(video_url),
        "scene" | "interval" => ResolvedCaptureMode::no_fallback(requested_mode),
        other => {
            eprintln!("[capture_fallback] Unknown capture mode '{}', defaulting to scene", other);
            ResolvedCaptureMode::no_fallback("scene")
        }
    }
}

/// 자막 가용성을 확인하고 scene 모드로 폴백할지 결정한다.
fn resolve_subtitle_mode(video_url: &str) -> ResolvedCaptureMode {
    println!("[capture_fallback] Checking subtitle availability for: {}", video_url);

    let check_result = check_subtitles(video_url);

    // If the subtitle check itself errored (e.g., yt-dlp not found), fall back
    if !check_result.error.is_empty() {
        eprintln!(
            "[capture_fallback] Subtitle check failed: {}. Falling back to scene-change mode.",
            check_result.error
        );
        return ResolvedCaptureMode::fallback_to_scene(
            &format!("Subtitle check failed: {}", check_result.error),
            "fallback_subtitle_check_error",
            check_result,
        );
    }

    // If no subtitles at all (neither manual nor auto), fall back
    if !check_result.has_subtitles {
        println!(
            "[capture_fallback] No subtitles available. Falling back to scene-change mode."
        );
        return ResolvedCaptureMode::fallback_to_scene(
            "No subtitles available for this video",
            "fallback_no_subtitles",
            check_result,
        );
    }

    // Subtitles are available — select best language (Korean priority → English fallback)
    match select_best_subtitle_language(&check_result) {
        Some(lang_selection) => {
            println!(
                "[capture_fallback] Subtitles found. Selected: {} (preferred={}, manual={})",
                lang_selection.description, lang_selection.is_preferred, lang_selection.is_manual
            );
            ResolvedCaptureMode::subtitle_confirmed(check_result, lang_selection)
        }
        None => {
            // Shouldn't happen since has_subtitles is true, but handle gracefully
            println!(
                "[capture_fallback] Subtitles reported available but no suitable language found. Falling back."
            );
            ResolvedCaptureMode::fallback_to_scene(
                "No suitable subtitle language found (requires Korean or English)",
                "fallback_no_suitable_language",
                check_result,
            )
        }
    }
}

/// Tauri 이벤트 시스템을 통해 폴백 이벤트를 프론트엔드에 발송한다.
///
/// 프론트엔드는 `capture:fallback` 이벤트를 수신해 캡쳐 모드가
/// 자동으로 변경됐음을 알리는 토스트 알림을 표시한다.
pub fn emit_fallback_event(
    app: &AppHandle,
    queue_id: u32,
    url: &str,
    resolved: &ResolvedCaptureMode,
) {
    if !resolved.did_fallback {
        return;
    }

    let event = FallbackEvent {
        queue_id,
        url: url.to_string(),
        requested_mode: resolved.requested_mode.clone(),
        effective_mode: resolved.effective_mode.clone(),
        reason_key: resolved.fallback_reason_key.clone(),
        reason: resolved.fallback_reason.clone(),
    };

    if let Err(e) = app.emit("capture:fallback", &event) {
        eprintln!("[capture_fallback] Failed to emit fallback event: {}", e);
    }
}

/// Tauri 커맨드: 주어진 영상 URL의 실제 캡쳐 모드를 결정한다.
///
/// 파이프라인 처리 전(또는 시작 시) 프론트엔드가 호출하여
/// subtitle 모드 사용 가능 여부 또는 폴백 필요 여부를 판단한다.
///
/// UI 블로킹 방지를 위해 yt-dlp를 백그라운드 스레드에서 실행한다.
#[tauri::command]
pub async fn resolve_capture_mode_cmd(
    url: String,
    capture_mode: String,
    queue_id: Option<u32>,
    app: AppHandle,
) -> Result<ResolvedCaptureMode, String> {
    let url_clone = url.clone();
    let mode_clone = capture_mode.clone();

    let resolved = tauri::async_runtime::spawn_blocking(move || {
        resolve_capture_mode(&url_clone, &mode_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    // Emit fallback event to frontend if a fallback occurred
    if resolved.did_fallback {
        let qid = queue_id.unwrap_or(0);
        emit_fallback_event(&app, qid, &url, &resolved);

        println!(
            "[capture_fallback] Fallback for queue_id={}: {} -> {} (reason: {})",
            qid, resolved.requested_mode, resolved.effective_mode, resolved.fallback_reason
        );
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_fallback_for_scene_mode() {
        let result = ResolvedCaptureMode::no_fallback("scene");
        assert_eq!(result.effective_mode, "scene");
        assert_eq!(result.requested_mode, "scene");
        assert!(!result.did_fallback);
        assert!(result.fallback_reason.is_empty());
        assert!(result.subtitle_check.is_none());
    }

    #[test]
    fn no_fallback_for_interval_mode() {
        let result = ResolvedCaptureMode::no_fallback("interval");
        assert_eq!(result.effective_mode, "interval");
        assert!(!result.did_fallback);
    }

    #[test]
    fn fallback_to_scene_from_subtitle() {
        let check = SubtitleCheckResult {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: vec![],
            auto_languages: vec![],
            error: String::new(),
        };
        let result = ResolvedCaptureMode::fallback_to_scene(
            "No subtitles available",
            "fallback_no_subtitles",
            check,
        );
        assert_eq!(result.effective_mode, "scene");
        assert_eq!(result.requested_mode, "subtitle");
        assert!(result.did_fallback);
        assert_eq!(result.fallback_reason_key, "fallback_no_subtitles");
        assert!(result.subtitle_check.is_some());
        assert!(!result.subtitle_check.unwrap().has_subtitles);
    }

    #[test]
    fn subtitle_confirmed_no_fallback() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: false,
            manual_languages: vec!["ko".to_string(), "en".to_string()],
            auto_languages: vec![],
            error: String::new(),
        };
        let lang = SubtitleLanguageSelection {
            language: "ko".to_string(),
            is_manual: true,
            is_preferred: true,
            description: "Manual Korean subtitles (ko)".to_string(),
            i18n_key: "subtitle_lang_korean_manual".to_string(),
        };
        let result = ResolvedCaptureMode::subtitle_confirmed(check, lang);
        assert_eq!(result.effective_mode, "subtitle");
        assert!(!result.did_fallback);
        assert!(result.subtitle_check.is_some());
        assert!(result.subtitle_check.unwrap().has_manual_subtitles);
        assert!(result.selected_language.is_some());
        assert_eq!(result.selected_language.unwrap().language, "ko");
    }

    #[test]
    fn fallback_on_subtitle_check_error() {
        let check = SubtitleCheckResult {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: vec![],
            auto_languages: vec![],
            error: "yt-dlp not found".to_string(),
        };
        let result = ResolvedCaptureMode::fallback_to_scene(
            "Subtitle check failed: yt-dlp not found",
            "fallback_subtitle_check_error",
            check,
        );
        assert!(result.did_fallback);
        assert_eq!(result.effective_mode, "scene");
        assert_eq!(result.fallback_reason_key, "fallback_subtitle_check_error");
        assert!(result.fallback_reason.contains("yt-dlp not found"));
    }

    #[test]
    fn resolve_scene_mode_passthrough() {
        // Scene and interval modes should pass through without any subtitle check
        let result = resolve_capture_mode("https://youtube.com/watch?v=test", "scene");
        assert_eq!(result.effective_mode, "scene");
        assert!(!result.did_fallback);
        assert!(result.subtitle_check.is_none());
    }

    #[test]
    fn resolve_interval_mode_passthrough() {
        let result = resolve_capture_mode("https://youtube.com/watch?v=test", "interval");
        assert_eq!(result.effective_mode, "interval");
        assert!(!result.did_fallback);
    }

    #[test]
    fn resolve_unknown_mode_defaults_to_scene() {
        let result = resolve_capture_mode("https://youtube.com/watch?v=test", "unknown_mode");
        assert_eq!(result.effective_mode, "scene");
        assert!(!result.did_fallback); // Not a "fallback" per se, just a default
    }

    #[test]
    fn serialization_roundtrip() {
        let result = ResolvedCaptureMode::fallback_to_scene(
            "No subtitles",
            "fallback_no_subtitles",
            SubtitleCheckResult {
                has_subtitles: false,
                has_manual_subtitles: false,
                has_auto_subtitles: false,
                manual_languages: vec![],
                auto_languages: vec![],
                error: String::new(),
            },
        );
        let json = serde_json::to_string(&result).unwrap();
        let loaded: ResolvedCaptureMode = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.effective_mode, "scene");
        assert_eq!(loaded.requested_mode, "subtitle");
        assert!(loaded.did_fallback);
        assert_eq!(loaded.fallback_reason_key, "fallback_no_subtitles");
    }

    #[test]
    fn fallback_event_serialization() {
        let event = FallbackEvent {
            queue_id: 42,
            url: "https://youtube.com/watch?v=test123".to_string(),
            requested_mode: "subtitle".to_string(),
            effective_mode: "scene".to_string(),
            reason_key: "fallback_no_subtitles".to_string(),
            reason: "No subtitles available for this video".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"queue_id\":42"));
        assert!(json.contains("subtitle"));
        assert!(json.contains("scene"));
        assert!(json.contains("fallback_no_subtitles"));

        let loaded: FallbackEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.queue_id, 42);
        assert_eq!(loaded.effective_mode, "scene");
    }

    #[test]
    fn subtitle_check_none_when_no_fallback_non_subtitle_mode() {
        let result = ResolvedCaptureMode::no_fallback("scene");
        // subtitle_check should be None for non-subtitle modes
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("subtitle_check"));
    }

    #[test]
    fn fallback_reason_key_is_set_correctly() {
        // Test the two different fallback reason keys
        let check_no_subs = SubtitleCheckResult {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: vec![],
            auto_languages: vec![],
            error: String::new(),
        };
        let result1 = ResolvedCaptureMode::fallback_to_scene(
            "No subtitles available",
            "fallback_no_subtitles",
            check_no_subs,
        );
        assert_eq!(result1.fallback_reason_key, "fallback_no_subtitles");

        let check_error = SubtitleCheckResult {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: vec![],
            auto_languages: vec![],
            error: "network error".to_string(),
        };
        let result2 = ResolvedCaptureMode::fallback_to_scene(
            "Subtitle check failed",
            "fallback_subtitle_check_error",
            check_error,
        );
        assert_eq!(result2.fallback_reason_key, "fallback_subtitle_check_error");
    }
}
