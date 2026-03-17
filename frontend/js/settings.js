/**
 * @file settings.js
 * @description framepick 설정 UI 모듈
 *
 * 설정 모달을 Tauri 백엔드 명령과 연결한다:
 *   - get_settings: config.json에서 현재 설정 로드
 *   - update_settings: config.json에 변경사항 저장
 *   - validate_settings: 도구 가용성 및 경로 확인
 *   - reset_settings: 모든 설정을 기본값으로 복원
 * 네이티브 폴더 선택기를 위해 Tauri dialog 플러그인을 사용한다.
 */

const SettingsUI = (() => {
  // DOM 참조 (init에서 결정됨)
  let modal, btnOpen, btnClose, btnCancel, btnSave, btnBrowse, btnReset;
  let inputLibraryPath, selectQuality, selectLanguage, checkboxMp4;
  // 캡쳐 모드 컨트롤
  let selectCaptureMode, inputInterval, rangeThreshold, thresholdValueEl;
  let intervalGroup;

  // 모달 열릴 때의 설정 스냅샷 (취소/되돌리기용)
  let snapshot = null;
  let settingsLoaded = false;

  /**
   * 설정 UI를 초기화한다. DOMContentLoaded 후 한 번 호출한다.
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

    // 캡쳐 모드 컨트롤
    selectCaptureMode = document.getElementById('settings-capture-mode');
    inputInterval = document.getElementById('settings-interval');
    intervalGroup = document.getElementById('settings-interval-group');
    rangeThreshold = document.getElementById('settings-scene-threshold');
    thresholdValueEl = document.getElementById('settings-threshold-value');

    if (!modal || !btnOpen) {
      console.warn('Settings UI: modal or open button not found');
      return;
    }

    // 이벤트 바인딩
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

    // 캡쳐 모드 변경 → 간격 그룹 표시/숨김
    if (selectCaptureMode) {
      selectCaptureMode.addEventListener('change', function() {
        toggleIntervalVisibility();
      });
    }

    // 임계값 슬라이더 → 값 표시 업데이트
    if (rangeThreshold && thresholdValueEl) {
      rangeThreshold.addEventListener('input', function() {
        thresholdValueEl.textContent = rangeThreshold.value + '%';
      });
    }

    // 오버레이 클릭 시 닫기
    modal.addEventListener('click', function(e) {
      if (e.target === modal) closeModal();
    });

    // Escape 키로 닫기
    document.addEventListener('keydown', function(e) {
      if (e.key === 'Escape' && !modal.hidden) {
        e.preventDefault();
        closeModal();
      }
    });

    // 앱 시작 시 백엔드에서 초기 설정 로드
    loadFromBackend();
  }

  /** 선택된 캡쳐 모드에 따라 간격 입력 필드를 표시/숨긴다 */
  function toggleIntervalVisibility() {
    if (!selectCaptureMode || !intervalGroup) return;
    var isInterval = selectCaptureMode.value === 'interval';
    intervalGroup.style.display = isInterval ? 'block' : 'none';
  }

  // ── Tauri invoke 헬퍼 ──────────────────────────────────

  function tauriInvoke(cmd, args) {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    // 개발 환경 폴백 (목 데이터 사용)
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

  // ── 설정 로드/적용 ──────────────────────────────────

  async function loadFromBackend() {
    try {
      var settings = await tauriInvoke('get_settings');
      applyToForm(settings);
      settingsLoaded = true;

      // 헤더 언어 선택기를 저장된 언어와 동기화
      if (settings.language) {
        var langSelect = document.getElementById('lang-select');
        if (langSelect && langSelect.value !== settings.language) {
          langSelect.value = settings.language;
          if (typeof setLanguage === 'function') {
            setLanguage(settings.language);
          }
        }
      }

      // 저장된 기본값으로 대시보드 캡쳐 모드 동기화
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

    // 캡쳐 모드 기본값
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

  // ── 시스템 정보 로드 ────────────────────────────────────

  async function loadSystemInfo() {
    try {
      var result = await tauriInvoke('validate_settings');
      if (!result) return;

      // 설정 파일 경로
      var configPathEl = document.getElementById('settings-config-path-value');
      if (configPathEl) {
        configPathEl.textContent = result.config_path || '—';
        configPathEl.title = result.config_path || '';
      }

      // ffmpeg 상태
      var ffmpegEl = document.getElementById('settings-ffmpeg-status');
      if (ffmpegEl) {
        var ffLabel = result.ffmpeg_found
          ? (typeof t === 'function' ? t('settings_tool_found') : 'Available')
          : (typeof t === 'function' ? t('settings_tool_missing') : 'Not found');
        var ffClass = result.ffmpeg_found ? 'settings-tool-found' : 'settings-tool-missing';
        ffmpegEl.innerHTML = '<span class="settings-tool-badge ' + ffClass + '">' + ffLabel + '</span>';
      }

      // yt-dlp 상태
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

  // ── 모달 열기/닫기 ──────────────────────────────────────

  async function openModal() {
    // 표시 전 백엔드에서 새로고침
    try {
      var settings = await tauriInvoke('get_settings');
      applyToForm(settings);
    } catch (err) {
      console.error('[SettingsUI] Refresh failed:', err);
    }

    // 취소/되돌리기용 스냅샷 저장
    snapshot = readFromForm();
    modal.hidden = false;
    modal.setAttribute('aria-hidden', 'false');

    // 시스템 정보 로드 (비차단)
    loadSystemInfo();

    // 접근성을 위해 저장 버튼에 포커스
    setTimeout(function() {
      if (btnBrowse) btnBrowse.focus();
    }, 60);
  }

  function closeModal() {
    // 취소 시 스냅샷으로 폼 되돌리기
    if (snapshot) {
      applyToForm(snapshot);
    }
    modal.hidden = true;
    modal.setAttribute('aria-hidden', 'true');
    snapshot = null;
  }

  // ── 저장 ──────────────────────────────────────────────────

  async function handleSave() {
    var patch = readFromForm();

    // 간격값 유효성 검사
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

    // 저장 버튼 시각적 피드백
    btnSave.disabled = true;
    var originalText = btnSave.textContent;
    btnSave.textContent = (typeof t === 'function' ? t('settings_saving') : null) || '저장 중...';

    try {
      var updated = await tauriInvoke('update_settings', { patch: patch });
      applyToForm(updated);

      // 언어가 변경된 경우 전역으로 적용
      var newLang = updated.language || patch.language;
      if (newLang && typeof getLanguage === 'function' && newLang !== getLanguage()) {
        if (typeof setLanguage === 'function') {
          setLanguage(newLang);
        }
        var langSelect = document.getElementById('lang-select');
        if (langSelect) langSelect.value = newLang;
      }

      // 새 기본값으로 대시보드 캡쳐 모드 동기화
      if (updated.default_capture_mode && typeof setCaptureModeConfig === 'function') {
        setCaptureModeConfig(
          updated.default_capture_mode,
          updated.default_interval_seconds || 30
        );
      }

      // 다른 컴포넌트를 위한 settingsChanged 이벤트 발행
      document.dispatchEvent(new CustomEvent('settingsChanged', {
        detail: updated
      }));

      // 닫을 때 되돌리지 않도록 스냅샷 초기화
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

  // ── 기본값으로 초기화 ──────────────────────────────────────

  async function handleReset() {
    var confirmMsg = (typeof t === 'function' ? t('settings_reset_confirm') : null) ||
      '모든 설정을 기본값으로 초기화하시겠습니까?';
    if (!confirm(confirmMsg)) return;

    try {
      var defaults = await tauriInvoke('reset_settings');
      applyToForm(defaults);

      // 취소 시 이전 값으로 되돌리지 않도록 스냅샷 업데이트
      snapshot = readFromForm();

      showToast(
        (typeof t === 'function' ? t('settings_reset_done') : null) || '설정이 기본값으로 초기화되었습니다'
      );
    } catch (err) {
      console.error('[SettingsUI] Reset failed:', err);
      showToast('Reset failed: ' + err, 'error');
    }
  }

  // ── 폴더 선택기 ─────────────────────────────────────────

  async function browseFolder() {
    var selected = null;

    try {
      if (window.__TAURI__ && window.__TAURI__.dialog) {
        // Tauri v2 dialog 플러그인
        selected = await window.__TAURI__.dialog.open({
          directory: true,
          multiple: false,
          title: (typeof t === 'function' ? t('settings_select_folder') : null) || '라이브러리 폴더 선택',
        });
      } else if (window.__TAURI__ && window.__TAURI__.core) {
        // 대안: 플러그인 직접 호출
        selected = await window.__TAURI__.core.invoke('plugin:dialog|open', {
          options: {
            directory: true,
            multiple: false,
            title: (typeof t === 'function' ? t('settings_select_folder') : null) || '라이브러리 폴더 선택',
          }
        });
      } else {
        // 개발 환경 폴백: prompt 사용
        selected = prompt(
          (typeof t === 'function' ? t('settings_enter_path') : null) || '폴더 경로를 입력하세요:',
          inputLibraryPath ? inputLibraryPath.value : './library'
        );
      }
    } catch (err) {
      console.error('[SettingsUI] Folder picker error:', err);
      // 오류 시 prompt로 폴백
      selected = prompt(
        (typeof t === 'function' ? t('settings_enter_path') : null) || '폴더 경로를 입력하세요:',
        inputLibraryPath ? inputLibraryPath.value : './library'
      );
    }

    if (selected && inputLibraryPath) {
      inputLibraryPath.value = selected;
    }
  }

  // ── 토스트 알림 ────────────────────────────────────────

  function showToast(message, type) {
    // 전역 showToast가 있으면 사용 (app.js에서)
    if (typeof window.showToast === 'function' && window.showToast !== showToast) {
      window.showToast(message, type || 'success');
      return;
    }

    // 기존 토스트 제거
    var existing = document.querySelector('.toast');
    if (existing) existing.remove();

    var toast = document.createElement('div');
    toast.className = 'toast' + (type === 'error' ? ' toast-error' : ' toast-success');
    toast.textContent = message;
    document.body.appendChild(toast);

    // 표시 애니메이션 트리거
    requestAnimationFrame(function() {
      toast.classList.add('toast-show');
    });

    // 2.5초 후 자동 닫기
    setTimeout(function() {
      toast.classList.remove('toast-show');
      setTimeout(function() {
        if (toast.parentNode) toast.parentNode.removeChild(toast);
      }, 300);
    }, 2500);
  }

  // ── 공개 API ────────────────────────────────────────────

  return {
    init: init,
    loadSettings: loadFromBackend,
    openModal: openModal,
    getCurrentSettings: readFromForm,
  };
})();

// 하위 호환성: app.js용 전역 함수로 노출
function initSettings() { SettingsUI.init(); }
function openSettingsModal() { SettingsUI.openModal(); }
function closeSettingsModal() {
  var modal = document.getElementById('settings-modal');
  if (modal) modal.hidden = true;
}
