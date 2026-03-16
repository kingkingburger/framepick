/**
 * FramePick - Application State Management
 *
 * Central state store for URL input, capture mode, and queue items.
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
    intervalSeconds: 30,

    /** Current UI language: 'ko' | 'en' */
    language: 'ko',

    /**
     * Queue of items to process.
     * Each entry: { id, url, captureMode, intervalSeconds, status, title?, error?, progress? }
     * status: 'pending' | 'downloading' | 'capturing' | 'done' | 'error'
     */
    queue: [],
  };

  // ─── Listeners (key -> Set<fn>) ──────────────────────────────
  const _listeners = {};

  /** Subscribe to changes on a specific state key. Returns unsubscribe function. */
  function on(key, fn) {
    if (!_listeners[key]) _listeners[key] = new Set();
    _listeners[key].add(fn);
    return () => _listeners[key].delete(fn);
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

  /** Get a specific state value by key. */
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
   * Returns the created queue item or null if URL is empty.
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
      progress: 0,
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

  /** Remove a queue item by id. */
  function removeQueueItem(id) {
    _state.queue = _state.queue.filter((q) => q.id !== id);
    _notify('queue');
  }

  /** Clear all completed or errored items from the queue. */
  function clearFinishedItems() {
    _state.queue = _state.queue.filter((q) => q.status === 'pending' || q.status === 'downloading' || q.status === 'capturing');
    _notify('queue');
  }

  // ─── Validation helpers ──────────────────────────────────────

  const YT_REGEX = /^https?:\/\/(?:www\.)?(?:youtube\.com\/(?:watch\?v=|shorts\/|embed\/)|youtu\.be\/)[\w-]+/;

  /** Validate a YouTube URL. Returns true if valid. */
  function isValidYouTubeUrl(url) {
    return YT_REGEX.test((url || '').trim());
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
    removeQueueItem,
    clearFinishedItems,
    isValidYouTubeUrl,
    buildPipelineInput,
  };
})();
