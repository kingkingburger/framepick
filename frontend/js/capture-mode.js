/**
 * @file capture-mode.js
 * @description 프레임 캡쳐 방식 선택 컴포넌트
 *
 * 드롭다운으로 캡쳐 전략을 선택할 수 있게 한다:
 *   - subtitle (기본값): 자막 세그먼트 시작 시점에 프레임 캡쳐
 *   - scene: 장면 변화 감지 시 프레임 캡쳐 (변화율 30% 이상)
 *   - interval: 고정 간격(10/30/60초 프리셋 + 직접 입력)으로 프레임 캡쳐
 *
 * AppState와 통합되어 중앙 집중식 상태 관리를 지원한다.
 */

const CAPTURE_MODES = {
  subtitle: {
    id: 'subtitle',
    i18nLabel: 'capture_mode_subtitle',
    i18nDesc: 'capture_mode_subtitle_desc',
  },
  scene: {
    id: 'scene',
    i18nLabel: 'capture_mode_scene',
    i18nDesc: 'capture_mode_scene_desc',
  },
  interval: {
    id: 'interval',
    i18nLabel: 'capture_mode_interval',
    i18nDesc: 'capture_mode_interval_desc',
  },
};

const DEFAULT_CAPTURE_MODE = 'subtitle';
const INTERVAL_OPTIONS = [10, 30, 60];
const DEFAULT_INTERVAL = 30;
const CUSTOM_INTERVAL_MIN = 1;
const CUSTOM_INTERVAL_MAX = 3600;

/**
 * 캡쳐 모드 컴포넌트를 초기화한다.
 * id="capture-mode-container" 컨테이너 요소가 있어야 한다.
 */
function initCaptureMode() {
  const container = document.getElementById('capture-mode-container');
  if (!container) return;

  // HTML 구조 생성
  container.innerHTML = `
    <div class="form-group capture-mode-group">
      <label for="capture-mode-select" class="form-label" data-i18n="capture_mode_label">${t('capture_mode_label')}</label>
      <div class="capture-mode-controls">
        <select id="capture-mode-select" class="form-select">
          ${Object.values(CAPTURE_MODES).map(mode => `
            <option value="${mode.id}" data-i18n="${mode.i18nLabel}" ${mode.id === DEFAULT_CAPTURE_MODE ? 'selected' : ''}>
              ${t(mode.i18nLabel)}
            </option>
          `).join('')}
        </select>
        <div id="interval-options" class="interval-options" style="display: none;">
          <label for="interval-select" class="form-label form-label-sm" data-i18n="interval_label">${t('interval_label')}</label>
          <select id="interval-select" class="form-select form-select-sm">
            ${INTERVAL_OPTIONS.map(n => `
              <option value="${n}" data-i18n="interval_seconds" data-i18n-params='{"n":"${n}"}' ${n === DEFAULT_INTERVAL ? 'selected' : ''}>
                ${t('interval_seconds', { n })}
              </option>
            `).join('')}
            <option value="custom" data-i18n="interval_custom">${t('interval_custom')}</option>
          </select>
          <div id="custom-interval-group" class="custom-interval-group" style="display: none;">
            <div class="custom-interval-input-row">
              <input type="number" id="custom-interval-input" class="form-input form-input-sm"
                min="${CUSTOM_INTERVAL_MIN}" max="${CUSTOM_INTERVAL_MAX}" value="15"
                placeholder="${t('interval_custom_placeholder')}"
                data-i18n-placeholder="interval_custom_placeholder">
              <span class="custom-interval-unit" data-i18n="interval_unit_seconds">${t('interval_unit_seconds')}</span>
            </div>
            <p id="custom-interval-hint" class="form-hint form-hint-sm" data-i18n="interval_custom_hint"
              >${t('interval_custom_hint', { min: CUSTOM_INTERVAL_MIN, max: CUSTOM_INTERVAL_MAX })}</p>
          </div>
        </div>
      </div>
      <p id="capture-mode-desc" class="form-hint" data-i18n="${CAPTURE_MODES[DEFAULT_CAPTURE_MODE].i18nDesc}">
        ${t(CAPTURE_MODES[DEFAULT_CAPTURE_MODE].i18nDesc)}
      </p>
    </div>
  `;

  // 이벤트 바인딩
  const modeSelect = document.getElementById('capture-mode-select');
  const intervalOptions = document.getElementById('interval-options');
  const intervalSelect = document.getElementById('interval-select');
  const customIntervalGroup = document.getElementById('custom-interval-group');
  const customIntervalInput = document.getElementById('custom-interval-input');
  const descEl = document.getElementById('capture-mode-desc');

  /** 간격 선택값에 따라 직접 입력 필드를 표시/숨긴다 */
  function toggleCustomInterval() {
    const isCustom = intervalSelect.value === 'custom';
    customIntervalGroup.style.display = isCustom ? 'block' : 'none';
    if (isCustom) {
      customIntervalInput.focus();
    }
  }

  /** 프리셋 또는 직접 입력값으로부터 유효한 간격(초)을 반환한다 */
  function getEffectiveInterval() {
    if (intervalSelect.value === 'custom') {
      const val = parseInt(customIntervalInput.value, 10);
      if (isNaN(val) || val < CUSTOM_INTERVAL_MIN || val > CUSTOM_INTERVAL_MAX) {
        return DEFAULT_INTERVAL; // fallback
      }
      return val;
    }
    return parseInt(intervalSelect.value, 10);
  }

  /** 직접 입력 간격의 유효성을 검사하고 시각적 피드백을 표시한다 */
  function validateCustomInterval() {
    const val = parseInt(customIntervalInput.value, 10);
    const isValid = !isNaN(val) && val >= CUSTOM_INTERVAL_MIN && val <= CUSTOM_INTERVAL_MAX;
    customIntervalInput.classList.toggle('input-invalid', !isValid && customIntervalInput.value !== '');
    customIntervalInput.classList.toggle('input-valid', isValid);
    return isValid;
  }

  modeSelect.addEventListener('change', () => {
    const mode = modeSelect.value;

    // 간격 옵션 표시/숨김 전환
    intervalOptions.style.display = mode === 'interval' ? 'flex' : 'none';

    // 간격 모드로 전환 시 직접 입력 필드 표시 초기화
    if (mode === 'interval') {
      toggleCustomInterval();
    }

    // 설명 텍스트 업데이트
    const descKey = CAPTURE_MODES[mode].i18nDesc;
    descEl.setAttribute('data-i18n', descKey);
    descEl.textContent = t(descKey);

    // 중앙 상태(AppState) 업데이트
    if (typeof AppState !== 'undefined') {
      AppState.setCaptureMode(mode);
      if (mode === 'interval') {
        AppState.setIntervalSeconds(getEffectiveInterval());
      }
    }

    // 다른 리스너를 위한 커스텀 이벤트 발행
    document.dispatchEvent(new CustomEvent('captureModeChanged', {
      detail: getCaptureModeConfig()
    }));
  });

  intervalSelect.addEventListener('change', () => {
    toggleCustomInterval();
    const seconds = getEffectiveInterval();

    // 중앙 상태(AppState) 업데이트
    if (typeof AppState !== 'undefined') {
      AppState.setIntervalSeconds(seconds);
    }

    // 커스텀 이벤트 발행
    document.dispatchEvent(new CustomEvent('captureModeChanged', {
      detail: getCaptureModeConfig()
    }));
  });

  // 직접 입력 간격 변경 처리 (디바운스 적용)
  let customIntervalTimer = null;
  customIntervalInput.addEventListener('input', () => {
    validateCustomInterval();
    clearTimeout(customIntervalTimer);
    customIntervalTimer = setTimeout(() => {
      if (validateCustomInterval()) {
        const seconds = getEffectiveInterval();

        if (typeof AppState !== 'undefined') {
          AppState.setIntervalSeconds(seconds);
        }

        document.dispatchEvent(new CustomEvent('captureModeChanged', {
          detail: getCaptureModeConfig()
        }));
      }
    }, 300);
  });

  // Enter 키로 직접 입력값 즉시 확정 처리
  customIntervalInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      clearTimeout(customIntervalTimer);
      if (validateCustomInterval()) {
        const seconds = getEffectiveInterval();
        if (typeof AppState !== 'undefined') {
          AppState.setIntervalSeconds(seconds);
        }
        document.dispatchEvent(new CustomEvent('captureModeChanged', {
          detail: getCaptureModeConfig()
        }));
      }
    }
  });

  // 언어 변경 시 설명 및 직접 입력 힌트 업데이트
  document.addEventListener('languageChanged', () => {
    const mode = modeSelect.value;
    const descKey = CAPTURE_MODES[mode].i18nDesc;
    descEl.setAttribute('data-i18n', descKey);
    descEl.textContent = t(descKey);
    // 직접 입력 힌트에 최소/최대값 반영
    const hintEl = document.getElementById('custom-interval-hint');
    if (hintEl) {
      hintEl.textContent = t('interval_custom_hint', { min: CUSTOM_INTERVAL_MIN, max: CUSTOM_INTERVAL_MAX });
    }
  });

  // AppState → UI 동기화 (외부에서 상태 변경 시, 예: 설정 로드)
  if (typeof AppState !== 'undefined') {
    AppState.on('captureMode', (mode) => {
      if (modeSelect.value !== mode) {
        modeSelect.value = mode;
        intervalOptions.style.display = mode === 'interval' ? 'flex' : 'none';
        if (mode === 'interval') {
          toggleCustomInterval();
        }
        const descKey = CAPTURE_MODES[mode].i18nDesc;
        descEl.setAttribute('data-i18n', descKey);
        descEl.textContent = t(descKey);
      }
    });

    AppState.on('intervalSeconds', (seconds) => {
      // 프리셋 값인지 직접 입력값인지 확인
      if (INTERVAL_OPTIONS.includes(seconds)) {
        if (intervalSelect.value !== String(seconds)) {
          intervalSelect.value = String(seconds);
          toggleCustomInterval();
        }
      } else {
        // 직접 입력값인 경우
        intervalSelect.value = 'custom';
        customIntervalInput.value = seconds;
        toggleCustomInterval();
      }
    });
  }
}

/**
 * 현재 캡쳐 모드 설정을 반환한다.
 * AppState가 사용 가능하면 AppState에서, 없으면 DOM에서 읽는다.
 * @returns {{ mode: string, interval?: number }}
 */
function getCaptureModeConfig() {
  if (typeof AppState !== 'undefined') {
    const mode = AppState.get('captureMode');
    const config = { mode };
    if (mode === 'interval') {
      config.interval = AppState.get('intervalSeconds');
    }
    return config;
  }

  // 폴백: DOM에서 직접 읽기
  const modeSelect = document.getElementById('capture-mode-select');
  const mode = modeSelect ? modeSelect.value : DEFAULT_CAPTURE_MODE;
  const config = { mode };
  if (mode === 'interval') {
    const intervalSelect = document.getElementById('interval-select');
    if (intervalSelect && intervalSelect.value === 'custom') {
      const customInput = document.getElementById('custom-interval-input');
      const val = parseInt(customInput ? customInput.value : DEFAULT_INTERVAL, 10);
      config.interval = (isNaN(val) || val < CUSTOM_INTERVAL_MIN || val > CUSTOM_INTERVAL_MAX)
        ? DEFAULT_INTERVAL : val;
    } else {
      config.interval = parseInt(intervalSelect ? intervalSelect.value : DEFAULT_INTERVAL, 10);
    }
  }
  return config;
}

/**
 * 캡쳐 모드를 프로그래밍 방식으로 설정한다.
 * @param {string} mode - 'subtitle', 'scene', 'interval' 중 하나
 * @param {number} [interval] - 간격(초), 'interval' 모드에서만 사용
 */
function setCaptureModeConfig(mode, interval) {
  if (typeof AppState !== 'undefined') {
    AppState.setCaptureMode(mode);
    if (mode === 'interval' && interval) {
      AppState.setIntervalSeconds(interval);
    }
    return;
  }

  // 폴백: DOM 직접 설정
  const modeSelect = document.getElementById('capture-mode-select');
  if (!modeSelect) return;
  modeSelect.value = mode;
  modeSelect.dispatchEvent(new Event('change'));
  if (mode === 'interval' && interval) {
    const intervalSelect = document.getElementById('interval-select');
    if (intervalSelect) {
      if (INTERVAL_OPTIONS.includes(interval)) {
        intervalSelect.value = String(interval);
      } else {
        intervalSelect.value = 'custom';
        const customInput = document.getElementById('custom-interval-input');
        if (customInput) {
          customInput.value = interval;
        }
      }
      intervalSelect.dispatchEvent(new Event('change'));
    }
  }
}
