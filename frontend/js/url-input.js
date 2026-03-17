/**
 * @file url-input.js
 * @description YouTube URL 입력 필드 컴포넌트 (유효성 검사 포함)
 *
 * 역할:
 *  - 실시간 클라이언트 측 URL 유효성 검사 및 시각적 피드백 제공
 *  - 대기열 제출 전 Tauri 백엔드 명령을 통한 서버 측 검증
 *  - 단일 영상 URL 및 재생목록 URL 모두 지원
 */

const UrlInput = (() => {
  // 클라이언트 측 유효성 검사용 YouTube URL 패턴
  const YOUTUBE_PATTERNS = [
    // 표준 시청 URL: youtube.com/watch?v=VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/watch\?(?:.*&)?v=([A-Za-z0-9_-]{11})(?:[&#]|$)/,
    // 단축 URL: youtu.be/VIDEO_ID
    /^https?:\/\/youtu\.be\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // 임베드 URL: youtube.com/embed/VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/embed\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // 쇼츠 URL: youtube.com/shorts/VIDEO_ID
    /^https?:\/\/(?:www\.)?youtube\.com\/shorts\/([A-Za-z0-9_-]{11})(?:[?&#/]|$)/,
    // 모바일 URL: m.youtube.com/watch?v=VIDEO_ID
    /^https?:\/\/m\.youtube\.com\/watch\?(?:.*&)?v=([A-Za-z0-9_-]{11})(?:[&#]|$)/,
  ];

  let inputEl = null;
  let startBtn = null;
  let feedbackEl = null;
  let debounceTimer = null;
  let lastValidationResult = null;

  /**
   * URL 입력 컴포넌트를 초기화한다.
   * 이벤트 리스너를 바인딩하고 피드백 요소를 생성한다.
   */
  function init() {
    inputEl = document.getElementById('url-input');
    startBtn = document.getElementById('start-btn');

    if (!inputEl || !startBtn) {
      console.warn('UrlInput: Missing #url-input or #start-btn elements');
      return;
    }

    // 유효성 검사 피드백 요소 생성
    feedbackEl = document.createElement('div');
    feedbackEl.className = 'url-feedback';
    feedbackEl.setAttribute('role', 'status');
    feedbackEl.setAttribute('aria-live', 'polite');
    inputEl.parentElement.insertBefore(feedbackEl, inputEl.nextSibling);

    // 피드백 위치 지정을 위한 입력 컨테이너 래핑
    _wrapInputGroup();

    // 이벤트 바인딩
    inputEl.addEventListener('input', _onInput);
    inputEl.addEventListener('paste', _onPaste);
    inputEl.addEventListener('keydown', _onKeyDown);
    startBtn.addEventListener('click', _onStartClick);

    // 초기 상태: 시작 버튼 비활성화
    startBtn.disabled = true;
    lastValidationResult = null;

    // 언어 변경 시 피드백 텍스트 업데이트
    document.addEventListener('languageChanged', () => {
      if (lastValidationResult !== null) {
        _showFeedback(lastValidationResult.valid, lastValidationResult.messageKey);
      }
    });
  }

  /**
   * 피드백 위치 지정을 지원하기 위해 DOM 구조를 재구성한다.
   */
  function _wrapInputGroup() {
    const group = inputEl.closest('.url-input-group');
    if (group && feedbackEl) {
      // 피드백을 flex 행 밖으로 이동하여 입력 그룹 아래에 배치
      group.parentElement.insertBefore(feedbackEl, group.nextSibling);
    }
  }

  // 재생목록 URL 패턴 (영상 ID 불필요)
  const PLAYLIST_PATTERNS = [
    /^https?:\/\/(?:www\.)?youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    /^https?:\/\/m\.youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
  ];

  /**
   * 클라이언트 측 YouTube URL 유효성 검사.
   * 단일 영상 URL과 재생목록 URL 모두 허용한다.
   * @param {string} url - 검사할 URL
   * @returns {{ valid: boolean, videoId: string|null, isPlaylist: boolean }}
   */
  function validateClientSide(url) {
    const trimmed = url.trim();
    if (!trimmed) {
      return { valid: false, videoId: null, empty: true, isPlaylist: false };
    }

    // 단일 영상 패턴 먼저 검사
    for (const pattern of YOUTUBE_PATTERNS) {
      const match = trimmed.match(pattern);
      if (match && match[1]) {
        return { valid: true, videoId: match[1], empty: false, isPlaylist: false };
      }
    }

    // 재생목록 전용 패턴 검사 (URL에 영상 ID 없음)
    for (const pattern of PLAYLIST_PATTERNS) {
      const match = trimmed.match(pattern);
      if (match && match[1]) {
        return { valid: true, videoId: null, empty: false, isPlaylist: true };
      }
    }

    return { valid: false, videoId: null, empty: false, isPlaylist: false };
  }

  /**
   * Tauri 명령을 통한 백엔드 URL 유효성 검사.
   * @param {string} url - 검사할 URL
   * @returns {Promise<{valid: boolean, video_id: string, error: string}>}
   */
  async function validateBackend(url) {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      try {
        return await window.__TAURI__.core.invoke('validate_youtube_url', { url });
      } catch (err) {
        console.warn('Backend validation failed, using client-side only:', err);
        // 백엔드 실패 시 클라이언트 측 검사로 폴백
        const result = validateClientSide(url);
        return {
          valid: result.valid,
          video_id: result.videoId || '',
          error: result.valid ? '' : 'Invalid YouTube URL',
        };
      }
    }
    // Tauri 미사용 환경 (브라우저 개발 모드 등)
    const result = validateClientSide(url);
    return {
      valid: result.valid,
      video_id: result.videoId || '',
      error: result.valid ? '' : 'Invalid YouTube URL',
    };
  }

  /**
   * 디바운스 처리된 유효성 검사와 함께 입력 이벤트를 처리한다.
   */
  function _onInput() {
    clearTimeout(debounceTimer);
    const value = inputEl.value.trim();

    // AppState에 URL 동기화
    if (typeof AppState !== 'undefined') {
      AppState.setUrl(value);
    }

    // 입력값이 비어있으면 즉시 피드백 초기화
    if (!value) {
      _clearFeedback();
      startBtn.disabled = true;
      lastValidationResult = null;
      inputEl.classList.remove('url-valid', 'url-invalid');
      return;
    }

    // 디바운스 처리된 클라이언트 측 빠른 검사
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
   * 붙여넣기 이벤트를 처리한다. 디바운스 없이 즉시 유효성을 검사한다.
   */
  function _onPaste(e) {
    // setTimeout으로 붙여넣기 완료 후 값을 가져옴
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
   * Enter 키 입력 시 시작 동작을 트리거한다.
   */
  function _onKeyDown(e) {
    if (e.key === 'Enter' && !startBtn.disabled) {
      e.preventDefault();
      _onStartClick();
    }
  }

  /**
   * 시작 버튼 클릭을 처리한다. 전체 유효성 검사 후 urlSubmitted 이벤트를 발행한다.
   */
  async function _onStartClick() {
    const url = inputEl.value.trim();
    if (!url) return;

    // 유효성 검사 중 버튼 비활성화
    startBtn.disabled = true;
    startBtn.textContent = t('url_validating');

    try {
      // 클라이언트 측 먼저 검사 — 단일 영상과 재생목록 모두 처리
      const clientResult = validateClientSide(url);

      if (clientResult.valid) {
        // 검증된 URL과 영상 ID로 커스텀 이벤트 발행
        // 재생목록 URL의 경우 videoId가 null일 수 있음 — app.js가 감지하여 재생목록 모달을 열어줌
        document.dispatchEvent(new CustomEvent('urlSubmitted', {
          detail: {
            url: url,
            videoId: clientResult.videoId || '',
            isPlaylist: clientResult.isPlaylist || false,
          }
        }));

        // 제출 성공 후 입력 필드 초기화
        inputEl.value = '';
        if (typeof AppState !== 'undefined') {
          AppState.setUrl('');
        }
        _clearFeedback();
        inputEl.classList.remove('url-valid', 'url-invalid');
        lastValidationResult = null;
        startBtn.disabled = true;
      } else {
        // 엣지 케이스 처리를 위해 백엔드 검증으로 폴백
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
   * 유효한 URL 상태의 시각적 표시를 설정한다.
   */
  function _setValidState(videoId) {
    inputEl.classList.remove('url-invalid');
    inputEl.classList.add('url-valid');
    startBtn.disabled = false;
    lastValidationResult = { valid: true, messageKey: 'url_valid', videoId };
    _showFeedback(true, 'url_valid');
  }

  /**
   * 유효하지 않은 URL 상태의 시각적 표시를 설정한다.
   */
  function _setInvalidState(errorMsg) {
    inputEl.classList.remove('url-valid');
    inputEl.classList.add('url-invalid');
    startBtn.disabled = true;
    lastValidationResult = { valid: false, messageKey: 'url_invalid' };
    _showFeedback(false, 'url_invalid');
  }

  /**
   * 유효성 검사 피드백 메시지를 표시한다.
   */
  function _showFeedback(isValid, messageKey) {
    if (!feedbackEl) return;
    feedbackEl.textContent = t(messageKey);
    feedbackEl.className = 'url-feedback ' + (isValid ? 'url-feedback-valid' : 'url-feedback-invalid');
  }

  /**
   * 유효성 검사 피드백을 초기화한다.
   */
  function _clearFeedback() {
    if (!feedbackEl) return;
    feedbackEl.textContent = '';
    feedbackEl.className = 'url-feedback';
  }

  /**
   * 현재 입력 필드의 URL 값을 반환한다.
   * @returns {string} 현재 URL 값
   */
  function getValue() {
    return inputEl ? inputEl.value.trim() : '';
  }

  /**
   * 입력 필드를 초기 빈 상태로 되돌린다.
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
