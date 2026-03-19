//! yt-dlp를 사용한 YouTube 영상 다운로드 모듈.
//!
//! 화질 설정에 따라 포맷을 선택하여 YouTube 영상을 다운로드하고,
//! 자동 생성 자막(한국어 우선, 영어 폴백)을 json3 포맷으로 함께 다운로드한다.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cmd_util::HideWindow;
use crate::tools_manager::resolve_ytdlp_path;

/// 영상 다운로드 성공 결과.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// 다운로드된 MP4 파일 경로.
    pub mp4_path: PathBuf,
    /// 다운로드된 자막 파일 경로 (찾지 못하면 None).
    pub subtitle_path: Option<PathBuf>,
}

/// yt-dlp로 YouTube 영상을 다운로드한다.
///
/// `{output_dir}/source/`를 생성하고 그 안에 영상을 저장한다.
/// 포맷 선택은 `quality`에 따라 결정된다:
/// - `"best"` — `bestvideo+bestaudio/best`
/// - 그 외 (예: `"720"`) — `bestvideo[height<=N]+bestaudio/best[height<=N]`
///
/// 자막은 `--write-auto-sub --sub-lang ko,en --convert-subs json3`으로
/// 영상과 별도로 다운로드된다.
///
/// 성공 시 [`DownloadResult`](MP4 경로 및 선택적 자막 경로)를 반환한다.
/// yt-dlp 실패 또는 다운로드 후 MP4 파일을 찾지 못하면 `Err`를 반환한다.
pub fn download_video(
    url: &str,
    output_dir: &Path,
    video_id: &str,
    quality: &str,
) -> Result<DownloadResult, String> {
    if quality != "best" && quality.parse::<u32>().is_err() {
        return Err(format!("Invalid quality value: {}", quality));
    }

    let source_dir = output_dir.join("source");
    std::fs::create_dir_all(&source_dir)
        .map_err(|e| format!("Failed to create source directory: {e}"))?;

    let format = build_format_string(quality);
    let output_template = source_dir
        .join(format!("{video_id}.%(ext)s"))
        .to_string_lossy()
        .to_string();

    let ytdlp = resolve_ytdlp_path();

    // Step 1: Download video only (no subtitles — keeps this call robust)
    let output = Command::new(&ytdlp)
        .args([
            "-f",
            &format,
            "--merge-output-format",
            "mp4",
            "--no-playlist",
            "-o",
            &output_template,
            url,
        ])
        .hide_window()
        .output()
        .map_err(|e| {
            format!(
                "Failed to run yt-dlp ({}): {e}",
                ytdlp.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp exited with error: {}", stderr.trim()));
    }

    // Locate the downloaded MP4
    let mp4_path = source_dir.join(format!("{video_id}.mp4"));
    if !mp4_path.exists() {
        return Err(format!(
            "MP4 file not found after download: {}",
            mp4_path.display()
        ));
    }

    // Step 2: Try to download subtitles separately (non-fatal)
    let sub_output = Command::new(&ytdlp)
        .args([
            "--write-auto-sub",
            "--sub-lang",
            "ko,en",
            "--sub-format",
            "json3/best",
            "--skip-download",
            "--no-playlist",
            "-o",
            &output_template,
            url,
        ])
        .hide_window()
        .output();

    if let Ok(out) = &sub_output {
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("[downloader] Subtitle download failed (non-fatal): {}", stderr.trim());
        }
    }

    let subtitle_path = find_subtitle_file(&source_dir, video_id);

    Ok(DownloadResult {
        mp4_path,
        subtitle_path,
    })
}

/// 화질 문자열로 yt-dlp 포맷 선택 문자열을 생성한다.
///
/// `"best"`는 `bestvideo+bestaudio/best`를 반환한다.
/// `"720"` 같은 숫자 높이 문자열은
/// `bestvideo[height<=720]+bestaudio/best[height<=720]`를 반환한다.
fn build_format_string(quality: &str) -> String {
    if quality == "best" {
        "bestvideo+bestaudio/best".to_string()
    } else {
        format!(
            "bestvideo[height<={quality}]+bestaudio/best[height<={quality}]"
        )
    }
}

/// `source_dir`에서 `video_id`에 해당하는 json3 자막 파일을 찾는다.
///
/// 탐색 순서:
/// 1. `{video_id}.ko.json3` — 한국어 (우선)
/// 2. `{video_id}.en.json3` — 영어 폴백
/// 3. 디렉토리 내 임의의 `.json3` 파일
/// 4. 없으면 `None`
pub fn find_subtitle_file(source_dir: &Path, video_id: &str) -> Option<PathBuf> {
    // Priority 1: Korean
    let ko_path = source_dir.join(format!("{video_id}.ko.json3"));
    if ko_path.exists() {
        return Some(ko_path);
    }

    // Priority 2: English
    let en_path = source_dir.join(format!("{video_id}.en.json3"));
    if en_path.exists() {
        return Some(en_path);
    }

    // Priority 3: Any .json3 file
    if let Ok(entries) = std::fs::read_dir(source_dir) {
        let mut json3_files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().map_or(false, |ext| ext == "json3"))
            .collect();

        // Stable ordering for determinism
        json3_files.sort();

        if let Some(path) = json3_files.into_iter().next() {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("failed to create temp dir")
    }

    #[test]
    fn find_subtitle_prefers_korean() {
        let tmp = make_temp_dir();
        let dir = tmp.path();
        fs::write(dir.join("abc123.en.json3"), "en").unwrap();
        fs::write(dir.join("abc123.ko.json3"), "ko").unwrap();

        let result = find_subtitle_file(dir, "abc123").unwrap();
        assert_eq!(result.file_name().unwrap(), "abc123.ko.json3");
    }

    #[test]
    fn find_subtitle_falls_back_to_english() {
        let tmp = make_temp_dir();
        let dir = tmp.path();
        fs::write(dir.join("abc123.en.json3"), "en").unwrap();

        let result = find_subtitle_file(dir, "abc123").unwrap();
        assert_eq!(result.file_name().unwrap(), "abc123.en.json3");
    }

    #[test]
    fn find_subtitle_falls_back_to_any_json3() {
        let tmp = make_temp_dir();
        let dir = tmp.path();
        fs::write(dir.join("abc123.fr.json3"), "fr").unwrap();

        let result = find_subtitle_file(dir, "abc123").unwrap();
        assert_eq!(result.file_name().unwrap(), "abc123.fr.json3");
    }

    #[test]
    fn find_subtitle_returns_none_when_no_json3() {
        let tmp = make_temp_dir();
        let dir = tmp.path();
        fs::write(dir.join("abc123.mp4"), "video").unwrap();

        let result = find_subtitle_file(dir, "abc123");
        assert!(result.is_none());
    }

    #[test]
    fn find_subtitle_returns_none_for_empty_dir() {
        let tmp = make_temp_dir();
        let result = find_subtitle_file(tmp.path(), "abc123");
        assert!(result.is_none());
    }

    #[test]
    fn find_subtitle_korean_wins_over_any_json3() {
        let tmp = make_temp_dir();
        let dir = tmp.path();
        // Create an arbitrary json3 file that sorts before .ko lexically
        fs::write(dir.join("abc123.aa.json3"), "aa").unwrap();
        fs::write(dir.join("abc123.ko.json3"), "ko").unwrap();

        let result = find_subtitle_file(dir, "abc123").unwrap();
        assert_eq!(result.file_name().unwrap(), "abc123.ko.json3");
    }

    #[test]
    fn build_format_string_best() {
        assert_eq!(build_format_string("best"), "bestvideo+bestaudio/best");
    }

    #[test]
    fn build_format_string_height() {
        assert_eq!(
            build_format_string("720"),
            "bestvideo[height<=720]+bestaudio/best[height<=720]"
        );
        assert_eq!(
            build_format_string("1080"),
            "bestvideo[height<=1080]+bestaudio/best[height<=1080]"
        );
    }
}
