//! Shared dark theme constants for framepick.
//!
//! These values mirror `frontend/css/theme.css` so that generated HTML
//! files (slides.html) use the same visual language as the Tauri app UI.
//! When updating the theme, change both this file and `theme.css`.

// ── Background colors ──

pub const BG_BODY: &str = "#1a1a2e";
pub const BG_SIDEBAR: &str = "#16213e";
pub const BG_SURFACE: &str = "#0f3460";
pub const BG_CARD: &str = "#1a1a2e";
pub const BG_INPUT: &str = "#1a1a3e";
pub const BG_OVERLAY: &str = "rgba(0, 0, 0, 0.78)";
pub const BG_OVERLAY_LIGHT: &str = "rgba(0, 0, 0, 0.6)";
pub const BG_HOVER: &str = "#1e1e32";
pub const BG_ACTIVE: &str = "#252540";
pub const BG_IMAGE: &str = "#000";
pub const BG_SCROLLBAR_TRACK: &str = "transparent";
pub const BG_SCROLLBAR_THUMB: &str = "#333";
pub const BG_SCROLLBAR_THUMB_HOVER: &str = "#555";

// ── Text colors ──

pub const TEXT_PRIMARY: &str = "#e0e0e0";
pub const TEXT_SECONDARY: &str = "#a0a0b8";
pub const TEXT_HINT: &str = "#7a7a94";
pub const TEXT_MUTED: &str = "#666";
pub const TEXT_HEADING: &str = "#ffffff";
pub const TEXT_BODY: &str = "#d4d4d4";
pub const TEXT_META: &str = "#888";
pub const TEXT_FOOTER: &str = "#444";

// ── Accent colors ──

pub const ACCENT: &str = "#e94560";
pub const ACCENT_HOVER: &str = "#ff6b81";
pub const ACCENT_LIGHT: &str = "#ff8fa3";
pub const ACCENT_SUBTLE: &str = "rgba(233, 69, 96, 0.12)";

// ── Border colors ──

pub const BORDER: &str = "#2a2a4a";
pub const BORDER_FOCUS: &str = "#e94560";
pub const BORDER_SUBTLE: &str = "#1a1a2a";

// ── Shadows ──

pub const SHADOW: &str = "rgba(0, 0, 0, 0.3)";
pub const SHADOW_CARD: &str = "0 2px 12px rgba(0, 0, 0, 0.5)";
pub const SHADOW_CARD_HOVER: &str = "0 4px 24px rgba(233, 69, 96, 0.12)";
pub const SHADOW_TOGGLE: &str = "0 2px 12px rgba(233, 69, 96, 0.4)";

// ── Border radius ──

pub const RADIUS: &str = "8px";
pub const RADIUS_SM: &str = "4px";
pub const RADIUS_LG: &str = "12px";

// ── Typography ──

pub const FONT_FAMILY: &str =
    "-apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans KR', sans-serif";
pub const FONT_MONO: &str = "'JetBrains Mono', 'Fira Code', 'Consolas', monospace";

// ── Transition ──

pub const TRANSITION: &str = "0.2s ease";
pub const TRANSITION_FAST: &str = "0.18s ease";

/// Returns the theme as a CSS `:root` block with `--fp-*` custom properties.
///
/// This is intended to be embedded in standalone HTML files (e.g. slides.html)
/// so they share the same theme as the Tauri app without needing an external
/// CSS file.
pub fn css_variables_block() -> &'static str {
    r#":root {
  --fp-bg-body: #1a1a2e;
  --fp-bg-sidebar: #16213e;
  --fp-bg-surface: #0f3460;
  --fp-bg-card: #1a1a2e;
  --fp-bg-input: #1a1a3e;
  --fp-bg-overlay: rgba(0, 0, 0, 0.78);
  --fp-bg-overlay-light: rgba(0, 0, 0, 0.6);
  --fp-bg-hover: #1e1e32;
  --fp-bg-active: #252540;
  --fp-bg-image: #000;
  --fp-bg-scrollbar-track: transparent;
  --fp-bg-scrollbar-thumb: #333;
  --fp-bg-scrollbar-thumb-hover: #555;
  --fp-text-primary: #e0e0e0;
  --fp-text-secondary: #a0a0b8;
  --fp-text-hint: #7a7a94;
  --fp-text-muted: #666;
  --fp-text-heading: #ffffff;
  --fp-text-body: #d4d4d4;
  --fp-text-meta: #888;
  --fp-text-footer: #444;
  --fp-accent: #e94560;
  --fp-accent-hover: #ff6b81;
  --fp-accent-light: #ff8fa3;
  --fp-accent-subtle: rgba(233, 69, 96, 0.12);
  --fp-border: #2a2a4a;
  --fp-border-focus: #e94560;
  --fp-border-subtle: #1a1a2a;
  --fp-shadow: rgba(0, 0, 0, 0.3);
  --fp-shadow-card: 0 2px 12px rgba(0, 0, 0, 0.5);
  --fp-shadow-card-hover: 0 4px 24px rgba(233, 69, 96, 0.12);
  --fp-shadow-toggle: 0 2px 12px rgba(233, 69, 96, 0.4);
  --fp-radius: 8px;
  --fp-radius-sm: 4px;
  --fp-radius-lg: 12px;
  --fp-font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans KR', sans-serif;
  --fp-font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  --fp-transition: 0.2s ease;
  --fp-transition-fast: 0.18s ease;
  --fp-spacing-xs: 4px;
  --fp-spacing-sm: 8px;
  --fp-spacing-md: 16px;
  --fp-spacing-lg: 24px;
  --fp-spacing-xl: 48px;
  --fp-select-arrow: #a0a0b8;
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
  --fp-bg-deep: #111;
  --fp-bg-deepest: #0f0f0f;
  --fp-text-on-accent: #fff;
  --fp-accent-glow: rgba(233, 69, 96, 0.15);
  --fp-accent-bg: rgba(233, 69, 96, 0.08);
  --fp-accent-bg-faint: rgba(233, 69, 96, 0.05);
  --fp-accent-bg-light: rgba(233, 69, 96, 0.1);
  --fp-accent-focus-ring: rgba(233, 69, 96, 0.15);
  --fp-accent-focus-ring-strong: rgba(233, 69, 96, 0.25);
  --fp-accent-shadow: rgba(233, 69, 96, 0.3);
  --fp-accent-border: rgba(233, 69, 96, 0.6);
  --fp-accent-solid: rgba(233, 69, 96, 0.85);
  --fp-overlay: rgba(0, 0, 0, 0.6);
  --fp-overlay-heavy: rgba(0, 0, 0, 0.78);
  --fp-overlay-light: rgba(0, 0, 0, 0.75);
  --fp-shadow-deep: rgba(0, 0, 0, 0.4);
  --fp-shadow-footer: rgba(0, 0, 0, 0.1);
  --fp-neutral-tint: rgba(160, 160, 184, 0.15);
  --fp-neutral-tint-light: rgba(160, 160, 184, 0.12);
  --fp-neutral-tint-border: rgba(160, 160, 184, 0.3);
  --fp-border-translucent: rgba(42, 42, 74, 0.4);
  --fp-border-solid-bg: rgba(42, 42, 74, 0.85);
  --fp-border-solid-bg-full: rgba(42, 42, 74, 1);
  --fp-info-bg: rgba(106, 153, 255, 0.15);
}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_variables_block_contains_all_tokens() {
        let css = css_variables_block();
        // Background tokens
        assert!(css.contains("--fp-bg-body:"));
        assert!(css.contains("--fp-bg-sidebar:"));
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
    }

    #[test]
    fn constants_match_css_variables_block() {
        let css = css_variables_block();
        // Verify key constants match the CSS block values
        assert!(css.contains(BG_BODY));
        assert!(css.contains(BG_SIDEBAR));
        assert!(css.contains(TEXT_PRIMARY));
        assert!(css.contains(ACCENT));
        assert!(css.contains(BORDER));
        assert!(css.contains(RADIUS));
        assert!(css.contains(FONT_MONO));
    }

    #[test]
    fn css_variables_block_is_valid_css_root() {
        let css = css_variables_block();
        assert!(css.starts_with(":root {"));
        assert!(css.ends_with("}"));
    }
}
