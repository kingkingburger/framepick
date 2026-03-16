/**
 * FramePick - Internationalization (i18n)
 *
 * Supports Korean (default) and English.
 * Uses data-i18n attributes on DOM elements for automatic translation.
 */

const I18n = (() => {
  const strings = {
    ko: {
      urlLabel: 'YouTube URL',
      addToQueue: '대기열에 추가',
      captureModeLabel: '캡쳐 모드',
      modeSubtitle: '자막 구간별',
      modeScene: '장면 변화',
      modeInterval: '고정 간격',
      intervalLabel: '간격 (초)',
      customInterval: '직접 입력',
      intervalUnit: '초',
      customIntervalHint: '1~3600초 사이 값을 입력하세요',
      queueTitle: '대기열',
      queueEmpty: '대기열이 비어 있습니다',
      langLabel: '한국어',
      errorInvalidUrl: '유효한 YouTube URL을 입력해주세요',
      errorDuplicate: '이미 대기열에 있는 URL입니다',
      errorDuplicateLibrary: '이미 라이브러리에 존재하는 영상입니다',
      toastDuplicateSkipped: '이미 처리된 영상입니다 — 건너뜁니다',
      toastProcessingFailed: '처리 실패',
      toastProcessingFailedDetail: '오류가 발생했습니다',
      statusPending: '대기',
      statusProcessing: '처리 중',
      statusCompleted: '완료',
      statusDone: '완료',
      statusFailed: '오류',
      statusError: '오류',
      statusSkipped: '건너뜀',
      // Pipeline stage labels
      progress_downloading: '다운로드 중',
      progress_extracting_subtitles: '자막 추출 중',
      progress_extracting_frames: '프레임 추출 중',
      progress_generating_slides: '슬라이드 생성 중',
      progress_cleanup: '정리 중',
      progress_done: '완료',
      // Progress display
      progressStage: '단계',
      progressOf: '/',
      queueProgress: '진행 상황',
      queueCompleted: '완료',
      queueFailed: '실패',
      queueRemaining: '남은 항목',
      // Queue actions
      clearCompleted: '완료 항목 지우기',
      retryItem: '재시도',
      removeItem: '삭제',
      viewSlides: '슬라이드 보기',
      cancelItem: '취소',
      // Queue item time
      elapsed: '경과',
      elapsedSec: '초',
      elapsedMin: '분',
      elapsedHour: '시간',
      addedJustNow: '방금 추가됨',
      // Playlist modal
      playlistDetected: '재생목록이 감지되었습니다',
      playlistFetching: '재생목록 정보를 가져오는 중...',
      playlistTitle: '재생목록',
      playlistVideoCount: '개 동영상',
      playlistSelectAll: '전체 선택',
      playlistSelectNone: '전체 해제',
      playlistSelected: '개 선택됨',
      playlistAddSelected: '선택 항목 추가',
      playlistCancel: '취소',
      playlistEmpty: '재생목록에 동영상이 없습니다',
      playlistFetchError: '재생목록을 가져올 수 없습니다',
      playlistDuration: '길이',
      // Library
      libraryTitle: '라이브러리',
      libraryEmpty: '라이브러리가 비어 있습니다',
      libraryLoading: '라이브러리를 불러오는 중...',
      librarySlides: '슬라이드',
      libraryNoSlides: '슬라이드 없음',
      libraryRefresh: '새로고침',
      libraryOpenViewer: '뷰어 열기',
      libraryOpenFolder: '폴더 열기',
      // Settings
      settingsTitle: '설정',
      settingsBtn: '설정',
      settingsLibraryPath: '라이브러리 경로',
      settingsBrowse: '찾아보기',
      settingsLibraryHint: '슬라이드와 캡쳐 이미지가 저장되는 폴더',
      settingsQuality: '다운로드 품질',
      settingsQualityHint: '영상 다운로드 시 최대 해상도',
      settingsLanguage: '인터페이스 언어',
      settingsMp4Retention: 'MP4 파일 보존',
      settingsMp4Hint: '비활성화 시 프레임 캡쳐 후 원본 영상을 삭제하여 디스크 절약',
      settingsCancel: '취소',
      settingsSave: '저장',
      settingsSaving: '저장 중...',
      settingsSaved: '설정이 저장되었습니다',
      settingsSaveError: '설정 저장 실패',
      settingsCaptureGroup: '캡쳐 설정',
      settingsDefaultMode: '기본 캡쳐 모드',
      settingsDefaultModeHint: '새 항목 추가 시 기본으로 선택되는 캡쳐 모드',
      settingsDefaultInterval: '기본 간격 (초)',
      settingsIntervalHint: '고정 간격 모드의 기본 캡쳐 간격 (1~3600초)',
      settingsSceneThreshold: '장면 변화 감도',
      settingsSceneThresholdHint: '낮을수록 더 많은 프레임 캡쳐 (0.01~1.0)',
      settingsDownloadGroup: '다운로드 설정',
      settingsQualityBest: '최고 품질',
      settingsStorageGroup: '저장 경로',
      settingsReset: '초기화',
      settingsResetConfirm: '모든 설정을 초기값으로 되돌리시겠습니까?',
      settingsResetDone: '설정이 초기화되었습니다',
      settingsSystemSection: '시스템 정보',
      settingsConfigPath: '설정 파일 경로',
      settingsFfmpegStatus: 'ffmpeg 상태',
      settingsYtdlpStatus: 'yt-dlp 상태',
      settingsToolFound: '사용 가능',
      settingsToolMissing: '찾을 수 없음',
      // Workflow hint
      workflowHint: 'YouTube URL을 입력하고 시작 버튼을 누르면 프레임 캡쳐가 시작됩니다',
      // Capture list
      captureListTitle: '캡쳐된 프레임',
      captureListEmpty: '캡쳐된 프레임이 없습니다',
      captureListViewSlides: '슬라이드 보기',
      captureEmpty: '캕쳐된 프레임이 없습니다',
      captureFrameCount: '개 프레임',
      captureBack: '뒤로',
      openSlides: '슬라이드 열기',
      captureNoText: '(텍스트 없음)',
      captureFrameOf: '/',
      captureLoading: '프레임을 불러오는 중...',
      openFolder: '폴더 열기',
      // Library delete
      libraryDelete: '삭제',
      libraryDeleteConfirm: '이 항목을 삭제하시겠습니까? 모든 캡쳐 이미지와 슬라이드가 영구적으로 삭제됩니다.',
      libraryDeleteSuccess: '라이브러리 항목이 삭제되었습니다',
      libraryDeleteError: '삭제 실패',
    },
    en: {
      urlLabel: 'YouTube URL',
      addToQueue: 'Add to Queue',
      captureModeLabel: 'Capture Mode',
      modeSubtitle: 'Subtitle-based',
      modeScene: 'Scene Change',
      modeInterval: 'Fixed Interval',
      intervalLabel: 'Interval (sec)',
      customInterval: 'Custom',
      intervalUnit: 'sec',
      customIntervalHint: 'Enter a value between 1 and 3600 seconds',
      queueTitle: 'Queue',
      queueEmpty: 'Queue is empty',
      langLabel: 'English',
      errorInvalidUrl: 'Please enter a valid YouTube URL',
      errorDuplicate: 'This URL is already in the queue',
      errorDuplicateLibrary: 'This video already exists in the library',
      toastDuplicateSkipped: 'Video already processed — skipped',
      toastProcessingFailed: 'Processing failed',
      toastProcessingFailedDetail: 'An error occurred',
      statusPending: 'Pending',
      statusProcessing: 'Processing',
      statusCompleted: 'Done',
      statusDone: 'Done',
      statusFailed: 'Error',
      statusError: 'Error',
      statusSkipped: 'Skipped',
      // Pipeline stage labels
      progress_downloading: 'Downloading',
      progress_extracting_subtitles: 'Extracting subtitles',
      progress_extracting_frames: 'Extracting frames',
      progress_generating_slides: 'Generating slides',
      progress_cleanup: 'Cleaning up',
      progress_done: 'Done',
      // Progress display
      progressStage: 'Stage',
      progressOf: '/',
      queueProgress: 'Progress',
      queueCompleted: 'Completed',
      queueFailed: 'Failed',
      queueRemaining: 'Remaining',
      // Queue actions
      clearCompleted: 'Clear completed',
      retryItem: 'Retry',
      removeItem: 'Remove',
      viewSlides: 'View Slides',
      cancelItem: 'Cancel',
      // Queue item time
      elapsed: 'elapsed',
      elapsedSec: 's',
      elapsedMin: 'm',
      elapsedHour: 'h',
      addedJustNow: 'Just added',
      // Playlist modal
      playlistDetected: 'Playlist detected',
      playlistFetching: 'Fetching playlist info...',
      playlistTitle: 'Playlist',
      playlistVideoCount: ' videos',
      playlistSelectAll: 'Select All',
      playlistSelectNone: 'Deselect All',
      playlistSelected: ' selected',
      playlistAddSelected: 'Add Selected',
      playlistCancel: 'Cancel',
      playlistEmpty: 'No videos found in playlist',
      playlistFetchError: 'Failed to fetch playlist',
      playlistDuration: 'Duration',
      // Library
      libraryTitle: 'Library',
      libraryEmpty: 'Library is empty',
      libraryLoading: 'Loading library...',
      librarySlides: 'slides',
      libraryNoSlides: 'No slides',
      libraryRefresh: 'Refresh',
      libraryOpenViewer: 'Open Viewer',
      libraryOpenFolder: 'Open Folder',
      // Settings
      settingsTitle: 'Settings',
      settingsBtn: 'Settings',
      settingsLibraryPath: 'Library Path',
      settingsBrowse: 'Browse',
      settingsLibraryHint: 'Folder where slides and captured images are stored',
      settingsQuality: 'Download Quality',
      settingsQualityHint: 'Maximum resolution for video downloads',
      settingsLanguage: 'Interface Language',
      settingsMp4Retention: 'Keep MP4 Files',
      settingsMp4Hint: 'When disabled, source videos are deleted after frame capture to save disk space',
      settingsCancel: 'Cancel',
      settingsSave: 'Save',
      settingsSaving: 'Saving...',
      settingsSaved: 'Settings saved successfully',
      settingsSaveError: 'Failed to save settings',
      settingsCaptureGroup: 'Capture Settings',
      settingsDefaultMode: 'Default Capture Mode',
      settingsDefaultModeHint: 'Default capture mode when adding new items',
      settingsDefaultInterval: 'Default Interval (sec)',
      settingsIntervalHint: 'Default capture interval for fixed interval mode (1-3600 sec)',
      settingsSceneThreshold: 'Scene Change Sensitivity',
      settingsSceneThresholdHint: 'Lower values capture more frames (0.01-1.0)',
      settingsDownloadGroup: 'Download Settings',
      settingsQualityBest: 'Best Quality',
      settingsStorageGroup: 'Storage Path',
      settingsReset: 'Reset',
      settingsResetConfirm: 'Reset all settings to defaults?',
      settingsResetDone: 'Settings have been reset',
      settingsSystemSection: 'System Info',
      settingsConfigPath: 'Config File Path',
      settingsFfmpegStatus: 'ffmpeg Status',
      settingsYtdlpStatus: 'yt-dlp Status',
      settingsToolFound: 'Available',
      settingsToolMissing: 'Not found',
      // Workflow hint
      workflowHint: 'Enter a YouTube URL and click Start to begin frame capture',
      // Capture list
      captureListTitle: 'Captured Frames',
      captureListEmpty: 'No frames captured',
      captureListViewSlides: 'View Slides',
      captureEmpty: 'No captured frames',
      captureFrameCount: ' frames',
      captureBack: 'Back',
      openSlides: 'Open Slides',
      captureNoText: '(no text)',
      captureFrameOf: '/',
      captureLoading: 'Loading frames...',
      openFolder: 'Open Folder',
      // Library delete
      libraryDelete: 'Delete',
      libraryDeleteConfirm: 'Delete this item? All captured images and slides will be permanently removed.',
      libraryDeleteSuccess: 'Library item deleted',
      libraryDeleteError: 'Failed to delete',
    },
  };

  let currentLang = 'ko';

  /** Get a translated string by key. */
  function t(key) {
    return (strings[currentLang] && strings[currentLang][key]) || key;
  }

  /** Apply translations to all elements with data-i18n attribute. */
  function applyToDOM() {
    document.querySelectorAll('[data-i18n]').forEach((el) => {
      const key = el.getAttribute('data-i18n');
      el.textContent = t(key);
    });
    // Update title attributes
    document.querySelectorAll('[data-i18n-title]').forEach((el) => {
      const key = el.getAttribute('data-i18n-title');
      el.title = t(key);
    });
    // Update placeholder attributes
    document.querySelectorAll('[data-i18n-placeholder]').forEach((el) => {
      const key = el.getAttribute('data-i18n-placeholder');
      el.placeholder = t(key);
    });
    // Update lang label button
    const langLabel = document.getElementById('lang-label');
    if (langLabel) langLabel.textContent = t('langLabel');
  }

  /** Set the active language and update the DOM. */
  function setLanguage(lang) {
    if (!strings[lang]) return;
    currentLang = lang;
    document.documentElement.lang = lang;
    applyToDOM();
  }

  /** Get current language code. */
  function getLanguage() {
    return currentLang;
  }

  /** Toggle between ko and en. */
  function toggle() {
    const next = currentLang === 'ko' ? 'en' : 'ko';
    setLanguage(next);
    return next;
  }

  return { t, applyToDOM, setLanguage, getLanguage, toggle };
})();
