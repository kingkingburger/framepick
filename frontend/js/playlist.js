/**
 * playlist.js - Playlist video selection modal for framepick
 *
 * When a YouTube playlist URL is detected, this module shows a modal
 * allowing users to select which videos to add to the processing queue.
 *
 * Features:
 *   - Checkbox per video with title and duration
 *   - Select All / Select None controls
 *   - Video count summary
 *   - Loading state while fetching playlist info
 *   - Error handling for failed playlist fetches
 *
 * Emits DOM events:
 *   - playlistVideosSelected: { videos: Array<{ url, videoId, title, duration }> }
 */

const PlaylistUI = (() => {
  // YouTube playlist URL patterns
  const PLAYLIST_PATTERNS = [
    // youtube.com/playlist?list=PLxxxxxx
    /^https?:\/\/(?:www\.)?youtube\.com\/playlist\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    // youtube.com/watch?v=xxx&list=PLxxxxxx
    /^https?:\/\/(?:www\.)?youtube\.com\/watch\?(?:.*&)?list=([A-Za-z0-9_-]+)/,
    // m.youtube.com variants
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
   * Initialize the playlist modal component.
   * Should be called after DOM is ready.
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

    // Bind events
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

    // Close on overlay click
    modalEl.addEventListener('click', (e) => {
      if (e.target === modalEl) close();
    });

    // Close on Escape key
    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape' && isOpen) close();
    });

    // Listen for language changes to re-render
    document.addEventListener('languageChanged', () => {
      if (isOpen) _renderList();
    });
  }

  /**
   * Check if a URL is a YouTube playlist URL.
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
   * Open the playlist selection modal.
   * Fetches playlist info from the backend and displays video list.
   * @param {string} url - The playlist URL
   * @param {string} listId - The playlist ID
   */
  async function open(url, listId) {
    if (!modalEl) return;

    isOpen = true;
    videos = [];
    modalEl.hidden = false;

    // Show loading state
    _showLoading(true);
    _showError(null);
    _updateCount();

    try {
      // Fetch playlist entries from backend using yt-dlp --flat-playlist
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
        // No videos or backend unavailable — show error
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
   * Open the modal with pre-loaded video data (no backend fetch needed).
   * Useful for testing or when playlist data is already available.
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
   * Close the playlist modal.
   */
  function close() {
    if (!modalEl) return;
    isOpen = false;
    modalEl.hidden = true;
    videos = [];
    if (videoListEl) videoListEl.innerHTML = '';
  }

  /**
   * Get currently selected videos.
   * @returns {Array<{ videoId: string, title: string, duration: string, url: string }>}
   */
  function getSelectedVideos() {
    return videos
      .filter(v => v.selected)
      .map(({ videoId, title, duration, url }) => ({ videoId, title, duration, url }));
  }

  /**
   * Check if the modal is currently open.
   * @returns {boolean}
   */
  function isModalOpen() {
    return isOpen;
  }

  // ---- Internal ----

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
    // If a formatted string is provided, use it
    if (durationStr) return durationStr;
    // Otherwise format from seconds
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

    // Bind checkbox change events
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

    // Update all checkboxes in the DOM
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

    // Dispatch event for app.js to handle
    document.dispatchEvent(new CustomEvent('playlistVideosSelected', {
      detail: { videos: selected }
    }));

    close();
  }

  /**
   * Select all videos.
   */
  function selectAll() {
    videos.forEach(v => { v.selected = true; });
    _renderList();
    _updateCount();
    _updateSelectAllState();
  }

  /**
   * Deselect all videos.
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
