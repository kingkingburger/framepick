/**
 * @file state.js
 * @description FramePick 앱의 글로벌 상태 관리 (AppState 싱글턴)
 *
 * URL 입력, 캡쳐 모드, 큐 항목에 대한 중앙 상태 저장소.
 * 간단한 pub/sub 패턴을 통해 반응형 업데이트를 제공하여
 * UI 컴포넌트와 Tauri 백엔드가 동기화 상태를 유지한다.
 *
 * 공개 API:
 *  - on/off: 상태 키 변경 구독/해제
 *  - get/getAll: 상태값 조회
 *  - setUrl/setCaptureMode/setIntervalSeconds/setLanguage: 상태 설정
 *  - addToQueue/updateQueueItem/removeQueueItem/clearFinishedItems: 큐 관리
 *  - isValidYouTubeUrl: URL 유효성 검사
 *  - buildPipelineInput: 백엔드 파이프라인 입력 페이로드 생성
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
   * 처리 큐에 새 항목을 추가한다.
   * 생성된 큐 항목을 반환하며, URL이 비어있으면 null을 반환한다.
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
   * 다운로드/캡쳐 파이프라인의 입력 페이로드 객체를 생성한다.
   * Tauri 백엔드로 전송되는 데이터이다.
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
