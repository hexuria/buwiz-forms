//! Application menu for bir-shell.
//!
//! macOS gets the native menu bar (`cx.set_menus`): the app menu App Store
//! review expects, plus File, Edit, View, Window and Help. Linux and Windows
//! get the same actions in an in-window [`gpui_component::menu::AppMenuBar`],
//! which is what GPUI draws there — `set_menus` only stores the model.
//!
//! Cmd+Q / Ctrl+Q quits: the submission worker stops and the database lock is
//! released. Cmd+H / Ctrl+H hides to the menu-bar status item. That used to
//! be what Cmd+Q did in the painted app; it is not `cx.hide()` (which keeps
//! the Dock icon).

// gpui-shell re-exports whichever engine the `gpui-fast` feature selected.
// A direct `gpui` import stays on crates.io gpui-pre and is a different type
// once that feature is on.
#[cfg(not(target_os = "macos"))]
use gpui_component::menu::AppMenuBar;
use gpui_shell::ShellRoot;
use gpui_shell::action::ShellAction;
use gpui_shell::gpui::{
    self, Action, App, Entity, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, Window,
};
#[cfg(not(target_os = "macos"))]
use gpui_shell::gpui::{div, prelude::*, px};

gpui::actions!(
    bir_shell,
    [
        AboutApplication,
        OpenSettings,
        HideToTray,
        HideOthers,
        ShowAllApplications,
        QuitApplication,
        NewReturn,
        CloseWindow,
        ToggleSidebar,
        ToggleTheme,
        OpenCommandPalette,
        ToggleFullScreen,
        MinimizeWindow,
        ZoomWindow,
        BringAllToFront,
        OpenSupportEmail,
        OpenCompanyWebsite,
    ]
);

pub const SUPPORT_URL: &str = "mailto:support@goldcoders.dev?subject=eBIRForms%20Support%20Request";
pub const WEBSITE_URL: &str = "https://goldcoders.dev";
pub const COPYRIGHT: &str = "© 2026 Goldcoders Corp";

/// About text. The dev binary's Info.plist still has version placeholders, so
/// the dialog carries the Cargo version itself.
pub fn about_body() -> String {
    format!(
        "eBIRForms\nVersion {}\n{COPYRIGHT}",
        env!("CARGO_PKG_VERSION")
    )
}

/// Which menu shape to build. Tests ask for every platform; the running app
/// uses [`current_platform`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MenuPlatform {
    Macos,
    Linux,
    Windows,
}

pub(crate) fn current_platform() -> MenuPlatform {
    if cfg!(target_os = "macos") {
        MenuPlatform::Macos
    } else if cfg!(target_os = "windows") {
        MenuPlatform::Windows
    } else {
        MenuPlatform::Linux
    }
}

/// Install key bindings, action handlers and the menu model. Call this before
/// the main window opens, so the macOS menu bar exists on first display and
/// the in-window bar has something to read.
pub fn install(cx: &mut App) {
    if !cx.has_global::<gpui_component::GlobalState>() {
        gpui_component::GlobalState::init(cx);
    }
    bind_global_keys(cx);
    register_actions(cx);
    publish_menus(cx);
}

fn publish_menus(cx: &mut App) {
    let menus = app_menus(current_platform());
    gpui_component::GlobalState::global_mut(cx)
        .set_app_menus(menus.into_iter().map(Menu::owned).collect());
    cx.set_menus(app_menus(current_platform()));
}

/// Close hides. The tray poll starts with the window. On macOS the menu is
/// the native bar, so the script root stays the window root. Elsewhere the
/// script view sits under an in-window menu bar.
#[cfg(target_os = "macos")]
pub fn attach_window(
    root: Entity<ShellRoot>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ShellRoot> {
    arm_window(window, cx);
    root
}

/// Close hides. The tray poll starts with the window, and the script view
/// sits under the in-window menu bar.
#[cfg(not(target_os = "macos"))]
pub fn attach_window(
    root: Entity<ShellRoot>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<DesktopMenuFrame> {
    arm_window(window, cx);
    let menu = AppMenuBar::new(cx);
    cx.new(|_| DesktopMenuFrame { menu, root })
}

fn arm_window(window: &mut Window, cx: &mut App) {
    crate::tray::start(cx);
    window.on_window_should_close(cx, |_, cx| {
        hide_or_quit(cx);
        false
    });
}

#[cfg(not(target_os = "macos"))]
struct DesktopMenuFrame {
    menu: Entity<AppMenuBar>,
    root: Entity<ShellRoot>,
}

#[cfg(not(target_os = "macos"))]
impl Render for DesktopMenuFrame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(30.0))
                    .flex_none()
                    .child(self.menu.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .child(self.root.clone()),
            )
    }
}

/// Stop the submission worker, release the database lock, drop the tray, quit.
pub fn quit_application(cx: &mut App) {
    crate::tray::drop_tray();
    crate::bir_module::shutdown_for_quit();
    cx.quit();
}

/// Hide to the tray. Where there is no tray (and hiding would remove the only
/// way back, which on macOS includes the Dock icon), quit instead.
pub fn hide_or_quit(cx: &mut App) {
    if cfg!(target_os = "macos") || crate::tray::tray_available() {
        crate::tray::hide_to_tray();
    } else {
        tracing::warn!("system tray unavailable; quitting instead of hiding");
        quit_application(cx);
    }
}

fn register_actions(cx: &mut App) {
    cx.on_action(|_: &AboutApplication, _| show_about())
        .on_action(|_: &OpenSettings, cx| dispatch_script("open-settings", cx))
        .on_action(|_: &NewReturn, cx| dispatch_script("new-return", cx))
        .on_action(|_: &ToggleSidebar, cx| dispatch_script("toggle-sidebar", cx))
        .on_action(|_: &ToggleTheme, cx| dispatch_script("toggle-theme", cx))
        .on_action(|_: &OpenCommandPalette, cx| dispatch_script("open-palette", cx))
        .on_action(|_: &HideToTray, cx| hide_or_quit(cx))
        .on_action(|_: &CloseWindow, cx| hide_or_quit(cx))
        .on_action(|_: &HideOthers, cx| cx.hide_other_apps())
        .on_action(|_: &ShowAllApplications, cx| cx.unhide_other_apps())
        .on_action(|_: &QuitApplication, cx| quit_application(cx))
        .on_action(|_: &BringAllToFront, cx| bring_all_to_front(cx))
        .on_action(|_: &MinimizeWindow, cx| {
            with_window(cx, |window, _| window.minimize_window());
        })
        .on_action(|_: &ZoomWindow, cx| {
            with_window(cx, |window, _| window.zoom_window());
        })
        .on_action(|_: &ToggleFullScreen, cx| {
            with_window(cx, |window, _| window.toggle_fullscreen());
        })
        .on_action(|_: &OpenSupportEmail, cx| cx.open_url(SUPPORT_URL))
        .on_action(|_: &OpenCompanyWebsite, cx| cx.open_url(WEBSITE_URL))
        .on_action(|action: &ShellAction, cx| {
            // The caption close button dispatches this. Every other script
            // action still belongs to the view that named it.
            if action.id() == "hide-to-tray" {
                hide_or_quit(cx);
            } else {
                cx.propagate();
            }
        });
}

fn dispatch_script(id: &'static str, cx: &mut App) {
    with_window(cx, move |window, cx| {
        window.dispatch_action(Box::new(ShellAction::new(id)), cx);
    });
}

fn with_window(cx: &mut App, f: impl FnOnce(&mut Window, &mut App)) {
    let Some(handle) = cx
        .active_window()
        .or_else(|| cx.windows().into_iter().next())
    else {
        return;
    };
    if let Err(error) = handle.update(cx, |_, window, cx| f(window, cx)) {
        tracing::warn!("menu action had no window: {error}");
    }
}

fn bring_all_to_front(cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        let _ = cx;
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            app.arrangeInFront(None);
        }
    }
    #[cfg(not(target_os = "macos"))]
    crate::tray::show_app(cx);
}

fn show_about() {
    let body = about_body();
    // rfd's macOS dialog hops to the main thread itself. Spawning keeps the
    // GPUI action handler from nesting a modal run loop inside the menu click.
    std::thread::spawn(move || {
        let _ = rfd::MessageDialog::new()
            .set_title("About eBIRForms")
            .set_description(&body)
            .set_level(rfd::MessageLevel::Info)
            .show();
    });
}

fn bind(cx: &mut App, key: &str, action: impl Action + Clone) {
    // The platform modifier is bound first: the menu shortcut label is the
    // first binding `set_menus` sees.
    let (first, second) = if cfg!(target_os = "macos") {
        (format!("cmd-{key}"), format!("ctrl-{key}"))
    } else {
        (format!("ctrl-{key}"), format!("cmd-{key}"))
    };
    cx.bind_keys([
        KeyBinding::new(&first, action.clone(), None),
        KeyBinding::new(&second, action, None),
    ]);
}

fn bind_global_keys(cx: &mut App) {
    bind(cx, "q", QuitApplication);
    bind(cx, "h", HideToTray);
    bind(cx, "w", CloseWindow);
    bind(cx, ",", OpenSettings);
    bind(cx, "n", NewReturn);
    bind(cx, "b", ToggleSidebar);
    bind(cx, "shift-t", ToggleTheme);
    bind(cx, "k", OpenCommandPalette);
    #[cfg(target_os = "macos")]
    {
        cx.bind_keys([
            KeyBinding::new("cmd-alt-h", HideOthers, None),
            KeyBinding::new("cmd-m", MinimizeWindow, None),
            KeyBinding::new("cmd-ctrl-f", ToggleFullScreen, None),
        ]);
    }
    #[cfg(target_os = "windows")]
    cx.bind_keys([KeyBinding::new("win-m", MinimizeWindow, None)]);
    cx.bind_keys([KeyBinding::new("f11", ToggleFullScreen, None)]);
}

pub(crate) fn app_menus(platform: MenuPlatform) -> Vec<Menu> {
    let mut menus = Vec::new();
    if platform == MenuPlatform::Macos {
        menus.push(macos_app_menu());
    }
    menus.push(file_menu(platform));
    menus.push(edit_menu(platform == MenuPlatform::Linux));
    menus.push(view_menu(platform));
    menus.push(window_menu(platform == MenuPlatform::Macos));
    menus.push(help_menu(platform != MenuPlatform::Macos));
    menus
}

fn macos_app_menu() -> Menu {
    Menu::new("eBIRForms").items([
        MenuItem::action("About eBIRForms", AboutApplication),
        MenuItem::separator(),
        MenuItem::action("Settings…", OpenSettings),
        MenuItem::separator(),
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action("Hide eBIRForms", HideToTray),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAllApplications),
        MenuItem::separator(),
        MenuItem::action("Quit eBIRForms", QuitApplication),
    ])
}

fn file_menu(platform: MenuPlatform) -> Menu {
    let mut items = vec![MenuItem::action("New Return…", NewReturn)];
    match platform {
        MenuPlatform::Macos => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Close Window", CloseWindow));
        }
        MenuPlatform::Windows => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Settings…", OpenSettings));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Hide eBIRForms", HideToTray));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Quit eBIRForms", QuitApplication));
        }
        MenuPlatform::Linux => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Hide eBIRForms", HideToTray));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Quit eBIRForms", QuitApplication));
        }
    }
    Menu::new("File").items(items)
}

fn edit_menu(settings_at_end: bool) -> Menu {
    let mut items = vec![
        MenuItem::os_action("Undo", gpui_component::input::Undo, OsAction::Undo),
        MenuItem::os_action("Redo", gpui_component::input::Redo, OsAction::Redo),
        MenuItem::separator(),
        MenuItem::os_action("Cut", gpui_component::input::Cut, OsAction::Cut),
        MenuItem::os_action("Copy", gpui_component::input::Copy, OsAction::Copy),
        MenuItem::os_action("Paste", gpui_component::input::Paste, OsAction::Paste),
        MenuItem::action("Delete", gpui_component::input::Delete),
        MenuItem::separator(),
        MenuItem::os_action(
            "Select All",
            gpui_component::input::SelectAll,
            OsAction::SelectAll,
        ),
    ];
    if settings_at_end {
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Settings…", OpenSettings));
    }
    Menu::new("Edit").items(items)
}

fn view_menu(platform: MenuPlatform) -> Menu {
    let mut items = vec![
        MenuItem::action("Toggle Sidebar", ToggleSidebar),
        MenuItem::action("Toggle Theme", ToggleTheme),
        MenuItem::separator(),
        MenuItem::action("Command Palette…", OpenCommandPalette),
    ];
    // AppKit inserts its own Enter Full Screen item into the View menu.
    // Adding another one shows the command twice.
    if platform != MenuPlatform::Macos {
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Enter Full Screen", ToggleFullScreen));
    }
    Menu::new("View").items(items)
}

fn window_menu(bring_all: bool) -> Menu {
    let mut items = vec![
        MenuItem::action("Minimize", MinimizeWindow),
        MenuItem::action("Zoom", ZoomWindow),
    ];
    if bring_all {
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Bring All to Front", BringAllToFront));
    }
    Menu::new("Window").items(items)
}

fn help_menu(include_about: bool) -> Menu {
    let mut items = Vec::new();
    if include_about {
        items.push(MenuItem::action("About eBIRForms", AboutApplication));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Contact Support…", OpenSupportEmail));
    items.push(MenuItem::action("Goldcoders Website", OpenCompanyWebsite));
    Menu::new("Help").items(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu<'a>(menus: &'a [Menu], name: &str) -> &'a Menu {
        menus
            .iter()
            .find(|menu| menu.name.as_ref() == name)
            .unwrap_or_else(|| panic!("no {name} menu"))
    }

    fn titles(menu: &Menu) -> Vec<String> {
        menu.items
            .iter()
            .map(|item| match item {
                MenuItem::Separator => "-".to_string(),
                MenuItem::Action { name, .. } => name.to_string(),
                MenuItem::SystemMenu(os_menu) => os_menu.name.to_string(),
                MenuItem::Submenu(submenu) => submenu.name.to_string(),
            })
            .collect()
    }

    fn named<'a>(menu: &'a Menu, title: &str) -> &'a MenuItem {
        menu.items
            .iter()
            .find(|item| matches!(item, MenuItem::Action { name, .. } if name.as_ref() == title))
            .unwrap_or_else(|| panic!("no item {title}"))
    }

    fn is_action<A: Action>(item: &MenuItem) -> bool {
        match item {
            MenuItem::Action { action, .. } => action.as_any().is::<A>(),
            _ => false,
        }
    }

    fn os_action(item: &MenuItem) -> Option<OsAction> {
        match item {
            MenuItem::Action { os_action, .. } => *os_action,
            _ => None,
        }
    }

    #[test]
    fn about_text_names_the_app_version_and_copyright() {
        let body = about_body();
        assert!(body.contains("eBIRForms"));
        assert!(body.contains(env!("CARGO_PKG_VERSION")));
        assert!(body.contains(COPYRIGHT));
        assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
    }

    #[test]
    fn macos_menus_match_the_app_store_list() {
        let menus = app_menus(MenuPlatform::Macos);
        let names: Vec<_> = menus.iter().map(|menu| menu.name.to_string()).collect();
        assert_eq!(
            names,
            ["eBIRForms", "File", "Edit", "View", "Window", "Help"]
        );
        assert_eq!(
            titles(menu(&menus, "eBIRForms")),
            [
                "About eBIRForms",
                "-",
                "Settings…",
                "-",
                "Services",
                "-",
                "Hide eBIRForms",
                "Hide Others",
                "Show All",
                "-",
                "Quit eBIRForms",
            ]
        );
        let app = menu(&menus, "eBIRForms");
        assert!(is_action::<QuitApplication>(named(app, "Quit eBIRForms")));
        assert!(is_action::<HideToTray>(named(app, "Hide eBIRForms")));
        assert!(is_action::<OpenSettings>(named(app, "Settings…")));
        assert!(is_action::<AboutApplication>(named(app, "About eBIRForms")));

        let file = menu(&menus, "File");
        assert_eq!(titles(file), ["New Return…", "-", "Close Window"]);
        assert!(is_action::<NewReturn>(named(file, "New Return…")));
        assert!(is_action::<CloseWindow>(named(file, "Close Window")));
        assert!(
            !titles(menu(&menus, "View"))
                .iter()
                .any(|title| title == "Enter Full Screen")
        );
    }

    #[test]
    fn edit_items_use_the_text_actions_and_native_selectors() {
        let menus = app_menus(MenuPlatform::Macos);
        let edit = menu(&menus, "Edit");
        let expected = [
            ("Undo", OsAction::Undo),
            ("Redo", OsAction::Redo),
            ("Cut", OsAction::Cut),
            ("Copy", OsAction::Copy),
            ("Paste", OsAction::Paste),
            ("Select All", OsAction::SelectAll),
        ];
        for (title, selector) in expected {
            let item = named(edit, title);
            assert!(os_action(item) == Some(selector), "{title}");
        }
        assert!(os_action(named(edit, "Delete")).is_none());
        assert!(titles(edit).iter().all(|title| title != "Settings…"));
    }

    #[test]
    fn linux_puts_settings_in_edit_and_quit_in_file() {
        let menus = app_menus(MenuPlatform::Linux);
        let names: Vec<_> = menus.iter().map(|menu| menu.name.to_string()).collect();
        assert_eq!(names, ["File", "Edit", "View", "Window", "Help"]);
        let file = menu(&menus, "File");
        assert_eq!(
            titles(file),
            ["New Return…", "-", "Hide eBIRForms", "-", "Quit eBIRForms",]
        );
        assert!(is_action::<QuitApplication>(named(file, "Quit eBIRForms")));
        assert!(is_action::<HideToTray>(named(file, "Hide eBIRForms")));
        assert!(is_action::<OpenSettings>(named(
            menu(&menus, "Edit"),
            "Settings…"
        )));
        assert!(is_action::<AboutApplication>(named(
            menu(&menus, "Help"),
            "About eBIRForms"
        )));
        assert_eq!(titles(menu(&menus, "Window")), ["Minimize", "Zoom"]);
        assert!(titles(menu(&menus, "View")).contains(&"Enter Full Screen".to_string()));
    }

    #[test]
    fn windows_puts_settings_in_file() {
        let menus = app_menus(MenuPlatform::Windows);
        let file = menu(&menus, "File");
        assert!(titles(file).contains(&"Settings…".to_string()));
        assert!(is_action::<OpenSettings>(named(file, "Settings…")));
        assert!(is_action::<QuitApplication>(named(file, "Quit eBIRForms")));
        assert!(
            titles(menu(&menus, "Edit"))
                .iter()
                .all(|title| title != "Settings…")
        );
        assert!(is_action::<AboutApplication>(named(
            menu(&menus, "Help"),
            "About eBIRForms"
        )));
        assert!(titles(menu(&menus, "View")).contains(&"Enter Full Screen".to_string()));
    }

    #[test]
    fn help_links_are_the_ones_the_painted_app_already_had() {
        assert!(SUPPORT_URL.starts_with("mailto:support@goldcoders.dev"));
        assert_eq!(WEBSITE_URL, "https://goldcoders.dev");
        let menus = app_menus(MenuPlatform::Macos);
        let help = menu(&menus, "Help");
        assert_eq!(titles(help), ["Contact Support…", "Goldcoders Website"]);
        assert!(is_action::<OpenSupportEmail>(named(
            help,
            "Contact Support…"
        )));
        assert!(is_action::<OpenCompanyWebsite>(named(
            help,
            "Goldcoders Website"
        )));
    }
}
