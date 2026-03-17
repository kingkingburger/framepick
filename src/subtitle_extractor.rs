//! YouTube 영상 자막 추출 모듈.
//!
//! yt-dlp로 언어 우선순위(한국어 우선, 영어 폴백)에 따라 자막을 다운로드하고,
//! SRT/VTT 자막 파일을 파싱해 자막 기반 프레임 캡쳐용 타임스탬프를 추출한다.
//!
//! 언어 우선순위:
//! 1. 한국어(`ko`) — 수동 자막을 자동 생성 자막보다 우선
//! 2. 영어(`en`) — 수동 자막을 자동 생성 자막보다 우선
//! 3. 기타 가용 언어 — 최후 폴백

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cmd_util::HideWindow;
use crate::subtitle_detector::{resolve_ytdlp_path, SubtitleCheckResult};

/// 기본 언어 우선순위: 한국어 우선, 그 다음 영어.
pub const LANGUAGE_PRIORITY: &[&str] = &["ko", "en"];

/// 자막 언어 선택 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleLanguageSelection {
    /// 선택된 언어 코드 (예: "ko", "en").
    pub language: String,
    /// 수동(사람이 작성) 자막 여부.
    pub is_manual: bool,
    /// 선호 언어인지 폴백인지 여부.
    pub is_preferred: bool,
    /// 로깅용 선택 결과 설명.
    pub description: String,
    /// 선택 결과를 설명하는 i18n 키.
    pub i18n_key: String,
}

/// 시작/종료 타임스탬프가 있는 단일 파싱된 자막 큐.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleCue {
    /// 시작 시간(초).
    pub start_secs: f64,
    /// 종료 시간(초).
    pub end_secs: f64,
    /// 자막 텍스트 내용.
    pub text: String,
}

/// 자막 추출 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleExtractionResult {
    /// 추출된 언어.
    pub language: SubtitleLanguageSelection,
    /// 타임스탬프가 포함된 파싱된 자막 큐.
    pub cues: Vec<SubtitleCue>,
    /// 다운로드된 자막 파일 경로 (보존된 경우).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle_file: Option<String>,
    /// 추출 실패 시 오류 메시지 (성공 시 빈 문자열).
    pub error: String,
}

impl SubtitleExtractionResult {
    fn success(language: SubtitleLanguageSelection, cues: Vec<SubtitleCue>, file: Option<String>) -> Self {
        Self {
            language,
            cues,
            subtitle_file: file,
            error: String::new(),
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            language: SubtitleLanguageSelection {
                language: String::new(),
                is_manual: false,
                is_preferred: false,
                description: String::new(),
                i18n_key: String::new(),
            },
            cues: Vec::new(),
            subtitle_file: None,
            error: msg.into(),
        }
    }
}

/// 가용 옵션 중 최적의 자막 언어를 선택한다.
///
/// 우선순위 로직:
/// 1. 한국어 수동 자막 (최상위)
/// 2. 한국어 자동 생성 자막
/// 3. 영어 수동 자막
/// 4. 영어 자동 생성 자막
/// 5. 임의 언어의 첫 번째 수동 자막
/// 6. 임의 언어의 첫 번째 자동 생성 자막
/// 7. None — 자막 없음
///
/// 수동 자막은 프레임 캡쳐 타이밍 정확도가 높아 자동 생성 자막보다 우선된다.
pub fn select_best_subtitle_language(
    check: &SubtitleCheckResult,
) -> Option<SubtitleLanguageSelection> {
    // If no subtitles at all, return None
    if !check.has_subtitles {
        return None;
    }

    // Try each priority language in order: ko, en
    for (priority_idx, &lang) in LANGUAGE_PRIORITY.iter().enumerate() {
        let is_preferred = priority_idx == 0; // ko is preferred

        // Check manual subtitles first (better quality)
        if check.manual_languages.iter().any(|l| language_matches(l, lang)) {
            let matched_lang = check
                .manual_languages
                .iter()
                .find(|l| language_matches(l, lang))
                .unwrap()
                .clone();
            return Some(SubtitleLanguageSelection {
                language: matched_lang.clone(),
                is_manual: true,
                is_preferred,
                description: format!(
                    "Manual {} subtitles ({})",
                    lang_display_name(lang),
                    matched_lang
                ),
                i18n_key: if is_preferred {
                    "subtitle_lang_korean_manual".to_string()
                } else {
                    "subtitle_lang_english_manual".to_string()
                },
            });
        }

        // Then check auto-generated
        if check.auto_languages.iter().any(|l| language_matches(l, lang)) {
            let matched_lang = check
                .auto_languages
                .iter()
                .find(|l| language_matches(l, lang))
                .unwrap()
                .clone();
            return Some(SubtitleLanguageSelection {
                language: matched_lang.clone(),
                is_manual: false,
                is_preferred,
                description: format!(
                    "Auto-generated {} subtitles ({})",
                    lang_display_name(lang),
                    matched_lang
                ),
                i18n_key: if is_preferred {
                    "subtitle_lang_korean_auto".to_string()
                } else {
                    "subtitle_lang_english_auto".to_string()
                },
            });
        }
    }

    // Fallback: try any available manual subtitle
    if let Some(lang) = check.manual_languages.first() {
        return Some(SubtitleLanguageSelection {
            language: lang.clone(),
            is_manual: true,
            is_preferred: false,
            description: format!("Manual subtitles ({})", lang),
            i18n_key: "subtitle_lang_other_manual".to_string(),
        });
    }

    // Fallback: try any available auto-generated subtitle
    if let Some(lang) = check.auto_languages.first() {
        return Some(SubtitleLanguageSelection {
            language: lang.clone(),
            is_manual: false,
            is_preferred: false,
            description: format!("Auto-generated subtitles ({})", lang),
            i18n_key: "subtitle_lang_other_auto".to_string(),
        });
    }

    None
}

/// 언어 코드가 대상 언어와 일치하는지 확인한다.
///
/// "ko", "ko-KR", "en", "en-US", "en-GB" 등의 변형을 처리한다.
fn language_matches(code: &str, target: &str) -> bool {
    code == target || code.starts_with(&format!("{}-", target))
}

/// 언어 코드의 표시 이름을 반환한다.
fn lang_display_name(code: &str) -> &str {
    match code {
        "ko" => "Korean",
        "en" => "English",
        "ja" => "Japanese",
        "zh" => "Chinese",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        "pt" => "Portuguese",
        _ => code,
    }
}

/// yt-dlp로 YouTube 영상의 자막을 다운로드한다.
///
/// 언어 선택 결과에 따라 다운로드할 자막 트랙을 결정하고
/// 파싱이 쉬운 SRT 형식으로 다운로드한다.
///
/// 다운로드된 자막 파일 경로를 반환한다.
pub fn download_subtitles(
    video_url: &str,
    output_dir: &Path,
    selection: &SubtitleLanguageSelection,
) -> Result<PathBuf, String> {
    let ytdlp = resolve_ytdlp_path();

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {e}"))?;

    // Build yt-dlp command for subtitle download
    let sub_lang = &selection.language;
    let output_template = output_dir.join("%(id)s.%(ext)s");

    let mut args = vec![
        "--skip-download".to_string(),
        "--convert-subs".to_string(),
        "srt".to_string(),
        "--sub-lang".to_string(),
        sub_lang.clone(),
        "-o".to_string(),
        output_template.to_string_lossy().to_string(),
    ];

    // Use --write-subs for manual, --write-auto-subs for auto-generated
    if selection.is_manual {
        args.push("--write-subs".to_string());
    } else {
        args.push("--write-auto-subs".to_string());
    }

    args.push(video_url.to_string());

    println!(
        "[subtitle_extractor] Downloading {} subtitles (lang={}, manual={}) for: {}",
        lang_display_name(sub_lang.split('-').next().unwrap_or(sub_lang)),
        sub_lang,
        selection.is_manual,
        video_url
    );

    let output = Command::new(&ytdlp)
        .args(&args)
        .hide_window()
        .output()
        .map_err(|e| format!("Failed to run yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp subtitle download failed: {}", stderr.trim()));
    }

    // Find the downloaded subtitle file
    find_subtitle_file(output_dir, sub_lang)
}

/// 출력 디렉토리에서 다운로드된 자막 파일을 찾는다.
///
/// yt-dlp는 `{video_id}.{lang}.srt` 등의 패턴으로 파일을 명명한다.
fn find_subtitle_file(dir: &Path, lang: &str) -> Result<PathBuf, String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read output dir: {e}"))?;

    // Look for .srt files matching the language
    let mut srt_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            if let Some(ext) = p.extension() {
                if ext == "srt" {
                    // Check if the filename contains the language code
                    let name = p.file_stem().unwrap_or_default().to_string_lossy();
                    return name.contains(lang) || name.ends_with(&format!(".{}", lang));
                }
            }
            false
        })
        .collect();

    // Sort by modification time (most recent first)
    srt_files.sort_by(|a, b| {
        let time_a = a.metadata().and_then(|m| m.modified()).ok();
        let time_b = b.metadata().and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    if let Some(file) = srt_files.first() {
        return Ok(file.clone());
    }

    // Broader search: any .srt file in the directory
    let any_srt: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read output dir: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "srt"))
        .collect();

    if let Some(file) = any_srt.first() {
        println!(
            "[subtitle_extractor] Warning: Could not find lang-specific file for '{}', using: {}",
            lang,
            file.display()
        );
        return Ok(file.clone());
    }

    // Try .vtt files as fallback
    let any_vtt: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read output dir: {e}"))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "vtt"))
        .collect();

    if let Some(file) = any_vtt.first() {
        println!(
            "[subtitle_extractor] Warning: No .srt found, using .vtt file: {}",
            file.display()
        );
        return Ok(file.clone());
    }

    Err(format!(
        "No subtitle file found in {} for language '{}'",
        dir.display(),
        lang
    ))
}

/// SRT 자막 파일을 자막 큐로 파싱한다.
///
/// SRT 형식:
/// ```text
/// 1
/// 00:00:01,000 --> 00:00:04,500
/// Subtitle text here
///
/// 2
/// 00:00:05,000 --> 00:00:08,200
/// Another subtitle line
/// ```
pub fn parse_srt_file(path: &Path) -> Result<Vec<SubtitleCue>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read subtitle file: {e}"))?;

    parse_srt(&content)
}

/// SRT 형식 자막 문자열을 큐로 파싱한다.
pub fn parse_srt(content: &str) -> Result<Vec<SubtitleCue>, String> {
    let mut cues = Vec::new();
    let mut lines = content.lines().peekable();

    while lines.peek().is_some() {
        // Skip blank lines and BOM
        while let Some(&line) = lines.peek() {
            let trimmed = line.trim().trim_start_matches('\u{feff}');
            if trimmed.is_empty() {
                lines.next();
            } else {
                break;
            }
        }

        // Skip cue number (integer line)
        if let Some(line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.chars().all(|c| c.is_ascii_digit()) {
                lines.next();
            }
        }

        // Parse timestamp line: "HH:MM:SS,mmm --> HH:MM:SS,mmm"
        let timestamp_line = match lines.next() {
            Some(line) => line.trim().to_string(),
            None => break,
        };

        if !timestamp_line.contains("-->") {
            continue; // Not a valid timestamp line, skip
        }

        let parts: Vec<&str> = timestamp_line.split("-->").collect();
        if parts.len() != 2 {
            continue;
        }

        let start = parse_srt_timestamp(parts[0].trim());
        let end = parse_srt_timestamp(parts[1].trim());

        let (start_secs, end_secs) = match (start, end) {
            (Some(s), Some(e)) => (s, e),
            _ => continue,
        };

        // Collect text lines until blank line or end
        let mut text_lines = Vec::new();
        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                lines.next();
                break;
            }
            text_lines.push(trimmed.to_string());
            lines.next();
        }

        let text = text_lines.join(" ");
        // Skip empty cues
        if !text.is_empty() {
            cues.push(SubtitleCue {
                start_secs,
                end_secs,
                text,
            });
        }
    }

    Ok(cues)
}

/// VTT/WebVTT 자막 파일을 큐로 파싱한다.
///
/// VTT는 SRT와 유사하지만 밀리초 구분자로 ','가 아닌 '.'을 사용하고
/// "WEBVTT" 헤더가 있을 수 있다.
pub fn parse_vtt_file(path: &Path) -> Result<Vec<SubtitleCue>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read subtitle file: {e}"))?;

    parse_vtt(&content)
}

/// VTT 형식 자막 문자열을 큐로 파싱한다.
pub fn parse_vtt(content: &str) -> Result<Vec<SubtitleCue>, String> {
    // Convert VTT to SRT-like format by replacing '.' with ',' in timestamps
    // and stripping the WEBVTT header
    let mut normalized = String::new();
    let mut past_header = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip WEBVTT header and metadata
        if !past_header {
            if trimmed.starts_with("WEBVTT") || trimmed.starts_with("Kind:") || trimmed.starts_with("Language:") {
                continue;
            }
            if trimmed.is_empty() {
                past_header = true;
                continue;
            }
            // If it looks like a cue, we're past the header
            if trimmed.contains("-->") {
                past_header = true;
            }
        }

        if past_header {
            // In timestamp lines, replace '.' with ',' for SRT compatibility
            if trimmed.contains("-->") {
                normalized.push_str(&trimmed.replace('.', ","));
            } else {
                // Strip VTT positioning tags like <c>, </c>, position:...
                let clean = strip_vtt_tags(trimmed);
                normalized.push_str(&clean);
            }
            normalized.push('\n');
        }
    }

    parse_srt(&normalized)
}

/// VTT 자막 텍스트에서 HTML 유사 태그를 제거한다.
fn strip_vtt_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;

    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }

    result
}

/// SRT 타임스탬프 문자열(예: "00:01:23,456")을 초 단위로 변환한다.
fn parse_srt_timestamp(ts: &str) -> Option<f64> {
    // Format: HH:MM:SS,mmm or HH:MM:SS.mmm
    let ts = ts.trim();
    let parts: Vec<&str> = ts.splitn(2, |c| c == ',' || c == '.').collect();

    let time_part = parts.first()?;
    let millis_part = parts.get(1).unwrap_or(&"0");

    let time_parts: Vec<&str> = time_part.split(':').collect();
    if time_parts.len() != 3 {
        return None;
    }

    let hours: f64 = time_parts[0].parse().ok()?;
    let minutes: f64 = time_parts[1].parse().ok()?;
    let seconds: f64 = time_parts[2].parse().ok()?;
    let millis: f64 = millis_part.parse::<f64>().ok().unwrap_or(0.0);

    Some(hours * 3600.0 + minutes * 60.0 + seconds + millis / 1000.0)
}

/// 프레임 캡쳐용 자막 타임스탬프를 추출한다.
///
/// 자막 큐에서 중복 제거된 시작 시간 목록을 반환한다.
/// 0.5초 이내의 인접한 큐는 병합된다.
pub fn extract_capture_timestamps(cues: &[SubtitleCue]) -> Vec<f64> {
    let mut timestamps: Vec<f64> = Vec::with_capacity(cues.len());

    for cue in cues {
        // Deduplicate: skip if within 0.5s of previous timestamp
        if timestamps
            .last()
            .map_or(true, |&prev| (cue.start_secs - prev).abs() > 0.5)
        {
            timestamps.push(cue.start_secs);
        }
    }

    timestamps
}

/// 자막 추출 전체 파이프라인: 언어 선택 → 다운로드 → 파싱.
///
/// 자막 기반 프레임 캡쳐의 주 진입점이다.
/// 전체 파이프라인을 수행한다:
/// 1. 기존 자막 확인 결과로 최적 언어 선택 (ko > en > 기타)
/// 2. yt-dlp로 자막 다운로드
/// 3. 다운로드된 자막 파일 파싱
/// 4. 타임스탬프 포함 큐 반환
pub fn extract_subtitles(
    video_url: &str,
    output_dir: &Path,
    check_result: &SubtitleCheckResult,
) -> SubtitleExtractionResult {
    // Step 1: Select best language
    let selection = match select_best_subtitle_language(check_result) {
        Some(sel) => sel,
        None => {
            return SubtitleExtractionResult::error(
                "No suitable subtitle language found",
            );
        }
    };

    println!(
        "[subtitle_extractor] Selected: {} (preferred={})",
        selection.description, selection.is_preferred
    );

    // Step 2: Download subtitles
    let subtitle_path = match download_subtitles(video_url, output_dir, &selection) {
        Ok(path) => path,
        Err(e) => {
            return SubtitleExtractionResult::error(format!(
                "Failed to download subtitles: {e}"
            ));
        }
    };

    println!(
        "[subtitle_extractor] Downloaded subtitle file: {}",
        subtitle_path.display()
    );

    // Step 3: Parse subtitle file
    let cues = if subtitle_path
        .extension()
        .map_or(false, |ext| ext == "vtt")
    {
        parse_vtt_file(&subtitle_path)
    } else {
        parse_srt_file(&subtitle_path)
    };

    match cues {
        Ok(cues) => {
            println!(
                "[subtitle_extractor] Parsed {} subtitle cues",
                cues.len()
            );
            SubtitleExtractionResult::success(
                selection,
                cues,
                Some(subtitle_path.to_string_lossy().to_string()),
            )
        }
        Err(e) => SubtitleExtractionResult::error(format!(
            "Failed to parse subtitle file: {e}"
        )),
    }
}

/// Tauri 커맨드: 영상 URL의 자막을 추출한다.
///
/// 자막 가용 여부를 확인하고, 최적 언어(한국어 우선)를 선택한 뒤
/// 자막 파일을 다운로드하여 파싱한다.
#[tauri::command]
pub async fn extract_subtitles_cmd(
    video_url: String,
    output_dir: String,
) -> Result<SubtitleExtractionResult, String> {
    let url = video_url.clone();
    let out = PathBuf::from(&output_dir);

    let result = tauri::async_runtime::spawn_blocking(move || {
        // First check what subtitles are available
        let check = crate::subtitle_detector::check_subtitles(&url);
        if !check.error.is_empty() {
            return SubtitleExtractionResult::error(format!(
                "Subtitle check failed: {}",
                check.error
            ));
        }

        extract_subtitles(&url, &out, &check)
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?;

    Ok(result)
}

/// Tauri 커맨드: 영상의 최적 자막 언어를 선택한다.
///
/// 자막 확인 결과를 입력받아 한국어 > 영어 우선 로직으로
/// 선택될 언어를 반환한다.
#[tauri::command]
pub async fn select_subtitle_language(
    check_result: SubtitleCheckResult,
) -> Result<Option<SubtitleLanguageSelection>, String> {
    Ok(select_best_subtitle_language(&check_result))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Language Selection Tests ─────────────────────────────────

    #[test]
    fn selects_korean_manual_first() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: true,
            manual_languages: vec!["ko".to_string(), "en".to_string()],
            auto_languages: vec!["ko".to_string(), "en".to_string()],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "ko");
        assert!(sel.is_manual);
        assert!(sel.is_preferred);
        assert_eq!(sel.i18n_key, "subtitle_lang_korean_manual");
    }

    #[test]
    fn selects_korean_auto_over_english_manual() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: true,
            manual_languages: vec!["en".to_string()],
            auto_languages: vec!["ko".to_string()],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "ko");
        assert!(!sel.is_manual); // auto-generated
        assert!(sel.is_preferred); // Korean is preferred
        assert_eq!(sel.i18n_key, "subtitle_lang_korean_auto");
    }

    #[test]
    fn falls_back_to_english_manual_when_no_korean() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: false,
            manual_languages: vec!["en".to_string(), "ja".to_string()],
            auto_languages: vec![],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "en");
        assert!(sel.is_manual);
        assert!(!sel.is_preferred); // Not Korean
        assert_eq!(sel.i18n_key, "subtitle_lang_english_manual");
    }

    #[test]
    fn falls_back_to_english_auto_when_no_korean_no_english_manual() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: false,
            has_auto_subtitles: true,
            manual_languages: vec![],
            auto_languages: vec!["en".to_string(), "ja".to_string()],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "en");
        assert!(!sel.is_manual);
        assert!(!sel.is_preferred);
        assert_eq!(sel.i18n_key, "subtitle_lang_english_auto");
    }

    #[test]
    fn falls_back_to_other_manual_when_no_ko_or_en() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: false,
            manual_languages: vec!["ja".to_string(), "fr".to_string()],
            auto_languages: vec![],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "ja");
        assert!(sel.is_manual);
        assert!(!sel.is_preferred);
        assert_eq!(sel.i18n_key, "subtitle_lang_other_manual");
    }

    #[test]
    fn falls_back_to_other_auto_when_no_ko_en_no_manual() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: false,
            has_auto_subtitles: true,
            manual_languages: vec![],
            auto_languages: vec!["ja".to_string()],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "ja");
        assert!(!sel.is_manual);
        assert!(!sel.is_preferred);
        assert_eq!(sel.i18n_key, "subtitle_lang_other_auto");
    }

    #[test]
    fn returns_none_when_no_subtitles() {
        let check = SubtitleCheckResult {
            has_subtitles: false,
            has_manual_subtitles: false,
            has_auto_subtitles: false,
            manual_languages: vec![],
            auto_languages: vec![],
            error: String::new(),
        };
        assert!(select_best_subtitle_language(&check).is_none());
    }

    #[test]
    fn handles_language_variants_ko_kr() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: false,
            manual_languages: vec!["ko-KR".to_string(), "en-US".to_string()],
            auto_languages: vec![],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "ko-KR");
        assert!(sel.is_manual);
        assert!(sel.is_preferred);
    }

    #[test]
    fn handles_language_variants_en_us() {
        let check = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: false,
            has_auto_subtitles: true,
            manual_languages: vec![],
            auto_languages: vec!["en-US".to_string(), "ja".to_string()],
            error: String::new(),
        };
        let sel = select_best_subtitle_language(&check).unwrap();
        assert_eq!(sel.language, "en-US");
        assert!(!sel.is_manual);
        assert!(!sel.is_preferred);
    }

    // ─── Language Matching Tests ──────────────────────────────────

    #[test]
    fn language_matches_exact() {
        assert!(language_matches("ko", "ko"));
        assert!(language_matches("en", "en"));
        assert!(!language_matches("ko", "en"));
    }

    #[test]
    fn language_matches_with_region() {
        assert!(language_matches("ko-KR", "ko"));
        assert!(language_matches("en-US", "en"));
        assert!(language_matches("en-GB", "en"));
        assert!(!language_matches("en-US", "ko"));
    }

    // ─── SRT Parsing Tests ───────────────────────────────────────

    #[test]
    fn parse_srt_basic() {
        let srt = "\
1
00:00:01,000 --> 00:00:04,500
Hello world

2
00:00:05,000 --> 00:00:08,200
Second subtitle
";
        let cues = parse_srt(srt).unwrap();
        assert_eq!(cues.len(), 2);
        assert!((cues[0].start_secs - 1.0).abs() < 0.01);
        assert!((cues[0].end_secs - 4.5).abs() < 0.01);
        assert_eq!(cues[0].text, "Hello world");
        assert!((cues[1].start_secs - 5.0).abs() < 0.01);
        assert_eq!(cues[1].text, "Second subtitle");
    }

    #[test]
    fn parse_srt_with_bom() {
        let srt = "\u{feff}1
00:00:01,000 --> 00:00:04,500
BOM test
";
        let cues = parse_srt(srt).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "BOM test");
    }

    #[test]
    fn parse_srt_multiline_text() {
        let srt = "\
1
00:00:01,000 --> 00:00:04,500
Line one
Line two

2
00:00:05,000 --> 00:00:08,200
Single line
";
        let cues = parse_srt(srt).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "Line one Line two");
    }

    #[test]
    fn parse_srt_korean_text() {
        let srt = "\
1
00:00:01,000 --> 00:00:04,500
안녕하세요

2
00:00:05,000 --> 00:00:08,200
감사합니다
";
        let cues = parse_srt(srt).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].text, "안녕하세요");
        assert_eq!(cues[1].text, "감사합니다");
    }

    #[test]
    fn parse_srt_empty() {
        let cues = parse_srt("").unwrap();
        assert!(cues.is_empty());
    }

    #[test]
    fn parse_srt_timestamp_only_seconds() {
        let srt = "\
1
00:00:00,000 --> 00:00:01,000
First

2
00:01:30,500 --> 00:01:35,000
Later
";
        let cues = parse_srt(srt).unwrap();
        assert!((cues[0].start_secs - 0.0).abs() < 0.01);
        assert!((cues[1].start_secs - 90.5).abs() < 0.01);
    }

    // ─── VTT Parsing Tests ───────────────────────────────────────

    #[test]
    fn parse_vtt_basic() {
        let vtt = "\
WEBVTT
Kind: captions
Language: ko

00:00:01.000 --> 00:00:04.500
안녕하세요

00:00:05.000 --> 00:00:08.200
감사합니다
";
        let cues = parse_vtt(vtt).unwrap();
        assert_eq!(cues.len(), 2);
        assert!((cues[0].start_secs - 1.0).abs() < 0.01);
        assert_eq!(cues[0].text, "안녕하세요");
    }

    #[test]
    fn parse_vtt_with_tags() {
        let vtt = "\
WEBVTT

00:00:01.000 --> 00:00:04.500
<c>Hello</c> <c>world</c>
";
        let cues = parse_vtt(vtt).unwrap();
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "Hello world");
    }

    // ─── Timestamp Extraction Tests ──────────────────────────────

    #[test]
    fn extract_timestamps_deduplicates() {
        let cues = vec![
            SubtitleCue { start_secs: 1.0, end_secs: 3.0, text: "A".to_string() },
            SubtitleCue { start_secs: 1.3, end_secs: 3.0, text: "B".to_string() },
            SubtitleCue { start_secs: 5.0, end_secs: 7.0, text: "C".to_string() },
            SubtitleCue { start_secs: 10.0, end_secs: 12.0, text: "D".to_string() },
        ];
        let timestamps = extract_capture_timestamps(&cues);
        // 1.3 should be deduplicated (within 0.5s of 1.0)
        assert_eq!(timestamps, vec![1.0, 5.0, 10.0]);
    }

    #[test]
    fn extract_timestamps_empty() {
        let timestamps = extract_capture_timestamps(&[]);
        assert!(timestamps.is_empty());
    }

    // ─── SRT Timestamp Parsing Tests ─────────────────────────────

    #[test]
    fn parse_srt_timestamp_valid() {
        assert!((parse_srt_timestamp("00:00:01,000").unwrap() - 1.0).abs() < 0.001);
        assert!((parse_srt_timestamp("00:01:30,500").unwrap() - 90.5).abs() < 0.001);
        assert!((parse_srt_timestamp("01:00:00,000").unwrap() - 3600.0).abs() < 0.001);
        assert!((parse_srt_timestamp("01:23:45,678").unwrap() - 5025.678).abs() < 0.001);
    }

    #[test]
    fn parse_srt_timestamp_with_dot() {
        // VTT-style with dot separator
        assert!((parse_srt_timestamp("00:00:01.000").unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn parse_srt_timestamp_invalid() {
        assert!(parse_srt_timestamp("invalid").is_none());
        assert!(parse_srt_timestamp("").is_none());
    }

    // ─── VTT Tag Stripping Tests ─────────────────────────────────

    #[test]
    fn strip_vtt_tags_basic() {
        assert_eq!(strip_vtt_tags("<c>Hello</c>"), "Hello");
        assert_eq!(strip_vtt_tags("No tags here"), "No tags here");
        assert_eq!(strip_vtt_tags("<b>Bold</b> and <i>italic</i>"), "Bold and italic");
    }

    // ─── Serialization Tests ─────────────────────────────────────

    #[test]
    fn subtitle_language_selection_serialization() {
        let sel = SubtitleLanguageSelection {
            language: "ko".to_string(),
            is_manual: true,
            is_preferred: true,
            description: "Manual Korean subtitles (ko)".to_string(),
            i18n_key: "subtitle_lang_korean_manual".to_string(),
        };
        let json = serde_json::to_string(&sel).unwrap();
        assert!(json.contains("\"language\":\"ko\""));
        assert!(json.contains("\"is_manual\":true"));
        assert!(json.contains("\"is_preferred\":true"));

        let loaded: SubtitleLanguageSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.language, "ko");
    }

    #[test]
    fn subtitle_cue_serialization() {
        let cue = SubtitleCue {
            start_secs: 1.5,
            end_secs: 4.0,
            text: "안녕하세요".to_string(),
        };
        let json = serde_json::to_string(&cue).unwrap();
        let loaded: SubtitleCue = serde_json::from_str(&json).unwrap();
        assert!((loaded.start_secs - 1.5).abs() < 0.001);
        assert_eq!(loaded.text, "안녕하세요");
    }

    #[test]
    fn extraction_result_error() {
        let result = SubtitleExtractionResult::error("test error");
        assert_eq!(result.error, "test error");
        assert!(result.cues.is_empty());
        assert!(result.subtitle_file.is_none());
    }

    #[test]
    fn extraction_result_success() {
        let sel = SubtitleLanguageSelection {
            language: "ko".to_string(),
            is_manual: true,
            is_preferred: true,
            description: "Korean manual".to_string(),
            i18n_key: "subtitle_lang_korean_manual".to_string(),
        };
        let cues = vec![SubtitleCue {
            start_secs: 1.0,
            end_secs: 3.0,
            text: "Test".to_string(),
        }];
        let result = SubtitleExtractionResult::success(sel, cues, Some("/tmp/test.srt".to_string()));
        assert!(result.error.is_empty());
        assert_eq!(result.cues.len(), 1);
        assert_eq!(result.language.language, "ko");
        assert_eq!(result.subtitle_file, Some("/tmp/test.srt".to_string()));
    }

    // ─── Priority Order Comprehensive Test ───────────────────────

    #[test]
    fn priority_order_comprehensive() {
        // Test the full priority chain:
        // ko manual > ko auto > en manual > en auto > other manual > other auto

        // 1. Only "other" auto → should pick that
        let check1 = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: false,
            has_auto_subtitles: true,
            manual_languages: vec![],
            auto_languages: vec!["fr".to_string()],
            error: String::new(),
        };
        let sel1 = select_best_subtitle_language(&check1).unwrap();
        assert_eq!(sel1.language, "fr");
        assert!(!sel1.is_manual);
        assert!(!sel1.is_preferred);

        // 2. "other" manual + "other" auto → should pick manual
        let check2 = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: true,
            manual_languages: vec!["fr".to_string()],
            auto_languages: vec!["de".to_string()],
            error: String::new(),
        };
        let sel2 = select_best_subtitle_language(&check2).unwrap();
        assert_eq!(sel2.language, "fr");
        assert!(sel2.is_manual);

        // 3. en auto + other manual → should pick en auto (en is higher priority)
        let check3 = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: true,
            manual_languages: vec!["fr".to_string()],
            auto_languages: vec!["en".to_string()],
            error: String::new(),
        };
        let sel3 = select_best_subtitle_language(&check3).unwrap();
        assert_eq!(sel3.language, "en");
        assert!(!sel3.is_manual);

        // 4. ko auto + en manual → should pick ko auto (ko is higher priority)
        let check4 = SubtitleCheckResult {
            has_subtitles: true,
            has_manual_subtitles: true,
            has_auto_subtitles: true,
            manual_languages: vec!["en".to_string()],
            auto_languages: vec!["ko".to_string()],
            error: String::new(),
        };
        let sel4 = select_best_subtitle_language(&check4).unwrap();
        assert_eq!(sel4.language, "ko");
        assert!(!sel4.is_manual);
        assert!(sel4.is_preferred);
    }

    #[test]
    fn language_priority_constant() {
        assert_eq!(LANGUAGE_PRIORITY, &["ko", "en"]);
    }
}
