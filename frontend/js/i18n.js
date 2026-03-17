/**
 * @file i18n.js
 * @description framepick 다국어 지원 모듈
 *
 * 기본 언어: 한국어(ko), 영어(en) 지원
 * 제공 기능:
 *  - t(key, params): 현재 언어로 번역된 문자열 반환 (파라미터 보간 지원)
 *  - setLanguage(lang): UI 전체 언어를 변경하고 data-i18n 요소를 일괄 업데이트
 *  - getLanguage(): 현재 언어 코드 반환
 */
const I18N = {
  ko: {
    // Capture mode
    'capture_mode_label': '캡쳐 모드',
    'capture_mode_subtitle': '자막 구간별',
    'capture_mode_scene': '장면 변화 감지',
    'capture_mode_interval': '고정 간격',
    'capture_mode_subtitle_desc': '각 자막 세그먼트 시작 시점에서 프레임을 캡쳐합니다',
    'capture_mode_scene_desc': '화면 변화도 30% 이상일 때 프레임을 캡쳐합니다',
    'capture_mode_interval_desc': '설정된 간격(초)마다 프레임을 캡쳐합니다',
    // Interval options
    'interval_seconds': '{n}초마다',
    'interval_label': '캡쳐 간격',
    'interval_custom': '직접 입력',
    'interval_custom_placeholder': '초',
    'interval_custom_hint': '{min}~{max}초 사이 값을 입력하세요',
    'interval_unit_seconds': '초',
    // General
    'app_title': 'FramePick',
    'url_placeholder': 'YouTube URL을 입력하세요',
    'start_button': '시작',
    'url_validating': '확인 중...',
    'url_valid': '유효한 YouTube URL입니다',
    'url_invalid': '올바른 YouTube URL을 입력해 주세요',
    'language': '언어',
    // Settings
    'settings_title': '설정',
    'settings_library_path': '라이브러리 경로',
    'settings_browse': '찾아보기',
    'settings_library_hint': '슬라이드와 캡쳐 이미지가 저장되는 폴더',
    'settings_quality': '다운로드 품질',
    'settings_quality_hint': '영상 다운로드 시 최대 해상도',
    'settings_language': '인터페이스 언어',
    'settings_mp4_retention': 'MP4 파일 보존',
    'settings_mp4_hint': '비활성화 시 프레임 캡쳐 후 원본 영상을 삭제하여 디스크 절약',
    'settings_cancel': '취소',
    'settings_save': '저장',
    'settings_saving': '저장 중...',
    'settings_saved': '설정이 저장되었습니다',
    'settings_save_error': '설정 저장 실패',
    'settings_select_folder': '라이브러리 폴더 선택',
    'settings_enter_path': '폴더 경로를 입력하세요:',
    // Settings — capture defaults
    'settings_capture_section': '캡쳐 기본값',
    'settings_default_capture_mode': '기본 캡쳐 모드',
    'settings_default_capture_mode_hint': '새 작업의 기본 캡쳐 방식',
    'settings_default_interval': '기본 캡쳐 간격',
    'settings_default_interval_hint': '고정 간격 모드 사용 시 기본 초 단위',
    'settings_scene_threshold': '장면 변화 임계값',
    'settings_scene_threshold_hint': '낮을수록 민감하게 장면 변화를 감지합니다 (기본: 30%)',
    'settings_scene_threshold_value': '{n}%',
    // Settings — system info
    'settings_system_section': '시스템 정보',
    'settings_config_path': '설정 파일 경로',
    'settings_ffmpeg_status': 'ffmpeg 상태',
    'settings_ytdlp_status': 'yt-dlp 상태',
    'settings_tool_found': '사용 가능',
    'settings_tool_missing': '찾을 수 없음',
    'settings_reset': '기본값으로 초기화',
    'settings_reset_confirm': '모든 설정을 기본값으로 초기화하시겠습니까?',
    'settings_reset_done': '설정이 기본값으로 초기화되었습니다',
    // Library
    'library_title': '라이브러리',
    'library_refresh': '새로고침',
    'library_empty': '아직 생성된 슬라이드가 없습니다',
    'library_slides': '{n}개 슬라이드',
    'library_delete': '삭제',
    'library_delete_confirm': '정말로 이 항목을 삭제하시겠습니까? 모든 관련 파일이 영구적으로 삭제됩니다.',
    'library_delete_success': '항목이 삭제되었습니다',
    'library_delete_error': '항목 삭제 실패',
    'library_deleting': '삭제 중...',
    'btn_cancel': '취소',
    'library_open_folder': '폴더 열기',
    'library_open_folder_error': '폴더를 열 수 없습니다',
    'library_open_viewer': '뷰어에서 열기',
    'library_open_browser': '브라우저에서 열기',
    'library_open_browser_failed': '브라우저에서 열 수 없습니다',
    'library_item_count': '{n}개 항목',
    // Viewer
    'viewer_loading': '로딩 중...',
    'viewer_back': '뒤로',
    'viewer_error': '슬라이드를 불러올 수 없습니다',
    'viewer_error_title': '오류',
    'viewer_open_external': '브라우저에서 열기',
    'viewer_slide_count': '{n}개 슬라이드',
    'viewer_retry': '다시 시도',
    'viewer_open_failed': '외부 브라우저에서 열 수 없습니다',
    // Queue
    'queue_title': '처리 대기열',
    'queue_count': '총 {n}개 / 완료 {done}개',
    'queue_clear': '완료 항목 정리',
    'queue_remove': '삭제',
    'queue_steps': '단계',
    'queue_status_pending': '대기 중',
    'queue_status_done': '완료',
    'queue_error_unknown': '알 수 없는 오류',
    'queue_stage_download': '다운로드 중',
    'queue_stage_subtitle': '자막 가져오는 중',
    'queue_stage_capture': '프레임 캡쳐 중',
    'queue_stage_generate': '슬라이드 생성 중',
    'queue_stage_cleanup': '정리 중',
    'queue_position': '대기 #{n}',
    'queue_retry': '다시 시도',
    'queue_retrying': '다시 시도 중...',
    'queue_elapsed': '경과 시간',
    'queue_status_failed': '실패',
    'queue_status_skipped': '건너뜀',
    'error_processing_failed': '처리 실패: {title} — {error}',
    'error_processing_failed_short': '처리 실패: {error}',
    // Queue / errors
    'error_invalid_url': '유효한 YouTube URL을 입력해주세요',
    'error_duplicate': '이미 대기열에 있는 URL입니다',
    'error_already_exists': '이미 라이브러리에 존재하는 영상입니다',
    'queue_added': '대기열에 추가되었습니다',
    // Pipeline progress stages (from backend events)
    'progress_downloading': '영상 다운로드 중',
    'progress_extracting_subtitles': '자막 추출 중',
    'progress_extracting_frames': '프레임 추출 중',
    'progress_generating_slides': '슬라이드 생성 중',
    'progress_cleanup': '정리 중',
    'progress_done': '완료',
    'progress_error': '오류 발생',
    'progress_stage_of': '{current}/{total} 단계',
    // Capture mode fallback
    'fallback_no_subtitles': '자막이 없어 장면 변화 감지 모드로 자동 전환되었습니다',
    'fallback_subtitle_check_error': '자막 확인 실패로 장면 변화 감지 모드로 전환되었습니다',
    'fallback_no_suitable_language': '적합한 자막 언어를 찾을 수 없어 장면 변화 감지 모드로 전환되었습니다',
    'fallback_notification_title': '캡쳐 모드 변경',
    // Subtitle language selection
    'subtitle_lang_korean_manual': '한국어 수동 자막 선택됨',
    'subtitle_lang_korean_auto': '한국어 자동 생성 자막 선택됨',
    'subtitle_lang_english_manual': '영어 수동 자막 선택됨 (한국어 없음)',
    'subtitle_lang_english_auto': '영어 자동 생성 자막 선택됨 (한국어 없음)',
    'subtitle_lang_other_manual': '기타 언어 수동 자막 선택됨 ({lang})',
    'subtitle_lang_other_auto': '기타 언어 자동 생성 자막 선택됨 ({lang})',
    'subtitle_lang_selected': '자막 언어: {lang}',
    'subtitle_lang_priority': '자막 우선순위: 한국어 → 영어 → 기타',
    // Re-capture
    'recapture_title': '다시 캡쳐',
    'recapture_desc': '다른 캡쳐 모드로 프레임을 다시 추출합니다',
    'recapture_mode_label': '캡쳐 모드 선택',
    'recapture_start': '다시 캡쳐',
    'recapture_cancel': '취소',
    'recapture_processing': '캡쳐 중...',
    'recapture_success': '{n}개 프레임이 캡쳐되었습니다',
    'recapture_error': '다시 캡쳐 실패',
    'recapture_no_video': '원본 영상 파일이 없습니다. 먼저 영상을 다시 다운로드하세요.',
    'recapture_btn': '다시 캡쳐',
    'recapture_checking': '확인 중...',
    'recapture_frames_cleared': '기존 프레임을 삭제하고 다시 캡쳐합니다',
    'recapture_confirm': '기존 {n}개 프레임이 새로 캡쳐한 프레임으로 대체됩니다. 계속하시겠습니까?',
    // Playlist selection
    'playlist_title': '재생목록 영상 선택',
    'playlist_loading': '재생목록 정보를 가져오는 중...',
    'playlist_fetch_error': '재생목록 정보를 가져올 수 없습니다',
    'playlist_select_all': '전체 선택',
    'playlist_selected_count': '{selected}/{total}개 선택',
    'playlist_add_selected': '선택한 영상 추가 ({n})',
    'playlist_cancel': '취소',
    'playlist_no_videos': '재생목록에 영상이 없습니다',
    'playlist_untitled': '제목 없음',
    'playlist_detected': '재생목록이 감지되었습니다',
    'playlist_videos_added': '{n}개 영상이 대기열에 추가되었습니다',
    'playlist_skipped_existing': '{n}개 영상이 이미 존재하여 건너뛰었습니다',
    // Queue batch operations (from playlist selection → queue bridge)
    'queue_added_batch': '{n}개 영상이 대기열에 추가되었습니다',
    'queue_added_partial': '{added}개 추가, {skipped}개 건너뜀 (중복/기존)',
    'error_all_duplicates': '모든 선택 영상이 이미 대기열에 있습니다',
    // Capture list
    'capture_list_title': '캡쳐된 프레임',
    'capture_list_count': '{n}개 프레임',
    'capture_list_loading': '프레임 로딩 중...',
    'capture_list_view_slides': '슬라이드 보기',
    'capture_list_collapse': '접기',
    'capture_list_expand': '펼치기',
    'capture_list_empty': '캡쳐된 프레임이 없습니다',
    // Workflow hint
    'workflow_hint': 'YouTube URL을 입력하고 시작 버튼을 누르면 프레임 캡쳐가 시작됩니다',
    // Tools setup overlay
    'tools_setup_title': '도구 설치 중...',
    'tools_setup_message': '필요한 도구를 다운로드하는 중입니다',
    'tools_setup_checking': '확인 중...',
    'tools_setup_downloading': '다운로드 중... {pct}%',
    'tools_setup_extracting': '압축 해제 중...',
    'tools_setup_ready': '준비 완료',
    'tools_setup_error': '도구 설치 실패: {error}',
    'tools_update_available': 'yt-dlp 업데이트 사용 가능 ({latest})',
  },
  en: {
    // Capture mode
    'capture_mode_label': 'Capture Mode',
    'capture_mode_subtitle': 'Subtitle-based',
    'capture_mode_scene': 'Scene Change',
    'capture_mode_interval': 'Fixed Interval',
    'capture_mode_subtitle_desc': 'Captures frame at the start of each subtitle segment',
    'capture_mode_scene_desc': 'Captures frame when scene change exceeds 30%',
    'capture_mode_interval_desc': 'Captures frame at fixed time intervals (seconds)',
    // Interval options
    'interval_seconds': 'Every {n}s',
    'interval_label': 'Capture Interval',
    'interval_custom': 'Custom',
    'interval_custom_placeholder': 'sec',
    'interval_custom_hint': 'Enter a value between {min} and {max} seconds',
    'interval_unit_seconds': 'sec',
    // General
    'app_title': 'FramePick',
    'url_placeholder': 'Enter YouTube URL',
    'start_button': 'Start',
    'url_validating': 'Validating...',
    'url_valid': 'Valid YouTube URL',
    'url_invalid': 'Please enter a valid YouTube URL',
    'language': 'Language',
    // Settings
    'settings_title': 'Settings',
    'settings_library_path': 'Library Path',
    'settings_browse': 'Browse',
    'settings_library_hint': 'Folder where slides and captured images are stored',
    'settings_quality': 'Download Quality',
    'settings_quality_hint': 'Maximum resolution for video downloads',
    'settings_language': 'Interface Language',
    'settings_mp4_retention': 'Keep MP4 Files',
    'settings_mp4_hint': 'When disabled, source videos are deleted after frame capture to save disk space',
    'settings_cancel': 'Cancel',
    'settings_save': 'Save',
    'settings_saving': 'Saving...',
    'settings_saved': 'Settings saved successfully',
    'settings_save_error': 'Failed to save settings',
    'settings_select_folder': 'Select Library Folder',
    'settings_enter_path': 'Enter folder path:',
    // Settings — capture defaults
    'settings_capture_section': 'Capture Defaults',
    'settings_default_capture_mode': 'Default Capture Mode',
    'settings_default_capture_mode_hint': 'Default capture strategy for new jobs',
    'settings_default_interval': 'Default Capture Interval',
    'settings_default_interval_hint': 'Default seconds for fixed-interval mode',
    'settings_scene_threshold': 'Scene Change Threshold',
    'settings_scene_threshold_hint': 'Lower values detect more scene changes (default: 30%)',
    'settings_scene_threshold_value': '{n}%',
    // Settings — system info
    'settings_system_section': 'System Info',
    'settings_config_path': 'Config File Path',
    'settings_ffmpeg_status': 'ffmpeg Status',
    'settings_ytdlp_status': 'yt-dlp Status',
    'settings_tool_found': 'Available',
    'settings_tool_missing': 'Not found',
    'settings_reset': 'Reset to Defaults',
    'settings_reset_confirm': 'Reset all settings to defaults?',
    'settings_reset_done': 'Settings have been reset to defaults',
    // Library
    'library_title': 'Library',
    'library_refresh': 'Refresh',
    'library_empty': 'No slides generated yet',
    'library_slides': '{n} slides',
    'library_delete': 'Delete',
    'library_delete_confirm': 'Are you sure you want to delete this item? All associated files will be permanently removed.',
    'library_delete_success': 'Item deleted successfully',
    'library_delete_error': 'Failed to delete item',
    'library_deleting': 'Deleting...',
    'btn_cancel': 'Cancel',
    'library_open_folder': 'Open Folder',
    'library_open_folder_error': 'Could not open folder',
    'library_open_viewer': 'Open in Viewer',
    'library_open_browser': 'Open in Browser',
    'library_open_browser_failed': 'Could not open in browser',
    'library_item_count': '{n} items',
    // Viewer
    'viewer_loading': 'Loading...',
    'viewer_back': 'Back',
    'viewer_error': 'Failed to load slides',
    'viewer_error_title': 'Error',
    'viewer_open_external': 'Open in Browser',
    'viewer_slide_count': '{n} slides',
    'viewer_retry': 'Retry',
    'viewer_open_failed': 'Could not open in external browser',
    // Queue
    'queue_title': 'Processing Queue',
    'queue_count': '{n} total / {done} done',
    'queue_clear': 'Clear completed',
    'queue_remove': 'Remove',
    'queue_steps': 'steps',
    'queue_status_pending': 'Pending',
    'queue_status_done': 'Done',
    'queue_error_unknown': 'Unknown error',
    'queue_stage_download': 'Downloading',
    'queue_stage_subtitle': 'Fetching subtitles',
    'queue_stage_capture': 'Capturing frames',
    'queue_stage_generate': 'Generating slides',
    'queue_stage_cleanup': 'Cleaning up',
    'queue_position': 'Queue #{n}',
    'queue_retry': 'Retry',
    'queue_retrying': 'Retrying...',
    'queue_elapsed': 'Elapsed time',
    'queue_status_failed': 'Failed',
    'queue_status_skipped': 'Skipped',
    'error_processing_failed': 'Processing failed: {title} — {error}',
    'error_processing_failed_short': 'Processing failed: {error}',
    // Queue / errors
    'error_invalid_url': 'Please enter a valid YouTube URL',
    'error_duplicate': 'This URL is already in the queue',
    'error_already_exists': 'This video already exists in the library',
    'queue_added': 'Added to queue',
    // Pipeline progress stages (from backend events)
    'progress_downloading': 'Downloading video',
    'progress_extracting_subtitles': 'Extracting subtitles',
    'progress_extracting_frames': 'Extracting frames',
    'progress_generating_slides': 'Generating slides',
    'progress_cleanup': 'Cleaning up',
    'progress_done': 'Done',
    'progress_error': 'Error occurred',
    'progress_stage_of': 'Stage {current}/{total}',
    // Capture mode fallback
    'fallback_no_subtitles': 'No subtitles available — automatically switched to scene-change mode',
    'fallback_subtitle_check_error': 'Subtitle check failed — switched to scene-change mode',
    'fallback_no_suitable_language': 'No suitable subtitle language found — switched to scene-change mode',
    'fallback_notification_title': 'Capture Mode Changed',
    // Subtitle language selection
    'subtitle_lang_korean_manual': 'Korean manual subtitles selected',
    'subtitle_lang_korean_auto': 'Korean auto-generated subtitles selected',
    'subtitle_lang_english_manual': 'English manual subtitles selected (no Korean available)',
    'subtitle_lang_english_auto': 'English auto-generated subtitles selected (no Korean available)',
    'subtitle_lang_other_manual': 'Manual subtitles selected ({lang})',
    'subtitle_lang_other_auto': 'Auto-generated subtitles selected ({lang})',
    'subtitle_lang_selected': 'Subtitle language: {lang}',
    'subtitle_lang_priority': 'Subtitle priority: Korean → English → Other',
    // Re-capture
    'recapture_title': 'Re-capture',
    'recapture_desc': 'Re-extract frames with a different capture mode',
    'recapture_mode_label': 'Select capture mode',
    'recapture_start': 'Re-capture',
    'recapture_cancel': 'Cancel',
    'recapture_processing': 'Capturing...',
    'recapture_success': '{n} frames captured',
    'recapture_error': 'Re-capture failed',
    'recapture_no_video': 'Source video file not found. Re-download the video first.',
    'recapture_btn': 'Re-capture',
    'recapture_checking': 'Checking...',
    'recapture_frames_cleared': 'Clearing existing frames and re-capturing',
    'recapture_confirm': 'Existing {n} frames will be replaced with new captures. Continue?',
    // Playlist selection
    'playlist_title': 'Select Playlist Videos',
    'playlist_loading': 'Fetching playlist info...',
    'playlist_fetch_error': 'Failed to fetch playlist info',
    'playlist_select_all': 'Select All',
    'playlist_selected_count': '{selected}/{total} selected',
    'playlist_add_selected': 'Add Selected ({n})',
    'playlist_cancel': 'Cancel',
    'playlist_no_videos': 'No videos in playlist',
    'playlist_untitled': 'Untitled',
    'playlist_detected': 'Playlist detected',
    'playlist_videos_added': '{n} videos added to queue',
    'playlist_skipped_existing': '{n} videos skipped (already exist)',
    // Queue batch operations (from playlist selection → queue bridge)
    'queue_added_batch': '{n} videos added to queue',
    'queue_added_partial': '{added} added, {skipped} skipped (duplicate/existing)',
    'error_all_duplicates': 'All selected videos are already in the queue',
    // Capture list
    'capture_list_title': 'Captured Frames',
    'capture_list_count': '{n} frames',
    'capture_list_loading': 'Loading frames...',
    'capture_list_view_slides': 'View Slides',
    'capture_list_collapse': 'Collapse',
    'capture_list_expand': 'Expand',
    'capture_list_empty': 'No frames captured',
    // Workflow hint
    'workflow_hint': 'Enter a YouTube URL and click Start to begin frame capture',
    // Tools setup overlay
    'tools_setup_title': 'Installing tools...',
    'tools_setup_message': 'Downloading required tools',
    'tools_setup_checking': 'Checking...',
    'tools_setup_downloading': 'Downloading... {pct}%',
    'tools_setup_extracting': 'Extracting...',
    'tools_setup_ready': 'Ready',
    'tools_setup_error': 'Tool installation failed: {error}',
    'tools_update_available': 'yt-dlp update available ({latest})',
  }
};

let currentLang = 'ko';

/**
 * 현재 언어로 번역된 문자열을 반환한다.
 * 현재 언어에 키가 없으면 한국어로 폴백하고, 그것도 없으면 키 자체를 반환한다.
 * @param {string} key - 번역 키
 * @param {Object} [params] - 보간할 파라미터 ({ n: 5 } → "{n}" 치환)
 * @returns {string} 번역된 문자열
 */
function t(key, params) {
  const str = (I18N[currentLang] && I18N[currentLang][key]) || (I18N['ko'][key]) || key;
  if (params) {
    return Object.entries(params).reduce((s, [k, v]) => s.replace(`{${k}}`, v), str);
  }
  return str;
}

/**
 * UI 전체 언어를 변경하고 data-i18n 속성이 있는 모든 요소를 업데이트한다.
 * @param {string} lang - 언어 코드 ('ko' | 'en')
 */
function setLanguage(lang) {
  currentLang = lang;
  document.documentElement.setAttribute('lang', lang);
  // data-i18n 속성이 있는 모든 요소 텍스트 업데이트
  document.querySelectorAll('[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    const params = el.getAttribute('data-i18n-params');
    el.textContent = t(key, params ? JSON.parse(params) : null);
  });
  // placeholder 속성 업데이트
  document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
    el.placeholder = t(el.getAttribute('data-i18n-placeholder'));
  });
  // title 속성 업데이트
  document.querySelectorAll('[data-i18n-title]').forEach(el => {
    el.title = t(el.getAttribute('data-i18n-title'));
  });
  // select option의 data-i18n 업데이트
  document.querySelectorAll('option[data-i18n]').forEach(el => {
    const key = el.getAttribute('data-i18n');
    const params = el.getAttribute('data-i18n-params');
    el.textContent = t(key, params ? JSON.parse(params) : null);
  });
  // 커스텀 업데이트 로직이 필요한 컴포넌트를 위해 이벤트 발행
  document.dispatchEvent(new CustomEvent('languageChanged', { detail: { lang } }));
}

/**
 * 현재 활성 언어 코드를 반환한다.
 * @returns {string} 현재 언어 코드 ('ko' | 'en')
 */
function getLanguage() {
  return currentLang;
}
