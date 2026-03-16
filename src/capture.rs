//! Frame capture module supporting multiple capture modes.
//!
//! Capture modes:
//! - **scene**: Detect scene changes using ffmpeg's `select` filter with
//!   a configurable threshold (default 30%).
//! - **interval**: Capture frames at fixed time intervals.
//! - **subtitle**: Capture frames at subtitle cue start times (VTT/SRT).
//!   Falls back to scene-change mode at 30% threshold if no subtitles found.
//!
//! All modes shell out to ffmpeg which must be present next to the executable
//! (portable distribution) or on the system PATH.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cmd_util::HideWindow;

/// Default scene-change detection threshold (0.0–1.0).
/// A value of 0.30 means a frame is captured when the difference
/// from the previous frame exceeds 30%.
pub const DEFAULT_SCENE_THRESHOLD: f64 = 0.30;

/// Result of a single captured frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedFrame {
    /// Zero-based index of this frame in the capture sequence.
    pub index: usize,
    /// Timestamp in the source video (seconds).
    pub timestamp_secs: f64,
    /// Human-readable timestamp string, e.g. "00:01:23".
    pub timestamp: String,
    /// Output image filename (relative to the output directory).
    pub filename: String,
}

/// Options for the capture operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureOptions {
    /// Capture mode: "scene", "interval", or "subtitle".
    pub mode: String,
    /// Scene-change threshold (0.0–1.0). Only used when mode == "scene".
    #[serde(default = "default_scene_threshold")]
    pub scene_threshold: f64,
    /// Interval in seconds. Only used when mode == "interval".
    #[serde(default = "default_interval")]
    pub interval_seconds: u32,
}

fn default_scene_threshold() -> f64 {
    DEFAULT_SCENE_THRESHOLD
}

fn default_interval() -> u32 {
    10
}

impl Default for CaptureOptions {
    fn default() -> Self {
        Self {
            mode: "scene".to_string(),
            scene_threshold: DEFAULT_SCENE_THRESHOLD,
            interval_seconds: 10,
        }
    }
}

/// Errors from the capture pipeline.
#[derive(Debug)]
pub enum CaptureError {
    /// ffmpeg binary not found.
    FfmpegNotFound(String),
    /// ffmpeg exited with a non-zero status.
    FfmpegFailed { exit_code: Option<i32>, stderr: String },
    /// Failed to parse ffmpeg output.
    ParseError(String),
    /// General I/O error.
    Io(std::io::Error),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::FfmpegNotFound(msg) => write!(f, "ffmpeg not found: {msg}"),
            CaptureError::FfmpegFailed { exit_code, stderr } => {
                write!(f, "ffmpeg failed (exit {:?}): {}", exit_code, stderr)
            }
            CaptureError::ParseError(msg) => write!(f, "Parse error: {msg}"),
            CaptureError::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<std::io::Error> for CaptureError {
    fn from(e: std::io::Error) -> Self {
        CaptureError::Io(e)
    }
}

/// Resolve the path to the ffmpeg binary.
///
/// Search order:
/// 1. `ffmpeg.exe` / `ffmpeg` next to the running executable (portable mode).
/// 2. System PATH (fallback).
pub fn resolve_ffmpeg_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = if cfg!(windows) {
                dir.join("ffmpeg.exe")
            } else {
                dir.join("ffmpeg")
            };
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // Fallback: rely on system PATH
    PathBuf::from("ffmpeg")
}

/// Resolve the path to ffprobe (for duration queries).
pub fn resolve_ffprobe_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = if cfg!(windows) {
                dir.join("ffprobe.exe")
            } else {
                dir.join("ffprobe")
            };
            if candidate.exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("ffprobe")
}

/// Format seconds into "HH:MM:SS" display string.
pub fn format_timestamp(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Format seconds into ffmpeg-friendly "HH-MM-SS" for filenames.
fn format_timestamp_filename(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    format!("{:02}-{:02}-{:02}", h, m, s)
}

// ─── Scene-Change Capture ────────────────────────────────────────

/// Capture frames at scene-change boundaries using ffmpeg's `select` filter.
///
/// The workflow:
/// 1. Run ffmpeg with `-vf select='gt(scene,THRESHOLD)',showinfo` to detect
///    scene changes and log frame timestamps via `showinfo`.
/// 2. Parse stderr for `pts_time:` values to find timestamps.
/// 3. Run ffmpeg again to extract a JPEG frame at each detected timestamp.
///
/// Returns an ordered list of `CapturedFrame` entries.
pub fn capture_scene_change(
    video_path: &Path,
    output_dir: &Path,
    threshold: f64,
) -> Result<Vec<CapturedFrame>, CaptureError> {
    let ffmpeg = resolve_ffmpeg_path();
    let images_dir = output_dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    // Clamp threshold to valid range
    let threshold = threshold.clamp(0.01, 0.99);

    // ── Step 1: Detect scene-change timestamps ──────────────────
    let timestamps = detect_scene_changes(&ffmpeg, video_path, threshold)?;

    if timestamps.is_empty() {
        // No scene changes detected — capture at least the first frame
        return capture_single_frame(&ffmpeg, video_path, &images_dir, 0.0);
    }

    // ── Step 2: Extract frames at detected timestamps ───────────
    let mut frames = Vec::with_capacity(timestamps.len());

    for (idx, &ts) in timestamps.iter().enumerate() {
        let ts_file = format_timestamp_filename(ts);
        let filename = format!("frame_{:04}_{}.jpg", idx, ts_file);
        let output_path = images_dir.join(&filename);

        extract_frame_at(&ffmpeg, video_path, ts, &output_path)?;

        frames.push(CapturedFrame {
            index: idx,
            timestamp_secs: ts,
            timestamp: format_timestamp(ts),
            filename,
        });
    }

    Ok(frames)
}

/// Use ffmpeg's `select` + `showinfo` filters to detect scene-change timestamps.
///
/// The `select='gt(scene,T)'` filter passes only frames where the scene-change
/// score exceeds threshold T (0.0–1.0). The `showinfo` filter logs metadata
/// including `pts_time` for each passed frame. We parse stderr for these values.
fn detect_scene_changes(
    ffmpeg: &Path,
    video_path: &Path,
    threshold: f64,
) -> Result<Vec<f64>, CaptureError> {
    let filter = format!("select='gt(scene,{:.2})',showinfo", threshold);

    let output = Command::new(ffmpeg)
        .args([
            "-i",
            video_path.to_str().unwrap_or(""),
            "-vf",
            &filter,
            "-f",
            "null",
            "-",
        ])
        .hide_window()
        .output()
        .map_err(|e| CaptureError::FfmpegNotFound(format!("Failed to run ffmpeg: {e}")))?;

    // ffmpeg writes filter output to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse pts_time values from showinfo output lines
    // Format: "[Parsed_showinfo_1 ...] n:   0 pts:  12345 pts_time:1.234 ..."
    let mut timestamps: Vec<f64> = Vec::new();
    for line in stderr.lines() {
        if let Some(ts) = parse_pts_time(line) {
            // Deduplicate: skip if within 0.5s of previous timestamp
            if timestamps.last().map_or(true, |&prev| (ts - prev).abs() > 0.5) {
                timestamps.push(ts);
            }
        }
    }

    timestamps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Ok(timestamps)
}

/// Parse a `pts_time:` value from a ffmpeg showinfo log line.
fn parse_pts_time(line: &str) -> Option<f64> {
    // Look for "pts_time:" followed by a float
    let marker = "pts_time:";
    let pos = line.find(marker)?;
    let after = &line[pos + marker.len()..];
    // Take chars that form a valid float (digits, '.', '-')
    let num_str: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    num_str.parse::<f64>().ok()
}

/// Extract a single JPEG frame from a video at the given timestamp.
fn extract_frame_at(
    ffmpeg: &Path,
    video_path: &Path,
    timestamp_secs: f64,
    output_path: &Path,
) -> Result<(), CaptureError> {
    let ts_str = format!("{:.3}", timestamp_secs);

    let output = Command::new(ffmpeg)
        .args([
            "-ss",
            &ts_str,
            "-i",
            video_path.to_str().unwrap_or(""),
            "-frames:v",
            "1",
            "-q:v",
            "2", // JPEG quality (2 = high quality)
            "-y", // overwrite
            output_path.to_str().unwrap_or(""),
        ])
        .hide_window()
        .output()
        .map_err(|e| CaptureError::FfmpegNotFound(format!("Failed to run ffmpeg: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CaptureError::FfmpegFailed {
            exit_code: output.status.code(),
            stderr,
        });
    }

    Ok(())
}

/// Capture a single frame (fallback when no scene changes detected).
fn capture_single_frame(
    ffmpeg: &Path,
    video_path: &Path,
    images_dir: &Path,
    timestamp_secs: f64,
) -> Result<Vec<CapturedFrame>, CaptureError> {
    let ts_file = format_timestamp_filename(timestamp_secs);
    let filename = format!("frame_0000_{}.jpg", ts_file);
    let output_path = images_dir.join(&filename);

    extract_frame_at(ffmpeg, video_path, timestamp_secs, &output_path)?;

    Ok(vec![CapturedFrame {
        index: 0,
        timestamp_secs,
        timestamp: format_timestamp(timestamp_secs),
        filename,
    }])
}

// ─── Interval Capture ────────────────────────────────────────────

/// Capture frames at fixed intervals throughout the video.
pub fn capture_interval(
    video_path: &Path,
    output_dir: &Path,
    interval_secs: u32,
    duration_secs: Option<f64>,
) -> Result<Vec<CapturedFrame>, CaptureError> {
    let ffmpeg = resolve_ffmpeg_path();
    let images_dir = output_dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    let duration = match duration_secs {
        Some(d) => d,
        None => probe_duration(&video_path)?,
    };

    let interval = interval_secs.max(1) as f64;
    let mut frames = Vec::new();
    let mut ts = 0.0;
    let mut idx = 0usize;

    while ts < duration {
        let ts_file = format_timestamp_filename(ts);
        let filename = format!("frame_{:04}_{}.jpg", idx, ts_file);
        let output_path = images_dir.join(&filename);

        extract_frame_at(&ffmpeg, video_path, ts, &output_path)?;

        frames.push(CapturedFrame {
            index: idx,
            timestamp_secs: ts,
            timestamp: format_timestamp(ts),
            filename,
        });

        ts += interval;
        idx += 1;
    }

    Ok(frames)
}

/// Query video duration using ffprobe.
fn probe_duration(video_path: &Path) -> Result<f64, CaptureError> {
    let ffprobe = resolve_ffprobe_path();

    let output = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            video_path.to_str().unwrap_or(""),
        ])
        .hide_window()
        .output()
        .map_err(|e| CaptureError::FfmpegNotFound(format!("Failed to run ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CaptureError::FfmpegFailed {
            exit_code: output.status.code(),
            stderr,
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .trim()
        .parse::<f64>()
        .map_err(|e| CaptureError::ParseError(format!("Cannot parse duration '{}': {e}", stdout.trim())))
}

// ─── Subtitle-Based Capture ──────────────────────────────────────

/// Result of subtitle-based capture attempt, indicating whether fallback occurred.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleCaptureResult {
    /// The captured frames.
    pub frames: Vec<CapturedFrame>,
    /// Whether fallback to scene-change mode was used.
    pub did_fallback: bool,
    /// Reason for fallback (empty if no fallback).
    pub fallback_reason: String,
    /// Parsed subtitle cues with text (empty if fallback/no subtitles).
    /// When available, these are used to associate subtitle text with
    /// captured frames. When empty, frames display timestamp only.
    #[serde(default)]
    pub cues: Vec<crate::subtitle_extractor::SubtitleCue>,
}

/// Capture frames at subtitle cue timestamps.
///
/// Looks for subtitle files (`.vtt`, `.srt`) in the same directory as the video.
/// If subtitle files are found, parses cue start times and captures a frame at
/// each unique cue time. If no subtitle files are found, automatically falls back
/// to scene-change detection with the 30% threshold.
///
/// Returns a `SubtitleCaptureResult` that includes the frames and fallback info.
pub fn capture_subtitle(
    video_path: &Path,
    output_dir: &Path,
) -> Result<SubtitleCaptureResult, CaptureError> {
    // Look for subtitle files alongside the video — parse full cues (with text)
    let subtitle_cues = find_and_parse_subtitle_cues(video_path);

    match subtitle_cues {
        Some(cues) if !cues.is_empty() => {
            // Subtitles found — extract timestamps from cues and capture frames
            let timestamps: Vec<f64> = crate::subtitle_extractor::extract_capture_timestamps(&cues);
            println!(
                "[capture] Subtitle mode: found {} cue timestamps (from {} cues)",
                timestamps.len(),
                cues.len()
            );
            let frames = capture_at_timestamps(video_path, output_dir, &timestamps)?;
            Ok(SubtitleCaptureResult {
                frames,
                did_fallback: false,
                fallback_reason: String::new(),
                cues,
            })
        }
        Some(_) => {
            // Subtitle file found but no valid cues parsed
            println!(
                "[capture] Subtitle mode: subtitle file found but no valid cues. \
                 Falling back to scene-change detection (threshold={:.0}%).",
                DEFAULT_SCENE_THRESHOLD * 100.0
            );
            let frames =
                capture_scene_change(video_path, output_dir, DEFAULT_SCENE_THRESHOLD)?;
            Ok(SubtitleCaptureResult {
                frames,
                did_fallback: true,
                fallback_reason: "Subtitle file found but contained no valid cues".to_string(),
                cues: Vec::new(),
            })
        }
        None => {
            // No subtitle files found — fall back to scene-change
            println!(
                "[capture] Subtitle mode: no subtitle files found. \
                 Falling back to scene-change detection (threshold={:.0}%).",
                DEFAULT_SCENE_THRESHOLD * 100.0
            );
            let frames =
                capture_scene_change(video_path, output_dir, DEFAULT_SCENE_THRESHOLD)?;
            Ok(SubtitleCaptureResult {
                frames,
                did_fallback: true,
                fallback_reason: "No subtitle files available".to_string(),
                cues: Vec::new(),
            })
        }
    }
}

/// Search for subtitle files (`.vtt`, `.srt`) alongside the video and parse cue timestamps.
///
/// Returns `None` if no subtitle files are found.
/// Returns `Some(vec![])` if files are found but contain no valid cues.
/// Returns `Some(timestamps)` with deduplicated, sorted cue start times in seconds.
#[cfg(test)]
fn find_and_parse_subtitles(video_path: &Path) -> Option<Vec<f64>> {
    let video_dir = video_path.parent()?;
    let video_stem = video_path.file_stem()?.to_str()?;

    // Look for subtitle files matching the video name or any subtitle file in the directory
    let mut subtitle_files: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "vtt" || ext_lower == "srt" || ext_lower == "json3" {
                    // Prefer files matching the video stem, but collect all
                    subtitle_files.push(path);
                }
            }
        }
    }

    if subtitle_files.is_empty() {
        return None;
    }

    // Sort: prioritize by (1) video stem match, (2) Korean language, (3) English language.
    // This ensures Korean subtitles are tried first, then English, matching the
    // LANGUAGE_PRIORITY in subtitle_extractor.rs (ko > en > other).
    subtitle_files.sort_by(|a, b| {
        let a_name = a.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let b_name = b.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let a_matches_stem = a_name.starts_with(video_stem);
        let b_matches_stem = b_name.starts_with(video_stem);

        // Primary sort: files matching the video stem first
        let stem_cmp = b_matches_stem.cmp(&a_matches_stem);
        if stem_cmp != std::cmp::Ordering::Equal {
            return stem_cmp;
        }

        // Secondary sort: language priority (ko > en > other)
        let lang_priority = |name: &str| -> u8 {
            let lower = name.to_lowercase();
            if lower.contains(".ko") || lower.contains("_ko") || lower.contains("-ko") {
                0 // Korean: highest priority
            } else if lower.contains(".en") || lower.contains("_en") || lower.contains("-en") {
                1 // English: second priority
            } else {
                2 // Other languages
            }
        };
        lang_priority(a_name).cmp(&lang_priority(b_name))
    });

    // Try parsing each subtitle file until we get valid timestamps
    for sub_path in &subtitle_files {
        if let Ok(content) = std::fs::read_to_string(sub_path) {
            let ext = sub_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let timestamps = match ext.as_str() {
                "vtt" => parse_vtt_timestamps(&content),
                "srt" => parse_srt_timestamps(&content),
                "json3" => parse_json3_timestamps(&content),
                _ => continue,
            };
            if !timestamps.is_empty() {
                return Some(timestamps);
            }
        }
    }

    // Files found but no valid cues parsed from any of them
    Some(Vec::new())
}

/// Search for subtitle files alongside the video and parse full subtitle cues (with text).
///
/// Similar to `find_and_parse_subtitles` but returns `SubtitleCue` objects
/// that include the subtitle text, not just timestamps. This enables
/// associating subtitle text with captured frames in the slides viewer.
///
/// Returns `None` if no subtitle files found.
/// Returns `Some(vec![])` if files found but no valid cues.
/// Returns `Some(cues)` with parsed cues including text.
fn find_and_parse_subtitle_cues(
    video_path: &Path,
) -> Option<Vec<crate::subtitle_extractor::SubtitleCue>> {
    let video_dir = video_path.parent()?;
    let video_stem = video_path.file_stem()?.to_str()?;

    // Look for subtitle files matching the video name or any subtitle file in the directory
    let mut subtitle_files: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "vtt" || ext_lower == "srt" || ext_lower == "json3" {
                    subtitle_files.push(path);
                }
            }
        }
    }

    if subtitle_files.is_empty() {
        return None;
    }

    // Sort: prioritize by (1) video stem match, (2) Korean language, (3) English language.
    subtitle_files.sort_by(|a, b| {
        let a_name = a.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let b_name = b.file_stem().and_then(|s| s.to_str()).unwrap_or("");

        let a_matches_stem = a_name.starts_with(video_stem);
        let b_matches_stem = b_name.starts_with(video_stem);

        let stem_cmp = b_matches_stem.cmp(&a_matches_stem);
        if stem_cmp != std::cmp::Ordering::Equal {
            return stem_cmp;
        }

        let lang_priority = |name: &str| -> u8 {
            let lower = name.to_lowercase();
            if lower.contains(".ko") || lower.contains("_ko") || lower.contains("-ko") {
                0
            } else if lower.contains(".en") || lower.contains("_en") || lower.contains("-en") {
                1
            } else {
                2
            }
        };
        lang_priority(a_name).cmp(&lang_priority(b_name))
    });

    // Try parsing each subtitle file until we get valid cues with text
    for sub_path in &subtitle_files {
        if let Ok(content) = std::fs::read_to_string(sub_path) {
            let ext = sub_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let cues_result = match ext.as_str() {
                "vtt" => crate::subtitle_extractor::parse_vtt(&content),
                "srt" => crate::subtitle_extractor::parse_srt(&content),
                "json3" => parse_json3_cues(&content),
                _ => continue,
            };
            if let Ok(cues) = cues_result {
                if !cues.is_empty() {
                    println!(
                        "[capture] Parsed {} subtitle cues with text from: {}",
                        cues.len(),
                        sub_path.display()
                    );
                    return Some(cues);
                }
            }
        }
    }

    // Files found but no valid cues parsed from any of them
    Some(Vec::new())
}

/// Parse WebVTT (.vtt) subtitle file for cue start timestamps.
///
/// VTT format:
/// ```text
/// WEBVTT
///
/// 00:00:01.000 --> 00:00:04.000
/// First subtitle text
///
/// 00:00:05.500 --> 00:00:08.000
/// Second subtitle text
/// ```
///
/// Returns deduplicated, sorted timestamps in seconds.
pub fn parse_vtt_timestamps(content: &str) -> Vec<f64> {
    let mut timestamps: Vec<f64> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // Match lines containing " --> " (the cue timing line)
        if let Some(arrow_pos) = trimmed.find(" --> ") {
            let start_str = &trimmed[..arrow_pos].trim();
            if let Some(secs) = parse_subtitle_timestamp(start_str) {
                // Deduplicate: skip if within 1.0s of previous timestamp
                if timestamps
                    .last()
                    .map_or(true, |&prev| (secs - prev).abs() > 1.0)
                {
                    timestamps.push(secs);
                }
            }
        }
    }

    timestamps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    timestamps
}

/// Parse SRT (.srt) subtitle file for cue start timestamps.
///
/// SRT format:
/// ```text
/// 1
/// 00:00:01,000 --> 00:00:04,000
/// First subtitle text
///
/// 2
/// 00:00:05,500 --> 00:00:08,000
/// Second subtitle text
/// ```
///
/// Returns deduplicated, sorted timestamps in seconds.
pub fn parse_srt_timestamps(content: &str) -> Vec<f64> {
    let mut timestamps: Vec<f64> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        // SRT uses comma for millisecond separator; normalize to period
        if let Some(arrow_pos) = trimmed.find(" --> ") {
            let start_str = trimmed[..arrow_pos].trim().replace(',', ".");
            if let Some(secs) = parse_subtitle_timestamp(&start_str) {
                // Deduplicate: skip if within 1.0s of previous timestamp
                if timestamps
                    .last()
                    .map_or(true, |&prev| (secs - prev).abs() > 1.0)
                {
                    timestamps.push(secs);
                }
            }
        }
    }

    timestamps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    timestamps
}

/// Parse YouTube json3 subtitle file for cue start timestamps.
///
/// json3 format (as downloaded by yt-dlp):
/// ```json
/// {"events":[{"tStartMs":0,"dDurationMs":5000,"segs":[{"utf8":"Hello"}]}, ...]}
/// ```
///
/// Returns deduplicated, sorted timestamps in seconds.
pub fn parse_json3_timestamps(content: &str) -> Vec<f64> {
    let mut timestamps: Vec<f64> = Vec::new();

    let Ok(root) = serde_json::from_str::<serde_json::Value>(content) else {
        return timestamps;
    };

    let Some(events) = root.get("events").and_then(|e| e.as_array()) else {
        return timestamps;
    };

    for event in events {
        // Skip events without segments (header/metadata events)
        if event.get("segs").is_none() {
            continue;
        }
        if let Some(start_ms) = event.get("tStartMs").and_then(|v| v.as_f64()) {
            let secs = start_ms / 1000.0;
            // Deduplicate: skip if within 1.0s of previous timestamp
            if timestamps
                .last()
                .map_or(true, |&prev| (secs - prev).abs() > 1.0)
            {
                timestamps.push(secs);
            }
        }
    }

    timestamps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    timestamps
}

/// Parse YouTube json3 subtitle file into full subtitle cues (with text).
///
/// json3 format (as downloaded by yt-dlp):
/// ```json
/// {"events":[{"tStartMs":0,"dDurationMs":5000,"segs":[{"utf8":"Hello"}]}, ...]}
/// ```
fn parse_json3_cues(content: &str) -> Result<Vec<crate::subtitle_extractor::SubtitleCue>, String> {
    let root: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("json3 parse error: {e}"))?;

    let events = root
        .get("events")
        .and_then(|e| e.as_array())
        .ok_or_else(|| "json3: missing 'events' array".to_string())?;

    let mut cues = Vec::new();

    for event in events {
        let segs = match event.get("segs").and_then(|s| s.as_array()) {
            Some(s) => s,
            None => continue, // no text segments — skip
        };

        let start_ms = match event.get("tStartMs").and_then(|v| v.as_f64()) {
            Some(v) => v,
            None => continue,
        };
        let duration_ms = event
            .get("dDurationMs")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let text: String = segs
            .iter()
            .filter_map(|seg| seg.get("utf8").and_then(|t| t.as_str()))
            .collect::<Vec<&str>>()
            .join("");

        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }

        cues.push(crate::subtitle_extractor::SubtitleCue {
            start_secs: start_ms / 1000.0,
            end_secs: (start_ms + duration_ms) / 1000.0,
            text,
        });
    }

    Ok(cues)
}

/// Parse a subtitle timestamp string into seconds.
///
/// Supports formats:
/// - `HH:MM:SS.mmm` (e.g., "00:01:23.456")
/// - `MM:SS.mmm` (e.g., "01:23.456")
/// - `HH:MM:SS,mmm` (SRT format, caller should replace comma first)
fn parse_subtitle_timestamp(ts: &str) -> Option<f64> {
    let parts: Vec<&str> = ts.split(':').collect();
    match parts.len() {
        3 => {
            // HH:MM:SS.mmm
            let hours: f64 = parts[0].parse().ok()?;
            let minutes: f64 = parts[1].parse().ok()?;
            let seconds: f64 = parts[2].parse().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        }
        2 => {
            // MM:SS.mmm
            let minutes: f64 = parts[0].parse().ok()?;
            let seconds: f64 = parts[1].parse().ok()?;
            Some(minutes * 60.0 + seconds)
        }
        _ => None,
    }
}

/// Capture frames at specific timestamps from a video.
///
/// Used by subtitle-based capture to extract frames at cue start times.
fn capture_at_timestamps(
    video_path: &Path,
    output_dir: &Path,
    timestamps: &[f64],
) -> Result<Vec<CapturedFrame>, CaptureError> {
    let ffmpeg = resolve_ffmpeg_path();
    let images_dir = output_dir.join("images");
    std::fs::create_dir_all(&images_dir)?;

    let mut frames = Vec::with_capacity(timestamps.len());

    for (idx, &ts) in timestamps.iter().enumerate() {
        let ts_file = format_timestamp_filename(ts);
        let filename = format!("frame_{:04}_{}.jpg", idx, ts_file);
        let output_path = images_dir.join(&filename);

        extract_frame_at(&ffmpeg, video_path, ts, &output_path)?;

        frames.push(CapturedFrame {
            index: idx,
            timestamp_secs: ts,
            timestamp: format_timestamp(ts),
            filename,
        });
    }

    Ok(frames)
}

// ─── Tauri Commands ──────────────────────────────────────────────

/// Run frame capture on a video file with the given options.
///
/// Returns a list of captured frames (index, timestamp, filename).
/// The caller is responsible for generating slides.html from these frames.
#[tauri::command]
pub async fn capture_frames(
    video_path: String,
    output_dir: String,
    options: CaptureOptions,
) -> Result<Vec<CapturedFrame>, String> {
    let video = PathBuf::from(&video_path);
    let out = PathBuf::from(&output_dir);

    if !video.exists() {
        return Err(format!("Video file not found: {}", video_path));
    }

    std::fs::create_dir_all(&out).map_err(|e| format!("Cannot create output dir: {e}"))?;

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<Vec<CapturedFrame>, CaptureError> {
        match options.mode.as_str() {
            "scene" => capture_scene_change(&video, &out, options.scene_threshold),
            "interval" => capture_interval(&video, &out, options.interval_seconds, None),
            "subtitle" => {
                // Subtitle mode: attempt subtitle-based capture, fall back to
                // scene-change detection with 30% threshold if no subtitles found
                let sub_result = capture_subtitle(&video, &out)?;
                if sub_result.did_fallback {
                    println!(
                        "[capture_frames] Subtitle fallback: {}",
                        sub_result.fallback_reason
                    );
                }
                Ok(sub_result.frames)
            }
            other => Err(CaptureError::ParseError(format!(
                "Unknown capture mode: {other}"
            ))),
        }
    })
    .await
    .map_err(|e| format!("Task join error: {e}"))?
    .map_err(|e| e.to_string())?;

    Ok(result)
}

/// Get the default scene-change threshold value.
#[tauri::command]
pub fn get_scene_threshold() -> f64 {
    DEFAULT_SCENE_THRESHOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "00:00:00");
        assert_eq!(format_timestamp(61.0), "00:01:01");
        assert_eq!(format_timestamp(3661.0), "01:01:01");
        assert_eq!(format_timestamp(5.7), "00:00:06"); // rounds
        assert_eq!(format_timestamp(3599.4), "00:59:59");
        assert_eq!(format_timestamp(3600.0), "01:00:00");
    }

    #[test]
    fn test_format_timestamp_filename() {
        assert_eq!(format_timestamp_filename(0.0), "00-00-00");
        assert_eq!(format_timestamp_filename(90.0), "00-01-30");
        assert_eq!(format_timestamp_filename(3661.0), "01-01-01");
    }

    #[test]
    fn test_parse_pts_time_valid() {
        let line = "[Parsed_showinfo_1 @ 0x...] n:   0 pts:  12345 pts_time:1.234567 pos:1234 fmt:yuv420p";
        assert_eq!(parse_pts_time(line), Some(1.234567));
    }

    #[test]
    fn test_parse_pts_time_integer() {
        let line = "[Parsed_showinfo_1 @ 0x...] n:   5 pts:  60000 pts_time:60 pos:5678";
        assert_eq!(parse_pts_time(line), Some(60.0));
    }

    #[test]
    fn test_parse_pts_time_zero() {
        let line = "[Parsed_showinfo_1 @ 0x...] n:   0 pts:      0 pts_time:0.000000 pos:0";
        assert_eq!(parse_pts_time(line), Some(0.0));
    }

    #[test]
    fn test_parse_pts_time_missing() {
        let line = "frame=  100 fps=24 q=-1.0 size=N/A time=00:00:04.17";
        assert_eq!(parse_pts_time(line), None);
    }

    #[test]
    fn test_parse_pts_time_empty() {
        assert_eq!(parse_pts_time(""), None);
    }

    #[test]
    fn test_default_threshold() {
        assert!((DEFAULT_SCENE_THRESHOLD - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn test_capture_options_default() {
        let opts = CaptureOptions::default();
        assert_eq!(opts.mode, "scene");
        assert!((opts.scene_threshold - 0.30).abs() < f64::EPSILON);
        assert_eq!(opts.interval_seconds, 10);
    }

    #[test]
    fn test_capture_options_serde_roundtrip() {
        let opts = CaptureOptions {
            mode: "scene".to_string(),
            scene_threshold: 0.30,
            interval_seconds: 10,
        };
        let json = serde_json::to_string(&opts).unwrap();
        let loaded: CaptureOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mode, "scene");
        assert!((loaded.scene_threshold - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn test_capture_options_serde_defaults() {
        // When scene_threshold is missing, should default to 0.30
        let json = r#"{"mode":"scene"}"#;
        let opts: CaptureOptions = serde_json::from_str(json).unwrap();
        assert!((opts.scene_threshold - 0.30).abs() < f64::EPSILON);
        assert_eq!(opts.interval_seconds, 10);
    }

    #[test]
    fn test_captured_frame_serde() {
        let frame = CapturedFrame {
            index: 0,
            timestamp_secs: 1.5,
            timestamp: "00:00:02".to_string(),
            filename: "frame_0000_00-00-02.jpg".to_string(),
        };
        let json = serde_json::to_string(&frame).unwrap();
        assert!(json.contains("frame_0000_00-00-02.jpg"));
        let loaded: CapturedFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.index, 0);
        assert!((loaded.timestamp_secs - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_threshold_clamping() {
        // The capture_scene_change function clamps threshold to [0.01, 0.99]
        // We can verify this by testing the clamping logic directly
        let t1: f64 = 0.0_f64.clamp(0.01, 0.99);
        assert!((t1 - 0.01).abs() < f64::EPSILON);

        let t2: f64 = 1.5_f64.clamp(0.01, 0.99);
        assert!((t2 - 0.99).abs() < f64::EPSILON);

        let t3: f64 = 0.30_f64.clamp(0.01, 0.99);
        assert!((t3 - 0.30).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resolve_ffmpeg_path_fallback() {
        // Should at least return "ffmpeg" as fallback
        let path = resolve_ffmpeg_path();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name == "ffmpeg" || name == "ffmpeg.exe",
            "Expected ffmpeg binary name, got: {}",
            name
        );
    }

    #[test]
    fn test_resolve_ffprobe_path_fallback() {
        let path = resolve_ffprobe_path();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(
            name == "ffprobe" || name == "ffprobe.exe",
            "Expected ffprobe binary name, got: {}",
            name
        );
    }

    #[test]
    fn test_capture_error_display() {
        let err = CaptureError::FfmpegNotFound("not on path".to_string());
        assert!(err.to_string().contains("not on path"));

        let err = CaptureError::FfmpegFailed {
            exit_code: Some(1),
            stderr: "error msg".to_string(),
        };
        assert!(err.to_string().contains("error msg"));

        let err = CaptureError::ParseError("bad format".to_string());
        assert!(err.to_string().contains("bad format"));
    }

    #[test]
    fn test_deduplicate_close_timestamps() {
        // Simulate the deduplication logic from detect_scene_changes
        let raw_timestamps = vec![0.0, 0.3, 0.4, 5.0, 5.1, 10.0, 10.6];
        let mut deduped: Vec<f64> = Vec::new();
        for ts in &raw_timestamps {
            if deduped.last().map_or(true, |prev: &f64| (ts - prev).abs() > 0.5) {
                deduped.push(*ts);
            }
        }
        // 0.0, 0.3 (skip — within 0.5 of 0.0), 0.4 (skip), 5.0, 5.1 (skip), 10.0, 10.6
        assert_eq!(deduped, vec![0.0, 5.0, 10.0, 10.6]);
    }

    #[test]
    fn test_parse_multiple_showinfo_lines() {
        let stderr = r#"
[Parsed_showinfo_1 @ 0x5630c] n:   0 pts:      0 pts_time:0.000000 pos:0 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:1 type:I cmb:0 poc:0
frame=    1 fps=0.0 q=-0.0 size=N/A time=00:00:00.04 bitrate=N/A speed=N/A
[Parsed_showinfo_1 @ 0x5630c] n:   1 pts:  72072 pts_time:3.003000 pos:123456 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:0 type:P cmb:0 poc:2
frame=   75 fps=24 q=-0.0 size=N/A time=00:00:03.12 bitrate=N/A speed=3.5x
[Parsed_showinfo_1 @ 0x5630c] n:   2 pts: 360360 pts_time:15.015000 pos:567890 fmt:yuv420p sar:1/1 s:1920x1080 i:P iskey:1 type:I cmb:0 poc:4
"#;
        let mut timestamps: Vec<f64> = Vec::new();
        for line in stderr.lines() {
            if let Some(ts) = parse_pts_time(line) {
                if timestamps.last().map_or(true, |&prev| (ts - prev).abs() > 0.5) {
                    timestamps.push(ts);
                }
            }
        }
        assert_eq!(timestamps.len(), 3);
        assert!((timestamps[0] - 0.0).abs() < 0.001);
        assert!((timestamps[1] - 3.003).abs() < 0.001);
        assert!((timestamps[2] - 15.015).abs() < 0.001);
    }

    // ─── Subtitle Parsing Tests ──────────────────────────────────

    #[test]
    fn test_parse_subtitle_timestamp_hhmmss() {
        assert_eq!(parse_subtitle_timestamp("00:00:00.000"), Some(0.0));
        assert_eq!(parse_subtitle_timestamp("00:01:23.456"), Some(83.456));
        assert_eq!(parse_subtitle_timestamp("01:00:00.000"), Some(3600.0));
        assert_eq!(parse_subtitle_timestamp("01:30:45.500"), Some(5445.5));
    }

    #[test]
    fn test_parse_subtitle_timestamp_mmss() {
        assert_eq!(parse_subtitle_timestamp("01:23.456"), Some(83.456));
        assert_eq!(parse_subtitle_timestamp("00:05.000"), Some(5.0));
    }

    #[test]
    fn test_parse_subtitle_timestamp_invalid() {
        assert_eq!(parse_subtitle_timestamp(""), None);
        assert_eq!(parse_subtitle_timestamp("abc"), None);
        assert_eq!(parse_subtitle_timestamp("1:2:3:4"), None);
    }

    #[test]
    fn test_parse_vtt_timestamps_basic() {
        let vtt = "\
WEBVTT

00:00:01.000 --> 00:00:04.000
Hello world

00:00:05.500 --> 00:00:08.000
Second cue

00:00:15.000 --> 00:00:18.000
Third cue
";
        let timestamps = parse_vtt_timestamps(vtt);
        assert_eq!(timestamps.len(), 3);
        assert!((timestamps[0] - 1.0).abs() < 0.001);
        assert!((timestamps[1] - 5.5).abs() < 0.001);
        assert!((timestamps[2] - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_vtt_timestamps_deduplication() {
        // Cues within 1.0s of each other should be deduplicated
        let vtt = "\
WEBVTT

00:00:01.000 --> 00:00:02.000
First

00:00:01.500 --> 00:00:02.500
Too close to first

00:00:10.000 --> 00:00:12.000
Far enough
";
        let timestamps = parse_vtt_timestamps(vtt);
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 1.0).abs() < 0.001);
        assert!((timestamps[1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_vtt_timestamps_empty() {
        let vtt = "WEBVTT\n\n";
        let timestamps = parse_vtt_timestamps(vtt);
        assert!(timestamps.is_empty());
    }

    #[test]
    fn test_parse_srt_timestamps_basic() {
        let srt = "\
1
00:00:01,000 --> 00:00:04,000
Hello world

2
00:00:05,500 --> 00:00:08,000
Second cue

3
00:01:00,000 --> 00:01:05,000
Third cue
";
        let timestamps = parse_srt_timestamps(srt);
        assert_eq!(timestamps.len(), 3);
        assert!((timestamps[0] - 1.0).abs() < 0.001);
        assert!((timestamps[1] - 5.5).abs() < 0.001);
        assert!((timestamps[2] - 60.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_srt_timestamps_deduplication() {
        let srt = "\
1
00:00:01,000 --> 00:00:02,000
First

2
00:00:01,800 --> 00:00:02,500
Too close

3
00:00:10,000 --> 00:00:12,000
Far enough
";
        let timestamps = parse_srt_timestamps(srt);
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 1.0).abs() < 0.001);
        assert!((timestamps[1] - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_srt_timestamps_empty() {
        let timestamps = parse_srt_timestamps("");
        assert!(timestamps.is_empty());
    }

    #[test]
    fn test_parse_vtt_with_style_header() {
        // VTT files can have STYLE and NOTE blocks before cues
        let vtt = "\
WEBVTT

STYLE
::cue {
  color: white;
}

NOTE This is a comment

00:00:05.000 --> 00:00:10.000
First real cue

00:00:20.000 --> 00:00:25.000
Second cue
";
        let timestamps = parse_vtt_timestamps(vtt);
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 5.0).abs() < 0.001);
        assert!((timestamps[1] - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_subtitle_capture_result_serialization() {
        let result = SubtitleCaptureResult {
            frames: vec![CapturedFrame {
                index: 0,
                timestamp_secs: 1.0,
                timestamp: "00:00:01".to_string(),
                filename: "frame_0000_00-00-01.jpg".to_string(),
            }],
            did_fallback: true,
            fallback_reason: "No subtitle files available".to_string(),
            cues: Vec::new(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("did_fallback"));
        assert!(json.contains("true"));
        assert!(json.contains("No subtitle files available"));

        let loaded: SubtitleCaptureResult = serde_json::from_str(&json).unwrap();
        assert!(loaded.did_fallback);
        assert_eq!(loaded.frames.len(), 1);
        assert!(loaded.cues.is_empty());
    }

    #[test]
    fn test_subtitle_capture_result_with_cues() {
        use crate::subtitle_extractor::SubtitleCue;
        let result = SubtitleCaptureResult {
            frames: vec![CapturedFrame {
                index: 0,
                timestamp_secs: 1.0,
                timestamp: "00:00:01".to_string(),
                filename: "frame_0000_00-00-01.jpg".to_string(),
            }],
            did_fallback: false,
            fallback_reason: String::new(),
            cues: vec![SubtitleCue {
                start_secs: 1.0,
                end_secs: 4.0,
                text: "안녕하세요".to_string(),
            }],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("안녕하세요"));
        assert!(!result.did_fallback);

        let loaded: SubtitleCaptureResult = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.cues.len(), 1);
        assert_eq!(loaded.cues[0].text, "안녕하세요");
    }

    #[test]
    fn test_subtitle_capture_result_deserialize_without_cues_field() {
        // Backward compatibility: old JSON without "cues" field should deserialize
        // with empty cues thanks to #[serde(default)]
        let json = r#"{"frames":[],"did_fallback":false,"fallback_reason":""}"#;
        let loaded: SubtitleCaptureResult = serde_json::from_str(json).unwrap();
        assert!(loaded.cues.is_empty());
    }

    #[test]
    fn test_find_subtitles_no_directory() {
        // A video path with no parent dir should return None
        let result = find_and_parse_subtitles(Path::new("nonexistent_video.mp4"));
        // The parent exists (current dir), but no subtitle files should be found
        // in the test environment, so None is expected
        assert!(result.is_none() || result.as_ref().map_or(false, |v| v.is_empty()));
    }

    #[test]
    fn test_find_subtitles_with_temp_dir() {
        // Create a temp directory with a VTT file to test file discovery
        let temp_dir = std::env::temp_dir().join("framepick_test_subtitles");
        let _ = std::fs::create_dir_all(&temp_dir);

        let video_path = temp_dir.join("test_video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        let vtt_path = temp_dir.join("test_video.en.vtt");
        let vtt_content = "\
WEBVTT

00:00:03.000 --> 00:00:06.000
Hello

00:00:10.000 --> 00:00:14.000
World
";
        std::fs::write(&vtt_path, vtt_content).unwrap();

        let result = find_and_parse_subtitles(&video_path);
        assert!(result.is_some());
        let timestamps = result.unwrap();
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 3.0).abs() < 0.001);
        assert!((timestamps[1] - 10.0).abs() < 0.001);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_subtitles_srt_format() {
        let temp_dir = std::env::temp_dir().join("framepick_test_srt");
        let _ = std::fs::create_dir_all(&temp_dir);

        let video_path = temp_dir.join("my_video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        let srt_path = temp_dir.join("my_video.ko.srt");
        let srt_content = "\
1
00:00:02,000 --> 00:00:05,000
안녕하세요

2
00:00:08,500 --> 00:00:12,000
자막 테스트
";
        std::fs::write(&srt_path, srt_content).unwrap();

        let result = find_and_parse_subtitles(&video_path);
        assert!(result.is_some());
        let timestamps = result.unwrap();
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 2.0).abs() < 0.001);
        assert!((timestamps[1] - 8.5).abs() < 0.001);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_subtitles_empty_subtitle_file() {
        let temp_dir = std::env::temp_dir().join("framepick_test_empty_sub");
        let _ = std::fs::create_dir_all(&temp_dir);

        let video_path = temp_dir.join("video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        // Write an empty VTT file
        let vtt_path = temp_dir.join("video.vtt");
        std::fs::write(&vtt_path, "WEBVTT\n\n").unwrap();

        let result = find_and_parse_subtitles(&video_path);
        // Should return Some(empty vec) — file found but no cues
        assert!(result.is_some());
        assert!(result.unwrap().is_empty());

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_subtitles_prioritizes_korean_over_english() {
        // When both Korean and English subtitle files exist, Korean should be used first
        let temp_dir = std::env::temp_dir().join("framepick_test_ko_priority");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let video_path = temp_dir.join("video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        // English subtitle file with different timestamps
        let en_srt = temp_dir.join("video.en.srt");
        std::fs::write(&en_srt, "\
1
00:00:10,000 --> 00:00:15,000
English subtitle
").unwrap();

        // Korean subtitle file — should be prioritized
        let ko_srt = temp_dir.join("video.ko.srt");
        std::fs::write(&ko_srt, "\
1
00:00:02,000 --> 00:00:05,000
한국어 자막

2
00:00:20,000 --> 00:00:25,000
두 번째 자막
").unwrap();

        let result = find_and_parse_subtitles(&video_path);
        assert!(result.is_some());
        let timestamps = result.unwrap();
        // Should use Korean timestamps (2.0 and 20.0), not English (10.0)
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 2.0).abs() < 0.001, "First timestamp should be 2.0 (Korean), got {}", timestamps[0]);
        assert!((timestamps[1] - 20.0).abs() < 0.001, "Second timestamp should be 20.0 (Korean), got {}", timestamps[1]);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_subtitles_falls_back_to_english() {
        // When only English subtitles exist (no Korean), English should be used
        let temp_dir = std::env::temp_dir().join("framepick_test_en_fallback");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let video_path = temp_dir.join("video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        let en_srt = temp_dir.join("video.en.srt");
        std::fs::write(&en_srt, "\
1
00:00:05,000 --> 00:00:08,000
English only

2
00:00:15,000 --> 00:00:18,000
Second English cue
").unwrap();

        let result = find_and_parse_subtitles(&video_path);
        assert!(result.is_some());
        let timestamps = result.unwrap();
        assert_eq!(timestamps.len(), 2);
        assert!((timestamps[0] - 5.0).abs() < 0.001);
        assert!((timestamps[1] - 15.0).abs() < 0.001);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_find_subtitles_english_before_other_languages() {
        // English should be prioritized over other non-Korean languages
        let temp_dir = std::env::temp_dir().join("framepick_test_en_over_other");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let video_path = temp_dir.join("video.mp4");
        std::fs::write(&video_path, b"fake video").unwrap();

        // Japanese subtitle file
        let ja_srt = temp_dir.join("video.ja.srt");
        std::fs::write(&ja_srt, "\
1
00:00:30,000 --> 00:00:35,000
日本語字幕
").unwrap();

        // English subtitle file — should be prioritized over Japanese
        let en_srt = temp_dir.join("video.en.srt");
        std::fs::write(&en_srt, "\
1
00:00:07,000 --> 00:00:12,000
English subtitle
").unwrap();

        let result = find_and_parse_subtitles(&video_path);
        assert!(result.is_some());
        let timestamps = result.unwrap();
        assert_eq!(timestamps.len(), 1);
        // English (7.0) should be used before Japanese (30.0)
        assert!((timestamps[0] - 7.0).abs() < 0.001, "Should use English subtitle timestamp 7.0, got {}", timestamps[0]);

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ─── Interval Capture Tests ──────────────────────────────────

    #[test]
    fn test_capture_options_interval_mode_serde() {
        // Verify interval mode options serialize/deserialize correctly
        let opts = CaptureOptions {
            mode: "interval".to_string(),
            scene_threshold: DEFAULT_SCENE_THRESHOLD,
            interval_seconds: 30,
        };
        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains("\"interval\""));
        assert!(json.contains("30"));

        let loaded: CaptureOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mode, "interval");
        assert_eq!(loaded.interval_seconds, 30);
    }

    #[test]
    fn test_capture_options_interval_presets() {
        // Test all three preset values: 10, 30, 60
        for &preset in &[10u32, 30, 60] {
            let opts = CaptureOptions {
                mode: "interval".to_string(),
                scene_threshold: DEFAULT_SCENE_THRESHOLD,
                interval_seconds: preset,
            };
            let json = serde_json::to_string(&opts).unwrap();
            let loaded: CaptureOptions = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded.interval_seconds, preset, "Preset {} should roundtrip", preset);
        }
    }

    #[test]
    fn test_capture_options_custom_interval_values() {
        // Test custom interval values beyond presets (1-3600 range)
        for &custom in &[1u32, 5, 15, 45, 90, 120, 300, 600, 1800, 3600] {
            let opts = CaptureOptions {
                mode: "interval".to_string(),
                scene_threshold: DEFAULT_SCENE_THRESHOLD,
                interval_seconds: custom,
            };
            let json = serde_json::to_string(&opts).unwrap();
            let loaded: CaptureOptions = serde_json::from_str(&json).unwrap();
            assert_eq!(loaded.interval_seconds, custom, "Custom value {} should roundtrip", custom);
        }
    }

    #[test]
    fn test_interval_timestamp_generation() {
        // Simulate the interval capture timestamp logic
        let duration = 65.0; // 1 minute 5 seconds
        let interval = 10u32;
        let interval_f = interval.max(1) as f64;

        let mut timestamps = Vec::new();
        let mut ts = 0.0;
        while ts < duration {
            timestamps.push(ts);
            ts += interval_f;
        }

        // At 10s intervals over 65s: 0, 10, 20, 30, 40, 50, 60
        assert_eq!(timestamps.len(), 7);
        assert!((timestamps[0] - 0.0).abs() < f64::EPSILON);
        assert!((timestamps[1] - 10.0).abs() < f64::EPSILON);
        assert!((timestamps[6] - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_interval_30s_timestamp_generation() {
        let duration = 125.0;
        let interval = 30u32;
        let interval_f = interval.max(1) as f64;

        let mut timestamps = Vec::new();
        let mut ts = 0.0;
        while ts < duration {
            timestamps.push(ts);
            ts += interval_f;
        }

        // At 30s intervals over 125s: 0, 30, 60, 90, 120
        assert_eq!(timestamps.len(), 5);
        assert!((timestamps[4] - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_interval_60s_timestamp_generation() {
        let duration = 300.0; // 5 minutes
        let interval = 60u32;
        let interval_f = interval.max(1) as f64;

        let mut timestamps = Vec::new();
        let mut ts = 0.0;
        while ts < duration {
            timestamps.push(ts);
            ts += interval_f;
        }

        // At 60s intervals over 300s: 0, 60, 120, 180, 240
        assert_eq!(timestamps.len(), 5);
    }

    #[test]
    fn test_interval_custom_15s_timestamp_generation() {
        let duration = 50.0;
        let interval = 15u32;
        let interval_f = interval.max(1) as f64;

        let mut timestamps = Vec::new();
        let mut ts = 0.0;
        while ts < duration {
            timestamps.push(ts);
            ts += interval_f;
        }

        // At 15s intervals over 50s: 0, 15, 30, 45
        assert_eq!(timestamps.len(), 4);
        assert!((timestamps[3] - 45.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_interval_minimum_1s() {
        // Minimum interval is clamped to 1s
        let interval = 0u32;
        let clamped = interval.max(1) as f64;
        assert!((clamped - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_interval_filenames_sequential() {
        // Verify that interval frames get sequential filenames
        let duration = 25.0;
        let interval = 10u32;
        let interval_f = interval.max(1) as f64;

        let mut filenames = Vec::new();
        let mut ts = 0.0;
        let mut idx = 0usize;

        while ts < duration {
            let ts_file = format_timestamp_filename(ts);
            let filename = format!("frame_{:04}_{}.jpg", idx, ts_file);
            filenames.push(filename);
            ts += interval_f;
            idx += 1;
        }

        assert_eq!(filenames.len(), 3);
        assert_eq!(filenames[0], "frame_0000_00-00-00.jpg");
        assert_eq!(filenames[1], "frame_0001_00-00-10.jpg");
        assert_eq!(filenames[2], "frame_0002_00-00-20.jpg");
    }
}
