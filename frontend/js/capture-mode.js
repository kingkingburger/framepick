/**
 * Capture Mode Component
 * Provides a dropdown/select for choosing frame capture strategy:
 *   - subtitle (default): capture at subtitle segment start times
 *   - scene: capture on scene change detection (>30%)
 *   - interval: capture at fixed intervals (10/30/60s presets + custom value)
 *
 * Integrates with AppState for centralized state management.
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
 * Initialize the capture mode component.
 * Expects a container element with id="capture-mode-container".
 */
function initCaptureMode() {
  const container = document.getElementById('capture-mode-container');
  if (!container) return;

  // Build the HTML
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

  // Bind events
  const modeSelect = document.getElementById('capture-mode-select');
  const intervalOptions = document.getElementById('interval-options');
  const intervalSelect = document.getElementById('interval-select');
  const customIntervalGroup = document.getElementById('custom-interval-group');
  const customIntervalInput = document.getElementById('custom-interval-input');
  const descEl = document.getElementById('capture-mode-desc');

  /** Show/hide the custom interval input based on the interval select value */
  function toggleCustomInterval() {
    const isCustom = intervalSelect.value === 'custom';
    customIntervalGroup.style.display = isCustom ? 'block' : 'none';
    if (isCustom) {
      customIntervalInput.focus();
    }
  }

  /** Get the effective interval seconds from either preset or custom input */
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

  /** Validate custom interval input and show visual feedback */
  function validateCustomInterval() {
    const val = parseInt(customIntervalInput.value, 10);
    const isValid = !isNaN(val) && val >= CUSTOM_INTERVAL_MIN && val <= CUSTOM_INTERVAL_MAX;
    customIntervalInput.classList.toggle('input-invalid', !isValid && customIntervalInput.value !== '');
    customIntervalInput.classList.toggle('input-valid', isValid);
    return isValid;
  }

  modeSelect.addEventListener('change', () => {
    const mode = modeSelect.value;

    // Toggle interval options visibility
    intervalOptions.style.display = mode === 'interval' ? 'flex' : 'none';

    // Reset custom interval visibility when switching to interval mode
    if (mode === 'interval') {
      toggleCustomInterval();
    }

    // Update description
    const descKey = CAPTURE_MODES[mode].i18nDesc;
    descEl.setAttribute('data-i18n', descKey);
    descEl.textContent = t(descKey);

    // Update centralized state
    if (typeof AppState !== 'undefined') {
      AppState.setCaptureMode(mode);
      if (mode === 'interval') {
        AppState.setIntervalSeconds(getEffectiveInterval());
      }
    }

    // Dispatch custom event for any other listeners
    document.dispatchEvent(new CustomEvent('captureModeChanged', {
      detail: getCaptureModeConfig()
    }));
  });

  intervalSelect.addEventListener('change', () => {
    toggleCustomInterval();
    const seconds = getEffectiveInterval();

    // Update centralized state
    if (typeof AppState !== 'undefined') {
      AppState.setIntervalSeconds(seconds);
    }

    // Dispatch custom event
    document.dispatchEvent(new CustomEvent('captureModeChanged', {
      detail: getCaptureModeConfig()
    }));
  });

  // Handle custom interval input changes (with debounce)
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

  // Also handle Enter key in custom input for immediate commit
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

  // Listen for language changes to update description and custom hint
  document.addEventListener('languageChanged', () => {
    const mode = modeSelect.value;
    const descKey = CAPTURE_MODES[mode].i18nDesc;
    descEl.setAttribute('data-i18n', descKey);
    descEl.textContent = t(descKey);
    // Update custom interval hint with min/max values
    const hintEl = document.getElementById('custom-interval-hint');
    if (hintEl) {
      hintEl.textContent = t('interval_custom_hint', { min: CUSTOM_INTERVAL_MIN, max: CUSTOM_INTERVAL_MAX });
    }
  });

  // Sync AppState -> UI when state changes externally (e.g., from settings load)
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
      // Check if this is a preset value or custom
      if (INTERVAL_OPTIONS.includes(seconds)) {
        if (intervalSelect.value !== String(seconds)) {
          intervalSelect.value = String(seconds);
          toggleCustomInterval();
        }
      } else {
        // It's a custom value
        intervalSelect.value = 'custom';
        customIntervalInput.value = seconds;
        toggleCustomInterval();
      }
    });
  }
}

/**
 * Get the current capture mode configuration.
 * Reads from AppState if available, otherwise from DOM.
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

  // Fallback: read from DOM
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
 * Set the capture mode programmatically.
 * @param {string} mode - 'subtitle', 'scene', or 'interval'
 * @param {number} [interval] - interval seconds (only for 'interval' mode)
 */
function setCaptureModeConfig(mode, interval) {
  if (typeof AppState !== 'undefined') {
    AppState.setCaptureMode(mode);
    if (mode === 'interval' && interval) {
      AppState.setIntervalSeconds(interval);
    }
    return;
  }

  // Fallback: set DOM directly
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
