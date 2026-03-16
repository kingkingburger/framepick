/**
 * FramePick - Capture List Component
 *
 * Displays captured frames/slides with thumbnails and metadata.
 * Supports grid and list view modes, plus a lightbox for full-size preview.
 * Integrates with AppState for reactive updates and the Tauri backend
 * via the get_capture_frames command.
 */

const CaptureListComponent = (() => {
  let _lightboxCurrentIdx = 0;
  let _initialized = false;

  // DOM references (set on init)
  let captureSection, captureListEl, captureMeta, captureMetaTitle, captureMetaCount;
  let btnCaptureBack, btnViewGrid, btnViewList, btnOpenSlides, btnOpenFolder;
  let frameLightbox, lightboxBackdrop, lightboxCloseBtn, lightboxPrev, lightboxNext;
  let lightboxImage, lightboxIndex, lightboxTimestamp, lightboxText;

  // Invoke helper (will be injected on init)
  let _invoke = (cmd, args) => Promise.resolve(null);
  let _escapeHtml = (s) => s;
  let _escapeAttr = (s) => s;
  let _handleOpenSlides = () => {};

  function init(opts) {
    if (_initialized) return;
    _initialized = true;

    _invoke = opts.invoke || _invoke;
    _escapeHtml = opts.escapeHtml || _escapeHtml;
    _escapeAttr = opts.escapeAttr || _escapeAttr;
    _handleOpenSlides = opts.handleOpenSlides || _handleOpenSlides;

    captureSection     = document.getElementById('capture-list-section');
    captureListEl      = document.getElementById('capture-list-container');
    captureMeta        = document.getElementById('capture-meta');
    captureMetaTitle   = document.getElementById('capture-meta-title');
    captureMetaCount   = document.getElementById('capture-meta-count');
    btnCaptureBack     = document.getElementById('btn-capture-back');
    btnViewGrid        = document.getElementById('btn-view-grid');
    btnViewList        = document.getElementById('btn-view-list');
    btnOpenSlides      = document.getElementById('btn-open-slides');
    btnOpenFolder      = document.getElementById('btn-open-folder');
    frameLightbox      = document.getElementById('frame-lightbox');
    lightboxBackdrop   = document.getElementById('lightbox-backdrop');
    lightboxCloseBtn   = document.getElementById('lightbox-close');
    lightboxPrev       = document.getElementById('lightbox-prev');
    lightboxNext       = document.getElementById('lightbox-next');
    lightboxImage      = document.getElementById('lightbox-image');
    lightboxIndex      = document.getElementById('lightbox-index');
    lightboxTimestamp   = document.getElementById('lightbox-timestamp');
    lightboxText       = document.getElementById('lightbox-text');

    // Bind state listeners
    AppState.on('captureFrames', renderCaptureList);
    AppState.on('captureViewMode', function () {
      var data = AppState.get('captureFrames');
      if (data) renderCaptureList(data);
    });

    // Lightbox event listeners
    if (lightboxCloseBtn) lightboxCloseBtn.addEventListener('click', closeLightbox);
    if (lightboxBackdrop) lightboxBackdrop.addEventListener('click', closeLightbox);
    if (lightboxPrev) lightboxPrev.addEventListener('click', function () { lightboxNavigate(-1); });
    if (lightboxNext) lightboxNext.addEventListener('click', function () { lightboxNavigate(1); });

    // Keyboard navigation for lightbox
    document.addEventListener('keydown', function (e) {
      if (!frameLightbox || frameLightbox.hidden) return;
      switch (e.key) {
        case 'Escape':  closeLightbox(); break;
        case 'ArrowLeft': case 'j': lightboxNavigate(-1); break;
        case 'ArrowRight': case 'k': lightboxNavigate(1); break;
      }
    });

    // Back button
    if (btnCaptureBack) {
      btnCaptureBack.addEventListener('click', function () {
        AppState.clearCaptureFrames();
        if (captureSection) captureSection.hidden = true;
      });
    }

    // View toggle buttons
    if (btnViewGrid) {
      btnViewGrid.addEventListener('click', function () {
        AppState.setCaptureViewMode('grid');
        btnViewGrid.classList.add('active');
        if (btnViewList) btnViewList.classList.remove('active');
      });
    }
    if (btnViewList) {
      btnViewList.addEventListener('click', function () {
        AppState.setCaptureViewMode('list');
        btnViewList.classList.add('active');
        if (btnViewGrid) btnViewGrid.classList.remove('active');
      });
    }

    // Open slides button
    if (btnOpenSlides) {
      btnOpenSlides.addEventListener('click', function () {
        var data = AppState.get('captureFrames');
        if (data && data.videoId) _handleOpenSlides(data.videoId);
      });
    }

    // Open folder button
    if (btnOpenFolder) {
      btnOpenFolder.addEventListener('click', function () {
        var data = AppState.get('captureFrames');
        if (data && data.videoId) {
          _invoke('open_folder', { videoId: data.videoId });
        }
      });
    }
  }

  /**
   * Load and display capture frames for a video from the library.
   */
  async function openCaptureList(videoId) {
    if (!captureSection || !captureListEl) return;

    captureSection.hidden = false;
    captureListEl.innerHTML = '<p class="capture-empty">' + I18n.t('captureLoading') + '</p>';
    if (captureMeta) captureMeta.hidden = true;
    if (btnOpenSlides) btnOpenSlides.hidden = true;
    if (btnOpenFolder) btnOpenFolder.hidden = true;

    try {
      var result = await _invoke('get_capture_frames', { videoId: videoId });
      if (result) {
        AppState.setCaptureFrames({
          videoId: result.video_id,
          title: result.title,
          frameCount: result.frame_count,
          frames: result.frames || [],
        });
      } else {
        AppState.setCaptureFrames(null);
      }
    } catch (e) {
      console.warn('[capture-list] Failed to load frames for', videoId, e);
      captureListEl.innerHTML = '<p class="capture-empty">' + I18n.t('captureEmpty') + '</p>';
    }
  }

  /**
   * Render the capture list with frames in grid or list view.
   */
  function renderCaptureList(data) {
    if (!captureSection || !captureListEl) return;

    if (!data || !data.frames || data.frames.length === 0) {
      captureSection.hidden = !data;
      captureListEl.innerHTML = '<p class="capture-empty">' + I18n.t('captureEmpty') + '</p>';
      if (captureMeta) captureMeta.hidden = true;
      if (btnOpenSlides) btnOpenSlides.hidden = true;
      if (btnOpenFolder) btnOpenFolder.hidden = true;
      return;
    }

    captureSection.hidden = false;

    // Update metadata bar
    if (captureMeta) {
      captureMeta.hidden = false;
      if (captureMetaTitle) captureMetaTitle.textContent = data.title || data.videoId;
      if (captureMetaCount) {
        captureMetaCount.textContent =
          (data.frameCount || data.frames.length) + I18n.t('captureFrameCount');
      }
    }

    if (btnOpenSlides && data.videoId) btnOpenSlides.hidden = false;
    if (btnOpenFolder && data.videoId) btnOpenFolder.hidden = false;

    var viewMode = AppState.get('captureViewMode');
    var frames = data.frames;

    if (viewMode === 'list') {
      renderListView(frames);
    } else {
      renderGridView(frames);
    }

    // Click handlers for lightbox
    captureListEl.querySelectorAll('[data-frame-idx]').forEach(function (el) {
      el.addEventListener('click', function () {
        openLightbox(Number(el.dataset.frameIdx));
      });
    });
  }

  function renderGridView(frames) {
    captureListEl.className = 'capture-list capture-grid';
    captureListEl.innerHTML = frames.map(function (frame, idx) {
      var text = frame.text || I18n.t('captureNoText');
      var textClass = frame.text ? '' : ' empty';
      var thumbSrc = frame.thumbnail_url || '';
      var thumbHtml = thumbSrc
        ? '<img src="' + _escapeAttr(thumbSrc) + '" alt="Frame ' + (frame.index + 1) + '" loading="lazy">'
        : '<div class="frame-placeholder">\uD83D\uDDBC</div>';

      return '<div class="capture-frame-card" data-frame-idx="' + idx + '">' +
        '<div class="capture-frame-thumb">' +
          thumbHtml +
          '<span class="capture-frame-index-badge">#' + (frame.index + 1) + '</span>' +
          '<span class="capture-frame-timestamp-badge">' + _escapeHtml(frame.timestamp || '') + '</span>' +
        '</div>' +
        '<div class="capture-frame-info">' +
          '<div class="capture-frame-text' + textClass + '">' + _escapeHtml(text) + '</div>' +
        '</div></div>';
    }).join('');
  }

  function renderListView(frames) {
    captureListEl.className = 'capture-list capture-list-view';
    captureListEl.innerHTML = frames.map(function (frame, idx) {
      var text = frame.text || I18n.t('captureNoText');
      var textClass = frame.text ? '' : ' empty';
      var thumbSrc = frame.thumbnail_url || '';
      var thumbHtml = thumbSrc
        ? '<img src="' + _escapeAttr(thumbSrc) + '" alt="Frame ' + (frame.index + 1) + '" loading="lazy">'
        : '';

      return '<div class="capture-frame-row" data-frame-idx="' + idx + '">' +
        '<span class="capture-frame-row-index">#' + (frame.index + 1) + '</span>' +
        '<div class="capture-frame-row-thumb">' + thumbHtml + '</div>' +
        '<div class="capture-frame-row-info">' +
          '<div class="capture-frame-row-text' + textClass + '">' + _escapeHtml(text) + '</div>' +
          '<div class="capture-frame-row-timestamp">' + _escapeHtml(frame.timestamp || '') + '</div>' +
        '</div></div>';
    }).join('');
  }

  // ── Lightbox ──────────────────────────────────────────────────
  function openLightbox(idx) {
    var data = AppState.get('captureFrames');
    if (!data || !data.frames || idx < 0 || idx >= data.frames.length) return;
    if (!frameLightbox) return;

    _lightboxCurrentIdx = idx;
    updateLightbox(idx);
    frameLightbox.hidden = false;
    document.body.style.overflow = 'hidden';
  }

  function closeLightbox() {
    if (!frameLightbox) return;
    frameLightbox.hidden = true;
    document.body.style.overflow = '';
  }

  function updateLightbox(idx) {
    var data = AppState.get('captureFrames');
    if (!data || !data.frames) return;
    var frame = data.frames[idx];
    if (!frame) return;

    _lightboxCurrentIdx = idx;

    if (lightboxImage) {
      lightboxImage.src = frame.thumbnail_url || '';
      lightboxImage.alt = 'Frame ' + (frame.index + 1);
    }
    if (lightboxIndex) {
      lightboxIndex.textContent = '#' + (frame.index + 1) + I18n.t('captureFrameOf') + data.frames.length;
    }
    if (lightboxTimestamp) lightboxTimestamp.textContent = frame.timestamp || '';
    if (lightboxText) lightboxText.textContent = frame.text || '';
    if (lightboxPrev) lightboxPrev.disabled = (idx === 0);
    if (lightboxNext) lightboxNext.disabled = (idx >= data.frames.length - 1);
  }

  function lightboxNavigate(delta) {
    var data = AppState.get('captureFrames');
    if (!data || !data.frames) return;
    var newIdx = _lightboxCurrentIdx + delta;
    if (newIdx >= 0 && newIdx < data.frames.length) {
      updateLightbox(newIdx);
    }
  }

  // ── Public API ────────────────────────────────────────────────
  return {
    init: init,
    openCaptureList: openCaptureList,
  };
})();
