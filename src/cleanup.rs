//! Cleanup module — handles post-extraction file cleanup.
//!
//! After frame extraction completes, this module conditionally deletes
//! the downloaded MP4 source file based on the `mp4_retention` config setting.
//! When `mp4_retention` is `false` (the default), the MP4 is removed to save disk space.
//! When `mp4_retention` is `true`, the MP4 is preserved alongside the extracted frames.

use std::fs;
use std::path::{Path, PathBuf};

/// Result of a cleanup operation.
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /// Whether any MP4 files were deleted
    pub mp4_deleted: bool,
    /// Number of MP4 files deleted
    pub files_deleted: u32,
    /// Total bytes freed by deletion
    pub bytes_freed: u64,
    /// Files that were deleted (paths)
    pub deleted_files: Vec<PathBuf>,
    /// If retention was active, the reason cleanup was skipped
    pub skipped_reason: Option<String>,
}

/// Find all MP4 files in a video output directory.
///
/// Scans the given directory and its immediate subdirectories (e.g. `source/`)
/// for files with `.mp4` extension. The downloader places MP4 files in
/// `{video_dir}/source/`, so we must check subdirectories too.
pub fn find_mp4_files(video_dir: &Path) -> Vec<PathBuf> {
    let mut mp4_files = Vec::new();

    if !video_dir.is_dir() {
        return mp4_files;
    }

    // Collect directories to scan: the video_dir itself + immediate subdirectories
    let mut dirs_to_scan = vec![video_dir.to_path_buf()];
    if let Ok(entries) = fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs_to_scan.push(path);
            }
        }
    }

    for dir in &dirs_to_scan {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext.eq_ignore_ascii_case("mp4") {
                            mp4_files.push(path);
                        }
                    }
                }
            }
        }
    }

    mp4_files
}

/// Perform post-extraction cleanup on a video directory.
///
/// If `retain_mp4` is `false`, all `.mp4` files in `video_dir` are deleted.
/// If `retain_mp4` is `true`, no files are deleted and the result indicates skipped.
///
/// This function is designed to be called after frame extraction completes
/// successfully. It logs actions taken and returns a summary.
///
/// # Arguments
/// * `video_dir` - Path to the video's output directory (e.g., `library/<video_id>/`)
/// * `retain_mp4` - Whether to keep the MP4 file (from config.mp4_retention)
///
/// # Returns
/// A `CleanupResult` summarizing what was done.
pub fn cleanup_after_extraction(video_dir: &Path, retain_mp4: bool) -> CleanupResult {
    if retain_mp4 {
        println!(
            "[cleanup] MP4 retention enabled — skipping deletion in {}",
            video_dir.display()
        );
        return CleanupResult {
            mp4_deleted: false,
            files_deleted: 0,
            bytes_freed: 0,
            deleted_files: Vec::new(),
            skipped_reason: Some("mp4_retention is enabled".to_string()),
        };
    }

    let mp4_files = find_mp4_files(video_dir);

    if mp4_files.is_empty() {
        println!(
            "[cleanup] No MP4 files found in {} — nothing to delete",
            video_dir.display()
        );
        return CleanupResult {
            mp4_deleted: false,
            files_deleted: 0,
            bytes_freed: 0,
            deleted_files: Vec::new(),
            skipped_reason: None,
        };
    }

    let mut files_deleted: u32 = 0;
    let mut bytes_freed: u64 = 0;
    let mut deleted_files = Vec::new();

    for mp4_path in &mp4_files {
        // Get file size before deletion for reporting
        let file_size = fs::metadata(mp4_path)
            .map(|m| m.len())
            .unwrap_or(0);

        match fs::remove_file(mp4_path) {
            Ok(()) => {
                println!(
                    "[cleanup] Deleted MP4: {} ({} bytes)",
                    mp4_path.display(),
                    file_size
                );
                files_deleted += 1;
                bytes_freed += file_size;
                deleted_files.push(mp4_path.clone());
            }
            Err(e) => {
                eprintln!(
                    "[cleanup] Failed to delete MP4 {}: {}",
                    mp4_path.display(),
                    e
                );
                // Continue with other files — don't fail the whole pipeline
            }
        }
    }

    CleanupResult {
        mp4_deleted: files_deleted > 0,
        files_deleted,
        bytes_freed,
        deleted_files,
        skipped_reason: None,
    }
}

/// Format bytes into a human-readable string (e.g., "1.5 MB").
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    /// Helper to create a temp dir with test files.
    fn setup_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join("framepick_cleanup_test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_test_file(dir: &Path, filename: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(filename);
        let mut f = File::create(&path).unwrap();
        f.write_all(content).unwrap();
        path
    }

    #[test]
    fn cleanup_deletes_mp4_when_retention_disabled() {
        let dir = setup_test_dir("delete_mp4");
        create_test_file(&dir, "video.mp4", b"fake mp4 content here");
        create_test_file(&dir, "frame_001.jpg", b"image data");

        let result = cleanup_after_extraction(&dir, false);

        assert!(result.mp4_deleted);
        assert_eq!(result.files_deleted, 1);
        assert!(result.bytes_freed > 0);
        assert_eq!(result.deleted_files.len(), 1);
        assert!(result.skipped_reason.is_none());

        // MP4 should be gone
        assert!(!dir.join("video.mp4").exists());
        // JPG should still be there
        assert!(dir.join("frame_001.jpg").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_preserves_mp4_when_retention_enabled() {
        let dir = setup_test_dir("retain_mp4");
        create_test_file(&dir, "video.mp4", b"fake mp4 content");

        let result = cleanup_after_extraction(&dir, true);

        assert!(!result.mp4_deleted);
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_freed, 0);
        assert!(result.deleted_files.is_empty());
        assert!(result.skipped_reason.is_some());
        assert!(result.skipped_reason.unwrap().contains("retention"));

        // MP4 should still exist
        assert!(dir.join("video.mp4").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_handles_no_mp4_files() {
        let dir = setup_test_dir("no_mp4");
        create_test_file(&dir, "frame_001.jpg", b"image");
        create_test_file(&dir, "segments.json", b"{}");

        let result = cleanup_after_extraction(&dir, false);

        assert!(!result.mp4_deleted);
        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.bytes_freed, 0);
        assert!(result.deleted_files.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_handles_nonexistent_directory() {
        let dir = PathBuf::from("/nonexistent/path/for/test");

        let result = cleanup_after_extraction(&dir, false);

        assert!(!result.mp4_deleted);
        assert_eq!(result.files_deleted, 0);
    }

    #[test]
    fn cleanup_handles_multiple_mp4_files() {
        let dir = setup_test_dir("multi_mp4");
        create_test_file(&dir, "video_720p.mp4", b"mp4 content 720");
        create_test_file(&dir, "video_1080p.mp4", b"mp4 content 1080p longer");
        create_test_file(&dir, "slides.html", b"<html>slides</html>");

        let result = cleanup_after_extraction(&dir, false);

        assert!(result.mp4_deleted);
        assert_eq!(result.files_deleted, 2);
        assert!(result.bytes_freed > 0);
        assert_eq!(result.deleted_files.len(), 2);

        // Both MP4s should be gone
        assert!(!dir.join("video_720p.mp4").exists());
        assert!(!dir.join("video_1080p.mp4").exists());
        // HTML should remain
        assert!(dir.join("slides.html").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_mp4_files_case_insensitive_extension() {
        let dir = setup_test_dir("case_mp4");
        create_test_file(&dir, "video.MP4", b"upper case ext");
        create_test_file(&dir, "other.Mp4", b"mixed case ext");
        create_test_file(&dir, "image.jpg", b"not mp4");

        let files = find_mp4_files(&dir);
        assert_eq!(files.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_mp4_files_empty_dir() {
        let dir = setup_test_dir("empty_dir");
        let files = find_mp4_files(&dir);
        assert!(files.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_mp4_files_in_source_subdir() {
        let dir = setup_test_dir("source_subdir");
        let source = dir.join("source");
        fs::create_dir_all(&source).unwrap();
        create_test_file(&source, "abc123.mp4", b"mp4 in source subdir");
        create_test_file(&dir, "frame_001.jpg", b"image");

        let files = find_mp4_files(&dir);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("abc123.mp4"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_deletes_mp4_in_source_subdir() {
        let dir = setup_test_dir("cleanup_source");
        let source = dir.join("source");
        fs::create_dir_all(&source).unwrap();
        create_test_file(&source, "video.mp4", b"fake mp4 in source");
        create_test_file(&dir, "frame_001.jpg", b"image");

        let result = cleanup_after_extraction(&dir, false);

        assert!(result.mp4_deleted);
        assert_eq!(result.files_deleted, 1);
        assert!(result.bytes_freed > 0);
        assert!(!source.join("video.mp4").exists());
        assert!(dir.join("frame_001.jpg").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_bytes_various_sizes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(1073741824), "1.0 GB");
        assert_eq!(format_bytes(1610612736), "1.5 GB");
    }

    #[test]
    fn cleanup_only_affects_mp4_not_other_files() {
        let dir = setup_test_dir("selective");
        create_test_file(&dir, "video.mp4", b"mp4");
        create_test_file(&dir, "frame_001.jpg", b"jpg");
        create_test_file(&dir, "segments.json", b"json");
        create_test_file(&dir, "slides.html", b"html");
        create_test_file(&dir, "notes.txt", b"txt");

        cleanup_after_extraction(&dir, false);

        assert!(!dir.join("video.mp4").exists());
        assert!(dir.join("frame_001.jpg").exists());
        assert!(dir.join("segments.json").exists());
        assert!(dir.join("slides.html").exists());
        assert!(dir.join("notes.txt").exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
