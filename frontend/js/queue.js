/**
 * @file queue.js
 * @description framepick 작업 큐 UI 컴포넌트
 *
 * 큐에 등록된 URL 목록을 표시하며 다음을 포함한다:
 *   - 현재 처리 중인 URL과 진행 표시줄
 *   - 단계별 진행 상태 (예: "3/5 단계: 다운로드 중... 45%")
 *   - 각 항목의 상태 아이콘 (대기/처리/완료/실패)
 *
 * 수신하는 Tauri 이벤트:
 *   - queue-item-status: { id, status, progress?, title?, error? }
 *   - queue-progress: { total, completed, failed, is_processing }
 *   - pipeline:progress: { queue_id, stage, stage_number, total_stages, percent, detail? }
 *   - pipeline:error: { queue_id, stage, message }
 *
 * 발행하는 DOM 이벤트:
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

  /** 현재 처리 중인 항목의 경과 시간 추적기 */
  let _elapsedTimer = null;
  let _processingStartTime = null;

  /** 중복 토스트 방지를 위해 이미 실패 토스트를 표시한 ID 추적 */
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

  // 파이프라인 단계 정의 (자막 모드 기준 최대 세트; 다른 모드는 하위 집합 사용)
  const PIPELINE_STAGES = [
    { key: 'queue_stage_download', label: 'Downloading' },
    { key: 'queue_stage_subtitle', label: 'Fetching subtitles' },
    { key: 'queue_stage_capture', label: 'Capturing frames' },
    { key: 'queue_stage_generate', label: 'Generating slides' },
    { key: 'queue_stage_cleanup', label: 'Cleaning up' },
  ];

  const TOTAL_STEPS = PIPELINE_STAGES.length;

  // 백엔드 snake_case 단계 키를 PIPELINE_STAGES 인덱스로 매핑
  const STAGE_KEY_MAP = {
    'downloading': 'queue_stage_download',
    'extracting_subtitles': 'queue_stage_subtitle',
    'extracting_frames': 'queue_stage_capture',
    'generating_slides': 'queue_stage_generate',
    'cleanup': 'queue_stage_cleanup',
    'done': 'queue_status_done',
  };

  // 상태별 인라인 SVG 아이콘
  const STATUS_ICONS = {
    pending: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>',
    processing: '<svg class="queue-spin" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>',
    completed: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#22c55e" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    failed: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#ef4444" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
    skipped: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="8" y1="12" x2="16" y2="12"/></svg>',
  };

  /**
   * 큐 UI를 초기화한다.
   */
  function init() {
    containerEl = document.getElementById('queue-container');
    if (!containerEl) {
      console.warn('QueueUI: Missing #queue-container element');
      return;
    }
    render();

    // 언어 변경 시 큐 UI 재렌더링
    document.addEventListener('languageChanged', () => render());

    // 백엔드에서 실시간 업데이트를 받기 위한 Tauri 이벤트 리스너 설정
    _setupTauriListeners();
  }

  /**
   * 백엔드 → 프론트엔드 통신을 위한 Tauri 이벤트 리스너를 설정한다.
   */
  function _setupTauriListeners() {
    if (!window.__TAURI__ || !window.__TAURI__.event) return;
    const { listen } = window.__TAURI__.event;

    // 큐 항목 상태 변경 (queue_processor.rs에서 발행)
    listen('queue-item-status', (event) => {
      const data = event.payload;
      _handleBackendStatusUpdate(data);
    });

    // 전체 큐 진행 상태 (queue_processor.rs에서 발행)
    listen('queue-progress', (event) => {
      const data = event.payload;
      isProcessing = data.is_processing;
      // 처리 중 표시 업데이트를 위해 재렌더링
      render();
    });

    // 파이프라인 단계별 진행 상태 (progress.rs ProgressTracker에서 발행)
    listen('pipeline:progress', (event) => {
      const data = event.payload;
      _handlePipelineProgress(data);
    });

    // 파이프라인 오류 이벤트 (progress.rs ProgressTracker에서 발행)
    listen('pipeline:error', (event) => {
      const data = event.payload;
      _handlePipelineError(data);
    });
  }

  /**
   * 백엔드에서 받은 큐 항목 상태 업데이트를 처리한다.
   * @param {{ id: number, status: string, progress?: number, title?: string, error?: string }} data
   */
  function _handleBackendStatusUpdate(data) {
    const item = _findItem(data.id);
    if (!item) return;

    // 백엔드 상태를 로컬 상태로 매핑
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

      // 실패 토스트 알림 표시 (중복 방지)
      _showFailureToast(data.id, item.title, item.errorMessage);

      document.dispatchEvent(new CustomEvent('queueItemFailed', {
        detail: { id: data.id, error: data.error }
      }));
    }

    // "skipped" 상태: 처리 중 중복 감지 — 완료와 유사하지만 경고로 처리
    if (data.status === 'skipped') {
      _stopElapsedTimer();
      item.status = 'skipped';
      item.errorMessage = data.error || t('error_already_exists');
      _checkProcessingDone();
    }

    render();
  }

  /**
   * 백엔드에서 받은 파이프라인 단계별 진행 상태를 처리한다.
   * progress.rs 단계를 PIPELINE_STAGES에 매핑한다.
   * @param {{ queue_id: number, stage: string, stage_number: number, total_stages: number, percent: number, detail?: string }} data
   */
  function _handlePipelineProgress(data) {
    const item = _findItem(data.queue_id);
    if (!item) return;

    // 'done' 단계 처리
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

    // 진행 데이터를 받으면 상태를 'processing'으로 설정
    if (item.status === 'pending') {
      item.status = 'processing';
      isProcessing = true;
      _startElapsedTimer(data.queue_id);
    }

    // 백엔드 단계를 0-based 단계 인덱스로 매핑하고 백엔드의 total_stages 사용
    item.currentStep = Math.max(0, data.stage_number - 1);
    item.totalSteps = data.total_stages;
    item.stagePercent = data.percent;
    item.stageLabel = STAGE_KEY_MAP[data.stage] || 'queue_stage_download';
    item._detail = data.detail || null;

    render();
  }

  /**
   * 백엔드에서 받은 파이프라인 오류 이벤트를 처리한다.
   * @param {{ queue_id: number, stage: string, message: string }} data
   */
  function _handlePipelineError(data) {
    const item = _findItem(data.queue_id);
    if (!item) return;

    _stopElapsedTimer();
    item.status = 'failed';
    item.errorMessage = data.message;
    _checkProcessingDone();

    // 실패 토스트 알림 표시 (중복 방지)
    _showFailureToast(data.queue_id, item.title, data.message);

    document.dispatchEvent(new CustomEvent('queueItemFailed', {
      detail: { id: data.queue_id, error: data.message }
    }));

    render();
  }

  /**
   * URL을 처리 큐에 추가한다.
   * 로컬 상태와 백엔드 큐에 모두 추가한 후 처리를 시작한다.
   * @param {string} url
   * @param {string} videoId
   * @returns {Promise<number>} 큐 항목 id
   */
  async function addItem(url, videoId) {
    // DOM에서 현재 캡쳐 모드 설정 읽기
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

    // 먼저 백엔드 큐에 추가
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
        // 백엔드가 할당한 값 사용
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

    // 자동으로 처리 시작
    _startProcessing();

    return item.id;
  }

  /**
   * 백엔드 큐 처리 루프를 시작한다.
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
   * 큐 항목 처리를 시작한다 (수동/로컬 사용 시).
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
   * 큐 항목의 진행 상태를 업데이트한다 (수동/로컬 사용 시).
   * @param {number} id
   * @param {number} step - 0부터 시작하는 단계 인덱스
   * @param {number} percent - 현재 단계 내 0-100 진행률
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
   * 큐 항목을 완료 상태로 표시한다 (수동/로컬 사용 시).
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
   * 큐 항목을 실패 상태로 표시한다 (수동/로컬 사용 시).
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
   * 큐에서 특정 항목을 제거한다 (처리 중이 아닌 경우에만).
   * @param {number} id
   */
  async function removeItem(id) {
    const item = _findItem(id);
    if (!item || item.status === 'processing') return;

    // 백엔드에서 제거
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
   * 완료 및 실패한 모든 항목을 큐에서 제거한다.
   */
  async function clearCompleted() {
    const toRemove = queue.filter(q => q.status === 'completed' || q.status === 'failed' || q.status === 'skipped');

    // 백엔드에서 제거
    if (window.__TAURI__ && window.__TAURI__.core) {
      for (const item of toRemove) {
        try {
          await window.__TAURI__.core.invoke('remove_queue_item', { id: item.id });
        } catch (err) {
          console.warn('Failed to remove queue item:', err);
        }
      }
    }

    // 제거된 항목의 실패 토스트 추적 데이터 정리
    toRemove.forEach(item => _failToastShown.delete(item.id));

    queue = queue.filter(q => q.status === 'pending' || q.status === 'processing');
    render();

    document.dispatchEvent(new CustomEvent('queueCleared'));
  }

  /**
   * 전체 큐 상태를 반환한다 (외부 소비자용).
   * @returns {Array<QueueItem>}
   */
  function getQueue() {
    return queue.map(item => ({ ...item }));
  }

  /**
   * 상태별 항목 수를 반환한다.
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
   * 실패한 큐 항목을 재시도한다 — 대기 상태로 초기화하고 재큐잉한다.
   * @param {number} id
   */
  async function retryItem(id) {
    const item = _findItem(id);
    if (!item || item.status !== 'failed') return;

    // 로컬 상태 초기화
    item.status = 'pending';
    item.currentStep = 0;
    item.stagePercent = 0;
    item.stageLabel = '';
    item.errorMessage = null;
    item._detail = null;
    _failToastShown.delete(id);

    // 백엔드 상태 초기화
    if (window.__TAURI__ && window.__TAURI__.core) {
      try {
        await window.__TAURI__.core.invoke('retry_queue_item', { id: item.id });
      } catch (err) {
        // retry 명령이 없으면 제거 후 재추가 방식으로 처리
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

    // 처리 중이 아닌 경우 자동으로 처리 시작
    _startProcessing();
  }

  // ---- 내부 헬퍼 함수 ----

  function _findItem(id) {
    return queue.find(q => q.id === id) || null;
  }

  /**
   * 실패한 큐 항목에 대한 토스트 알림을 표시한다 (항목 ID별 중복 방지).
   * @param {number} itemId
   * @param {string|null} title - 영상 제목 (있는 경우)
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

    // 전역 showToast가 있으면 사용, 없으면 로컬 _showToast 사용
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
   * 현재 처리 중인 항목의 경과 시간 추적을 시작한다.
   * @param {number} itemId
   */
  function _startElapsedTimer(itemId) {
    _stopElapsedTimer();
    const item = _findItem(itemId);
    if (item) {
      item._startedAt = Date.now();
    }
    _processingStartTime = Date.now();
    // 1초마다 경과 시간 표시 업데이트
    _elapsedTimer = setInterval(() => {
      _updateElapsedDisplay();
    }, 1000);
  }

  /**
   * 경과 시간 추적 인터벌을 중지한다.
   */
  function _stopElapsedTimer() {
    if (_elapsedTimer) {
      clearInterval(_elapsedTimer);
      _elapsedTimer = null;
    }
    _processingStartTime = null;
  }

  /**
   * 전체 재렌더링 없이 경과 시간 표시만 업데이트한다.
   */
  function _updateElapsedDisplay() {
    if (!containerEl) return;
    const el = containerEl.querySelector('.queue-active-elapsed');
    if (!el || !_processingStartTime) return;
    el.textContent = _formatElapsed(Date.now() - _processingStartTime);
  }

  /**
   * 밀리초를 사람이 읽기 쉬운 경과 시간 문자열로 변환한다.
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
   * 대기 중인 항목의 큐 순서를 반환한다 (1부터 시작).
   * @param {QueueItem} item
   * @returns {number} 순서 (1부터 시작), 대기 중이 아니면 0
   */
  function _getPendingPosition(item) {
    if (item.status !== 'pending') return 0;
    const pendingItems = queue.filter(q => q.status === 'pending');
    return pendingItems.indexOf(item) + 1;
  }

  /**
   * 표시용으로 URL을 잘라낸다.
   * @param {string} url
   * @param {number} maxLen
   * @returns {string}
   */
  function _truncateUrl(url, maxLen) {
    if (url.length <= maxLen) return url;
    return url.substring(0, maxLen - 3) + '...';
  }

  /**
   * 진행 표시줄의 전체 진행률(%)을 계산한다.
   * @param {QueueItem} item
   * @returns {number} 0-100
   */
  function _overallPercent(item) {
    if (item.status === 'completed') return 100;
    if (item.status === 'failed' || item.status === 'pending') return 0;
    // 각 단계가 동일한 비중을 가짐
    const steps = item.totalSteps || TOTAL_STEPS;
    const stepWeight = 100 / steps;
    return Math.round(item.currentStep * stepWeight + (item.stagePercent / 100) * stepWeight);
  }

  /**
   * "3/5 단계: 다운로드 중... 45%"와 같은 단계 레이블 문자열을 생성한다.
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

  // ---- 렌더링 스로틀링 ----
  let _renderScheduled = false;

  /**
   * 다음 애니메이션 프레임에 렌더링을 예약한다 (연속 업데이트 디바운스).
   * 대기 중인 프레임이 없으면 즉시 렌더링한다.
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
   * 공개 렌더링 진입점 — 배치 렌더링을 예약한다.
   */
  function render() {
    _scheduleRender();
  }

  /**
   * 실제 렌더링 구현체.
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

    // 현재 처리 중인 항목 찾기
    const processingItem = queue.find(q => q.status === 'processing');

    let html = '<div class="queue-panel">';

    // 상태 배지가 포함된 큐 헤더
    html += '<div class="queue-header">';
    html += `<h3 class="queue-title" data-i18n="queue_title">${t('queue_title')}</h3>`;
    html += '<div class="queue-header-actions">';

    // 상태 요약 배지
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

    // 다중 세그먼트 진행 표시줄이 포함된 현재 처리 섹션
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
      // 경과 시간
      const elapsed = _processingStartTime ? _formatElapsed(Date.now() - _processingStartTime) : '';
      if (elapsed) {
        html += `<span class="queue-active-elapsed" title="${t('queue_elapsed')}">${elapsed}</span>`;
      }
      html += `<span class="queue-active-pct-badge">${overallPct}%</span>`;
      html += '</div>';
      html += '</div>';

      // 단계 정보 행: 단계 배지 + 단계명 + 단계 진행률
      html += '<div class="queue-stage-info">';
      html += `<span class="queue-stage-step">${stepNum}/${steps}</span>`;
      html += `<span class="queue-stage-name">${_escapeHtml(stageName)}</span>`;
      const detail = processingItem._detail ? ` (${_escapeHtml(processingItem._detail)})` : '';
      html += `<span class="queue-stage-pct">${stagePct}%${detail}</span>`;
      html += '</div>';

      // 파이프라인 단계당 하나의 세그먼트로 구성된 다중 세그먼트 진행 표시줄
      html += '<div class="queue-segments">';
      for (let i = 0; i < steps; i++) {
        let segClass = 'queue-segment';
        let segWidth = 0;
        if (i < processingItem.currentStep) {
          // 완료된 단계
          segClass += ' queue-segment-done';
          segWidth = 100;
        } else if (i === processingItem.currentStep) {
          // 현재 단계
          segClass += ' queue-segment-active';
          segWidth = processingItem.stagePercent;
        }
        // else: 대기 중인 단계, segWidth는 0 유지

        html += `<div class="${segClass}">`;
        html += `<div class="queue-segment-fill" style="width: ${segWidth}%"></div>`;
        html += '</div>';
      }
      html += '</div>';

      html += '</div>';
    }

    // 큐 목록
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
        // 대기 중인 항목에 큐 순서 배지 표시
        const pos = _getPendingPosition(item);
        if (pos > 0) {
          html += `<span class="queue-position-badge"><svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>${t('queue_position', { n: pos })}</span>`;
        } else {
          html += `<span class="queue-item-pending-text">${t('queue_status_pending')}</span>`;
        }
      }

      html += '</div>';

      // 처리 중이 아닌 항목의 액션 버튼
      html += '<div class="queue-item-actions">';
      if (item.status === 'failed') {
        // 실패한 항목의 재시도 버튼
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

    // 클릭 이벤트 바인딩
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
   * 토스트 알림을 표시한다.
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
