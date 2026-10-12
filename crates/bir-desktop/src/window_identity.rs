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
        let png = include_bytes!("../embedded/dev.goldcoders.bir-128.png");
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

    /// The repo's launcher file, or `None` in a packaged crate (assets/ is not shipped).
    fn desktop_entry() -> Option<String> {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../assets/linux/dev.goldcoders.bir.desktop"),
        )
        .ok()
    }

    #[test]
    fn desktop_entry_matches_app_id() {
        let Some(text) = desktop_entry() else {
            return;
        };
        let entry = |key: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
        };
        assert_eq!(entry("Icon"), Some(APP_ID));
        assert_eq!(entry("StartupWMClass"), Some(APP_ID));
        // The .deb installs the binary as /usr/bin/ebirforms.
        assert_eq!(entry("Exec"), Some("ebirforms"));
    }

    /// The crate-local copies embedded in the binary must match the repo
    /// originals that the installers ship (checked only inside the repo).
    #[test]
    fn embedded_copies_match_repo_originals() {
        let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo = crate_dir.join("../..");
        if !repo.join("assets").is_dir() {
            return;
        }
        for (embedded, original) in [
            ("embedded/e_logo.png", "assets/images/e_logo.png"),
            (
                "embedded/dev.goldcoders.bir-128.png",
                "assets/linux/icons/hicolor/128x128/apps/dev.goldcoders.bir.png",
            ),
            (
                "embedded/rules/2551q-v2018-fields.json",
                "rules/forms/2551q-v2018/fields.json",
            ),
            (
                "embedded/rules/1601c-v2018-fields.json",
                "rules/forms/1601c-v2018/fields.json",
            ),
        ] {
            assert_eq!(
                std::fs::read(crate_dir.join(embedded)).unwrap(),
                std::fs::read(repo.join(original)).unwrap(),
                "{embedded} drifted from {original}; copy the original over it"
            );
        }
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
