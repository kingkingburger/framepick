/**
 * FramePick - Main Application Controller
 *
 * Wires DOM elements to AppState and Tauri backend commands.
 */

document.addEventListener('DOMContentLoaded', () => {
  // ─── DOM references ────────────────────────────────────────
  const urlInput      = document.getElementById('url-input');
  const urlError      = document.getElementById('url-error');
  const btnAddQueue   = document.getElementById('btn-add-queue');
  const btnLang       = document.getElementById('btn-lang');
  const modeBtns      = document.querySelectorAll('.mode-btn');
  const intervalOpts  = document.getElementById('interval-options');
  const intervalBtns  = document.querySelectorAll('.interval-btn');
  const queueList     = document.getElementById('queue-list');

  // ─── Tauri interop helpers ─────────────────────────────────
  // window.__TAURI__ is available when running inside Tauri webview.
  // For dev/testing outside Tauri, we fall back to no-ops.
  const invoke = (cmd, args) => {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    console.log('[dev] invoke:', cmd, args);
    return Promise.resolve(null);
  };

  // ─── Playlist Dialog Controller ──────────────────────────
  const playlistDialog      = document.getElementById('playlist-dialog');
  const playlistDialogClose = document.getElementById('playlist-dialog-close');
  const playlistDialogTitle = document.getElementById('playlist-dialog-title');
  const playlistLoading     = document.getElementById('playlist-loading');
  const playlistErrorEl     = document.getElementById('playlist-error');
  const playlistContent     = document.getElementById('playlist-content');
  const playlistItemsList      = document.getElementById('playlist-items');
  const playlistInfoName       = document.getElementById('playlist-info-name');
  const playlistInfoCount      = document.getElementById('playlist-info-count');
  const playlistSelectAllBtn   = document.getElementById('playlist-select-all-btn');
  const playlistSelectNoneBtn  = document.getElementById('playlist-select-none-btn');
  const playlistSelectedCount  = document.getElementById('playlist-selected-count');
  const playlistBtnCancel      = document.getElementById('playlist-btn-cancel');
  const playlistBtnAdd      = document.getElementById('playlist-btn-add');

  let _playlistEntries = [];

  /** Format duration seconds to MM:SS or HH:MM:SS */
  function formatDuration(seconds) {
    if (!seconds || seconds <= 0) return '--:--';
    const total = Math.round(seconds);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    if (h > 0) return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  function showPlaylistDialog() {
    if (!playlistDialog) return;
    playlistDialog.hidden = false;
    if (playlistLoading) playlistLoading.hidden = false;
    if (playlistErrorEl) playlistErrorEl.hidden = true;
    if (playlistContent) playlistContent.hidden = true;
    _playlistEntries = [];
  }

  function hidePlaylistDialog() {
    if (!playlistDialog) return;
    playlistDialog.hidden = true;
    _playlistEntries = [];
  }

  function renderPlaylistEntries(result) {
    if (playlistLoading) playlistLoading.hidden = true;

    if (!result || !result.entries || result.entries.length === 0) {
      if (playlistErrorEl) {
        playlistErrorEl.textContent = I18n.t('playlistEmpty');
        playlistErrorEl.hidden = false;
      }
      return;
    }

    _playlistEntries = result.entries;

    // Update dialog title with playlist name
    if (playlistDialogTitle && result.playlist_title) {
      playlistDialogTitle.textContent = result.playlist_title;
    }

    // Update info bar
    if (playlistInfoName) {
      playlistInfoName.textContent = result.playlist_title || I18n.t('playlistTitle');
    }
    if (playlistInfoCount) {
      playlistInfoCount.textContent = _playlistEntries.length + I18n.t('playlistVideoCount');
    }

    // Render individual video items with checkboxes
    if (playlistItemsList) {
      playlistItemsList.innerHTML = _playlistEntries.map((entry, idx) => `
        <label class="playlist-item">
          <input type="checkbox" class="playlist-item-cb" data-index="${idx}" checked>
          <span class="playlist-item-index">${idx + 1}</span>
          <div class="playlist-item-info">
            <div class="playlist-item-title" title="${escapeHtml(entry.title)}">${escapeHtml(entry.title)}</div>
          </div>
          <span class="playlist-item-duration">${formatDuration(entry.duration)}</span>
        </label>
      `).join('');

      // Wire individual checkbox changes to update selected count
      playlistItemsList.querySelectorAll('.playlist-item-cb').forEach((cb) => {
        cb.addEventListener('change', () => {
          const row = cb.closest('.playlist-item');
          if (row) row.classList.toggle('unchecked', !cb.checked);
          updatePlaylistSelectedState();
        });
      });
    }

    if (playlistContent) playlistContent.hidden = false;
    updatePlaylistSelectedState();
  }

  /** Update the selected count display and enable/disable add button. */
  function updatePlaylistSelectedState() {
    if (!playlistItemsList) return;
    const checked = playlistItemsList.querySelectorAll('.playlist-item-cb:checked').length;

    if (playlistSelectedCount) {
      playlistSelectedCount.textContent = checked + I18n.t('playlistSelected');
    }
    if (playlistBtnAdd) {
      playlistBtnAdd.disabled = checked === 0;
      playlistBtnAdd.textContent = `${I18n.t('playlistAddSelected')} (${checked})`;
    }
  }

  /** Set all playlist checkboxes to a given state and update visuals. */
  function setAllPlaylistChecked(checked) {
    if (!playlistItemsList) return;
    playlistItemsList.querySelectorAll('.playlist-item-cb').forEach((cb) => {
      cb.checked = checked;
      const row = cb.closest('.playlist-item');
      if (row) row.classList.toggle('unchecked', !checked);
    });
    updatePlaylistSelectedState();
  }

  // Select-all / Select-none buttons
  if (playlistSelectAllBtn) {
    playlistSelectAllBtn.addEventListener('click', () => setAllPlaylistChecked(true));
  }
  if (playlistSelectNoneBtn) {
    playlistSelectNoneBtn.addEventListener('click', () => setAllPlaylistChecked(false));
  }

  // Close / cancel handlers
  if (playlistDialogClose) playlistDialogClose.addEventListener('click', hidePlaylistDialog);
  if (playlistBtnCancel) playlistBtnCancel.addEventListener('click', hidePlaylistDialog);
  if (playlistDialog) {
    playlistDialog.addEventListener('click', (e) => {
      if (e.target === playlistDialog) hidePlaylistDialog();
    });
  }
  // Escape key closes the playlist dialog
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && playlistDialog && !playlistDialog.hidden) {
      hidePlaylistDialog();
    }
  });

  // Add selected playlist videos to queue as individual entries
  if (playlistBtnAdd) {
    playlistBtnAdd.addEventListener('click', () => {
      const captureMode = AppState.get('captureMode');
      const intervalSeconds = AppState.get('intervalSeconds');
      let addedCount = 0;

      if (playlistItemsList) {
        playlistItemsList.querySelectorAll('.playlist-item-cb:checked').forEach((cb) => {
          const idx = Number(cb.dataset.index);
          const entry = _playlistEntries[idx];
          if (!entry) return;

          // Skip duplicates already in local queue (pending/processing)
          const queue = AppState.get('queue');
          const isDuplicate = queue.some((q) => {
            if (q.status !== 'pending' && q.status !== 'processing') return false;
            if (q.url === entry.url) return true;
            if (q.videoId === entry.video_id) return true;
            return false;
          });
          if (isDuplicate) {
            console.log('[playlist] Skipping duplicate:', entry.video_id, entry.title);
            return;
          }

          const item = AppState.addToQueue(entry.url, captureMode, intervalSeconds);
          if (item) {
            // Set title and videoId from playlist metadata immediately
            AppState.updateQueueItem(item.id, {
              title: entry.title,
              videoId: entry.video_id,
            });
            addedCount++;

            // Notify backend (non-blocking)
            invoke('add_queue_item', {
              item: {
                id: item.id,
                url: entry.url,
                capture_mode: captureMode,
                interval_seconds: intervalSeconds,
                status: 'pending',
                title: entry.title,
                error: null,
              },
            }).catch((err) => console.warn('[playlist] add_queue_item error:', err));
          }
        });
      }

      console.log(`[playlist] Added ${addedCount} videos to queue`);
      hidePlaylistDialog();
      urlInput.focus();
    });
  }

  /**
   * Handle playlist URL detection: open dialog, fetch entries via backend.
   * @param {string} url - Playlist URL
   */
  async function handlePlaylistDetected(url) {
    showPlaylistDialog();

    // Verify via backend detection
    try {
      const detection = await invoke('detect_playlist_url', { url });
      if (detection && !detection.is_playlist) {
        hidePlaylistDialog();
        return;
      }
    } catch (e) {
      console.warn('[playlist] Backend detect_playlist_url unavailable:', e);
    }

    // Fetch playlist entries via yt-dlp
    try {
      const result = await invoke('fetch_playlist', { url });
      renderPlaylistEntries(result);
    } catch (e) {
      if (playlistLoading) playlistLoading.hidden = true;
      if (playlistErrorEl) {
        playlistErrorEl.textContent = I18n.t('playlistFetchError') + (e ? ': ' + e : '');
        playlistErrorEl.hidden = false;
      }
    }
  }

  // ─── URL Input Component ─────────────────────────────────
  // Initialize the encapsulated URL input component.
  // It handles input/paste events, validation, dedup, backend calls,
  // and emits submitted items to the parent dashboard via onSubmit.
  const urlInputCtrl = UrlInputComponent.init({
    inputEl: urlInput,
    errorEl: urlError,
    submitBtnEl: btnAddQueue,
    onSubmit: (submittedItem) => {
      // Download request emitted from URL input → parent dashboard
      console.log('[dashboard] URL submitted to queue:', submittedItem);
      // Queue rendering is triggered automatically via AppState 'queue' listener
    },
    onValidationError: (message) => {
      console.log('[dashboard] URL validation failed:', message);
    },
    onPlaylistDetected: handlePlaylistDetected,
  });

  // ─── Capture mode binding ──────────────────────────────────
  modeBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = btn.dataset.mode;
      AppState.setCaptureMode(mode);
    });
  });

  // React to captureMode changes – update button active states & interval visibility
  AppState.on('captureMode', (mode) => {
    modeBtns.forEach((b) => {
      b.classList.toggle('active', b.dataset.mode === mode);
    });
    intervalOpts.hidden = mode !== 'interval';

    // Sync to backend
    invoke('set_input_state', { state: AppState.buildPipelineInput() });
  });

  // ─── Interval selector binding ─────────────────────────────
  const customIntervalGroup = document.getElementById('custom-interval-group');
  const customIntervalInput = document.getElementById('custom-interval-input');
  let _isCustomInterval = false;

  intervalBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      if (btn.dataset.seconds === 'custom') {
        _isCustomInterval = true;
        if (customIntervalGroup) customIntervalGroup.hidden = false;
        if (customIntervalInput) customIntervalInput.focus();
        intervalBtns.forEach((b) => b.classList.remove('active'));
        btn.classList.add('active');
      } else {
        _isCustomInterval = false;
        if (customIntervalGroup) customIntervalGroup.hidden = true;
        AppState.setIntervalSeconds(Number(btn.dataset.seconds));
      }
    });
  });

  // Handle custom interval input with debounce
  if (customIntervalInput) {
    let _customTimer = null;
    customIntervalInput.addEventListener('input', () => {
      clearTimeout(_customTimer);
      const val = parseInt(customIntervalInput.value, 10);
      const isValid = !isNaN(val) && val >= 1 && val <= 3600;
      customIntervalInput.classList.toggle('invalid', !isValid && customIntervalInput.value !== '');
      _customTimer = setTimeout(() => {
        if (isValid) {
          AppState.setIntervalSeconds(val);
        }
      }, 300);
    });
    customIntervalInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        clearTimeout(_customTimer);
        const val = parseInt(customIntervalInput.value, 10);
        if (!isNaN(val) && val >= 1 && val <= 3600) {
          AppState.setIntervalSeconds(val);
        }
      }
    });
  }

  AppState.on('intervalSeconds', (sec) => {
    const presets = [10, 30, 60];
    if (presets.includes(sec)) {
      _isCustomInterval = false;
      if (customIntervalGroup) customIntervalGroup.hidden = true;
    }
    intervalBtns.forEach((b) => {
      if (b.dataset.seconds === 'custom') {
        b.classList.toggle('active', _isCustomInterval);
      } else {
        b.classList.toggle('active', Number(b.dataset.seconds) === sec && !_isCustomInterval);
      }
    });
    invoke('set_input_state', { state: AppState.buildPipelineInput() });
  });

  // ─── URL state sync (debounced to avoid flooding backend) ──
  let _urlSyncTimer = null;
  AppState.on('url', () => {
    clearTimeout(_urlSyncTimer);
    _urlSyncTimer = setTimeout(() => {
      invoke('set_input_state', { state: AppState.buildPipelineInput() });
    }, 300);
  });

  // ─── Tauri event listeners for pipeline progress ───────────
  function listenTauriEvent(eventName, handler) {
    if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
      window.__TAURI__.event.listen(eventName, (event) => handler(event.payload));
    }
  }

  // Listen for per-item pipeline progress events from backend
  listenTauriEvent('pipeline:progress', (payload) => {
    // payload: { queue_id, stage, stage_number, total_stages, percent, detail? }
    AppState.updateQueueItemProgress(payload.queue_id, payload);

    // If stage is 'done', mark the item as completed with timestamp
    if (payload.stage === 'done') {
      AppState.updateQueueItem(payload.queue_id, {
        status: 'completed',
        finishedAt: Date.now(),
      });
    }
  });

  // Listen for pipeline error events
  listenTauriEvent('pipeline:error', (payload) => {
    // payload: { queue_id, stage, message }
    AppState.updateQueueItem(payload.queue_id, {
      status: 'failed',
      finishedAt: Date.now(),
      error: payload.message,
      progress: {
        stage: payload.stage,
        stageNumber: 0,
        totalStages: 0,
        percent: 0,
        detail: payload.message,
      },
    });

    // Note: toast notification is shown via the queue-item-status listener
    // to avoid duplicate toasts when both events fire for the same failure.
  });

  // Listen for queue item status changes from backend
  listenTauriEvent('queue-item-status', (payload) => {
    // payload: { id, status, progress?, title?, error?, slides_path? }
    const updates = { status: payload.status };
    if (payload.title != null) updates.title = payload.title;
    if (payload.error != null) updates.error = payload.error;
    if (payload.slides_path != null) updates.slidesPath = payload.slides_path;
    if (payload.status === 'processing') updates.startedAt = Date.now();
    if (payload.status === 'completed' || payload.status === 'failed') updates.finishedAt = Date.now();
    AppState.updateQueueItem(payload.id, updates);

    // Show toast notification on failure so the user is informed immediately
    if (payload.status === 'failed') {
      const label = payload.title || '';
      const errorDetail = payload.error || I18n.t('toastProcessingFailedDetail');
      const toastTitle = I18n.t('toastProcessingFailed');
      const msg = label
        ? toastTitle + ' — ' + label + ': ' + errorDetail
        : toastTitle + ': ' + errorDetail;
      showAppToast(msg, 'error', 5000);
    }
  });

  // Listen for overall queue progress
  listenTauriEvent('queue-progress', (payload) => {
    // payload: { total, completed, failed, is_processing }
    _queueSummary = payload;
    renderQueue(AppState.get('queue'));
  });

  // Listen for duplicate-skipped events
  listenTauriEvent('queue:duplicate-skipped', (payload) => {
    AppState.updateQueueItem(payload.id, {
      status: 'skipped',
      finishedAt: Date.now(),
    });
    // Show toast notification so the user sees why it was skipped
    const label = payload.title || payload.video_id || '';
    const msg = label
      ? I18n.t('toastDuplicateSkipped') + ' (' + label + ')'
      : I18n.t('toastDuplicateSkipped');
    showAppToast(msg, 'warning');
  });

  // Track overall queue summary from backend
  let _queueSummary = null;

  // Timer to update elapsed time display for processing items
  let _elapsedTimer = null;

  function startElapsedTimer() {
    if (_elapsedTimer) return;
    _elapsedTimer = setInterval(() => {
      const queue = AppState.get('queue');
      if (queue.some((q) => q.status === 'processing')) {
        renderQueue(queue);
      } else {
        clearInterval(_elapsedTimer);
        _elapsedTimer = null;
      }
    }, 1000);
  }

  // ─── Queue list rendering ──────────────────────────────────
  AppState.on('queue', (queue) => {
    renderQueue(queue);
    // Start elapsed timer if any items are processing
    if (queue.some((q) => q.status === 'processing')) {
      startElapsedTimer();
    }
  });

  /** Format elapsed time in human-readable form */
  function formatElapsed(ms) {
    const sec = Math.floor(ms / 1000);
    if (sec < 60) return `${sec}${I18n.t('elapsedSec')}`;
    const min = Math.floor(sec / 60);
    const remSec = sec % 60;
    if (min < 60) return `${min}${I18n.t('elapsedMin')} ${remSec}${I18n.t('elapsedSec')}`;
    const hr = Math.floor(min / 60);
    const remMin = min % 60;
    return `${hr}${I18n.t('elapsedHour')} ${remMin}${I18n.t('elapsedMin')}`;
  }

  /** Get status icon character for each queue status */
  function statusIcon(status) {
    switch (status) {
      case 'pending':    return '\u23F3'; // hourglass
      case 'processing': return '\u25B6'; // play
      case 'completed':
      case 'done':       return '\u2714'; // checkmark
      case 'failed':
      case 'error':      return '\u2716'; // cross
      case 'skipped':    return '\u23ED'; // skip
      default:           return '\u2022'; // bullet
    }
  }

  /** Sort queue items: processing first, then pending, then failed, then completed */
  function sortedQueue(queue) {
    const order = { processing: 0, pending: 1, failed: 2, error: 2, skipped: 3, completed: 4, done: 4 };
    return [...queue].sort((a, b) => {
      const oa = order[a.status] ?? 5;
      const ob = order[b.status] ?? 5;
      if (oa !== ob) return oa - ob;
      return a.id - b.id;
    });
  }

  function renderQueue(queue) {
    if (!queue || queue.length === 0) {
      queueList.innerHTML = `<p class="queue-empty" data-i18n="queueEmpty">${I18n.t('queueEmpty')}</p>`;
      return;
    }

    const stats = AppState.getQueueStats();
    const overallPercent = stats.total > 0
      ? Math.round(((stats.completed + stats.failed + stats.skipped) / stats.total) * 100)
      : 0;

    // Build queue summary bar with clear-completed button
    let summaryHtml = '';
    if (stats.total > 1 || stats.completed > 0 || stats.failed > 0) {
      const clearBtn = stats.completed > 0
        ? `<button class="btn-clear-completed" data-action="clear-completed">${I18n.t('clearCompleted')}</button>`
        : '';
      summaryHtml = `
        <div class="queue-summary">
          <div class="queue-summary-stats">
            <div class="queue-summary-item">
              <span class="queue-summary-count count-completed">${stats.completed}</span>
              <span>${I18n.t('queueCompleted')}</span>
            </div>
            <div class="queue-summary-item">
              <span class="queue-summary-count count-failed">${stats.failed}</span>
              <span>${I18n.t('queueFailed')}</span>
            </div>
            <div class="queue-summary-item">
              <span class="queue-summary-count count-remaining">${stats.pending + stats.processing}</span>
              <span>${I18n.t('queueRemaining')}</span>
            </div>
            ${clearBtn}
          </div>
          <div class="progress-bar-track summary-progress">
            <div class="progress-bar-fill${overallPercent >= 100 ? ' complete' : ''}" style="width: ${overallPercent}%"></div>
          </div>
        </div>`;
    }

    const sorted = sortedQueue(queue);
    const itemsHtml = sorted
      .map((item) => {
        const modeKey =
          item.captureMode === 'subtitle' ? 'modeSubtitle' :
          item.captureMode === 'scene'    ? 'modeScene' :
          'modeInterval';
        const statusKey = 'status' + item.status.charAt(0).toUpperCase() + item.status.slice(1);
        const modeLabel = I18n.t(modeKey) +
          (item.captureMode === 'interval' ? ` (${item.intervalSeconds}s)` : '');
        const icon = statusIcon(item.status);

        // Elapsed / time info
        let timeHtml = '';
        if (item.status === 'processing' && item.startedAt) {
          const elapsed = Date.now() - item.startedAt;
          timeHtml = `<span class="queue-item-time">${I18n.t('elapsed')}: ${formatElapsed(elapsed)}</span>`;
        } else if ((item.status === 'completed' || item.status === 'done') && item.startedAt && item.finishedAt) {
          const duration = item.finishedAt - item.startedAt;
          timeHtml = `<span class="queue-item-time">${formatElapsed(duration)}</span>`;
        }

        // Progress bar for processing items
        let progressHtml = '';
        if (item.status === 'processing' && item.progress) {
          const p = item.progress;
          const stageName = I18n.t(p.stage ? ('progress_' + p.stage) : 'statusProcessing');
          const stageCounter = p.totalStages > 0
            ? `${I18n.t('progressStage')} ${p.stageNumber}${I18n.t('progressOf')}${p.totalStages}`
            : '';
          const barClass = p.percent >= 100 ? ' complete' : '';
          const detailHtml = p.detail ? `<div class="progress-detail">${escapeHtml(p.detail)}</div>` : '';

          progressHtml = `
            <div class="queue-item-progress">
              <div class="progress-stage-info">
                <span class="progress-stage-label"><span class="processing-indicator"></span>${escapeHtml(stageName)}</span>
                <span class="progress-stage-counter">${stageCounter}</span>
                <span class="progress-percent">${p.percent}%</span>
              </div>
              <div class="progress-bar-track">
                <div class="progress-bar-fill${barClass}" style="width: ${p.percent}%"></div>
              </div>
              ${detailHtml}
            </div>`;
        }

        // Error display
        let errorHtml = '';
        if (item.status === 'failed' && item.error) {
          errorHtml = `<div class="queue-item-error" title="${escapeHtml(item.error)}">${escapeHtml(item.error)}</div>`;
        }

        // Action buttons based on status
        let actionsHtml = '<div class="queue-item-actions">';
        if (item.status === 'pending') {
          actionsHtml += `<button class="btn-queue-action btn-remove" data-action="remove" data-id="${item.id}" title="${I18n.t('removeItem')}">&#x2715;</button>`;
        } else if (item.status === 'completed' || item.status === 'done') {
          actionsHtml += `<button class="btn-queue-action btn-view-slides" data-action="view-slides" data-id="${item.id}" title="${I18n.t('viewSlides')}">&#x1F4C4;</button>`;
          actionsHtml += `<button class="btn-queue-action btn-remove" data-action="remove" data-id="${item.id}" title="${I18n.t('removeItem')}">&#x2715;</button>`;
        } else if (item.status === 'failed' || item.status === 'error') {
          actionsHtml += `<button class="btn-queue-action btn-retry" data-action="retry" data-id="${item.id}" title="${I18n.t('retryItem')}">&#x21BB;</button>`;
          actionsHtml += `<button class="btn-queue-action btn-remove" data-action="remove" data-id="${item.id}" title="${I18n.t('removeItem')}">&#x2715;</button>`;
        } else if (item.status === 'skipped') {
          actionsHtml += `<button class="btn-queue-action btn-remove" data-action="remove" data-id="${item.id}" title="${I18n.t('removeItem')}">&#x2715;</button>`;
        }
        actionsHtml += '</div>';

        return `
          <div class="queue-item queue-item--${item.status}" data-queue-id="${item.id}">
            <div class="queue-item-row">
              <span class="queue-item-icon status-${item.status}">${icon}</span>
              <div class="queue-item-info">
                <span class="queue-item-title">${escapeHtml(item.title || item.url)}</span>
                <div class="queue-item-meta">
                  <span class="queue-item-mode">${modeLabel}</span>
                  <span class="queue-item-status status-${item.status}">${I18n.t(statusKey)}</span>
                  ${timeHtml}
                </div>
              </div>
              ${actionsHtml}
            </div>
            ${progressHtml}
            ${errorHtml}
          </div>`;
      })
      .join('');

    queueList.innerHTML = summaryHtml + itemsHtml;
  }

  // ─── Queue action event delegation (bound once) ────────────
  async function handleQueueAction(e) {
    const btn = e.target.closest('[data-action]');
    if (!btn) return;

    const action = btn.dataset.action;
    const id = btn.dataset.id ? Number(btn.dataset.id) : null;

    switch (action) {
      case 'remove':
        if (id != null) {
          AppState.removeQueueItem(id);
          try { await invoke('remove_queue_item', { id }); } catch (_) { /* ok */ }
        }
        break;

      case 'retry':
        if (id != null) {
          AppState.updateQueueItem(id, {
            status: 'pending',
            error: null,
            progress: null,
            startedAt: null,
            finishedAt: null,
          });
          try { await invoke('retry_queue_item', { id }); } catch (_) { /* ok */ }
        }
        break;

      case 'view-slides': {
        if (id != null) {
          const queue = AppState.get('queue');
          const item = queue.find((q) => q.id === id);
          if (item && item.slidesPath) {
            try { await invoke('open_slides', { path: item.slidesPath }); } catch (_) { /* ok */ }
          } else {
            try { await invoke('view_slides_for_item', { id }); } catch (_) { /* ok */ }
          }
        }
        break;
      }

      case 'clear-completed': {
        const queue = AppState.get('queue');
        const completedIds = queue
          .filter((q) => q.status === 'completed' || q.status === 'done' || q.status === 'skipped')
          .map((q) => q.id);
        AppState.clearCompletedItems();
        for (const cid of completedIds) {
          try { await invoke('remove_queue_item', { id: cid }); } catch (_) { /* ok */ }
        }
        break;
      }
    }
  }

  // Bind event delegation once on the queue list container
  queueList.addEventListener('click', handleQueueAction);

  function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  // ─── Toast notifications ────────────────────────────────────
  /**
   * Show a toast notification in the dashboard.
   * @param {string} message - Text to display
   * @param {'success'|'error'|'warning'} [type='warning'] - Toast variant
   * @param {number} [duration=3500] - Auto-dismiss ms
   */
  function showAppToast(message, type, duration) {
    type = type || 'warning';
    duration = duration || 3500;
    const container = document.getElementById('toast-container');
    if (!container) return;
    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    toast.textContent = message;
    container.appendChild(toast);
    requestAnimationFrame(() => {
      toast.classList.add('toast-show');
    });
    setTimeout(() => {
      toast.classList.remove('toast-show');
      setTimeout(() => toast.remove(), 350);
    }, duration);
  }

  // ─── Language toggle ───────────────────────────────────────
  btnLang.addEventListener('click', () => {
    const lang = I18n.toggle();
    AppState.setLanguage(lang);
    invoke('set_language', { language: lang });
  });

  AppState.on('language', () => {
    // Re-render queue and library with new language strings
    renderQueue(AppState.get('queue'));
    renderLibrary(_libraryEntries);
  });

  // ─── Library section ─────────────────────────────────────────
  const libraryGrid   = document.getElementById('library-grid');
  const btnRefreshLib = document.getElementById('btn-refresh-library');
  let _libraryEntries = [];

  /** Fetch library entries from backend and render them. */
  async function loadLibrary() {
    if (!libraryGrid) return;

    libraryGrid.innerHTML = `<p class="library-loading"><span class="spinner"></span>${I18n.t('libraryLoading')}</p>`;

    try {
      const entries = await invoke('list_library_entries');
      _libraryEntries = entries || [];
      renderLibrary(_libraryEntries);
    } catch (e) {
      console.warn('[library] Failed to load library:', e);
      _libraryEntries = [];
      renderLibrary([]);
    }
  }

  /** Render library entries as thumbnail grid cards. */
  function renderLibrary(entries) {
    if (!libraryGrid) return;

    if (!entries || entries.length === 0) {
      libraryGrid.innerHTML = `<p class="library-empty" data-i18n="libraryEmpty">${I18n.t('libraryEmpty')}</p>`;
      return;
    }

    const cardsHtml = entries.map((entry) => {
      const title = entry.title || entry.video_id;
      const slideInfo = entry.slide_count != null
        ? `${entry.slide_count} ${I18n.t('librarySlides')}`
        : I18n.t('libraryNoSlides');

      let thumbHtml;
      if (entry.thumbnail) {
        thumbHtml = `<img src="${escapeAttr(entry.thumbnail)}" alt="${escapeAttr(title)}" loading="lazy">`;
      } else {
        thumbHtml = `<div class="thumb-placeholder">🎬</div>`;
      }

      const badgeHtml = entry.slide_count != null
        ? `<span class="library-card-badge">${entry.slide_count}</span>`
        : '';

      return `
        <div class="library-card" data-video-id="${escapeAttr(entry.video_id)}" title="${escapeAttr(title)}">
          <div class="library-card-thumb">
            ${thumbHtml}
            ${badgeHtml}
          </div>
          <div class="library-card-info">
            <div class="library-card-title">${escapeHtml(title)}</div>
            <div class="library-card-meta">${escapeHtml(slideInfo)}</div>
          </div>
          <div class="library-card-actions">
            <button class="btn-icon btn-open-folder" data-video-id="${escapeAttr(entry.video_id)}" title="${I18n.t('openFolder')}">
              <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor"><path d="M1 3.5A1.5 1.5 0 0 1 2.5 2h3.879a1.5 1.5 0 0 1 1.06.44l1.122 1.12A1.5 1.5 0 0 0 9.62 4H13.5A1.5 1.5 0 0 1 15 5.5v7a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 1 12.5v-9z"/></svg>
            </button>
          </div>
        </div>`;
    }).join('');

    libraryGrid.innerHTML = cardsHtml;

    // Attach click handlers for viewing slides
    libraryGrid.querySelectorAll('.library-card').forEach((card) => {
      card.addEventListener('click', () => {
        const videoId = card.dataset.videoId;
        if (videoId) {
          CaptureListComponent.openCaptureList(videoId);
        }
      });
    });
  }

  /** Open slides for a video in external browser. */
  async function handleOpenSlides(videoId) {
    try {
      await invoke('open_slides_external', { videoId });
    } catch (e) {
      console.warn('[library] Failed to open slides for', videoId, e);
    }
  }

  /** Escape string for use in HTML attributes. */
  function escapeAttr(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // Refresh button
  if (btnRefreshLib) {
    btnRefreshLib.addEventListener('click', () => {
      loadLibrary();
    });
  }

  // Auto-refresh library when a queue item completes
  listenTauriEvent('pipeline:progress', (payload) => {
    if (payload.stage === 'done') {
      // Debounce library refresh to avoid rapid re-fetches
      clearTimeout(_libRefreshTimer);
      _libRefreshTimer = setTimeout(() => loadLibrary(), 1000);
    }
  });
  let _libRefreshTimer = null;

  // ─── Settings Modal Controller ────────────────────────────
  const settingsModal       = document.getElementById('settings-modal');
  const btnSettingsOpen     = document.getElementById('btn-settings');
  const btnSettingsClose    = document.getElementById('btn-settings-close');
  const btnSettingsSave     = document.getElementById('btn-settings-save');
  const btnSettingsCancel   = document.getElementById('btn-settings-cancel');
  const btnSettingsReset    = document.getElementById('btn-settings-reset');
  const settingsModeBtns    = document.querySelectorAll('.settings-mode-btn');
  const settingsInterval    = document.getElementById('settings-interval');
  const settingsThreshold   = document.getElementById('settings-scene-threshold');
  const settingsThresholdVal = document.getElementById('settings-scene-threshold-value');
  const settingsQuality     = document.getElementById('settings-quality');
  const settingsLanguage    = document.getElementById('settings-language');
  const settingsMp4         = document.getElementById('settings-mp4-retention');
  const settingsLibPath     = document.getElementById('settings-library-path');
  const settingsConfigPath  = document.getElementById('settings-config-path-value');
  const settingsFfmpeg      = document.getElementById('settings-ffmpeg-status');
  const settingsYtdlp       = document.getElementById('settings-ytdlp-status');
  const btnBrowseLibrary    = document.getElementById('btn-browse-library');

  // Track pending changes in the modal (not yet saved)
  let _pendingSettings = {};

  /** Open the settings modal and populate fields from backend. */
  async function openSettings() {
    if (!settingsModal) return;
    settingsModal.hidden = false;
    _pendingSettings = {};

    // Load current settings from backend
    try {
      const settings = await invoke('get_settings');
      if (settings) {
        populateSettingsForm(settings);
      }
    } catch (e) {
      console.warn('[settings] Could not load settings:', e);
    }

    // Validate to show tool status
    try {
      const validation = await invoke('validate_settings');
      if (validation) {
        showValidationStatus(validation);
      }
    } catch (e) {
      console.warn('[settings] Could not validate settings:', e);
    }
  }

  /** Populate settings form fields from a settings object. */
  function populateSettingsForm(s) {
    // Capture mode
    settingsModeBtns.forEach((btn) => {
      btn.classList.toggle('active', btn.dataset.settingMode === s.default_capture_mode);
    });

    // Interval
    if (settingsInterval) settingsInterval.value = s.default_interval_seconds || 30;

    // Scene threshold
    if (settingsThreshold) {
      settingsThreshold.value = s.scene_change_threshold || 0.30;
      if (settingsThresholdVal) {
        settingsThresholdVal.textContent = parseFloat(s.scene_change_threshold || 0.30).toFixed(2);
      }
    }

    // Quality
    if (settingsQuality) settingsQuality.value = s.download_quality || '720';

    // Language
    if (settingsLanguage) settingsLanguage.value = s.language || 'ko';

    // MP4 retention
    if (settingsMp4) settingsMp4.checked = !!s.mp4_retention;

    // Library path
    if (settingsLibPath) settingsLibPath.value = s.library_path || './library/';
  }

  /** Show validation results (tool availability, config path). */
  function showValidationStatus(v) {
    if (settingsConfigPath) {
      settingsConfigPath.textContent = v.config_path || '—';
    }
    if (settingsFfmpeg) {
      settingsFfmpeg.textContent = v.ffmpeg_found ? I18n.t('settingsToolFound') : I18n.t('settingsToolMissing');
      settingsFfmpeg.style.color = v.ffmpeg_found ? 'var(--success)' : 'var(--error)';
    }
    if (settingsYtdlp) {
      settingsYtdlp.textContent = v.ytdlp_found ? I18n.t('settingsToolFound') : I18n.t('settingsToolMissing');
      settingsYtdlp.style.color = v.ytdlp_found ? 'var(--success)' : 'var(--error)';
    }
  }

  /** Close settings modal without saving. */
  function closeSettings() {
    if (!settingsModal) return;
    settingsModal.hidden = true;
    _pendingSettings = {};
  }

  /** Collect values from form and save to backend. */
  async function saveSettings() {
    const patch = {};

    // Capture mode
    const activeMode = document.querySelector('.settings-mode-btn.active');
    if (activeMode) patch.default_capture_mode = activeMode.dataset.settingMode;

    // Interval
    if (settingsInterval) {
      const val = parseInt(settingsInterval.value, 10);
      if (!isNaN(val) && val >= 1 && val <= 3600) {
        patch.default_interval_seconds = val;
      }
    }

    // Scene threshold
    if (settingsThreshold) {
      const val = parseFloat(settingsThreshold.value);
      if (!isNaN(val) && val >= 0.01 && val <= 1.0) {
        patch.scene_change_threshold = val;
      }
    }

    // Quality
    if (settingsQuality) patch.download_quality = settingsQuality.value;

    // Language
    if (settingsLanguage) patch.language = settingsLanguage.value;

    // MP4 retention
    if (settingsMp4) patch.mp4_retention = settingsMp4.checked;

    // Library path
    if (settingsLibPath && settingsLibPath.value.trim()) {
      patch.library_path = settingsLibPath.value.trim();
    }

    // Save to backend
    if (btnSettingsSave) {
      btnSettingsSave.disabled = true;
      btnSettingsSave.textContent = I18n.t('settingsSaving');
    }

    try {
      const updated = await invoke('update_settings', { patch });
      if (updated) {
        // Apply language change immediately
        if (patch.language) {
          I18n.setLanguage(patch.language);
          AppState.setLanguage(patch.language);
        }
        // Apply default capture mode to main UI
        if (patch.default_capture_mode) {
          AppState.setCaptureMode(patch.default_capture_mode);
        }
        if (patch.default_interval_seconds) {
          AppState.setIntervalSeconds(patch.default_interval_seconds);
        }
      }
      closeSettings();
      showAppToast(I18n.t('settingsSaved'), 'success');
      // Reload library in case library path changed
      loadLibrary();
    } catch (e) {
      console.error('[settings] Save failed:', e);
      showAppToast(I18n.t('settingsSaveError') + ': ' + (e || ''), 'error', 5000);
    } finally {
      if (btnSettingsSave) {
        btnSettingsSave.disabled = false;
        btnSettingsSave.textContent = I18n.t('settingsSave');
      }
    }
  }

  /** Reset all settings to defaults. */
  async function resetSettings() {
    if (!confirm(I18n.t('settingsResetConfirm'))) return;

    try {
      const defaults = await invoke('reset_settings');
      if (defaults) {
        populateSettingsForm(defaults);
        // Apply language
        I18n.setLanguage(defaults.language || 'ko');
        AppState.setLanguage(defaults.language || 'ko');
        AppState.setCaptureMode(defaults.default_capture_mode || 'subtitle');
        AppState.setIntervalSeconds(defaults.default_interval_seconds || 30);
      }
      showAppToast(I18n.t('settingsResetDone'), 'success');
    } catch (e) {
      console.error('[settings] Reset failed:', e);
      showAppToast(I18n.t('settingsSaveError') + ': ' + (e || ''), 'error');
    }
  }

  // Wire settings modal events
  if (btnSettingsOpen) btnSettingsOpen.addEventListener('click', openSettings);
  if (btnSettingsClose) btnSettingsClose.addEventListener('click', closeSettings);
  if (btnSettingsCancel) btnSettingsCancel.addEventListener('click', closeSettings);
  if (btnSettingsSave) btnSettingsSave.addEventListener('click', saveSettings);
  if (btnSettingsReset) btnSettingsReset.addEventListener('click', resetSettings);

  // Close on overlay click
  if (settingsModal) {
    settingsModal.addEventListener('click', (e) => {
      if (e.target === settingsModal) closeSettings();
    });
  }

  // Close on Escape key
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && settingsModal && !settingsModal.hidden) {
      closeSettings();
    }
  });

  // Settings capture mode selector
  settingsModeBtns.forEach((btn) => {
    btn.addEventListener('click', () => {
      settingsModeBtns.forEach((b) => b.classList.remove('active'));
      btn.classList.add('active');
    });
  });

  // Scene threshold slider real-time update
  if (settingsThreshold) {
    settingsThreshold.addEventListener('input', () => {
      if (settingsThresholdVal) {
        settingsThresholdVal.textContent = parseFloat(settingsThreshold.value).toFixed(2);
      }
    });
  }

  // Browse library path button
  if (btnBrowseLibrary) {
    btnBrowseLibrary.addEventListener('click', async () => {
      try {
        // Use Tauri dialog API to select folder
        if (window.__TAURI__ && window.__TAURI__.dialog) {
          const selected = await window.__TAURI__.dialog.open({
            directory: true,
            title: I18n.t('settingsLibraryPath'),
          });
          if (selected && settingsLibPath) {
            settingsLibPath.value = selected;
          }
        }
      } catch (e) {
        console.warn('[settings] Browse dialog error:', e);
      }
    });
  }

  // ─── Initialize from backend settings ──────────────────────
  async function initFromSettings() {
    try {
      const settings = await invoke('get_settings');
      if (settings) {
        if (settings.language) {
          I18n.setLanguage(settings.language);
          AppState.setLanguage(settings.language);
        }
        // Apply default capture mode and interval from settings
        if (settings.default_capture_mode) {
          AppState.setCaptureMode(settings.default_capture_mode);
        }
        if (settings.default_interval_seconds) {
          AppState.setIntervalSeconds(settings.default_interval_seconds);
        }
      }
    } catch (e) {
      console.warn('[init] Could not load settings from backend:', e);
    }
  }

  // Apply default i18n
  I18n.applyToDOM();

  // Try loading from backend
  initFromSettings();

  
  // Initialize capture list component
  CaptureListComponent.init({
    invoke: invoke,
    escapeHtml: escapeHtml,
    escapeAttr: escapeAttr,
    handleOpenSlides: handleOpenSlides,
  });

  // ─── Workflow Empty Hint Visibility ──────────────────────────
  const workflowHint = document.getElementById('workflow-empty-hint');

  /** Show or hide the workflow hint based on queue and capture state. */
  function updateWorkflowHint() {
    if (!workflowHint) return;
    const queue = AppState.get('queue');
    const captureSec = document.getElementById('capture-list-section');
    const captureVisible = captureSec && !captureSec.hidden;
    // Hide hint when queue has items or capture list is visible
    workflowHint.hidden = (queue.length > 0) || captureVisible;
  }

  // Listen for queue changes to update hint visibility
  AppState.on('queue', updateWorkflowHint);
  // Listen for capture frame changes to update hint visibility
  AppState.on('captureFrames', updateWorkflowHint);

  // Auto-show capture list when a pipeline item completes
  listenTauriEvent('pipeline:progress', (payload) => {
    if (payload.stage === 'done' && payload.queue_id) {
      const queue = AppState.get('queue');
      const item = queue.find((q) => q.id === payload.queue_id);
      if (item && item.videoId) {
        CaptureListComponent.openCaptureList(item.videoId);
      }
    }
  });

  // Load library on startup
  loadLibrary();

  // Initial hint visibility
  updateWorkflowHint();
});
