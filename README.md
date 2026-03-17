# FramePick

YouTube 영상에서 프레임을 손쉽게 캡쳐하는 데스크톱 애플리케이션입니다. **Tauri 2** 기반의 경량 크로스플랫폼 앱으로, 3가지 캡쳐 모드와 자동 도구 설치, 다국어 지원을 제공합니다. 캡쳐한 프레임은 인터랙티브한 HTML 슬라이드쇼(slides.html)로 자동 생성됩니다.

## 주요 기능

- **3가지 캡쳐 모드**
  - 자막 구간별: 각 자막 세그먼트 시작 시점에서 프레임 캡쳐
  - 장면 변화 감지: 영상의 장면이 바뀌는 순간을 감지하여 자동 캡쳐
  - 고정 간격: 사용자가 지정한 시간 간격으로 프레임 캡쳐

- **자동 도구 설치**: ffmpeg, yt-dlp를 첫 실행 시 자동으로 다운로드하고 설정
- **슬라이드 뷰어**: 캡쳐한 프레임을 독립 HTML 파일(slides.html)로 변환하여 브라우저에서 열람 가능
- **라이브러리 관리**: 이전 캡쳐 작업을 저장하고, 다른 캡쳐 모드로 재캡쳐 가능
- **플레이리스트 지원**: YouTube 재생목록의 여러 영상을 한 번에 캡쳐
- **다국어 지원**: 한국어, 영어 UI 제공
- **다크 테마**: 글래스모피즘 디자인과 민트 그린 액센트 컬러

## 설치 및 빌드

### 사전 요구사항

- **Rust 1.70+**: [https://rustup.rs](https://rustup.rs) 에서 설치
- **Node.js 18+** (선택): 프론트엔드 변경 시에만 필요

### 개발 환경 실행

```bash
# 저장소 복제
git clone https://github.com/yourusername/framepick.git
cd framepick

# 의존성 설치 및 개발 모드 실행
cargo tauri dev
```

앱이 열리면 자동으로 ffmpeg와 yt-dlp가 다운로드됩니다(첫 실행 시).

### 프로덕션 빌드

```bash
# 최적화된 바이너리 생성
cargo tauri build
```

생성된 실행 파일은 `src-tauri/target/release/` 디렉토리에 위치합니다.

## 프로젝트 구조

### 백엔드 (Rust - `src/`)

Tauri 커맨드 핸들러 및 핵심 로직:

```
src/
├── main.rs                  # 앱 진입점 (CLI 엔트리)
├── lib.rs                   # 라이브러리 진입점, Tauri 빌더 설정
├── capture.rs               # 프레임 캡쳐 엔진 (ffmpeg 래퍼)
├── capture_fallback.rs      # 캡쳐 모드 자동 결정 로직
├── cleanup.rs               # 임시 파일 및 리소스 정리
├── cmd_util.rs              # 외부 명령어 실행 유틸리티
├── config.rs                # 설정 파일 경로 및 마이그레이션
├── downloader.rs            # ffmpeg, yt-dlp 다운로드 및 설치
├── input_state.rs           # 작업 큐 및 파이프라인 상태 관리
├── metadata.rs              # 캡쳐 메타데이터 (제목, 설명 등)
├── playlist.rs              # YouTube 재생목록 처리
├── progress.rs              # 진행률 및 이벤트 스트림
├── queue_processor.rs        # 큐 아이템 순차 처리 엔진
├── settings.rs              # 설정 로드/저장 (라이브러리 경로, 품질 등)
├── slides_generator.rs       # slides.html 생성 로직
├── slides_viewer.rs         # 라이브러리 조회 및 슬라이드 관리 API
├── subtitle_detector.rs     # 자막 유무 감지
├── subtitle_extractor.rs    # ffprobe를 이용한 자막 추출 및 언어 선택
├── theme.rs                 # 앱 테마 설정 및 색상 정의
├── tools_manager.rs         # ffmpeg, yt-dlp 버전 관리
└── url_validator.rs         # YouTube URL 검증
```

### 프론트엔드 (바닐라 JS/CSS - `frontend/`)

```
frontend/
├── index.html               # 메인 HTML 템플릿
├── css/
│   ├── theme.css            # CSS 변수, 컬러 팔레트, 글래스모피즘 스타일
│   └── style.css            # 레이아웃, 컴포넌트, 모달, 반응형 디자인
├── js/
│   ├── app.js               # 앱 초기화 및 라우팅
│   ├── capture-mode.js      # 캡쳐 모드 선택 UI 및 옵션 관리
│   ├── capture-list.js      # 진행 중인 작업 목록 표시
│   ├── i18n.js              # 다국어 번역 시스템 (한국어/영어)
│   ├── playlist.js          # 재생목록 선택 모달
│   ├── progress.js          # 진행률 표시 및 실시간 업데이트
│   ├── queue.js             # 작업 큐 UI 관리
│   ├── settings.js          # 설정 모달 (경로, 품질, 언어, 도구 상태)
│   ├── slides-viewer.js     # slides.html 뷰어 및 라이브러리 관리
│   ├── state.js             # 클라이언트 상태 관리 (Tauri IPC 연동)
│   └── url-input.js         # URL 입력 필드 및 유효성 검사
└── icons/
    ├── icon.png             # 앱 아이콘 (PNG)
    └── icon.ico             # 앱 아이콘 (Windows ICO)
```

## 기술 스택

### 백엔드
- **Tauri 2**: 경량 데스크톱 앱 프레임워크
- **Rust 2021**: 시스템 프로그래밍 언어
- **Tokio**: 비동기 런타임
- **serde**: JSON 직렬화/역직렬화
- **reqwest**: HTTP 클라이언트 (도구 다운로드)
- **zip**: ZIP 파일 처리

### 프론트엔드
- **바닐라 JavaScript**: 프레임워크 미사용, 순수 DOM 조작
- **CSS3**: 글래스모피즘, CSS 변수, 그리드/플렉스 레이아웃
- **HTML5**: 시맨틱 마크업

### 외부 도구 (자동 설치)
- **ffmpeg**: 영상 프레임 추출
- **yt-dlp**: YouTube 영상 다운로드 및 메타데이터 조회
- **ffprobe**: 영상 메타데이터 및 자막 정보 조회

## 주요 워크플로우

1. **URL 입력**: YouTube 영상 또는 재생목록 URL 입력
2. **캡쳐 모드 선택**: 3가지 캡쳐 모드 중 선택 (기본값은 설정에서 변경 가능)
3. **영상 다운로드**: yt-dlp를 사용하여 지정된 품질로 MP4 다운로드
4. **자막 검사**: 영상의 자막 가용성 및 언어 확인
5. **자막 추출** (모드별):
   - 자막 구간별: 자막이 있으면 추출하여 사용, 없으면 고정 간격 자동 전환
   - 장면 변화 감지: 자막 무시하고 ffmpeg 필터로 장면 변화 감지
   - 고정 간격: 사용자 지정 간격으로 프레임 추출
6. **프레임 캡쳐**: ffmpeg으로 지정된 시점의 프레임을 JPEG/PNG로 저장
7. **슬라이드 생성**: 캡쳐 이미지와 메타데이터를 사용하여 독립 HTML 슬라이드쇼 생성
8. **라이브러리 저장**: 작업 정보와 이미지를 library 폴더에 저장

## 라이선스

MIT License - 자유롭게 사용, 수정, 배포할 수 있습니다.
