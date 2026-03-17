/**
 * @file slides-viewer.js
 * @description 슬라이드 뷰어 컴포넌트
 *
 * Tauri 웹뷰 내부에 slides.html을 로드하여 표시한다.
 * iframe 방식으로 독립형 slides.html을 앱 안에 렌더링하며,
 * 뒤로 가기 버튼으로 대시보드 뷰로 돌아갈 수 있다.
 * 백엔드 rewriting을 통해 로컬 이미지의 asset protocol URL 해석을 처리한다.
 */

const SlidesViewer = {
  /** 현재 로드된 영상 ID */
  currentVideoId: null,

  /**
   * 슬라이드 뷰어를 초기화한다.
   * 뷰어 패널의 이벤트 리스너를 설정한다.
   */
  init() {
    const backBtn = document.getElementById('viewer-back-btn');
    if (backBtn) {
      backBtn.addEventListener('click', () => this.close());
    }

    // 재시도 버튼
    const retryBtn = document.getElementById('viewer-retry-btn');
    if (retryBtn) {
      retryBtn.addEventListener('click', () => {
        if (this.currentVideoId) {
          this.open(this.currentVideoId);
        }
      });
    }

    // 외부 브라우저에서 열기 버튼
    const openExternalBtn = document.getElementById('viewer-open-external');
    if (openExternalBtn) {
      openExternalBtn.addEventListener('click', () => this._openExternal());
    }

    // 키보드 단축키: Escape로 뷰어 닫기
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && this.isOpen()) {
        this.close();
      }
    });
  },

  /**
   * 뷰어 패널이 현재 표시 중인지 확인한다.
   * @returns {boolean}
   */
  isOpen() {
    const viewer = document.getElementById('slides-viewer');
    return viewer && !viewer.hidden;
  },

  /**
   * 주어진 영상 ID에 대한 슬라이드 뷰어를 연다.
   * Tauri 명령을 통해 slides.html을 로드하고 iframe에 렌더링한다.
   * 백엔드가 이미지 경로를 Tauri asset protocol URL로 재작성하여
   * 로컬 파일이 웹뷰에서 올바르게 표시된다.
   * @param {string} videoId
   * @param {number} [frameIndex] - 로드 후 스크롤할 0부터 시작하는 프레임 인덱스 (선택사항)
   */
  async open(videoId, frameIndex) {
    const viewer = document.getElementById('slides-viewer');
    const dashboard = document.getElementById('dashboard-view');
    const iframe = document.getElementById('slides-iframe');
    const viewerTitle = document.getElementById('viewer-title');
    const viewerError = document.getElementById('viewer-error');
    const viewerErrorText = document.getElementById('viewer-error-text');
    const loadingOverlay = document.getElementById('viewer-loading-overlay');
    const slideCountEl = document.getElementById('viewer-slide-count');
    const openExternalBtn = document.getElementById('viewer-open-external');

    if (!viewer || !iframe) {
      console.error('Slides viewer elements not found');
      return;
    }

    // 로딩 상태 표시
    if (viewerError) viewerError.hidden = true;
    if (loadingOverlay) loadingOverlay.hidden = false;
    iframe.hidden = true;
    iframe.srcdoc = '';
    viewerTitle.textContent = t('viewer_loading');
    if (slideCountEl) slideCountEl.textContent = '';
    if (openExternalBtn) openExternalBtn.hidden = true;
    dashboard.hidden = true;
    viewer.hidden = false;
    this.currentVideoId = videoId;

    try {
      let html;
      if (window.__TAURI__) {
        // 백엔드가 slides.html을 읽고, image src="images/..."를
        // https://asset.localhost/... URL로 재작성하며, iframe srcdoc에서
        // 올바른 asset 해석을 위해 <base> 태그와 CSP 메타를 주입한다.
        html = await window.__TAURI__.core.invoke('load_slides_html', { videoId });
      } else {
        // Tauri 없는 개발 환경용 폴백
        html = this._generateSampleSlides(videoId);
      }

      // srcdoc를 사용하여 HTML 콘텐츠를 iframe에 작성한다.
      // iframe의 sandbox="allow-scripts allow-same-origin" 속성이
      // slides.html의 스크립트가 격리된 상태에서 실행되도록 보장한다.
      iframe.srcdoc = html;

      // iframe 로드 후 제목 및 메타데이터 업데이트
      iframe.onload = () => {
        // 로딩 숨기기, iframe 표시
        if (loadingOverlay) loadingOverlay.hidden = true;
        iframe.hidden = false;

        try {
          const iframeDoc = iframe.contentDocument;
          const iframeTitle = iframeDoc?.title;
          if (iframeTitle) {
            viewerTitle.textContent = iframeTitle;
          } else {
            viewerTitle.textContent = videoId;
          }

          // 로드된 문서의 슬라이드 수를 세어 툴바에 표시
          const slides = iframeDoc?.querySelectorAll('.slide');
          if (slides && slides.length > 0 && slideCountEl) {
            slideCountEl.textContent = t('viewer_slide_count', { n: slides.length });
          }
        } catch (e) {
          // 크로스 오리진 또는 기타 접근 오류 — videoId만 표시
          viewerTitle.textContent = videoId;
        }

        // 외부 열기 버튼 표시 (Tauri 컨텍스트에서만)
        if (openExternalBtn && window.__TAURI__) {
          openExternalBtn.hidden = false;
        }

        // frameIndex가 제공된 경우 특정 프레임으로 이동
        if (typeof frameIndex === 'number' && frameIndex >= 0) {
          try {
            const iframeWin = iframe.contentWindow;
            if (iframeWin) {
              // slides.html 스크립트가 처리하는 해시 탐색 사용
              iframeWin.location.hash = '#slide-' + frameIndex;
            }
          } catch (e) {
            console.warn('Could not navigate to frame index:', e);
          }
        }
      };

      // iframe 로드 오류 처리
      iframe.onerror = () => {
        if (loadingOverlay) loadingOverlay.hidden = true;
        iframe.hidden = true;
        this._showError(t('viewer_error'));
      };
    } catch (err) {
      console.error('Failed to load slides:', err);
      if (loadingOverlay) loadingOverlay.hidden = true;
      iframe.hidden = true;
      const errorMsg = typeof err === 'string' ? err : err.message || t('viewer_error');
      this._showError(errorMsg);
      viewerTitle.textContent = t('viewer_error_title');
    }
  },

  /**
   * 뷰어에 재시도 옵션과 함께 오류 메시지를 표시한다.
   * @param {string} message
   */
  _showError(message) {
    const viewerError = document.getElementById('viewer-error');
    const viewerErrorText = document.getElementById('viewer-error-text');
    if (viewerError) {
      viewerError.hidden = false;
      if (viewerErrorText) {
        viewerErrorText.textContent = message;
      }
    }
  },

  /**
   * 뷰어를 닫고 대시보드로 돌아간다.
   */
  close() {
    const viewer = document.getElementById('slides-viewer');
    const dashboard = document.getElementById('dashboard-view');
    const iframe = document.getElementById('slides-iframe');
    const loadingOverlay = document.getElementById('viewer-loading-overlay');

    if (viewer) viewer.hidden = true;
    if (dashboard) dashboard.hidden = false;
    if (iframe) {
      iframe.srcdoc = '';
      iframe.hidden = false;
    }
    if (loadingOverlay) loadingOverlay.hidden = true;

    this.currentVideoId = null;
  },

  /**
   * 기본 외부 브라우저에서 slides.html 파일을 연다.
   * 독립형 파일은 상대 이미지 경로를 사용하므로 독립적으로 작동한다.
   */
  async _openExternal() {
    if (!this.currentVideoId || !window.__TAURI__) return;

    try {
      // 경로를 해석하고 브라우저에서 여는 통합 백엔드 명령 사용
      await window.__TAURI__.core.invoke('open_slides_external', {
        videoId: this.currentVideoId,
      });
    } catch (err) {
      console.error('Failed to open slides externally:', err);
      // 토스트 및 버튼 툴팁으로 간단한 오류 피드백 표시
      if (typeof showToast === 'function') {
        showToast(t('viewer_open_failed'), 'error');
      }
      const openBtn = document.getElementById('viewer-open-external');
      if (openBtn) {
        const original = openBtn.title;
        openBtn.title = t('viewer_open_failed');
        setTimeout(() => { openBtn.title = original; }, 2000);
      }
    }
  },

  /**
   * Tauri 백엔드 없이 개발/테스트용 샘플 슬라이드 HTML을 생성한다.
   * Rust slides_generator가 생성하는 구조와 동일하다.
   * @param {string} videoId
   * @returns {string}
   */
  _generateSampleSlides(videoId) {
    return `<!DOCTYPE html>
<html lang="ko">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Sample Slides - ${videoId}</title>
<style>
/* === Reset & Base === */
*,*::before,*::after{ margin:0; padding:0; box-sizing:border-box; }
html{ scroll-behavior:smooth; height:100%; }
body{
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans KR', sans-serif;
  background: #0f0f0f;
  color: #e1e1e1;
  overflow: hidden;
  height: 100vh;
}
.main-content{ scroll-behavior:smooth; }

/* === Layout: sidebar + main === */
.app-layout{ display: flex; height: 100vh; }

/* --- Sidebar --- */
.sidebar{
  width: 300px; min-width: 260px;
  background: #161622;
  border-right: 1px solid #2a2a3e;
  display: flex; flex-direction: column;
  overflow: hidden; flex-shrink: 0;
}
.sidebar-header{
  padding: 20px 16px 12px;
  border-bottom: 1px solid #2a2a3e;
}
.sidebar-header h1{
  font-size: 1rem; color: #fff; line-height: 1.4; margin-bottom: 6px;
  overflow: hidden; text-overflow: ellipsis;
  display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical;
}
.sidebar-header .meta{ font-size: 0.78rem; color: #888; margin-bottom: 6px; }

.toc-title{
  padding: 10px 16px 6px;
  font-size: 0.8rem; color: #7c83ff;
  font-weight: 600; text-transform: uppercase; letter-spacing: 0.05em;
}

.toc-list{
  list-style: none; overflow-y: auto; flex: 1; padding: 0 8px 16px;
}
.toc-list::-webkit-scrollbar{ width:6px; }
.toc-list::-webkit-scrollbar-track{ background:transparent; }
.toc-list::-webkit-scrollbar-thumb{ background:#333; border-radius:3px; }
.toc-list::-webkit-scrollbar-thumb:hover{ background:#555; }
.toc-list li{ margin-bottom: 1px; }

.toc-link{
  display: flex; align-items: baseline; gap: 8px;
  padding: 6px 8px; border-radius: 6px;
  border-left: 3px solid transparent;
  text-decoration: none;
  transition: background 0.18s ease, border-color 0.18s ease, color 0.18s ease;
}
.toc-link:hover{ background: #1e1e32; }
.toc-link.active{
  background: #252540;
  border-left-color: #7c83ff;
}
.toc-link.active .toc-ts{ color: #9da3ff; }
.toc-link.active .toc-preview{ color: #ddd; }

.toc-ts{
  color: #7c83ff;
  font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  font-size: 0.75rem; font-weight: 600;
  white-space: nowrap; flex-shrink: 0;
}
.toc-preview{
  color: #999; font-size: 0.78rem;
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}

/* --- Main content area --- */
.main-content{ flex: 1; overflow-y: auto; padding: 0; }
.main-content::-webkit-scrollbar{ width:8px; }
.main-content::-webkit-scrollbar-track{ background:#0f0f0f; }
.main-content::-webkit-scrollbar-thumb{ background:#333; border-radius:4px; }
.main-content::-webkit-scrollbar-thumb:hover{ background:#555; }

.slides-container{ max-width: 900px; margin: 0 auto; padding: 24px 24px 48px; }

/* --- Slide card --- */
.slide{
  background: #1a1a2a; border-radius: 12px; margin-bottom: 20px;
  overflow: hidden; box-shadow: 0 2px 12px rgba(0,0,0,0.5);
  transition: transform 0.2s ease, box-shadow 0.2s ease, outline-color 0.25s ease;
  scroll-margin-top: 24px;
  outline: 2px solid transparent; outline-offset: -2px;
}
.slide:hover{
  transform: translateY(-2px);
  box-shadow: 0 4px 24px rgba(124,131,255,0.12);
}
.slide.highlight{ outline-color: #7c83ff; }

.slide-image{ position: relative; background: #000; line-height: 0; }
.slide-image .placeholder{
  width: 100%; height: 200px; display: flex;
  align-items: center; justify-content: center;
  color: #555; font-size: 0.9rem; background: #111;
}
.timestamp-badge{
  position: absolute; top: 10px; left: 10px;
  background: rgba(0,0,0,0.78); color: #7c83ff;
  padding: 4px 10px; border-radius: 6px;
  font-size: 0.82rem;
  font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  font-weight: 600; backdrop-filter: blur(4px);
}
.slide-number{
  position: absolute; top: 10px; right: 10px;
  background: rgba(0,0,0,0.6); color: #666;
  padding: 3px 8px; border-radius: 4px;
  font-size: 0.72rem; font-family: monospace;
}
.slide-text{ padding: 16px 20px; }
.slide-text p{ font-size: 0.95rem; line-height: 1.8; color: #d4d4d4; }
.footer{
  text-align: center; color: #444; padding: 16px;
  font-size: 0.78rem; border-top: 1px solid #1a1a1a;
}

/* --- Sidebar toggle (mobile) --- */
.sidebar-toggle{
  display: none; position: fixed; bottom: 20px; left: 20px; z-index: 1000;
  background: #7c83ff; color: #fff; border: none; border-radius: 50%;
  width: 48px; height: 48px; font-size: 1.3rem; cursor: pointer;
  box-shadow: 0 2px 12px rgba(124,131,255,0.4);
}
.kbd-hint{
  position: fixed; bottom: 12px; right: 16px;
  color: #444; font-size: 0.7rem; font-family: monospace; pointer-events: none;
}

/* === Responsive === */
@media (max-width: 768px){
  .sidebar{
    position: fixed; left: -300px; top: 0; height: 100vh; z-index: 900;
    transition: left 0.25s ease;
  }
  .sidebar.open{ left: 0; box-shadow: 4px 0 24px rgba(0,0,0,0.6); }
  .sidebar-toggle{ display: flex; align-items: center; justify-content: center; }
  .slides-container{ padding: 16px 12px 48px; }
  .slide-text{ padding: 12px 14px; }
}
@media (max-width: 480px){
  .sidebar{ width: 260px; left: -260px; }
  .slide{ border-radius: 8px; }
}
</style>
</head>
<body>
<div class="app-layout">
  <aside class="sidebar" id="sidebar">
    <div class="sidebar-header">
      <h1>Sample Video: ${videoId}</h1>
      <div class="meta">Test Channel &middot; 2025-01-01 &middot; 10:30</div>
    </div>
    <div class="toc-title">목차 — 3개 슬라이드</div>
    <ul class="toc-list" id="tocList">
      <li><a href="javascript:void(0)" data-href="#slide-0" class="toc-link" data-slide="0"><span class="toc-ts">00:00:00</span><span class="toc-preview">안녕하세요, 테스트 슬라이드입니다</span></a></li>
      <li><a href="javascript:void(0)" data-href="#slide-1" class="toc-link" data-slide="1"><span class="toc-ts">00:03:15</span><span class="toc-preview">두 번째 슬라이드 내용</span></a></li>
      <li><a href="javascript:void(0)" data-href="#slide-2" class="toc-link" data-slide="2"><span class="toc-ts">00:07:42</span><span class="toc-preview">마지막 슬라이드입니다</span></a></li>
    </ul>
  </aside>
  <main class="main-content" id="mainContent">
    <div class="slides-container" id="slidesContainer">
      <div class="slide" id="slide-0" data-index="0">
        <div class="slide-image">
          <div class="placeholder">Frame Placeholder 1</div>
          <span class="timestamp-badge">00:00:00</span>
          <span class="slide-number">#1</span>
        </div>
        <div class="slide-text"><p>안녕하세요, 테스트 슬라이드입니다. 이것은 Tauri 웹뷰에서 slides.html이 올바르게 렌더링되는지 확인하기 위한 샘플입니다.</p></div>
      </div>
      <div class="slide" id="slide-1" data-index="1">
        <div class="slide-image">
          <div class="placeholder">Frame Placeholder 2</div>
          <span class="timestamp-badge">00:03:15</span>
          <span class="slide-number">#2</span>
        </div>
        <div class="slide-text"><p>두 번째 슬라이드 내용입니다. 다크 테마가 일관되게 적용되었는지 확인합니다.</p></div>
      </div>
      <div class="slide" id="slide-2" data-index="2">
        <div class="slide-image">
          <div class="placeholder">Frame Placeholder 3</div>
          <span class="timestamp-badge">00:07:42</span>
          <span class="slide-number">#3</span>
        </div>
        <div class="slide-text"><p>마지막 슬라이드입니다. 키보드 탐색 (← → / j k)이 작동하는지도 확인해주세요.</p></div>
      </div>
    </div>
    <div class="footer">3개 슬라이드 &middot; Generated by framepick (sample)</div>
  </main>
</div>
<button class="sidebar-toggle" id="sidebarToggle" aria-label="Toggle sidebar">☰</button>
<div class="kbd-hint">← → / j k 키로 탐색</div>
<script>
(function(){
  'use strict';
  var slides = document.querySelectorAll('.slide');
  var tocLinks = document.querySelectorAll('.toc-link');
  var mainContent = document.getElementById('mainContent');
  var sidebar = document.getElementById('sidebar');
  var sidebarToggle = document.getElementById('sidebarToggle');
  var tocList = document.getElementById('tocList');
  var currentIndex = 0;

  /* Build index maps for fast lookup */
  var slideByIdx = {};
  var linkByIdx = {};
  slides.forEach(function(s) { slideByIdx[s.dataset.index] = s; });
  tocLinks.forEach(function(l) { linkByIdx[l.dataset.slide] = l; });

  /* Debounce helper for scroll-spy updates */
  var scrollSpyTimer = null;
  function debouncedSetActive(idx) {
    if (scrollSpyTimer) clearTimeout(scrollSpyTimer);
    scrollSpyTimer = setTimeout(function() {
      setActive(idx, false);
    }, 50);
  }

  /* Scroll-spy via IntersectionObserver with scroll fallback */
  var observerActive = false;

  if (typeof IntersectionObserver !== 'undefined' && mainContent) {
    var observer = new IntersectionObserver(function(entries) {
      var topMost = null;
      entries.forEach(function(entry) {
        if (entry.isIntersecting) {
          var idx = parseInt(entry.target.dataset.index, 10);
          if (!isNaN(idx) && (topMost === null || idx < topMost)) {
            topMost = idx;
          }
        }
      });
      if (topMost !== null) {
        observerActive = true;
        debouncedSetActive(topMost);
      }
    }, { root: mainContent, rootMargin: '-10% 0px -70% 0px', threshold: 0 });
    slides.forEach(function(slide) { observer.observe(slide); });
  }

  /* Fallback scroll listener */
  mainContent.addEventListener('scroll', function() {
    if (observerActive) return;
    var scrollTop = mainContent.scrollTop;
    var closest = 0;
    var closestDist = Infinity;
    slides.forEach(function(slide, i) {
      var dist = Math.abs(slide.offsetTop - scrollTop - 24);
      if (dist < closestDist) { closestDist = dist; closest = i; }
    });
    debouncedSetActive(closest);
  }, { passive: true });

  /* Set active slide + TOC highlight */
  function setActive(idx, doScroll) {
    if (idx < 0 || idx >= slides.length) return;
    currentIndex = idx;

    tocLinks.forEach(function(link) {
      var isActive = parseInt(link.dataset.slide, 10) === idx;
      link.classList.toggle('active', isActive);
    });

    /* Scroll the TOC sidebar so active item is visible */
    var activeLink = linkByIdx[idx];
    if (activeLink && tocList) {
      var listRect = tocList.getBoundingClientRect();
      var linkRect = activeLink.getBoundingClientRect();
      if (linkRect.top < listRect.top || linkRect.bottom > listRect.bottom) {
        activeLink.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      }
    }

    /* Highlight the active slide card */
    slides.forEach(function(s) { s.classList.remove('highlight'); });
    if (slides[idx]) slides[idx].classList.add('highlight');

    /* Scroll main content to the slide */
    if (doScroll && slides[idx]) {
      slides[idx].scrollIntoView({ behavior: 'smooth', block: 'start' });
    }

    /* Update URL hash without triggering scroll */
    var newHash = '#slide-' + idx;
    if (window.location.hash !== newHash) {
      if (history.replaceState) {
        history.replaceState(null, '', newHash);
      }
    }
  }

  /* TOC click handler (event delegation) */
  tocList.addEventListener('click', function(e) {
    var link = e.target.closest('.toc-link');
    if (!link) return;
    e.preventDefault();
    var idx = parseInt(link.dataset.slide, 10);
    if (!isNaN(idx)) {
      setActive(idx, true);
      if (window.innerWidth <= 768) sidebar.classList.remove('open');
    }
  });

  /* Hash-based navigation */
  function navigateToHash() {
    var hash = window.location.hash;
    if (hash && hash.indexOf('#slide-') === 0) {
      var idx = parseInt(hash.replace('#slide-', ''), 10);
      if (!isNaN(idx) && idx >= 0 && idx < slides.length) {
        setActive(idx, true);
      }
    }
  }
  window.addEventListener('hashchange', navigateToHash);
  if (window.location.hash) { setTimeout(navigateToHash, 80); }

  /* Keyboard navigation */
  document.addEventListener('keydown', function(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    var handled = false;
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown' || e.key === 'j') {
      if (currentIndex < slides.length - 1) { setActive(currentIndex + 1, true); handled = true; }
    } else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp' || e.key === 'k') {
      if (currentIndex > 0) { setActive(currentIndex - 1, true); handled = true; }
    } else if (e.key === 'Home') {
      setActive(0, true); handled = true;
    } else if (e.key === 'End') {
      setActive(slides.length - 1, true); handled = true;
    }
    if (handled) e.preventDefault();
  });

  /* Mobile sidebar toggle */
  sidebarToggle.addEventListener('click', function() { sidebar.classList.toggle('open'); });
  mainContent.addEventListener('click', function() {
    if (window.innerWidth <= 768) sidebar.classList.remove('open');
  });

  /* Initialize */
  if (!window.location.hash && slides.length > 0) { setActive(0, false); }
})();
</script>
</body>
</html>`;
  }
};
