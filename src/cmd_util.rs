//! Utility helpers for spawning subprocesses.

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Extension trait to suppress console window creation on Windows.
///
/// On Windows, child processes spawned from a GUI application (like a Tauri app)
/// would normally open a visible CMD window. Adding `CREATE_NO_WINDOW` (0x08000000)
/// to the process creation flags prevents that flash.
///
/// On non-Windows targets this is a no-op.
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
