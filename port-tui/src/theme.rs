//! One Dark theme — semantic color slots.
//!
//! Phase 1 uses a single hardcoded theme. Theme switching (6 presets,
//! TOML files, `t` key toggle) is Phase 6.

use ratatui::style::Color;

/// Semantic color slots mapped to One Dark palette values.
pub struct Theme {
    pub fg_default: Color,
    pub fg_muted: Color,
    pub fg_emphasis: Color,
    pub bg_base: Color,
    pub bg_surface: Color,
    pub bg_overlay: Color,
    pub bg_selection: Color,
    pub accent_primary: Color,
    pub status_success: Color,
    pub status_warning: Color,
    pub status_error: Color,
    pub status_info: Color,
}

/// One Dark color palette.
pub fn one_dark() -> Theme {
    Theme {
        fg_default: Color::Rgb(171, 178, 191),
        fg_muted: Color::Rgb(92, 99, 112),
        fg_emphasis: Color::Rgb(229, 229, 229),
        bg_base: Color::Rgb(26, 27, 38),
        bg_surface: Color::Rgb(36, 40, 59),
        bg_overlay: Color::Rgb(44, 48, 67),
        bg_selection: Color::Rgb(54, 74, 130),
        accent_primary: Color::Rgb(97, 175, 239),
        status_success: Color::Rgb(152, 195, 121),
        status_warning: Color::Rgb(229, 192, 123),
        status_error: Color::Rgb(224, 108, 117),
        status_info: Color::Rgb(86, 182, 194),
    }
}

/// Default theme. Hardcoded to One Dark for Phase 1.
pub fn default_theme() -> Theme {
    one_dark()
}
