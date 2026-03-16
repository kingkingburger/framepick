/**
 * FramePick - Application State Management
 *
 * Central state store for input fields and queue items.
 * Provides reactive updates via a simple pub/sub pattern so that
 * UI components and the Tauri backend stay in sync.
 */

const AppState = (() => {
  // ─── Internal state ──────────────────────────────────────────
  const _state = {
    /** Current YouTube URL typed by the user */
    url: '',

    /** Capture mode: 'subtitle' | 'scene' | 'interval' */
    captureMode: 'subtitle',

    /** Interval in seconds when captureMode === 'interval' */
    intervalSeconds: 10,

    /** Current UI language: 'ko' | 'en' */
    language: 'ko',

    /**
     * Queue of items to process.
     * Each entry: { id, url, captureMode, intervalSeconds, status, title?, error? }
     * status: 'pending' | 'processing' | 'done' | 'error'
     */
    queue: [],

    /**
     * Currently displayed capture frames result.
     * Null when no video is selected for viewing.
     * Shape: { videoId, title, frameCount, frames: [{index, image, timestamp, text, thumbnail_url}] }
     */
    captureFrames: null,

    /** ID of the video whose capture frames are currently shown */
    activeVideoId: null,

    /** View mode for capture list: 'grid' | 'list' */
    captureViewMode: 'grid',
  };

  // ─── Listeners (key -> Set<fn>) ──────────────────────────────
  const _listeners = {};

  /** Subscribe to changes on a specific state key. */
  function on(key, fn) {
    if (!_listeners[key]) _listeners[key] = new Set();
    _listeners[key].add(fn);
  }

  /** Unsubscribe a listener. */
  function off(key, fn) {
    if (_listeners[key]) _listeners[key].delete(fn);
  }

  /** Notify all listeners for a key with the new value. */
  function _notify(key) {
    const fns = _listeners[key];
    if (!fns) return;
    const val = _state[key];
    fns.forEach((fn) => {
      try { fn(val); } catch (e) { console.error(`[state] listener error for "${key}":`, e); }
    });
  }

  // ─── Getters ─────────────────────────────────────────────────

  /** Return a shallow copy of the current state. */
  function getAll() {
    return { ..._state, queue: [..._state.queue] };
  }

  function get(key) {
    return _state[key];
  }

  // ─── Setters ─────────────────────────────────────────────────

  function setUrl(value) {
    const v = (value || '').trim();
    if (_state.url === v) return;
    _state.url = v;
    _notify('url');
  }

  function setCaptureMode(mode) {
    if (!['subtitle', 'scene', 'interval'].includes(mode)) return;
    if (_state.captureMode === mode) return;
    _state.captureMode = mode;
    _notify('captureMode');
  }

  function setIntervalSeconds(seconds) {
    const s = Number(seconds);
    if (isNaN(s) || s < 1 || s > 3600) return;
    if (_state.intervalSeconds === s) return;
    _state.intervalSeconds = s;
    _notify('intervalSeconds');
  }

  function setLanguage(lang) {
    if (!['ko', 'en'].includes(lang)) return;
    if (_state.language === lang) return;
    _state.language = lang;
    _notify('language');
  }

  // ─── Queue operations ────────────────────────────────────────

  let _nextId = 1;

  /**
   * Add a new item to the processing queue.
   * Returns the created queue item or null if the URL is invalid.
   */
  function addToQueue(url, captureMode, intervalSeconds) {
    if (!url) return null;
    const item = {
      id: _nextId++,
      url,
      captureMode: captureMode || _state.captureMode,
      intervalSeconds: intervalSeconds || _state.intervalSeconds,
      status: 'pending',
      title: null,
      error: null,
      /** @type {null|{stage: string, stageNumber: number, totalStages: number, percent: number, detail: string|null}} */
      progress: null,
      /** @type {number} Timestamp when the item was added */
      addedAt: Date.now(),
      /** @type {number|null} Timestamp when processing started */
      startedAt: null,
      /** @type {number|null} Timestamp when processing finished */
      finishedAt: null,
      /** @type {string|null} Path to generated slides.html (set on completion) */
      slidesPath: null,
    };
    _state.queue.push(item);
    _notify('queue');
    return item;
  }

  /** Update a queue item by id (partial merge). */
  function updateQueueItem(id, updates) {
    const item = _state.queue.find((q) => q.id === id);
    if (!item) return;
    Object.assign(item, updates);
    _notify('queue');
  }

  /**
   * Update pipeline progress for a queue item.
   * @param {number} id Queue item ID
   * @param {{stage: string, stage_number: number, total_stages: number, percent: number, detail?: string}} progressData
   */
  function updateQueueItemProgress(id, progressData) {
    const item = _state.queue.find((q) => q.id === id);
    if (!item) return;
    item.progress = {
      stage: progressData.stage,
      stageNumber: progressData.stage_number,
      totalStages: progressData.total_stages,
      percent: progressData.percent,
      detail: progressData.detail || null,
    };
    // Also ensure status is 'processing' when we get progress events
    if (item.status === 'pending') {
      item.status = 'processing';
      item.startedAt = item.startedAt || Date.now();
    }
    _notify('queue');
  }

  /** Remove a queue item by id. */
  function removeQueueItem(id) {
    _state.queue = _state.queue.filter((q) => q.id !== id);
    _notify('queue');
  }

  /** Remove all completed/done and skipped items from the queue. */
  function clearCompletedItems() {
    const before = _state.queue.length;
    _state.queue = _state.queue.filter(
      (q) => q.status !== 'completed' && q.status !== 'done' && q.status !== 'skipped'
    );
    if (_state.queue.length !== before) {
      _notify('queue');
    }
  }

  /** Get summary stats for the queue. */
  function getQueueStats() {
    const q = _state.queue;
    return {
      total: q.length,
      pending: q.filter((i) => i.status === 'pending').length,
      processing: q.filter((i) => i.status === 'processing').length,
      completed: q.filter((i) => i.status === 'completed' || i.status === 'done').length,
      failed: q.filter((i) => i.status === 'failed' || i.status === 'error').length,
      skipped: q.filter((i) => i.status === 'skipped').length,
    };
  }

  // ─── Validation helpers ──────────────────────────────────────

  const YT_REGEX = /^https?:\/\/(?:www\.)?(?:youtube\.com\/(?:watch\?v=|shorts\/|embed\/)|youtu\.be\/)[\w-]+/;
  const YT_PLAYLIST_REGEX = /^https?:\/\/(?:www\.)?youtube\.com\/playlist\?list=[\w-]+/;

  /** Validate a YouTube URL (single video or playlist). Returns true if valid. */
  function isValidYouTubeUrl(url) {
    const trimmed = (url || '').trim();
    return YT_REGEX.test(trimmed) || YT_PLAYLIST_REGEX.test(trimmed) || isPlaylistUrl(trimmed);
  }

  /**
   * Check if a URL contains a YouTube playlist parameter.
   * Returns true for URLs with list= parameter on youtube.com/youtu.be domains.
   */
  function isPlaylistUrl(url) {
    const trimmed = (url || '').trim();
    const isYoutube = trimmed.includes('youtube.com') || trimmed.includes('youtu.be');
    return isYoutube && /[?&]list=[\w-]+/.test(trimmed);
  }

  /**
   * Build the input payload object for the download/capture pipeline.
   * This is what gets sent to the Tauri backend.
   */
  function buildPipelineInput() {
    return {
      url: _state.url,
      capture_mode: _state.captureMode,
      interval_seconds: _state.intervalSeconds,
    };
  }

  // ─── Capture frames state ─────────────────────────────────────────

  /**
   * Set the capture frames result for display.
   * @param {null|{videoId: string, title: string, frameCount: number, frames: Array}} result
   */
  function setCaptureFrames(result) {
    _state.captureFrames = result;
    _state.activeVideoId = result ? result.videoId || result.video_id : null;
    _notify('captureFrames');
  }

  /** Clear the active capture frames (go back to queue/library view). */
  function clearCaptureFrames() {
    _state.captureFrames = null;
    _state.activeVideoId = null;
    _notify('captureFrames');
  }

  /** Set the capture list view mode. */
  function setCaptureViewMode(mode) {
    if (mode !== 'grid' && mode !== 'list') return;
    if (_state.captureViewMode === mode) return;
    _state.captureViewMode = mode;
    _notify('captureViewMode');
  }

    // ─── Public API ──────────────────────────────────────────────

  return {
    on,
    off,
    get,
    getAll,
    setUrl,
    setCaptureMode,
    setIntervalSeconds,
    setLanguage,
    addToQueue,
    updateQueueItem,
    updateQueueItemProgress,
    removeQueueItem,
    clearCompletedItems,
    getQueueStats,
    isValidYouTubeUrl,
    isPlaylistUrl,
    buildPipelineInput,
    setCaptureFrames,
    clearCaptureFrames,
    setCaptureViewMode,
  };
})();
