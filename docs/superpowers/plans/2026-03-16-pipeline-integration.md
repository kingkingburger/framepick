# Pipeline Integration Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing capture/subtitle/slides modules into the queue_processor pipeline so videos are actually downloaded, frames captured, and slides generated.

**Architecture:** The individual modules (capture.rs, subtitle_extractor.rs, slides_generator.rs, cleanup.rs) are already implemented with tests. The `queue_processor::process_single_item()` function currently has placeholder stages. This plan replaces each placeholder with real calls to existing modules, plus adds a new metadata fetch module.

**Tech Stack:** Rust (Tauri 2), yt-dlp (CLI), ffmpeg (CLI), serde_json

---

## Current State

- **11 of 13 Rust modules**: COMPLETE with tests
- **All 11 JS frontend modules**: COMPLETE
- **CSS/HTML**: COMPLETE
- **Compilation**: SUCCESS (1 deprecation warning)
- **Gap**: `queue_processor::process_single_item()` has 4 placeholder stages

### Placeholder Stages in queue_processor.rs (lines 250-269)

```
Stage: Download         → placeholder (no yt-dlp call)
Stage: Extract Subtitles → placeholder (no subtitle_extractor call)
Stage: Extract Frames   → placeholder (no capture call)
Stage: Generate Slides  → placeholder (no slides_generator call)
```

### Already-Implemented Modules to Wire In

| Module | Key Function | Status |
|--------|-------------|--------|
| `subtitle_extractor.rs` | `extract_subtitles_cmd()` | COMPLETE, 50+ tests |
| `capture.rs` | `capture_frames()` | COMPLETE, supports scene/interval/subtitle modes |
| `slides_generator.rs` | HTML generation | COMPLETE, 40+ tests |
| `cleanup.rs` | `cleanup_after_extraction()` | COMPLETE, already wired in |

### What's Missing

1. **Metadata fetch module** - yt-dlp `--dump-json` to get title, duration, channel
2. **Video download function** - yt-dlp download with quality setting
3. **Wiring** in `process_single_item()` connecting existing modules

---

## File Structure

| Action | Path | Responsibility |
|--------|------|---------------|
| Create | `src/metadata.rs` | Fetch video metadata via yt-dlp --dump-json |
| Create | `src/downloader.rs` | Download video via yt-dlp with quality setting |
| Modify | `src/queue_processor.rs` | Replace placeholders with real module calls |
| Modify | `src/lib.rs` | Register new modules |

---

## Chunk 1: Metadata Fetch Module

### Task 1: Create metadata.rs

**Files:**
- Create: `src/metadata.rs`
- Modify: `src/lib.rs` (add `pub mod metadata;`)

- [ ] **Step 1: Write the metadata module**

```rust
// src/metadata.rs
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: f64,
    pub upload_date: String,
}

/// Resolve yt-dlp executable path (next to app exe, or on PATH).
pub fn resolve_ytdlp_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("yt-dlp.exe");
            if candidate.exists() {
                return candidate;
            }
            // Also try without .exe for non-Windows or dev
            let candidate2 = dir.join("yt-dlp");
            if candidate2.exists() {
                return candidate2;
            }
        }
    }
    PathBuf::from("yt-dlp")
}

/// Fetch video metadata using yt-dlp --dump-json.
pub fn fetch_metadata(url: &str) -> Result<VideoMetadata, String> {
    let ytdlp = resolve_ytdlp_path();
    let output = Command::new(&ytdlp)
        .args(["--dump-json", "--no-download", "--no-playlist", url])
        .output()
        .map_err(|e| format!("Failed to run yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp metadata fetch failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("Failed to parse metadata JSON: {e}"))?;

    Ok(VideoMetadata {
        id: json["id"].as_str().unwrap_or("unknown").to_string(),
        title: json["title"].as_str().unwrap_or("Untitled").to_string(),
        channel: json["channel"].as_str()
            .or_else(|| json["uploader"].as_str())
            .unwrap_or("Unknown").to_string(),
        duration: json["duration"].as_f64().unwrap_or(0.0),
        upload_date: json["upload_date"].as_str().unwrap_or("").to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_ytdlp_returns_path() {
        let path = resolve_ytdlp_path();
        // Should return a PathBuf (either found or fallback)
        assert!(!path.as_os_str().is_empty());
    }

    #[test]
    fn metadata_struct_serialization() {
        let meta = VideoMetadata {
            id: "abc123".to_string(),
            title: "Test Video".to_string(),
            channel: "Test Channel".to_string(),
            duration: 120.5,
            upload_date: "20260101".to_string(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let decoded: VideoMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, "abc123");
        assert_eq!(decoded.title, "Test Video");
        assert_eq!(decoded.duration, 120.5);
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod metadata;` to `src/lib.rs` module declarations.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib metadata`
Expected: PASS (2 tests)

- [ ] **Step 4: Commit**

```bash
git add src/metadata.rs src/lib.rs
git commit -m "feat: add metadata fetch module (yt-dlp --dump-json)"
```

---

### Task 2: Create downloader.rs

**Files:**
- Create: `src/downloader.rs`
- Modify: `src/lib.rs` (add `pub mod downloader;`)

- [ ] **Step 1: Write the downloader module**

```rust
// src/downloader.rs
use crate::metadata::resolve_ytdlp_path;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of a video download operation.
pub struct DownloadResult {
    /// Path to the downloaded MP4 file.
    pub mp4_path: PathBuf,
    /// Path to the subtitle file (if downloaded).
    pub subtitle_path: Option<PathBuf>,
}

/// Download a YouTube video to the specified output directory.
///
/// Uses yt-dlp with the given quality setting.
/// Also downloads subtitles (ko > en) in the same call.
pub fn download_video(
    url: &str,
    output_dir: &Path,
    video_id: &str,
    quality: &str,
) -> Result<DownloadResult, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {e}"))?;

    let source_dir = output_dir.join("source");
    std::fs::create_dir_all(&source_dir)
        .map_err(|e| format!("Failed to create source dir: {e}"))?;

    let ytdlp = resolve_ytdlp_path();

    // Build format string based on quality
    let format_str = if quality == "best" {
        "bestvideo+bestaudio/best".to_string()
    } else {
        format!("bestvideo[height<={}]+bestaudio/best[height<={}]", quality, quality)
    };

    let output_template = source_dir
        .join(format!("{}.%(ext)s", video_id))
        .to_string_lossy()
        .to_string();

    let mut args = vec![
        "-f".to_string(),
        format_str,
        "--merge-output-format".to_string(),
        "mp4".to_string(),
        "-o".to_string(),
        output_template,
        "--no-playlist".to_string(),
        // Also download subtitles
        "--write-auto-sub".to_string(),
        "--sub-lang".to_string(),
        "ko,en".to_string(),
        "--convert-subs".to_string(),
        "json3".to_string(),
        url.to_string(),
    ];

    let output = Command::new(&ytdlp)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run yt-dlp download: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp download failed: {stderr}"));
    }

    // Find the downloaded MP4
    let mp4_path = source_dir.join(format!("{}.mp4", video_id));
    if !mp4_path.exists() {
        // Try to find any mp4 in the source dir
        let found = std::fs::read_dir(&source_dir)
            .ok()
            .and_then(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .find(|e| {
                        e.path().extension().map(|ext| ext == "mp4").unwrap_or(false)
                    })
                    .map(|e| e.path())
            });
        if let Some(found_path) = found {
            // Rename to expected name
            let _ = std::fs::rename(&found_path, &mp4_path);
        }
    }

    if !mp4_path.exists() {
        return Err("Download completed but MP4 file not found".to_string());
    }

    // Find subtitle file (prefer ko, fallback en)
    let subtitle_path = find_subtitle_file(&source_dir, video_id);

    Ok(DownloadResult {
        mp4_path,
        subtitle_path,
    })
}

/// Find the best subtitle file in the source directory.
fn find_subtitle_file(source_dir: &Path, video_id: &str) -> Option<PathBuf> {
    // Priority: ko.json3 > en.json3 > any .json3
    let ko_path = source_dir.join(format!("{}.ko.json3", video_id));
    if ko_path.exists() {
        return Some(ko_path);
    }
    let en_path = source_dir.join(format!("{}.en.json3", video_id));
    if en_path.exists() {
        return Some(en_path);
    }
    // Try any json3 file
    std::fs::read_dir(source_dir)
        .ok()
        .and_then(|entries| {
            entries
                .filter_map(|e| e.ok())
                .find(|e| {
                    e.path()
                        .extension()
                        .map(|ext| ext == "json3")
                        .unwrap_or(false)
                })
                .map(|e| e.path())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_subtitle_prefers_korean() {
        let dir = std::env::temp_dir().join("framepick_dl_test_ko");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create both ko and en subtitle files
        std::fs::write(dir.join("abc.ko.json3"), "{}").unwrap();
        std::fs::write(dir.join("abc.en.json3"), "{}").unwrap();

        let result = find_subtitle_file(&dir, "abc");
        assert!(result.is_some());
        assert!(result.unwrap().to_string_lossy().contains("ko.json3"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_subtitle_falls_back_to_english() {
        let dir = std::env::temp_dir().join("framepick_dl_test_en");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        std::fs::write(dir.join("abc.en.json3"), "{}").unwrap();

        let result = find_subtitle_file(&dir, "abc");
        assert!(result.is_some());
        assert!(result.unwrap().to_string_lossy().contains("en.json3"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_subtitle_returns_none_when_missing() {
        let dir = std::env::temp_dir().join("framepick_dl_test_none");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = find_subtitle_file(&dir, "abc");
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Add `pub mod downloader;` to `src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test --lib downloader`
Expected: PASS (3 tests)

- [ ] **Step 4: Commit**

```bash
git add src/downloader.rs src/lib.rs
git commit -m "feat: add downloader module (yt-dlp video + subtitle download)"
```

---

## Chunk 2: Wire Pipeline in queue_processor.rs

### Task 3: Replace placeholders with real pipeline calls

**Files:**
- Modify: `src/queue_processor.rs:180-319` (replace `process_single_item`)

This is the core task. Replace the placeholder stages with actual calls to:
1. `metadata::fetch_metadata()` - get video title
2. `downloader::download_video()` - download MP4 + subtitles
3. `capture::capture_frames()` - extract frames using ffmpeg
4. `slides_generator` - generate slides.html

- [ ] **Step 1: Add imports to queue_processor.rs**

Add at top of file:
```rust
use crate::downloader;
use crate::metadata;
use crate::slides_generator;
use crate::capture::{CaptureOptions, CapturedFrame};
```

- [ ] **Step 2: Replace process_single_item function**

Replace the entire `process_single_item` function (lines 180-319) with real pipeline logic:

```rust
async fn process_single_item(
    app: &AppHandle,
    item: &QueueItem,
) -> Result<Option<String>, String> {
    // Stage 1: Validate URL
    let video_id = crate::url_validator::extract_video_id(&item.url)
        .ok_or_else(|| "Invalid YouTube URL".to_string())?;

    // Stage 1b: Duplicate detection
    let (library_path, quality, retain_mp4) = {
        let settings_state = app.state::<SettingsState>();
        let s = settings_state.0.lock().map_err(|e| e.to_string())?;
        (s.library_path.clone(), s.download_quality.clone(), s.mp4_retention)
    };
    let video_dir = ConfigState::resolved_library_path(&library_path).join(&video_id);
    if video_dir.exists() && video_dir.is_dir() {
        let _ = app.emit("queue:duplicate-skipped", serde_json::json!({
            "id": item.id,
            "video_id": video_id,
            "title": item.title,
        }));
        update_item_status(app, item.id, "skipped", None, None, None);
        return Err(format!("Video '{}' already exists in library", video_id));
    }

    // ── Resolve effective capture mode (subtitle fallback) ──
    let url_clone = item.url.clone();
    let mode_clone = item.capture_mode.clone();
    let resolved = tauri::async_runtime::spawn_blocking(move || {
        crate::capture_fallback::resolve_capture_mode(&url_clone, &mode_clone)
    })
    .await
    .map_err(|e| format!("Capture mode resolution failed: {e}"))?;

    let effective_mode = resolved.effective_mode.clone();
    if resolved.did_fallback {
        crate::capture_fallback::emit_fallback_event(app, item.id, &item.url, &resolved);
        update_item_capture_mode(app, item.id, &effective_mode);
    }

    let mut tracker = ProgressTracker::new(item.id, &effective_mode);

    // ── Stage: Fetch Metadata ──
    tracker.emit(app, 0, Some("Fetching video metadata...".to_string()));
    let url_for_meta = item.url.clone();
    let meta = tauri::async_runtime::spawn_blocking(move || {
        metadata::fetch_metadata(&url_for_meta)
    })
    .await
    .map_err(|e| format!("Metadata task failed: {e}"))?
    .map_err(|e| format!("Metadata fetch failed: {e}"))?;

    let title = meta.title.clone();
    update_item_status(app, item.id, "processing", Some(title.clone()), None, Some(10));

    // ── Stage: Download Video ──
    tracker.emit(app, 0, Some("Downloading video...".to_string()));
    let url_for_dl = item.url.clone();
    let vid_id = video_id.clone();
    let qual = quality.clone();
    let out_dir = video_dir.clone();
    let dl_result = tauri::async_runtime::spawn_blocking(move || {
        downloader::download_video(&url_for_dl, &out_dir, &vid_id, &qual)
    })
    .await
    .map_err(|e| format!("Download task failed: {e}"))?
    .map_err(|e| format!("Download failed: {e}"))?;

    tracker.emit(app, 100, None);
    tracker.complete_stage(app);

    // ── Stage: Extract Subtitles (subtitle mode only) ──
    let subtitle_cues = if effective_mode == "subtitle" {
        tracker.emit(app, 0, Some("Extracting subtitles...".to_string()));
        if let Some(sub_path) = &dl_result.subtitle_path {
            let content = std::fs::read_to_string(sub_path).ok();
            let cues = content.and_then(|c| {
                crate::subtitle_extractor::parse_json3_subtitles(&c).ok()
            });
            tracker.emit(app, 100, None);
            tracker.complete_stage(app);
            cues
        } else {
            tracker.emit(app, 100, Some("No subtitles found".to_string()));
            tracker.complete_stage(app);
            None
        }
    } else {
        None
    };

    // ── Stage: Extract Frames ──
    tracker.emit(app, 0, Some("Capturing frames...".to_string()));

    let images_dir = video_dir.join("images");
    std::fs::create_dir_all(&images_dir)
        .map_err(|e| format!("Failed to create images dir: {e}"))?;

    let mp4_path = dl_result.mp4_path.clone();
    let mode_for_capture = effective_mode.clone();
    let interval = item.interval_seconds.unwrap_or(30);
    let threshold = {
        let settings_state = app.state::<SettingsState>();
        settings_state.0.lock()
            .map(|s| s.scene_change_threshold)
            .unwrap_or(0.30)
    };
    let sub_cues = subtitle_cues.clone();
    let img_dir = images_dir.clone();

    let captured_frames: Vec<CapturedFrame> = tauri::async_runtime::spawn_blocking(move || {
        let opts = CaptureOptions {
            mode: mode_for_capture,
            scene_threshold: threshold,
            interval_seconds: interval,
            subtitle_cues: sub_cues,
            video_path: mp4_path.to_string_lossy().to_string(),
            output_dir: img_dir.to_string_lossy().to_string(),
        };
        crate::capture::run_capture(&opts)
    })
    .await
    .map_err(|e| format!("Capture task failed: {e}"))?
    .map_err(|e| format!("Frame capture failed: {e}"))?;

    tracker.emit(app, 100, Some(format!("{} frames captured", captured_frames.len())));
    tracker.complete_stage(app);

    // ── Stage: Generate Slides ──
    tracker.emit(app, 0, Some("Generating slides...".to_string()));

    let slides_path = video_dir.join("slides.html");
    let segments_path = video_dir.join("segments.json");

    // Save segments.json
    let segments_json = serde_json::to_string_pretty(&captured_frames)
        .map_err(|e| format!("Failed to serialize segments: {e}"))?;
    std::fs::write(&segments_path, &segments_json)
        .map_err(|e| format!("Failed to write segments.json: {e}"))?;

    // Generate slides.html using slides_generator
    let html = crate::slides_generator::generate_slides_html(
        &title,
        &video_id,
        &captured_frames,
        &subtitle_cues,
    );
    std::fs::write(&slides_path, &html)
        .map_err(|e| format!("Failed to write slides.html: {e}"))?;

    tracker.emit(app, 100, None);
    tracker.complete_stage(app);

    // ── Stage: Cleanup ──
    tracker.emit(app, 0, Some("Cleaning up...".to_string()));

    let cleanup_result = cleanup::cleanup_after_extraction(&video_dir, retain_mp4);
    if cleanup_result.mp4_deleted {
        let detail = format!(
            "Deleted {} file(s), freed {}",
            cleanup_result.files_deleted,
            cleanup::format_bytes(cleanup_result.bytes_freed)
        );
        tracker.emit(app, 100, Some(detail));
    } else {
        tracker.emit(app, 100, Some("Done".to_string()));
    }
    tracker.complete_stage(app);

    tracker.emit_done(app);
    Ok(Some(title))
}
```

**IMPORTANT**: The exact function signatures for `capture::run_capture()`, `slides_generator::generate_slides_html()`, and `subtitle_extractor::parse_json3_subtitles()` must be verified against the actual implementations before writing code. The above is a reference — adapt parameter types and names to match what exists.

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: SUCCESS (fix any type mismatches between modules)

- [ ] **Step 4: Run all tests**

Run: `cargo test`
Expected: All existing 200+ tests pass

- [ ] **Step 5: Commit**

```bash
git add src/queue_processor.rs
git commit -m "feat: wire real pipeline into queue_processor (download/capture/slides)"
```

---

## Chunk 3: Adapter Layer (if needed)

### Task 4: Adapt module interfaces

The existing modules may have slightly different function signatures than what `process_single_item` expects. This task handles any adapter code needed.

**Files:**
- Modify: `src/capture.rs` (add `run_capture` public function if missing)
- Modify: `src/slides_generator.rs` (add `generate_slides_html` public function if missing)
- Modify: `src/subtitle_extractor.rs` (ensure `parse_json3_subtitles` is public)

- [ ] **Step 1: Check capture.rs public API**

Read `capture.rs` and verify there is a function that:
- Takes a `CaptureOptions` struct (mode, threshold, interval, video_path, output_dir, subtitle_cues)
- Returns `Result<Vec<CapturedFrame>, String>`
- If not, create a `run_capture()` wrapper

- [ ] **Step 2: Check slides_generator.rs public API**

Read `slides_generator.rs` and verify there is a function that:
- Takes title, video_id, frames, and optional subtitle cues
- Returns HTML string
- If not, create a `generate_slides_html()` wrapper

- [ ] **Step 3: Check subtitle_extractor.rs public API**

Read `subtitle_extractor.rs` and verify `parse_json3_subtitles()` is public and returns subtitle cue data that capture.rs can use.

- [ ] **Step 4: Implement any needed adapters**

Write thin adapter functions to bridge interface mismatches.

- [ ] **Step 5: Verify full compilation and tests**

Run: `cargo check && cargo test`
Expected: SUCCESS

- [ ] **Step 6: Commit**

```bash
git add src/capture.rs src/slides_generator.rs src/subtitle_extractor.rs
git commit -m "feat: add adapter functions for pipeline integration"
```

---

## Chunk 4: Fix Deprecation Warning

### Task 5: Fix tauri-plugin-shell deprecation

**Files:**
- Modify: `src/slides_viewer.rs` (fix Shell::open deprecation)

- [ ] **Step 1: Find and fix the deprecation**

Run: `cargo check 2>&1 | grep -i deprec`
Fix the deprecated Shell::open API call in slides_viewer.rs.

- [ ] **Step 2: Verify**

Run: `cargo check`
Expected: No warnings

- [ ] **Step 3: Commit**

```bash
git add src/slides_viewer.rs
git commit -m "fix: resolve tauri-plugin-shell deprecation warning"
```

---

## Chunk 5: Integration Smoke Test

### Task 6: Manual verification

- [ ] **Step 1: Build the app**

Run: `cargo build`
Expected: SUCCESS

- [ ] **Step 2: Verify all tests pass**

Run: `cargo test`
Expected: All 200+ tests pass

- [ ] **Step 3: Document completion**

Update this plan with checkmarks for all completed steps.

---

## Summary

| Chunk | Tasks | Estimated Steps |
|-------|-------|----------------|
| 1: New Modules | metadata.rs + downloader.rs | 8 steps |
| 2: Wire Pipeline | Replace placeholders in queue_processor.rs | 5 steps |
| 3: Adapters | Bridge any interface mismatches | 6 steps |
| 4: Deprecation | Fix Shell::open warning | 3 steps |
| 5: Smoke Test | Build + test | 3 steps |
| **Total** | **6 tasks** | **25 steps** |
