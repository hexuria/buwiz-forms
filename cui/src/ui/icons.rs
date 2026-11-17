//! The app's icons: the Lucide set the NSH app ships (`nsh/assets/icons`, `nsh/ui/icons.ts`),
//! mapped onto CUI's embedded Lucide glyphs. Screens name icons the NSH way ("file-text",
//! "chevron-down"); this module resolves them to the `cui::ICON_*` constant to draw.

/// An NSH icon name (its SVG file name, no extension) as a CUI icon glyph.
/// Unknown names fall back to a plain dot so a typo never blanks a button.
pub fn glyph(name: &str) -> &'static str {
    match name {
        // navigation and chrome
        "house" => cui::ICON_HOUSE,
        "file-text" => cui::ICON_FILE_TEXT,
        "calendar" => cui::ICON_CALENDAR,
        "history" => cui::ICON_HISTORY,
        "settings" => cui::ICON_SETTINGS,
        "user" => cui::ICON_USER,
        "users" => cui::ICON_USERS,
        "git-branch" => cui::ICON_GIT_BRANCH,
        "layout-template" => cui::ICON_LAYOUT_DASHBOARD,
        "monitor" => cui::ICON_MONITOR,
        "list" => cui::ICON_LIST,
        "bell" => cui::ICON_BELL,
        "search" => cui::ICON_SEARCH,
        "plus" => cui::ICON_PLUS,
        "minus" => cui::ICON_MINUS,
        "x" => cui::ICON_X,
        "check" => cui::ICON_CHECK,
        "circle-check" => cui::ICON_CIRCLE_CHECK,
        "chevron-down" => cui::ICON_CHEVRON_DOWN,
        "chevron-up" => cui::ICON_CHEVRON_UP,
        "chevron-left" => cui::ICON_CHEVRON_LEFT,
        "chevron-right" => cui::ICON_CHEVRON_RIGHT,
        "chevrons-up-down" => cui::ICON_CHEVRONS_UP_DOWN,
        "chevrons-down-up" => cui::ICON_CHEVRONS_UP_DOWN,
        "arrow-left" => cui::ICON_ARROW_LEFT,
        "arrow-right" => cui::ICON_ARROW_RIGHT,
        "pencil" => cui::ICON_PENCIL,
        "trash" => cui::ICON_TRASH_2,
        "archive" => cui::ICON_ARCHIVE,
        "copy" => cui::ICON_COPY,
        "save" => cui::ICON_SAVE,
        "printer" => cui::ICON_PRINTER,
        "send" => cui::ICON_SEND,
        "upload" => cui::ICON_UPLOAD,
        "download" => cui::ICON_DOWNLOAD,
        "lock" => cui::ICON_LOCK,
        "unlock" => cui::ICON_UNLOCK,
        "key-round" => cui::ICON_KEY_ROUND,
        "eye" => cui::ICON_EYE,
        "eye-off" => cui::ICON_EYE_OFF,
        "log-out" => cui::ICON_LOG_OUT,
        "log-in" => cui::ICON_LOG_IN,
        "user-plus" => cui::ICON_USER_PLUS,
        "map-pin" => cui::ICON_MAP_PIN,
        "building" => cui::ICON_BUILDING_2,
        "hash" => cui::ICON_HASH,
        "info" => cui::ICON_INFO,
        "triangle-alert" => cui::ICON_TRIANGLE_ALERT,
        "circle-alert" => cui::ICON_CIRCLE_ALERT,
        "loader" => cui::ICON_LOADER_CIRCLE,
        "sun" => cui::ICON_SUN,
        "moon" => cui::ICON_MOON,
        "panel-left" => cui::ICON_PANEL_LEFT,
        "panel-right" => cui::ICON_PANEL_LEFT,
        "square" => cui::ICON_SQUARE,
        "square-check" => cui::ICON_SQUARE_CHECK,
        "square-terminal" => cui::ICON_SQUARE_TERMINAL,
        "hard-drive" => cui::ICON_DATABASE,
        "laptop" => cui::ICON_MONITOR,
        "power" => cui::ICON_LOG_OUT,
        "refresh-cw" => cui::ICON_REFRESH_CW,
        _ => cui::ICON_DOT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_nsh_route_icon_resolves() {
        for name in [
            "house",
            "file-text",
            "calendar",
            "history",
            "settings",
            "user",
            "layout-template",
            "git-branch",
            "monitor",
        ] {
            assert_ne!(glyph(name), cui::ICON_DOT, "{name} unmapped");
        }
    }

    #[test]
    fn unknown_names_fall_back() {
        assert_eq!(glyph("not-an-icon"), cui::ICON_DOT);
    }
}
