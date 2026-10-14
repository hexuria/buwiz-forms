//! macOS-specific UI integrations for bir-desktop.

use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
/// macOS enforces this natively via the application bundle, so this is a no-op.
pub fn enforce_single_instance() {}

/// Routes app-level `QuitApplication` (menu Quit / Cmd+Q while no window of
/// ours handles it, e.g. with the main window hidden) to the main window's
/// `AppState`, so every Quit takes the same `handle_quit_application` path.
#[derive(Clone, Default)]
pub struct MacosQuitRouter {
    target: Rc<RefCell<Option<MacosQuitTarget>>>,
}

type MacosQuitTarget = (AnyWindowHandle, WeakEntity<crate::app::AppState>);

impl MacosQuitRouter {
    pub fn bind(&self, main_window: AnyWindowHandle, app_state: &Entity<crate::app::AppState>) {
        self.target
            .replace(Some((main_window, app_state.downgrade())));
    }

    /// Before the main window exists there is nothing to keep resident or
    /// save, so quit outright. Otherwise defer (the action may be dispatched
    /// from inside the main window, which cannot be re-entered) and let
    /// `AppState::handle_quit_application` apply the user's Cmd+Q setting.
    fn request_quit(&self, cx: &mut App) {
        let target = self.target.borrow().clone();
        let Some((main_window, app_state)) = target else {
            cx.quit();
            return;
        };
        cx.defer(move |cx| {
            let Some(app_state) = app_state.upgrade() else {
                cx.quit();
                return;
            };
            let routed = main_window.update(cx, |_, window, cx| {
                app_state.update(cx, |state, cx| {
                    state.handle_quit_application(
                        &crate::global_actions::QuitApplication,
                        window,
                        cx,
                    );
                });
            });
            if let Err(error) = routed {
                tracing::warn!(%error, "Main window gone; quitting from the application menu");
                cx.quit();
            }
        });
    }
}

/// App-level handlers for the application menu, so its items work even when
/// no window of ours is key (window hidden to the menu bar).
fn register_app_menu_actions(cx: &mut App) -> MacosQuitRouter {
    use crate::global_actions::*;

    let quit_router = MacosQuitRouter::default();
    let quit_action_router = quit_router.clone();
    cx.on_action(|_: &AboutApplication, _| show_standard_about_panel())
        .on_action(|_: &HideApplication, cx| cx.hide())
        .on_action(|_: &HideOthers, cx| cx.hide_other_apps())
        .on_action(|_: &ShowAllApplications, cx| cx.unhide_other_apps())
        .on_action(move |_: &QuitApplication, cx| quit_action_router.request_quit(cx))
        .on_action(|_: &BringAllToFront, _| bring_all_windows_to_front())
        .on_action(|_: &OpenSupportEmail, cx| {
            cx.open_url("mailto:support@goldcoders.dev?subject=eBIRForms%20Support%20Request");
        })
        .on_action(|_: &OpenCompanyWebsite, cx| {
            cx.open_url("https://goldcoders.dev");
        });
    quit_router
}

/// Install the standard macOS application menus before opening the main window.
///
/// GPUI does not create a native `NSApplication.mainMenu` unless `set_menus`
/// is called. Without it, the application name appears in the macOS menu bar
/// but highlights without opening a menu. The Quit label starts as the
/// default (real quit); `set_quit_menu_label` updates it once the database
/// (and with it the user's Cmd+Q setting) is open.
pub fn install_app_menu(cx: &mut App) -> MacosQuitRouter {
    let quit_router = register_app_menu_actions(cx);
    cx.set_menus(app_menus(false));
    log_native_menu_installation();
    quit_router
}

/// Rebuild the menu bar so the Quit item names what Cmd+Q does:
/// "Quit eBIRForms", or "Close to Menu Bar" with "Cmd+Q hides to menu bar" on.
pub fn set_quit_menu_label(cx: &mut App, quit_hides: bool) {
    cx.set_menus(app_menus(quit_hides));
}

fn quit_menu_label(quit_hides: bool) -> &'static str {
    if quit_hides {
        "Close to Menu Bar"
    } else {
        "Quit eBIRForms"
    }
}

/// Keep Settings enabled in the native application menu and forward it to the
/// existing in-app Settings page.
///
/// GPUI validates native menu items against application-level handlers. The
/// view-level handler remains useful for keyboard dispatch, but is not always
/// visible while AppKit is validating the application menu.
pub fn register_settings_menu_action(
    main_window: AnyWindowHandle,
    app_state: Entity<crate::app::AppState>,
    cx: &mut App,
) {
    use crate::{app::ActiveView, global_actions::OpenSettings};

    cx.on_action(move |_: &OpenSettings, cx| {
        let app_state = app_state.clone();
        if let Err(error) = main_window.update(cx, move |_, window, cx| {
            app_state.update(cx, |state, cx| {
                state.request_admin_access(ActiveView::Settings, window, cx);
            });
        }) {
            tracing::warn!(%error, "Could not open Settings from the macOS application menu");
        }
    });
}

fn app_menus(quit_hides: bool) -> Vec<Menu> {
    use crate::global_actions::*;

    vec![
        Menu::new("eBIRForms").items([
            MenuItem::action("About eBIRForms", AboutApplication),
            MenuItem::separator(),
            MenuItem::action("Settings...", OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide eBIRForms", HideApplication),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAllApplications),
            MenuItem::separator(),
            MenuItem::action(quit_menu_label(quit_hides), QuitApplication),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Profile", CreateProfile),
            MenuItem::separator(),
            MenuItem::action("Close Window", CloseWindow),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", gpui_component::input::Undo, OsAction::Undo),
            MenuItem::os_action("Redo", gpui_component::input::Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", gpui_component::input::Cut, OsAction::Cut),
            MenuItem::os_action("Copy", gpui_component::input::Copy, OsAction::Copy),
            MenuItem::os_action("Paste", gpui_component::input::Paste, OsAction::Paste),
            MenuItem::action("Delete", gpui_component::input::Delete),
            MenuItem::separator(),
            MenuItem::action("Find", FocusSearch),
            MenuItem::os_action(
                "Select All",
                gpui_component::input::SelectAll,
                OsAction::SelectAll,
            ),
        ]),
        Menu::new("View").items([
            MenuItem::action("Toggle Sidebar", ToggleSidebar),
            MenuItem::action("Toggle Compact Sidebar", ToggleSidebarMini),
            MenuItem::action("Toggle Theme", ToggleTheme),
            MenuItem::separator(),
            MenuItem::action("Command Palette...", OpenCommandPalette),
            MenuItem::separator(),
            MenuItem::action("Enter Full Screen", ToggleFullScreen),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", MinimizeWindow),
            MenuItem::action("Zoom", ZoomWindow),
            MenuItem::separator(),
            MenuItem::action("Bring All to Front", BringAllToFront),
        ]),
        Menu::new("Help").items([
            MenuItem::action("Contact Support...", OpenSupportEmail),
            MenuItem::action("Goldcoders Website", OpenCompanyWebsite),
        ]),
    ]
}

fn show_standard_about_panel() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    // SAFETY: GPUI invokes global action handlers on the application main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.orderFrontStandardAboutPanel(None) };
}

fn bring_all_windows_to_front() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    // SAFETY: GPUI invokes global action handlers on the application main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.arrangeInFront(None) };
}

fn log_native_menu_installation() {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    // SAFETY: Menu installation and this verification run synchronously on the
    // GPUI application thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    let top_level_menu_count = unsafe {
        app.mainMenu()
            .map(|menu| menu.numberOfItems())
            .unwrap_or_default()
    };

    if top_level_menu_count >= 6 {
        tracing::info!(
            top_level_menu_count,
            "Native macOS application menu installed"
        );
    } else {
        tracing::error!(
            top_level_menu_count,
            "Native macOS application menu installation is incomplete"
        );
    }
}

// ── Keybindings ──────────────────────────────────────────────────────────────

/// Register global keybindings using the macOS `cmd` modifier.
pub fn bind_global_keys(cx: &mut App) {
    use crate::global_actions::*;

    cx.bind_keys([
        KeyBinding::new("cmd-enter", SubmitCurrentForm, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-shift-b", ToggleSidebarMini, None),
        KeyBinding::new("cmd-f", FocusSearch, None),
        KeyBinding::new("cmd-n", CreateProfile, None),
        KeyBinding::new("cmd-shift-t", ToggleTheme, None),
        KeyBinding::new("cmd-shift-x", OpenCronTasks, None),
        KeyBinding::new("cmd-,", OpenSettings, None),
        KeyBinding::new("cmd-k", OpenCommandPalette, None),
        KeyBinding::new("f1", OpenGlobalDashboard, None),
        KeyBinding::new("cmd-q", QuitApplication, None),
        KeyBinding::new("cmd-h", HideApplication, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", MinimizeWindow, None),
        KeyBinding::new("cmd-ctrl-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-=", ZoomIn, None),
        KeyBinding::new("cmd--", ZoomOut, None),
        KeyBinding::new("cmd-0", ResetZoom, None),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in Finder using `open -R`.
pub(crate) fn reveal_in_file_manager(path: &std::path::Path) {
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
}

/// Open a file with the default system application.
pub fn open_in_system(path: &std::path::Path) {
    let _ = open::that(path);
}

// ── Native Print ─────────────────────────────────────────────────────────────

/// Print a PDF using macOS native AppKit/PDFKit via a Swift helper script.
///
/// Falls back to `open::that` if the Swift runtime is unavailable.
pub fn print_pdf(path: &std::path::Path) -> Result<(), &'static str> {
    let path = path.to_path_buf();

    std::thread::spawn(move || {
        use std::io::Write;

        let script = r#"
import AppKit
import PDFKit

func printPDF(path: String) {
    let url = URL(fileURLWithPath: path)
    guard let pdfDoc = PDFDocument(url: url) else { exit(1) }

    let printInfo = NSPrintInfo.shared
    printInfo.isHorizontallyCentered = true
    printInfo.isVerticallyCentered = true

    let printOp = pdfDoc.printOperation(for: printInfo, scalingMode: .pageScaleDownToFit, autoRotate: true)

    let app = NSApplication.shared
    app.setActivationPolicy(.accessory)
    app.activate(ignoringOtherApps: true)

    printOp?.showsPrintPanel = true
    printOp?.showsProgressPanel = true
    printOp?.run()
}

let args = CommandLine.arguments
if args.count > 1 {
    printPDF(path: args[1])
}
"#;

        let mut child = match std::process::Command::new("swift")
            .arg("-")
            .arg(&path)
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => {
                let _ = open::that(&path);
                return;
            }
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(script.as_bytes());
        }

        let output = child.wait();
        if output.is_err() || !output.unwrap().success() {
            // Silent fallback to standard open if the Swift script fails
            let _ = open::that(&path);
        }
    });

    Ok(())
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
/// `Menlo` ships with every macOS. The previous `.SF NS Mono` is a hidden
/// system family CoreText will not resolve by name, so every monospace text
/// element fell back to the UI font after a failed lookup — and, with
/// `RUST_BACKTRACE=1`, after gpui captured a backtrace for the error, per
/// element, per frame.
pub const MONOSPACE_FONT: &str = "Menlo";

// ── Dock Management ──────────────────────────────────────────────────────────

/// Hides the application from the macOS Dock and explicitly hides all windows
/// from tiling window managers (e.g. AeroSpace).
pub fn hide_from_dock() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        // SAFETY: These dock management functions are only ever called by GPUI
        // from the main thread (enforced by GPUI's async executor). Constructing
        // a MainThreadMarker here is sound because the calling context guarantees
        // main-thread execution.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let app = NSApplication::sharedApplication(mtm);

        // Order out all windows so tiling WMs drop them from the layout.
        let windows = app.windows();
        for i in 0..windows.len() {
            if let Some(window) = windows.get(i) {
                window.orderOut(None);
            }
        }

        // Set activation policy to Accessory (removes the Dock icon).
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

/// Restores the application to the macOS Dock and brings all windows back.
///
/// Switching an already-running app from `Accessory` back to `Regular` leaves
/// AppKit's menu bar in a known broken state: the app name shows in the menu
/// bar and highlights when clicked, but no menu opens until the app is
/// deactivated and activated again. Every hide-to-menu-bar path (close
/// button, Cmd+W, tray Hide, Cmd+Q with "hides to menu bar" on) followed by
/// any show path (tray Show, Dock/Finder relaunch, the global hotkey) hits
/// it. After such a switch, hand activation to the Dock and take it back on
/// the next main-queue turn, which makes the menu bar ours again.
pub fn show_in_dock() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        // SAFETY: See `hide_from_dock` — always called on the GPUI main thread.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let app = NSApplication::sharedApplication(mtm);

        // SAFETY: reading the policy has no side effects (see `toggle_app_visibility`).
        let was_regular =
            unsafe { app.activationPolicy() } == NSApplicationActivationPolicy::Regular;

        // 1. Restore Dock icon first.
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        // 2. Restore all windows.
        bring_windows_to_front(&app);

        // 3. Activate. `activateIgnoringOtherApps` rather than `activate`:
        // the latter does not exist before macOS 14 (the minimum is 13) and is
        // only a cooperative request that is refused when, e.g., Show comes
        // from the tray menu while another app is frontmost.
        if was_regular {
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        } else {
            reclaim_menu_bar();
        }
    }
}

#[cfg(target_os = "macos")]
fn bring_windows_to_front(app: &objc2_app_kit::NSApplication) {
    let windows = app.windows();
    for i in 0..windows.len() {
        if let Some(window) = windows.get(i) {
            window.makeKeyAndOrderFront(None);
        }
    }
}

/// See `show_in_dock`: activate the Dock, then re-activate this app after
/// ~100 ms on the main queue. Without a running Dock, activate directly.
#[cfg(target_os = "macos")]
fn reclaim_menu_bar() {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    use objc2_foundation::NSString;
    use std::ffi::c_void;

    unsafe extern "C" {
        static _dispatch_main_q: c_void;
        fn dispatch_time(when: u64, delta: i64) -> u64;
        fn dispatch_after_f(
            when: u64,
            queue: *const c_void,
            context: *mut c_void,
            work: extern "C" fn(*mut c_void),
        );
    }

    extern "C" fn activate_self(_context: *mut c_void) {
        use objc2_app_kit::NSApplication;
        use objc2_foundation::MainThreadMarker;

        // SAFETY: dispatched onto the main queue below.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        bring_windows_to_front(&app);
    }

    // SAFETY: plain AppKit class methods on the main thread.
    let dock = unsafe {
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
            "com.apple.dock",
        ))
    };
    let handed_off = match dock.first() {
        #[allow(deprecated)]
        Some(dock) => unsafe {
            dock.activateWithOptions(
                NSApplicationActivationOptions::NSApplicationActivateIgnoringOtherApps,
            )
        },
        None => false,
    };
    if !handed_off {
        activate_self(std::ptr::null_mut());
        return;
    }
    // SAFETY: `_dispatch_main_q` is libdispatch's main queue object (what
    // `dispatch_get_main_queue()` returns); DISPATCH_TIME_NOW is 0. The work
    // function ignores its null context.
    unsafe {
        dispatch_after_f(
            dispatch_time(0, 100_000_000),
            &raw const _dispatch_main_q,
            std::ptr::null_mut(),
            activate_self,
        );
    }
}

/// Toggles the application visibility: hides if visible, shows if hidden.
pub fn toggle_app_visibility() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        // SAFETY: See `hide_from_dock` — always called on the GPUI main thread.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let app = NSApplication::sharedApplication(mtm);

        // SAFETY: `activationPolicy` is marked unsafe in objc2-app-kit 0.2 because the
        // Objective-C runtime cannot statically verify the policy value. We only read
        // the policy and pass it to a safe match — no invalid policy value is ever
        // written, and `NSApplication::sharedApplication` guarantees a valid receiver.
        let policy = unsafe { app.activationPolicy() };

        if policy == NSApplicationActivationPolicy::Regular {
            hide_from_dock();
        } else {
            show_in_dock();
        }
    }
}

#[cfg(test)]
mod menu_tests {
    use super::{app_menus, register_app_menu_actions};
    use gpui::{Action, Menu, MenuItem, OsAction};

    fn menu<'a>(menus: &'a [Menu], name: &str) -> &'a Menu {
        menus
            .iter()
            .find(|menu| menu.name.as_ref() == name)
            .unwrap_or_else(|| panic!("no {name} menu"))
    }

    /// Item titles; separators as "-", the Services submenu as "Services".
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

    fn actions(menu: &Menu) -> Vec<(&str, &dyn Action, Option<OsAction>)> {
        menu.items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Action {
                    name,
                    action,
                    os_action,
                    ..
                } => Some((name.as_ref(), action.as_ref(), *os_action)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn application_menu_is_standard_and_quit_names_what_cmd_q_does() {
        for (quit_hides, quit_title) in [(false, "Quit eBIRForms"), (true, "Close to Menu Bar")] {
            let menus = app_menus(quit_hides);
            assert_eq!(
                titles(menu(&menus, "eBIRForms")),
                [
                    "About eBIRForms",
                    "-",
                    "Settings...",
                    "-",
                    "Services",
                    "-",
                    "Hide eBIRForms",
                    "Hide Others",
                    "Show All",
                    "-",
                    quit_title,
                ]
            );
            let quit = actions(menu(&menus, "eBIRForms"))
                .into_iter()
                .find(|(name, ..)| *name == quit_title)
                .unwrap();
            assert_eq!(quit.1.name(), crate::global_actions::QuitApplication.name());
        }
    }

    #[test]
    fn edit_items_use_the_native_text_selectors() {
        let menus = app_menus(false);
        let edit = actions(menu(&menus, "Edit"));
        for (title, os_action) in [
            ("Undo", OsAction::Undo),
            ("Redo", OsAction::Redo),
            ("Cut", OsAction::Cut),
            ("Copy", OsAction::Copy),
            ("Paste", OsAction::Paste),
            ("Select All", OsAction::SelectAll),
        ] {
            let (_, action, os) = edit
                .iter()
                .find(|(name, ..)| *name == title)
                .unwrap_or_else(|| panic!("Edit menu lacks {title}"));
            assert!(*os == Some(os_action), "{title} has the wrong OS action");
            assert!(
                action.name().starts_with("input::"),
                "{title} must reach the text input, got {}",
                action.name()
            );
        }
    }

    /// The app and Help menus must work with the main window hidden to the
    /// menu bar, so each of their actions needs an app-level handler. Settings
    /// gets its handler once the main window exists (`register_settings_menu_action`).
    #[gpui::test]
    fn app_and_help_items_are_enabled_with_no_window(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let _router = register_app_menu_actions(cx);
            let menus = app_menus(false);
            for menu_name in ["eBIRForms", "Help"] {
                for (title, action, _) in actions(menu(&menus, menu_name)) {
                    if title == "Settings..." {
                        continue;
                    }
                    assert!(
                        cx.is_action_available(action),
                        "{menu_name} > {title} has no app-level handler"
                    );
                }
            }
        });
    }
}
