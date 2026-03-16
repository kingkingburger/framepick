/**
 * framepick - Main application entry point
 */

document.addEventListener('DOMContentLoaded', () => {
  // ─── Tauri interop helper ──────────────────────────────────
  const invoke = (cmd, args) => {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    console.log('[dev] invoke:', cmd, args);
    return Promise.resolve(null);
  };

  // ─── Initialize components ─────────────────────────────────
  UrlInput.init();
  initCaptureMode();
  if (typeof SettingsUI !== 'undefined') {
    SettingsUI.init();
  } else if (typeof initSettings === 'function') {
    initSettings();
  }
  SlidesViewer.init();
  PipelineProgress.init();
  QueueUI.init();
  CaptureList.init();
  PlaylistUI.init();

  // ─── Workflow empty hint visibility ────────────────────────
  // Show hint when queue is empty and no captures are displayed
  const workflowHint = document.getElementById('workflow-empty-hint');
  function updateWorkflowHint() {
    if (!workflowHint) return;
    const queueCounts = QueueUI.getCounts();
    const hasQueue = queueCounts.total > 0;
    const hasCaptureList = typeof CaptureList !== 'undefined' && CaptureList.isVisible();
    workflowHint.classList.toggle('hidden', hasQueue || hasCaptureList);
  }
  // Update hint on queue and capture list changes
  document.addEventListener('queueItemAdded', updateWorkflowHint);
  document.addEventListener('queueItemCompleted', updateWorkflowHint);
  document.addEventListener('queueItemFailed', updateWorkflowHint);
  document.addEventListener('queueCleared', updateWorkflowHint);
  // Initial state
  updateWorkflowHint();

  // ─── Queue completion → Library refresh bridge ─────────────────
  // QueueUI handles Tauri events internally; we just refresh library on completion
  document.addEventListener('queueItemCompleted', () => {
    loadLibrary();
  });

  // ─── Language switcher in header ───────────────────────────
  const langSelect = document.getElementById('lang-select');
  langSelect.addEventListener('change', () => {
    const lang = langSelect.value;
    setLanguage(lang);
    AppState.setLanguage(lang);
    invoke('update_settings', { patch: { language: lang } })
      .catch(err => console.warn('Failed to persist language:', err));
  });

  // Sync AppState language → header dropdown
  AppState.on('language', (lang) => {
    if (langSelect.value !== lang) {
      langSelect.value = lang;
    }
    setLanguage(lang);
  });

  // Listen for validated URL submissions → add to queue (or open playlist modal)
  document.addEventListener('urlSubmitted', (e) => {
    const { url, videoId } = e.detail;

    // Check if this is a playlist URL
    const playlistCheck = PlaylistUI.detectPlaylist(url);
    if (playlistCheck.isPlaylist) {
      // Open playlist selection modal instead of adding directly
      console.log('Playlist detected:', playlistCheck.listId);
      PlaylistUI.open(url, playlistCheck.listId);
      return;
    }

    // Single video: check duplicate in local queue (by URL or video ID)
    const currentQueue = QueueUI.getQueue();
    const isDuplicateInQueue = currentQueue.some((q) => {
      if (q.status !== 'pending' && q.status !== 'processing') return false;
      // Exact URL match
      if (q.url === url) return true;
      // Video ID match (handles different URL formats for same video)
      if (videoId && q.videoId && q.videoId === videoId) return true;
      return false;
    });
    if (isDuplicateInQueue) {
      showToast(t('error_duplicate') || 'This URL is already in the queue', 'warning');
      return;
    }

    // Check if video already exists in library (already processed) then add
    const checkAndAdd = async () => {
      if (videoId && window.__TAURI__ && window.__TAURI__.core) {
        try {
          const exists = await window.__TAURI__.core.invoke('check_video_exists', { videoId });
          if (exists) {
            showToast(t('error_already_exists'), 'warning');
            console.log('Video already exists in library:', videoId);
            return;
          }
        } catch (err) {
          // If check fails, proceed anyway — don't block the user
          console.warn('Failed to check video existence:', err);
        }
      }

      try {
        const id = await QueueUI.addItem(url, videoId || '');
        if (id > 0) {
          showToast(t('queue_added'), 'success');
          console.log('Added to queue:', { id, url, videoId });
          // Auto-scroll to make queue section visible
          const queueSection = document.getElementById('queue-section');
          if (queueSection) {
            queueSection.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
          }
        }
      } catch (err) {
        console.warn('Failed to add to queue:', err);
      }
    };

    checkAndAdd();
  });

  // ─── Playlist selection dialog → Queue bridge ────────────────
  // When the user confirms video selections in the PlaylistUI dialog,
  // add each selected video as an individual entry in the download queue.
  // Videos are added sequentially to preserve ordering and avoid race conditions.
  document.addEventListener('playlistVideosSelected', (e) => {
    const { videos } = e.detail;
    if (!videos || videos.length === 0) return;

    let addedCount = 0;
    let skippedCount = 0;

    // Track URLs already in queue AND newly added within this batch
    // to prevent intra-batch duplicates (e.g., same video appearing twice in playlist)
    const activeUrls = new Set(
      QueueUI.getQueue()
        .filter(q => q.status === 'pending' || q.status === 'processing')
        .map(q => q.url)
    );

    const addNext = async (index) => {
      if (index >= videos.length) {
        // Show summary toast when all videos have been processed
        if (addedCount > 0 && skippedCount > 0) {
          showToast(
            t('queue_added_partial', { added: addedCount, skipped: skippedCount }),
            'success'
          );
        } else if (addedCount > 0) {
          showToast(t('queue_added_batch', { n: addedCount }), 'success');
        } else if (skippedCount > 0) {
          showToast(t('error_all_duplicates'), 'warning');
        }
        return;
      }

      const video = videos[index];

      // Skip duplicates (includes intra-batch duplicate detection via activeUrls Set)
      if (activeUrls.has(video.url)) {
        skippedCount++;
        console.log('Skipping duplicate in queue:', video.url);
        addNext(index + 1);
        return;
      }

      // Skip videos already in library (if backend is available)
      if (video.videoId && window.__TAURI__ && window.__TAURI__.core) {
        try {
          const exists = await window.__TAURI__.core.invoke('check_video_exists', { videoId: video.videoId });
          if (exists) {
            skippedCount++;
            console.log('Skipping video already in library:', video.videoId);
            addNext(index + 1);
            return;
          }
        } catch (_) { /* proceed if check fails */ }
      }

      QueueUI.addItem(video.url, video.videoId || '').then((id) => {
        if (id > 0) {
          addedCount++;
          activeUrls.add(video.url); // Track to prevent intra-batch duplicates
          console.log('Added playlist video to queue:', {
            id,
            url: video.url,
            title: video.title,
            index: index + 1,
            total: videos.length,
          });
        }
        addNext(index + 1);
      }).catch((err) => {
        console.warn('Failed to add playlist video to queue:', video.url, err);
        addNext(index + 1);
      });
    };

    addNext(0);
  });

  // Listen for capture mode changes → sync to backend
  document.addEventListener('captureModeChanged', (e) => {
    console.log('Capture mode config:', e.detail);
    invoke('set_input_state', { state: AppState.buildPipelineInput() }).catch(() => {});
  });

  // Debounced URL sync to backend
  let _urlSyncTimer = null;
  AppState.on('url', () => {
    clearTimeout(_urlSyncTimer);
    _urlSyncTimer = setTimeout(() => {
      invoke('set_input_state', { state: AppState.buildPipelineInput() }).catch(() => {});
    }, 300);
  });

  // Library refresh button
  const refreshBtn = document.getElementById('btn-refresh-library');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', () => loadLibrary());
  }

  // ─── Listen for queue:duplicate-skipped events from backend ─────
  // When the backend detects that a video already exists in the library
  // during processing, it emits a queue:duplicate-skipped event.
  if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
    window.__TAURI__.event.listen('queue:duplicate-skipped', (event) => {
      const payload = event.payload;
      console.log('[queue_processor] Duplicate skipped:', payload);
      showToast(t('error_already_exists'), 'warning');
    });
  }

  // ─── Listen for capture:fallback events from backend ─────
  // When the backend detects that subtitle mode can't be used (no subtitles
  // available or subtitle check failed), it emits a capture:fallback event.
  // We show a toast notification and log the fallback.
  if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
    window.__TAURI__.event.listen('capture:fallback', (event) => {
      const payload = event.payload;
      console.log('[capture_fallback] Mode fallback occurred:', payload);

      // Use the i18n key from the backend payload, or fall back to generic message
      const reasonKey = payload.reason_key || 'fallback_no_subtitles';
      const message = t(reasonKey) || payload.reason || 'Capture mode changed';

      showToast(message, 'warning');

      // Update the queue item's capture mode in local state if applicable
      if (payload.queue_id && payload.queue_id > 0) {
        AppState.updateQueueItem(payload.queue_id, {
          captureMode: payload.effective_mode,
        });
      }
    });
  }

  // Set initial language (will be overridden by settings if backend available)
  setLanguage('ko');

  // Load library on startup
  loadLibrary();
});

/**
 * Load and display library entries from the backend.
 */
async function loadLibrary() {
  const grid = document.getElementById('library-grid');
  const countEl = document.getElementById('library-count');
  if (!grid) return;

  try {
    let entries = [];
    if (window.__TAURI__ && window.__TAURI__.core) {
      entries = await window.__TAURI__.core.invoke('list_library_entries');
    }

    // Update library count badge
    if (countEl) {
      countEl.textContent = entries.length > 0
        ? t('library_item_count', { n: entries.length })
        : '';
    }

    if (entries.length === 0) {
      grid.innerHTML = `<p class="library-empty" data-i18n="library_empty">${t('library_empty')}</p>`;
      return;
    }

    grid.innerHTML = entries.map(entry => {
      const title = entry.title || entry.video_id;
      const slideCount = entry.slide_count != null ? t('library_slides', { n: entry.slide_count }) : '';
      // Use first captured frame as thumbnail; fall back to placeholder icon
      const thumbHtml = entry.thumbnail
        ? `<img class="library-card-thumb" src="${escapeHtml(entry.thumbnail)}" alt="${escapeHtml(title)}" loading="lazy" onerror="this.parentElement.innerHTML='<div class=\\'library-card-thumb-placeholder\\'>&#9654;</div>'">`
        : `<div class="library-card-thumb-placeholder">&#9654;</div>`;

      return `
        <div class="library-card${entry.has_slides ? '' : ' library-card-disabled'}" data-video-id="${escapeHtml(entry.video_id)}">
          <div class="library-card-thumb-wrap">
            ${thumbHtml}
            ${slideCount ? `<span class="library-card-badge">${slideCount}</span>` : ''}
            ${entry.has_slides ? `<div class="library-card-overlay">
              <button class="library-card-overlay-btn library-card-view" data-view-id="${escapeHtml(entry.video_id)}" title="${t('library_open_viewer')}">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                <span>${t('library_open_viewer')}</span>
              </button>
              <button class="library-card-overlay-btn library-card-browser" data-browser-id="${escapeHtml(entry.video_id)}" title="${t('library_open_browser')}">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                <span>${t('library_open_browser')}</span>
              </button>
            </div>` : ''}
          </div>
          <div class="library-card-info">
            <div class="library-card-title" title="${escapeHtml(title)}">${escapeHtml(title)}</div>
            <div class="library-card-meta">
              <span>${entry.video_id}</span>
              <div class="library-card-actions">
                <button class="library-card-action library-card-open-folder" data-folder-id="${escapeHtml(entry.video_id)}" title="${t('library_open_folder')}">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                </button>
                <button class="library-card-action library-card-recapture" data-recapture-id="${escapeHtml(entry.video_id)}" data-recapture-title="${escapeHtml(title)}" title="${t('recapture_btn')}">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
                </button>
                <button class="library-card-action library-card-delete" data-delete-id="${escapeHtml(entry.video_id)}" title="${t('library_delete')}">&times;</button>
              </div>
            </div>
          </div>
        </div>
      `;
    }).join('');

    // Bind click events to open viewer
    grid.querySelectorAll('.library-card[data-video-id]').forEach(card => {
      card.addEventListener('click', (e) => {
        // Don't open viewer if action button or overlay button was clicked
        if (e.target.closest('.library-card-action') || e.target.closest('.library-card-overlay-btn')) return;
        const videoId = card.dataset.videoId;
        if (videoId) {
          SlidesViewer.open(videoId);
        }
      });
    });

    // Bind "Open in Viewer" overlay button events
    grid.querySelectorAll('.library-card-view').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.viewId;
        if (videoId) {
          SlidesViewer.open(videoId);
        }
      });
    });

    // Bind "Open in Browser" overlay button events
    grid.querySelectorAll('.library-card-browser').forEach(btn => {
      btn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.browserId;
        if (!videoId) return;

        try {
          if (window.__TAURI__ && window.__TAURI__.core) {
            await window.__TAURI__.core.invoke('open_slides_external', { videoId });
          }
        } catch (err) {
          console.error('Failed to open slides in browser:', err);
          showToast(t('library_open_browser_failed'), 'error');
        }
      });
    });

    // Bind open-folder button events
    grid.querySelectorAll('.library-card-open-folder').forEach(btn => {
      btn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.folderId;
        if (!videoId) return;

        try {
          if (window.__TAURI__ && window.__TAURI__.core) {
            await window.__TAURI__.core.invoke('open_folder', { videoId });
          }
        } catch (err) {
          console.error('Failed to open folder:', err);
          showToast(t('library_open_folder_error') + ': ' + err, 'error');
        }
      });
    });

    // Bind re-capture button events
    grid.querySelectorAll('.library-card-recapture').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.recaptureId;
        const title = btn.dataset.recaptureTitle || videoId;
        if (videoId) {
          openRecaptureModal(videoId, title);
        }
      });
    });

    // Bind delete button events
    grid.querySelectorAll('.library-card-delete').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.deleteId;
        if (!videoId) return;

        // Find the card's title for the confirmation dialog
        const card = btn.closest('.library-card');
        const titleEl = card ? card.querySelector('.library-card-title') : null;
        const title = titleEl ? titleEl.textContent : videoId;

        openDeleteConfirmModal(videoId, title);
      });
    });
  } catch (err) {
    console.warn('Failed to load library:', err);
    grid.innerHTML = `<p class="library-empty">${t('library_empty')}</p>`;
  }
}

/**
 * Show a toast notification with stacking support.
 * Multiple toasts stack upward so rapid failures don't overwrite each other.
 * Error toasts include an icon and a close button for better UX.
 * @param {string} message
 * @param {'success'|'error'|'warning'} type
 */
function showToast(message, type = 'success') {
  // Toast icons for visual distinction
  const TOAST_ICONS = {
    success: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    error: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
    warning: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
  };

  // Cap max visible toasts to 5 to prevent DOM overflow
  const existingToasts = document.querySelectorAll('.toast');
  if (existingToasts.length >= 5) {
    existingToasts[0].remove();
  }

  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  const iconHtml = TOAST_ICONS[type] || '';
  const safeMsg = escapeHtml(message);
  toast.innerHTML = `<span class="toast-icon">${iconHtml}</span><span class="toast-message">${safeMsg}</span><button class="toast-close" aria-label="Close">&times;</button>`;
  document.body.appendChild(toast);

  // Close button allows manual dismissal
  toast.querySelector('.toast-close').addEventListener('click', () => {
    toast.classList.remove('toast-show');
    setTimeout(() => {
      toast.remove();
      _repositionToasts();
    }, 300);
  });

  // Reposition all visible toasts to stack upward
  _repositionToasts();

  // Trigger show animation
  requestAnimationFrame(() => {
    toast.classList.add('toast-show');
  });

  // Error toasts stay longer (6s) so the user can read the error details
  const duration = type === 'error' ? 6000 : type === 'warning' ? 4000 : 3000;
  setTimeout(() => {
    toast.classList.remove('toast-show');
    setTimeout(() => {
      toast.remove();
      _repositionToasts();
    }, 300);
  }, duration);
}

/**
 * Reposition stacked toasts so they don't overlap.
 * Called whenever a toast is added or removed.
 */
function _repositionToasts() {
  const toasts = document.querySelectorAll('.toast');
  let bottomOffset = 24;
  toasts.forEach((toast) => {
    toast.style.bottom = bottomOffset + 'px';
    bottomOffset += toast.offsetHeight + 8;
  });
}

/**
 * Simple HTML escape for safe attribute/content injection.
 * @param {string} str
 * @returns {string}
 */
function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

// ─── Re-capture Modal ──────────────────────────────────────────

/** Currently targeted video ID for re-capture */
let _recaptureVideoId = null;
/** Cached settings for re-capture (scene threshold, interval) */
let _recaptureSettings = null;

/** Mode description i18n key mapping */
const RECAPTURE_MODE_DESC_KEYS = {
  subtitle: 'capture_mode_subtitle_desc',
  scene: 'capture_mode_scene_desc',
  interval: 'capture_mode_interval_desc',
};

/**
 * Update the mode description text and show/hide mode-specific options.
 */
function _updateRecaptureModeUI() {
  const modeSelect = document.getElementById('recapture-mode');
  const descEl = document.getElementById('recapture-mode-desc');
  const intervalGroup = document.getElementById('recapture-interval-group');
  const sceneGroup = document.getElementById('recapture-scene-group');

  if (!modeSelect) return;
  const mode = modeSelect.value;

  // Update description text
  if (descEl) {
    const descKey = RECAPTURE_MODE_DESC_KEYS[mode] || '';
    descEl.textContent = descKey ? t(descKey) : '';
    descEl.setAttribute('data-i18n', descKey);
  }

  // Show/hide interval options
  if (intervalGroup) intervalGroup.hidden = mode !== 'interval';
  // Show/hide scene threshold
  if (sceneGroup) sceneGroup.hidden = mode !== 'scene';
}

/**
 * Get the effective interval seconds from recapture modal (preset or custom).
 * @returns {number}
 */
function _getRecaptureInterval() {
  const intervalSelect = document.getElementById('recapture-interval');
  if (!intervalSelect) return 10;

  if (intervalSelect.value === 'custom') {
    const customInput = document.getElementById('recapture-custom-interval');
    const val = customInput ? parseInt(customInput.value, 10) : NaN;
    if (isNaN(val) || val < 1 || val > 3600) return 10;
    return val;
  }
  return parseInt(intervalSelect.value, 10) || 10;
}

/**
 * Open the re-capture modal for a library item.
 * @param {string} videoId
 * @param {string} title
 */
async function openRecaptureModal(videoId, title) {
  const modal = document.getElementById('recapture-modal');
  const titleEl = document.getElementById('recapture-video-title');
  const modeSelect = document.getElementById('recapture-mode');
  const errorEl = document.getElementById('recapture-error');
  const startBtn = document.getElementById('btn-recapture-start');
  const progressEl = document.getElementById('recapture-progress');
  const thresholdSlider = document.getElementById('recapture-scene-threshold');
  const thresholdValue = document.getElementById('recapture-threshold-value');

  if (!modal) return;

  _recaptureVideoId = videoId;

  // Reset state
  if (titleEl) titleEl.textContent = title;
  if (modeSelect) modeSelect.value = 'subtitle';
  if (errorEl) { errorEl.hidden = true; errorEl.textContent = ''; }
  if (progressEl) progressEl.hidden = true;
  if (startBtn) { startBtn.disabled = false; startBtn.textContent = t('recapture_start'); }

  // Load scene threshold from settings
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      _recaptureSettings = await window.__TAURI__.core.invoke('get_settings');
      if (_recaptureSettings && thresholdSlider) {
        const threshPercent = Math.round((_recaptureSettings.scene_change_threshold || 0.30) * 100);
        thresholdSlider.value = threshPercent;
        if (thresholdValue) thresholdValue.textContent = threshPercent + '%';
      }
    }
  } catch (err) {
    console.warn('Failed to load settings for recapture:', err);
  }

  // Update mode-specific UI
  _updateRecaptureModeUI();

  // Reset custom interval input
  const customIntervalGroup = document.getElementById('recapture-custom-interval-group');
  const customIntervalInput = document.getElementById('recapture-custom-interval');
  const intervalSelect = document.getElementById('recapture-interval');
  if (customIntervalGroup) customIntervalGroup.hidden = true;
  if (customIntervalInput) customIntervalInput.value = '';
  if (intervalSelect) intervalSelect.value = '10';

  // Check if source video is available
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      const available = await window.__TAURI__.core.invoke('check_recapture_available', { videoId });
      if (!available) {
        if (errorEl) {
          errorEl.textContent = t('recapture_no_video');
          errorEl.hidden = false;
        }
        if (startBtn) startBtn.disabled = true;
      }
    }
  } catch (err) {
    console.warn('Failed to check recapture availability:', err);
  }

  modal.hidden = false;
}

/**
 * Close the re-capture modal.
 */
function closeRecaptureModal() {
  const modal = document.getElementById('recapture-modal');
  if (modal) modal.hidden = true;
  _recaptureVideoId = null;
}

/**
 * Execute re-capture with selected mode.
 */
async function executeRecapture() {
  if (!_recaptureVideoId) return;

  const modeSelect = document.getElementById('recapture-mode');
  const startBtn = document.getElementById('btn-recapture-start');
  const cancelBtn = document.getElementById('btn-recapture-cancel');
  const errorEl = document.getElementById('recapture-error');
  const progressEl = document.getElementById('recapture-progress');
  const progressFill = document.getElementById('recapture-progress-fill');
  const progressText = document.getElementById('recapture-progress-text');
  const thresholdSlider = document.getElementById('recapture-scene-threshold');

  const captureMode = modeSelect ? modeSelect.value : 'scene';
  const intervalSeconds = _getRecaptureInterval();

  // Validate custom interval
  if (captureMode === 'interval') {
    const intervalSelect = document.getElementById('recapture-interval');
    if (intervalSelect && intervalSelect.value === 'custom') {
      const customInput = document.getElementById('recapture-custom-interval');
      const val = customInput ? parseInt(customInput.value, 10) : NaN;
      if (isNaN(val) || val < 1 || val > 3600) {
        if (errorEl) {
          errorEl.textContent = t('interval_custom_hint', { min: 1, max: 3600 });
          errorEl.hidden = false;
        }
        return;
      }
    }
  }

  // Disable button and show processing state
  if (startBtn) {
    startBtn.disabled = true;
    startBtn.textContent = t('recapture_processing');
  }
  if (cancelBtn) cancelBtn.disabled = true;
  if (errorEl) errorEl.hidden = true;

  // Show progress indicator
  if (progressEl) progressEl.hidden = false;
  if (progressFill) {
    progressFill.style.width = '30%';
  }
  if (progressText) progressText.textContent = t('recapture_processing');

  try {
    const args = {
      videoId: _recaptureVideoId,
      captureMode: captureMode,
    };
    if (captureMode === 'interval') {
      args.intervalSeconds = intervalSeconds;
    }
    if (captureMode === 'scene' && thresholdSlider) {
      args.sceneThreshold = parseInt(thresholdSlider.value, 10) / 100.0;
    }

    // Simulate progress stages
    if (progressFill) progressFill.style.width = '50%';

    let result = null;
    if (window.__TAURI__ && window.__TAURI__.core) {
      result = await window.__TAURI__.core.invoke('recapture_library_item', args);
    }

    if (progressFill) progressFill.style.width = '100%';
    if (progressText) progressText.textContent = t('progress_done');

    // Brief pause to show completion
    await new Promise(resolve => setTimeout(resolve, 400));

    closeRecaptureModal();

    if (result) {
      showToast(t('recapture_success', { n: result.frame_count }), 'success');
    } else {
      showToast(t('recapture_success', { n: '?' }), 'success');
    }

    // Refresh library to show updated thumbnails/counts
    loadLibrary();
  } catch (err) {
    console.error('Re-capture failed:', err);
    if (progressEl) progressEl.hidden = true;
    if (errorEl) {
      errorEl.textContent = typeof err === 'string' ? err : (err.message || t('recapture_error'));
      errorEl.hidden = false;
    }
    if (startBtn) {
      startBtn.disabled = false;
      startBtn.textContent = t('recapture_start');
    }
    if (cancelBtn) cancelBtn.disabled = false;
  }
}

// Initialize re-capture modal event handlers
document.addEventListener('DOMContentLoaded', () => {
  // Re-capture modal close buttons
  const closeBtn = document.getElementById('btn-recapture-close');
  if (closeBtn) closeBtn.addEventListener('click', closeRecaptureModal);

  const cancelBtn = document.getElementById('btn-recapture-cancel');
  if (cancelBtn) cancelBtn.addEventListener('click', closeRecaptureModal);

  // Re-capture start button
  const startBtn = document.getElementById('btn-recapture-start');
  if (startBtn) startBtn.addEventListener('click', executeRecapture);

  // Mode selection → show/hide mode-specific options
  const modeSelect = document.getElementById('recapture-mode');
  if (modeSelect) {
    modeSelect.addEventListener('change', _updateRecaptureModeUI);
  }

  // Interval select → show/hide custom input
  const intervalSelect = document.getElementById('recapture-interval');
  const customIntervalGroup = document.getElementById('recapture-custom-interval-group');
  if (intervalSelect && customIntervalGroup) {
    intervalSelect.addEventListener('change', () => {
      customIntervalGroup.hidden = intervalSelect.value !== 'custom';
      if (intervalSelect.value === 'custom') {
        const customInput = document.getElementById('recapture-custom-interval');
        if (customInput) customInput.focus();
      }
    });
  }

  // Scene threshold slider → update display value
  const thresholdSlider = document.getElementById('recapture-scene-threshold');
  const thresholdValue = document.getElementById('recapture-threshold-value');
  if (thresholdSlider && thresholdValue) {
    thresholdSlider.addEventListener('input', () => {
      thresholdValue.textContent = thresholdSlider.value + '%';
    });
  }

  // Close modal on overlay click
  const modal = document.getElementById('recapture-modal');
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) closeRecaptureModal();
    });
  }

  // ─── Delete Confirmation Modal event handlers ────────────────
  const deleteCloseBtn = document.getElementById('btn-delete-close');
  if (deleteCloseBtn) deleteCloseBtn.addEventListener('click', closeDeleteConfirmModal);

  const deleteCancelBtn = document.getElementById('btn-delete-cancel');
  if (deleteCancelBtn) deleteCancelBtn.addEventListener('click', closeDeleteConfirmModal);

  const deleteConfirmBtn = document.getElementById('btn-delete-confirm');
  if (deleteConfirmBtn) deleteConfirmBtn.addEventListener('click', executeDelete);

  // Close delete modal on overlay click
  const deleteModal = document.getElementById('delete-confirm-modal');
  if (deleteModal) {
    deleteModal.addEventListener('click', (e) => {
      if (e.target === deleteModal) closeDeleteConfirmModal();
    });
  }
});

// ─── Delete Confirmation Modal ─────────────────────────────────────

/** Currently targeted video ID for deletion */
let _deleteVideoId = null;

/**
 * Open the delete confirmation modal for a library item.
 * @param {string} videoId
 * @param {string} title
 */
function openDeleteConfirmModal(videoId, title) {
  const modal = document.getElementById('delete-confirm-modal');
  const titleEl = document.getElementById('delete-confirm-title');
  const confirmBtn = document.getElementById('btn-delete-confirm');

  if (!modal) return;

  _deleteVideoId = videoId;

  if (titleEl) titleEl.textContent = title;
  if (confirmBtn) {
    confirmBtn.disabled = false;
    confirmBtn.textContent = t('library_delete');
  }

  modal.hidden = false;
}

/**
 * Close the delete confirmation modal.
 */
function closeDeleteConfirmModal() {
  const modal = document.getElementById('delete-confirm-modal');
  if (modal) modal.hidden = true;
  _deleteVideoId = null;
}

/**
 * Execute the deletion after user confirms.
 */
async function executeDelete() {
  if (!_deleteVideoId) return;

  const videoId = _deleteVideoId;
  const confirmBtn = document.getElementById('btn-delete-confirm');

  // Disable button to prevent double-click
  if (confirmBtn) {
    confirmBtn.disabled = true;
    confirmBtn.textContent = t('library_deleting') || '...';
  }

  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      await window.__TAURI__.core.invoke('delete_library_entry', { videoId });
    }
    closeDeleteConfirmModal();
    showToast(t('library_delete_success'), 'success');
    loadLibrary();
  } catch (err) {
    console.error('Failed to delete library entry:', err);
    closeDeleteConfirmModal();
    showToast(t('library_delete_error') + ': ' + err, 'error');
  }
}
