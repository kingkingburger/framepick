/**
 * capture-list.js - Captured frames list component for framepick
 *
 * Displays a browsable list of captured frames with thumbnails and metadata
 * after a pipeline completes. Users can preview frames before opening the
 * full slides.html viewer.
 *
 * Features:
 *   - Grid view showing frame thumbnails with timestamp badges
 *   - Subtitle text preview on hover / below each frame
 *   - Capture mode + frame count metadata header
 *   - Click on a frame to open the full slides viewer at that index
 *   - Collapsible panel that auto-shows after capture completion
 *   - Responds to language changes
 *
 * Data source:
 *   - Reads segments.json + images/ from the library entry folder
 *   - Uses Tauri `get_slides_metadata` command for metadata
 *   - Uses asset protocol URLs for image thumbnails
 *
 * Events listened:
 *   - queueItemCompleted: auto-show capture list for the completed item
 *   - languageChanged: re-render labels
 *
 * Events emitted:
 *   - captureListFrameClicked: { videoId, frameIndex } when user clicks a frame
 */

const CaptureList = (() => {
  let containerEl = null;
  let currentVideoId = null;
  let currentSegments = [];
  let currentMetadata = null;
  let isExpanded = true;
  let viewMode = 'grid'; // 'grid' | 'list'
  let lightboxIdx = -1;  // -1 = closed

  /**
   * Initialize the capture list component.
   * Creates the container element and sets up event listeners.
   */
  function init() {
    // Find or create the container element
    containerEl = document.getElementById('capture-list-container');
    if (!containerEl) {
      // Insert capture list container after queue container
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

    // Listen for queue item completion to auto-show capture list
    document.addEventListener('queueItemCompleted', _onItemCompleted);

    // Re-render on language changes
    document.addEventListener('languageChanged', () => {
      if (currentVideoId && currentSegments.length > 0) {
        _render();
      }
    });
  }

  /**
   * Handle queue item completion — load and show the captured frames.
   * @param {CustomEvent} event
   */
  async function _onItemCompleted(event) {
    const { id } = event.detail;

    // Get the queue item to find its video ID
    const queue = typeof QueueUI !== 'undefined' ? QueueUI.getQueue() : [];
    const item = queue.find(q => q.id === id);

    if (!item) return;

    // Extract video ID from URL
    const videoId = item.videoId || _extractVideoId(item.url);
    if (!videoId) return;

    // Load the captured frames for this video
    await loadFrames(videoId);
  }

  /**
   * Load and display captured frames for a given video ID.
   * @param {string} videoId - YouTube video ID (library folder name)
   */
  async function loadFrames(videoId) {
    if (!containerEl) return;
    if (!videoId) return;

    currentVideoId = videoId;
    currentSegments = [];
    currentMetadata = null;

    // Show loading state
    _renderLoading();

    try {
      if (window.__TAURI__ && window.__TAURI__.core) {
        // Use the dedicated get_capture_frames command which reads segments.json
        // and builds asset protocol thumbnail URLs
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
          // Fallback: try get_slides_metadata for basic image list
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
   * Load segments data for a video entry via the backend.
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
   * Extract a video ID from a YouTube URL.
   * @param {string} url
   * @returns {string|null}
   */
  function _extractVideoId(url) {
    if (!url) return null;
    const match = url.match(/(?:v=|\/shorts\/|youtu\.be\/)([\w-]{11})/);
    return match ? match[1] : null;
  }

  /**
   * Extract a human-readable timestamp from an image filename.
   * e.g. "frame_0001_00-01-23.jpg" → "00:01:23"
   * @param {string} filename
   * @returns {string}
   */
  function _formatTimestampFromFilename(filename) {
    // Match pattern like "00-01-23" in filename
    const match = filename.match(/(\d{2})-(\d{2})-(\d{2})/);
    if (match) {
      return `${match[1]}:${match[2]}:${match[3]}`;
    }
    return '';
  }

  /**
   * Build an asset protocol URL for an image in the library.
   * @param {string} videoId
   * @param {string} imageName
   * @returns {string}
   */
  function _buildImageUrl(videoId, imageName) {
    // The get_slides_metadata returns image names relative to images/ dir
    // We need to build the full asset URL via the backend
    // For now, use the library path convention
    if (currentMetadata && currentMetadata.video_id) {
      // In Tauri context, images are served via asset protocol
      // The slides_viewer.rs already does this — we can use the same approach
      // For the capture list thumbnails, we'll reference them as relative paths
      // and let the backend resolve them
      return `asset://localhost/images/${imageName}`;
    }
    return `images/${imageName}`;
  }

  /**
   * Render loading state.
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
   * Render error state.
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
    // Auto-hide after 5 seconds
    setTimeout(() => {
      if (containerEl) containerEl.innerHTML = '';
    }, 5000);
  }

  /**
   * Main render function — builds the full capture list UI.
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

    // Header with toggle
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
    // View mode toggle (grid/list)
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

    // Frame grid or list (collapsible)
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
        // Grid view (default)
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

      // Summary footer
      html += '<div class="capture-list-footer">';
      html += `<span class="capture-list-video-title" title="${_escapeAttr(title)}">${_escapeHtml(_truncate(title, 50))}</span>`;
      html += '</div>';
    }

    html += '</div>';

    containerEl.innerHTML = html;

    // Load actual thumbnails asynchronously
    _loadThumbnails();

    // Bind events
    _bindEvents();
  }

  /**
   * Load actual thumbnail images from the asset protocol.
   * Updates placeholder divs with actual <img> elements.
   */
  async function _loadThumbnails() {
    if (!containerEl || !currentVideoId) return;

    const placeholders = containerEl.querySelectorAll('.capture-list-thumb-placeholder[data-image]');
    if (placeholders.length === 0) return;

    // Get the library path base for building asset URLs
    let libraryBasePath = null;
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        const slidesPath = await window.__TAURI__.core.invoke('get_slides_path', {
          videoId: currentVideoId
        });
        if (slidesPath) {
          // Extract the directory containing slides.html
          libraryBasePath = slidesPath.replace(/[/\\]slides\.html$/, '');
        }
      } catch (e) {
        // slides.html may not exist yet — try constructing from settings
        try {
          const settings = await window.__TAURI__.core.invoke('get_settings');
          if (settings && settings.library_path) {
            libraryBasePath = settings.library_path + '/' + currentVideoId;
          }
        } catch (e2) {
          console.warn('CaptureList: Cannot resolve library path for thumbnails');
        }
      }
    }

    if (!libraryBasePath) return;

    // Normalize path separators
    const basePath = libraryBasePath.replace(/\\/g, '/');

    placeholders.forEach(placeholder => {
      const imageName = placeholder.dataset.image;
      if (!imageName) return;

      const imagePath = `${basePath}/images/${imageName}`;
      // Build Tauri asset protocol URL
      const encodedPath = _percentEncodePath(imagePath);
      const assetUrl = `https://asset.localhost/${encodedPath}`;

      const img = document.createElement('img');
      img.className = 'capture-list-thumb';
      img.alt = `Frame ${imageName}`;
      img.loading = 'lazy';
      img.src = assetUrl;
      img.onerror = function() {
        // Keep the placeholder with frame number on error
        this.style.display = 'none';
      };

      // Keep the frame number badge
      const badge = placeholder.querySelector('.capture-list-frame-num');
      placeholder.innerHTML = '';
      placeholder.appendChild(img);
      if (badge) {
        placeholder.appendChild(badge);
      }
    });
  }

  /**
   * Percent-encode a file path for use in an asset protocol URL.
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
        // Encode each UTF-8 byte
        const bytes = new TextEncoder().encode(ch);
        for (const b of bytes) {
          encoded += '%' + b.toString(16).toUpperCase().padStart(2, '0');
        }
      }
    }
    return encoded;
  }

  /**
   * Bind click events to the rendered capture list.
   */
  function _bindEvents() {
    if (!containerEl) return;

    // Toggle expand/collapse
    containerEl.querySelectorAll('[data-action="toggle"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        isExpanded = !isExpanded;
        _render();
      });
    });

    // View mode toggle (grid/list)
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

    // View slides button — open in slides viewer
    containerEl.querySelectorAll('[data-action="view-slides"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (currentVideoId && typeof SlidesViewer !== 'undefined') {
          SlidesViewer.open(currentVideoId);
        }
      });
    });

    // Click on individual frames — open lightbox preview
    const frameEls = containerEl.querySelectorAll('.capture-list-frame, .capture-list-row');
    frameEls.forEach(frame => {
      frame.addEventListener('click', () => {
        const frameIndex = parseInt(frame.dataset.frameIndex, 10);
        if (isNaN(frameIndex)) return;

        // Dispatch event for other components
        document.dispatchEvent(new CustomEvent('captureListFrameClicked', {
          detail: { videoId: currentVideoId, frameIndex: frameIndex }
        }));

        // Open lightbox preview
        _openLightbox(frameIndex);
      });
    });
  }

  // ── Lightbox Preview ──────────────────────────────────────────

  /**
   * Open the lightbox at the given frame index.
   */
  function _openLightbox(idx) {
    if (idx < 0 || idx >= currentSegments.length) return;
    lightboxIdx = idx;

    // Create or find lightbox overlay
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

      // Bind lightbox events
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

  // Keyboard navigation for lightbox
  document.addEventListener('keydown', (e) => {
    if (lightboxIdx < 0) return;
    switch (e.key) {
      case 'Escape': _closeLightbox(); break;
      case 'ArrowLeft': case 'j': _navigateLightbox(-1); break;
      case 'ArrowRight': case 'k': _navigateLightbox(1); break;
    }
  });

  /**
   * Get the capture mode label from the queue item.
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
   * Clear/hide the capture list.
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
   * Check if the capture list is currently showing.
   * @returns {boolean}
   */
  function isVisible() {
    return currentVideoId !== null && currentSegments.length > 0;
  }

  /**
   * Get the current video ID being displayed.
   * @returns {string|null}
   */
  function getVideoId() {
    return currentVideoId;
  }

  // ── Utility helpers ──

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

  // ── Public API ──

  return {
    init,
    loadFrames,
    clear,
    isVisible,
    getVideoId,
  };
})();
