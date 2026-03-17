//! 다크 테마 뷰어 레이아웃의 slides.html 생성 모듈.
//!
//! 출력은 다음을 포함하는 완전 독립형 HTML 파일이다:
//! - 타임스탬프 탐색이 가능한 사이드바 목차(TOC)
//! - 자막 텍스트와 함께 캡쳐 프레임을 표시하는 메인 콘텐츠 영역
//! - 슬라이드 간 키보드 탐색 (←/→, j/k)
//! - 좁은 화면에서 사이드바가 접히는 반응형 레이아웃
//! - 앱 UI와 일관된 다크 테마

use crate::capture::CapturedFrame;
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io;
use std::path::Path;

/// 캡쳐된 프레임 하나의 세그먼트.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// 세그먼트의 0-기반 인덱스.
    pub index: usize,
    /// 표시용 타임스탬프, 예: "00:01:23".
    pub timestamp: String,
    /// 이 프레임의 자막/캡션 텍스트.
    pub text: String,
    /// 이미지 파일명 (`images/` 디렉토리 기준 상대 경로), 예: "frame_0001_00-01-23.jpg".
    pub image: String,
}

/// 원본 영상의 메타데이터.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub video_id: String,
}

// ─── CapturedFrame → Segment Conversion ──────────────────────────

impl Segment {
    /// 자막 텍스트가 있는 캡쳐 프레임에서 세그먼트를 생성한다.
    ///
    /// 자막이 있을 때(자막 캡쳐 모드) 사용한다.
    pub fn from_frame_with_text(frame: &CapturedFrame, text: String) -> Self {
        Self {
            index: frame.index,
            timestamp: frame.timestamp.clone(),
            text,
            image: frame.filename.clone(),
        }
    }

    /// 자막 텍스트 없이 캡쳐 프레임에서 세그먼트를 생성한다.
    ///
    /// text 필드는 `[00:01:30]` 형식의 타임스탬프로 설정되며,
    /// 장면 변화 또는 고정 간격 모드로 캡쳐된 프레임에서
    /// 슬라이드 뷰어의 시각적 마커 역할을 한다.
    pub fn from_frame_timestamp_only(frame: &CapturedFrame) -> Self {
        Self {
            index: frame.index,
            timestamp: frame.timestamp.clone(),
            text: format!("[{}]", frame.timestamp),
            image: frame.filename.clone(),
        }
    }
}

/// 자막 텍스트 없이 캡쳐 프레임 목록을 세그먼트로 변환한다.
///
/// 각 세그먼트의 text는 `[HH:MM:SS]` 타임스탬프 형식으로 설정된다.
/// 자막 큐가 없는 장면 변화 및 고정 간격 캡쳐 모드에서 사용한다.
pub fn frames_to_segments(frames: &[CapturedFrame]) -> Vec<Segment> {
    frames
        .iter()
        .map(Segment::from_frame_timestamp_only)
        .collect()
}

/// 자막 큐 텍스트를 사용해 캡쳐 프레임을 세그먼트로 변환한다.
///
/// 각 프레임을 시작 시간 기준으로 가장 가까운 자막 큐와 매칭한다
/// (2초 허용 오차 범위). 매칭되는 큐가 없는 프레임은 `[HH:MM:SS]` 형식으로 폴백한다.
pub fn frames_to_segments_with_subtitles(
    frames: &[CapturedFrame],
    cues: &[crate::subtitle_extractor::SubtitleCue],
) -> Vec<Segment> {
    frames
        .iter()
        .map(|frame| {
            // Find the closest subtitle cue within 2 seconds of this frame's timestamp
            let matched_text = cues.iter().find(|cue| {
                (frame.timestamp_secs - cue.start_secs).abs() < 2.0
            });

            match matched_text {
                Some(cue) if !cue.text.trim().is_empty() => {
                    Segment::from_frame_with_text(frame, cue.text.clone())
                }
                _ => Segment::from_frame_timestamp_only(frame),
            }
        })
        .collect()
}

/// 슬라이드 생성 오류.
#[derive(Debug)]
pub enum GeneratorError {
    Io(io::Error),
    Json(serde_json::Error),
    Fmt(std::fmt::Error),
}

impl std::fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeneratorError::Io(e) => write!(f, "I/O error: {e}"),
            GeneratorError::Json(e) => write!(f, "JSON error: {e}"),
            GeneratorError::Fmt(e) => write!(f, "Format error: {e}"),
        }
    }
}

impl std::error::Error for GeneratorError {}

impl From<io::Error> for GeneratorError {
    fn from(e: io::Error) -> Self {
        GeneratorError::Io(e)
    }
}

impl From<serde_json::Error> for GeneratorError {
    fn from(e: serde_json::Error) -> Self {
        GeneratorError::Json(e)
    }
}

impl From<std::fmt::Error> for GeneratorError {
    fn from(e: std::fmt::Error) -> Self {
        GeneratorError::Fmt(e)
    }
}

/// HTML 특수 문자를 이스케이프한다.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// 텍스트가 타임스탬프 전용 마커인지 확인한다 (예: "[00:01:30]").
///
/// 자막 없이 캡쳐된 프레임에서 `Segment::from_frame_timestamp_only`가
/// 생성하는 `[HH:MM:SS]` 패턴에 일치하면 true를 반환한다.
fn is_timestamp_only_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.len() < 10 || !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return false;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    // Match HH:MM:SS pattern
    let parts: Vec<&str> = inner.split(':').collect();
    parts.len() == 3 && parts.iter().all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_digit()))
}

/// 텍스트를 `max_len` 글자로 자르고, 잘린 경우 "…"을 덧붙인다.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}…")
    }
}

/// `output_dir/slides.html`에 독립형 slides.html 파일을 생성한다.
///
/// `segments`가 순서대로 정렬되어 있고 `output_dir/images/`에
/// 참조된 이미지 파일이 있어야 한다.
pub fn generate_slides_html(
    output_dir: &Path,
    segments: &[Segment],
    metadata: &VideoMetadata,
) -> Result<(), GeneratorError> {
    let html = render_slides_html(segments, metadata)?;
    let html_path = output_dir.join("slides.html");
    fs::write(&html_path, html)?;
    Ok(())
}

/// `output_dir/segments.json`에서 세그먼트를 불러와 slides.html을 생성한다.
pub fn generate_from_segments_json(
    output_dir: &Path,
    metadata: &VideoMetadata,
) -> Result<(), GeneratorError> {
    let seg_path = output_dir.join("segments.json");
    let content = fs::read_to_string(&seg_path)?;
    let segments: Vec<Segment> = serde_json::from_str(&content)?;
    generate_slides_html(output_dir, &segments, metadata)
}

/// 슬라이드 뷰어용 전체 HTML 문자열을 렌더링한다.
pub fn render_slides_html(
    segments: &[Segment],
    metadata: &VideoMetadata,
) -> Result<String, GeneratorError> {
    let title = html_escape(&metadata.title);
    let url = html_escape(&metadata.url);
    let channel = html_escape(&metadata.channel);
    let date = html_escape(&metadata.date);
    let duration = html_escape(&metadata.duration);
    let count = segments.len();

    let meta_parts: Vec<&str> = [channel.as_str(), date.as_str(), duration.as_str()]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let meta_info = meta_parts.join(" · ");

    // Build TOC items
    let mut toc_items = String::new();
    for seg in segments {
        let ts = html_escape(&seg.timestamp);
        let preview = html_escape(&truncate(&seg.text, 50));
        writeln!(
            toc_items,
            "<li><a href=\"javascript:void(0)\" data-slide=\"{idx}\" class=\"toc-link\"><span class=\"toc-ts\">{ts}</span><span class=\"toc-preview\">{preview}</span></a></li>",
            idx = seg.index,
            ts = ts,
            preview = preview,
        )?;
    }

    // Build slide cards
    let mut slide_cards = String::new();
    for seg in segments {
        let ts = html_escape(&seg.timestamp);
        let text = html_escape(&seg.text);
        let img = html_escape(&seg.image);
        // Detect timestamp-only text (format: [HH:MM:SS])
        let is_timestamp_only = is_timestamp_only_text(&seg.text);
        let p_class = if is_timestamp_only {
            " class=\"timestamp-only\""
        } else {
            ""
        };
        writeln!(
            slide_cards,
            r##"<div class="slide" id="slide-{idx}" data-index="{idx}">
  <div class="slide-image">
    <img src="images/{img}" alt="Frame at {ts}" loading="lazy">
    <span class="timestamp-badge">{ts}</span>
    <span class="slide-number">#{num}</span>
  </div>
  <div class="slide-text">
    <p{p_class}>{text}</p>
  </div>
</div>"##,
            idx = seg.index,
            img = img,
            ts = ts,
            num = seg.index + 1,
            text = text,
            p_class = p_class,
        )?;
    }

    let youtube_link = if url.is_empty() {
        String::new()
    } else {
        format!(
            r#"<a href="{url}" target="_blank" rel="noopener" class="yt-link">YouTube에서 보기 →</a>"#,
            url = url
        )
    };

    let html = format!(
        r##"<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{title}</title>
<style>
/* === Shared Theme (embedded from framepick theme.rs) === */
{theme_css}

/* Short aliases (match src-ui/theme.css alias layer) */
:root{{
  --bg-primary: var(--fp-bg-primary);
  --bg-secondary: var(--fp-bg-secondary);
  --bg-card: var(--fp-bg-card);
  --bg-input: var(--fp-bg-input);
  --bg-hover: var(--fp-bg-hover);
  --text-primary: var(--fp-text-primary);
  --text-secondary: var(--fp-text-secondary);
  --text-muted: var(--fp-text-muted);
  --accent: var(--fp-accent);
  --accent-hover: var(--fp-accent-hover);
  --accent-dim: var(--fp-accent-dim);
  --error: var(--fp-error);
  --success: var(--fp-success);
  --border: var(--fp-border);
  --radius: var(--fp-radius);
  --radius-sm: var(--fp-radius-sm);
}}

/* === Reset & Base === */
*,*::before,*::after{{ margin:0; padding:0; box-sizing:border-box; }}
html{{ scroll-behavior:smooth; height:100%; }}
body{{
  font-family: var(--fp-font-family);
  background: var(--fp-bg-primary);
  color: var(--fp-text-primary);
  overflow: hidden;
  height: 100vh;
}}
/* Smooth scrolling for the main content scroll container */
.main-content{{ scroll-behavior:smooth; }}

/* === Layout: sidebar + main === */
.app-layout{{
  display: flex;
  height: 100vh;
}}

/* --- Sidebar --- */
.sidebar{{
  width: 300px;
  min-width: 260px;
  background: var(--fp-bg-secondary);
  border-right: 1px solid var(--fp-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  flex-shrink: 0;
}}
.sidebar-header{{
  padding: 20px 16px 12px;
  border-bottom: 1px solid var(--fp-border);
}}
.sidebar-header h1{{
  font-size: var(--fp-font-size-lg);
  color: var(--fp-text-heading);
  line-height: 1.4;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}}
.sidebar-header .meta{{
  font-size: var(--fp-font-size-sm);
  color: var(--fp-text-secondary);
  margin-bottom: 6px;
}}
.sidebar-header .yt-link{{
  color: var(--fp-accent);
  text-decoration: none;
  font-size: 0.8rem;
}}
.sidebar-header .yt-link:hover{{ color: var(--fp-accent-hover); text-decoration: underline; }}

.toc-title{{
  padding: 10px 16px 6px;
  font-size: 0.8rem;
  color: var(--fp-accent);
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}}

.toc-list{{
  list-style: none;
  overflow-y: auto;
  flex: 1;
  padding: 0 8px 16px;
}}
.toc-list::-webkit-scrollbar{{ width:6px; }}
.toc-list::-webkit-scrollbar-track{{ background: var(--fp-bg-scrollbar-track); }}
.toc-list::-webkit-scrollbar-thumb{{ background: var(--fp-bg-scrollbar-thumb); border-radius:3px; }}
.toc-list::-webkit-scrollbar-thumb:hover{{ background: var(--fp-bg-scrollbar-thumb-hover); }}

.toc-list li{{ margin-bottom: 1px; }}
.toc-link{{
  display: flex;
  align-items: baseline;
  gap: var(--fp-spacing-sm);
  padding: 6px 8px;
  border-radius: 6px;
  border-left: 3px solid transparent;
  text-decoration: none;
  transition: background var(--fp-transition), border-color var(--fp-transition), color var(--fp-transition);
}}
.toc-link:hover{{
  background: var(--fp-bg-surface);
}}
.toc-link.active{{
  background: var(--fp-bg-surface);
  border-left-color: var(--fp-accent);
}}
.toc-link.active .toc-ts{{ color: var(--fp-accent-hover); }}
.toc-link.active .toc-preview{{ color: var(--fp-text-primary); }}

.toc-ts{{
  color: var(--fp-accent);
  font-family: var(--fp-font-mono);
  font-size: 0.75rem;
  font-weight: 600;
  white-space: nowrap;
  flex-shrink: 0;
}}
.toc-preview{{
  color: var(--fp-text-secondary);
  font-size: var(--fp-font-size-sm);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}}

/* --- Main content area --- */
.main-content{{
  flex: 1;
  overflow-y: auto;
  padding: 0;
}}
.main-content::-webkit-scrollbar{{ width:8px; }}
.main-content::-webkit-scrollbar-track{{ background: var(--fp-bg-primary); }}
.main-content::-webkit-scrollbar-thumb{{ background: var(--fp-bg-scrollbar-thumb); border-radius: var(--fp-radius-sm); }}
.main-content::-webkit-scrollbar-thumb:hover{{ background: var(--fp-bg-scrollbar-thumb-hover); }}

.slides-container{{
  max-width: 900px;
  margin: 0 auto;
  padding: var(--fp-spacing-lg) var(--fp-spacing-lg) var(--fp-spacing-xl);
}}

/* --- Slide card --- */
.slide{{
  background: var(--fp-bg-secondary);
  border-radius: var(--fp-radius-lg);
  margin-bottom: 20px;
  overflow: hidden;
  border: 1px solid var(--fp-border);
  box-shadow: var(--fp-shadow-card);
  transition: transform var(--fp-transition), box-shadow var(--fp-transition), outline-color 0.25s ease;
  scroll-margin-top: var(--fp-spacing-lg);
  outline: 2px solid transparent;
  outline-offset: -2px;
}}
.slide:hover{{
  transform: translateY(-2px);
  box-shadow: var(--fp-shadow-card-hover);
}}
.slide.highlight{{
  outline-color: var(--fp-accent);
}}

.slide-image{{
  position: relative;
  background: var(--fp-bg-image);
  line-height: 0;
  cursor: pointer;
}}
.slide-image img{{
  width: 100%;
  display: block;
}}
.slide-image:hover img{{
  opacity: 0.92;
}}
.timestamp-badge{{
  position: absolute;
  top: 10px;
  left: 10px;
  background: var(--fp-bg-overlay);
  color: var(--fp-accent);
  padding: var(--fp-spacing-xs) 10px;
  border-radius: 6px;
  font-size: 0.82rem;
  font-family: var(--fp-font-mono);
  font-weight: 600;
  backdrop-filter: blur(4px);
}}
.slide-number{{
  position: absolute;
  top: 10px;
  right: 10px;
  background: var(--fp-bg-overlay-light);
  color: var(--fp-text-muted);
  padding: 3px var(--fp-spacing-sm);
  border-radius: var(--fp-radius-sm);
  font-size: var(--fp-font-size-xs);
  font-family: var(--fp-font-mono);
}}

.slide-text{{
  padding: var(--fp-spacing-md) 20px;
}}
.slide-text p{{
  font-size: var(--fp-font-size-base);
  line-height: 1.8;
  color: var(--fp-text-body);
}}
.slide-text p.timestamp-only{{
  color: var(--fp-accent);
  font-family: var(--fp-font-mono);
  font-size: 0.88rem;
  opacity: 0.8;
}}

/* --- Footer --- */
.footer{{
  text-align: center;
  color: var(--fp-text-muted);
  padding: var(--fp-spacing-md);
  font-size: var(--fp-font-size-sm);
  border-top: 1px solid var(--fp-border);
}}

/* --- Sidebar toggle (mobile) --- */
.sidebar-toggle{{
  display: none;
  position: fixed;
  bottom: 20px;
  left: 20px;
  z-index: 1000;
  background: var(--fp-accent);
  color: var(--fp-text-on-accent);
  border: none;
  border-radius: 50%;
  width: 48px;
  height: 48px;
  font-size: 1.3rem;
  cursor: pointer;
  box-shadow: var(--fp-shadow-toggle);
}}
.sidebar-toggle:hover{{
  background: var(--fp-accent-hover);
}}

/* --- Keyboard help hint --- */
.kbd-hint{{
  position: fixed;
  bottom: 12px;
  right: 16px;
  color: var(--fp-text-muted);
  font-size: var(--fp-font-size-xs);
  font-family: var(--fp-font-mono);
  pointer-events: none;
}}

/* --- Fullscreen Image Viewer (Lightbox) --- */
.lightbox-overlay{{
  display: none;
  position: fixed;
  inset: 0;
  z-index: 2000;
  background: var(--fp-bg-overlay);
  backdrop-filter: blur(8px);
  align-items: center;
  justify-content: center;
  cursor: zoom-out;
}}
.lightbox-overlay.open{{ display: flex; }}
.lightbox-img{{
  max-width: 95vw;
  max-height: 92vh;
  object-fit: contain;
  border-radius: var(--fp-radius);
  box-shadow: 0 8px 48px rgba(0,0,0,0.7);
  transition: transform var(--fp-transition);
}}
.lightbox-close{{
  position: fixed;
  top: 16px;
  right: 20px;
  background: var(--fp-bg-overlay-light);
  color: var(--fp-text-primary);
  border: 1px solid var(--fp-border);
  border-radius: var(--fp-radius-sm);
  padding: 6px 14px;
  font-size: 1.1rem;
  cursor: pointer;
  z-index: 2001;
  transition: background var(--fp-transition-fast), color var(--fp-transition-fast);
}}
.lightbox-close:hover{{
  background: var(--fp-accent);
  color: var(--fp-text-on-accent);
}}
.lightbox-nav{{
  position: fixed;
  top: 50%;
  transform: translateY(-50%);
  background: var(--fp-bg-overlay-light);
  color: var(--fp-text-primary);
  border: 1px solid var(--fp-border);
  border-radius: var(--fp-radius);
  padding: 12px 16px;
  font-size: 1.3rem;
  cursor: pointer;
  z-index: 2001;
  transition: background var(--fp-transition-fast), color var(--fp-transition-fast);
}}
.lightbox-nav:hover{{
  background: var(--fp-accent);
  color: var(--fp-text-on-accent);
}}
.lightbox-prev{{ left: 16px; }}
.lightbox-next{{ right: 16px; }}
.lightbox-caption{{
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  background: var(--fp-bg-overlay);
  color: var(--fp-text-body);
  padding: var(--fp-spacing-sm) var(--fp-spacing-md);
  border-radius: var(--fp-radius);
  font-size: var(--fp-font-size-sm);
  font-family: var(--fp-font-mono);
  z-index: 2001;
  white-space: nowrap;
}}

/* === Responsive === */
@media (max-width: 768px){{
  .sidebar{{
    position: fixed;
    left: -300px;
    top: 0;
    height: 100vh;
    z-index: 900;
    transition: left 0.25s ease;
  }}
  .sidebar.open{{
    left: 0;
    box-shadow: 4px 0 24px var(--fp-shadow);
  }}
  .sidebar-toggle{{
    display: flex;
    align-items: center;
    justify-content: center;
  }}
  .slides-container{{
    padding: var(--fp-spacing-md) 12px var(--fp-spacing-xl);
  }}
  .slide-text{{ padding: 12px 14px; }}
  .lightbox-nav{{ display: none; }}
}}

@media (max-width: 480px){{
  .sidebar{{ width: 260px; left: -260px; }}
  .slide{{ border-radius: var(--fp-radius); }}
}}
</style>
</head>
<body>

<div class="app-layout">
  <!-- Sidebar TOC -->
  <aside class="sidebar" id="sidebar">
    <div class="sidebar-header">
      <h1 title="{title}">{title}</h1>
      <div class="meta">{meta_info}</div>
      {youtube_link}
    </div>
    <div class="toc-title">목차 — {count}개 슬라이드</div>
    <ul class="toc-list" id="tocList">
      {toc_items}
    </ul>
  </aside>

  <!-- Main content area -->
  <main class="main-content" id="mainContent">
    <div class="slides-container" id="slidesContainer">
      {slide_cards}
    </div>
    <div class="footer">{count}개 슬라이드 · Generated by framepick</div>
  </main>
</div>

<!-- Mobile sidebar toggle -->
<button class="sidebar-toggle" id="sidebarToggle" aria-label="Toggle sidebar">☰</button>
<div class="kbd-hint">← → / j k 키로 탐색 · 클릭하여 확대</div>

<!-- Fullscreen Image Viewer (Lightbox) -->
<div class="lightbox-overlay" id="lightbox">
  <button class="lightbox-close" id="lightboxClose" aria-label="Close">&times;</button>
  <button class="lightbox-nav lightbox-prev" id="lightboxPrev" aria-label="Previous">&#8249;</button>
  <button class="lightbox-nav lightbox-next" id="lightboxNext" aria-label="Next">&#8250;</button>
  <img class="lightbox-img" id="lightboxImg" src="" alt="">
  <div class="lightbox-caption" id="lightboxCaption"></div>
</div>

<script>
(function(){{
  'use strict';

  var slides = document.querySelectorAll('.slide');
  var tocLinks = document.querySelectorAll('.toc-link');
  var mainContent = document.getElementById('mainContent');
  var sidebar = document.getElementById('sidebar');
  var sidebarToggle = document.getElementById('sidebarToggle');
  var tocList = document.getElementById('tocList');
  var currentIndex = 0;

  /* ── Build index map for fast lookup ── */
  var slideByIdx = {{}};
  var linkByIdx = {{}};
  slides.forEach(function(s) {{ slideByIdx[s.dataset.index] = s; }});
  tocLinks.forEach(function(l) {{ linkByIdx[l.dataset.slide] = l; }});

  /* ── Debounce helper to avoid rapid scroll-spy updates ── */
  var scrollSpyTimer = null;
  function debouncedSetActive(idx) {{
    if (scrollSpyTimer) clearTimeout(scrollSpyTimer);
    scrollSpyTimer = setTimeout(function() {{
      setActive(idx, false);
    }}, 50);
  }}

  /* ── Scroll-spy via IntersectionObserver ──
     Uses the mainContent div as the scroll root so it works whether
     the page is viewed inside a Tauri webview (where body is the
     viewport) or opened standalone in a browser.
     Falls back to scroll event listener if IntersectionObserver
     doesn't fire (some embedded webview edge cases). */
  var observerActive = false;
  var observer = null;

  function initScrollSpy() {{
    if (typeof IntersectionObserver !== 'undefined' && mainContent) {{
      observer = new IntersectionObserver(function(entries) {{
        var topMost = null;
        entries.forEach(function(entry) {{
          if (entry.isIntersecting) {{
            var idx = parseInt(entry.target.dataset.index, 10);
            if (!isNaN(idx) && (topMost === null || idx < topMost)) {{
              topMost = idx;
            }}
          }}
        }});
        if (topMost !== null) {{
          observerActive = true;
          debouncedSetActive(topMost);
        }}
      }}, {{
        root: mainContent,
        rootMargin: '-10% 0px -70% 0px',
        threshold: 0
      }});
      slides.forEach(function(slide) {{ observer.observe(slide); }});
    }}

    /* Fallback scroll listener for environments where
       IntersectionObserver may not trigger reliably */
    mainContent.addEventListener('scroll', function() {{
      if (observerActive) return;
      var scrollTop = mainContent.scrollTop;
      var closest = 0;
      var closestDist = Infinity;
      slides.forEach(function(slide, i) {{
        var dist = Math.abs(slide.offsetTop - scrollTop - 24);
        if (dist < closestDist) {{
          closestDist = dist;
          closest = i;
        }}
      }});
      debouncedSetActive(closest);
    }}, {{ passive: true }});
  }}

  initScrollSpy();

  /* ── Set active slide + TOC highlight ── */
  function setActive(idx, doScroll) {{
    if (idx < 0 || idx >= slides.length) return;
    currentIndex = idx;

    // Update TOC highlights
    tocLinks.forEach(function(link) {{
      var isActive = parseInt(link.dataset.slide, 10) === idx;
      link.classList.toggle('active', isActive);
    }});

    // Scroll the TOC sidebar so active item is visible
    var activeLink = linkByIdx[idx];
    if (activeLink && tocList) {{
      var listRect = tocList.getBoundingClientRect();
      var linkRect = activeLink.getBoundingClientRect();
      if (linkRect.top < listRect.top || linkRect.bottom > listRect.bottom) {{
        activeLink.scrollIntoView({{ block: 'nearest', behavior: 'smooth' }});
      }}
    }}

    // Highlight the active slide card
    slides.forEach(function(s) {{ s.classList.remove('highlight'); }});
    if (slides[idx]) slides[idx].classList.add('highlight');

    // Scroll main content to the slide
    if (doScroll && slides[idx]) {{
      slides[idx].scrollIntoView({{ behavior: 'smooth', block: 'start' }});
    }}

    // Update URL hash without triggering scroll (for bookmarking / sharing)
    var newHash = '#slide-' + idx;
    if (window.location.hash !== newHash) {{
      if (history.replaceState) {{
        history.replaceState(null, '', newHash);
      }}
    }}
  }}

  /* ── TOC click handler (event delegation for efficiency) ── */
  tocList.addEventListener('click', function(e) {{
    var link = e.target.closest('.toc-link');
    if (!link) return;
    e.preventDefault();
    var idx = parseInt(link.dataset.slide, 10);
    if (!isNaN(idx)) {{
      setActive(idx, true);
      // Close mobile sidebar after navigation
      if (window.innerWidth <= 768) {{
        sidebar.classList.remove('open');
      }}
    }}
  }});

  /* ── Hash-based navigation ──
     Handles initial page load with a #slide-N hash and
     browser back/forward navigation between anchors. */
  function navigateToHash() {{
    var hash = window.location.hash;
    if (hash && hash.indexOf('#slide-') === 0) {{
      var idx = parseInt(hash.replace('#slide-', ''), 10);
      if (!isNaN(idx) && idx >= 0 && idx < slides.length) {{
        setActive(idx, true);
      }}
    }}
  }}

  // Listen for hash changes (back/forward, manual URL edits)
  window.addEventListener('hashchange', navigateToHash);

  // Handle initial hash on page load (with slight delay for layout)
  if (window.location.hash) {{
    setTimeout(navigateToHash, 80);
  }}

  /* ── Keyboard navigation ── */
  document.addEventListener('keydown', function(e) {{
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    if (lightbox.classList.contains('open')) return;
    var handled = false;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || e.key === 'j') {{
      if (currentIndex < slides.length - 1) {{ setActive(currentIndex + 1, true); handled = true; }}
    }} else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || e.key === 'k') {{
      if (currentIndex > 0) {{ setActive(currentIndex - 1, true); handled = true; }}
    }} else if (e.key === 'Home') {{
      setActive(0, true); handled = true;
    }} else if (e.key === 'End') {{
      setActive(slides.length - 1, true); handled = true;
    }}
    if (handled) e.preventDefault();
  }});

  /* ── Mobile sidebar toggle ── */
  sidebarToggle.addEventListener('click', function() {{
    sidebar.classList.toggle('open');
  }});

  // Close sidebar when clicking main content on mobile
  mainContent.addEventListener('click', function() {{
    if (window.innerWidth <= 768) {{
      sidebar.classList.remove('open');
    }}
  }});

  /* ── Lightbox (fullscreen image viewer) ── */
  var lightbox = document.getElementById('lightbox');
  var lightboxImg = document.getElementById('lightboxImg');
  var lightboxCaption = document.getElementById('lightboxCaption');
  var lightboxClose = document.getElementById('lightboxClose');
  var lightboxPrev = document.getElementById('lightboxPrev');
  var lightboxNext = document.getElementById('lightboxNext');
  var lightboxIndex = 0;

  function openLightbox(idx) {{
    if (idx < 0 || idx >= slides.length) return;
    lightboxIndex = idx;
    var img = slides[idx].querySelector('.slide-image img');
    var badge = slides[idx].querySelector('.timestamp-badge');
    if (img) {{
      lightboxImg.src = img.src;
      lightboxImg.alt = img.alt;
    }}
    lightboxCaption.textContent = badge ? badge.textContent + ' (#' + (idx + 1) + '/' + slides.length + ')' : '';
    lightbox.classList.add('open');
    document.body.style.overflow = 'hidden';
  }}

  function closeLightbox() {{
    lightbox.classList.remove('open');
    document.body.style.overflow = '';
  }}

  function lightboxNav(dir) {{
    var next = lightboxIndex + dir;
    if (next >= 0 && next < slides.length) {{
      openLightbox(next);
      setActive(next, false);
    }}
  }}

  // Open lightbox on slide image click
  slides.forEach(function(slide, i) {{
    var imgArea = slide.querySelector('.slide-image');
    if (imgArea) {{
      imgArea.addEventListener('click', function(e) {{
        e.stopPropagation();
        openLightbox(i);
      }});
    }}
  }});

  lightboxClose.addEventListener('click', closeLightbox);
  lightboxPrev.addEventListener('click', function(e) {{ e.stopPropagation(); lightboxNav(-1); }});
  lightboxNext.addEventListener('click', function(e) {{ e.stopPropagation(); lightboxNav(1); }});
  lightbox.addEventListener('click', function(e) {{
    if (e.target === lightbox || e.target === lightboxImg) closeLightbox();
  }});

  // Keyboard support in lightbox
  document.addEventListener('keydown', function(e) {{
    if (!lightbox.classList.contains('open')) return;
    if (e.key === 'Escape') {{ closeLightbox(); e.preventDefault(); }}
    else if (e.key === 'ArrowLeft') {{ lightboxNav(-1); e.preventDefault(); }}
    else if (e.key === 'ArrowRight') {{ lightboxNav(1); e.preventDefault(); }}
  }});

  /* ── Initialize ──
     If there is a hash, navigate to it; otherwise activate first slide. */
  if (!window.location.hash && slides.length > 0) {{
    setActive(0, false);
  }}
}})();
</script>
</body>
</html>"##,
        theme_css = crate::theme::css_variables_block(),
        title = title,
        meta_info = meta_info,
        youtube_link = youtube_link,
        count = count,
        toc_items = toc_items,
        slide_cards = slide_cards,
    );

    Ok(html)
}

/// slides.html과 함께 slides.md(마크다운 버전)를 생성한다.
pub fn generate_slides_md(
    output_dir: &Path,
    segments: &[Segment],
    metadata: &VideoMetadata,
) -> Result<(), GeneratorError> {
    let mut md = String::new();
    writeln!(md, "---")?;
    writeln!(md, "title: \"{}\"", metadata.title)?;
    writeln!(md, "url: {}", metadata.url)?;
    writeln!(md, "channel: {}", metadata.channel)?;
    writeln!(md, "date: {}", metadata.date)?;
    writeln!(md, "duration: {}", metadata.duration)?;
    writeln!(md, "---")?;
    writeln!(md)?;
    writeln!(md, "# {}", metadata.title)?;
    writeln!(md)?;

    for seg in segments {
        writeln!(md, "## [{}]", seg.timestamp)?;
        writeln!(md)?;
        writeln!(md, "![{}](images/{})", seg.timestamp, seg.image)?;
        writeln!(md)?;
        writeln!(md, "> {}", seg.text)?;
        writeln!(md)?;
    }

    let md_path = output_dir.join("slides.md");
    fs::write(&md_path, md)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_segments() -> Vec<Segment> {
        vec![
            Segment {
                index: 0,
                timestamp: "00:00:00".to_string(),
                text: "안녕하세요, 오늘 강의를 시작하겠습니다.".to_string(),
                image: "frame_0000_00-00-00.jpg".to_string(),
            },
            Segment {
                index: 1,
                timestamp: "00:01:30".to_string(),
                text: "첫 번째 주제는 Rust의 소유권입니다.".to_string(),
                image: "frame_0001_00-01-30.jpg".to_string(),
            },
            Segment {
                index: 2,
                timestamp: "00:05:00".to_string(),
                text: "다음으로 <borrowing>에 대해 알아보겠습니다.".to_string(),
                image: "frame_0002_00-05-00.jpg".to_string(),
            },
        ]
    }

    fn sample_metadata() -> VideoMetadata {
        VideoMetadata {
            title: "Rust 소유권 완벽 가이드".to_string(),
            url: "https://www.youtube.com/watch?v=test123".to_string(),
            channel: "RustKR".to_string(),
            date: "2025-01-15".to_string(),
            duration: "15:30".to_string(),
            video_id: "test123".to_string(),
        }
    }

    #[test]
    fn render_html_contains_structure() {
        let segments = sample_segments();
        let metadata = sample_metadata();
        let html = render_slides_html(&segments, &metadata).unwrap();

        // Basic structure checks
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html lang=\"ko\">"));
        assert!(html.contains("class=\"sidebar\""));
        assert!(html.contains("class=\"main-content\""));
        assert!(html.contains("class=\"toc-list\""));

        // Title
        assert!(html.contains("Rust 소유권 완벽 가이드"));

        // TOC items
        assert!(html.contains("00:00:00"));
        assert!(html.contains("00:01:30"));
        assert!(html.contains("00:05:00"));

        // Slide cards
        assert!(html.contains("id=\"slide-0\""));
        assert!(html.contains("id=\"slide-1\""));
        assert!(html.contains("id=\"slide-2\""));

        // Image references
        assert!(html.contains("frame_0000_00-00-00.jpg"));
        assert!(html.contains("frame_0002_00-05-00.jpg"));

        // Meta info
        assert!(html.contains("RustKR"));
        assert!(html.contains("15:30"));

        // YouTube link
        assert!(html.contains("youtube.com/watch"));

        // Count
        assert!(html.contains("3개 슬라이드"));

        // Navigation script
        assert!(html.contains("ArrowRight"));
        assert!(html.contains("IntersectionObserver"));
    }

    #[test]
    fn html_escapes_special_chars() {
        let segments = sample_segments();
        let metadata = sample_metadata();
        let html = render_slides_html(&segments, &metadata).unwrap();

        // The text "<borrowing>" should be escaped
        assert!(html.contains("&lt;borrowing&gt;"));
        assert!(!html.contains("<borrowing>"));
    }

    #[test]
    fn render_empty_segments() {
        let segments: Vec<Segment> = vec![];
        let metadata = sample_metadata();
        let html = render_slides_html(&segments, &metadata).unwrap();

        assert!(html.contains("0개 슬라이드"));
        assert!(html.contains("class=\"sidebar\""));
    }

    #[test]
    fn truncate_long_text() {
        assert_eq!(truncate("short", 50), "short");
        let long = "a".repeat(100);
        let result = truncate(&long, 50);
        assert!(result.ends_with('…'));
        assert_eq!(result.chars().count(), 51); // 50 chars + …
    }

    #[test]
    fn render_without_url() {
        let segments = sample_segments();
        let mut metadata = sample_metadata();
        metadata.url = String::new();
        let html = render_slides_html(&segments, &metadata).unwrap();
        assert!(!html.contains("YouTube에서 보기"));
    }

    #[test]
    fn generate_to_disk() {
        let dir = std::env::temp_dir().join("framepick_test_slides");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let segments = sample_segments();
        let metadata = sample_metadata();

        generate_slides_html(&dir, &segments, &metadata).unwrap();
        generate_slides_md(&dir, &segments, &metadata).unwrap();

        assert!(dir.join("slides.html").exists());
        assert!(dir.join("slides.md").exists());

        let html = std::fs::read_to_string(dir.join("slides.html")).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));

        let md = std::fs::read_to_string(dir.join("slides.md")).unwrap();
        assert!(md.contains("# Rust 소유권 완벽 가이드"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dark_theme_css_properties() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Verify shared --fp-* theme CSS custom properties are embedded
        assert!(html.contains("--fp-bg-primary:"), "Missing --fp-bg-primary");
        assert!(html.contains("--fp-bg-secondary:"), "Missing --fp-bg-secondary");
        assert!(html.contains("--fp-bg-surface:"), "Missing --fp-bg-surface");
        assert!(html.contains("--fp-text-primary:"), "Missing --fp-text-primary");
        assert!(html.contains("--fp-text-secondary:"), "Missing --fp-text-secondary");
        assert!(html.contains("--fp-accent:"), "Missing --fp-accent");
        assert!(html.contains("--fp-accent-hover:"), "Missing --fp-accent-hover");
        assert!(html.contains("--fp-border:"), "Missing --fp-border");
        assert!(html.contains("--fp-radius:"), "Missing --fp-radius");
        assert!(html.contains("--fp-font-family:"), "Missing --fp-font-family");
        assert!(html.contains("--fp-font-mono:"), "Missing --fp-font-mono");
        assert!(html.contains("--fp-shadow-card:"), "Missing --fp-shadow-card");
        assert!(html.contains("--fp-bg-overlay:"), "Missing --fp-bg-overlay");
        assert!(html.contains("--fp-bg-image:"), "Missing --fp-bg-image");
        assert!(html.contains("--fp-text-on-accent:"), "Missing --fp-text-on-accent");
        // Verify legacy aliases map to --fp-* tokens
        assert!(html.contains("--bg-primary: var(--fp-bg-primary)"));
        assert!(html.contains("--accent: var(--fp-accent)"));
        // Verify theme variables are used directly in key elements (--fp-* tokens)
        assert!(html.contains("background: var(--fp-bg-primary)"));     // body
        assert!(html.contains("background: var(--fp-bg-secondary)")); // sidebar + cards
        assert!(html.contains("color: var(--fp-text-primary)"));    // body text
        assert!(html.contains("color: var(--fp-accent)"));          // accent elements
        assert!(html.contains("background: var(--fp-bg-image)"));   // slide image bg
        assert!(html.contains("background: var(--fp-bg-overlay)")); // timestamp badge + lightbox
        assert!(html.contains("var(--fp-shadow-card)"));            // card shadow
        assert!(html.contains("var(--fp-shadow-card-hover)"));      // card hover shadow
        assert!(html.contains("var(--fp-shadow-toggle)"));          // mobile toggle shadow
        assert!(html.contains("var(--fp-font-mono)"));              // monospace font usage
    }

    #[test]
    fn responsive_css_present() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains("@media (max-width: 768px)"));
        assert!(html.contains("sidebar-toggle"));
    }

    #[test]
    fn keyboard_navigation_present() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains("ArrowRight"));
        assert!(html.contains("ArrowLeft"));
        assert!(html.contains("ArrowDown"));
        assert!(html.contains("ArrowUp"));
        assert!(html.contains("keydown"));
        // Home/End keys
        assert!(html.contains("'Home'"));
        assert!(html.contains("'End'"));
    }

    #[test]
    fn toc_anchor_links_match_slide_ids() {
        let segments = sample_segments();
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();
        // TOC 링크는 javascript:void(0)을 사용하고, data-slide로 인덱스를 참조
        // 각 슬라이드에는 id=slide-N이 존재해야 함
        for seg in &segments {
            let data_slide = format!("data-slide=\"{}\"", seg.index);
            let id = format!("id=\"slide-{}\"", seg.index);
            assert!(html.contains(&data_slide), "Missing TOC data-slide for slide {}", seg.index);
            assert!(html.contains(&id), "Missing slide id for slide {}", seg.index);
        }
        // href는 javascript:void(0)이어야 iframe 해시 네비게이션 충돌 방지
        assert!(html.contains("href=\"javascript:void(0)\""), "TOC links should use javascript:void(0)");
    }

    #[test]
    fn toc_links_have_data_slide_attr() {
        let segments = sample_segments();
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();
        for seg in &segments {
            let attr = format!("data-slide=\"{}\"", seg.index);
            assert!(html.contains(&attr), "Missing data-slide attr for slide {}", seg.index);
        }
    }

    #[test]
    fn slides_have_data_index_attr() {
        let segments = sample_segments();
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();
        for seg in &segments {
            let attr = format!("data-index=\"{}\"", seg.index);
            assert!(html.contains(&attr), "Missing data-index attr for slide {}", seg.index);
        }
    }

    #[test]
    fn smooth_scroll_on_main_content() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // CSS smooth scroll on main content container
        assert!(html.contains("scroll-behavior:smooth"));
        // JS smooth scroll in scrollIntoView calls
        assert!(html.contains("behavior: 'smooth'"));
    }

    #[test]
    fn hash_navigation_support() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains("hashchange"));
        assert!(html.contains("navigateToHash"));
        assert!(html.contains("replaceState"));
    }

    #[test]
    fn intersection_observer_scroll_spy() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains("IntersectionObserver"));
        assert!(html.contains("root: mainContent"));
    }

    #[test]
    fn toc_sidebar_scrolls_active_into_view() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Verify the TOC auto-scroll logic is present
        assert!(html.contains("activeLink.scrollIntoView"));
        assert!(html.contains("block: 'nearest'"));
    }

    #[test]
    fn toc_active_state_has_left_border_indicator() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Active TOC link should have visible left border accent using theme variable
        assert!(html.contains("border-left: 3px solid transparent"));
        assert!(html.contains("border-left-color: var(--fp-accent)"));
    }

    #[test]
    fn slide_highlight_outline_transition() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Slide highlight should transition smoothly using theme accent
        assert!(html.contains("outline: 2px solid transparent"));
        assert!(html.contains("outline-color 0.25s ease"));
        assert!(html.contains("outline-color: var(--fp-accent)"));
    }

    #[test]
    fn scroll_spy_fallback_for_webview_compat() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Should have scroll event fallback for environments where IO may not trigger
        assert!(html.contains("mainContent.addEventListener('scroll'"));
        assert!(html.contains("passive: true"));
        assert!(html.contains("debouncedSetActive"));
    }

    #[test]
    fn scroll_margin_top_on_slides() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains("scroll-margin-top"));
    }

    #[test]
    fn event_delegation_on_toc() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Uses event delegation on tocList instead of individual listeners
        assert!(html.contains("tocList.addEventListener('click'"));
        assert!(html.contains("e.target.closest('.toc-link')"));
    }

    #[test]
    fn lightbox_image_viewer_present() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Lightbox overlay and elements
        assert!(html.contains("id=\"lightbox\""), "Missing lightbox overlay");
        assert!(html.contains("class=\"lightbox-overlay\""), "Missing lightbox-overlay class");
        assert!(html.contains("lightbox-img"), "Missing lightbox image element");
        assert!(html.contains("lightbox-close"), "Missing lightbox close button");
        assert!(html.contains("lightbox-prev"), "Missing lightbox prev button");
        assert!(html.contains("lightbox-next"), "Missing lightbox next button");
        assert!(html.contains("lightbox-caption"), "Missing lightbox caption");
    }

    #[test]
    fn lightbox_uses_theme_variables() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Lightbox CSS should use --fp-* theme variables
        assert!(html.contains(".lightbox-overlay"));
        assert!(html.contains("background: var(--fp-bg-overlay)"));
        assert!(html.contains("border-radius: var(--fp-radius)"));
    }

    #[test]
    fn lightbox_keyboard_navigation() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Lightbox should handle Escape and arrow keys
        assert!(html.contains("'Escape'"));
        assert!(html.contains("closeLightbox"));
        assert!(html.contains("lightboxNav"));
    }

    #[test]
    fn slide_image_clickable_for_lightbox() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Slide images should be clickable to open lightbox
        assert!(html.contains("cursor: pointer"));
        assert!(html.contains("openLightbox"));
    }

    #[test]
    fn no_hardcoded_colors_in_main_css() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        // Extract the <style> block content
        let style_start = html.find("<style>").unwrap() + 7;
        let style_end = html.find("</style>").unwrap();
        let style = &html[style_start..style_end];
        // Skip the :root block (theme definitions have raw values)
        let after_root = if let Some(pos) = style.find("/* Legacy aliases") {
            &style[pos..]
        } else {
            style
        };
        // Body background should use theme variable, not raw hex
        assert!(!after_root.contains("background: #1a1a2e"), "Body bg should use var, not hardcoded");
        assert!(!after_root.contains("background: #16213e"), "Sidebar bg should use var, not hardcoded");
        // Accent colors should be via variable
        assert!(!after_root.contains("color: #e94560"), "Accent color should use var, not hardcoded");
    }

    // ─── AC 15: Timestamp-only display tests ─────────────────────

    fn sample_frames() -> Vec<CapturedFrame> {
        vec![
            CapturedFrame {
                index: 0,
                timestamp_secs: 0.0,
                timestamp: "00:00:00".to_string(),
                filename: "frame_0000_00-00-00.jpg".to_string(),
            },
            CapturedFrame {
                index: 1,
                timestamp_secs: 90.0,
                timestamp: "00:01:30".to_string(),
                filename: "frame_0001_00-01-30.jpg".to_string(),
            },
            CapturedFrame {
                index: 2,
                timestamp_secs: 300.0,
                timestamp: "00:05:00".to_string(),
                filename: "frame_0002_00-05-00.jpg".to_string(),
            },
        ]
    }

    #[test]
    fn from_frame_timestamp_only_formats_bracket_timestamp() {
        let frame = CapturedFrame {
            index: 0,
            timestamp_secs: 90.0,
            timestamp: "00:01:30".to_string(),
            filename: "frame_0001_00-01-30.jpg".to_string(),
        };
        let seg = Segment::from_frame_timestamp_only(&frame);
        assert_eq!(seg.text, "[00:01:30]");
        assert_eq!(seg.timestamp, "00:01:30");
        assert_eq!(seg.index, 0);
        assert_eq!(seg.image, "frame_0001_00-01-30.jpg");
    }

    #[test]
    fn from_frame_with_text_uses_subtitle() {
        let frame = CapturedFrame {
            index: 1,
            timestamp_secs: 5.0,
            timestamp: "00:00:05".to_string(),
            filename: "frame_0001_00-00-05.jpg".to_string(),
        };
        let seg = Segment::from_frame_with_text(&frame, "안녕하세요".to_string());
        assert_eq!(seg.text, "안녕하세요");
        assert_eq!(seg.timestamp, "00:00:05");
    }

    #[test]
    fn frames_to_segments_all_timestamp_only() {
        let frames = sample_frames();
        let segments = frames_to_segments(&frames);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].text, "[00:00:00]");
        assert_eq!(segments[1].text, "[00:01:30]");
        assert_eq!(segments[2].text, "[00:05:00]");
    }

    #[test]
    fn frames_to_segments_preserves_index_and_image() {
        let frames = sample_frames();
        let segments = frames_to_segments(&frames);
        for (frame, seg) in frames.iter().zip(segments.iter()) {
            assert_eq!(frame.index, seg.index);
            assert_eq!(frame.filename, seg.image);
            assert_eq!(frame.timestamp, seg.timestamp);
        }
    }

    #[test]
    fn frames_to_segments_with_subtitles_matches_cues() {
        use crate::subtitle_extractor::SubtitleCue;

        let frames = sample_frames();
        let cues = vec![
            SubtitleCue {
                start_secs: 0.5,
                end_secs: 3.0,
                text: "첫 번째 자막".to_string(),
            },
            SubtitleCue {
                start_secs: 90.2,
                end_secs: 93.0,
                text: "두 번째 자막".to_string(),
            },
            // No cue near 300.0 — should fall back to timestamp
        ];

        let segments = frames_to_segments_with_subtitles(&frames, &cues);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].text, "첫 번째 자막");
        assert_eq!(segments[1].text, "두 번째 자막");
        assert_eq!(segments[2].text, "[00:05:00]"); // No matching cue
    }

    #[test]
    fn frames_to_segments_with_subtitles_empty_cue_text_falls_back() {
        use crate::subtitle_extractor::SubtitleCue;

        let frame = CapturedFrame {
            index: 0,
            timestamp_secs: 5.0,
            timestamp: "00:00:05".to_string(),
            filename: "frame_0000_00-00-05.jpg".to_string(),
        };
        let cues = vec![SubtitleCue {
            start_secs: 5.0,
            end_secs: 8.0,
            text: "   ".to_string(), // Whitespace-only
        }];

        let segments = frames_to_segments_with_subtitles(&[frame], &cues);
        assert_eq!(segments[0].text, "[00:00:05]");
    }

    #[test]
    fn frames_to_segments_with_subtitles_tolerance_window() {
        use crate::subtitle_extractor::SubtitleCue;

        let frame = CapturedFrame {
            index: 0,
            timestamp_secs: 10.0,
            timestamp: "00:00:10".to_string(),
            filename: "frame_0000.jpg".to_string(),
        };

        // Cue within 2s tolerance
        let cues_near = vec![SubtitleCue {
            start_secs: 11.5,
            end_secs: 14.0,
            text: "Close enough".to_string(),
        }];
        let segs = frames_to_segments_with_subtitles(&[frame.clone()], &cues_near);
        assert_eq!(segs[0].text, "Close enough");

        // Cue outside 2s tolerance
        let cues_far = vec![SubtitleCue {
            start_secs: 13.0,
            end_secs: 16.0,
            text: "Too far".to_string(),
        }];
        let segs = frames_to_segments_with_subtitles(&[frame], &cues_far);
        assert_eq!(segs[0].text, "[00:00:10]");
    }

    #[test]
    fn frames_to_segments_empty_input() {
        let segments = frames_to_segments(&[]);
        assert!(segments.is_empty());
    }

    #[test]
    fn is_timestamp_only_text_valid() {
        assert!(is_timestamp_only_text("[00:00:00]"));
        assert!(is_timestamp_only_text("[00:01:30]"));
        assert!(is_timestamp_only_text("[01:23:45]"));
        assert!(is_timestamp_only_text("[99:59:59]"));
    }

    #[test]
    fn is_timestamp_only_text_invalid() {
        assert!(!is_timestamp_only_text("Hello world"));
        assert!(!is_timestamp_only_text("00:01:30"));        // No brackets
        assert!(!is_timestamp_only_text("[00:01]"));          // Too short
        assert!(!is_timestamp_only_text("[00:01:30:00]"));    // Too many parts
        assert!(!is_timestamp_only_text("[aa:bb:cc]"));       // Not digits
        assert!(!is_timestamp_only_text(""));                 // Empty
        assert!(!is_timestamp_only_text("[0:1:3]"));          // Wrong digit count
    }

    #[test]
    fn timestamp_only_segments_get_css_class_in_html() {
        let segments = vec![
            Segment {
                index: 0,
                timestamp: "00:00:00".to_string(),
                text: "[00:00:00]".to_string(),
                image: "frame_0000.jpg".to_string(),
            },
            Segment {
                index: 1,
                timestamp: "00:01:30".to_string(),
                text: "실제 자막 텍스트".to_string(),
                image: "frame_0001.jpg".to_string(),
            },
        ];
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();

        // Timestamp-only segment should have the CSS class
        assert!(html.contains("class=\"timestamp-only\""));
        // Regular subtitle should NOT have the class
        assert!(html.contains("<p>실제 자막 텍스트</p>"));
    }

    #[test]
    fn timestamp_only_css_style_present() {
        let html = render_slides_html(&sample_segments(), &sample_metadata()).unwrap();
        assert!(html.contains(".timestamp-only"));
        assert!(html.contains("var(--fp-font-mono)"));
    }

    #[test]
    fn toc_preview_shows_timestamp_for_no_subtitle() {
        let segments = vec![Segment {
            index: 0,
            timestamp: "00:01:30".to_string(),
            text: "[00:01:30]".to_string(),
            image: "frame_0000.jpg".to_string(),
        }];
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();
        // TOC should show the bracketed timestamp as preview text
        assert!(html.contains("[00:01:30]"));
    }

    #[test]
    fn mixed_subtitle_and_timestamp_segments() {
        use crate::subtitle_extractor::SubtitleCue;

        let frames = vec![
            CapturedFrame {
                index: 0,
                timestamp_secs: 0.0,
                timestamp: "00:00:00".to_string(),
                filename: "frame_0000.jpg".to_string(),
            },
            CapturedFrame {
                index: 1,
                timestamp_secs: 30.0,
                timestamp: "00:00:30".to_string(),
                filename: "frame_0001.jpg".to_string(),
            },
            CapturedFrame {
                index: 2,
                timestamp_secs: 60.0,
                timestamp: "00:01:00".to_string(),
                filename: "frame_0002.jpg".to_string(),
            },
        ];

        // Only one cue matches frame at 30s
        let cues = vec![SubtitleCue {
            start_secs: 30.0,
            end_secs: 35.0,
            text: "중간 자막".to_string(),
        }];

        let segments = frames_to_segments_with_subtitles(&frames, &cues);

        // First and last should be timestamp-only
        assert_eq!(segments[0].text, "[00:00:00]");
        assert_eq!(segments[1].text, "중간 자막");
        assert_eq!(segments[2].text, "[00:01:00]");

        // Render to HTML and check mixed styling
        let html = render_slides_html(&segments, &sample_metadata()).unwrap();
        assert!(html.contains("class=\"timestamp-only\""));
        assert!(html.contains("중간 자막"));
    }
}
