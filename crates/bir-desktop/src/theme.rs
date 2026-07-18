//! Application theme management — cycle between Light/Dark/System.

use gpui::*;
use gpui_component::*;

use crate::app::AppThemeMode;
use bir_core::db::Database;
use std::sync::{Arc, Mutex};

/// Cycle the theme preference and persist to database.
pub fn cycle_theme(
    preference: &mut AppThemeMode,
    db: &Arc<Mutex<Database>>,
    window: &mut Window,
    cx: &mut App,
) {
    *preference = preference.next();
    if let Ok(db) = db.lock()
        && let Ok(val) = serde_json::to_string(preference)
    {
        let _ = db.set_setting("theme_preference", &val);
    }
    let target_mode = resolve_theme_mode(*preference, window);
    Theme::change(target_mode, Some(window), cx);
}

/// Darken a color by scaling its HSL lightness. Used to derive a legible
/// light-mode text hue from the theme's mid-lightness semantic colors.
fn darkened(mut color: Hsla, factor: f32) -> Hsla {
    color.l = (color.l * factor).clamp(0.0, 1.0);
    color
}

/// Text color for content sitting on a faint `warning.opacity(..)` tint.
///
/// The theme's `warning_foreground` is meant for text on a SOLID warning
/// fill; in the light theme it is near-white and disappears on a pale tint
/// over the white page background. Dark mode can use the warning hue as-is;
/// light mode needs it darkened toward amber-700.
pub fn warning_on_tint(theme: &Theme) -> Hsla {
    if theme.mode.is_dark() {
        theme.warning
    } else {
        darkened(theme.warning, 0.55)
    }
}

/// Text color for content sitting on a faint `danger.opacity(..)` tint or the
/// plain page background. See [`warning_on_tint`].
pub fn danger_on_tint(theme: &Theme) -> Hsla {
    if theme.mode.is_dark() {
        theme.danger
    } else {
        darkened(theme.danger, 0.8)
    }
}

/// Text color for content sitting on a faint `success.opacity(..)` tint or
/// the plain page background. See [`warning_on_tint`].
pub fn success_on_tint(theme: &Theme) -> Hsla {
    if theme.mode.is_dark() {
        theme.success
    } else {
        darkened(theme.success, 0.7)
    }
}

/// Generic variant of the `*_on_tint` helpers for arbitrary hue chips: text
/// legible on a faint tint of the same hue in both themes.
pub fn hue_on_tint(theme: &Theme, hue: Hsla) -> Hsla {
    if theme.mode.is_dark() {
        hue
    } else {
        darkened(hue, 0.7)
    }
}

/// Resolve the effective ThemeMode from an AppThemeMode + current window appearance.
pub fn resolve_theme_mode(preference: AppThemeMode, window: &Window) -> ThemeMode {
    match preference {
        AppThemeMode::Light => ThemeMode::Light,
        AppThemeMode::Dark => ThemeMode::Dark,
        AppThemeMode::System => match window.appearance() {
            gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => {
                ThemeMode::Light
            }
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ThemeMode::Dark,
        },
    }
}
