/**
 * queue.js - Processing queue UI component for framepick
 *
 * Displays a list of queued URLs with:
 *   - Current URL being processed with a progress bar
 *   - Stage-based progress (e.g., "3/5 steps: downloading... 45%")
 *   - Status icons for each queued item (pending, processing, completed, failed)
 *
 * Listens for backend Tauri events:
 *   - queue-item-status: { id, status, progress?, title?, error? }
 *   - queue-progress: { total, completed, failed, is_processing }
 *   - pipeline:progress: { queue_id, stage, stage_number, total_stages, percent, detail? }
 *   - pipeline:error: { queue_id, stage, message }
 *
 * Emits DOM events:
 *   - queueItemAdded: { id, url, videoId }
 *   - queueItemCompleted: { id }
 *   - queueItemFailed: { id, error }
 *   - queueCleared: {}
 */

const QueueUI = (() => {
  /** @type {Array<QueueItem>} */
  let queue = [];
  let nextId = 1;
  let containerEl = null;
  let isProcessing = false;

  /** Elapsed-time tracker for the currently processing item */
  let _elapsedTimer = null;
  let _processingStartTime = null;

  /** Track IDs that already showed a failure toast to prevent duplicates */
  let _failToastShown = new Set();

  /**
   * @typedef {Object} QueueItem
   * @property {number} id
   * @property {string} url
   * @property {string} videoId
   * @property {string} captureMode
   * @property {number} intervalSeconds
   * @property {'pending'|'processing'|'completed'|'failed'} status
   * @property {number} currentStep - 0-based index of current pipeline step
   * @property {number} totalSteps
   * @property {string} stageLabel - i18n key for current stage
   * @property {number} stagePercent - 0-100 within current stage
   * @property {string|null} title - Video title from backend
   * @property {string|null} [errorMessage]
   */

  // Pipeline stages definition (max set for subtitle mode; non-subtitle modes use a subset)
  const PIPELINE_STAGES = [
    { key: 'queue_stage_download', label: 'Downloading' },
    { key: 'queue_stage_subtitle', label: 'Fetching subtitles' },
    { key: 'queue_stage_capture', label: 'Capturing frames' },
    { key: 'queue_stage_generate', label: 'Generating slides' },
    { key: 'queue_stage_cleanup', label: 'Cleaning up' },
  ];

  const TOTAL_STEPS = PIPELINE_STAGES.length;

  // Stage key mapping from backend snake_case to our PIPELINE_STAGES indices
  const STAGE_KEY_MAP = {
    'downloading': 'queue_stage_download',
    'extracting_subtitles': 'queue_stage_subtitle',
    'extracting_frames': 'queue_stage_capture',
    'generating_slides': 'queue_stage_generate',
    'cleanup': 'queue_stage_cleanup',
    'done': 'queue_status_done',
  };

  // Status icons as inline SVG
  const STATUS_ICONS = {
    pending: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>',
    processing: '<svg class="queue-spin" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>',
    completed: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#22c55e" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    failed: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
    skipped: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="8" y1="12" x2="16" y2="12"/></svg>',
  };

  /**
   * Initialize the queue UI.
   */
  function init() {
    containerEl = document.getElementById('queue-container');
    if (!containerEl) {
      console.warn('QueueUI: Missing #queue-container element');
      return;
    }
    render();

    // Listen for language changes to re-render
    document.addEventListener('languageChanged', () => render());

    // Set up Tauri event listeners for real-time updates from the backend
    _setupTauriListeners();
  }

  /**
   * Set up Tauri event listeners for backend → frontend communication.
   */
  function _setupTauriListeners() {
    if (!window.__TAURI__ || !window.__TAURI__.event) return;
    const { listen } = window.__TAURI__.event;

    // Queue item status changes (from queue_processor.rs)
    listen('queue-item-status', (event) => {
      const data = event.payload;
      _handleBackendStatusUpdate(data);
    });

    // Overall queue progress (from queue_processor.rs)
    listen('queue-progress', (event) => {
      const data = event.payload;
      isProcessing = data.is_processing;
      // Re-render to update processing indicator
      render();
    });

    // Pipeline stage-level progress (from progress.rs ProgressTracker)
    listen('pipeline:progress', (event) => {
      const data = event.payload;
      _handlePipelineProgress(data);
    });

    // Pipeline errors (from progress.rs ProgressTracker)
    listen('pipeline:error', (event) => {
      const data = event.payload;
      _handlePipelineError(data);
    });
  }

  /**
   * Handle a queue item status update from the backend.
   * @param {{ id: number, status: string, progress?: number, title?: string, error?: string }} data
   */
  function _handleBackendStatusUpdate(data) {
    const item = _findItem(data.id);
    if (!item) return;

    // Map backend status to our local status
    item.status = data.status;
    if (data.title) item.title = data.title;
    if (data.error) item.errorMessage = data.error;

    if (data.status === 'processing') {
      isProcessing = true;
      _startElapsedTimer(data.id);
    }

    if (data.status === 'completed') {
      _stopElapsedTimer();
      item.currentStep = item.totalSteps || TOTAL_STEPS;
      item.stagePercent = 100;
      _checkProcessingDone();
      document.dispatchEvent(new CustomEvent('queueItemCompleted', {
        detail: { id: data.id }
      }));
    }

    if (data.status === 'failed') {
      _stopElapsedTimer();
      item.errorMessage = data.error || t('queue_error_unknown');
      _checkProcessingDone();

      // Show toast notification for the failure (deduped)
      _showFailureToast(data.id, item.title, item.errorMessage);

      document.dispatchEvent(new CustomEvent('queueItemFailed', {
        detail: { id: data.id, error: data.error }
      }));
    }

    // "skipped" status: duplicate detected during processing — treat like completed but with a warning
    if (data.status === 'skipped') {
      _stopElapsedTimer();
      item.status = 'skipped';
      item.errorMessage = data.error || t('error_already_exists');
      _checkProcessingDone();
    }

    render();
  }

  /**
   * Handle pipeline stage-level progress from the backend.
   * Maps progress.rs stages to our PIPELINE_STAGES.
   * @param {{ queue_id: number, stage: string, stage_number: number, total_stages: number, percent: number, detail?: string }} data
   */
  function _handlePipelineProgress(data) {
    const item = _findItem(data.queue_id);
    if (!item) return;

    // Handle 'done' stage
    if (data.stage === 'done') {
      _stopElapsedTimer();
      item.status = 'completed';
      item.currentStep = item.totalSteps || data.total_stages;
      item.stagePercent = 100;
      _checkProcessingDone();

      document.dispatchEvent(new CustomEvent('queueItemCompleted', {
        detail: { id: data.queue_id }
      }));

      render();
      return;
    }

    // Ensure status is 'processing' when we get progress
    if (item.status === 'pending') {
      item.status = 'processing';
      isProcessing = true;
      _startElapsedTimer(data.queue_id);
    }

    // Map backend stage to our step index (0-based) and use backend's total_stages
    item.currentStep = Math.max(0, data.stage_number - 1);
    item.totalSteps = data.total_stages;
    item.stagePercent = data.percent;
    item.stageLabel = STAGE_KEY_MAP[data.stage] || 'queue_stage_download';
    item._detail = data.detail || null;

    render();
  }

  /**
   * Handle pipeline error event from the backend.
   * @param {{ queue_id: number, stage: string, message: string }} data
   */
  function _handlePipelineError(data) {
    const item = _findItem(data.queue_id);
    if (!item) return;

    _stopElapsedTimer();
    item.status = 'failed';
    item.errorMessage = data.message;
    _checkProcessingDone();

    // Show toast notification for the failure (deduped)
    _showFailureToast(data.queue_id, item.title, data.message);

    document.dispatchEvent(new CustomEvent('queueItemFailed', {
      detail: { id: data.queue_id, error: data.message }
    }));

    render();
  }

  /**
   * Add a URL to the processing queue.
   * Adds to both local state and backend queue, then starts processing.
   * @param {string} url
   * @param {string} videoId
   * @returns {Promise<number>} queue item id
   */
  async function addItem(url, videoId) {
    // Get current capture mode config from DOM
    const modeConfig = typeof getCaptureModeConfig === 'function'
      ? getCaptureModeConfig()
      : { mode: 'subtitle' };

    const id = nextId++;
    const item = {
      id,
      url,
      videoId,
      captureMode: modeConfig.mode || 'subtitle',
      intervalSeconds: modeConfig.interval || 30,
      status: 'pending',
      currentStep: 0,
      totalSteps: TOTAL_STEPS,
      stageLabel: '',
      stagePercent: 0,
      title: null,
      errorMessage: null,
    };

    // Add to backend queue first
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        const backendItem = await window.__TAURI__.core.invoke('add_queue_item', {
          item: {
            id: item.id,
            url: item.url,
            capture_mode: item.captureMode,
            interval_seconds: item.intervalSeconds,
            status: 'pending',
            title: null,
            error: null,
            progress: null,
          }
        });
        // Use backend's assigned values
        item.id = backendItem.id;
      } catch (err) {
        console.warn('Failed to add to backend queue:', err);
        _showToast(String(err), 'error');
        return -1;
      }
    }

    queue.push(item);
    render();

    document.dispatchEvent(new CustomEvent('queueItemAdded', {
      detail: { id: item.id, url, videoId }
    }));

    // Auto-start processing
    _startProcessing();

    return item.id;
  }

  /**
   * Start the backend queue processing loop.
   */
  async function _startProcessing() {
    if (isProcessing) return;

    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        await window.__TAURI__.core.invoke('start_queue_processing');
        isProcessing = true;
      } catch (err) {
        console.warn('Failed to start queue processing:', err);
      }
    }
  }

  /**
   * Start processing a queue item (for manual/local use).
   * @param {number} id
   */
  function startItem(id) {
    const item = _findItem(id);
    if (!item) return;
    item.status = 'processing';
    item.currentStep = 0;
    item.stageLabel = PIPELINE_STAGES[0].key;
    item.stagePercent = 0;
    isProcessing = true;
    render();

    document.dispatchEvent(new CustomEvent('queueItemStarted', {
      detail: { id }
    }));
  }

  /**
   * Update progress for a queue item (for manual/local use).
   * @param {number} id
   * @param {number} step - 0-based step index
   * @param {number} percent - 0-100 within the current step
   */
  function updateProgress(id, step, percent) {
    const item = _findItem(id);
    if (!item || item.status !== 'processing') return;

    item.currentStep = Math.min(step, TOTAL_STEPS - 1);
    item.stageLabel = PIPELINE_STAGES[item.currentStep].key;
    item.stagePercent = Math.max(0, Math.min(100, percent));
    render();
  }

  /**
   * Mark a queue item as completed (for manual/local use).
   * @param {number} id
   */
  function completeItem(id) {
    const item = _findItem(id);
    if (!item) return;
    item.status = 'completed';
    item.currentStep = TOTAL_STEPS;
    item.stagePercent = 100;
    _checkProcessingDone();
    render();

    document.dispatchEvent(new CustomEvent('queueItemCompleted', {
      detail: { id }
    }));
  }

  /**
   * Mark a queue item as failed (for manual/local use).
   * @param {number} id
   * @param {string} errorMessage
   */
  function failItem(id, errorMessage) {
    const item = _findItem(id);
    if (!item) return;
    item.status = 'failed';
    item.errorMessage = errorMessage || t('queue_error_unknown');
    _checkProcessingDone();
    render();

    document.dispatchEvent(new CustomEvent('queueItemFailed', {
      detail: { id, error: errorMessage }
    }));
  }

  /**
   * Remove a specific item from the queue (only if not processing).
   * @param {number} id
   */
  async function removeItem(id) {
    const item = _findItem(id);
    if (!item || item.status === 'processing') return;

    // Remove from backend
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        await window.__TAURI__.core.invoke('remove_queue_item', { id });
      } catch (err) {
        console.warn('Failed to remove queue item from backend:', err);
      }
    }

    _failToastShown.delete(id);
    queue = queue.filter(q => q.id !== id);
    render();
  }

  /**
   * Clear all completed and failed items from the queue.
   */
  async function clearCompleted() {
    const toRemove = queue.filter(q => q.status === 'completed' || q.status === 'failed' || q.status === 'skipped');

    // Remove from backend
    if (window.__TAURI__ && window.__TAURI__.core) {
      for (const item of toRemove) {
        try {
          await window.__TAURI__.core.invoke('remove_queue_item', { id: item.id });
        } catch (err) {
          console.warn('Failed to remove queue item:', err);
        }
      }
    }

    // Clean up failure toast tracking for removed items
    toRemove.forEach(item => _failToastShown.delete(item.id));

    queue = queue.filter(q => q.status === 'pending' || q.status === 'processing');
    render();

    document.dispatchEvent(new CustomEvent('queueCleared'));
  }

  /**
   * Get the full queue state (for external consumers).
   * @returns {Array<QueueItem>}
   */
  function getQueue() {
    return queue.map(item => ({ ...item }));
  }

  /**
   * Get the count of items by status.
   * @returns {{ pending: number, processing: number, completed: number, failed: number, total: number }}
   */
  function getCounts() {
    const counts = { pending: 0, processing: 0, completed: 0, failed: 0, skipped: 0, total: queue.length };
    queue.forEach(item => {
      if (counts[item.status] !== undefined) counts[item.status]++;
    });
    return counts;
  }

  /**
   * Retry a failed queue item — resets it to pending and re-queues.
   * @param {number} id
   */
  async function retryItem(id) {
    const item = _findItem(id);
    if (!item || item.status !== 'failed') return;

    // Reset local state
    item.status = 'pending';
    item.currentStep = 0;
    item.stagePercent = 0;
    item.stageLabel = '';
    item.errorMessage = null;
    item._detail = null;
    _failToastShown.delete(id);

    // Reset on backend
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        await window.__TAURI__.core.invoke('retry_queue_item', { id: item.id });
      } catch (err) {
        // If retry command doesn't exist, try remove + re-add
        console.warn('retry_queue_item not available, using remove+add:', err);
        try {
          await window.__TAURI__.core.invoke('remove_queue_item', { id: item.id });
          await window.__TAURI__.core.invoke('add_queue_item', {
            item: {
              id: item.id,
              url: item.url,
              capture_mode: item.captureMode,
              interval_seconds: item.intervalSeconds,
              status: 'pending',
              title: item.title,
              error: null,
              progress: null,
            }
          });
        } catch (innerErr) {
          console.warn('Failed to retry via remove+add:', innerErr);
        }
      }
    }

    render();

    // Auto-start processing if not already running
    _startProcessing();
  }

  // ---- Internal helpers ----

  function _findItem(id) {
    return queue.find(q => q.id === id) || null;
  }

  /**
   * Show a toast notification for a failed queue item (deduped per item ID).
   * @param {number} itemId
   * @param {string|null} title - Video title if available
   * @param {string} errorMessage
   */
  function _showFailureToast(itemId, title, errorMessage) {
    if (_failToastShown.has(itemId)) return;
    _failToastShown.add(itemId);

    const error = errorMessage || t('queue_error_unknown');
    let message;
    if (title) {
      message = t('error_processing_failed', { title, error });
    } else {
      message = t('error_processing_failed_short', { error });
    }

    // Use the global showToast if available, otherwise use our local _showToast
    if (typeof showToast === 'function') {
      showToast(message, 'error');
    } else {
      _showToast(message, 'error');
    }
  }

  function _checkProcessingDone() {
    const hasProcessing = queue.some(q => q.status === 'processing');
    if (!hasProcessing) {
      isProcessing = false;
    }
  }

  /**
   * Start elapsed time tracking for the currently processing item.
   * @param {number} itemId
   */
  function _startElapsedTimer(itemId) {
    _stopElapsedTimer();
    const item = _findItem(itemId);
    if (item) {
      item._startedAt = Date.now();
    }
    _processingStartTime = Date.now();
    // Update elapsed display every second
    _elapsedTimer = setInterval(() => {
      _updateElapsedDisplay();
    }, 1000);
  }

  /**
   * Stop the elapsed time tracking interval.
   */
  function _stopElapsedTimer() {
    if (_elapsedTimer) {
      clearInterval(_elapsedTimer);
      _elapsedTimer = null;
    }
    _processingStartTime = null;
  }

  /**
   * Update just the elapsed time display without full re-render.
   */
  function _updateElapsedDisplay() {
    if (!containerEl) return;
    const el = containerEl.querySelector('.queue-active-elapsed');
    if (!el || !_processingStartTime) return;
    el.textContent = _formatElapsed(Date.now() - _processingStartTime);
  }

  /**
   * Format milliseconds into a human-readable elapsed time string.
   * @param {number} ms
   * @returns {string}
   */
  function _formatElapsed(ms) {
    const totalSec = Math.floor(ms / 1000);
    if (totalSec < 60) return `${totalSec}s`;
    const min = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    if (min < 60) return `${min}m ${sec.toString().padStart(2, '0')}s`;
    const hr = Math.floor(min / 60);
    const remMin = min % 60;
    return `${hr}h ${remMin.toString().padStart(2, '0')}m`;
  }

  /**
   * Get the queue position for pending items (1-based).
   * @param {QueueItem} item
   * @returns {number} position (1-based) or 0 if not pending
   */
  function _getPendingPosition(item) {
    if (item.status !== 'pending') return 0;
    const pendingItems = queue.filter(q => q.status === 'pending');
    return pendingItems.indexOf(item) + 1;
  }

  /**
   * Truncate a URL for display.
   * @param {string} url
   * @param {number} maxLen
   * @returns {string}
   */
  function _truncateUrl(url, maxLen) {
    if (url.length <= maxLen) return url;
    return url.substring(0, maxLen - 3) + '...';
  }

  /**
   * Compute overall progress percentage for the progress bar.
   * @param {QueueItem} item
   * @returns {number} 0-100
   */
  function _overallPercent(item) {
    if (item.status === 'completed') return 100;
    if (item.status === 'failed' || item.status === 'pending') return 0;
    // Each step contributes equally
    const steps = item.totalSteps || TOTAL_STEPS;
    const stepWeight = 100 / steps;
    return Math.round(item.currentStep * stepWeight + (item.stagePercent / 100) * stepWeight);
  }

  /**
   * Build the stage label string like "3/5 steps: Downloading... 45%"
   * @param {QueueItem} item
   * @returns {string}
   */
  function _buildStageText(item) {
    if (item.status === 'completed') return t('queue_status_done');
    if (item.status === 'failed') return item.errorMessage || t('queue_error_unknown');
    if (item.status === 'pending') return t('queue_status_pending');

    const stepNum = item.currentStep + 1;
    const steps = item.totalSteps || TOTAL_STEPS;
    const stageName = t(item.stageLabel);
    const pct = item.stagePercent;
    const detail = item._detail ? ` (${item._detail})` : '';
    return `${stepNum}/${steps} ${t('queue_steps')}: ${stageName}${pct > 0 ? '... ' + pct + '%' : ''}${detail}`;
  }

  // ---- Render throttling ----
  let _renderScheduled = false;

  /**
   * Schedule a render on the next animation frame (debounce rapid updates).
   * Immediate render if called the first time without a pending frame.
   */
  function _scheduleRender() {
    if (_renderScheduled) return;
    _renderScheduled = true;
    requestAnimationFrame(() => {
      _renderScheduled = false;
      _doRender();
    });
  }

  /**
   * Public render entry point — schedules a batched render.
   */
  function render() {
    _scheduleRender();
  }

  /**
   * Actual render implementation.
   */
  function _doRender() {
    if (!containerEl) return;

    if (queue.length === 0) {
      containerEl.innerHTML = '';
      containerEl.classList.remove('queue-visible');
      return;
    }

    containerEl.classList.add('queue-visible');

    const counts = getCounts();
    const hasCompleted = counts.completed > 0 || counts.failed > 0 || counts.skipped > 0;

    // Find the currently processing item
    const processingItem = queue.find(q => q.status === 'processing');

    let html = '<div class="queue-panel">';

    // Queue header with status badges
    html += '<div class="queue-header">';
    html += `<h3 class="queue-title" data-i18n="queue_title">${t('queue_title')}</h3>`;
    html += '<div class="queue-header-actions">';

    // Status summary badges
    html += '<div class="queue-badges">';
    if (counts.processing > 0) {
      html += `<span class="queue-badge queue-badge-active">${STATUS_ICONS.processing} ${counts.processing}</span>`;
    }
    if (counts.pending > 0) {
      html += `<span class="queue-badge queue-badge-pending">${counts.pending} ${t('queue_status_pending')}</span>`;
    }
    if (counts.completed > 0) {
      html += `<span class="queue-badge queue-badge-done">${counts.completed} ${t('queue_status_done')}</span>`;
    }
    if (counts.failed > 0) {
      html += `<span class="queue-badge queue-badge-error">${counts.failed} ${t('queue_status_failed')}</span>`;
    }
    if (counts.skipped > 0) {
      html += `<span class="queue-badge queue-badge-skipped">${counts.skipped} ${t('queue_status_skipped')}</span>`;
    }
    html += '</div>';

    if (hasCompleted) {
      html += `<button class="queue-clear-btn" data-action="clear-completed" data-i18n="queue_clear">${t('queue_clear')}</button>`;
    }
    html += '</div></div>';

    // Active processing section with multi-segment progress bar
    if (processingItem) {
      const overallPct = _overallPercent(processingItem);
      const displayName = processingItem.title || _truncateUrl(processingItem.url, 60);
      const steps = processingItem.totalSteps || TOTAL_STEPS;
      const stepNum = processingItem.currentStep + 1;
      const stageName = t(processingItem.stageLabel || PIPELINE_STAGES[0].key);
      const stagePct = processingItem.stagePercent || 0;

      html += '<div class="queue-active">';
      html += '<div class="queue-active-header">';
      html += `<span class="queue-active-url" title="${_escapeAttr(processingItem.url)}">${_escapeHtml(displayName)}</span>`;
      html += '<div class="queue-active-stats">';
      // Elapsed time
      const elapsed = _processingStartTime ? _formatElapsed(Date.now() - _processingStartTime) : '';
      if (elapsed) {
        html += `<span class="queue-active-elapsed" title="${t('queue_elapsed')}">${elapsed}</span>`;
      }
      html += `<span class="queue-active-pct-badge">${overallPct}%</span>`;
      html += '</div>';
      html += '</div>';

      // Stage info row: step badge + stage name + stage percentage
      html += '<div class="queue-stage-info">';
      html += `<span class="queue-stage-step">${stepNum}/${steps}</span>`;
      html += `<span class="queue-stage-name">${_escapeHtml(stageName)}</span>`;
      const detail = processingItem._detail ? ` (${_escapeHtml(processingItem._detail)})` : '';
      html += `<span class="queue-stage-pct">${stagePct}%${detail}</span>`;
      html += '</div>';

      // Multi-segment progress bar — one segment per pipeline stage
      html += '<div class="queue-segments">';
      for (let i = 0; i < steps; i++) {
        let segClass = 'queue-segment';
        let segWidth = 0;
        if (i < processingItem.currentStep) {
          // Completed stage
          segClass += ' queue-segment-done';
          segWidth = 100;
        } else if (i === processingItem.currentStep) {
          // Current stage
          segClass += ' queue-segment-active';
          segWidth = processingItem.stagePercent;
        }
        // else: pending stage, segWidth stays 0

        html += `<div class="${segClass}">`;
        html += `<div class="queue-segment-fill" style="width: ${segWidth}%"></div>`;
        html += '</div>';
      }
      html += '</div>';

      html += '</div>';
    }

    // Queue list
    html += '<div class="queue-list">';
    queue.forEach((item, idx) => {
      const isActive = item.status === 'processing';
      const statusClass = item.status === 'completed' ? 'queue-item-done'
        : item.status === 'failed' ? 'queue-item-error'
        : item.status === 'skipped' ? 'queue-item-skipped'
        : item.status === 'processing' ? 'queue-item-active'
        : '';
      const icon = STATUS_ICONS[item.status] || STATUS_ICONS.pending;
      const displayName = item.title || _truncateUrl(item.url, 50);

      html += `<div class="queue-item ${statusClass} queue-item-enter" data-id="${item.id}" style="animation-delay: ${idx * 30}ms">`;
      html += `<span class="queue-item-icon">${icon}</span>`;
      html += '<div class="queue-item-info">';
      html += `<span class="queue-item-url" title="${_escapeAttr(item.url)}">${_escapeHtml(displayName)}</span>`;

      if (item.status === 'processing') {
        const pct = _overallPercent(item);
        html += `<span class="queue-item-progress-text">${pct}%</span>`;
      } else if (item.status === 'failed') {
        html += `<span class="queue-item-error-text" title="${_escapeAttr(item.errorMessage || '')}">${_escapeHtml(item.errorMessage || t('queue_error_unknown'))}</span>`;
      } else if (item.status === 'skipped') {
        html += `<span class="queue-item-skipped-text" title="${_escapeAttr(item.errorMessage || '')}">${t('error_already_exists')}</span>`;
      } else if (item.status === 'completed') {
        html += `<span class="queue-item-done-text">${t('queue_status_done')}</span>`;
      } else {
        // Show queue position for pending items with badge
        const pos = _getPendingPosition(item);
        if (pos > 0) {
          html += `<span class="queue-position-badge"><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>${t('queue_position', { n: pos })}</span>`;
        } else {
          html += `<span class="queue-item-pending-text">${t('queue_status_pending')}</span>`;
        }
      }

      html += '</div>';

      // Action buttons for non-processing items
      html += '<div class="queue-item-actions">';
      if (item.status === 'failed') {
        // Retry button for failed items
        html += `<button class="queue-item-retry" data-action="retry" data-id="${item.id}" title="${t('queue_retry')}">`;
        html += '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>';
        html += '</button>';
      }
      if (item.status !== 'processing') {
        html += `<button class="queue-item-remove" data-action="remove" data-id="${item.id}" title="${t('queue_remove')}">`;
        html += '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>';
        html += '</button>';
      }
      html += '</div>';

      html += '</div>';
    });
    html += '</div>';

    html += '</div>';

    containerEl.innerHTML = html;

    // Bind click events
    containerEl.querySelectorAll('[data-action="remove"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const id = parseInt(btn.getAttribute('data-id'), 10);
        removeItem(id);
      });
    });

    containerEl.querySelectorAll('[data-action="retry"]').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const id = parseInt(btn.getAttribute('data-id'), 10);
        retryItem(id);
      });
    });

    containerEl.querySelectorAll('[data-action="clear-completed"]').forEach(btn => {
      btn.addEventListener('click', () => clearCompleted());
    });
  }

  function _escapeHtml(str) {
    if (!str) return '';
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  function _escapeAttr(str) {
    if (!str) return '';
    return str.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/'/g, '&#39;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  /**
   * Show a toast notification.
   */
  function _showToast(message, type) {
    let toast = document.querySelector('.toast');
    if (!toast) {
      toast = document.createElement('div');
      toast.className = 'toast';
      document.body.appendChild(toast);
    }
    toast.textContent = message;
    toast.className = `toast toast-${type || 'success'}`;
    requestAnimationFrame(() => {
      toast.classList.add('toast-show');
      setTimeout(() => toast.classList.remove('toast-show'), 3000);
    });
  }

  return {
    init,
    addItem,
    startItem,
    updateProgress,
    completeItem,
    failItem,
    removeItem,
    retryItem,
    clearCompleted,
    getQueue,
    getCounts,
    render,
    PIPELINE_STAGES,
  };
})();
