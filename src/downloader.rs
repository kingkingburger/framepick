//! YouTube video downloader via yt-dlp.
//!
//! Downloads a YouTube video with format selection based on quality settings,
//! and optionally downloads auto-generated subtitles (Korean priority, English
//! fallback) in json3 format.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::metadata::resolve_ytdlp_path;

/// Result of a successful video download.
#[derive(Debug, Clone)]
pub struct DownloadResult {
    /// Path to the downloaded MP4 file.
    pub mp4_path: PathBuf,
    /// Path to the downloaded subtitle file (if found).
    pub subtitle_path: Option<PathBuf>,
}

/// Download a YouTube video using yt-dlp.
///
/// Creates `{output_dir}/source/` and downloads the video there.
/// Format selection is based on `quality`:
/// - `"best"` — `bestvideo+bestaudio/best`
/// - anything else (e.g. `"720"`) — `bestvideo[height<=N]+bestaudio/best[height<=N]`
///
/// Subtitles are downloaded alongside the video using
/// `--write-auto-sub --sub-lang ko,en --convert-subs json3`.
///
/// Returns a [`DownloadResult`] with the MP4 path and optional subtitle path.
/// Returns `Err` if yt-dlp fails or no MP4 file is found after download.
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

    let output = Command::new(&ytdlp)
        .args([
            "-f",
            &format,
            "--merge-output-format",
            "mp4",
            "--write-auto-sub",
            "--sub-lang",
            "ko,en",
            "--convert-subs",
            "json3",
            "--no-playlist",
            "-o",
            &output_template,
            url,
        ])
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

    let subtitle_path = find_subtitle_file(&source_dir, video_id);

    Ok(DownloadResult {
        mp4_path,
        subtitle_path,
    })
}

/// Build the yt-dlp format selection string from a quality string.
///
/// `"best"` returns `bestvideo+bestaudio/best`.
/// A numeric height string like `"720"` returns
/// `bestvideo[height<=720]+bestaudio/best[height<=720]`.
fn build_format_string(quality: &str) -> String {
    if quality == "best" {
        "bestvideo+bestaudio/best".to_string()
    } else {
        format!(
            "bestvideo[height<={quality}]+bestaudio/best[height<={quality}]"
        )
    }
}

/// Look for a json3 subtitle file in `source_dir` for the given `video_id`.
///
/// Search order:
/// 1. `{video_id}.ko.json3` — Korean (preferred)
/// 2. `{video_id}.en.json3` — English fallback
/// 3. Any `.json3` file in the directory
/// 4. `None` if nothing is found
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
