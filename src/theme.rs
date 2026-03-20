//! framepick 공유 다크 테마 (글래스모피즘 + 민트 그린)
//!
//! `css_variables_block()`이 반환하는 CSS 커스텀 프로퍼티 블록을
//! slides.html 등 독립 HTML 파일에 임베딩하여 Tauri 앱과 동일한 테마를 적용한다.
//! 값은 반드시 `frontend/css/theme.css`와 일치해야 함.

/// CSS `:root` 블록을 반환한다. `--fp-*` 커스텀 프로퍼티 포함.
///
/// slides.html 등 독립 HTML 파일에 임베딩하여
/// Tauri 앱과 동일한 테마를 적용한다.
/// 값은 반드시 `frontend/css/theme.css`와 일치해야 함.
pub fn css_variables_block() -> &'static str {
    r#":root {
  --fp-bg-primary: #0f172a;
  --fp-bg-secondary: rgba(15, 23, 42, 0.85);
  --fp-bg-card: rgba(15, 23, 42, 0.7);
  --fp-bg-input: rgba(15, 23, 42, 0.8);
  --fp-bg-hover: rgba(30, 41, 59, 0.5);
  --fp-bg-active: rgba(16, 185, 129, 0.1);
  --fp-bg-surface: rgba(30, 41, 59, 0.6);
  --fp-bg-overlay: rgba(0, 0, 0, 0.78);
  --fp-bg-overlay-light: rgba(0, 0, 0, 0.6);
  --fp-bg-image: #000;
  --fp-bg-deep: #020617;
  --fp-bg-deepest: #010409;
  --fp-bg-scrollbar-track: transparent;
  --fp-bg-scrollbar-thumb: rgba(148, 163, 184, 0.2);
  --fp-bg-scrollbar-thumb-hover: rgba(148, 163, 184, 0.35);
  --fp-text-primary: #e2e8f0;
  --fp-text-secondary: #94a3b8;
  --fp-text-muted: #475569;
  --fp-text-heading: #f1f5f9;
  --fp-text-body: #cbd5e1;
  --fp-text-hint: #64748b;
  --fp-text-meta: #64748b;
  --fp-text-footer: #334155;
  --fp-text-on-accent: #fff;
  --fp-accent: #10b981;
  --fp-accent-hover: #34d399;
  --fp-accent-light: #6ee7b7;
  --fp-accent-dim: rgba(16, 185, 129, 0.15);
  --fp-accent-subtle: rgba(16, 185, 129, 0.12);
  --fp-accent-glow: rgba(16, 185, 129, 0.15);
  --fp-accent-bg: rgba(16, 185, 129, 0.08);
  --fp-accent-bg-faint: rgba(16, 185, 129, 0.05);
  --fp-accent-bg-light: rgba(16, 185, 129, 0.1);
  --fp-accent-focus-ring: rgba(16, 185, 129, 0.15);
  --fp-accent-focus-ring-strong: rgba(16, 185, 129, 0.25);
  --fp-accent-shadow: rgba(16, 185, 129, 0.3);
  --fp-accent-border: rgba(16, 185, 129, 0.6);
  --fp-accent-solid: rgba(16, 185, 129, 0.85);
  --fp-border: rgba(148, 163, 184, 0.12);
  --fp-border-focus: #10b981;
  --fp-border-subtle: rgba(148, 163, 184, 0.06);
  --fp-border-translucent: rgba(148, 163, 184, 0.08);
  --fp-border-solid-bg: rgba(30, 41, 59, 0.85);
  --fp-border-solid-bg-full: rgba(30, 41, 59, 1);
  --fp-shadow: rgba(0, 0, 0, 0.3);
  --fp-shadow-deep: rgba(0, 0, 0, 0.4);
  --fp-shadow-card: 0 4px 16px rgba(0, 0, 0, 0.25);
  --fp-shadow-card-hover: 0 8px 32px rgba(16, 185, 129, 0.15);
  --fp-shadow-toggle: 0 2px 12px rgba(16, 185, 129, 0.4);
  --fp-shadow-footer: rgba(0, 0, 0, 0.1);
  --fp-radius: 12px;
  --fp-radius-sm: 6px;
  --fp-radius-lg: 16px;
  --fp-font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans KR', sans-serif;
  --fp-font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  --fp-font-size-xs: 0.7rem;
  --fp-font-size-sm: 0.78rem;
  --fp-font-size-base: 0.95rem;
  --fp-font-size-lg: 1rem;
  --fp-transition: 0.2s ease;
  --fp-transition-fast: 0.15s ease;
  --fp-spacing-xs: 4px;
  --fp-spacing-sm: 8px;
  --fp-spacing-md: 16px;
  --fp-spacing-lg: 24px;
  --fp-spacing-xl: 48px;
  --fp-success: #22c55e;
  --fp-success-light: #4ade80;
  --fp-success-bg: rgba(34, 197, 94, 0.12);
  --fp-error: #ef4444;
  --fp-error-light: #f87171;
  --fp-error-bg: rgba(239, 68, 68, 0.12);
  --fp-error-border: rgba(239, 68, 68, 0.3);
  --fp-warning: #f59e0b;
  --fp-warning-bg: rgba(245, 158, 11, 0.12);
  --fp-warning-border: rgba(245, 158, 11, 0.3);
  --fp-overlay: rgba(0, 0, 0, 0.6);
  --fp-overlay-heavy: rgba(0, 0, 0, 0.78);
  --fp-overlay-light: rgba(0, 0, 0, 0.75);
  --fp-neutral-tint: rgba(148, 163, 184, 0.1);
  --fp-neutral-tint-light: rgba(148, 163, 184, 0.06);
  --fp-neutral-tint-border: rgba(148, 163, 184, 0.15);
  --fp-info-bg: rgba(56, 189, 248, 0.12);
  --fp-select-arrow: #94a3b8;
  --fp-glass-bg: rgba(255, 255, 255, 0.04);
  --fp-glass-bg-strong: rgba(255, 255, 255, 0.07);
  --fp-glass-border: rgba(255, 255, 255, 0.08);
  --fp-glass-blur: 12px;
  --fp-glass-blur-strong: 20px;
}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_variables_block_contains_all_tokens() {
        let css = css_variables_block();
        // Background tokens
        assert!(css.contains("--fp-bg-primary:"));
        assert!(css.contains("--fp-bg-secondary:"));
        assert!(css.contains("--fp-bg-surface:"));
        assert!(css.contains("--fp-bg-card:"));
        assert!(css.contains("--fp-bg-input:"));
        assert!(css.contains("--fp-bg-overlay:"));
        assert!(css.contains("--fp-bg-hover:"));
        assert!(css.contains("--fp-bg-active:"));
        // Text tokens
        assert!(css.contains("--fp-text-primary:"));
        assert!(css.contains("--fp-text-secondary:"));
        assert!(css.contains("--fp-text-body:"));
        assert!(css.contains("--fp-text-heading:"));
        assert!(css.contains("--fp-text-meta:"));
        assert!(css.contains("--fp-text-footer:"));
        // Accent tokens
        assert!(css.contains("--fp-accent:"));
        assert!(css.contains("--fp-accent-hover:"));
        assert!(css.contains("--fp-accent-subtle:"));
        // Border tokens
        assert!(css.contains("--fp-border:"));
        assert!(css.contains("--fp-border-focus:"));
        // Shadow tokens
        assert!(css.contains("--fp-shadow:"));
        assert!(css.contains("--fp-shadow-card:"));
        // Radius tokens
        assert!(css.contains("--fp-radius:"));
        assert!(css.contains("--fp-radius-sm:"));
        assert!(css.contains("--fp-radius-lg:"));
        // Typography tokens
        assert!(css.contains("--fp-font-family:"));
        assert!(css.contains("--fp-font-mono:"));
        // Transition tokens
        assert!(css.contains("--fp-transition:"));
        // Spacing tokens
        assert!(css.contains("--fp-spacing-xs:"));
        assert!(css.contains("--fp-spacing-md:"));
        // Status tokens
        assert!(css.contains("--fp-success:"));
        assert!(css.contains("--fp-error:"));
        assert!(css.contains("--fp-warning:"));
        // Glassmorphism tokens
        assert!(css.contains("--fp-glass-bg:"));
        assert!(css.contains("--fp-glass-blur:"));
    }

    #[test]
    fn css_variables_block_is_valid_css_root() {
        let css = css_variables_block();
        assert!(css.starts_with(":root {"));
        assert!(css.ends_with("}"));
    }
}
