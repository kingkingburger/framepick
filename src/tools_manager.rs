//! Tools manager — auto-downloads yt-dlp and ffmpeg on first run.
//!
//! On startup, call [`setup_tools`] to ensure both binaries exist in the
//! `tools/` directory next to the executable. Progress is emitted via
//! `tools:status` Tauri events so the frontend can show a loading overlay.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

// ─── Download URLs ────────────────────────────────────────────────────────────

const YTDLP_URL: &str =
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe";

const FFMPEG_ZIP_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/latest/download/ffmpeg-master-latest-win64-gpl.zip";

// ─── Public types ─────────────────────────────────────────────────────────────

/// Aggregate status of both tools after [`setup_tools`] completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsStatus {
    pub ytdlp_ready: bool,
    pub ffmpeg_ready: bool,
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
    pub ytdlp_version: Option<String>,
}

/// Result of an update-availability check for yt-dlp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: Option<String>,
    pub latest_version: Option<String>,
}

// ─── Progress event payload ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
struct ToolProgress<'a> {
    tool: &'a str,
    status: &'a str,
    progress: u8,
    message: &'a str,
}

fn emit_progress(app: &AppHandle, tool: &str, status: &str, progress: u8, message: &str) {
    let _ = app.emit(
        "tools:status",
        ToolProgress { tool, status, progress, message },
    );
}

// ─── Directory helpers ────────────────────────────────────────────────────────

/// Returns the `tools/` directory next to the running executable.
pub fn tools_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("tools");
        }
    }
    PathBuf::from("tools")
}

/// Returns the full path to `name` inside `tools/`.
pub fn tool_path(name: &str) -> PathBuf {
    tools_dir().join(name)
}

/// Returns `true` if the binary exists inside `tools/`.
pub fn tool_exists(name: &str) -> bool {
    tool_path(name).exists()
}

// ─── yt-dlp version helpers ───────────────────────────────────────────────────

fn version_file_path() -> PathBuf {
    tools_dir().join("ytdlp-version.txt")
}

fn read_stored_version() -> Option<String> {
    std::fs::read_to_string(version_file_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_stored_version(version: &str) {
    let _ = std::fs::write(version_file_path(), version);
}

// ─── GitHub latest-release tag helper ────────────────────────────────────────

/// Fetch the latest release tag name for a GitHub repo.
///
/// Uses the GitHub redirect: `releases/latest` → `releases/tag/<tag>`.
async fn fetch_latest_tag(owner: &str, repo: &str) -> Result<String, String> {
    let url = format!("https://github.com/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    // GitHub returns 302 to /releases/tag/<tag>
    if let Some(loc) = resp.headers().get("location") {
        let loc = loc.to_str().unwrap_or("");
        if let Some(tag) = loc.split("/tag/").nth(1) {
            return Ok(tag.to_string());
        }
    }

    Err("Could not determine latest release tag".to_string())
}

// ─── Download helper ──────────────────────────────────────────────────────────

/// Download a URL to `dest`, emitting `tools:status` progress events.
async fn download_to_file(
    app: &AppHandle,
    tool_name: &str,
    url: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;

    emit_progress(app, tool_name, "downloading", 0, "Starting download...");

    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed with HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("Failed to create file {}: {e}", dest.display()))?;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Download stream error: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write to file: {e}"))?;
        downloaded += chunk.len() as u64;

        if total > 0 {
            let pct = ((downloaded * 100) / total).min(99) as u8;
            emit_progress(
                app,
                tool_name,
                "downloading",
                pct,
                &format!("Downloading... {pct}%"),
            );
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush file: {e}"))?;

    emit_progress(app, tool_name, "downloading", 100, "Download complete");
    Ok(())
}

// ─── yt-dlp ───────────────────────────────────────────────────────────────────

/// Ensure `yt-dlp.exe` exists in `tools/`. Downloads if missing.
/// Returns the path to the binary.
pub async fn ensure_ytdlp(app: &AppHandle) -> Result<PathBuf, String> {
    let exe_name = "yt-dlp.exe";
    let dest = tool_path(exe_name);

    emit_progress(app, "yt-dlp", "checking", 0, "Checking yt-dlp...");

    if dest.exists() {
        emit_progress(app, "yt-dlp", "ready", 100, "yt-dlp is ready");
        return Ok(dest);
    }

    // Create tools/ directory if needed
    std::fs::create_dir_all(tools_dir())
        .map_err(|e| format!("Failed to create tools directory: {e}"))?;

    download_to_file(app, "yt-dlp", YTDLP_URL, &dest).await?;

    // Fetch and store version tag
    emit_progress(app, "yt-dlp", "checking", 100, "Fetching version info...");
    if let Ok(tag) = fetch_latest_tag("yt-dlp", "yt-dlp").await {
        write_stored_version(&tag);
    }

    emit_progress(app, "yt-dlp", "ready", 100, "yt-dlp installed");
    Ok(dest)
}

// ─── ffmpeg ───────────────────────────────────────────────────────────────────

/// Ensure `ffmpeg.exe` and `ffprobe.exe` exist in `tools/`. Downloads and extracts if missing.
/// Returns the path to `ffmpeg.exe`.
pub async fn ensure_ffmpeg(app: &AppHandle) -> Result<PathBuf, String> {
    let ffmpeg_dest = tool_path("ffmpeg.exe");
    let ffprobe_dest = tool_path("ffprobe.exe");

    emit_progress(app, "ffmpeg", "checking", 0, "Checking ffmpeg...");

    if ffmpeg_dest.exists() && ffprobe_dest.exists() {
        emit_progress(app, "ffmpeg", "ready", 100, "ffmpeg is ready");
        return Ok(ffmpeg_dest);
    }

    // Create tools/ directory if needed
    std::fs::create_dir_all(tools_dir())
        .map_err(|e| format!("Failed to create tools directory: {e}"))?;

    // Download zip to a temp file inside tools/
    let zip_path = tools_dir().join("ffmpeg-download.zip");
    download_to_file(app, "ffmpeg", FFMPEG_ZIP_URL, &zip_path).await?;

    // Extract ffmpeg.exe and ffprobe.exe from the zip
    emit_progress(app, "ffmpeg", "extracting", 50, "Extracting ffmpeg & ffprobe...");
    extract_binaries_from_zip(&zip_path, &[
        ("ffmpeg.exe", &ffmpeg_dest),
        ("ffprobe.exe", &ffprobe_dest),
    ])?;

    // Clean up zip
    let _ = std::fs::remove_file(&zip_path);

    emit_progress(app, "ffmpeg", "ready", 100, "ffmpeg installed");
    Ok(ffmpeg_dest)
}

/// Extract multiple binaries from `*/bin/<name>` inside the BtbN zip archive.
///
/// `targets` is a slice of `(binary_name, destination_path)` pairs.
fn extract_binaries_from_zip(
    zip_path: &std::path::Path,
    targets: &[(&str, &std::path::Path)],
) -> Result<(), String> {
    let zip_file = std::fs::File::open(zip_path)
        .map_err(|e| format!("Failed to open zip: {e}"))?;
    let mut archive = zip::ZipArchive::new(zip_file)
        .map_err(|e| format!("Failed to read zip archive: {e}"))?;

    let mut found: Vec<bool> = vec![false; targets.len()];

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {i}: {e}"))?;
        let name = entry.name().to_string();

        for (idx, (bin_name, dest)) in targets.iter().enumerate() {
            if found[idx] {
                continue;
            }
            let suffix = format!("/bin/{bin_name}");
            if name.ends_with(&suffix) || name == format!("bin/{bin_name}") {
                let mut out = std::fs::File::create(dest)
                    .map_err(|e| format!("Failed to create {bin_name}: {e}"))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| format!("Failed to extract {bin_name}: {e}"))?;
                out.flush()
                    .map_err(|e| format!("Failed to flush {bin_name}: {e}"))?;
                found[idx] = true;
                break;
            }
        }

        if found.iter().all(|&f| f) {
            return Ok(());
        }
    }

    let missing: Vec<&str> = targets
        .iter()
        .zip(found.iter())
        .filter(|(_, &f)| !f)
        .map(|((name, _), _)| *name)
        .collect();

    if !missing.is_empty() {
        return Err(format!(
            "{} not found inside the downloaded zip archive",
            missing.join(", ")
        ));
    }

    Ok(())
}

// ─── Tauri commands ───────────────────────────────────────────────────────────

/// Check all tools and download missing ones. Emits `tools:status` events.
///
/// Called at app startup from the frontend.
#[tauri::command]
pub async fn setup_tools(app: AppHandle) -> Result<ToolsStatus, String> {
    // Ensure both tools exist (downloads happen in sequence to avoid saturating
    // bandwidth and to keep the progress bar easy to understand)
    let ytdlp_result = ensure_ytdlp(&app).await;
    let ffmpeg_result = ensure_ffmpeg(&app).await;

    let ytdlp_path = ytdlp_result.unwrap_or_default();
    let ffmpeg_path = ffmpeg_result.unwrap_or_default();

    let ytdlp_ready = ytdlp_path.exists();
    let ffmpeg_ready = ffmpeg_path.exists();

    let ytdlp_version = read_stored_version();

    Ok(ToolsStatus {
        ytdlp_ready,
        ffmpeg_ready,
        ytdlp_path: ytdlp_path.to_string_lossy().into_owned(),
        ffmpeg_path: ffmpeg_path.to_string_lossy().into_owned(),
        ytdlp_version,
    })
}

/// Check whether a newer yt-dlp release is available on GitHub.
#[tauri::command]
pub async fn check_ytdlp_update(_app: AppHandle) -> Result<UpdateInfo, String> {
    let current = read_stored_version();

    let latest = fetch_latest_tag("yt-dlp", "yt-dlp").await.ok();

    let update_available = match (&current, &latest) {
        (Some(c), Some(l)) => c != l,
        (None, Some(_)) => true,
        _ => false,
    };

    Ok(UpdateInfo {
        update_available,
        current_version: current,
        latest_version: latest,
    })
}

/// Update yt-dlp to the latest version.
#[tauri::command]
pub async fn update_ytdlp(app: AppHandle) -> Result<(), String> {
    let dest = tool_path("yt-dlp.exe");

    // Remove existing binary so ensure_ytdlp re-downloads it
    if dest.exists() {
        std::fs::remove_file(&dest)
            .map_err(|e| format!("Failed to remove old yt-dlp.exe: {e}"))?;
        // Also clear version file so the new tag gets written fresh
        let _ = std::fs::remove_file(version_file_path());
    }

    ensure_ytdlp(&app).await?;
    Ok(())
}

// ─── Path resolution helpers (used by metadata.rs / capture.rs) ──────────────

/// Resolve yt-dlp: tools/ first, then next to exe, then PATH.
pub fn resolve_ytdlp_path() -> PathBuf {
    // 1. tools/ directory
    let in_tools = tool_path("yt-dlp.exe");
    if in_tools.exists() {
        return in_tools;
    }
    // 2. Next to exe (backward compat / portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" });
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // 3. System PATH
    PathBuf::from(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" })
}

/// Resolve ffmpeg: tools/ first, then next to exe, then PATH.
pub fn resolve_ffmpeg_path() -> PathBuf {
    // 1. tools/ directory
    let in_tools = tool_path("ffmpeg.exe");
    if in_tools.exists() {
        return in_tools;
    }
    // 2. Next to exe (backward compat / portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" });
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // 3. System PATH
    PathBuf::from("ffmpeg")
}

/// Resolve ffprobe: tools/ first, then next to exe, then PATH.
pub fn resolve_ffprobe_path() -> PathBuf {
    // 1. tools/ directory
    let in_tools = tool_path("ffprobe.exe");
    if in_tools.exists() {
        return in_tools;
    }
    // 2. Next to exe (backward compat / portable)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
            if candidate.exists() {
                return candidate;
            }
        }
    }
    // 3. System PATH
    PathBuf::from("ffprobe")
}
