/**
 * progress.js - Stage-based pipeline progress tracking
 *
 * Listens for Tauri `pipeline:progress` and `pipeline:error` events,
 * updates the queue UI with stage info and progress bars, and dispatches
 * DOM events for other modules to react to.
 */
const PipelineProgress = (() => {
  /** Map of queue_id → latest progress payload */
  const _state = new Map();

  /**
   * Initialize the progress listener.
   * Must be called after DOMContentLoaded and after Tauri is available.
   */
  function init() {
    _listenTauriEvents();
  }

  /**
   * Get the current progress state for a queue item.
   * @param {number} queueId
   * @returns {object|null} Latest progress payload or null
   */
  function getProgress(queueId) {
    return _state.get(queueId) || null;
  }

  /**
   * Get all tracked progress states.
   * @returns {Map}
   */
  function getAllProgress() {
    return new Map(_state);
  }

  /**
   * Clear progress state for a queue item (e.g., after removal).
   * @param {number} queueId
   */
  function clearProgress(queueId) {
    _state.delete(queueId);
  }

  /**
   * Format a progress payload into a human-readable status string.
   * Uses i18n translation for stage names.
   * @param {object} payload - ProgressPayload from backend
   * @returns {string}
   */
  function formatStatus(payload) {
    if (!payload) return '';

    const stageName = t(payload.stage_i18n_key || _stageToI18nKey(payload.stage));
    const stageInfo = `[${payload.stage_number}/${payload.total_stages}]`;
    const pct = `${payload.percent}%`;
    const detail = payload.detail ? ` - ${payload.detail}` : '';

    return `${stageInfo} ${stageName} ${pct}${detail}`;
  }

  /**
   * Render a progress bar element for a queue item.
   * Creates or updates a progress bar inside the given container.
   * @param {HTMLElement} container - DOM element to render into
   * @param {object} payload - ProgressPayload from backend
   */
  function renderProgressBar(container, payload) {
    if (!container || !payload) return;

    let bar = container.querySelector('.progress-bar');
    let label = container.querySelector('.progress-label');
    let stageLabel = container.querySelector('.progress-stage');

    if (!bar) {
      // Create progress UI structure with stage info row
      container.innerHTML = `
        <div class="queue-stage-info">
          <span class="queue-stage-step progress-step-badge"></span>
          <span class="queue-stage-name progress-stage"></span>
          <span class="queue-stage-pct progress-label"></span>
        </div>
        <div class="progress-track">
          <div class="progress-bar"></div>
        </div>
      `;
      bar = container.querySelector('.progress-bar');
      label = container.querySelector('.progress-label');
      stageLabel = container.querySelector('.progress-stage');
    }

    // Calculate overall progress across all stages
    const overallPercent = _calculateOverallPercent(payload);
    bar.style.width = `${overallPercent}%`;

    // Step badge
    const stepBadge = container.querySelector('.progress-step-badge');
    if (stepBadge) {
      stepBadge.textContent = `${payload.stage_number}/${payload.total_stages}`;
    }

    // Stage label (name)
    const stageName = t(_stageToI18nKey(payload.stage));
    stageLabel.textContent = stageName;

    // Percentage label with detail
    const detail = payload.detail ? ` (${payload.detail})` : '';
    label.textContent = `${payload.percent}%${detail}`;

    // Update aria attributes
    const track = container.querySelector('.progress-track');
    if (track) {
      track.setAttribute('role', 'progressbar');
      track.setAttribute('aria-valuenow', overallPercent);
      track.setAttribute('aria-valuemin', 0);
      track.setAttribute('aria-valuemax', 100);
    }

    // Done state
    if (payload.stage === 'done') {
      bar.style.width = '100%';
      bar.style.animation = 'none';
      stageLabel.textContent = t('progress_done');
      label.textContent = '100%';
      const stepBadgeEl = container.querySelector('.progress-step-badge');
      if (stepBadgeEl) stepBadgeEl.textContent = `${payload.total_stages}/${payload.total_stages}`;
      container.classList.add('progress-complete');
    } else {
      container.classList.remove('progress-complete');
    }
  }

  /**
   * Render an error state for a queue item.
   * @param {HTMLElement} container - DOM element to render into
   * @param {object} errorPayload - ErrorPayload from backend
   */
  function renderError(container, errorPayload) {
    if (!container || !errorPayload) return;

    const stageName = t(_stageToI18nKey(errorPayload.stage));
    container.innerHTML = `
      <div class="progress-error">
        <span class="progress-error-icon">&#9888;</span>
        <span class="progress-error-text">${_escapeHtml(stageName)}: ${_escapeHtml(errorPayload.message)}</span>
      </div>
    `;
    container.classList.add('progress-failed');
    container.classList.remove('progress-complete');
  }

  // ── Private helpers ──────────────────────────────────────────────

  function _listenTauriEvents() {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
      console.warn('PipelineProgress: Tauri event API not available');
      return;
    }

    const { listen } = window.__TAURI__.event;

    // Listen for progress updates
    listen('pipeline:progress', (event) => {
      const payload = event.payload;
      if (!payload || payload.queue_id == null) return;

      _state.set(payload.queue_id, payload);

      // Dispatch a DOM event so other modules (queue UI) can react
      document.dispatchEvent(new CustomEvent('pipelineProgress', {
        detail: payload
      }));
    });

    // Listen for error events
    listen('pipeline:error', (event) => {
      const payload = event.payload;
      if (!payload || payload.queue_id == null) return;

      // Store error state
      _state.set(payload.queue_id, {
        ...(_state.get(payload.queue_id) || {}),
        error: payload.message,
        errorStage: payload.stage
      });

      document.dispatchEvent(new CustomEvent('pipelineError', {
        detail: payload
      }));
    });
  }

  /**
   * Calculate overall pipeline progress (0-100) across all stages.
   * Each stage contributes an equal share of the total.
   */
  function _calculateOverallPercent(payload) {
    const { stage_number, total_stages, percent } = payload;
    if (total_stages === 0) return 0;
    // Completed stages contribute fully, current stage contributes proportionally
    const completedStages = stage_number - 1;
    const stageShare = 100 / total_stages;
    return Math.round(completedStages * stageShare + (percent / 100) * stageShare);
  }

  /**
   * Map a stage enum value (snake_case string from backend) to its i18n key.
   */
  function _stageToI18nKey(stage) {
    const map = {
      'downloading': 'progress_downloading',
      'extracting_subtitles': 'progress_extracting_subtitles',
      'extracting_frames': 'progress_extracting_frames',
      'generating_slides': 'progress_generating_slides',
      'cleanup': 'progress_cleanup',
      'done': 'progress_done'
    };
    return map[stage] || stage;
  }

  function _escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
  }

  return {
    init,
    getProgress,
    getAllProgress,
    clearProgress,
    formatStatus,
    renderProgressBar,
    renderError
  };
})();
