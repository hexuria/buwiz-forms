//! The application menu as data (port of `nsh/shell/desktop/menu_model.ts`): which menus and items
//! exist, which are enabled, and their accelerators, for each state the app can be in. Pure: no UI
//! runtime, so a plain test reads every state.
//!
//! The old menu came in two shapes, and so does this one:
//!   * the filing menus (the app is signed in and on its main window): everything below;
//!   * the locked menus (first-run setup, the sign-in screens, the account gate): only About and Quit,
//!     the editing items (so a PIN box can paste), and on Windows and Linux a Help menu with About.
//!     Filing commands are absent, as before.
//! New here: with no taxpayer the items that need one are present but disabled (New Return…,
//! Toggle Tax Profile).
//!
//! Dropped on purpose: macOS's "Services" submenu and Edit's "Delete"; "Enter Full Screen" on macOS
//! is AppKit's own item. A shortcut the old app bound a second way is bound once, as "mod".
//!
//! Quit follows the platform docs: macOS X and Cmd+Q hide to the tray; Windows X and Alt+F4 hide,
//! no Ctrl+Q; Linux X and Ctrl+Q quit.

/// The platforms the menu has a shape for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Macos,
    Windows,
    Linux,
}

impl Os {
    pub fn word(self) -> &'static str {
        match self {
            Os::Macos => "macos",
            Os::Windows => "windows",
            Os::Linux => "linux",
        }
    }
    pub fn parse(word: &str) -> Option<Os> {
        Some(match word {
            "macos" => Os::Macos,
            "windows" => Os::Windows,
            "linux" => Os::Linux,
            _ => return None,
        })
    }
}

/// What a menu item does: the frame carries each one out (actions.ts's `perform`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    About,
    Settings,
    NewReturn,
    NewTaxProfile,
    ToggleSidebar,
    ToggleProfile,
    ToggleNotifications,
    ToggleTheme,
    Palette,
    SignOut,
    Hide,
    CloseWindow,
    Quit,
    BringAllToFront,
    CheckUpdates,
    Support,
    Website,
    Show,
    QuitNow,
}

impl MenuAction {
    pub fn word(self) -> &'static str {
        match self {
            MenuAction::About => "about",
            MenuAction::Settings => "settings",
            MenuAction::NewReturn => "new-return",
            MenuAction::NewTaxProfile => "new-tax-profile",
            MenuAction::ToggleSidebar => "toggle-sidebar",
            MenuAction::ToggleProfile => "toggle-profile",
            MenuAction::ToggleNotifications => "toggle-notifications",
            MenuAction::ToggleTheme => "toggle-theme",
            MenuAction::Palette => "palette",
            MenuAction::SignOut => "sign-out",
            MenuAction::Hide => "hide",
            MenuAction::CloseWindow => "close-window",
            MenuAction::Quit => "quit",
            MenuAction::BringAllToFront => "bring-all-to-front",
            MenuAction::CheckUpdates => "check-updates",
            MenuAction::Support => "support",
            MenuAction::Website => "website",
            MenuAction::Show => "show",
            MenuAction::QuitNow => "quit-now",
        }
    }
    pub fn parse(word: &str) -> Option<MenuAction> {
        Some(match word {
            "about" => MenuAction::About,
            "settings" => MenuAction::Settings,
            "new-return" => MenuAction::NewReturn,
            "new-tax-profile" => MenuAction::NewTaxProfile,
            "toggle-sidebar" => MenuAction::ToggleSidebar,
            "toggle-profile" => MenuAction::ToggleProfile,
            "toggle-notifications" => MenuAction::ToggleNotifications,
            "toggle-theme" => MenuAction::ToggleTheme,
            "palette" => MenuAction::Palette,
            "sign-out" => MenuAction::SignOut,
            "hide" => MenuAction::Hide,
            "close-window" => MenuAction::CloseWindow,
            "quit" => MenuAction::Quit,
            "bring-all-to-front" => MenuAction::BringAllToFront,
            "check-updates" => MenuAction::CheckUpdates,
            "support" => MenuAction::Support,
            "website" => MenuAction::Website,
            "show" => MenuAction::Show,
            "quit-now" => MenuAction::QuitNow,
            _ => return None,
        })
    }
}

/// What decides the menu.
#[derive(Debug, Clone, Copy)]
pub struct MenuState {
    pub os: Os,
    /// The filing chrome is up: not first-run setup, not the sign-in screens.
    pub unlocked: bool,
    /// At least one taxpayer exists in the database the session may see.
    pub has_taxpayer: bool,
    /// The form page is the view on screen. It changes no item.
    pub form_open: bool,
}

/// The app's name in the menus.
pub const APP_NAME: &str = "eBIRForms";
/// Help > Contact Support.
pub const SUPPORT_URL: &str = "mailto:support@goldcoders.dev?subject=eBIRForms%20Support%20Request";
/// Help > Goldcoders Website.
pub const WEBSITE_URL: &str = "https://goldcoders.dev";
/// The About text's last line.
pub const COPYRIGHT: &str = "© 2026 Goldcoders Corp";

/// What each platform does with the window's close button and the quit key.
pub struct ClosePolicy {
    pub close_hides: bool,
    pub quit_key_hides: bool,
    pub quit_shortcut: Option<&'static str>,
}

pub fn close_policy(os: Os) -> ClosePolicy {
    match os {
        Os::Macos => ClosePolicy {
            close_hides: true,
            quit_key_hides: true,
            quit_shortcut: Some("mod+q"),
        },
        Os::Windows => ClosePolicy {
            close_hides: true,
            quit_key_hides: false,
            quit_shortcut: None,
        },
        Os::Linux => ClosePolicy {
            close_hides: false,
            quit_key_hides: false,
            quit_shortcut: Some("mod+q"),
        },
    }
}

/// One menu entry: a separator, a leaf item (label or platform role, optional action), or a submenu.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuEntry {
    Separator,
    Item {
        label: Option<String>,
        role: Option<String>,
        shortcut: Option<String>,
        disabled: bool,
        /// What it does; `None` for a platform role (undo, minimize, …) the OS carries out.
        action: Option<MenuAction>,
    },
    Submenu {
        label: String,
        entries: Vec<MenuEntry>,
    },
}

fn item(label: &str, action: MenuAction, shortcut: Option<&str>, disabled: bool) -> MenuEntry {
    MenuEntry::Item {
        label: Some(label.to_owned()),
        role: None,
        shortcut: shortcut.map(str::to_owned),
        disabled,
        action: Some(action),
    }
}

fn role(name: &str) -> MenuEntry {
    MenuEntry::Item {
        label: None,
        role: Some(name.to_owned()),
        shortcut: None,
        disabled: false,
        action: None,
    }
}

const SEP: MenuEntry = MenuEntry::Separator;

/// Drops the entries that are not there, and separators that would lead, trail or double.
fn tidy(entries: Vec<Option<MenuEntry>>) -> Vec<MenuEntry> {
    let mut out: Vec<MenuEntry> = Vec::new();
    for e in entries.into_iter().flatten() {
        let separator = matches!(e, MenuEntry::Separator);
        if separator && (out.is_empty() || matches!(out.last(), Some(MenuEntry::Separator))) {
            continue;
        }
        out.push(e);
    }
    while matches!(out.last(), Some(MenuEntry::Separator)) {
        out.pop();
    }
    out
}

fn menu(label: &str, entries: Vec<Option<MenuEntry>>) -> MenuEntry {
    MenuEntry::Submenu {
        label: label.to_owned(),
        entries: tidy(entries),
    }
}

fn quit_item(state: &MenuState) -> MenuEntry {
    item(
        &format!("Quit {APP_NAME}"),
        MenuAction::Quit,
        close_policy(state.os).quit_shortcut,
        false,
    )
}

fn edit_menu(settings_at_end: Option<MenuEntry>) -> MenuEntry {
    menu(
        "Edit",
        vec![
            Some(role("undo")),
            Some(role("redo")),
            Some(SEP),
            Some(role("cut")),
            Some(role("copy")),
            Some(role("paste")),
            Some(SEP),
            Some(role("selectAll")),
            if settings_at_end.is_some() {
                Some(SEP)
            } else {
                None
            },
            settings_at_end,
        ],
    )
}

fn app_menu(state: &MenuState) -> MenuEntry {
    let unlocked = state.unlocked;
    menu(
        APP_NAME,
        vec![
            Some(item(
                &format!("About {APP_NAME}"),
                MenuAction::About,
                None,
                false,
            )),
            Some(SEP),
            unlocked.then(|| item("Settings…", MenuAction::Settings, Some("mod+,"), false)),
            unlocked.then_some(SEP),
            unlocked.then(|| {
                item(
                    &format!("Hide {APP_NAME}"),
                    MenuAction::Hide,
                    Some("mod+h"),
                    false,
                )
            }),
            unlocked.then(|| role("hideOthers")),
            unlocked.then(|| role("showAll")),
            unlocked.then_some(SEP),
            unlocked.then(|| item("Sign Out", MenuAction::SignOut, None, false)),
            unlocked.then_some(SEP),
            Some(quit_item(state)),
        ],
    )
}

fn file_menu(state: &MenuState) -> MenuEntry {
    let os = state.os;
    if !state.unlocked {
        return menu("File", vec![Some(quit_item(state))]);
    }
    // Cmd/Ctrl+N opens a new tax profile (the user's pick); New Return keeps its place in the
    // menu without an accelerator of its own.
    let new_profile = item(
        "New Tax Profile…",
        MenuAction::NewTaxProfile,
        Some("mod+n"),
        false,
    );
    let new_return = item(
        "New Return…",
        MenuAction::NewReturn,
        None,
        !state.has_taxpayer,
    );
    if os == Os::Macos {
        return menu(
            "File",
            vec![
                Some(new_profile),
                Some(new_return),
                Some(SEP),
                Some(item(
                    "Close Window",
                    MenuAction::CloseWindow,
                    Some("mod+w"),
                    false,
                )),
            ],
        );
    }
    if os == Os::Windows {
        return menu(
            "File",
            vec![
                Some(new_profile),
                Some(new_return),
                Some(SEP),
                Some(item(
                    "Settings…",
                    MenuAction::Settings,
                    Some("mod+,"),
                    false,
                )),
                Some(item("Sign Out", MenuAction::SignOut, None, false)),
                Some(SEP),
                Some(item(
                    &format!("Hide {APP_NAME}"),
                    MenuAction::Hide,
                    Some("mod+h"),
                    false,
                )),
                Some(SEP),
                Some(quit_item(state)),
            ],
        );
    }
    menu(
        "File",
        vec![
            Some(new_profile),
            Some(new_return),
            Some(SEP),
            Some(item(
                "Close Window",
                MenuAction::CloseWindow,
                Some("mod+w"),
                false,
            )),
            Some(item(
                &format!("Hide {APP_NAME}"),
                MenuAction::Hide,
                Some("mod+h"),
                false,
            )),
            Some(SEP),
            Some(item("Sign Out", MenuAction::SignOut, None, false)),
            Some(quit_item(state)),
        ],
    )
}

fn view_menu(state: &MenuState) -> MenuEntry {
    menu(
        "View",
        vec![
            Some(item(
                "Toggle Sidebar",
                MenuAction::ToggleSidebar,
                Some("mod+b"),
                false,
            )),
            Some(item(
                "Toggle Tax Profile",
                MenuAction::ToggleProfile,
                Some("mod+shift+b"),
                !state.has_taxpayer,
            )),
            Some(item(
                "Toggle Notifications",
                MenuAction::ToggleNotifications,
                Some("mod+shift+n"),
                false,
            )),
            Some(item(
                "Toggle Theme",
                MenuAction::ToggleTheme,
                Some("mod+t"),
                false,
            )),
            Some(SEP),
            Some(item(
                "Command Palette…",
                MenuAction::Palette,
                Some("mod+k"),
                false,
            )),
            // AppKit puts its own Enter Full Screen item in the View menu: a second one would show it twice.
            (state.os != Os::Macos).then_some(SEP),
            (state.os != Os::Macos).then(|| MenuEntry::Item {
                label: Some("Enter Full Screen".to_owned()),
                role: Some("toggleFullScreen".to_owned()),
                shortcut: Some("f11".to_owned()),
                disabled: false,
                action: None,
            }),
        ],
    )
}

fn window_menu(state: &MenuState) -> MenuEntry {
    menu(
        "Window",
        vec![
            Some(role("minimize")),
            Some(role("zoom")),
            (state.os == Os::Macos).then_some(SEP),
            (state.os == Os::Macos).then(|| {
                item(
                    "Bring All to Front",
                    MenuAction::BringAllToFront,
                    None,
                    false,
                )
            }),
        ],
    )
}

fn help_menu(state: &MenuState) -> MenuEntry {
    let about = (state.os != Os::Macos)
        .then(|| item(&format!("About {APP_NAME}"), MenuAction::About, None, false));
    if !state.unlocked {
        return menu("Help", vec![about]);
    }
    menu(
        "Help",
        vec![
            about.clone(),
            about.map(|_| SEP),
            Some(item(
                "Check for Updates…",
                MenuAction::CheckUpdates,
                None,
                false,
            )),
            Some(item("Contact Support…", MenuAction::Support, None, false)),
            Some(item("Goldcoders Website", MenuAction::Website, None, false)),
        ],
    )
}

/// The menus for a state, the way the old app_menu.rs listed them.
pub fn build_menu(state: &MenuState) -> Vec<MenuEntry> {
    let mut menus: Vec<Option<MenuEntry>> = Vec::new();
    if state.os == Os::Macos {
        menus.push(Some(app_menu(state)));
    }
    if state.unlocked || state.os != Os::Macos {
        menus.push(Some(file_menu(state)));
    }
    menus.push(Some(edit_menu(
        (state.unlocked && state.os == Os::Linux)
            .then(|| item("Settings…", MenuAction::Settings, Some("mod+,"), false)),
    )));
    if state.unlocked {
        menus.push(Some(view_menu(state)));
        menus.push(Some(window_menu(state)));
        menus.push(Some(help_menu(state)));
    } else if state.os != Os::Macos {
        menus.push(Some(help_menu(state)));
    }
    tidy(menus)
}

/// The in-window menu bar's menus: the application menu first (every OS — CUI has no system
/// menu bar, so the app's own menu leads the strip), then the state menus. On macOS `build_menu`
/// already carries the app menu, so it is not repeated.
pub fn build_menubar(state: &MenuState) -> Vec<MenuEntry> {
    let mut menus = vec![app_menu(state)];
    for entry in build_menu(state) {
        let is_app = matches!(&entry, MenuEntry::Submenu { label, .. } if label == APP_NAME);
        if !is_app {
            menus.push(entry);
        }
    }
    tidy(menus.into_iter().map(Some).collect())
}

/// The tray icon's menu (old tray.rs): Show, Hide, and Quit — the same in every state.
pub fn build_tray_menu() -> Vec<MenuEntry> {
    vec![
        item(&format!("Show {APP_NAME}"), MenuAction::Show, None, false),
        item(&format!("Hide {APP_NAME}"), MenuAction::Hide, None, false),
        SEP,
        item("Quit", MenuAction::QuitNow, None, false),
    ]
}

/// One line of a flattened menu.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuLine {
    /// "File > New Return…". A role's item with no label is named by its role ("Edit > role:undo").
    pub path: String,
    pub shortcut: Option<String>,
    pub disabled: bool,
    /// What it does; `None` for a role (the platform carries it out).
    pub action: Option<MenuAction>,
}

/// Every item with its path, in menu order, separators left out.
pub fn flatten(items: &[MenuEntry], prefix: &str) -> Vec<MenuLine> {
    let mut lines = Vec::new();
    for entry in items {
        match entry {
            MenuEntry::Separator => continue,
            MenuEntry::Submenu { label, entries } => {
                let path = if prefix.is_empty() {
                    label.clone()
                } else {
                    format!("{prefix} > {label}")
                };
                lines.extend(flatten(entries, &path));
            }
            MenuEntry::Item {
                label,
                role,
                shortcut,
                disabled,
                action,
            } => {
                let name = label
                    .clone()
                    .or_else(|| role.as_ref().map(|r| format!("role:{r}")))
                    .unwrap_or_else(|| "?".to_owned());
                let path = if prefix.is_empty() {
                    name
                } else {
                    format!("{prefix} > {name}")
                };
                lines.push(MenuLine {
                    path,
                    shortcut: shortcut.clone(),
                    disabled: *disabled,
                    action: *action,
                });
            }
        }
    }
    lines
}

/// The text of a flattened menu, one item per line: "File > New Return…  mod+n  (disabled)".
pub fn print_menu(items: &[MenuEntry]) -> Vec<String> {
    flatten(items, "")
        .iter()
        .map(|l| {
            let mut line = l.path.clone();
            if let Some(shortcut) = &l.shortcut {
                line.push_str("  ");
                line.push_str(shortcut);
            }
            if l.disabled {
                line.push_str("  (disabled)");
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(os: Os) -> MenuState {
        MenuState {
            os,
            unlocked: true,
            has_taxpayer: true,
            form_open: false,
        }
    }

    /// The top-level menu titles.
    fn titles(s: &MenuState) -> Vec<String> {
        build_menu(s)
            .iter()
            .map(|m| match m {
                MenuEntry::Submenu { label, .. } => label.clone(),
                _ => String::new(),
            })
            .collect()
    }

    /// The submenu entries of one top-level menu.
    fn submenu_of<'a>(menus: &'a [MenuEntry], menu_name: &str) -> Option<&'a [MenuEntry]> {
        menus.iter().find_map(|m| match m {
            MenuEntry::Submenu { label, entries } if label == menu_name => Some(entries.as_slice()),
            _ => None,
        })
    }

    /// "File > New Return…  mod+n" lines of one top-level menu, minus the "Menu > " prefix.
    fn lines_of(s: &MenuState, menu_name: &str) -> Vec<String> {
        let menus = build_menu(s);
        let Some(entries) = submenu_of(&menus, menu_name) else {
            return vec![];
        };
        let wrapped = vec![MenuEntry::Submenu {
            label: menu_name.to_owned(),
            entries: entries.to_vec(),
        }];
        print_menu(&wrapped)
            .into_iter()
            .map(|l| l[menu_name.len() + 3..].to_owned())
            .collect()
    }

    /// The items of one top-level menu, in order, with "-" for a separator and "role:x" for roles.
    fn shape(s: &MenuState, menu_name: &str) -> Vec<String> {
        let menus = build_menu(s);
        let Some(entries) = submenu_of(&menus, menu_name) else {
            return vec![];
        };
        entries
            .iter()
            .map(|e| match e {
                MenuEntry::Separator => "-".to_owned(),
                MenuEntry::Item { label, role, .. } => label
                    .clone()
                    .unwrap_or_else(|| format!("role:{}", role.as_deref().unwrap_or("?"))),
                MenuEntry::Submenu { .. } => "?".to_owned(),
            })
            .collect()
    }

    #[test]
    fn macos_signed_in_matches_the_old_menu_bar() {
        let s = state(Os::Macos);
        assert_eq!(
            titles(&s),
            ["eBIRForms", "File", "Edit", "View", "Window", "Help"]
        );
        assert_eq!(
            shape(&s, "eBIRForms"),
            [
                "About eBIRForms",
                "-",
                "Settings…",
                "-",
                "Hide eBIRForms",
                "role:hideOthers",
                "role:showAll",
                "-",
                "Sign Out",
                "-",
                "Quit eBIRForms"
            ]
        );
        assert!(lines_of(&s, "eBIRForms").contains(&"Settings…  mod+,".to_owned()));
        assert!(lines_of(&s, "eBIRForms").contains(&"Hide eBIRForms  mod+h".to_owned()));
        assert!(lines_of(&s, "eBIRForms").contains(&"Quit eBIRForms  mod+q".to_owned()));
        assert_eq!(
            shape(&s, "File"),
            ["New Tax Profile…", "New Return…", "-", "Close Window"]
        );
        assert_eq!(
            lines_of(&s, "File"),
            [
                "New Tax Profile…  mod+n",
                "New Return…",
                "Close Window  mod+w"
            ]
        );
        assert_eq!(
            shape(&s, "Edit"),
            [
                "role:undo",
                "role:redo",
                "-",
                "role:cut",
                "role:copy",
                "role:paste",
                "-",
                "role:selectAll"
            ]
        );
        assert_eq!(
            shape(&s, "View"),
            [
                "Toggle Sidebar",
                "Toggle Tax Profile",
                "Toggle Notifications",
                "Toggle Theme",
                "-",
                "Command Palette…"
            ]
        );
        assert_eq!(
            lines_of(&s, "View"),
            [
                "Toggle Sidebar  mod+b",
                "Toggle Tax Profile  mod+shift+b",
                "Toggle Notifications  mod+shift+n",
                "Toggle Theme  mod+t",
                "Command Palette…  mod+k",
            ]
        );
        assert_eq!(
            shape(&s, "Window"),
            ["role:minimize", "role:zoom", "-", "Bring All to Front"]
        );
        assert_eq!(
            shape(&s, "Help"),
            [
                "Check for Updates…",
                "Contact Support…",
                "Goldcoders Website"
            ]
        );
        // AppKit adds its own Enter Full Screen to View: ours would show it twice.
        assert!(
            !flatten(&build_menu(&s), "")
                .iter()
                .any(|l| l.path.contains("Full Screen"))
        );
    }

    #[test]
    fn windows_and_linux_signed_in() {
        let win = state(Os::Windows);
        assert_eq!(titles(&win), ["File", "Edit", "View", "Window", "Help"]);
        assert_eq!(
            shape(&win, "File"),
            [
                "New Tax Profile…",
                "New Return…",
                "-",
                "Settings…",
                "Sign Out",
                "-",
                "Hide eBIRForms",
                "-",
                "Quit eBIRForms"
            ]
        );
        assert!(lines_of(&win, "File").contains(&"Settings…  mod+,".to_owned()));
        assert!(lines_of(&win, "File").contains(&"Quit eBIRForms".to_owned())); // no Ctrl+Q on Windows
        assert!(!shape(&win, "Edit").contains(&"Settings…".to_owned()));
        assert_eq!(
            shape(&win, "Help"),
            [
                "About eBIRForms",
                "-",
                "Check for Updates…",
                "Contact Support…",
                "Goldcoders Website"
            ]
        );
        assert!(lines_of(&win, "View").contains(&"Enter Full Screen  f11".to_owned()));
        assert_eq!(shape(&win, "Window"), ["role:minimize", "role:zoom"]);

        let linux = state(Os::Linux);
        assert_eq!(titles(&linux), ["File", "Edit", "View", "Window", "Help"]);
        assert_eq!(
            shape(&linux, "File"),
            [
                "New Tax Profile…",
                "New Return…",
                "-",
                "Close Window",
                "Hide eBIRForms",
                "-",
                "Sign Out",
                "Quit eBIRForms"
            ]
        );
        assert!(lines_of(&linux, "File").contains(&"Quit eBIRForms  mod+q".to_owned()));
        let edit = shape(&linux, "Edit");
        assert_eq!(&edit[edit.len() - 2..], ["-", "Settings…"]);
        assert!(lines_of(&linux, "Edit").contains(&"Settings…  mod+,".to_owned()));
    }

    #[test]
    fn no_taxpayer_disables_what_needs_one() {
        for os in [Os::Macos, Os::Windows, Os::Linux] {
            let none = MenuState {
                has_taxpayer: false,
                ..state(os)
            };
            let some = state(os);
            let paths = |s: &MenuState| {
                flatten(&build_menu(s), "")
                    .iter()
                    .map(|l| l.path.clone())
                    .collect::<Vec<_>>()
            };
            assert_eq!(paths(&none), paths(&some));
            let disabled: Vec<String> = flatten(&build_menu(&none), "")
                .into_iter()
                .filter(|l| l.disabled)
                .map(|l| l.path)
                .collect();
            assert_eq!(
                disabled,
                ["File > New Return…", "View > Toggle Tax Profile"]
            );
            assert!(flatten(&build_menu(&some), "").iter().all(|l| !l.disabled));
        }
    }

    #[test]
    fn a_form_open_changes_nothing() {
        for os in [Os::Macos, Os::Windows, Os::Linux] {
            for has_taxpayer in [true, false] {
                let open = MenuState {
                    form_open: true,
                    has_taxpayer,
                    ..state(os)
                };
                let shut = MenuState {
                    form_open: false,
                    has_taxpayer,
                    ..state(os)
                };
                assert_eq!(
                    print_menu(&build_menu(&open)),
                    print_menu(&build_menu(&shut))
                );
            }
        }
    }

    #[test]
    fn locked_shows_only_about_quit_and_editing() {
        const BANNED: [&str; 12] = [
            "Settings…",
            "New Tax Profile…",
            "New Return…",
            "Sign Out",
            "Command Palette…",
            "Check for Updates…",
            "Toggle Sidebar",
            "Toggle Tax Profile",
            "Toggle Notifications",
            "Toggle Theme",
            "Hide eBIRForms",
            "Close Window",
        ];
        for os in [Os::Macos, Os::Windows, Os::Linux] {
            let s = MenuState {
                unlocked: false,
                has_taxpayer: false,
                ..state(os)
            };
            let lines = flatten(&build_menu(&s), "");
            for banned in BANNED {
                assert!(
                    !lines
                        .iter()
                        .any(|l| l.path.ends_with(&format!("> {banned}"))),
                    "{os:?} {banned}"
                );
            }
            assert!(lines.iter().any(|l| l.path.contains("About")));
            assert!(lines.iter().any(|l| l.path.contains("Quit")));
            assert!(lines.iter().all(|l| !l.disabled));
        }
        let mac = MenuState {
            unlocked: false,
            ..state(Os::Macos)
        };
        assert_eq!(titles(&mac), ["eBIRForms", "Edit"]);
        assert_eq!(
            shape(&mac, "eBIRForms"),
            ["About eBIRForms", "-", "Quit eBIRForms"]
        );
        assert!(shape(&mac, "Edit").contains(&"role:paste".to_owned()));
        assert_eq!(
            titles(&MenuState {
                unlocked: false,
                ..state(Os::Windows)
            }),
            ["File", "Edit", "Help"]
        );
        assert_eq!(
            shape(
                &MenuState {
                    unlocked: false,
                    ..state(Os::Windows)
                },
                "File"
            ),
            ["Quit eBIRForms"]
        );
        assert_eq!(
            shape(
                &MenuState {
                    unlocked: false,
                    ..state(Os::Linux)
                },
                "Help"
            ),
            ["About eBIRForms"]
        );
        assert!(
            !shape(
                &MenuState {
                    unlocked: false,
                    ..state(Os::Linux)
                },
                "Edit"
            )
            .contains(&"Settings…".to_owned())
        );
    }

    #[test]
    fn every_state_has_a_tidy_menu() {
        for os in [Os::Macos, Os::Windows, Os::Linux] {
            for unlocked in [true, false] {
                for has_taxpayer in [true, false] {
                    for form_open in [true, false] {
                        let s = MenuState {
                            os,
                            unlocked,
                            has_taxpayer,
                            form_open,
                        };
                        for menu in build_menu(&s) {
                            let MenuEntry::Submenu { entries, .. } = menu else {
                                panic!("a top-level item has no submenu")
                            };
                            let kinds: Vec<&str> = entries
                                .iter()
                                .map(|e| match e {
                                    MenuEntry::Separator => "-",
                                    _ => "item",
                                })
                                .collect();
                            assert_eq!(kinds[0], "item");
                            assert_eq!(kinds[kinds.len() - 1], "item");
                            assert!(!kinds.join("").contains("--"));
                        }
                        let lines = flatten(&build_menu(&s), "");
                        let shortcuts: Vec<&str> =
                            lines.iter().filter_map(|l| l.shortcut.as_deref()).collect();
                        let unique: std::collections::HashSet<_> = shortcuts.iter().collect();
                        assert_eq!(
                            unique.len(),
                            shortcuts.len(),
                            "{os:?} {unlocked} {has_taxpayer} {form_open}"
                        );
                        assert!(
                            flatten(&build_menu(&s), "")
                                .iter()
                                .any(|l| l.path.ends_with("Quit eBIRForms"))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn each_item_carries_its_action_and_roles_carry_none() {
        let lines = flatten(&build_menu(&state(Os::Macos)), "");
        let want: &[(&str, MenuAction)] = &[
            ("eBIRForms > About eBIRForms", MenuAction::About),
            ("eBIRForms > Settings…", MenuAction::Settings),
            ("eBIRForms > Hide eBIRForms", MenuAction::Hide),
            ("eBIRForms > Sign Out", MenuAction::SignOut),
            ("eBIRForms > Quit eBIRForms", MenuAction::Quit),
            ("File > New Tax Profile…", MenuAction::NewTaxProfile),
            ("File > New Return…", MenuAction::NewReturn),
            ("File > Close Window", MenuAction::CloseWindow),
            ("View > Toggle Sidebar", MenuAction::ToggleSidebar),
            ("View > Toggle Tax Profile", MenuAction::ToggleProfile),
            (
                "View > Toggle Notifications",
                MenuAction::ToggleNotifications,
            ),
            ("View > Toggle Theme", MenuAction::ToggleTheme),
            ("View > Command Palette…", MenuAction::Palette),
            ("Window > Bring All to Front", MenuAction::BringAllToFront),
            ("Help > Check for Updates…", MenuAction::CheckUpdates),
            ("Help > Contact Support…", MenuAction::Support),
            ("Help > Goldcoders Website", MenuAction::Website),
        ];
        for (path, action) in want {
            let line = lines
                .iter()
                .find(|l| &l.path == path)
                .unwrap_or_else(|| panic!("no item {path}"));
            assert_eq!(line.action, Some(*action), "{path}");
        }
        let roles: Vec<&str> = lines
            .iter()
            .filter(|l| l.action.is_none())
            .map(|l| l.path.as_str())
            .collect();
        assert_eq!(
            roles,
            [
                "eBIRForms > role:hideOthers",
                "eBIRForms > role:showAll",
                "Edit > role:undo",
                "Edit > role:redo",
                "Edit > role:cut",
                "Edit > role:copy",
                "Edit > role:paste",
                "Edit > role:selectAll",
                "Window > role:minimize",
                "Window > role:zoom",
            ]
        );
    }

    #[test]
    fn the_tray_menu_is_show_hide_quit_everywhere() {
        let lines = flatten(&build_tray_menu(), "");
        assert_eq!(
            lines.iter().map(|l| l.path.as_str()).collect::<Vec<_>>(),
            ["Show eBIRForms", "Hide eBIRForms", "Quit"]
        );
        assert_eq!(
            lines.iter().map(|l| l.action).collect::<Vec<_>>(),
            [
                Some(MenuAction::Show),
                Some(MenuAction::Hide),
                Some(MenuAction::QuitNow)
            ]
        );
    }

    #[test]
    fn the_platform_docs_decide_close_and_quit() {
        let mac = close_policy(Os::Macos);
        assert!(mac.close_hides && mac.quit_key_hides && mac.quit_shortcut == Some("mod+q"));
        let win = close_policy(Os::Windows);
        assert!(win.close_hides && !win.quit_key_hides && win.quit_shortcut.is_none());
        let linux = close_policy(Os::Linux);
        assert!(
            !linux.close_hides && !linux.quit_key_hides && linux.quit_shortcut == Some("mod+q")
        );
    }
}
