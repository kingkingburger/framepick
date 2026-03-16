//! End-to-end integration tests for the 3 capture modes.
//!
//! These tests require **yt-dlp** and **ffmpeg** to be installed (either in
//! `tools/` next to the test binary, on the system PATH, or pointed to via
//! the `YTDLP_PATH` / `FFMPEG_PATH` environment variables).
//!
//! They also require network access to download a short YouTube video.
//!
//! Run with:
//! ```sh
//! cargo test --test e2e_capture -- --ignored
//! ```
//!
//! Set `TEST_VIDEO_URL` env-var to override the default test video.

use std::path::Path;
use std::process::Command;

use framepick_lib::{capture, downloader, slides_generator, tools_manager};

// ─── Helpers ─────────────────────────────────────────────────────────

/// Default short test video (~10 seconds, CC-licensed, has auto-subs).
fn test_video_url() -> String {
    std::env::var("TEST_VIDEO_URL")
        .unwrap_or_else(|_| "https://www.youtube.com/watch?v=jNQXAC9IVRw".to_string())
}

fn test_video_id() -> String {
    std::env::var("TEST_VIDEO_ID")
        .unwrap_or_else(|_| "jNQXAC9IVRw".to_string())
}

/// Check that yt-dlp is reachable.
fn ytdlp_available() -> bool {
    let path = tools_manager::resolve_ytdlp_path();
    Command::new(&path)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check that ffmpeg is reachable.
fn ffmpeg_available() -> bool {
    let path = tools_manager::resolve_ffmpeg_path();
    Command::new(&path)
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check that ffprobe is reachable.
fn ffprobe_available() -> bool {
    let path = tools_manager::resolve_ffprobe_path();
    Command::new(&path)
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Skip test early if required tools are missing.
macro_rules! require_tools {
    () => {
        if !ytdlp_available() {
            eprintln!("SKIP: yt-dlp not found");
            return;
        }
        if !ffmpeg_available() {
            eprintln!("SKIP: ffmpeg not found");
            return;
        }
        if !ffprobe_available() {
            eprintln!("SKIP: ffprobe not found");
            return;
        }
    };
}

/// Download the test video into `output_dir` and return the DownloadResult.
fn download_test_video(output_dir: &Path) -> downloader::DownloadResult {
    let url = test_video_url();
    let video_id = test_video_id();
    downloader::download_video(&url, output_dir, &video_id, "best")
        .expect("Failed to download test video")
}

/// Assert that captured frames are non-empty and image files exist.
fn assert_frames_valid(frames: &[capture::CapturedFrame], output_dir: &Path) {
    assert!(
        !frames.is_empty(),
        "Expected at least one captured frame, got 0"
    );
    let images_dir = output_dir.join("images");
    for frame in frames {
        let img_path = images_dir.join(&frame.filename);
        assert!(
            img_path.exists(),
            "Frame image missing: {}",
            img_path.display()
        );
        let meta = std::fs::metadata(&img_path).unwrap();
        assert!(
            meta.len() > 0,
            "Frame image is empty: {}",
            img_path.display()
        );
    }
}

/// Generate slides.html and verify it exists.
fn assert_slides_generated(
    output_dir: &Path,
    segments: &[slides_generator::Segment],
    video_id: &str,
) {
    let meta = slides_generator::VideoMetadata {
        title: "E2E Test Video".to_string(),
        url: test_video_url(),
        channel: "Test".to_string(),
        date: "2024-01-01".to_string(),
        duration: "10s".to_string(),
        video_id: video_id.to_string(),
    };
    slides_generator::generate_slides_html(output_dir, segments, &meta)
        .expect("Failed to generate slides.html");

    let slides_path = output_dir.join("slides.html");
    assert!(slides_path.exists(), "slides.html not generated");
    let content = std::fs::read_to_string(&slides_path).unwrap();
    assert!(
        content.contains("E2E Test Video"),
        "slides.html missing video title"
    );
}

// ─── E2E Tests ──────────────────────────────────────────────────────

#[test]
#[ignore]
fn e2e_scene_capture_mode() {
    require_tools!();

    let tmp = tempfile::TempDir::new().unwrap();
    let output_dir = tmp.path().join("scene_test");
    std::fs::create_dir_all(&output_dir).unwrap();

    // Download
    let dl = download_test_video(&output_dir);
    assert!(dl.mp4_path.exists(), "MP4 not downloaded");

    // Capture — scene mode (30% threshold)
    let frames =
        capture::capture_scene_change(&dl.mp4_path, &output_dir, 0.30)
            .expect("Scene capture failed");

    println!("[scene] Captured {} frames", frames.len());
    assert_frames_valid(&frames, &output_dir);

    // Generate slides
    let segments = slides_generator::frames_to_segments(&frames);
    assert_slides_generated(&output_dir, &segments, &test_video_id());
}

#[test]
#[ignore]
fn e2e_interval_capture_mode() {
    require_tools!();

    let tmp = tempfile::TempDir::new().unwrap();
    let output_dir = tmp.path().join("interval_test");
    std::fs::create_dir_all(&output_dir).unwrap();

    // Download
    let dl = download_test_video(&output_dir);
    assert!(dl.mp4_path.exists(), "MP4 not downloaded");

    // Capture — interval mode (every 3 seconds)
    let frames =
        capture::capture_interval(&dl.mp4_path, &output_dir, 3, None)
            .expect("Interval capture failed");

    println!("[interval] Captured {} frames", frames.len());
    assert_frames_valid(&frames, &output_dir);

    // For a ~19 second video at 3s interval, expect at least 3 frames
    assert!(
        frames.len() >= 3,
        "Expected >= 3 frames at 3s interval, got {}",
        frames.len()
    );

    // Generate slides
    let segments = slides_generator::frames_to_segments(&frames);
    assert_slides_generated(&output_dir, &segments, &test_video_id());
}

#[test]
#[ignore]
fn e2e_subtitle_capture_mode() {
    require_tools!();

    let tmp = tempfile::TempDir::new().unwrap();
    let output_dir = tmp.path().join("subtitle_test");
    std::fs::create_dir_all(&output_dir).unwrap();

    // Download (includes subtitle download)
    let dl = download_test_video(&output_dir);
    assert!(dl.mp4_path.exists(), "MP4 not downloaded");

    // Capture — subtitle mode
    let result = capture::capture_subtitle(&dl.mp4_path, &output_dir);

    match result {
        Ok(sub_result) => {
            println!(
                "[subtitle] Captured {} frames, {} cues",
                sub_result.frames.len(),
                sub_result.cues.len()
            );
            assert_frames_valid(&sub_result.frames, &output_dir);

            // Generate slides with subtitle text
            let segments = slides_generator::frames_to_segments_with_subtitles(
                &sub_result.frames,
                &sub_result.cues,
            );
            assert_slides_generated(&output_dir, &segments, &test_video_id());
        }
        Err(e) => {
            // Subtitle mode may fail if no subtitles available — that's acceptable
            // as long as the error is about missing subtitles, not a crash
            let msg = e.to_string();
            assert!(
                msg.contains("subtitle") || msg.contains("No subtitle"),
                "Unexpected subtitle capture error: {msg}"
            );
            println!("[subtitle] No subtitles available (expected for some videos): {msg}");
        }
    }
}

#[test]
#[ignore]
fn e2e_tools_resolve_paths() {
    // Verify all path resolution functions return sensible values
    let ytdlp = tools_manager::resolve_ytdlp_path();
    let ffmpeg = tools_manager::resolve_ffmpeg_path();
    let ffprobe = tools_manager::resolve_ffprobe_path();

    println!("yt-dlp:  {}", ytdlp.display());
    println!("ffmpeg:  {}", ffmpeg.display());
    println!("ffprobe: {}", ffprobe.display());

    assert!(!ytdlp.to_string_lossy().is_empty());
    assert!(!ffmpeg.to_string_lossy().is_empty());
    assert!(!ffprobe.to_string_lossy().is_empty());
}
