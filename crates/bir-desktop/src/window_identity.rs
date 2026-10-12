//! Window identity for Linux docks and launchers.
//!
//! GPUI writes `app_id` as the X11 `WM_CLASS` / Wayland app id and `icon` as the
//! X11 `_NET_WM_ICON`; other platforms ignore both. Without them a dock cannot
//! match an open window to the installed `dev.goldcoders.bir.desktop` entry and
//! shows a generic window icon instead of the app's.

use gpui::WindowOptions;

/// Must match the desktop entry's file name, `Icon=` and `StartupWMClass=`
/// (`assets/linux/dev.goldcoders.bir.desktop`).
pub(crate) const APP_ID: &str = "dev.goldcoders.bir";

/// Tags `options` with the app id and window icon unless the caller set its own.
pub(crate) fn identified(options: WindowOptions) -> WindowOptions {
    WindowOptions {
        app_id: options.app_id.or_else(|| Some(APP_ID.to_string())),
        icon: options.icon.or_else(window_icon),
        ..options
    }
}

/// 128px keeps `_NET_WM_ICON` (4 bytes per pixel) under the core X11 request
/// limit; GPUI fails window creation if that property write fails.
#[cfg(target_os = "linux")]
fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    use std::sync::{Arc, OnceLock};

    static ICON: OnceLock<Option<Arc<image::RgbaImage>>> = OnceLock::new();
    ICON.get_or_init(|| {
        let png = include_bytes!(
            "../../../assets/linux/icons/hicolor/128x128/apps/dev.goldcoders.bir.png"
        );
        match image::load_from_memory(png) {
            Ok(img) => Some(Arc::new(img.into_rgba8())),
            Err(error) => {
                tracing::warn!(%error, "window icon failed to decode; continuing without it");
                None
            }
        }
    })
    .clone()
}

#[cfg(not(target_os = "linux"))]
fn window_icon() -> Option<std::sync::Arc<image::RgbaImage>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESKTOP_ENTRY: &str = include_str!("../../../assets/linux/dev.goldcoders.bir.desktop");

    fn entry(key: &str) -> Option<&'static str> {
        DESKTOP_ENTRY
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    }

    #[test]
    fn desktop_entry_matches_app_id() {
        assert_eq!(entry("Icon"), Some(APP_ID));
        assert_eq!(entry("StartupWMClass"), Some(APP_ID));
        // The .deb installs the binary as /usr/bin/ebirforms.
        assert_eq!(entry("Exec"), Some("ebirforms"));
    }

    #[test]
    fn identified_fills_app_id_and_keeps_caller_choice() {
        assert_eq!(
            identified(WindowOptions::default()).app_id.as_deref(),
            Some(APP_ID)
        );

        let custom = WindowOptions {
            app_id: Some("custom".into()),
            ..Default::default()
        };
        assert_eq!(identified(custom).app_id.as_deref(), Some("custom"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_window_icon_is_a_small_square() {
        let icon = window_icon().expect("embedded icon decodes");
        assert_eq!(icon.dimensions(), (128, 128));
    }
}
