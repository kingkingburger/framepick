//! framepick 앱 진입점.
//!
//! Windows 릴리즈 빌드에서 콘솔 창이 뜨지 않도록 설정하고,
//! 실제 앱 초기화는 `framepick_lib::run()`에 위임한다.

// 릴리즈 빌드에서 Windows 콘솔 창 표시 억제
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    framepick_lib::run();
}
