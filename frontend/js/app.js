/**
 * @file app.js
 * @description framepick 앱의 메인 진입점
 *
 * 역할:
 *  - DOMContentLoaded 시 모든 UI 컴포넌트 초기화
 *  - yt-dlp / ffmpeg 자동 다운로드(도구 설치) 오버레이 처리
 *  - URL 입력 → 대기열 추가 플로우 제어
 *  - 재생목록 URL 감지 → PlaylistUI 연동
 *  - 라이브러리 항목 로드 및 렌더링
 *  - 토스트 알림, 재캡쳐 모달, 삭제 확인 모달 관리
 *  - 언어 전환 및 AppState와 백엔드 간 동기화
 */

document.addEventListener('DOMContentLoaded', () => {
  // ─── 사이드바 토글 (모바일 반응형) ──────────────────────────
  const sidebarEl = document.getElementById('app-sidebar');
  const sidebarToggleBtn = document.getElementById('sidebar-toggle-btn');
  if (sidebarToggleBtn && sidebarEl) {
    sidebarToggleBtn.addEventListener('click', () => {
      sidebarEl.classList.toggle('open');
    });
    // 메인 영역 클릭 시 사이드바 닫기 (모바일)
    const appMain = document.querySelector('.app-main');
    if (appMain) {
      appMain.addEventListener('click', (e) => {
        if (window.innerWidth <= 768 && sidebarEl.classList.contains('open')) {
          if (!e.target.closest('.sidebar-toggle-btn')) {
            sidebarEl.classList.remove('open');
          }
        }
      });
    }
  }

  // ─── Tauri interop 헬퍼 ────────────────────────────────────
  const invoke = (cmd, args) => {
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke(cmd, args);
    }
    console.log('[dev] invoke:', cmd, args);
    return Promise.resolve(null);
  };

  // ─── 도구 설치 (yt-dlp + ffmpeg 자동 다운로드) ─────────────
  const toolsOverlay = document.getElementById('tools-setup-overlay');
  const toolsMessage = document.getElementById('tools-setup-message');
  const toolsProgressFill = document.getElementById('tools-progress-fill');
  const toolsDetail = document.getElementById('tools-setup-detail');

  function updateToolsOverlay(pct, message, detail) {
    if (toolsProgressFill) toolsProgressFill.style.width = pct + '%';
    if (toolsMessage) toolsMessage.textContent = message;
    if (toolsDetail) toolsDetail.textContent = detail || '';
  }

  async function runToolsSetup() {
    if (!window.__TAURI__ || !window.__TAURI__.core) return;

    // 백엔드에서 보내는 세부 진행 이벤트 수신
    let unlistenFn = null;
    if (window.__TAURI__.event && window.__TAURI__.event.listen) {
      unlistenFn = await window.__TAURI__.event.listen('tools:status', (event) => {
        const p = event.payload;
        if (!p) return;

        // 도구별 진행률 매핑: yt-dlp 0~50%, ffmpeg 50~100%
        let pct = p.progress != null ? p.progress : 0;
        // yt-dlp: 전체 진행 바의 앞 절반 담당
        if (p.tool === 'yt-dlp') {
          pct = Math.round(pct / 2);
        } else if (p.tool === 'ffmpeg') {
          pct = 50 + Math.round(pct / 2);
        }

        const statusLabel = p.status === 'downloading' ? t('tools_setup_downloading', { pct: p.progress })
          : p.status === 'extracting' ? t('tools_setup_extracting')
          : p.status === 'ready' ? t('tools_setup_ready')
          : t('tools_setup_checking');

        updateToolsOverlay(pct, t('tools_setup_message'), `${p.tool}: ${statusLabel}`);
      });
    }

    try {
      // setup_tools 호출: 이미 준비된 경우 빠르게 반환됨
      // 오버레이를 즉시 표시하고 완료 후 숨김
      if (toolsOverlay) toolsOverlay.hidden = false;
      updateToolsOverlay(0, t('tools_setup_message'), t('tools_setup_checking'));

      const status = await window.__TAURI__.core.invoke('setup_tools');
      console.log('[tools] Setup complete:', status);

      if (toolsOverlay) toolsOverlay.hidden = true;

      // 백그라운드에서 yt-dlp 업데이트 확인 (비차단)
      window.__TAURI__.core.invoke('check_ytdlp_update').then((info) => {
        if (info && info.update_available) {
          console.log('[tools] yt-dlp update available:', info.latest_version);
          showToast(t('tools_update_available', { latest: info.latest_version || '' }), 'warning');
        }
      }).catch(() => {});
    } catch (err) {
      console.error('[tools] Setup failed:', err);
      updateToolsOverlay(0, t('tools_setup_error', { error: String(err) }), '');
      // 오류 발생 시에도 앱을 차단하지 않음 — 4초 후 오버레이 자동 숨김
      setTimeout(() => {
        if (toolsOverlay) toolsOverlay.hidden = true;
      }, 4000);
    } finally {
      if (unlistenFn) unlistenFn();
    }
  }

  runToolsSetup();

  // ─── 컴포넌트 초기화 ───────────────────────────────────────
  UrlInput.init();
  initCaptureMode();
  if (typeof SettingsUI !== 'undefined') {
    SettingsUI.init();
  } else if (typeof initSettings === 'function') {
    initSettings();
  }
  SlidesViewer.init();
  PipelineProgress.init();
  QueueUI.init();
  CaptureList.init();
  PlaylistUI.init();

  // ─── 워크플로우 빈 화면 안내 표시 ─────────────────────────
  // 큐가 비어있고 캡쳐 목록도 없을 때 안내 문구 표시
  const workflowHint = document.getElementById('workflow-empty-hint');
  function updateWorkflowHint() {
    if (!workflowHint) return;
    const queueCounts = QueueUI.getCounts();
    const hasQueue = queueCounts.total > 0;
    const hasCaptureList = typeof CaptureList !== 'undefined' && CaptureList.isVisible();
    workflowHint.classList.toggle('hidden', hasQueue || hasCaptureList);
  }
  // 큐 및 캡쳐 목록 변경 시 안내 문구 갱신
  document.addEventListener('queueItemAdded', updateWorkflowHint);
  document.addEventListener('queueItemCompleted', updateWorkflowHint);
  document.addEventListener('queueItemFailed', updateWorkflowHint);
  document.addEventListener('queueCleared', updateWorkflowHint);
  // 초기 상태 설정
  updateWorkflowHint();

  // ─── 큐 완료 → 라이브러리 새로고침 연결 ───────────────────────
  // QueueUI가 Tauri 이벤트를 내부적으로 처리하므로 완료 시 라이브러리만 새로고침
  document.addEventListener('queueItemCompleted', () => {
    loadLibrary();
  });

  // ─── 헤더 언어 전환기 ──────────────────────────────────────
  const langSelect = document.getElementById('lang-select');
  langSelect.addEventListener('change', () => {
    const lang = langSelect.value;
    setLanguage(lang);
    AppState.setLanguage(lang);
    invoke('update_settings', { patch: { language: lang } })
      .catch(err => console.warn('Failed to persist language:', err));
  });

  // AppState 언어 변경 → 헤더 드롭다운 동기화
  AppState.on('language', (lang) => {
    if (langSelect.value !== lang) {
      langSelect.value = lang;
    }
    setLanguage(lang);
  });

  // 검증된 URL 제출 이벤트 수신 → 대기열 추가 또는 재생목록 모달 열기
  document.addEventListener('urlSubmitted', (e) => {
    const { url, videoId } = e.detail;

    // 재생목록 URL 여부 확인
    const playlistCheck = PlaylistUI.detectPlaylist(url);
    if (playlistCheck.isPlaylist) {
      // 단건 추가 대신 재생목록 선택 모달을 열어줌
      console.log('Playlist detected:', playlistCheck.listId);
      PlaylistUI.open(url, playlistCheck.listId);
      return;
    }

    // 단일 영상: 로컬 대기열 내 중복 확인 (URL 또는 영상 ID 기준)
    const currentQueue = QueueUI.getQueue();
    const isDuplicateInQueue = currentQueue.some((q) => {
      if (q.status !== 'pending' && q.status !== 'processing') return false;
      // URL 완전 일치
      if (q.url === url) return true;
      // 영상 ID 일치 (같은 영상의 다른 URL 형식 처리)
      if (videoId && q.videoId && q.videoId === videoId) return true;
      return false;
    });
    if (isDuplicateInQueue) {
      showToast(t('error_duplicate') || 'This URL is already in the queue', 'warning');
      return;
    }

    // 라이브러리에 이미 처리된 영상인지 확인 후 대기열에 추가
    const checkAndAdd = async () => {
      if (videoId && window.__TAURI__ && window.__TAURI__.core) {
        try {
          const exists = await window.__TAURI__.core.invoke('check_video_exists', { videoId });
          if (exists) {
            showToast(t('error_already_exists'), 'warning');
            console.log('Video already exists in library:', videoId);
            return;
          }
        } catch (err) {
          // 확인 실패 시에도 사용자를 차단하지 않고 진행
          console.warn('Failed to check video existence:', err);
        }
      }

      try {
        const id = await QueueUI.addItem(url, videoId || '');
        if (id > 0) {
          showToast(t('queue_added'), 'success');
          console.log('Added to queue:', { id, url, videoId });
          // 큐 섹션이 보이도록 자동 스크롤
          const queueSection = document.getElementById('queue-section');
          if (queueSection) {
            queueSection.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
          }
        }
      } catch (err) {
        console.warn('Failed to add to queue:', err);
      }
    };

    checkAndAdd();
  });

  // ─── 재생목록 선택 다이얼로그 → 대기열 연결 ─────────────────
  // 사용자가 PlaylistUI에서 영상을 선택·확인하면
  // 각 영상을 개별 항목으로 다운로드 대기열에 추가.
  // 순서 유지 및 경쟁 조건 방지를 위해 순차적으로 추가.
  document.addEventListener('playlistVideosSelected', (e) => {
    const { videos } = e.detail;
    if (!videos || videos.length === 0) return;

    let addedCount = 0;
    let skippedCount = 0;

    // 이미 대기열에 있는 URL과 이번 배치에서 새로 추가된 URL을 함께 추적
    // 배치 내 중복 방지 (예: 같은 영상이 재생목록에 두 번 등장하는 경우)
    const activeUrls = new Set(
      QueueUI.getQueue()
        .filter(q => q.status === 'pending' || q.status === 'processing')
        .map(q => q.url)
    );

    const addNext = async (index) => {
      if (index >= videos.length) {
        // 모든 영상 처리 완료 후 요약 토스트 표시
        if (addedCount > 0 && skippedCount > 0) {
          showToast(
            t('queue_added_partial', { added: addedCount, skipped: skippedCount }),
            'success'
          );
        } else if (addedCount > 0) {
          showToast(t('queue_added_batch', { n: addedCount }), 'success');
        } else if (skippedCount > 0) {
          showToast(t('error_all_duplicates'), 'warning');
        }
        return;
      }

      const video = videos[index];

      // 중복 건너뜀 (activeUrls Set으로 배치 내 중복도 감지)
      if (activeUrls.has(video.url)) {
        skippedCount++;
        console.log('Skipping duplicate in queue:', video.url);
        addNext(index + 1);
        return;
      }

      // 라이브러리에 이미 있는 영상 건너뜀 (백엔드 사용 가능 시)
      if (video.videoId && window.__TAURI__ && window.__TAURI__.core) {
        try {
          const exists = await window.__TAURI__.core.invoke('check_video_exists', { videoId: video.videoId });
          if (exists) {
            skippedCount++;
            console.log('Skipping video already in library:', video.videoId);
            addNext(index + 1);
            return;
          }
        } catch (_) { /* proceed if check fails */ }
      }

      QueueUI.addItem(video.url, video.videoId || '').then((id) => {
        if (id > 0) {
          addedCount++;
          activeUrls.add(video.url); // 배치 내 중복 방지를 위해 추적
          console.log('Added playlist video to queue:', {
            id,
            url: video.url,
            title: video.title,
            index: index + 1,
            total: videos.length,
          });
        }
        addNext(index + 1);
      }).catch((err) => {
        console.warn('Failed to add playlist video to queue:', video.url, err);
        addNext(index + 1);
      });
    };

    addNext(0);
  });

  // 캡쳐 모드 변경 이벤트 수신 → 백엔드에 동기화
  document.addEventListener('captureModeChanged', (e) => {
    console.log('Capture mode config:', e.detail);
    invoke('set_input_state', { state: AppState.buildPipelineInput() }).catch(() => {});
  });

  // URL 변경 시 디바운스 처리 후 백엔드에 동기화
  let _urlSyncTimer = null;
  AppState.on('url', () => {
    clearTimeout(_urlSyncTimer);
    _urlSyncTimer = setTimeout(() => {
      invoke('set_input_state', { state: AppState.buildPipelineInput() }).catch(() => {});
    }, 300);
  });

  // 라이브러리 새로고침 버튼
  const refreshBtn = document.getElementById('btn-refresh-library');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', () => loadLibrary());
  }

  // ─── 백엔드의 queue:duplicate-skipped 이벤트 수신 ──────────────
  // 처리 중 라이브러리에 이미 존재하는 영상이 감지되면
  // 백엔드에서 queue:duplicate-skipped 이벤트를 발행함.
  if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
    window.__TAURI__.event.listen('queue:duplicate-skipped', (event) => {
      const payload = event.payload;
      console.log('[queue_processor] Duplicate skipped:', payload);
      showToast(t('error_already_exists'), 'warning');
    });
  }

  // ─── 백엔드의 capture:fallback 이벤트 수신 ─────────────────
  // 자막 모드를 사용할 수 없을 때(자막 없음 또는 확인 실패)
  // 백엔드에서 capture:fallback 이벤트를 발행함.
  // 토스트 알림을 표시하고 폴백을 기록함.
  if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
    window.__TAURI__.event.listen('capture:fallback', (event) => {
      const payload = event.payload;
      console.log('[capture_fallback] Mode fallback occurred:', payload);

      // 백엔드 페이로드의 i18n 키 사용, 없으면 기본 메시지로 폴백
      const reasonKey = payload.reason_key || 'fallback_no_subtitles';
      const message = t(reasonKey) || payload.reason || 'Capture mode changed';

      showToast(message, 'warning');

      // 해당하는 경우 로컬 상태의 큐 항목 캡쳐 모드 업데이트
      if (payload.queue_id && payload.queue_id > 0) {
        AppState.updateQueueItem(payload.queue_id, {
          captureMode: payload.effective_mode,
        });
      }
    });
  }

  // 초기 언어 설정 (백엔드 설정이 있으면 이후에 덮어씌워짐)
  setLanguage('ko');

  // 앱 시작 시 라이브러리 로드
  loadLibrary();
});

/**
 * 백엔드에서 라이브러리 항목을 불러와 화면에 표시한다.
 */
async function loadLibrary() {
  const grid = document.getElementById('library-grid');
  const countEl = document.getElementById('library-count');
  if (!grid) return;

  try {
    let entries = [];
    if (window.__TAURI__ && window.__TAURI__.core) {
      entries = await window.__TAURI__.core.invoke('list_library_entries');
    }

    // 라이브러리 항목 수 뱃지 업데이트
    if (countEl) {
      countEl.textContent = entries.length > 0
        ? t('library_item_count', { n: entries.length })
        : '';
    }

    if (entries.length === 0) {
      grid.innerHTML = `<p class="library-empty" data-i18n="library_empty">${t('library_empty')}</p>`;
      return;
    }

    grid.innerHTML = entries.map(entry => {
      const title = entry.title || entry.video_id;
      const slideCount = entry.slide_count != null ? t('library_slides', { n: entry.slide_count }) : '';
      // 첫 번째 캡쳐 프레임을 썸네일로 사용; 없으면 플레이스홀더 아이콘으로 대체
      const thumbHtml = entry.thumbnail
        ? `<img class="library-card-thumb" src="${escapeHtml(entry.thumbnail)}" alt="${escapeHtml(title)}" loading="lazy" onerror="this.parentElement.innerHTML='<div class=\\'library-card-thumb-placeholder\\'>&#9654;</div>'">`
        : `<div class="library-card-thumb-placeholder">&#9654;</div>`;

      return `
        <div class="library-card${entry.has_slides ? '' : ' library-card-disabled'}" data-video-id="${escapeHtml(entry.video_id)}">
          <div class="library-card-thumb-wrap">
            ${thumbHtml}
            ${slideCount ? `<span class="library-card-badge">${slideCount}</span>` : ''}
            ${entry.has_slides ? `<div class="library-card-overlay">
              <button class="library-card-overlay-btn library-card-view" data-view-id="${escapeHtml(entry.video_id)}" title="${t('library_open_viewer')}">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polygon points="5 3 19 12 5 21 5 3"/></svg>
                <span>${t('library_open_viewer')}</span>
              </button>
              <button class="library-card-overlay-btn library-card-browser" data-browser-id="${escapeHtml(entry.video_id)}" title="${t('library_open_browser')}">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
                <span>${t('library_open_browser')}</span>
              </button>
            </div>` : ''}
          </div>
          <div class="library-card-info">
            <div class="library-card-title" title="${escapeHtml(title)}">${escapeHtml(title)}</div>
            <div class="library-card-meta">
              <span>${entry.video_id}</span>
              <div class="library-card-actions">
                <button class="library-card-action library-card-open-folder" data-folder-id="${escapeHtml(entry.video_id)}" title="${t('library_open_folder')}">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
                </button>
                <button class="library-card-action library-card-recapture" data-recapture-id="${escapeHtml(entry.video_id)}" data-recapture-title="${escapeHtml(title)}" title="${t('recapture_btn')}">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>
                </button>
                <button class="library-card-action library-card-delete" data-delete-id="${escapeHtml(entry.video_id)}" title="${t('library_delete')}">&times;</button>
              </div>
            </div>
          </div>
        </div>
      `;
    }).join('');

    // 뷰어 열기 클릭 이벤트 바인딩
    grid.querySelectorAll('.library-card[data-video-id]').forEach(card => {
      card.addEventListener('click', (e) => {
        // 액션 버튼 또는 오버레이 버튼 클릭 시 뷰어 열지 않음
        if (e.target.closest('.library-card-action') || e.target.closest('.library-card-overlay-btn')) return;
        const videoId = card.dataset.videoId;
        if (videoId) {
          SlidesViewer.open(videoId);
        }
      });
    });

    // "뷰어에서 열기" 오버레이 버튼 이벤트 바인딩
    grid.querySelectorAll('.library-card-view').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.viewId;
        if (videoId) {
          SlidesViewer.open(videoId);
        }
      });
    });

    // "브라우저에서 열기" 오버레이 버튼 이벤트 바인딩
    grid.querySelectorAll('.library-card-browser').forEach(btn => {
      btn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.browserId;
        if (!videoId) return;

        try {
          if (window.__TAURI__ && window.__TAURI__.core) {
            await window.__TAURI__.core.invoke('open_slides_external', { videoId });
          }
        } catch (err) {
          console.error('Failed to open slides in browser:', err);
          showToast(t('library_open_browser_failed'), 'error');
        }
      });
    });

    // 폴더 열기 버튼 이벤트 바인딩
    grid.querySelectorAll('.library-card-open-folder').forEach(btn => {
      btn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.folderId;
        if (!videoId) return;

        try {
          if (window.__TAURI__ && window.__TAURI__.core) {
            await window.__TAURI__.core.invoke('open_folder', { videoId });
          }
        } catch (err) {
          console.error('Failed to open folder:', err);
          showToast(t('library_open_folder_error') + ': ' + err, 'error');
        }
      });
    });

    // 다시 캡쳐 버튼 이벤트 바인딩
    grid.querySelectorAll('.library-card-recapture').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.recaptureId;
        const title = btn.dataset.recaptureTitle || videoId;
        if (videoId) {
          openRecaptureModal(videoId, title);
        }
      });
    });

    // 삭제 버튼 이벤트 바인딩
    grid.querySelectorAll('.library-card-delete').forEach(btn => {
      btn.addEventListener('click', (e) => {
        e.stopPropagation();
        const videoId = btn.dataset.deleteId;
        if (!videoId) return;

        // 확인 다이얼로그에 표시할 카드 제목 찾기
        const card = btn.closest('.library-card');
        const titleEl = card ? card.querySelector('.library-card-title') : null;
        const title = titleEl ? titleEl.textContent : videoId;

        openDeleteConfirmModal(videoId, title);
      });
    });
  } catch (err) {
    console.warn('Failed to load library:', err);
    grid.innerHTML = `<p class="library-empty">${t('library_empty')}</p>`;
  }
}

/**
 * 스택 지원이 있는 토스트 알림을 표시한다.
 * 여러 토스트가 위로 쌓여 빠른 연속 오류가 서로 덮어쓰지 않도록 한다.
 * 오류 토스트는 아이콘과 닫기 버튼을 포함하여 UX를 개선한다.
 * @param {string} message - 표시할 메시지
 * @param {'success'|'error'|'warning'} type - 알림 유형
 */
function showToast(message, type = 'success') {
  // 유형별 시각 구분을 위한 토스트 아이콘
  const TOAST_ICONS = {
    success: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    error: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
    warning: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="flex-shrink:0"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
  };

  // DOM 넘침 방지를 위해 최대 5개까지만 표시
  const existingToasts = document.querySelectorAll('.toast');
  if (existingToasts.length >= 5) {
    existingToasts[0].remove();
  }

  const toast = document.createElement('div');
  toast.className = `toast toast-${type}`;
  const iconHtml = TOAST_ICONS[type] || '';
  const safeMsg = escapeHtml(message);
  toast.innerHTML = `<span class="toast-icon">${iconHtml}</span><span class="toast-message">${safeMsg}</span><button class="toast-close" aria-label="Close">&times;</button>`;
  document.body.appendChild(toast);

  // 닫기 버튼으로 수동 닫기 가능
  toast.querySelector('.toast-close').addEventListener('click', () => {
    toast.classList.remove('toast-show');
    setTimeout(() => {
      toast.remove();
      _repositionToasts();
    }, 300);
  });

  // 위로 쌓이도록 모든 토스트 위치 재조정
  _repositionToasts();

  // 표시 애니메이션 트리거
  requestAnimationFrame(() => {
    toast.classList.add('toast-show');
  });

  // 오류 토스트는 6초, 경고는 4초, 성공은 3초 유지
  const duration = type === 'error' ? 6000 : type === 'warning' ? 4000 : 3000;
  setTimeout(() => {
    toast.classList.remove('toast-show');
    setTimeout(() => {
      toast.remove();
      _repositionToasts();
    }, 300);
  }, duration);
}

/**
 * 쌓인 토스트들이 겹치지 않도록 위치를 재조정한다.
 * 토스트가 추가되거나 제거될 때마다 호출된다.
 */
function _repositionToasts() {
  const toasts = document.querySelectorAll('.toast');
  let bottomOffset = 24;
  toasts.forEach((toast) => {
    toast.style.bottom = bottomOffset + 'px';
    bottomOffset += toast.offsetHeight + 8;
  });
}

/**
 * 속성/컨텐츠 삽입 시 XSS 방지를 위한 HTML 이스케이프 함수.
 * @param {string} str - 이스케이프할 문자열
 * @returns {string} 이스케이프된 문자열
 */
function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

// ─── 재캡쳐 모달 ───────────────────────────────────────────────

/** 현재 재캡쳐 대상 영상 ID */
let _recaptureVideoId = null;
/** 재캡쳐용 캐시된 설정값 (장면 임계값, 간격 등) */
let _recaptureSettings = null;

/** 모드별 설명 i18n 키 매핑 */
const RECAPTURE_MODE_DESC_KEYS = {
  subtitle: 'capture_mode_subtitle_desc',
  scene: 'capture_mode_scene_desc',
  interval: 'capture_mode_interval_desc',
};

/**
 * 재캡쳐 모달의 모드 설명 텍스트를 갱신하고 모드별 옵션을 표시/숨긴다.
 */
function _updateRecaptureModeUI() {
  const modeSelect = document.getElementById('recapture-mode');
  const descEl = document.getElementById('recapture-mode-desc');
  const intervalGroup = document.getElementById('recapture-interval-group');
  const sceneGroup = document.getElementById('recapture-scene-group');

  if (!modeSelect) return;
  const mode = modeSelect.value;

  // 설명 텍스트 업데이트
  if (descEl) {
    const descKey = RECAPTURE_MODE_DESC_KEYS[mode] || '';
    descEl.textContent = descKey ? t(descKey) : '';
    descEl.setAttribute('data-i18n', descKey);
  }

  // 간격 옵션 표시/숨김
  if (intervalGroup) intervalGroup.hidden = mode !== 'interval';
  // 장면 임계값 표시/숨김
  if (sceneGroup) sceneGroup.hidden = mode !== 'scene';
}

/**
 * 재캡쳐 모달에서 유효한 간격(초)을 반환한다. 프리셋 또는 직접 입력값 처리.
 * @returns {number} 간격(초)
 */
function _getRecaptureInterval() {
  const intervalSelect = document.getElementById('recapture-interval');
  if (!intervalSelect) return 10;

  if (intervalSelect.value === 'custom') {
    const customInput = document.getElementById('recapture-custom-interval');
    const val = customInput ? parseInt(customInput.value, 10) : NaN;
    if (isNaN(val) || val < 1 || val > 3600) return 10;
    return val;
  }
  return parseInt(intervalSelect.value, 10) || 10;
}

/**
 * 라이브러리 항목에 대한 재캡쳐 모달을 열고 초기 상태를 설정한다.
 * @param {string} videoId - 대상 영상 ID
 * @param {string} title - 영상 제목
 */
async function openRecaptureModal(videoId, title) {
  const modal = document.getElementById('recapture-modal');
  const titleEl = document.getElementById('recapture-video-title');
  const modeSelect = document.getElementById('recapture-mode');
  const errorEl = document.getElementById('recapture-error');
  const startBtn = document.getElementById('btn-recapture-start');
  const progressEl = document.getElementById('recapture-progress');
  const thresholdSlider = document.getElementById('recapture-scene-threshold');
  const thresholdValue = document.getElementById('recapture-threshold-value');

  if (!modal) return;

  _recaptureVideoId = videoId;

  // 상태 초기화
  if (titleEl) titleEl.textContent = title;
  if (modeSelect) modeSelect.value = 'subtitle';
  if (errorEl) { errorEl.hidden = true; errorEl.textContent = ''; }
  if (progressEl) progressEl.hidden = true;
  if (startBtn) { startBtn.disabled = false; startBtn.textContent = t('recapture_start'); }

  // 설정에서 장면 임계값 로드
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      _recaptureSettings = await window.__TAURI__.core.invoke('get_settings');
      if (_recaptureSettings && thresholdSlider) {
        const threshPercent = Math.round((_recaptureSettings.scene_change_threshold || 0.30) * 100);
        thresholdSlider.value = threshPercent;
        if (thresholdValue) thresholdValue.textContent = threshPercent + '%';
      }
    }
  } catch (err) {
    console.warn('Failed to load settings for recapture:', err);
  }

  // 모드별 UI 업데이트
  _updateRecaptureModeUI();

  // 직접 입력 간격 필드 초기화
  const customIntervalGroup = document.getElementById('recapture-custom-interval-group');
  const customIntervalInput = document.getElementById('recapture-custom-interval');
  const intervalSelect = document.getElementById('recapture-interval');
  if (customIntervalGroup) customIntervalGroup.hidden = true;
  if (customIntervalInput) customIntervalInput.value = '';
  if (intervalSelect) intervalSelect.value = '10';

  // 원본 영상 파일 사용 가능 여부 확인
  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      const available = await window.__TAURI__.core.invoke('check_recapture_available', { videoId });
      if (!available) {
        if (errorEl) {
          errorEl.textContent = t('recapture_no_video');
          errorEl.hidden = false;
        }
        if (startBtn) startBtn.disabled = true;
      }
    }
  } catch (err) {
    console.warn('Failed to check recapture availability:', err);
  }

  modal.hidden = false;
}

/**
 * 재캡쳐 모달을 닫고 상태를 초기화한다.
 */
function closeRecaptureModal() {
  const modal = document.getElementById('recapture-modal');
  if (modal) modal.hidden = true;
  _recaptureVideoId = null;
}

/**
 * 선택된 캡쳐 모드로 재캡쳐를 실행한다.
 */
async function executeRecapture() {
  if (!_recaptureVideoId) return;

  const modeSelect = document.getElementById('recapture-mode');
  const startBtn = document.getElementById('btn-recapture-start');
  const cancelBtn = document.getElementById('btn-recapture-cancel');
  const errorEl = document.getElementById('recapture-error');
  const progressEl = document.getElementById('recapture-progress');
  const progressFill = document.getElementById('recapture-progress-fill');
  const progressText = document.getElementById('recapture-progress-text');
  const thresholdSlider = document.getElementById('recapture-scene-threshold');

  const captureMode = modeSelect ? modeSelect.value : 'scene';
  const intervalSeconds = _getRecaptureInterval();

  // 직접 입력 간격 유효성 검사
  if (captureMode === 'interval') {
    const intervalSelect = document.getElementById('recapture-interval');
    if (intervalSelect && intervalSelect.value === 'custom') {
      const customInput = document.getElementById('recapture-custom-interval');
      const val = customInput ? parseInt(customInput.value, 10) : NaN;
      if (isNaN(val) || val < 1 || val > 3600) {
        if (errorEl) {
          errorEl.textContent = t('interval_custom_hint', { min: 1, max: 3600 });
          errorEl.hidden = false;
        }
        return;
      }
    }
  }

  // 버튼 비활성화 및 처리 중 상태 표시
  if (startBtn) {
    startBtn.disabled = true;
    startBtn.textContent = t('recapture_processing');
  }
  if (cancelBtn) cancelBtn.disabled = true;
  if (errorEl) errorEl.hidden = true;

  // 진행 표시기 표시
  if (progressEl) progressEl.hidden = false;
  if (progressFill) {
    progressFill.style.width = '30%';
  }
  if (progressText) progressText.textContent = t('recapture_processing');

  try {
    const args = {
      videoId: _recaptureVideoId,
      captureMode: captureMode,
    };
    if (captureMode === 'interval') {
      args.intervalSeconds = intervalSeconds;
    }
    if (captureMode === 'scene' && thresholdSlider) {
      args.sceneThreshold = parseInt(thresholdSlider.value, 10) / 100.0;
    }

    // 진행 단계 시뮬레이션
    if (progressFill) progressFill.style.width = '50%';

    let result = null;
    if (window.__TAURI__ && window.__TAURI__.core) {
      result = await window.__TAURI__.core.invoke('recapture_library_item', args);
    }

    if (progressFill) progressFill.style.width = '100%';
    if (progressText) progressText.textContent = t('progress_done');

    // 완료 상태를 잠깐 보여주기 위한 짧은 대기
    await new Promise(resolve => setTimeout(resolve, 400));

    closeRecaptureModal();

    if (result) {
      showToast(t('recapture_success', { n: result.frame_count }), 'success');
    } else {
      showToast(t('recapture_success', { n: '?' }), 'success');
    }

    // 업데이트된 썸네일/개수 표시를 위해 라이브러리 새로고침
    loadLibrary();
  } catch (err) {
    console.error('Re-capture failed:', err);
    if (progressEl) progressEl.hidden = true;
    if (errorEl) {
      errorEl.textContent = typeof err === 'string' ? err : (err.message || t('recapture_error'));
      errorEl.hidden = false;
    }
    if (startBtn) {
      startBtn.disabled = false;
      startBtn.textContent = t('recapture_start');
    }
    if (cancelBtn) cancelBtn.disabled = false;
  }
}

// 재캡쳐 모달 이벤트 핸들러 초기화
document.addEventListener('DOMContentLoaded', () => {
  // 재캡쳐 모달 닫기 버튼
  const closeBtn = document.getElementById('btn-recapture-close');
  if (closeBtn) closeBtn.addEventListener('click', closeRecaptureModal);

  const cancelBtn = document.getElementById('btn-recapture-cancel');
  if (cancelBtn) cancelBtn.addEventListener('click', closeRecaptureModal);

  // 재캡쳐 시작 버튼
  const startBtn = document.getElementById('btn-recapture-start');
  if (startBtn) startBtn.addEventListener('click', executeRecapture);

  // 모드 선택 → 모드별 옵션 표시/숨김
  const modeSelect = document.getElementById('recapture-mode');
  if (modeSelect) {
    modeSelect.addEventListener('change', _updateRecaptureModeUI);
  }

  // 간격 선택 → 직접 입력 필드 표시/숨김
  const intervalSelect = document.getElementById('recapture-interval');
  const customIntervalGroup = document.getElementById('recapture-custom-interval-group');
  if (intervalSelect && customIntervalGroup) {
    intervalSelect.addEventListener('change', () => {
      customIntervalGroup.hidden = intervalSelect.value !== 'custom';
      if (intervalSelect.value === 'custom') {
        const customInput = document.getElementById('recapture-custom-interval');
        if (customInput) customInput.focus();
      }
    });
  }

  // 장면 임계값 슬라이더 → 표시값 업데이트
  const thresholdSlider = document.getElementById('recapture-scene-threshold');
  const thresholdValue = document.getElementById('recapture-threshold-value');
  if (thresholdSlider && thresholdValue) {
    thresholdSlider.addEventListener('input', () => {
      thresholdValue.textContent = thresholdSlider.value + '%';
    });
  }

  // 오버레이 클릭 시 모달 닫기
  const modal = document.getElementById('recapture-modal');
  if (modal) {
    modal.addEventListener('click', (e) => {
      if (e.target === modal) closeRecaptureModal();
    });
  }

  // ─── 삭제 확인 모달 이벤트 핸들러 ───────────────────────────
  const deleteCloseBtn = document.getElementById('btn-delete-close');
  if (deleteCloseBtn) deleteCloseBtn.addEventListener('click', closeDeleteConfirmModal);

  const deleteCancelBtn = document.getElementById('btn-delete-cancel');
  if (deleteCancelBtn) deleteCancelBtn.addEventListener('click', closeDeleteConfirmModal);

  const deleteConfirmBtn = document.getElementById('btn-delete-confirm');
  if (deleteConfirmBtn) deleteConfirmBtn.addEventListener('click', executeDelete);

  // 삭제 모달 오버레이 클릭 시 닫기
  const deleteModal = document.getElementById('delete-confirm-modal');
  if (deleteModal) {
    deleteModal.addEventListener('click', (e) => {
      if (e.target === deleteModal) closeDeleteConfirmModal();
    });
  }
});

// ─── 삭제 확인 모달 ────────────────────────────────────────────────

/** 현재 삭제 대상 영상 ID */
let _deleteVideoId = null;

/**
 * 라이브러리 항목에 대한 삭제 확인 모달을 열고 대상 정보를 설정한다.
 * @param {string} videoId - 삭제할 영상 ID
 * @param {string} title - 영상 제목 (확인 다이얼로그에 표시)
 */
function openDeleteConfirmModal(videoId, title) {
  const modal = document.getElementById('delete-confirm-modal');
  const titleEl = document.getElementById('delete-confirm-title');
  const confirmBtn = document.getElementById('btn-delete-confirm');

  if (!modal) return;

  _deleteVideoId = videoId;

  if (titleEl) titleEl.textContent = title;
  if (confirmBtn) {
    confirmBtn.disabled = false;
    confirmBtn.textContent = t('library_delete');
  }

  modal.hidden = false;
}

/**
 * 삭제 확인 모달을 닫고 상태를 초기화한다.
 */
function closeDeleteConfirmModal() {
  const modal = document.getElementById('delete-confirm-modal');
  if (modal) modal.hidden = true;
  _deleteVideoId = null;
}

/**
 * 사용자 확인 후 라이브러리 항목을 삭제한다.
 */
async function executeDelete() {
  if (!_deleteVideoId) return;

  const videoId = _deleteVideoId;
  const confirmBtn = document.getElementById('btn-delete-confirm');

  // 더블 클릭 방지를 위해 버튼 비활성화
  if (confirmBtn) {
    confirmBtn.disabled = true;
    confirmBtn.textContent = t('library_deleting') || '...';
  }

  try {
    if (window.__TAURI__ && window.__TAURI__.core) {
      await window.__TAURI__.core.invoke('delete_library_entry', { videoId });
    }
    closeDeleteConfirmModal();
    showToast(t('library_delete_success'), 'success');
    loadLibrary();
  } catch (err) {
    console.error('Failed to delete library entry:', err);
    closeDeleteConfirmModal();
    showToast(t('library_delete_error') + ': ' + err, 'error');
  }
}
