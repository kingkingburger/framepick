/**
 * Settings UI module for framepick
 * Wires the settings modal to Tauri backend commands:
 *   - get_settings: load current config from config.json
 *   - update_settings: persist changes to config.json
 *   - validate_settings: check tool availability and paths
 *   - reset_settings: restore all defaults
 * Uses Tauri dialog plugin for native folder picker.
 */

const SettingsUI = (() => {
  // DOM references (resolved on init)
  let modal, btnOpen, btnClose, btnCancel, btnSave, btnBrowse, btnReset;
  let inputLibraryPath, selectQuality, selectLanguage, checkboxMp4;
  // Capture mode controls
  let selectCaptureMode, inputInterval, rangeThreshold, thresholdValueEl;
  let intervalGroup;

  // Snapshot of settings when modal opens (for cancel/revert)
  let snapshot = null;
  let settingsLoaded = false;

  /**
   * Initialize the settings UI. Call once after DOMContentLoaded.
   */
  function init() {
    modal = document.getElementById('settings-modal');
    btnOpen = document.getElementById('btn-settings');
    btnClose = document.getElementById('btn-settings-close');
    btnCancel = document.getElementById('btn-settings-cancel');
    btnSave = document.getElementById('btn-settings-save');
    btnBrowse = document.getElementById('btn-browse-library');
    btnReset = document.getElementById('btn-settings-reset');
    inputLibraryPath = document.getElementById('settings-library-path');
    selectQuality = document.getElementById('settings-quality');
    selectLanguage = document.getElementById('settings-language');
    checkboxMp4 = document.getElementById('settings-mp4-retention');

    // Capture mode controls
    selectCaptureMode = document.getElementById('settings-capture-mode');
    inputInterval = document.getElementById('settings-interval');
    intervalGroup = document.getElementById('settings-interval-group');
    rangeThreshold = document.getElementById('settings-scene-threshold');
    thresholdValueEl = document.getElementById('settings-threshold-value');

    if (!modal || !btnOpen) {
      console.warn('Settings UI: modal or open button not found');
      return;
    }

    // Bind events
    btnOpen.addEventListener('click', openModal);
    btnClose.addEventListener('click', closeModal);
    btnCancel.addEventListener('click', closeModal);
    btnSave.addEventListener('click', handleSave);

    if (btnBrowse) {
      btnBrowse.addEventListener('click', browseFolder);
    }

    if (btnReset) {
      btnReset.addEventListener('click', handleReset);
    }

    // Capture mode → show/hide interval group
    if (selectCaptureMode) {
      selectCaptureMode.addEventListener('change', function() {
        toggleIntervalVisibility();
      });
    }

    // Threshold slider → update value display
    if (rangeThreshold && thresholdValueEl) {
      rangeThreshold.addEventListener('input', function() {
        thresholdValueEl.textContent = rangeThreshold.value + '%';
      });
    }

    // Close on overlay click
    modal.addEventListener('click', function(e) {
      if (e.target === modal) closeModal();
    });

    // Close on Escape key
    document.addEventListener('keydown', function(e) {
      if (e.key === 'Escape' && !modal.hidden) {
        e.preventDefault();
        closeModal();
      }
    });

    // Load initial settings from backend on app startup
    loadFromBackend();
  }

  /** Show/hide the interval input based on selected capture mode */
  function toggleIntervalVisibility() {
    if (!selectCaptureMode || !intervalGroup) return;
    var isInterval = selectCaptureMode.value === 'interval';
    intervalGroup.style.display = isInterval ? 'block' : 'none';
  }

  // ── Tauri invoke helpers ──────────────────────────────────

  function tauriInvoke(cmd, args) {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    // Dev fallback with mock data
    console.warn('[SettingsUI] Tauri not available, using mock for:', cmd);
    return Promise.resolve(mockInvoke(cmd, args));
  }

  function mockInvoke(cmd, args) {
    var defaults = {
      library_path: './library/',
      download_quality: '720',
      language: 'ko',
      mp4_retention: false,
      default_capture_mode: 'subtitle',
      default_interval_seconds: 30,
      scene_change_threshold: 0.30,
    };
    if (cmd === 'get_settings') return defaults;
    if (cmd === 'update_settings') {
      var p = (args && args.patch) ? args.patch : {};
      return Object.assign({}, defaults, p);
    }
    if (cmd === 'reset_settings') return defaults;
    if (cmd === 'validate_settings') {
      return {
        valid: true,
        errors: [],
        ffmpeg_found: false,
        ytdlp_found: false,
        resolved_library_path: './library/',
        library_exists: false,
        config_path: './config.json',
      };
    }
    if (cmd === 'get_config_path') return './config.json';
    return null;
  }

  // ── Settings load/apply ──────────────────────────────────

  async function loadFromBackend() {
    try {
      var settings = await tauriInvoke('get_settings');
      applyToForm(settings);
      settingsLoaded = true;

      // Sync the header language selector with persisted language
      if (settings.language) {
        var langSelect = document.getElementById('lang-select');
        if (langSelect && langSelect.value !== settings.language) {
          langSelect.value = settings.language;
          if (typeof setLanguage === 'function') {
            setLanguage(settings.language);
          }
        }
      }

      // Sync capture mode on dashboard with saved default
      if (settings.default_capture_mode && typeof setCaptureModeConfig === 'function') {
        setCaptureModeConfig(
          settings.default_capture_mode,
          settings.default_interval_seconds || 30
        );
      }
    } catch (err) {
      console.error('[SettingsUI] Failed to load settings:', err);
    }
  }

  function applyToForm(s) {
    if (!s) return;
    if (inputLibraryPath) inputLibraryPath.value = s.library_path || './library/';
    if (selectQuality) selectQuality.value = s.download_quality || '720';
    if (selectLanguage) selectLanguage.value = s.language || 'ko';
    if (checkboxMp4) checkboxMp4.checked = !!s.mp4_retention;

    // Capture mode defaults
    if (selectCaptureMode) selectCaptureMode.value = s.default_capture_mode || 'subtitle';
    if (inputInterval) inputInterval.value = s.default_interval_seconds || 30;
    if (rangeThreshold) {
      var pct = Math.round((s.scene_change_threshold || 0.30) * 100);
      rangeThreshold.value = pct;
      if (thresholdValueEl) thresholdValueEl.textContent = pct + '%';
    }

    toggleIntervalVisibility();
  }

  function readFromForm() {
    var thresholdPct = rangeThreshold ? parseInt(rangeThreshold.value, 10) : 30;
    return {
      library_path: inputLibraryPath ? inputLibraryPath.value : './library/',
      download_quality: selectQuality ? selectQuality.value : '720',
      language: selectLanguage ? selectLanguage.value : 'ko',
      mp4_retention: checkboxMp4 ? checkboxMp4.checked : false,
      default_capture_mode: selectCaptureMode ? selectCaptureMode.value : 'subtitle',
      default_interval_seconds: inputInterval ? parseInt(inputInterval.value, 10) || 30 : 30,
      scene_change_threshold: thresholdPct / 100,
    };
  }

  // ── System info loading ────────────────────────────────────

  async function loadSystemInfo() {
    try {
      var result = await tauriInvoke('validate_settings');
      if (!result) return;

      // Config path
      var configPathEl = document.getElementById('settings-config-path-value');
      if (configPathEl) {
        configPathEl.textContent = result.config_path || '—';
        configPathEl.title = result.config_path || '';
      }

      // ffmpeg status
      var ffmpegEl = document.getElementById('settings-ffmpeg-status');
      if (ffmpegEl) {
        var ffLabel = result.ffmpeg_found
          ? (typeof t === 'function' ? t('settings_tool_found') : 'Available')
          : (typeof t === 'function' ? t('settings_tool_missing') : 'Not found');
        var ffClass = result.ffmpeg_found ? 'settings-tool-found' : 'settings-tool-missing';
        ffmpegEl.innerHTML = '<span class="settings-tool-badge ' + ffClass + '">' + ffLabel + '</span>';
      }

      // yt-dlp status
      var ytdlpEl = document.getElementById('settings-ytdlp-status');
      if (ytdlpEl) {
        var ytLabel = result.ytdlp_found
          ? (typeof t === 'function' ? t('settings_tool_found') : 'Available')
          : (typeof t === 'function' ? t('settings_tool_missing') : 'Not found');
        var ytClass = result.ytdlp_found ? 'settings-tool-found' : 'settings-tool-missing';
        ytdlpEl.innerHTML = '<span class="settings-tool-badge ' + ytClass + '">' + ytLabel + '</span>';
      }
    } catch (err) {
      console.error('[SettingsUI] Failed to load system info:', err);
    }
  }

  // ── Modal open/close ──────────────────────────────────────

  async function openModal() {
    // Refresh from backend before showing
    try {
      var settings = await tauriInvoke('get_settings');
      applyToForm(settings);
    } catch (err) {
      console.error('[SettingsUI] Refresh failed:', err);
    }

    // Take snapshot for cancel/revert
    snapshot = readFromForm();
    modal.hidden = false;
    modal.setAttribute('aria-hidden', 'false');

    // Load system info (non-blocking)
    loadSystemInfo();

    // Focus the save button for accessibility
    setTimeout(function() {
      if (btnBrowse) btnBrowse.focus();
    }, 60);
  }

  function closeModal() {
    // Revert form to snapshot if user cancelled
    if (snapshot) {
      applyToForm(snapshot);
    }
    modal.hidden = true;
    modal.setAttribute('aria-hidden', 'true');
    snapshot = null;
  }

  // ── Save ──────────────────────────────────────────────────

  async function handleSave() {
    var patch = readFromForm();

    // Validate interval
    if (patch.default_capture_mode === 'interval') {
      var iv = patch.default_interval_seconds;
      if (isNaN(iv) || iv < 1 || iv > 3600) {
        showToast(
          (typeof t === 'function' ? t('interval_custom_hint', { min: 1, max: 3600 }) : 'Interval must be 1-3600'),
          'error'
        );
        return;
      }
    }

    // Visual feedback on save button
    btnSave.disabled = true;
    var originalText = btnSave.textContent;
    btnSave.textContent = (typeof t === 'function' ? t('settings_saving') : null) || '저장 중...';

    try {
      var updated = await tauriInvoke('update_settings', { patch: patch });
      applyToForm(updated);

      // If language changed, apply globally
      var newLang = updated.language || patch.language;
      if (newLang && typeof getLanguage === 'function' && newLang !== getLanguage()) {
        if (typeof setLanguage === 'function') {
          setLanguage(newLang);
        }
        var langSelect = document.getElementById('lang-select');
        if (langSelect) langSelect.value = newLang;
      }

      // Sync capture mode on dashboard with new defaults
      if (updated.default_capture_mode && typeof setCaptureModeConfig === 'function') {
        setCaptureModeConfig(
          updated.default_capture_mode,
          updated.default_interval_seconds || 30
        );
      }

      // Dispatch settingsChanged event for other components
      document.dispatchEvent(new CustomEvent('settingsChanged', {
        detail: updated
      }));

      // Clear snapshot so close doesn't revert
      snapshot = null;
      modal.hidden = true;
      modal.setAttribute('aria-hidden', 'true');
      settingsLoaded = true;

      showToast((typeof t === 'function' ? t('settings_saved') : null) || '설정이 저장되었습니다');
    } catch (err) {
      console.error('[SettingsUI] Save failed:', err);
      var msg = (typeof t === 'function' ? t('settings_save_error') : null) || '설정 저장 실패';
      showToast(msg + ': ' + err, 'error');
    } finally {
      btnSave.disabled = false;
      btnSave.textContent = originalText;
    }
  }

  // ── Reset to defaults ──────────────────────────────────────

  async function handleReset() {
    var confirmMsg = (typeof t === 'function' ? t('settings_reset_confirm') : null) ||
      '모든 설정을 기본값으로 초기화하시겠습니까?';
    if (!confirm(confirmMsg)) return;

    try {
      var defaults = await tauriInvoke('reset_settings');
      applyToForm(defaults);

      // Update snapshot so cancel doesn't revert to old values
      snapshot = readFromForm();

      showToast(
        (typeof t === 'function' ? t('settings_reset_done') : null) || '설정이 기본값으로 초기화되었습니다'
      );
    } catch (err) {
      console.error('[SettingsUI] Reset failed:', err);
      showToast('Reset failed: ' + err, 'error');
    }
  }

  // ── Folder picker ─────────────────────────────────────────

  async function browseFolder() {
    var selected = null;

    try {
      if (window.__TAURI__ && window.__TAURI__.dialog) {
        // Tauri v2 dialog plugin
        selected = await window.__TAURI__.dialog.open({
          directory: true,
          multiple: false,
          title: (typeof t === 'function' ? t('settings_select_folder') : null) || '라이브러리 폴더 선택',
        });
      } else if (window.__TAURI__ && window.__TAURI__.core) {
        // Alternative: invoke plugin directly
        selected = await window.__TAURI__.core.invoke('plugin:dialog|open', {
          options: {
            directory: true,
            multiple: false,
            title: (typeof t === 'function' ? t('settings_select_folder') : null) || '라이브러리 폴더 선택',
          }
        });
      } else {
        // Dev fallback: prompt
        selected = prompt(
          (typeof t === 'function' ? t('settings_enter_path') : null) || '폴더 경로를 입력하세요:',
          inputLibraryPath ? inputLibraryPath.value : './library'
        );
      }
    } catch (err) {
      console.error('[SettingsUI] Folder picker error:', err);
      // Fallback to prompt on error
      selected = prompt(
        (typeof t === 'function' ? t('settings_enter_path') : null) || '폴더 경로를 입력하세요:',
        inputLibraryPath ? inputLibraryPath.value : './library'
      );
    }

    if (selected && inputLibraryPath) {
      inputLibraryPath.value = selected;
    }
  }

  // ── Toast notification ────────────────────────────────────

  function showToast(message, type) {
    // Use global showToast if available (from app.js)
    if (typeof window.showToast === 'function' && window.showToast !== showToast) {
      window.showToast(message, type || 'success');
      return;
    }

    // Remove any existing toast
    var existing = document.querySelector('.toast');
    if (existing) existing.remove();

    var toast = document.createElement('div');
    toast.className = 'toast' + (type === 'error' ? ' toast-error' : ' toast-success');
    toast.textContent = message;
    document.body.appendChild(toast);

    // Trigger show animation
    requestAnimationFrame(function() {
      toast.classList.add('toast-show');
    });

    // Auto-dismiss after 2.5s
    setTimeout(function() {
      toast.classList.remove('toast-show');
      setTimeout(function() {
        if (toast.parentNode) toast.parentNode.removeChild(toast);
      }, 300);
    }, 2500);
  }

  // ── Public API ────────────────────────────────────────────

  return {
    init: init,
    loadSettings: loadFromBackend,
    openModal: openModal,
    getCurrentSettings: readFromForm,
  };
})();

// Legacy compat: expose as global functions for app.js
function initSettings() { SettingsUI.init(); }
function openSettingsModal() { SettingsUI.openModal(); }
function closeSettingsModal() {
  var modal = document.getElementById('settings-modal');
  if (modal) modal.hidden = true;
}
