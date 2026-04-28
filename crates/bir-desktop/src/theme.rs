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
    if let Ok(db) = db.lock() {
        if let Ok(val) = serde_json::to_string(preference) {
            let _ = db.set_setting("theme_preference", &val);
        }
    }
    let target_mode = resolve_theme_mode(*preference, window);
    Theme::change(target_mode, Some(window), cx);
}

/// Resolve the effective ThemeMode from an AppThemeMode + current window appearance.
pub fn resolve_theme_mode(preference: AppThemeMode, window: &Window) -> ThemeMode {
    match preference {
        AppThemeMode::Light => ThemeMode::Light,
        AppThemeMode::Dark => ThemeMode::Dark,
        AppThemeMode::System => match window.appearance() {
            gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => ThemeMode::Light,
            gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => ThemeMode::Dark,
        },
    }
}
