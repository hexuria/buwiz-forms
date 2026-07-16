//! macOS-specific UI integrations for bir-desktop.

use gpui::*;
use std::cell::RefCell;
use std::rc::Rc;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
/// macOS enforces this natively via the application bundle, so this is a no-op.
pub fn enforce_single_instance() {}

/// Install the standard macOS application menus before opening the main window.
///
/// GPUI does not create a native `NSApplication.mainMenu` unless `set_menus`
/// is called. Without it, the application name appears in the macOS menu bar
/// but highlights without opening a menu.
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

    fn request_quit(&self, cx: &mut App) {
        let target = self.target.borrow().clone();
        let Some((main_window, app_state)) = target else {
            cx.quit();
            return;
        };
        let Some(app_state) = app_state.upgrade() else {
            cx.quit();
            return;
        };
        if let Err(error) = main_window.update(cx, move |_, window, cx| {
            app_state.update(cx, |state, cx| {
                state.request_application_quit(window, cx, || {});
            });
        }) {
            tracing::warn!(%error, "Could not route macOS Quit through the application state");
        }
    }
}

pub fn install_app_menu(cx: &mut App) -> MacosQuitRouter {
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

    cx.set_menus(app_menus());
    log_native_menu_installation();
    quit_router
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

fn app_menus() -> Vec<Menu> {
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
            MenuItem::action("Quit eBIRForms", QuitApplication),
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
pub const MONOSPACE_FONT: &str = ".SF NS Mono";

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
pub fn show_in_dock() {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
        use objc2_foundation::MainThreadMarker;

        // SAFETY: See `hide_from_dock` — always called on the GPUI main thread.
        let mtm = unsafe { MainThreadMarker::new_unchecked() };
        let app = NSApplication::sharedApplication(mtm);

        // 1. Restore Dock icon first.
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        // SAFETY: Activation is safe as it's a standard NSApplication method
        unsafe { app.activate() };

        // 3. Restore all windows.
        let windows = app.windows();
        for i in 0..windows.len() {
            if let Some(window) = windows.get(i) {
                window.makeKeyAndOrderFront(None);
            }
        }
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
