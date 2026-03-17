/**
 * @file capture-list.js
 * @description framepick 캡쳐된 프레임 목록 컴포넌트
 *
 * 파이프라인 완료 후 캡쳐된 프레임 목록을 썸네일 및 메타데이터와 함께 표시한다.
 * 사용자가 전체 slides.html 뷰어를 열기 전에 프레임을 미리 볼 수 있다.
 *
 * 주요 기능:
 *   - 타임스탬프 배지가 있는 프레임 썸네일 그리드 뷰
 *   - 호버/프레임 아래에 자막 텍스트 미리보기
 *   - 캡쳐 모드 + 프레임 수 메타데이터 헤더
 *   - 프레임 클릭 시 해당 인덱스의 슬라이드 뷰어 열기
 *   - 캡쳐 완료 후 자동으로 표시되는 접이식 패널
 *   - 언어 변경에 반응
 *
 * 데이터 소스:
 *   - 라이브러리 항목 폴더의 segments.json + images/에서 읽기
 *   - 메타데이터용 Tauri `get_slides_metadata` 명령 사용
 *   - 이미지 썸네일용 asset protocol URL 사용
 *
 * 수신 이벤트:
 *   - queueItemCompleted: 완료된 항목의 캡쳐 목록 자동 표시
 *   - languageChanged: 레이블 재렌더링
 *
 * 발행 이벤트:
 *   - captureListFrameClicked: { videoId, frameIndex } - 사용자가 프레임 클릭 시
 */

const CaptureList = (() => {
  let containerEl = null;
  let currentVideoId = null;
  let currentSegments = [];
  let currentMetadata = null;
  let isExpanded = true;
  let viewMode = 'grid'; // 'grid' | 'list'
  let lightboxIdx = -1;  // -1 = 닫힘

  /**
   * 캡쳐 목록 컴포넌트를 초기화한다.
   * 컨테이너 요소를 생성하고 이벤트 리스너를 설정한다.
   */
  function init() {
    // 컨테이너 요소 찾기 또는 생성
    containerEl = document.getElementById('capture-list-container');
    if (!containerEl) {
      // 큐 컨테이너 다음에 캡쳐 목록 컨테이너 삽입
      const queueContainer = document.getElementById('queue-container');
      if (queueContainer) {
        containerEl = document.createElement('div');
        containerEl.id = 'capture-list-container';
        queueContainer.insertAdjacentElement('afterend', containerEl);
      } else {
        console.warn('CaptureList: No queue-container found for insertion');
        return;
      }
    }

    // 큐 항목 완료 시 캡쳐 목록 자동 표시를 위한 이벤트 리스너
    document.addEventListener('queueItemCompleted', _onItemCompleted);

    // 언어 변경 시 재렌더링
    document.addEventListener('languageChanged', () => {
      if (currentVideoId && currentSegments.length > 0) {
        _render();
      }
    });
  }

  /**
   * 큐 항목 완료 이벤트를 처리한다 — 캡쳐된 프레임을 로드하고 표시한다.
   * @param {CustomEvent} event
   */
  async function _onItemCompleted(event) {
    const { id } = event.detail;

    // 영상 ID를 찾기 위해 큐 항목 가져오기
    const queue = typeof QueueUI !== 'undefined' ? QueueUI.getQueue() : [];
    const item = queue.find(q => q.id === id);

    if (!item) return;

    // URL에서 영상 ID 추출
    const videoId = item.videoId || _extractVideoId(item.url);
    if (!videoId) return;

    // 이 영상의 캡쳐된 프레임 로드
    await loadFrames(videoId);
  }

  /**
   * 주어진 영상 ID에 대한 캡쳐된 프레임을 로드하고 표시한다.
   * @param {string} videoId - YouTube 영상 ID (라이브러리 폴더명)
   */
  async function loadFrames(videoId) {
    if (!containerEl) return;
    if (!videoId) return;

    currentVideoId = videoId;
    currentSegments = [];
    currentMetadata = null;

    // 로딩 상태 표시
    _renderLoading();

    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        // segments.json을 읽고 asset protocol 썸네일 URL을 생성하는
        // 전용 get_capture_frames 명령 사용
        const result = await window.__TAURI__.core.invoke('get_capture_frames', {
          videoId: videoId
        });

        if (result && result.frames && result.frames.length > 0) {
          currentMetadata = {
            video_id: result.video_id,
            title: result.title,
            slide_count: result.frame_count,
          };
          currentSegments = result.frames.map((f, idx) => ({
            index: f.index != null ? f.index : idx,
            timestamp: f.timestamp || '',
            text: f.text || '',
            image: f.image || '',
            thumbnailUrl: f.thumbnail_url || '',
          }));
        } else {
          // 폴백: 기본 이미지 목록을 위해 get_slides_metadata 시도
          currentMetadata = await window.__TAURI__.core.invoke('get_slides_metadata', {
            videoId: videoId
          });
          if (currentMetadata) {
            currentSegments = (currentMetadata.images || []).map((img, idx) => ({
              index: idx,
              timestamp: _formatTimestampFromFilename(img),
              text: '',
              image: img,
              thumbnailUrl: '',
            }));
          }
        }
      }

      isExpanded = true;
      _render();
    } catch (err) {
      console.warn('CaptureList: Failed to load frames:', err);
      _renderError(String(err));
    }
  }

  /**
   * 백엔드를 통해 영상 항목의 세그먼트 데이터를 로드한다.
   * @param {string} videoId
   * @returns {Promise<Array|null>}
   */
  async function _loadSegments(videoId) {
    if (!window.__TAURI__ || !window.__TAURI__.core) return null;

    try {
      const result = await window.__TAURI__.core.invoke('get_capture_frames', {
        videoId: videoId
      });
      if (result && result.frames) {
        return result.frames.map((f, idx) => ({
          index: f.index != null ? f.index : idx,
          timestamp: f.timestamp || '',
          text: f.text || '',
          image: f.image || '',
          thumbnailUrl: f.thumbnail_url || '',
        }));
      }
      return null;
    } catch (err) {
      return null;
    }
  }

  /**
   * YouTube URL에서 영상 ID를 추출한다.
   * @param {string} url
   * @returns {string|null}
   */
  function _extractVideoId(url) {
    if (!url) return null;
    const match = url.match(/(?:v=|\/shorts\/|youtu\.be\/)([\w-]{11})/);
    return match ? match[1] : null;
  }

  /**
   * 이미지 파일명에서 사람이 읽기 쉬운 타임스탬프를 추출한다.
   * 예: "frame_0001_00-01-23.jpg" → "00:01:23"
   * @param {string} filename
   * @returns {string}
   */
  function _formatTimestampFromFilename(filename) {
    // 파일명에서 "00-01-23" 패턴 매칭
    const match = filename.match(/(\d{2})-(\d{2})-(\d{2})/);
    if (match) {
      return `${match[1]}:${match[2]}:${match[3]}`;
    }
    return '';
  }

  /**
   * 라이브러리의 이미지에 대한 asset protocol URL을 생성한다.
   * @param {string} videoId
   * @param {string} imageName
   * @returns {string}
   */
  function _buildImageUrl(videoId, imageName) {
    // get_slides_metadata는 images/ 디렉터리 기준 상대 이미지명을 반환
    // 백엔드를 통해 전체 asset URL을 생성해야 함
    // 현재는 라이브러리 경로 규칙 사용
    if (currentMetadata && currentMetadata.video_id) {
      // Tauri 컨텍스트에서 이미지는 asset protocol로 제공됨
      // slides_viewer.rs에서 이미 처리하는 방식과 동일하게 접근
      // 캡쳐 목록 썸네일은 상대 경로로 참조하고 백엔드가 해석하도록 함
      return `asset://localhost/images/${imageName}`;
    }
    return `images/${imageName}`;
  }

  /**
   * 로딩 상태를 렌더링한다.
   */
  function _renderLoading() {
    if (!containerEl) return;
    containerEl.innerHTML = `
      <div class="capture-list-panel">
        <div class="capture-list-loading">
          <div class="viewer-spinner"></div>
          <span>${t('capture_list_loading')}</span>
        </div>
      </div>
    `;
  }

  /**
   * 오류 상태를 렌더링한다.
   * @param {string} message
   */
  function _renderError(message) {
    if (!containerEl) return;
    containerEl.innerHTML = `
      <div class="capture-list-panel">
        <div class="capture-list-error">
          <span>${_escapeHtml(message)}</span>
        </div>
      </div>
    `;
    // 5초 후 자동 숨기기
    setTimeout(() => {
      if (containerEl) containerEl.innerHTML = '';
    }, 5000);
  }

  /**
   * 메인 렌더 함수 — 전체 캡쳐 목록 UI를 생성한다.
   */
  function _render() {
    if (!containerEl || !currentVideoId) return;

    const frameCount = currentSegments.length;
    if (frameCount === 0) {
      containerEl.innerHTML = '';
      return;
    }

    const title = (currentMetadata && currentMetadata.title) || currentVideoId;
    const captureMode = _getCaptureMode();

    let html = '<div class="capture-list-panel">';

    // 토글이 포함된 헤더
    html += '<div class="capture-list-header">';
    html += '<div class="capture-list-header-info">';
    html += `<h3 class="capture-list-title">`;
    html += `<svg class="capture-list-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>`;
    html += ` <span data-i18n="capture_list_title">${t('capture_list_title')}</span>`;
    html += `</h3>`;
    html += `<span class="capture-list-meta">`;
    html += `<span class="capture-list-count">${t('capture_list_count', { n: frameCount })}</span>`;
    if (captureMode) {
      html += ` · <span class="capture-list-mode">${_escapeHtml(captureMode)}</span>`;
    }
    html += `</span>`;
    html += '</div>';
    html += '<div class="capture-list-actions">';
    // 뷰 모드 전환 버튼 (그리드/목록)
    html += '<div class="capture-list-view-toggle">';
    html += `<button class="capture-list-view-mode-btn${viewMode === 'grid' ? ' active' : ''}" data-action="view-grid" title="Grid">`;
    html += '<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><rect x="1" y="1" width="6" height="6" rx="1"/><rect x="9" y="1" width="6" height="6" rx="1"/><rect x="1" y="9" width="6" height="6" rx="1"/><rect x="9" y="9" width="6" height="6" rx="1"/></svg>';
    html += '</button>';
    html += `<button class="capture-list-view-mode-btn${viewMode === 'list' ? ' active' : ''}" data-action="view-list" title="List">`;
    html += '<svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><rect x="1" y="2" width="14" height="3" rx="1"/><rect x="1" y="7" width="14" height="3" rx="1"/><rect x="1" y="12" width="14" height="3" rx="1"/></svg>';
    html += '</button>';
    html += '</div>';
    html += `<button class="btn-secondary capture-list-view-btn" data-action="view-slides" title="${t('capture_list_view_slides')}">${t('capture_list_view_slides')}</button>`;
    html += `<button class="btn-icon capture-list-toggle" data-action="toggle" title="${isExpanded ? t('capture_list_collapse') : t('capture_list_expand')}">`;
    html += isExpanded
      ? '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="18 15 12 9 6 15"/></svg>'
      : '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>';
    html += '</button>';
    html += '</div>';
    html += '</div>';

    // 프레임 그리드 또는 목록 (접이식)
    if (isExpanded) {
      if (viewMode === 'list') {
        html += '<div class="capture-list-rows">';
        currentSegments.forEach((seg, idx) => {
          const ts = seg.timestamp || '';
          const text = seg.text || '';
          const thumbUrl = seg.thumbnailUrl || '';
          const frameNum = idx + 1;

          html += `<div class="capture-list-row" data-frame-index="${idx}">`;
          html += `<span class="capture-list-row-index">#${frameNum}</span>`;
          html += '<div class="capture-list-row-thumb">';
          if (thumbUrl) {
            html += `<img src="${_escapeAttr(thumbUrl)}" alt="Frame ${frameNum}" loading="lazy" onerror="this.style.display='none'">`;
          }
          html += '</div>';
          html += '<div class="capture-list-row-info">';
          html += `<div class="capture-list-row-text${text ? '' : ' muted'}">${_escapeHtml(text || ts || '—')}</div>`;
          if (ts) {
            html += `<div class="capture-list-row-ts">${_escapeHtml(ts)}</div>`;
          }
          html += '</div>';
          html += '</div>';
        });
        html += '</div>';
      } else {
        // 그리드 뷰 (기본값)
        html += '<div class="capture-list-grid">';
        currentSegments.forEach((seg, idx) => {
          const ts = seg.timestamp || '';
          const text = seg.text || '';
          const imgSrc = seg.image || '';
          const thumbUrl = seg.thumbnailUrl || '';
          const frameNum = idx + 1;

          html += `<div class="capture-list-frame" data-frame-index="${idx}" title="${_escapeAttr(text || ts)}">`;
          html += '<div class="capture-list-thumb-wrap">';
          if (thumbUrl) {
            html += `<div class="capture-list-thumb-placeholder">`;
            html += `<img class="capture-list-thumb" src="${_escapeAttr(thumbUrl)}" alt="Frame ${frameNum}" loading="lazy" onerror="this.style.display='none'">`;
            html += `<span class="capture-list-frame-num">#${frameNum}</span>`;
            html += '</div>';
          } else if (imgSrc) {
            html += `<div class="capture-list-thumb-placeholder" data-video-id="${_escapeAttr(currentVideoId)}" data-image="${_escapeAttr(imgSrc)}">`;
            html += `<span class="capture-list-frame-num">#${frameNum}</span>`;
            html += '</div>';
          } else {
            html += `<div class="capture-list-thumb-placeholder"><span class="capture-list-frame-num">#${frameNum}</span></div>`;
          }
          if (ts) {
            html += `<span class="capture-list-ts-badge">${_escapeHtml(ts)}</span>`;
          }
          html += '</div>';
          if (text) {
            html += `<div class="capture-list-text" title="${_escapeAttr(text)}">${_escapeHtml(_truncate(text, 60))}</div>`;
          }
          html += '</div>';
        });
        html += '</div>';
      }

      // 요약 푸터
      html += '<div class="capture-list-footer">';
      html += `<span class="capture-list-video-title" title="${_escapeAttr(title)}">${_escapeHtml(_truncate(title, 50))}</span>`;
      html += '</div>';
    }

    html += '</div>';

    containerEl.innerHTML = html;

    // 실제 썸네일 비동기 로드
    _loadThumbnails();

    // 이벤트 바인딩
    _bindEvents();
  }

  /**
   * asset protocol에서 실제 썸네일 이미지를 로드한다.
   * 플레이스홀더 div를 실제 <img> 요소로 업데이트한다.
   */
  async function _loadThumbnails() {
    if (!containerEl || !currentVideoId) return;

    const placeholders = containerEl.querySelectorAll('.capture-list-thumb-placeholder[data-image]');
    if (placeholders.length === 0) return;

    // asset URL 생성을 위한 라이브러리 기본 경로 가져오기
    let libraryBasePath = null;
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        const slidesPath = await window.__TAURI__.core.invoke('get_slides_path', {
          videoId: currentVideoId
        });
        if (slidesPath) {
          // slides.html이 포함된 디렉터리 추출
          libraryBasePath = slidesPath.replace(/[/\\]slides\.html$/, '');
        }
      } catch (e) {
        // slides.html이 아직 없을 수 있음 — 백엔드에서 절대경로를 직접 받기
        try {
          const libPath = await window.__TAURI__.core.invoke('get_resolved_library_path');
          if (libPath) {
            libraryBasePath = libPath + '/' + currentVideoId;
          }
        } catch (e2) {
          console.warn('CaptureList: Cannot resolve library path for thumbnails');
        }
      }
    }

    if (!libraryBasePath) return;

    // 경로 구분자 정규화
    const basePath = libraryBasePath.replace(/\\/g, '/');

    placeholders.forEach(placeholder => {
      const imageName = placeholder.dataset.image;
      if (!imageName) return;

      const imagePath = `${basePath}/images/${imageName}`;
      // Tauri asset protocol URL 생성
      const encodedPath = _percentEncodePath(imagePath);
      const assetUrl = `https://asset.localhost/${encodedPath}`;

      const img = document.createElement('img');
      img.className = 'capture-list-thumb';
      img.alt = `Frame ${imageName}`;
      img.loading = 'lazy';
      img.src = assetUrl;
      img.onerror = function() {
        // 오류 시 프레임 번호가 있는 플레이스홀더 유지
        this.style.display = 'none';
      };

      // 프레임 번호 배지 유지
      const badge = placeholder.querySelector('.capture-list-frame-num');
      placeholder.innerHTML = '';
      placeholder.appendChild(img);
      if (badge) {
        placeholder.appendChild(badge);
      }
    });
  }

  /**
   * asset protocol URL에 사용하기 위해 파일 경로를 퍼센트 인코딩한다.
   * @param {string} pathStr
   * @returns {string}
   */
  function _percentEncodePath(pathStr) {
    let encoded = '';
    for (const ch of pathStr) {
      if (/[A-Za-z0-9\-_.~/:]/.test(ch)) {
        encoded += ch;
      } else if (ch === ' ') {
        encoded += '%20';
      } else {
        // 각 UTF-8 바이트 인코딩
        const bytes = new TextEncoder().encode(ch);
        for (const b of bytes) {
          encoded += '%' + b.toString(16).toUpperCase().padStart(2, '0');
        }
      }
    }
    return encoded;
  }

  /**
   * 렌더링된 캡쳐 목록에 클릭 이벤트를 바인딩한다.
   */
  function _bindEvents() {
    if (!containerEl) return;

    // 접기/펼치기 토글
    containerEl.querySelectorAll('[data-action="toggle"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        isExpanded = !isExpanded;
        _render();
      });
    });

    // 뷰 모드 전환 (그리드/목록)
    containerEl.querySelectorAll('[data-action="view-grid"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (viewMode !== 'grid') { viewMode = 'grid'; _render(); }
      });
    });
    containerEl.querySelectorAll('[data-action="view-list"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (viewMode !== 'list') { viewMode = 'list'; _render(); }
      });
    });

    // 슬라이드 보기 버튼 — 슬라이드 뷰어에서 열기
    containerEl.querySelectorAll('[data-action="view-slides"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (currentVideoId && typeof SlidesViewer !== 'undefined') {
          SlidesViewer.open(currentVideoId);
        }
      });
    });

    // 개별 프레임 클릭 — 라이트박스 미리보기 열기
    const frameEls = containerEl.querySelectorAll('.capture-list-frame, .capture-list-row');
    frameEls.forEach(frame => {
      frame.addEventListener('click', () => {
        const frameIndex = parseInt(frame.dataset.frameIndex, 10);
        if (isNaN(frameIndex)) return;

        // 다른 컴포넌트를 위한 이벤트 발행
        document.dispatchEvent(new CustomEvent('captureListFrameClicked', {
          detail: { videoId: currentVideoId, frameIndex: frameIndex }
        }));

        // 라이트박스 미리보기 열기
        _openLightbox(frameIndex);
      });
    });
  }

  // ── 라이트박스 미리보기 ──────────────────────────────────────────

  /**
   * 주어진 프레임 인덱스에서 라이트박스를 연다.
   */
  function _openLightbox(idx) {
    if (idx < 0 || idx >= currentSegments.length) return;
    lightboxIdx = idx;

    // 라이트박스 오버레이 찾기 또는 생성
    let overlay = document.getElementById('capture-lightbox');
    if (!overlay) {
      overlay = document.createElement('div');
      overlay.id = 'capture-lightbox';
      overlay.className = 'capture-lightbox-overlay';
      overlay.innerHTML = [
        '<div class="capture-lightbox-backdrop"></div>',
        '<div class="capture-lightbox-content">',
        '  <button class="capture-lightbox-close" aria-label="Close">\u00d7</button>',
        '  <button class="capture-lightbox-nav capture-lightbox-prev" aria-label="Previous">\u2039</button>',
        '  <div class="capture-lightbox-body">',
        '    <img class="capture-lightbox-img" src="" alt="">',
        '    <div class="capture-lightbox-info">',
        '      <div class="capture-lightbox-info-row">',
        '        <span class="capture-lightbox-idx"></span>',
        '        <span class="capture-lightbox-ts"></span>',
        '      </div>',
        '      <p class="capture-lightbox-text"></p>',
        '    </div>',
        '  </div>',
        '  <button class="capture-lightbox-nav capture-lightbox-next" aria-label="Next">\u203a</button>',
        '</div>',
      ].join('');
      document.body.appendChild(overlay);

      // 라이트박스 이벤트 바인딩
      overlay.querySelector('.capture-lightbox-backdrop').addEventListener('click', _closeLightbox);
      overlay.querySelector('.capture-lightbox-close').addEventListener('click', _closeLightbox);
      overlay.querySelector('.capture-lightbox-prev').addEventListener('click', () => _navigateLightbox(-1));
      overlay.querySelector('.capture-lightbox-next').addEventListener('click', () => _navigateLightbox(1));
    }

    _updateLightbox();
    overlay.hidden = false;
    document.body.style.overflow = 'hidden';
  }

  function _closeLightbox() {
    lightboxIdx = -1;
    const overlay = document.getElementById('capture-lightbox');
    if (overlay) overlay.hidden = true;
    document.body.style.overflow = '';
  }

  function _navigateLightbox(delta) {
    const newIdx = lightboxIdx + delta;
    if (newIdx >= 0 && newIdx < currentSegments.length) {
      lightboxIdx = newIdx;
      _updateLightbox();
    }
  }

  function _updateLightbox() {
    const overlay = document.getElementById('capture-lightbox');
    if (!overlay || lightboxIdx < 0) return;
    const seg = currentSegments[lightboxIdx];
    if (!seg) return;

    const img = overlay.querySelector('.capture-lightbox-img');
    const idxEl = overlay.querySelector('.capture-lightbox-idx');
    const tsEl = overlay.querySelector('.capture-lightbox-ts');
    const textEl = overlay.querySelector('.capture-lightbox-text');
    const prevBtn = overlay.querySelector('.capture-lightbox-prev');
    const nextBtn = overlay.querySelector('.capture-lightbox-next');

    if (img) {
      img.src = seg.thumbnailUrl || '';
      img.alt = 'Frame ' + (lightboxIdx + 1);
    }
    if (idxEl) idxEl.textContent = '#' + (lightboxIdx + 1) + '/' + currentSegments.length;
    if (tsEl) tsEl.textContent = seg.timestamp || '';
    if (textEl) textEl.textContent = seg.text || '';
    if (prevBtn) prevBtn.disabled = (lightboxIdx === 0);
    if (nextBtn) nextBtn.disabled = (lightboxIdx >= currentSegments.length - 1);
  }

  // 라이트박스 키보드 탐색
  document.addEventListener('keydown', (e) => {
    if (lightboxIdx < 0) return;
    switch (e.key) {
      case 'Escape': _closeLightbox(); break;
      case 'ArrowLeft': case 'j': _navigateLightbox(-1); break;
      case 'ArrowRight': case 'k': _navigateLightbox(1); break;
    }
  });

  /**
   * 큐 항목에서 캡쳐 모드 레이블을 가져온다.
   * @returns {string|null}
   */
  function _getCaptureMode() {
    if (!currentVideoId) return null;
    const queue = typeof QueueUI !== 'undefined' ? QueueUI.getQueue() : [];
    const item = queue.find(q => q.videoId === currentVideoId ||
      (q.url && _extractVideoId(q.url) === currentVideoId));
    if (item && item.captureMode) {
      const modeKey = `capture_mode_${item.captureMode}`;
      return t(modeKey) || item.captureMode;
    }
    return null;
  }

  /**
   * 캡쳐 목록을 초기화/숨긴다.
   */
  function clear() {
    currentVideoId = null;
    currentSegments = [];
    currentMetadata = null;
    if (containerEl) {
      containerEl.innerHTML = '';
    }
  }

  /**
   * 캡쳐 목록이 현재 표시 중인지 확인한다.
   * @returns {boolean}
   */
  function isVisible() {
    return currentVideoId !== null && currentSegments.length > 0;
  }

  /**
   * 현재 표시 중인 영상 ID를 반환한다.
   * @returns {string|null}
   */
  function getVideoId() {
    return currentVideoId;
  }

  // ── 유틸리티 헬퍼 함수 ──

  function _escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  function _escapeAttr(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function _truncate(str, maxLen) {
    if (!str || str.length <= maxLen) return str || '';
    return str.substring(0, maxLen - 1) + '…';
  }

  // ── 공개 API ──

  return {
    init,
    loadFrames,
    clear,
    isVisible,
    getVideoId,
  };
})();
