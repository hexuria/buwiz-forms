//! What `QuitApplication` (Cmd+Q, the app menu's Quit, agent `app.quit`;
//! Ctrl+Q / Alt+F4 elsewhere) does, and the macOS setting that chooses it.

/// Settings key for "Cmd+Q hides to menu bar" (macOS). Off unless stored as
/// "true", so Cmd+Q and the app menu's Quit really quit by default.
pub(crate) const QUIT_SHORTCUT_HIDES_SETTING: &str = "cmd_q_hides_to_menu_bar";

pub(crate) fn quit_shortcut_hides(db: &bir_core::db::Database) -> bool {
    db.get_setting(QUIT_SHORTCUT_HIDES_SETTING)
        .ok()
        .flatten()
        .as_deref()
        == Some("true")
}

pub(crate) fn set_quit_shortcut_hides(db: &bir_core::db::Database, hides: bool) {
    let _ = db.set_setting(
        QUIT_SHORTCUT_HIDES_SETTING,
        if hides { "true" } else { "false" },
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitShortcutPlatform {
    MacOs,
    Windows,
    Linux,
}

impl QuitShortcutPlatform {
    pub(crate) fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuitShortcutRoute {
    /// The guarded `request_application_quit`, same as tray Quit.
    Quit,
    /// Hide to the tray / menu bar and keep running.
    HideToTray,
}

/// Where `QuitApplication` goes: Cmd+Q / the app menu's Quit / agent
/// `app.quit` on macOS, Ctrl+Q and Alt+F4 on Linux, Alt+F4 on Windows.
///
/// macOS quits unless the user turned on "Cmd+Q hides to menu bar". Linux
/// always quits (often no tray host). Windows keeps Alt+F4 equal to the
/// close button, which hides to the tray; the setting does not apply there.
pub(crate) fn quit_shortcut_route(
    platform: QuitShortcutPlatform,
    hides_setting: bool,
) -> QuitShortcutRoute {
    match platform {
        QuitShortcutPlatform::MacOs if hides_setting => QuitShortcutRoute::HideToTray,
        QuitShortcutPlatform::MacOs | QuitShortcutPlatform::Linux => QuitShortcutRoute::Quit,
        QuitShortcutPlatform::Windows => QuitShortcutRoute::HideToTray,
    }
}

#[cfg(test)]
mod quit_shortcut_tests {
    use super::*;

    #[test]
    fn cmd_q_really_quits_on_a_fresh_database() {
        let db = bir_core::db::Database::open_ephemeral().unwrap();
        assert!(!quit_shortcut_hides(&db));
        assert_eq!(
            quit_shortcut_route(QuitShortcutPlatform::MacOs, quit_shortcut_hides(&db)),
            QuitShortcutRoute::Quit
        );
    }

    #[test]
    fn turning_the_setting_on_makes_cmd_q_hide_and_off_restores_quit() {
        let db = bir_core::db::Database::open_ephemeral().unwrap();
        set_quit_shortcut_hides(&db, true);
        assert_eq!(
            quit_shortcut_route(QuitShortcutPlatform::MacOs, quit_shortcut_hides(&db)),
            QuitShortcutRoute::HideToTray
        );
        set_quit_shortcut_hides(&db, false);
        assert_eq!(
            quit_shortcut_route(QuitShortcutPlatform::MacOs, quit_shortcut_hides(&db)),
            QuitShortcutRoute::Quit
        );
    }

    #[test]
    fn the_setting_only_changes_macos() {
        for hides in [false, true] {
            assert_eq!(
                quit_shortcut_route(QuitShortcutPlatform::Linux, hides),
                QuitShortcutRoute::Quit
            );
            assert_eq!(
                quit_shortcut_route(QuitShortcutPlatform::Windows, hides),
                QuitShortcutRoute::HideToTray
            );
        }
    }
}
