//! YouTube 영상 자막 가용 여부 감지 모듈.
//!
//! yt-dlp로 YouTube 영상에 다운로드 가능한 자막(수동 또는 자동 생성)이
//! 있는지 확인하고 결과를 프론트엔드에 반환한다.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

use crate::cmd_util::HideWindow;

/// 자막 가용 여부 확인 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleCheckResult {
    /// 수동/자동 자막이 하나라도 있는지 여부.
    pub has_subtitles: bool,
    /// 수동(사람이 작성한) 자막 존재 여부.
    pub has_manual_subtitles: bool,
    /// 자동 생성 자막 존재 여부.
    pub has_auto_subtitles: bool,
    /// 사용 가능한 수동 자막 언어 코드 목록 (예: ["en", "ko"]).
    pub manual_languages: Vec<String>,
    /// 사용 가능한 자동 생성 자막 언어 코드 목록.
    pub auto_languages: Vec<String>,
    /// 확인 실패 시 오류 메시지 (성공 시 빈 문자열).
    pub error: String,
}

impl SubtitleCheckResult {
    fn success(
        has_manual: bool,
        has_auto: bool,
        manual_langs: Vec<String>,
        auto_langs: Vec<String>,
    ) -> Self {
        Self {
            has_subtitles: has_manual || has_auto,
            has_manual_subtitles: has_manual,
            has_auto_subtitles: has_auto,
            manual_languages: manual_langs,
            auto_languages: auto_langs,
            error: String::new(),
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: Vec::new(),
            auto_languages: Vec::new(),
            error: msg.into(),
        }
    }
}

/// tools_manager를 통해 yt-dlp 실행 파일 경로를 반환한다.
pub fn resolve_ytdlp_path() -> PathBuf {
    crate::tools_manager::resolve_ytdlp_path()
}

/// yt-dlp로 YouTube 영상의 자막 가용 여부를 확인한다.
///
/// `yt-dlp --list-subs --skip-download <url>`을 실행하고 출력을 파싱해
/// 사용 가능한 자막을 결정한다.
pub fn check_subtitles(video_url: &str) -> SubtitleCheckResult {
    let ytdlp_path = resolve_ytdlp_path();

    let output = match Command::new(&ytdlp_path)
        .args(["--list-subs", "--skip-download", video_url])
        .hide_window()
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            return SubtitleCheckResult::error(format!(
                "Failed to run yt-dlp ({}): {}",
                ytdlp_path.display(),
                e
            ));
        }
    };

    // yt-dlp outputs subtitle info to stdout; some info may be in stderr
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for fatal errors
    if !output.status.success() && stdout.is_empty() {
        return SubtitleCheckResult::error(format!(
            "yt-dlp exited with error: {}",
            stderr.trim()
        ));
    }

    parse_subtitle_output(&stdout)
}

/// yt-dlp --list-subs 출력을 파싱해 자막 정보를 추출한다.
///
/// 출력 형식은 다음과 같은 섹션을 포함한다:
/// ```text
/// [info] Available subtitles for VIDEO_ID:
/// Language   Name         Formats
/// ko         Korean       vtt, ttml, srv3, srv2, srv1, json3
/// en         English      vtt, ttml, srv3, srv2, srv1, json3
///
/// [info] Available automatic captions for VIDEO_ID:
/// Language        Name                     Formats
/// af              Afrikaans                vtt, ttml, srv3, srv2, srv1, json3
/// ...
/// ```
///
/// 자막이 없는 경우:
/// ```text
/// [info] No subtitles available for VIDEO_ID
/// ```
pub fn parse_subtitle_output(output: &str) -> SubtitleCheckResult {
    let mut manual_languages = Vec::new();
    let mut auto_languages = Vec::new();

    // Track which section we're in
    #[derive(PartialEq)]
    enum Section {
        None,
        Manual,
        Auto,
    }
    let mut section = Section::None;
    let mut skip_header = false;

    for line in output.lines() {
        let trimmed = line.trim();

        // Detect section headers
        if trimmed.contains("Available subtitles for") {
            section = Section::Manual;
            skip_header = true;
            continue;
        }
        if trimmed.contains("Available automatic captions for") {
            section = Section::Auto;
            skip_header = true;
            continue;
        }

        // "No subtitles" message resets
        if trimmed.contains("no subtitles")
            || trimmed.contains("No subtitles")
        {
            // This is fine — there may still be auto captions
            continue;
        }

        // Skip the column header row ("Language  Name  Formats")
        if skip_header {
            if trimmed.starts_with("Language") || trimmed.starts_with("---") || trimmed.is_empty()
            {
                if trimmed.starts_with("Language") {
                    skip_header = false;
                }
                continue;
            }
        }

        // Empty line ends current section
        if trimmed.is_empty() {
            section = Section::None;
            continue;
        }

        // New [info] line starts a new context
        if trimmed.starts_with("[info]") || trimmed.starts_with("[") {
            if section != Section::None
                && !trimmed.contains("Available subtitles")
                && !trimmed.contains("Available automatic captions")
            {
                section = Section::None;
            }
            continue;
        }

        // Parse language code from the first column
        if section != Section::None {
            if let Some(lang_code) = extract_language_code(trimmed) {
                match section {
                    Section::Manual => {
                        if !manual_languages.contains(&lang_code) {
                            manual_languages.push(lang_code);
                        }
                    }
                    Section::Auto => {
                        if !auto_languages.contains(&lang_code) {
                            auto_languages.push(lang_code);
                        }
                    }
                    Section::None => {}
                }
            }
        }
    }

    let has_manual = !manual_languages.is_empty();
    let has_auto = !auto_languages.is_empty();

    SubtitleCheckResult::success(has_manual, has_auto, manual_languages, auto_languages)
}

/// 자막 목록 줄에서 언어 코드를 추출한다.
///
/// 줄 형식: `ko         Korean       vtt, ttml, srv3, srv2, srv1, json3`
/// 언어 코드는 공백으로 구분된 첫 번째 토큰이다.
fn extract_language_code(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // First token is the language code
    let code = trimmed.split_whitespace().next()?;

    // Basic validation: language codes are typically 2-10 chars, lowercase alphanumeric with hyphens
    if code.len() >= 2
        && code.len() <= 20
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(code.to_string())
    } else {
        None
    }
}

/// Tauri 커맨드: YouTube 영상의 자막 존재 여부를 확인한다.
///
/// 영상 URL(또는 영상 ID)을 받아 자막 가용 정보를 반환한다.
/// UI 스레드 블로킹을 방지하기 위해 비동기 커맨드로 구현한다.
#[tauri::command]
pub async fn check_subtitle_availability(url: String) -> Result<SubtitleCheckResult, String> {
    // Run the blocking yt-dlp call on a background thread
    let result =
        tauri::async_runtime::spawn_blocking(move || check_subtitles(&url))
            .await
            .map_err(|e| format!("Task join error: {e}"))?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_no_subtitles() {
        let output = "[info] VIDEO_ID: No subtitles available\n";
        let result = parse_subtitle_output(output);
        assert!(!result.has_subtitles);
        assert!(!result.has_manual_subtitles);
        assert!(!result.has_auto_subtitles);
        assert!(result.manual_languages.is_empty());
        assert!(result.auto_languages.is_empty());
        assert!(result.error.is_empty());
    }

    #[test]
    fn test_parse_manual_subtitles_only() {
        let output = "\
[info] Available subtitles for dQw4w9WgXcQ:
Language   Name         Formats
ko         Korean       vtt, ttml, srv3, srv2, srv1, json3
en         English      vtt, ttml, srv3, srv2, srv1, json3
";
        let result = parse_subtitle_output(output);
        assert!(result.has_subtitles);
        assert!(result.has_manual_subtitles);
        assert!(!result.has_auto_subtitles);
        assert_eq!(result.manual_languages, vec!["ko", "en"]);
        assert!(result.auto_languages.is_empty());
    }

    #[test]
    fn test_parse_auto_subtitles_only() {
        let output = "\
[info] Available automatic captions for dQw4w9WgXcQ:
Language        Name                     Formats
af              Afrikaans                vtt, ttml, srv3, srv2, srv1, json3
ko              Korean                   vtt, ttml, srv3, srv2, srv1, json3
en              English                  vtt, ttml, srv3, srv2, srv1, json3
";
        let result = parse_subtitle_output(output);
        assert!(result.has_subtitles);
        assert!(!result.has_manual_subtitles);
        assert!(result.has_auto_subtitles);
        assert!(result.manual_languages.is_empty());
        assert_eq!(result.auto_languages, vec!["af", "ko", "en"]);
    }

    #[test]
    fn test_parse_both_manual_and_auto() {
        let output = "\
[info] Available subtitles for dQw4w9WgXcQ:
Language   Name         Formats
ko         Korean       vtt, ttml, srv3, srv2, srv1, json3
en         English      vtt, ttml, srv3, srv2, srv1, json3

[info] Available automatic captions for dQw4w9WgXcQ:
Language        Name                     Formats
af              Afrikaans                vtt, ttml, srv3, srv2, srv1, json3
ja              Japanese                 vtt, ttml, srv3, srv2, srv1, json3
";
        let result = parse_subtitle_output(output);
        assert!(result.has_subtitles);
        assert!(result.has_manual_subtitles);
        assert!(result.has_auto_subtitles);
        assert_eq!(result.manual_languages, vec!["ko", "en"]);
        assert_eq!(result.auto_languages, vec!["af", "ja"]);
    }

    #[test]
    fn test_parse_empty_output() {
        let result = parse_subtitle_output("");
        assert!(!result.has_subtitles);
        assert!(result.manual_languages.is_empty());
        assert!(result.auto_languages.is_empty());
    }

    #[test]
    fn test_extract_language_code_valid() {
        assert_eq!(
            extract_language_code("ko         Korean       vtt, ttml"),
            Some("ko".to_string())
        );
        assert_eq!(
            extract_language_code("en-US      English (US) vtt"),
            Some("en-US".to_string())
        );
        assert_eq!(
            extract_language_code("zh-Hans    Chinese      vtt"),
            Some("zh-Hans".to_string())
        );
    }

    #[test]
    fn test_extract_language_code_invalid() {
        assert_eq!(extract_language_code(""), None);
        assert_eq!(extract_language_code("   "), None);
        // Single char is too short
        assert_eq!(extract_language_code("x  Something"), None);
    }

    #[test]
    fn test_error_result() {
        let result = SubtitleCheckResult::error("test error");
        assert!(!result.has_subtitles);
        assert_eq!(result.error, "test error");
    }

    #[test]
    fn test_success_result() {
        let result = SubtitleCheckResult::success(
            true,
            false,
            vec!["ko".to_string()],
            vec![],
        );
        assert!(result.has_subtitles);
        assert!(result.has_manual_subtitles);
        assert!(!result.has_auto_subtitles);
        assert_eq!(result.manual_languages, vec!["ko"]);
    }

    #[test]
    fn test_resolve_ytdlp_path_fallback() {
        // Should at least return a path (won't crash)
        let path = resolve_ytdlp_path();
        assert!(!path.to_string_lossy().is_empty());
    }

    #[test]
    fn test_parse_with_hyphenated_lang_codes() {
        let output = "\
[info] Available subtitles for test123:
Language   Name              Formats
zh-Hans    Chinese (Simp.)   vtt, ttml, srv3
pt-BR      Portuguese (BR)   vtt, ttml, srv3
";
        let result = parse_subtitle_output(output);
        assert!(result.has_manual_subtitles);
        assert_eq!(result.manual_languages, vec!["zh-Hans", "pt-BR"]);
    }

    #[test]
    fn test_parse_real_world_no_manual_with_auto() {
        // Common case: video has no manual subs but has auto-generated
        let output = "\
[info] dQw4w9WgXcQ: Downloading webpage
[info] dQw4w9WgXcQ: No subtitles available
[info] Available automatic captions for dQw4w9WgXcQ:
Language        Name                     Formats
en              English                  vtt, ttml, srv3, srv2, srv1, json3
ko              Korean                   vtt, ttml, srv3, srv2, srv1, json3
";
        let result = parse_subtitle_output(output);
        assert!(result.has_subtitles);
        assert!(!result.has_manual_subtitles);
        assert!(result.has_auto_subtitles);
        assert!(result.manual_languages.is_empty());
        assert_eq!(result.auto_languages, vec!["en", "ko"]);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let result = SubtitleCheckResult::success(
            true,
            true,
            vec!["ko".to_string(), "en".to_string()],
            vec!["ja".to_string()],
        );
        let json = serde_json::to_string(&result).unwrap();
        let loaded: SubtitleCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.has_subtitles, result.has_subtitles);
        assert_eq!(loaded.manual_languages, result.manual_languages);
        assert_eq!(loaded.auto_languages, result.auto_languages);
    }
}
