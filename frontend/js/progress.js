/**
 * @file progress.js
 * @description 파이프라인 단계별 진행 상태 추적 모듈
 *
 * Tauri의 `pipeline:progress` 및 `pipeline:error` 이벤트를 수신하고,
 * 큐 UI를 단계 정보 및 진행 표시줄로 업데이트하며,
 * 다른 모듈이 반응할 수 있도록 DOM 이벤트를 발행한다.
 */
const PipelineProgress = (() => {
  /** queue_id → 최신 진행 페이로드 맵 */
  const _state = new Map();

  /**
   * 진행 리스너를 초기화한다.
   * DOMContentLoaded 이후, Tauri가 사용 가능한 시점에 호출해야 한다.
   */
  function init() {
    _listenTauriEvents();
  }

  /**
   * 큐 항목의 현재 진행 상태를 반환한다.
   * @param {number} queueId
   * @returns {object|null} 최신 진행 페이로드 또는 null
   */
  function getProgress(queueId) {
    return _state.get(queueId) || null;
  }

  /**
   * 추적 중인 모든 진행 상태를 반환한다.
   * @returns {Map}
   */
  function getAllProgress() {
    return new Map(_state);
  }

  /**
   * 큐 항목의 진행 상태를 초기화한다 (예: 항목 제거 후).
   * @param {number} queueId
   */
  function clearProgress(queueId) {
    _state.delete(queueId);
  }

  /**
   * 진행 페이로드를 사람이 읽기 쉬운 상태 문자열로 변환한다.
   * 단계명에 i18n 번역을 사용한다.
   * @param {object} payload - 백엔드의 ProgressPayload
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
   * 큐 항목의 진행 표시줄 요소를 렌더링한다.
   * 주어진 컨테이너 내부에 진행 표시줄을 생성하거나 업데이트한다.
   * @param {HTMLElement} container - 렌더링할 DOM 요소
   * @param {object} payload - 백엔드의 ProgressPayload
   */
  function renderProgressBar(container, payload) {
    if (!container || !payload) return;

    let bar = container.querySelector('.progress-bar');
    let label = container.querySelector('.progress-label');
    let stageLabel = container.querySelector('.progress-stage');

    if (!bar) {
      // 단계 정보 행이 포함된 진행 UI 구조 생성
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

    // 전체 단계에 걸친 전체 진행률 계산
    const overallPercent = _calculateOverallPercent(payload);
    bar.style.width = `${overallPercent}%`;

    // 단계 배지
    const stepBadge = container.querySelector('.progress-step-badge');
    if (stepBadge) {
      stepBadge.textContent = `${payload.stage_number}/${payload.total_stages}`;
    }

    // 단계 레이블 (이름)
    const stageName = t(_stageToI18nKey(payload.stage));
    stageLabel.textContent = stageName;

    // 세부 정보가 포함된 진행률 레이블
    const detail = payload.detail ? ` (${payload.detail})` : '';
    label.textContent = `${payload.percent}%${detail}`;

    // aria 접근성 속성 업데이트
    const track = container.querySelector('.progress-track');
    if (track) {
      track.setAttribute('role', 'progressbar');
      track.setAttribute('aria-valuenow', overallPercent);
      track.setAttribute('aria-valuemin', 0);
      track.setAttribute('aria-valuemax', 100);
    }

    // 완료 상태 처리
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
   * 큐 항목의 오류 상태를 렌더링한다.
   * @param {HTMLElement} container - 렌더링할 DOM 요소
   * @param {object} errorPayload - 백엔드의 ErrorPayload
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

  // ── 내부 헬퍼 함수 ──────────────────────────────────────────────

  function _listenTauriEvents() {
    if (!window.__TAURI__ || !window.__TAURI__.event) {
      console.warn('PipelineProgress: Tauri event API not available');
      return;
    }

    const { listen } = window.__TAURI__.event;

    // 진행 상태 업데이트 수신
    listen('pipeline:progress', (event) => {
      const payload = event.payload;
      if (!payload || payload.queue_id == null) return;

      _state.set(payload.queue_id, payload);

      // 다른 모듈(큐 UI)이 반응할 수 있도록 DOM 이벤트 발행
      document.dispatchEvent(new CustomEvent('pipelineProgress', {
        detail: payload
      }));
    });

    // 오류 이벤트 수신
    listen('pipeline:error', (event) => {
      const payload = event.payload;
      if (!payload || payload.queue_id == null) return;

      // 오류 상태 저장
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
   * 전체 파이프라인 진행률(0-100)을 모든 단계에 걸쳐 계산한다.
   * 각 단계는 전체의 동일한 비중을 차지한다.
   */
  function _calculateOverallPercent(payload) {
    const { stage_number, total_stages, percent } = payload;
    if (total_stages === 0) return 0;
    // 완료된 단계는 전체 기여, 현재 단계는 비례 기여
    const completedStages = stage_number - 1;
    const stageShare = 100 / total_stages;
    return Math.round(completedStages * stageShare + (percent / 100) * stageShare);
  }

  /**
   * 백엔드의 snake_case 단계 열거값을 i18n 키로 매핑한다.
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
