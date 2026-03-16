/**
 * FramePick - URL Input Component
 *
 * Encapsulated URL input with form validation and submit handler.
 * Emits download requests to the parent dashboard via callback events.
 *
 * Usage:
 *   const urlInput = UrlInputComponent.init({
 *     onSubmit: (item) => { ... },         // Called when a valid URL is submitted to queue
 *     onValidationError: (msg) => { ... }, // Called on validation failure (optional)
 *   });
 */

const UrlInputComponent = (() => {
  // ─── DOM element references (bound on init) ──────────────
  let _elInput = null;
  let _elError = null;
  let _elSubmitBtn = null;

  // ─── Callbacks ───────────────────────────────────────────
  let _onSubmit = null;
  let _onValidationError = null;
  let _onPlaylistDetected = null;

  // ─── State ───────────────────────────────────────────────
  let _isSubmitting = false;

  // ─── YouTube URL patterns ────────────────────────────────
  // Client-side regex covering: watch, shorts, embed, youtu.be, playlist
  const YT_VIDEO_REGEX = /^https?:\/\/(?:www\.)?(?:youtube\.com\/(?:watch\?[^\s]*v=[\w-]{11}|shorts\/[\w-]{11}|embed\/[\w-]{11})|youtu\.be\/[\w-]{11})/;
  const YT_PLAYLIST_REGEX = /^https?:\/\/(?:www\.)?youtube\.com\/(?:playlist\?|watch\?[^\s]*list=)[\w-]+/;

  /**
   * Validate a URL as a YouTube video or playlist URL (client-side).
   * @param {string} url
   * @returns {{valid: boolean, type: 'video'|'playlist'|null}}
   */
  function validateUrl(url) {
    const trimmed = (url || '').trim();
    if (!trimmed) {
      return { valid: false, type: null };
    }
    if (YT_VIDEO_REGEX.test(trimmed)) {
      return { valid: true, type: 'video' };
    }
    if (YT_PLAYLIST_REGEX.test(trimmed)) {
      return { valid: true, type: 'playlist' };
    }
    // Fallback: also accept if AppState's broader regex matches
    if (AppState.isValidYouTubeUrl(trimmed)) {
      return { valid: true, type: 'video' };
    }
    return { valid: false, type: null };
  }

  /**
   * Normalize a YouTube URL (trim whitespace, strip tracking params).
   * @param {string} url
   * @returns {string}
   */
  function normalizeUrl(url) {
    let normalized = (url || '').trim();
    // Remove common tracking parameters while preserving v= and list=
    try {
      const urlObj = new URL(normalized);
      const keepParams = ['v', 'list', 't'];
      const params = new URLSearchParams();
      for (const key of keepParams) {
        if (urlObj.searchParams.has(key)) {
          params.set(key, urlObj.searchParams.get(key));
        }
      }
      // Only reconstruct if it's a youtube.com URL with query params
      if (urlObj.hostname.includes('youtube.com') && urlObj.searchParams.toString()) {
        urlObj.search = params.toString();
        normalized = urlObj.toString();
      }
    } catch (_) {
      // URL parsing failed, return as-is
    }
    return normalized;
  }

  /**
   * Show a validation error on the input.
   * @param {string} message
   */
  function showError(message) {
    if (_elError) {
      _elError.textContent = message;
      _elError.hidden = false;
    }
    if (_elInput) {
      _elInput.classList.add('invalid');
    }
    if (_onValidationError) {
      _onValidationError(message);
    }
  }

  /**
   * Show a toast notification.
   * @param {string} message
   * @param {'success'|'error'|'warning'} [type='warning']
   * @param {number} [duration=3500] - ms before auto-dismiss
   */
  function showToast(message, type, duration) {
    type = type || 'warning';
    duration = duration || 3500;
    const container = document.getElementById('toast-container');
    if (!container) return;
    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    toast.textContent = message;
    container.appendChild(toast);
    // Trigger reflow then show
    requestAnimationFrame(() => {
      toast.classList.add('toast-show');
    });
    setTimeout(() => {
      toast.classList.remove('toast-show');
      setTimeout(() => toast.remove(), 350);
    }, duration);
  }

  /**
   * Clear any visible validation error.
   */
  function clearError() {
    if (_elError) {
      _elError.hidden = true;
      _elError.textContent = '';
    }
    if (_elInput) {
      _elInput.classList.remove('invalid');
    }
  }

  /**
   * Set the submit button to loading/disabled state.
   * @param {boolean} loading
   */
  function setLoading(loading) {
    _isSubmitting = loading;
    if (_elSubmitBtn) {
      _elSubmitBtn.disabled = loading;
      if (loading) {
        _elSubmitBtn.dataset.originalText = _elSubmitBtn.textContent;
        _elSubmitBtn.classList.add('btn-loading');
      } else {
        _elSubmitBtn.classList.remove('btn-loading');
        // Restore original text (i18n will handle on next cycle)
        if (_elSubmitBtn.dataset.originalText) {
          _elSubmitBtn.textContent = _elSubmitBtn.dataset.originalText;
          delete _elSubmitBtn.dataset.originalText;
        }
      }
    }
  }

  /**
   * Invoke a Tauri backend command (with fallback for dev mode).
   */
  function invoke(cmd, args) {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    console.log('[url-input][dev] invoke:', cmd, args);
    return Promise.resolve(null);
  }

  /**
   * Handle form submission: validate, deduplicate, and emit to parent.
   * This is the core submit handler.
   */
  async function handleSubmit() {
    if (_isSubmitting) return;

    const rawUrl = AppState.get('url');
    const url = normalizeUrl(rawUrl);

    // ── Step 1: Client-side validation ──
    const clientResult = validateUrl(url);
    if (!clientResult.valid) {
      showError(I18n.t('errorInvalidUrl'));
      focusInput();
      return;
    }

    // ── Step 1b: Playlist detection — if URL is a playlist, delegate to playlist handler ──
    if (clientResult.type === 'playlist' || AppState.isPlaylistUrl(url)) {
      if (_onPlaylistDetected) {
        resetInput();
        _onPlaylistDetected(url);
        return; // Playlist dialog handles the rest
      }
      // If no playlist handler registered, fall through to single-video logic
    }

    // ── Step 2: Check duplicates in local queue ──
    const queue = AppState.get('queue');
    const isDuplicate = queue.some(
      (q) => (q.status === 'pending' || q.status === 'processing') && q.url === url
    );
    if (isDuplicate) {
      showError(I18n.t('errorDuplicate'));
      focusInput();
      return;
    }

    // ── Step 3: Backend validation (extract video ID, verify URL) ──
    setLoading(true);
    let videoId = null;

    try {
      const result = await invoke('validate_youtube_url', { url });
      if (result && !result.valid) {
        showError(result.error || I18n.t('errorInvalidUrl'));
        setLoading(false);
        focusInput();
        return;
      }
      if (result) {
        videoId = result.video_id;
      }
    } catch (e) {
      // Backend unavailable (dev mode) — proceed with client-side result
      console.warn('[url-input] Backend validation unavailable:', e);
    }

    // ── Step 4: Cross-format duplicate detection via video ID ──
    if (videoId) {
      const crossDuplicate = queue.some((q) => {
        if (q.status !== 'pending' && q.status !== 'processing') return false;
        if (q.videoId === videoId) return true;
        return false;
      });
      if (crossDuplicate) {
        showError(I18n.t('errorDuplicate'));
        setLoading(false);
        focusInput();
        return;
      }

      // ── Step 4b: Check if video already exists in library ──
      try {
        const existsInLibrary = await invoke('check_video_exists', { videoId });
        if (existsInLibrary) {
          showError(I18n.t('errorDuplicateLibrary'));
          showToast(I18n.t('errorDuplicateLibrary'), 'warning');
          setLoading(false);
          focusInput();
          return;
        }
      } catch (e) {
        // Backend unavailable — proceed without library check
        console.warn('[url-input] Library duplicate check unavailable:', e);
      }
    }

    // ── Step 5: Create queue item and emit to parent ──
    const captureMode = AppState.get('captureMode');
    const intervalSeconds = AppState.get('intervalSeconds');

    const item = AppState.addToQueue(url, captureMode, intervalSeconds);
    if (!item) {
      showError(I18n.t('errorInvalidUrl'));
      setLoading(false);
      return;
    }

    if (videoId) {
      AppState.updateQueueItem(item.id, { videoId });
    }

    // ── Step 6: Notify backend ──
    try {
      await invoke('add_queue_item', {
        item: {
          id: item.id,
          url: item.url,
          capture_mode: item.captureMode,
          interval_seconds: item.intervalSeconds,
          status: 'pending',
          title: null,
          error: null,
        },
      });
    } catch (e) {
      // If backend says duplicate, remove from local queue and notify user
      const errStr = String(e);
      if (errStr.includes('already in the queue')) {
        AppState.removeQueueItem(item.id);
        showError(I18n.t('errorDuplicate'));
        showToast(I18n.t('errorDuplicate'), 'warning');
        setLoading(false);
        focusInput();
        return;
      }
      console.warn('[url-input] Backend add_queue_item error:', e);
      // Item remains in local state; backend may not be available
    }

    // ── Step 7: Emit success to parent callback ──
    if (_onSubmit) {
      _onSubmit({
        id: item.id,
        url: item.url,
        captureMode: item.captureMode,
        intervalSeconds: item.intervalSeconds,
        videoId: videoId || null,
        urlType: clientResult.type,
      });
    }

    // ── Step 8: Reset input for next entry ──
    resetInput();
    setLoading(false);
    focusInput();
  }

  /**
   * Clear the input field and state.
   */
  function resetInput() {
    if (_elInput) {
      _elInput.value = '';
    }
    AppState.setUrl('');
    clearError();
  }

  /**
   * Focus the URL input field.
   */
  function focusInput() {
    if (_elInput) {
      _elInput.focus();
    }
  }

  /**
   * Get the current URL value from the input.
   * @returns {string}
   */
  function getValue() {
    return _elInput ? _elInput.value.trim() : '';
  }

  /**
   * Programmatically set the URL input value.
   * @param {string} url
   */
  function setValue(url) {
    if (_elInput) {
      _elInput.value = url;
    }
    AppState.setUrl(url);
    clearError();
  }

  // ─── Event wiring ────────────────────────────────────────

  /**
   * Bind DOM event listeners for the URL input component.
   */
  function _bindEvents() {
    if (!_elInput) return;

    // Real-time input: sync to state & clear errors
    _elInput.addEventListener('input', (e) => {
      AppState.setUrl(e.target.value);
      clearError();
    });

    // Handle paste (value available after microtask)
    _elInput.addEventListener('paste', () => {
      setTimeout(() => {
        AppState.setUrl(_elInput.value);
        clearError();
      }, 0);
    });

    // Enter key submits
    _elInput.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        handleSubmit();
      }
    });

    // Submit button click
    if (_elSubmitBtn) {
      _elSubmitBtn.addEventListener('click', () => {
        handleSubmit();
      });
    }
  }

  // ─── Public API ──────────────────────────────────────────

  /**
   * Initialize the URL input component.
   * @param {Object} options
   * @param {Function} [options.onSubmit]             - Called with queue item data on successful submit
   * @param {Function} [options.onValidationError]   - Called with error message on validation failure
   * @param {Function} [options.onPlaylistDetected]  - Called with URL when a playlist is detected
   * @param {HTMLElement} [options.inputEl]          - URL input element (defaults to #url-input)
   * @param {HTMLElement} [options.errorEl]          - Error display element (defaults to #url-error)
   * @param {HTMLElement} [options.submitBtnEl]      - Submit button element (defaults to #btn-add-queue)
   * @returns {Object} Component API
   */
  function init(options = {}) {
    _elInput = options.inputEl || document.getElementById('url-input');
    _elError = options.errorEl || document.getElementById('url-error');
    _elSubmitBtn = options.submitBtnEl || document.getElementById('btn-add-queue');
    _onSubmit = options.onSubmit || null;
    _onValidationError = options.onValidationError || null;
    _onPlaylistDetected = options.onPlaylistDetected || null;

    _bindEvents();

    return {
      submit: handleSubmit,
      reset: resetInput,
      focus: focusInput,
      getValue,
      setValue,
      validate: validateUrl,
      clearError,
    };
  }

  return {
    init,
    validateUrl,
    normalizeUrl,
  };
})();
