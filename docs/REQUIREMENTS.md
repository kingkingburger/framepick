# framepick - 요구사항 정의서

## 한 줄 요약

YouTube URL을 입력하면 자막 구간별 프레임을 캡쳐하고 슬라이드로 생성·열람할 수 있는 Windows 데스크톱 앱

## 원본 요구사항

> "youtube-slides를 rust를 활용해서 desktop app처럼 쓰고 싶어"

## 기능 범위

### 핵심 파이프라인

```
YouTube URL 입력
  → 메타데이터 추출 (제목, 채널, 날짜, 길이)
  → 자막 추출 (ko > en, JSON3 포맷)
  → 영상 다운로드 (720p 이하)
  → 프레임 캡쳐 (자막 구간별 / 장면 변화 / 고정 간격)
  → 슬라이드 생성 (마크다운 + HTML)
  → 앱 내 뷰어에서 바로 열람
```

### 뷰어

- 생성된 슬라이드를 앱 내에서 바로 열람
- 타임스탬프별 프레임 + 자막 텍스트 표시
- 목차 네비게이션

### 라이브러리

- 이전에 생성한 슬라이드 목록을 썸네일로 표시
- 재열람 가능

## 기술 스택

| 영역 | 선택 | 이유 |
|------|------|------|
| 프레임워크 | **Tauri** | Rust 백엔드 + 웹뷰 프론트. 번들 크기 작고 웹 기술 활용 가능 |
| 프론트엔드 | Tauri 웹뷰 (HTML/CSS/JS) | 기존 slides.html 다크 테마 스타일 재활용 가능 |
| 백엔드 로직 | **Rust** | Python 스크립트 전체 재작성. 자체 완결형 |
| 외부 도구 | **yt-dlp, ffmpeg 번들링** | 사용자 PC에 별도 설치 불필요 |
| 플랫폼 | **Windows 전용** | 빌드/테스트 단순화 |
| 배포 | **포터블 .exe** | 설치 없이 실행 파일 하나로 동작 |

## 재작성 대상 (Python → Rust)

| Python 스크립트 | Rust 대체 | 핵심 로직 |
|----------------|-----------|-----------|
| `extract_metadata.sh` | yt-dlp CLI 호출 래퍼 | `--dump-json` 파싱 |
| `extract_transcript.sh` | yt-dlp CLI 호출 래퍼 | `--write-auto-sub --sub-lang ko,en --convert-subs json3` |
| `download_video.sh` | yt-dlp CLI 호출 래퍼 | `-f bestvideo[height<=720]+bestaudio` |
| `capture_frames.py` | ffmpeg CLI 호출 + JSON3 파싱 | 세그먼트 병합, `-ss` seek, `-frames:v 1` |
| `generate_output.py` | Rust 템플릿 엔진 | 마크다운/HTML 생성 |

## 캡쳐 모드 (3가지)

| 모드 | 설명 | ffmpeg 옵션 |
|------|------|-------------|
| 자막 구간별 (기본) | 각 자막 세그먼트 시작 시점 캡쳐 | `-ss {ts} -frames:v 1` |
| 장면 변화 감지 | 화면 변화도 30% 이상 시 캡쳐 | `select='gt(scene,0.3)'` |
| 고정 간격 | N초마다 캡쳐 (10/30/60초) | `fps=1/N` |

## 출력 구조

```
{라이브러리_경로}/{video-id}/
├── slides.md
├── slides.html
├── segments.json
├── images/
│   ├── frame_0000_00-00.jpg
│   └── ...
└── source/
    ├── {video-id}.mp4
    └── {video-id}.ko.json3
```

## 번들링 대상

| 바이너리 | 용도 | 크기 (대략) |
|----------|------|------------|
| yt-dlp.exe | YouTube 메타/자막/영상 다운로드 | ~10MB |
| ffmpeg.exe | 프레임 캡쳐 | ~80-130MB |

## 원본 스킬 참조

- `D:/reference2/plugin-mh/skills/youtube-slides/` - 원본 Claude Code 스킬
- 스킬의 HTML 출력 스타일(다크 테마, YouTube 스타일)을 뷰어 디자인에 참고
