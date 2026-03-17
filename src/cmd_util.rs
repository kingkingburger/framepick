//! 서브프로세스 생성을 위한 유틸리티 헬퍼 모듈.

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows에서 콘솔 창 생성을 억제하는 확장 트레잇.
///
/// Windows에서 GUI 앱(Tauri 앱 등)이 자식 프로세스를 생성하면
/// 일반적으로 CMD 창이 잠깐 나타난다. 프로세스 생성 플래그에
/// `CREATE_NO_WINDOW` (0x08000000)를 추가하면 이를 방지한다.
///
/// 비-Windows 대상에서는 no-op이다.
pub trait HideWindow {
    fn hide_window(&mut self) -> &mut Self;
}

impl HideWindow for std::process::Command {
    #[cfg(windows)]
    fn hide_window(&mut self) -> &mut Self {
        self.creation_flags(0x08000000) // CREATE_NO_WINDOW
    }

    #[cfg(not(windows))]
    fn hide_window(&mut self) -> &mut Self {
        self
    }
}
