//! Shared dark theme constants for framepick.
//!
//! These values mirror `src-ui/theme.css` so that generated HTML
//! files (slides.html) use the same visual language as the Tauri app UI.
//! When updating the theme, change both this file and `theme.css`.

// ── Background colors ──

pub const BG_BODY: &str = "#1a1a2e";
pub const BG_SIDEBAR: &str = "#16213e";
pub const BG_SURFACE: &str = "#0f3460";
pub const BG_CARD: &str = "#1e293b";
pub const BG_INPUT: &str = "#0f172a";
pub const BG_OVERLAY: &str = "rgba(0, 0, 0, 0.78)";
pub const BG_OVERLAY_LIGHT: &str = "rgba(0, 0, 0, 0.6)";
pub const BG_HOVER: &str = "#2d3a50";
pub const BG_ACTIVE: &str = "#334155";
pub const BG_IMAGE: &str = "#000";
pub const BG_SCROLLBAR_TRACK: &str = "transparent";
pub const BG_SCROLLBAR_THUMB: &str = "#333";
pub const BG_SCROLLBAR_THUMB_HOVER: &str = "#555";

// ── Text colors ──

pub const TEXT_PRIMARY: &str = "#e2e8f0";
pub const TEXT_SECONDARY: &str = "#94a3b8";
pub const TEXT_HINT: &str = "#7a7a94";
pub const TEXT_MUTED: &str = "#64748b";
pub const TEXT_HEADING: &str = "#ffffff";
pub const TEXT_BODY: &str = "#d4d4d4";
pub const TEXT_META: &str = "#888";
pub const TEXT_FOOTER: &str = "#444";

// ── Accent colors ──

pub const ACCENT: &str = "#3b82f6";
pub const ACCENT_HOVER: &str = "#2563eb";
pub const ACCENT_LIGHT: &str = "#60a5fa";
pub const ACCENT_SUBTLE: &str = "rgba(59, 130, 246, 0.12)";

// ── Border colors ──

pub const BORDER: &str = "#334155";
pub const BORDER_FOCUS: &str = "#3b82f6";
pub const BORDER_SUBTLE: &str = "#1e293b";

// ── Shadows ──

pub const SHADOW: &str = "rgba(0, 0, 0, 0.3)";
pub const SHADOW_CARD: &str = "0 2px 12px rgba(0, 0, 0, 0.5)";
pub const SHADOW_CARD_HOVER: &str = "0 4px 24px rgba(59, 130, 246, 0.12)";
pub const SHADOW_TOGGLE: &str = "0 2px 12px rgba(59, 130, 246, 0.4)";

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
/// CSS file.  Values here MUST match `src-ui/theme.css`.
pub fn css_variables_block() -> &'static str {
    r#":root {
  --fp-bg-primary: #1a1a2e;
  --fp-bg-secondary: #16213e;
  --fp-bg-card: #1e293b;
  --fp-bg-input: #0f172a;
  --fp-bg-hover: #2d3a50;
  --fp-bg-active: #334155;
  --fp-bg-surface: #0f3460;
  --fp-bg-overlay: rgba(0, 0, 0, 0.78);
  --fp-bg-overlay-light: rgba(0, 0, 0, 0.6);
  --fp-bg-image: #000;
  --fp-bg-deep: #111;
  --fp-bg-deepest: #0f0f0f;
  --fp-bg-scrollbar-track: transparent;
  --fp-bg-scrollbar-thumb: #333;
  --fp-bg-scrollbar-thumb-hover: #555;
  --fp-text-primary: #e2e8f0;
  --fp-text-secondary: #94a3b8;
  --fp-text-muted: #64748b;
  --fp-text-heading: #ffffff;
  --fp-text-body: #d4d4d4;
  --fp-text-hint: #7a7a94;
  --fp-text-meta: #888;
  --fp-text-footer: #444;
  --fp-text-on-accent: #fff;
  --fp-accent: #3b82f6;
  --fp-accent-hover: #2563eb;
  --fp-accent-light: #60a5fa;
  --fp-accent-dim: rgba(59, 130, 246, 0.15);
  --fp-accent-subtle: rgba(59, 130, 246, 0.12);
  --fp-accent-glow: rgba(59, 130, 246, 0.15);
  --fp-accent-bg: rgba(59, 130, 246, 0.08);
  --fp-accent-bg-faint: rgba(59, 130, 246, 0.05);
  --fp-accent-bg-light: rgba(59, 130, 246, 0.1);
  --fp-accent-focus-ring: rgba(59, 130, 246, 0.15);
  --fp-accent-focus-ring-strong: rgba(59, 130, 246, 0.25);
  --fp-accent-shadow: rgba(59, 130, 246, 0.3);
  --fp-accent-border: rgba(59, 130, 246, 0.6);
  --fp-accent-solid: rgba(59, 130, 246, 0.85);
  --fp-border: #334155;
  --fp-border-focus: #3b82f6;
  --fp-border-subtle: #1e293b;
  --fp-border-translucent: rgba(51, 65, 85, 0.4);
  --fp-border-solid-bg: rgba(51, 65, 85, 0.85);
  --fp-border-solid-bg-full: rgba(51, 65, 85, 1);
  --fp-shadow: rgba(0, 0, 0, 0.3);
  --fp-shadow-deep: rgba(0, 0, 0, 0.4);
  --fp-shadow-card: 0 2px 12px rgba(0, 0, 0, 0.5);
  --fp-shadow-card-hover: 0 4px 24px rgba(59, 130, 246, 0.12);
  --fp-shadow-toggle: 0 2px 12px rgba(59, 130, 246, 0.4);
  --fp-shadow-footer: rgba(0, 0, 0, 0.1);
  --fp-radius: 8px;
  --fp-radius-sm: 4px;
  --fp-radius-lg: 12px;
  --fp-font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans KR', sans-serif;
  --fp-font-mono: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
  --fp-font-size-xs: 0.7rem;
  --fp-font-size-sm: 0.78rem;
  --fp-font-size-base: 0.95rem;
  --fp-font-size-lg: 1rem;
  --fp-transition: 0.2s ease;
  --fp-transition-fast: 0.18s ease;
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
  --fp-neutral-tint: rgba(148, 163, 184, 0.15);
  --fp-neutral-tint-light: rgba(148, 163, 184, 0.12);
  --fp-neutral-tint-border: rgba(148, 163, 184, 0.3);
  --fp-info-bg: rgba(106, 153, 255, 0.15);
  --fp-select-arrow: #94a3b8;
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
