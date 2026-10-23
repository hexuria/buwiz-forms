//! Application menu for bir-shell.
//!
//! macOS gets the native menu bar (`cx.set_menus`): the app menu App Store
//! review expects, plus File, Edit, View, Window and Help. Linux and Windows
//! get the same actions in an in-window bar drawn by [`DesktopMenuFrame`].
//! `set_menus` only stores the model on those platforms; the stock
//! `AppMenuBar` is not used, because its titles follow gpui-component's
//! light theme and disappear on the shell's dark window.
//!
//! On Linux that bar is also the client-side title row: the empty space
//! drags the window, a double-click toggles maximize, and minimize, zoom,
//! and close sit at the right end. `linux_csd` owns that policy.
//!
//! Cmd+Q / Ctrl+Q quits: the submission worker stops and the database lock is
//! released. Cmd+H / Ctrl+H hides to the menu-bar status item. That used to
//! be what Cmd+Q did in the painted app; it is not `cx.hide()` (which keeps
//! the Dock icon).

// gpui-shell re-exports whichever engine the `gpui-fast` feature selected.
// A direct `gpui` import stays on crates.io gpui-pre and is a different type
// once that feature is on.
#[cfg(not(target_os = "macos"))]
use gpui_component::menu::PopupMenu;
use gpui_shell::ShellRoot;
use gpui_shell::action::ShellAction;
use gpui_shell::gpui::{
    self, Action, App, Entity, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, Window,
};
#[cfg(not(target_os = "macos"))]
use gpui_shell::gpui::{
    Anchor, AnyWindowHandle, DismissEvent, MouseButton, OwnedMenu, OwnedMenuItem, Subscription,
    anchored, deferred, div, prelude::*, px,
};
#[cfg(not(target_os = "macos"))]
use std::cell::RefCell;

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
        CheckForUpdates,
        SignOut,
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
    // The updater relaunches with this set so a screenshot can show the
    // version. The dialog is the application About, not a second menu bar.
    if std::env::var_os("BIR_SHOW_ABOUT").is_some() {
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(2))
                .await;
            cx.update(show_about);
        })
        .detach();
    }
}

fn publish_menus(cx: &mut App) {
    let menus = app_menus(current_platform(), crate::account_gate::is_unlocked());
    gpui_component::GlobalState::global_mut(cx)
        .set_app_menus(menus.into_iter().map(Menu::owned).collect());
    cx.set_menus(app_menus(
        current_platform(),
        crate::account_gate::is_unlocked(),
    ));
}

/// Rebuild the menu bar after sign-in or sign-out.
pub(crate) fn refresh(cx: &mut App) {
    publish_menus(cx);
}

/// Close hides. The tray poll starts with the window. On macOS the menu is
/// the native bar, so the script root stays the window root. Elsewhere the
/// script view sits under an in-window menu bar.
#[cfg(target_os = "macos")]
pub(crate) fn attach_window(
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
pub(crate) fn attach_window(
    root: Entity<ShellRoot>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<DesktopMenuFrame> {
    arm_window(window, cx);
    let frame = cx.new(|_| DesktopMenuFrame {
        root,
        open: None,
        popup: None,
        dismiss: None,
        about: false,
    });
    #[cfg(target_os = "linux")]
    crate::linux_csd::watch_decorations(&frame, window, cx);
    frame
}

fn arm_window(window: &mut Window, cx: &mut App) {
    remember_main_window(window);
    crate::tray::start(cx);
    window.on_window_should_close(cx, |_, cx| {
        hide_or_quit(cx);
        false
    });
}

#[cfg(not(target_os = "macos"))]
thread_local! {
    static MAIN_WINDOW: RefCell<Option<AnyWindowHandle>> = const { RefCell::new(None) };
}

/// The shell window menu clicks should hit. Recorded when that window opens.
#[cfg(not(target_os = "macos"))]
fn remember_main_window(window: &Window) {
    let handle = window.window_handle();
    MAIN_WINDOW.with(|slot| *slot.borrow_mut() = Some(handle));
}

#[cfg(target_os = "macos")]
fn remember_main_window(window: &Window) {
    let _ = window;
}

/// In-window menu bar over the script root. `pub(crate)` because
/// `open_window` names this type, and a private type there is a hard error
/// off macOS.
///
/// The bar is not [`gpui_component::menu::AppMenuBar`]. That widget's titles
/// are ghost buttons, and those read gpui-component's theme, which stays on
/// the light palette (`secondary_foreground` near black). The shell paints
/// the window from gpui-base's theme, so on a dark theme the titles were
/// black on black and only showed up when the hover chip painted a lighter
/// background. These titles read gpui-base's foreground at render time, the
/// same palette the shell applies for light and dark.
#[cfg(not(target_os = "macos"))]
pub(crate) struct DesktopMenuFrame {
    root: Entity<ShellRoot>,
    open: Option<usize>,
    popup: Option<Entity<PopupMenu>>,
    dismiss: Option<Subscription>,
    about: bool,
}

#[cfg(any(test, not(target_os = "macos")))]
#[derive(Clone, Copy)]
struct MenuBarColors {
    background: gpui_shell::gpui::Hsla,
    foreground: gpui_shell::gpui::Hsla,
    hover: gpui_shell::gpui::Hsla,
    active: gpui_shell::gpui::Hsla,
    border: gpui_shell::gpui::Hsla,
    surface: gpui_shell::gpui::Hsla,
}

/// Bar and title colors from the theme the shell actually applies.
#[cfg(any(test, not(target_os = "macos")))]
fn menu_bar_colors(colors: &gpui_base::ColorTokens) -> MenuBarColors {
    MenuBarColors {
        background: colors.background,
        foreground: colors.foreground,
        // `secondary` is the shell's hover chip; `accent` is the selected chip.
        hover: colors.secondary,
        active: colors.accent,
        border: colors.border,
        surface: colors.surface,
    }
}

#[cfg(not(target_os = "macos"))]
impl DesktopMenuFrame {
    fn close_menu(&mut self) {
        self.dismiss.take();
        self.popup.take();
        self.open = None;
    }

    fn open_menu(&mut self, ix: usize) {
        if self.open == Some(ix) && self.popup.is_some() {
            return;
        }
        self.dismiss.take();
        self.popup.take();
        self.open = Some(ix);
    }

    fn ensure_popup(&mut self, menus: &[OwnedMenu], window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.open else {
            return;
        };
        if self.popup.is_some() {
            return;
        }
        let Some(menu) = menus.get(ix) else {
            self.open = None;
            return;
        };
        let items = menu.items.clone();
        let popup = PopupMenu::build(window, cx, move |menu, window, cx| {
            fill_popup(menu, &items, window, cx)
        });
        self.dismiss = Some(cx.subscribe_in(&popup, window, Self::on_popup_dismiss));
        self.popup = Some(popup);
    }

    fn on_popup_dismiss(
        &mut self,
        _: &Entity<PopupMenu>,
        _: &DismissEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_menu();
        cx.notify();
    }
}

#[cfg(not(target_os = "macos"))]
fn fill_popup(
    mut menu: PopupMenu,
    items: &[OwnedMenuItem],
    window: &mut Window,
    cx: &mut Context<PopupMenu>,
) -> PopupMenu {
    for item in items {
        menu = match item {
            OwnedMenuItem::Separator => menu.separator(),
            OwnedMenuItem::Action {
                name,
                action,
                checked,
                disabled,
                ..
            } => menu.menu_with_check_and_disabled(
                name.clone(),
                *checked,
                action.boxed_clone(),
                *disabled,
            ),
            OwnedMenuItem::Submenu(submenu) => {
                let nested = submenu.items.clone();
                menu.submenu(submenu.name.clone(), window, cx, move |menu, window, cx| {
                    fill_popup(menu, &nested, window, cx)
                })
            }
            OwnedMenuItem::SystemMenu(_) => menu,
        };
    }
    menu
}

#[cfg(not(target_os = "macos"))]
impl Render for DesktopMenuFrame {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menus: Vec<OwnedMenu> = gpui_component::GlobalState::global(cx).app_menus().to_vec();
        self.ensure_popup(&menus, window, cx);
        let colors = menu_bar_colors(&gpui_base::Theme::global(cx).tokens.colors);
        let open = self.open;
        let popup = self.popup.clone();
        let about = self.about;
        let foreground = colors.foreground;
        let hover = colors.hover;
        let active = colors.active;

        let menu_bar = div()
            .id("app-menu-bar")
            .w_full()
            .h(px(30.0))
            .min_h(px(30.0))
            .flex_none()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .bg(colors.background)
            .border_b_1()
            .border_color(colors.border);
        // Windows keeps the original padding. Linux leaves the right edge
        // flush so the caption buttons sit on the window edge.
        #[cfg(target_os = "linux")]
        let menu_bar = menu_bar.pl_1();
        #[cfg(not(target_os = "linux"))]
        let menu_bar = menu_bar.px_1();
        let menu_bar = menu_bar.children(menus.into_iter().enumerate().map(|(ix, menu)| {
            let opened = open == Some(ix);
            let title_fg = foreground;
            let title_hover = hover;
            let title_active = active;
            div()
                .id(("app-menu-title", ix))
                .relative()
                .h(px(24.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .rounded(px(4.0))
                .text_size(px(13.0))
                .text_color(title_fg)
                .when(opened, |this| this.bg(title_active).text_color(title_fg))
                .hover(move |style| style.bg(title_hover).text_color(title_fg))
                .child(menu.name.clone())
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    if *hovered && this.open.is_some() && this.open != Some(ix) {
                        this.open_menu(ix);
                        cx.notify();
                    }
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    window.listener_for(&cx.entity(), move |this, _, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        if this.open == Some(ix) {
                            this.close_menu();
                        } else {
                            this.open_menu(ix);
                        }
                        cx.notify();
                    }),
                )
                // Linux drags and zooms from the menu row. A press
                // on a title must not also move or maximize.
                .when(cfg!(target_os = "linux"), |this| {
                    this.on_click(|_, _, cx| cx.stop_propagation())
                })
                .when(opened, |this| {
                    let Some(popup) = popup.clone() else {
                        return this;
                    };
                    this.child(deferred(
                        anchored()
                            .anchor(Anchor::TopLeft)
                            .snap_to_window_with_margin(px(8.))
                            .child(div().occlude().top(px(4.)).child(popup)),
                    ))
                })
        }));
        #[cfg(target_os = "linux")]
        let menu_bar = {
            let palette = &gpui_base::Theme::global(cx).tokens.colors;
            menu_bar.child(crate::linux_csd::drag_spacer()).child(
                crate::linux_csd::caption_buttons(
                    window,
                    crate::linux_csd::CaptionColors {
                        foreground: colors.foreground,
                        hover: colors.hover,
                        destructive: palette.destructive,
                        destructive_foreground: palette.destructive_foreground,
                    },
                    |_window, cx| hide_or_quit(cx),
                ),
            )
        };
        #[cfg(target_os = "linux")]
        let menu_bar = crate::linux_csd::arm_drag("app-menu-drag", menu_bar, window, cx);
        let frame = div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(menu_bar)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.0))
                    .child(self.root.clone()),
            )
            .when(about, |this| this.child(about_overlay(colors, cx)));
        #[cfg(target_os = "linux")]
        {
            crate::linux_csd::wrap_client_border(frame)
        }
        #[cfg(not(target_os = "linux"))]
        frame
    }
}

#[cfg(not(target_os = "macos"))]
fn about_overlay(colors: MenuBarColors, cx: &mut Context<DesktopMenuFrame>) -> impl IntoElement {
    let body = about_body();
    div()
        .id("about-scrim")
        .absolute()
        .size_full()
        .top_0()
        .left_0()
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(colors.foreground.opacity(0.35))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                this.about = false;
                cx.notify();
            }),
        )
        .child(
            div()
                .id("about-panel")
                .occlude()
                .min_w(px(280.0))
                .p(px(20.0))
                .flex()
                .flex_col()
                .gap_2()
                .rounded(px(8.0))
                .bg(colors.surface)
                .text_color(colors.foreground)
                .border_1()
                .border_color(colors.border)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .children(
                    body.lines()
                        .map(|line| div().text_size(px(13.0)).child(line.to_string())),
                )
                .child(
                    div()
                        .id("about-ok")
                        .mt(px(8.0))
                        .h(px(28.0))
                        .px(px(12.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(4.0))
                        .bg(colors.active)
                        .text_color(colors.foreground)
                        .text_size(px(13.0))
                        .child("OK")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.about = false;
                                cx.notify();
                            }),
                        ),
                ),
        )
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
    cx.on_action(|_: &AboutApplication, cx| show_about(cx))
        .on_action(|_: &OpenSettings, cx| dispatch_if_unlocked("open-settings", cx))
        .on_action(|_: &NewReturn, cx| dispatch_if_unlocked("new-return", cx))
        .on_action(|_: &ToggleSidebar, cx| dispatch_if_unlocked("toggle-sidebar", cx))
        .on_action(|_: &ToggleTheme, cx| dispatch_if_unlocked("toggle-theme", cx))
        .on_action(|_: &OpenCommandPalette, cx| dispatch_if_unlocked("open-palette", cx))
        .on_action(|_: &SignOut, cx| dispatch_if_unlocked("cloud-sign-out", cx))
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
        .on_action(|_: &CheckForUpdates, _cx| {
            if crate::account_gate::is_unlocked() {
                crate::updater::check_now();
            }
        })
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

fn dispatch_if_unlocked(id: &'static str, cx: &mut App) {
    if crate::account_gate::is_unlocked() {
        dispatch_script(id, cx);
    }
}

fn dispatch_script(id: &'static str, cx: &mut App) {
    with_window(cx, move |window, cx| {
        window.dispatch_action(Box::new(ShellAction::new(id)), cx);
    });
}

/// The window menu actions belong to. The shell window recorded at open wins;
/// otherwise the active window, then any window still open.
fn target_window(cx: &App) -> Option<gpui_shell::gpui::AnyWindowHandle> {
    #[cfg(not(target_os = "macos"))]
    {
        let remembered = MAIN_WINDOW.with(|slot| *slot.borrow());
        if let Some(handle) = remembered
            && cx.windows().contains(&handle)
        {
            return Some(handle);
        }
    }
    cx.active_window()
        .filter(|handle| cx.windows().contains(handle))
        .or_else(|| cx.windows().into_iter().next())
}

/// Run `f` on the main window after the current update returns.
///
/// A menu click dispatches its action from inside the window update the popup
/// deferred into. Updating that same window again finds it taken out of the
/// map and logs `window not found`, so the click does nothing. Keyboard
/// shortcuts never take this path: the script binds them itself. Deferring
/// lands the call once the window is back in the map.
fn with_window(cx: &mut App, f: impl FnOnce(&mut Window, &mut App) + 'static) {
    let Some(handle) = target_window(cx) else {
        tracing::warn!("menu action had no window");
        return;
    };
    cx.defer(move |cx| {
        if let Err(error) = handle.update(cx, |_, window, cx| f(window, cx)) {
            tracing::warn!("menu action had no window: {error}");
        }
    });
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

fn show_about(cx: &mut App) {
    // macOS keeps the native dialog. Linux rfd shells out to zenity, which is
    // not part of the app, so Linux and Windows draw the same text in-window.
    #[cfg(target_os = "macos")]
    {
        let _ = cx;
        let body = about_body();
        std::thread::spawn(move || {
            let _ = rfd::MessageDialog::new()
                .set_title("About eBIRForms")
                .set_description(&body)
                .set_level(rfd::MessageLevel::Info)
                .show();
        });
    }
    #[cfg(not(target_os = "macos"))]
    with_window(cx, |window, cx| {
        let Some(Some(root)) = window.root::<DesktopMenuFrame>() else {
            return;
        };
        root.update(cx, |frame, cx| {
            frame.about = true;
            cx.notify();
        });
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

pub(crate) fn app_menus(platform: MenuPlatform, unlocked: bool) -> Vec<Menu> {
    let mut menus = Vec::new();
    if platform == MenuPlatform::Macos {
        menus.push(macos_app_menu(unlocked));
    }
    if unlocked || platform != MenuPlatform::Macos {
        menus.push(file_menu(platform, unlocked));
    }
    menus.push(edit_menu(unlocked && platform == MenuPlatform::Linux));
    if unlocked {
        menus.push(view_menu(platform));
        menus.push(window_menu(platform == MenuPlatform::Macos));
        menus.push(help_menu(platform != MenuPlatform::Macos, true));
    } else if platform != MenuPlatform::Macos {
        menus.push(help_menu(true, false));
    }
    menus
}

fn macos_app_menu(unlocked: bool) -> Menu {
    let mut items = vec![
        MenuItem::action("About eBIRForms", AboutApplication),
        MenuItem::separator(),
    ];
    if unlocked {
        items.push(MenuItem::action("Settings…", OpenSettings));
        items.push(MenuItem::separator());
        items.push(MenuItem::os_submenu("Services", SystemMenuType::Services));
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Hide eBIRForms", HideToTray));
        items.push(MenuItem::action("Hide Others", HideOthers));
        items.push(MenuItem::action("Show All", ShowAllApplications));
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Sign Out", SignOut));
        items.push(MenuItem::separator());
    }
    items.push(MenuItem::action("Quit eBIRForms", QuitApplication));
    Menu::new("eBIRForms").items(items)
}

fn file_menu(platform: MenuPlatform, unlocked: bool) -> Menu {
    if !unlocked {
        return Menu::new("File").items([MenuItem::action("Quit eBIRForms", QuitApplication)]);
    }
    let mut items = vec![MenuItem::action("New Return…", NewReturn)];
    match platform {
        MenuPlatform::Macos => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Close Window", CloseWindow));
        }
        MenuPlatform::Windows => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Settings…", OpenSettings));
            items.push(MenuItem::action("Sign Out", SignOut));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Hide eBIRForms", HideToTray));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Quit eBIRForms", QuitApplication));
        }
        MenuPlatform::Linux => {
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Close Window", CloseWindow));
            items.push(MenuItem::action("Hide eBIRForms", HideToTray));
            items.push(MenuItem::separator());
            items.push(MenuItem::action("Sign Out", SignOut));
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

fn help_menu(include_about: bool, unlocked: bool) -> Menu {
    let mut items = Vec::new();
    if include_about {
        items.push(MenuItem::action("About eBIRForms", AboutApplication));
        if unlocked {
            items.push(MenuItem::separator());
        }
    }
    if !unlocked {
        return Menu::new("Help").items(items);
    }
    items.push(MenuItem::action("Check for Updates…", CheckForUpdates));
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
        let menus = app_menus(MenuPlatform::Macos, true);
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
                "Sign Out",
                "-",
                "Quit eBIRForms",
            ]
        );
        let app = menu(&menus, "eBIRForms");
        assert!(is_action::<QuitApplication>(named(app, "Quit eBIRForms")));
        assert!(is_action::<HideToTray>(named(app, "Hide eBIRForms")));
        assert!(is_action::<OpenSettings>(named(app, "Settings…")));
        assert!(is_action::<AboutApplication>(named(app, "About eBIRForms")));
        assert!(is_action::<SignOut>(named(app, "Sign Out")));

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
        let menus = app_menus(MenuPlatform::Macos, true);
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
        let menus = app_menus(MenuPlatform::Linux, true);
        let names: Vec<_> = menus.iter().map(|menu| menu.name.to_string()).collect();
        assert_eq!(names, ["File", "Edit", "View", "Window", "Help"]);
        let file = menu(&menus, "File");
        assert_eq!(
            titles(file),
            [
                "New Return…",
                "-",
                "Close Window",
                "Hide eBIRForms",
                "-",
                "Sign Out",
                "Quit eBIRForms",
            ]
        );
        assert!(is_action::<CloseWindow>(named(file, "Close Window")));
        assert!(is_action::<QuitApplication>(named(file, "Quit eBIRForms")));
        assert!(is_action::<HideToTray>(named(file, "Hide eBIRForms")));
        assert!(is_action::<SignOut>(named(file, "Sign Out")));
        assert!(is_action::<OpenSettings>(named(
            menu(&menus, "Edit"),
            "Settings…"
        )));
        assert!(is_action::<AboutApplication>(named(
            menu(&menus, "Help"),
            "About eBIRForms"
        )));
        assert!(is_action::<CheckForUpdates>(named(
            menu(&menus, "Help"),
            "Check for Updates…"
        )));
        assert_eq!(titles(menu(&menus, "Window")), ["Minimize", "Zoom"]);
        assert!(titles(menu(&menus, "View")).contains(&"Enter Full Screen".to_string()));
    }

    #[test]
    fn windows_puts_settings_in_file() {
        let menus = app_menus(MenuPlatform::Windows, true);
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
        assert!(is_action::<CheckForUpdates>(named(
            menu(&menus, "Help"),
            "Check for Updates…"
        )));
        assert!(titles(menu(&menus, "View")).contains(&"Enter Full Screen".to_string()));
    }

    #[test]
    fn locked_menus_keep_about_and_quit_and_drop_filing_commands() {
        for platform in [
            MenuPlatform::Macos,
            MenuPlatform::Linux,
            MenuPlatform::Windows,
        ] {
            let menus = app_menus(platform, false);
            let flat: Vec<String> = menus.iter().flat_map(titles).collect();
            assert!(
                flat.iter().any(|title| title.contains("About")),
                "{platform:?} {flat:?}"
            );
            assert!(
                flat.iter().any(|title| title.contains("Quit")),
                "{platform:?} {flat:?}"
            );
            for banned in [
                "Settings…",
                "New Return…",
                "Sign Out",
                "Command Palette…",
                "Check for Updates…",
                "Toggle Sidebar",
            ] {
                assert!(
                    !flat.iter().any(|title| title == banned),
                    "{platform:?} still has {banned}"
                );
            }
        }
        let mac = app_menus(MenuPlatform::Macos, false);
        assert_eq!(
            titles(menu(&mac, "eBIRForms")),
            ["About eBIRForms", "-", "Quit eBIRForms"]
        );
        assert!(mac.iter().all(|item| item.name.as_ref() != "File"));
        assert!(titles(menu(&mac, "Edit")).contains(&"Paste".to_string()));
    }

    #[test]
    fn help_links_are_the_ones_the_painted_app_already_had() {
        assert!(SUPPORT_URL.starts_with("mailto:support@goldcoders.dev"));
        assert_eq!(WEBSITE_URL, "https://goldcoders.dev");
        let menus = app_menus(MenuPlatform::Macos, true);
        let help = menu(&menus, "Help");
        assert_eq!(
            titles(help),
            [
                "Check for Updates…",
                "Contact Support…",
                "Goldcoders Website"
            ]
        );
        assert!(is_action::<CheckForUpdates>(named(
            help,
            "Check for Updates…"
        )));
        assert!(is_action::<OpenSupportEmail>(named(
            help,
            "Contact Support…"
        )));
        assert!(is_action::<OpenCompanyWebsite>(named(
            help,
            "Goldcoders Website"
        )));
    }

    /// Titles take the theme foreground, not the light-theme secondary color
    /// the stock menu bar paints. Dark foreground is lighter than its bar;
    /// light foreground is darker than its bar.
    #[test]
    fn menu_bar_titles_follow_the_theme_foreground() {
        for colors in [
            gpui_base::ColorTokens::dark(),
            gpui_base::ColorTokens::light(),
        ] {
            let bar = menu_bar_colors(&colors);
            assert_eq!(bar.foreground, colors.foreground);
            assert_eq!(bar.background, colors.background);
            assert_eq!(bar.border, colors.border);
            assert_eq!(bar.surface, colors.surface);
            assert_ne!(bar.foreground, bar.background);
            let dark = colors.background.l < 0.5;
            if dark {
                assert!(bar.foreground.l > bar.background.l);
            } else {
                assert!(bar.foreground.l < bar.background.l);
            }
            assert_ne!(bar.hover, bar.foreground);
            assert_ne!(bar.active, bar.foreground);
        }
    }
}
