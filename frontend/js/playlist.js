/**
 * @file playlist.js
 * @description framepick 재생목록 영상 선택 모달 컴포넌트
 *
 * YouTube 재생목록 URL이 감지되면 처리 큐에 추가할
 * 영상을 선택할 수 있는 모달을 표시한다.
 *
 * 주요 기능:
 *   - 영상별 체크박스 (제목 및 재생시간 표시)
 *   - 전체 선택 / 전체 해제 컨트롤
 *   - 영상 수 요약 표시
 *   - 재생목록 정보 가져오는 동안 로딩 상태 표시
 *   - 재생목록 가져오기 실패 시 오류 처리
 *
 * 발행하는 DOM 이벤트:
 *   - playlistVideosSelected: { videos: Array<{ url, videoId, title, duration }> }
 */

const PlaylistUI = (() => {
  // YouTube 재생목록 URL 패턴
  const PLAYLIST_PATTERNS = [
    // youtube.com/playlist?list=PLxxxxxx
    /^https?:\/\/(?:www\.)?youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    // youtube.com/watch?v=xxx&list=PLxxxxxx
    /^https?:\/\/(?:www\.)?youtube\.com\/watch\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    // m.youtube.com 변형
    /^https?:\/\/m\.youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    /^https?:\/\/m\.youtube\.com\/watch\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
  ];

  let modalEl = null;
  let videoListEl = null;
  let selectAllCheckbox = null;
  let confirmBtn = null;
  let cancelBtn = null;
  let closeBtn = null;
  let countEl = null;
  let loadingEl = null;
  let errorEl = null;
  let bodyEl = null;

  /** @type {Array<{ videoId: string, title: string, duration: string, url: string, selected: boolean }>} */
  let videos = [];
  let isOpen = false;

  /**
   * 재생목록 모달 컴포넌트를 초기화한다.
   * DOM이 준비된 후 호출해야 한다.
   */
  function init() {
    modalEl = document.getElementById('playlist-modal');
    if (!modalEl) {
      console.warn('PlaylistUI: Missing #playlist-modal element');
      return;
    }

    videoListEl = document.getElementById('playlist-video-list');
    selectAllCheckbox = document.getElementById('playlist-select-all');
    confirmBtn = document.getElementById('playlist-confirm-btn');
    cancelBtn = document.getElementById('playlist-cancel-btn');
    closeBtn = document.getElementById('playlist-close-btn');
    countEl = document.getElementById('playlist-selected-count');
    loadingEl = document.getElementById('playlist-loading');
    errorEl = document.getElementById('playlist-error');
    bodyEl = document.getElementById('playlist-body');

    // 이벤트 바인딩
    if (selectAllCheckbox) {
      selectAllCheckbox.addEventListener('change', _onSelectAllChange);
    }
    if (confirmBtn) {
      confirmBtn.addEventListener('click', _onConfirm);
    }
    if (cancelBtn) {
      cancelBtn.addEventListener('click', close);
    }
    if (closeBtn) {
      closeBtn.addEventListener('click', close);
    }

    // 오버레이 클릭 시 닫기
    modalEl.addEventListener('click', (e) => {
      if (e.target === modalEl) close();
    });

    // Escape 키로 닫기
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && isOpen) close();
    });

    // 언어 변경 시 재렌더링
    document.addEventListener('languageChanged', () => {
      if (isOpen) _renderList();
    });
  }

  /**
   * URL이 YouTube 재생목록 URL인지 확인한다.
   * @param {string} url
   * @returns {{ isPlaylist: boolean, listId: string|null }}
   */
  function detectPlaylist(url) {
    const trimmed = (url || '').trim();
    for (const pattern of PLAYLIST_PATTERNS) {
      const match = trimmed.match(pattern);
      if (match && match[1]) {
        return { isPlaylist: true, listId: match[1] };
      }
    }
    return { isPlaylist: false, listId: null };
  }

  /**
   * 재생목록 선택 모달을 연다.
   * 백엔드에서 재생목록 정보를 가져와 영상 목록을 표시한다.
   * @param {string} url - 재생목록 URL
   * @param {string} listId - 재생목록 ID
   */
  async function open(url, listId) {
    if (!modalEl) return;

    isOpen = true;
    videos = [];
    modalEl.hidden = false;

    // 로딩 상태 표시
    _showLoading(true);
    _showError(null);
    _updateCount();

    try {
      // yt-dlp --flat-playlist를 사용하여 백엔드에서 재생목록 항목 가져오기
      let playlistData = null;
      if (window.__TAURI__ && window.__TAURI__.core) {
        playlistData = await window.__TAURI__.core.invoke('fetch_playlist', {
          url: url,
        });
      }

      if (playlistData && playlistData.entries && playlistData.entries.length > 0) {
        videos = playlistData.entries.map(v => ({
          videoId: v.video_id || '',
          title: v.title || t('playlist_untitled'),
          duration: '',
          durationSeconds: v.duration || 0,
          url: v.url || `https://www.youtube.com/watch?v=${v.video_id}`,
          selected: true,
        }));
      } else {
        // 영상 없음 또는 백엔드 미사용 — 오류 표시
        _showError(t('playlist_fetch_error'));
        _showLoading(false);
        return;
      }
    } catch (err) {
      console.warn('Failed to fetch playlist info:', err);
      _showError(t('playlist_fetch_error') + (err ? ': ' + err : ''));
      _showLoading(false);
      return;
    }

    _showLoading(false);
    _renderList();
    _updateCount();
    _updateSelectAllState();
  }

  /**
   * 미리 로드된 영상 데이터로 모달을 연다 (백엔드 가져오기 불필요).
   * 테스트 또는 재생목록 데이터가 이미 있는 경우 유용하다.
   * @param {Array<{ videoId: string, title: string, duration: string, url: string }>} videoData
   */
  function openWithData(videoData) {
    if (!modalEl) return;

    isOpen = true;
    videos = videoData.map(v => ({
      ...v,
      selected: true,
    }));

    modalEl.hidden = false;
    _showLoading(false);
    _showError(null);
    _renderList();
    _updateCount();
    _updateSelectAllState();
  }

  /**
   * 재생목록 모달을 닫는다.
   */
  function close() {
    if (!modalEl) return;
    isOpen = false;
    modalEl.hidden = true;
    videos = [];
    if (videoListEl) videoListEl.innerHTML = '';
  }

  /**
   * 현재 선택된 영상 목록을 반환한다.
   * @returns {Array<{ videoId: string, title: string, duration: string, url: string }>}
   */
  function getSelectedVideos() {
    return videos
      .filter(v => v.selected)
      .map(({ videoId, title, duration, url }) => ({ videoId, title, duration, url }));
  }

  /**
   * 모달이 현재 열려있는지 확인한다.
   * @returns {boolean}
   */
  function isModalOpen() {
    return isOpen;
  }

  // ---- 내부 함수 ----

  function _showLoading(show) {
    if (loadingEl) loadingEl.hidden = !show;
    if (bodyEl) bodyEl.hidden = show;
  }

  function _showError(message) {
    if (!errorEl) return;
    if (message) {
      errorEl.textContent = message;
      errorEl.hidden = false;
      if (bodyEl) bodyEl.hidden = true;
    } else {
      errorEl.textContent = '';
      errorEl.hidden = true;
    }
  }

  function _formatDuration(durationStr, durationSeconds) {
    // 형식화된 문자열이 있으면 그대로 사용
    if (durationStr) return durationStr;
    // 없으면 초에서 변환
    if (!durationSeconds || durationSeconds <= 0) return '';
    const h = Math.floor(durationSeconds / 3600);
    const m = Math.floor((durationSeconds % 3600) / 60);
    const s = durationSeconds % 60;
    if (h > 0) {
      return `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    }
    return `${m}:${String(s).padStart(2, '0')}`;
  }

  function _renderList() {
    if (!videoListEl) return;

    if (videos.length === 0) {
      videoListEl.innerHTML = `<div class="playlist-empty">${t('playlist_no_videos')}</div>`;
      if (confirmBtn) confirmBtn.disabled = true;
      return;
    }

    videoListEl.innerHTML = videos.map((video, index) => {
      const duration = _formatDuration(video.duration, video.durationSeconds);
      const checkedAttr = video.selected ? 'checked' : '';
      const titleText = _escapeHtml(video.title);
      const indexDisplay = String(index + 1).padStart(2, '0');

      return `
        <label class="playlist-video-item" data-index="${index}">
          <input type="checkbox" class="playlist-video-checkbox" data-index="${index}" ${checkedAttr}>
          <span class="playlist-video-index">${indexDisplay}</span>
          <div class="playlist-video-info">
            <span class="playlist-video-title" title="${_escapeAttr(video.title)}">${titleText}</span>
            ${duration ? `<span class="playlist-video-duration">${duration}</span>` : ''}
          </div>
        </label>
      `;
    }).join('');

    // 체크박스 변경 이벤트 바인딩
    videoListEl.querySelectorAll('.playlist-video-checkbox').forEach(cb => {
      cb.addEventListener('change', (e) => {
        const idx = parseInt(cb.getAttribute('data-index'), 10);
        if (idx >= 0 && idx < videos.length) {
          videos[idx].selected = cb.checked;
          _updateCount();
          _updateSelectAllState();
        }
      });
    });
  }

  function _updateCount() {
    const selectedCount = videos.filter(v => v.selected).length;
    const totalCount = videos.length;

    if (countEl) {
      countEl.textContent = t('playlist_selected_count', {
        selected: selectedCount,
        total: totalCount,
      });
    }

    if (confirmBtn) {
      confirmBtn.disabled = selectedCount === 0;
      confirmBtn.textContent = selectedCount > 0
        ? t('playlist_add_selected', { n: selectedCount })
        : t('playlist_add_selected', { n: 0 });
    }
  }

  function _updateSelectAllState() {
    if (!selectAllCheckbox) return;
    const selectedCount = videos.filter(v => v.selected).length;
    const totalCount = videos.length;

    if (selectedCount === 0) {
      selectAllCheckbox.checked = false;
      selectAllCheckbox.indeterminate = false;
    } else if (selectedCount === totalCount) {
      selectAllCheckbox.checked = true;
      selectAllCheckbox.indeterminate = false;
    } else {
      selectAllCheckbox.checked = false;
      selectAllCheckbox.indeterminate = true;
    }
  }

  function _onSelectAllChange() {
    const checked = selectAllCheckbox.checked;
    videos.forEach(v => { v.selected = checked; });

    // DOM의 모든 체크박스 업데이트
    if (videoListEl) {
      videoListEl.querySelectorAll('.playlist-video-checkbox').forEach(cb => {
        cb.checked = checked;
      });
    }

    _updateCount();
    _updateSelectAllState();
  }

  function _onConfirm() {
    const selected = getSelectedVideos();
    if (selected.length === 0) return;

    // app.js가 처리할 이벤트 발행
    document.dispatchEvent(new CustomEvent('playlistVideosSelected', {
      detail: { videos: selected }
    }));

    close();
  }

  /**
   * 모든 영상을 선택한다.
   */
  function selectAll() {
    videos.forEach(v => { v.selected = true; });
    _renderList();
    _updateCount();
    _updateSelectAllState();
  }

  /**
   * 모든 영상 선택을 해제한다.
   */
  function selectNone() {
    videos.forEach(v => { v.selected = false; });
    _renderList();
    _updateCount();
    _updateSelectAllState();
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

  return {
    init,
    detectPlaylist,
    open,
    openWithData,
    close,
    selectAll,
    selectNone,
    getSelectedVideos,
    isModalOpen,
  };
})();
