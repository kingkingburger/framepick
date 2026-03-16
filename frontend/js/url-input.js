/**
 * url-input.js - YouTube URL input field component with validation
 *
 * Provides real-time client-side validation with visual feedback,
 * plus backend validation via Tauri command before queue submission.
 */

const UrlInput = (() => {
  // YouTube URL patterns for client-side validation
  const YOUTUBE_PATTERNS = [
    // Standard watch URL: youtube.com/watch?v=VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/watch\?(?:.*&)?v=([A-Za-z0-9_-]{11})(?:[&#]|$)/,
    // Short URL: youtu.be/VIDEO_ID
    /^https?:\/\/youtu\.be\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // Embed URL: youtube.com/embed/VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/embed\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // Shorts URL: youtube.com/shorts/VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/shorts\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // Mobile URL: m.youtube.com/watch?v=VIDEO_ID
    /^https?:\/\/m\.youtube\.com\/watch\?(?:.*&)?v=([A-Za-z0-9_-]{11})(?:[&#]|$)/,
  ];

  let inputEl = null;
  let startBtn = null;
  let feedbackEl = null;
  let debounceTimer = null;
  let lastValidationResult = null;

  /**
   * Initialize the URL input component.
   * Binds event listeners and creates feedback element.
   */
  function init() {
    inputEl = document.getElementById('url-input');
    startBtn = document.getElementById('start-btn');

    if (!inputEl || !startBtn) {
      console.warn('UrlInput: Missing #url-input or #start-btn elements');
      return;
    }

    // Create validation feedback element
    feedbackEl = document.createElement('div');
    feedbackEl.className = 'url-feedback';
    feedbackEl.setAttribute('role', 'status');
    feedbackEl.setAttribute('aria-live', 'polite');
    inputEl.parentElement.insertBefore(feedbackEl, inputEl.nextSibling);

    // Wrap the input in a container for positioning the feedback
    _wrapInputGroup();

    // Bind events
    inputEl.addEventListener('input', _onInput);
    inputEl.addEventListener('paste', _onPaste);
    inputEl.addEventListener('keydown', _onKeyDown);
    startBtn.addEventListener('click', _onStartClick);

    // Initial state: disable start button
    startBtn.disabled = true;
    lastValidationResult = null;

    // Listen for language changes to update feedback text
    document.addEventListener('languageChanged', () => {
      if (lastValidationResult !== null) {
        _showFeedback(lastValidationResult.valid, lastValidationResult.messageKey);
      }
    });
  }

  /**
   * Restructure DOM to support feedback positioning.
   */
  function _wrapInputGroup() {
    const group = inputEl.closest('.url-input-group');
    if (group && feedbackEl) {
      // Move feedback outside the flex row, below the input group
      group.parentElement.insertBefore(feedbackEl, group.nextSibling);
    }
  }

  // Playlist URL patterns (no video ID required)
  const PLAYLIST_PATTERNS = [
    /^https?:\/\/(?:www\.)?youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    /^https?:\/\/m\.youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
  ];

  /**
   * Client-side YouTube URL validation.
   * Accepts both single video URLs and playlist URLs.
   * @param {string} url
   * @returns {{ valid: boolean, videoId: string|null, isPlaylist: boolean }}
   */
  function validateClientSide(url) {
    const trimmed = url.trim();
    if (!trimmed) {
      return { valid: false, videoId: null, empty: true, isPlaylist: false };
    }

    // Check single video patterns first
    for (const pattern of YOUTUBE_PATTERNS) {
      const match = trimmed.match(pattern);
      if (match && match[1]) {
        return { valid: true, videoId: match[1], empty: false, isPlaylist: false };
      }
    }

    // Check playlist-only patterns (no video ID in URL)
    for (const pattern of PLAYLIST_PATTERNS) {
      const match = trimmed.match(pattern);
      if (match && match[1]) {
        return { valid: true, videoId: null, empty: false, isPlaylist: true };
      }
    }

    return { valid: false, videoId: null, empty: false, isPlaylist: false };
  }

  /**
   * Backend validation via Tauri command.
   * @param {string} url
   * @returns {Promise<{valid: boolean, video_id: string, error: string}>}
   */
  async function validateBackend(url) {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      try {
        return await window.__TAURI__.core.invoke('validate_youtube_url', { url });
      } catch (err) {
        console.warn('Backend validation failed, using client-side only:', err);
        // Fallback to client-side
        const result = validateClientSide(url);
        return {
          valid: result.valid,
          video_id: result.videoId || '',
          error: result.valid ? '' : 'Invalid YouTube URL',
        };
      }
    }
    // No Tauri available (e.g., dev mode in browser)
    const result = validateClientSide(url);
    return {
      valid: result.valid,
      video_id: result.videoId || '',
      error: result.valid ? '' : 'Invalid YouTube URL',
    };
  }

  /**
   * Handle input event with debounced validation.
   */
  function _onInput() {
    clearTimeout(debounceTimer);
    const value = inputEl.value.trim();

    // Sync to AppState
    if (typeof AppState !== 'undefined') {
      AppState.setUrl(value);
    }

    // Clear feedback immediately when empty
    if (!value) {
      _clearFeedback();
      startBtn.disabled = true;
      lastValidationResult = null;
      inputEl.classList.remove('url-valid', 'url-invalid');
      return;
    }

    // Quick client-side check with debounce
    debounceTimer = setTimeout(() => {
      const result = validateClientSide(value);
      if (result.valid) {
        _setValidState(result.videoId);
      } else {
        _setInvalidState();
      }
    }, 300);
  }

  /**
   * Handle paste event - validate immediately without debounce.
   */
  function _onPaste(e) {
    // Use setTimeout to get the pasted value after it's applied
    setTimeout(() => {
      clearTimeout(debounceTimer);
      const value = inputEl.value.trim();
      if (!value) {
        _clearFeedback();
        startBtn.disabled = true;
        lastValidationResult = null;
        inputEl.classList.remove('url-valid', 'url-invalid');
        return;
      }
      const result = validateClientSide(value);

      // Sync to AppState
      if (typeof AppState !== 'undefined') {
        AppState.setUrl(value);
      }

      if (result.valid) {
        _setValidState(result.videoId);
      } else {
        _setInvalidState();
      }
    }, 0);
  }

  /**
   * Handle Enter key to trigger start.
   */
  function _onKeyDown(e) {
    if (e.key === 'Enter' && !startBtn.disabled) {
      e.preventDefault();
      _onStartClick();
    }
  }

  /**
   * Handle start button click - perform full validation then dispatch event.
   */
  async function _onStartClick() {
    const url = inputEl.value.trim();
    if (!url) return;

    // Disable button during validation
    startBtn.disabled = true;
    startBtn.textContent = t('url_validating');

    try {
      // Check client-side first — handles both single videos and playlists
      const clientResult = validateClientSide(url);

      if (clientResult.valid) {
        // Dispatch custom event with validated URL and video ID
        // For playlist URLs, videoId may be null — app.js will detect and open playlist modal
        document.dispatchEvent(new CustomEvent('urlSubmitted', {
          detail: {
            url: url,
            videoId: clientResult.videoId || '',
            isPlaylist: clientResult.isPlaylist || false,
          }
        }));

        // Clear input after successful submission
        inputEl.value = '';
        if (typeof AppState !== 'undefined') {
          AppState.setUrl('');
        }
        _clearFeedback();
        inputEl.classList.remove('url-valid', 'url-invalid');
        lastValidationResult = null;
        startBtn.disabled = true;
      } else {
        // Try backend validation as fallback for edge cases
        const result = await validateBackend(url);
        if (result.valid) {
          document.dispatchEvent(new CustomEvent('urlSubmitted', {
            detail: {
              url: url,
              videoId: result.video_id,
              isPlaylist: false,
            }
          }));

          inputEl.value = '';
          if (typeof AppState !== 'undefined') {
            AppState.setUrl('');
          }
          _clearFeedback();
          inputEl.classList.remove('url-valid', 'url-invalid');
          lastValidationResult = null;
          startBtn.disabled = true;
        } else {
          _setInvalidState(result.error);
          startBtn.disabled = false;
        }
      }
    } catch (err) {
      console.error('Validation error:', err);
      _setInvalidState();
      startBtn.disabled = false;
    }

    startBtn.textContent = t('start_button');
  }

  /**
   * Set visual valid state.
   */
  function _setValidState(videoId) {
    inputEl.classList.remove('url-invalid');
    inputEl.classList.add('url-valid');
    startBtn.disabled = false;
    lastValidationResult = { valid: true, messageKey: 'url_valid', videoId };
    _showFeedback(true, 'url_valid');
  }

  /**
   * Set visual invalid state.
   */
  function _setInvalidState(errorMsg) {
    inputEl.classList.remove('url-valid');
    inputEl.classList.add('url-invalid');
    startBtn.disabled = true;
    lastValidationResult = { valid: false, messageKey: 'url_invalid' };
    _showFeedback(false, 'url_invalid');
  }

  /**
   * Show validation feedback message.
   */
  function _showFeedback(isValid, messageKey) {
    if (!feedbackEl) return;
    feedbackEl.textContent = t(messageKey);
    feedbackEl.className = 'url-feedback ' + (isValid ? 'url-feedback-valid' : 'url-feedback-invalid');
  }

  /**
   * Clear validation feedback.
   */
  function _clearFeedback() {
    if (!feedbackEl) return;
    feedbackEl.textContent = '';
    feedbackEl.className = 'url-feedback';
  }

  /**
   * Get the current URL value.
   * @returns {string}
   */
  function getValue() {
    return inputEl ? inputEl.value.trim() : '';
  }

  /**
   * Reset the input to empty state.
   */
  function reset() {
    if (inputEl) {
      inputEl.value = '';
      inputEl.classList.remove('url-valid', 'url-invalid');
    }
    _clearFeedback();
    if (startBtn) startBtn.disabled = true;
    lastValidationResult = null;
  }

  return {
    init,
    validateClientSide,
    validateBackend,
    getValue,
    reset,
  };
})();
