//! Settings module — wraps config types and provides Tauri commands.
//!
//! The canonical data model lives in `crate::config`. This module provides
//! the Tauri-facing API (`SettingsState`, `get_settings`, `update_settings`,
//! `validate_settings`, `reset_settings`, `get_config_path`).

pub use crate::config::{AppConfig, Language, VALID_CAPTURE_MODES, VALID_QUALITIES};

use crate::config::ConfigState;
use serde::Serialize;
use std::sync::Mutex;

/// Alias used throughout the Tauri integration layer.
pub type Settings = AppConfig;

/// Thread-safe state wrapper for Tauri managed state.
pub struct SettingsState(pub Mutex<Settings>);

/// Load settings from the portable config path (beside executable).
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

/// Persist settings to the portable config path.
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

/// Tauri command: return current settings to the frontend.
#[tauri::command]
pub fn get_settings(state: tauri::State<'_, SettingsState>) -> Result<Settings, String> {
    let settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;
    Ok(settings.clone())
}

/// Tauri command: partially update settings and persist to disk.
///
/// Accepts a JSON object with any subset of `Settings` fields.
/// Only provided fields are overwritten; others keep their current values.
/// Validates all fields before persisting.
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

/// Result of settings validation — includes field-level details.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    /// Overall validity
    pub valid: bool,
    /// List of error messages (empty if valid)
    pub errors: Vec<String>,
    /// Whether ffmpeg.exe is found next to the executable
    pub ffmpeg_found: bool,
    /// Whether yt-dlp.exe is found next to the executable
    pub ytdlp_found: bool,
    /// Resolved absolute path of the library directory
    pub resolved_library_path: String,
    /// Whether the library directory exists
    pub library_exists: bool,
    /// Path where config.json is stored
    pub config_path: String,
}

/// Tauri command: validate current settings and check external tool availability.
///
/// Returns a `ValidationResult` with field errors, tool presence, and path info.
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

/// Tauri command: reset all settings to defaults and persist.
#[tauri::command]
pub fn reset_settings(
    state: tauri::State<'_, SettingsState>,
) -> Result<Settings, String> {
    let mut settings = state.0.lock().map_err(|e| format!("Lock error: {e}"))?;
    *settings = Settings::default();
    save_settings(&settings)?;
    Ok(settings.clone())
}

/// Tauri command: return the filesystem path to config.json.
///
/// Useful for the frontend to display where settings are stored.
#[tauri::command]
pub fn get_config_path() -> String {
    ConfigState::resolve_config_path()
        .to_string_lossy()
        .to_string()
}
