//! 설정 모듈 — config 타입을 래핑하고 Tauri 커맨드를 제공한다.
//!
//! 실제 데이터 모델은 `crate::config`에 있다. 이 모듈은
//! 프론트엔드와 통신하는 Tauri API(`SettingsState`, `get_settings`,
//! `update_settings`, `validate_settings`, `reset_settings`, `get_config_path`)를 담당한다.

pub use crate::config::{AppConfig, Language, VALID_CAPTURE_MODES, VALID_QUALITIES};

use crate::config::ConfigState;
use serde::Serialize;
use std::sync::Mutex;

/// Tauri 통합 레이어 전체에서 사용하는 타입 별칭.
pub type Settings = AppConfig;

/// Tauri managed state용 스레드 안전 래퍼.
pub struct SettingsState(pub Mutex<Settings>);

/// 실행 파일 옆 config.json에서 설정을 불러온다.
/// 파일이 없으면 기본값을 반환한다.
pub fn load_settings() -> Result<Settings, String> {
    let path = ConfigState::resolve_config_path();
    if path.exists() {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let settings: Settings = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        // Validate loaded settings — use defaults for invalid fields
        let errors = settings.validate();
        if !errors.is_empty() {
            eprintln!(
                "Warning: config.json has validation issues: {}",
                errors.join("; ")
            );
            // Still return loaded settings (they're usable, just potentially suboptimal)
        }
        Ok(settings)
    } else {
        Ok(Settings::default())
    }
}

/// 실행 파일 옆 config.json에 설정을 저장한다.
pub fn save_settings(settings: &Settings) -> Result<(), String> {
    let path = ConfigState::resolve_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))?;
    Ok(())
}

/// Tauri 커맨드: 현재 설정을 프론트엔드에 반환한다.
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, SettingsState>) -> Result<Settings, String> {
    let settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;
    Ok(settings.clone())
}

/// Tauri 커맨드: 설정을 부분 업데이트하고 디스크에 저장한다.
///
/// `Settings` 필드의 임의 부분 집합을 담은 JSON 객체를 받는다.
/// 제공된 필드만 덮어쓰고 나머지는 기존 값을 유지한다.
/// 저장 전에 모든 필드의 유효성을 검사한다.
#[tauri::command]
pub fn update_settings(
    state: tauri::State<'_, SettingsState>,
    patch: serde_json::Value,
) -> Result<Settings, String> {
    let mut settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;

    if let Some(v) = patch.get("library_path").and_then(|v| v.as_str()) {
        settings.library_path = v.to_string();
    }
    if let Some(v) = patch.get("download_quality").and_then(|v| v.as_str()) {
        // Validate quality value before accepting
        if !VALID_QUALITIES.contains(&v) {
            return Err(format!(
                "Invalid download_quality '{}'. Must be one of: {}",
                v,
                VALID_QUALITIES.join(", ")
            ));
        }
        settings.download_quality = v.to_string();
    }
    if let Some(v) = patch.get("language").and_then(|v| v.as_str()) {
        match v {
            "ko" => settings.language = Language::Ko,
            "en" => settings.language = Language::En,
            other => return Err(format!("Unknown language: {other}")),
        }
    }
    if let Some(v) = patch.get("mp4_retention").and_then(|v| v.as_bool()) {
        settings.mp4_retention = v;
    }
    if let Some(v) = patch.get("default_capture_mode").and_then(|v| v.as_str()) {
        if !VALID_CAPTURE_MODES.contains(&v) {
            return Err(format!(
                "Invalid default_capture_mode '{}'. Must be one of: {}",
                v,
                VALID_CAPTURE_MODES.join(", ")
            ));
        }
        settings.default_capture_mode = v.to_string();
    }
    if let Some(v) = patch.get("default_interval_seconds").and_then(|v| v.as_u64()) {
        let secs = v as u32;
        if secs == 0 || secs > 3600 {
            return Err(format!(
                "default_interval_seconds must be 1–3600, got {}",
                secs
            ));
        }
        settings.default_interval_seconds = secs;
    }
    if let Some(v) = patch.get("scene_change_threshold").and_then(|v| v.as_f64()) {
        if v < 0.01 || v > 1.0 {
            return Err(format!(
                "scene_change_threshold must be 0.01–1.0, got {}",
                v
            ));
        }
        settings.scene_change_threshold = v;
    }

    // Validate the resulting settings before saving
    let errors = settings.validate();
    if !errors.is_empty() {
        return Err(format!("Settings validation failed: {}", errors.join("; ")));
    }

    save_settings(&settings)?;
    Ok(settings.clone())
}

/// 설정 유효성 검사 결과 — 필드 수준의 세부 정보를 포함한다.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    /// 전체 유효성 여부
    pub valid: bool,
    /// 오류 메시지 목록 (유효하면 비어 있음)
    pub errors: Vec<String>,
    /// 실행 파일 옆에 ffmpeg.exe가 존재하는지 여부
    pub ffmpeg_found: bool,
    /// 실행 파일 옆에 yt-dlp.exe가 존재하는지 여부
    pub ytdlp_found: bool,
    /// 라이브러리 디렉토리의 절대 경로
    pub resolved_library_path: String,
    /// 라이브러리 디렉토리의 실제 존재 여부
    pub library_exists: bool,
    /// config.json이 저장된 경로
    pub config_path: String,
}

/// Tauri 커맨드: 현재 설정의 유효성을 검사하고 외부 도구 존재 여부를 확인한다.
///
/// 필드 오류, 도구 존재 여부, 경로 정보가 담긴 `ValidationResult`를 반환한다.
#[tauri::command]
pub fn validate_settings(
    state: tauri::State<'_, SettingsState>,
) -> Result<ValidationResult, String> {
    let settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;

    let errors = settings.validate();

    // Check for external tools next to the executable
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let ffmpeg_found = exe_dir.join("ffmpeg.exe").exists()
        || exe_dir.join("ffmpeg").exists();
    let ytdlp_found = exe_dir.join("yt-dlp.exe").exists()
        || exe_dir.join("yt-dlp").exists();

    let resolved_lib = settings.resolved_library_path();
    let library_exists = resolved_lib.exists();

    let config_path = ConfigState::resolve_config_path();

    Ok(ValidationResult {
        valid: errors.is_empty() && ffmpeg_found && ytdlp_found,
        errors,
        ffmpeg_found,
        ytdlp_found,
        resolved_library_path: resolved_lib.to_string_lossy().to_string(),
        library_exists,
        config_path: config_path.to_string_lossy().to_string(),
    })
}

/// Tauri 커맨드: 모든 설정을 기본값으로 초기화하고 저장한다.
#[tauri::command]
pub fn reset_settings(
    state: tauri::State<'_, SettingsState>,
) -> Result<Settings, String> {
    let mut settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;
    *settings = Settings::default();
    save_settings(&settings)?;
    Ok(settings.clone())
}

/// Tauri 커맨드: config.json의 파일 시스템 경로를 반환한다.
///
/// 프론트엔드에서 설정 파일 위치를 표시할 때 사용한다.
#[tauri::command]
pub fn get_config_path() -> String {
    ConfigState::resolve_config_path()
        .to_string_lossy()
        .to_string()
}
