use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ko,
    En,
}

impl Default for Language {
    fn default() -> Self {
        Language::Ko
    }
}

/// Allowed download quality values.
pub const VALID_QUALITIES: &[&str] = &["360", "480", "720", "1080", "1440", "2160", "best"];

/// Allowed capture mode values.
pub const VALID_CAPTURE_MODES: &[&str] = &["subtitle", "scene", "interval"];

/// Application settings persisted as config.json next to the executable.
///
/// Fields:
/// - `library_path` — root directory for downloads/output (default: `./library/`)
/// - `download_quality` — YouTube download quality (default: `"720"`)
/// - `language` — UI language: Korean (`ko`) or English (`en`)
/// - `mp4_retention` — keep source mp4 after frame extraction (default: `false`)
/// - `default_capture_mode` — default capture strategy (default: `"subtitle"`)
/// - `default_interval_seconds` — default interval for fixed-interval mode (default: `30`)
/// - `scene_change_threshold` — scene-change sensitivity 0.0–1.0 (default: `0.30`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub library_path: String,
    pub download_quality: String,
    pub language: Language,
    #[serde(default)]
    pub mp4_retention: bool,
    #[serde(default = "default_capture_mode")]
    pub default_capture_mode: String,
    #[serde(default = "default_interval_seconds")]
    pub default_interval_seconds: u32,
    #[serde(default = "default_scene_threshold")]
    pub scene_change_threshold: f64,
}

fn default_capture_mode() -> String {
    "subtitle".to_string()
}

fn default_interval_seconds() -> u32 {
    30
}

fn default_scene_threshold() -> f64 {
    0.30
}

impl AppConfig {
    /// Resolve `library_path` to an absolute path relative to the executable.
    pub fn resolved_library_path(&self) -> std::path::PathBuf {
        ConfigState::resolved_library_path(&self.library_path)
    }

    /// Validate all fields, returning a list of error messages.
    /// Returns an empty vec if everything is valid.
    pub fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        // Validate download quality
        if !VALID_QUALITIES.contains(&self.download_quality.as_str()) {
            errors.push(format!(
                "Invalid download_quality '{}'. Must be one of: {}",
                self.download_quality,
                VALID_QUALITIES.join(", ")
            ));
        }

        // Validate library_path is not empty
        if self.library_path.trim().is_empty() {
            errors.push("library_path cannot be empty".to_string());
        }

        // Validate library_path doesn't contain obviously invalid characters
        // (Windows-specific forbidden chars in path components)
        let forbidden = ['<', '>', '"', '|', '?', '*'];
        if self.library_path.chars().any(|c| forbidden.contains(&c)) {
            errors.push(format!(
                "library_path contains invalid characters: {}",
                self.library_path
            ));
        }

        // Validate capture mode
        if !VALID_CAPTURE_MODES.contains(&self.default_capture_mode.as_str()) {
            errors.push(format!(
                "Invalid default_capture_mode '{}'. Must be one of: {}",
                self.default_capture_mode,
                VALID_CAPTURE_MODES.join(", ")
            ));
        }

        // Validate interval seconds (1–3600)
        if self.default_interval_seconds == 0 || self.default_interval_seconds > 3600 {
            errors.push(format!(
                "default_interval_seconds must be 1–3600, got {}",
                self.default_interval_seconds
            ));
        }

        // Validate scene threshold (0.01–1.0)
        if self.scene_change_threshold < 0.01 || self.scene_change_threshold > 1.0 {
            errors.push(format!(
                "scene_change_threshold must be 0.01–1.0, got {}",
                self.scene_change_threshold
            ));
        }

        errors
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            library_path: "./library/".to_string(),
            download_quality: "720".to_string(),
            language: Language::default(), // Korean
            mp4_retention: false,
            default_capture_mode: default_capture_mode(),
            default_interval_seconds: default_interval_seconds(),
            scene_change_threshold: default_scene_threshold(),
        }
    }
}

/// Thread-safe wrapper for Tauri managed state.
pub struct ConfigState {
    pub config: Mutex<AppConfig>,
    pub config_path: PathBuf,
}

impl ConfigState {
    /// Create state by loading from the portable config path (next to exe).
    pub fn new() -> Self {
        let config_path = Self::resolve_config_path();
        let config = Self::load_from_path(&config_path);
        Self {
            config: Mutex::new(config),
            config_path,
        }
    }

    /// Returns the path to `config.json` beside the running executable.
    /// Falls back to `./config.json` if the exe path cannot be determined.
    pub fn resolve_config_path() -> PathBuf {
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                return exe_dir.join("config.json");
            }
        }
        PathBuf::from("config.json")
    }

    /// Load config from a specific path, returning defaults if missing/invalid.
    fn load_from_path(path: &PathBuf) -> AppConfig {
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => AppConfig::default(),
            }
        } else {
            AppConfig::default()
        }
    }

    /// Persist current config to disk.
    pub fn save(&self) -> Result<(), String> {
        let config = self.config.lock().map_err(|e| e.to_string())?;
        let json =
            serde_json::to_string_pretty(&*config).map_err(|e| e.to_string())?;
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        fs::write(&self.config_path, json)
            .map_err(|e| format!("Failed to write config: {e}"))?;
        Ok(())
    }

    /// Resolve `library_path` to an absolute path relative to the executable
    /// directory when it is relative.
    pub fn resolved_library_path(library_path: &str) -> PathBuf {
        let p = PathBuf::from(library_path);
        if p.is_absolute() {
            p
        } else {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|d| d.to_path_buf()))
                .unwrap_or_else(|| PathBuf::from("."))
                .join(&p)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.library_path, "./library/");
        assert_eq!(cfg.download_quality, "720");
        assert_eq!(cfg.language, Language::Ko);
        assert!(!cfg.mp4_retention);
    }

    #[test]
    fn round_trip_json() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let loaded: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.library_path, cfg.library_path);
        assert_eq!(loaded.download_quality, cfg.download_quality);
        assert_eq!(loaded.language, cfg.language);
        assert_eq!(loaded.mp4_retention, cfg.mp4_retention);
    }

    #[test]
    fn deserialize_language_variants() {
        let ko: Language = serde_json::from_str(r#""ko""#).unwrap();
        assert_eq!(ko, Language::Ko);
        let en: Language = serde_json::from_str(r#""en""#).unwrap();
        assert_eq!(en, Language::En);
    }

    #[test]
    fn serialize_language() {
        assert_eq!(serde_json::to_string(&Language::Ko).unwrap(), r#""ko""#);
        assert_eq!(serde_json::to_string(&Language::En).unwrap(), r#""en""#);
    }

    #[test]
    fn backward_compat_string_language() {
        // Old configs may have language as a plain string — they should still work
        // because serde rename_all = lowercase maps Ko <-> "ko"
        let json = r#"{"library_path":"./lib/","download_quality":"1080","language":"en","mp4_retention":true}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.language, Language::En);
        assert!(cfg.mp4_retention);
        assert_eq!(cfg.download_quality, "1080");
    }

    #[test]
    fn mp4_retention_defaults_false_when_missing() {
        let json = r#"{"library_path":"./library/","download_quality":"720","language":"ko"}"#;
        let cfg: AppConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.mp4_retention);
    }

    #[test]
    fn save_and_reload_from_disk() {
        let dir = std::env::temp_dir().join("framepick_cfg_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");

        let state = ConfigState {
            config: Mutex::new(AppConfig {
                library_path: "./custom/".to_string(),
                download_quality: "1080".to_string(),
                language: Language::En,
                mp4_retention: true,
                ..AppConfig::default()
            }),
            config_path: path.clone(),
        };
        state.save().unwrap();

        let content = fs::read_to_string(&path).unwrap();
        let loaded: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(loaded.library_path, "./custom/");
        assert_eq!(loaded.language, Language::En);
        assert!(loaded.mp4_retention);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn resolved_library_path_relative() {
        let resolved = ConfigState::resolved_library_path("./library/");
        // Should be absolute (joined with exe dir or ".")
        assert!(resolved.is_absolute() || resolved.starts_with("."));
    }

    #[test]
    fn validate_default_config_passes() {
        let cfg = AppConfig::default();
        let errors = cfg.validate();
        assert!(errors.is_empty(), "Default config should be valid: {:?}", errors);
    }

    #[test]
    fn validate_invalid_quality() {
        let cfg = AppConfig {
            download_quality: "999".to_string(),
            ..AppConfig::default()
        };
        let errors = cfg.validate();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("download_quality"));
    }

    #[test]
    fn validate_empty_library_path() {
        let cfg = AppConfig {
            library_path: "".to_string(),
            ..AppConfig::default()
        };
        let errors = cfg.validate();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("library_path"));
    }

    #[test]
    fn validate_forbidden_chars_in_path() {
        let cfg = AppConfig {
            library_path: "C:\\my<lib>".to_string(),
            ..AppConfig::default()
        };
        let errors = cfg.validate();
        assert!(!errors.is_empty());
        assert!(errors[0].contains("invalid characters"));
    }

    #[test]
    fn validate_all_quality_values() {
        for q in VALID_QUALITIES {
            let cfg = AppConfig {
                download_quality: q.to_string(),
                ..AppConfig::default()
            };
            assert!(cfg.validate().is_empty(), "Quality '{}' should be valid", q);
        }
    }

    #[test]
    fn validate_best_quality() {
        let cfg = AppConfig {
            download_quality: "best".to_string(),
            ..AppConfig::default()
        };
        assert!(cfg.validate().is_empty());
    }
}
