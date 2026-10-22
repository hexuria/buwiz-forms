//! Menu-bar status item, hide/show, and the global show hotkey.
//!
//! This is the painted app's tray (`tray-icon` + `global-hotkey`), not a new
//! one. Hide removes the window (and, on macOS, the Dock icon). Show is the
//! tray's Show item, a click on the status item while the app is hidden, the
//! stored global hotkey, or a Dock/taskbar reopen.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use global_hotkey::hotkey::HotKey;
use gpui::{App, AsyncApp};

static HIDDEN: AtomicBool = AtomicBool::new(false);
static TRAY_UP: AtomicBool = AtomicBool::new(false);

thread_local! {
    static TRAY: RefCell<Option<TrayUi>> = const { RefCell::new(None) };
}

struct TrayUi {
    _icon: tray_icon::TrayIcon,
    show: tray_icon::menu::MenuId,
    hide: tray_icon::menu::MenuId,
    quit: tray_icon::menu::MenuId,
}

/// GTK must be up before the tray menu is built, and before GPUI opens its
/// own display connection. No-op off Linux.
pub fn init_gtk() {
    #[cfg(target_os = "linux")]
    match gtk::init() {
        Ok(()) => tracing::info!("GTK initialized for the system tray"),
        Err(error) => {
            tracing::warn!("gtk::init failed; the system tray may be unavailable: {error}")
        }
    }
}

/// Whether a status item was created. Hide falls back to quit where a failed
/// tray would leave no way back.
pub fn tray_available() -> bool {
    TRAY_UP.load(Ordering::SeqCst)
}

/// Drop the status item. Quit does this so the icon does not outlive the process.
pub fn drop_tray() {
    TRAY.with(|slot| drop(slot.borrow_mut().take()));
    TRAY_UP.store(false, Ordering::SeqCst);
}

/// Create the status item and poll its menu, clicks, and the global hotkey
/// for the life of the process.
pub fn start(cx: &mut App) {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        if let Some(ui) = try_create_tray() {
            TRAY.with(|slot| *slot.borrow_mut() = Some(ui));
            TRAY_UP.store(true, Ordering::SeqCst);
        }
        cx.spawn(async move |cx| poll_loop(cx).await).detach();
    });
}

fn try_create_tray() -> Option<TrayUi> {
    let created = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let tray_menu = tray_icon::menu::Menu::new();
        let show = tray_icon::menu::MenuItem::new("Show eBIRForms", true, None);
        let hide = tray_icon::menu::MenuItem::new("Hide eBIRForms", true, None);
        let quit = tray_icon::menu::MenuItem::new("Quit", true, None);
        tray_menu
            .append_items(&[
                &show,
                &hide,
                &tray_icon::menu::PredefinedMenuItem::separator(),
                &quit,
            ])
            .map_err(|error| error.to_string())?;

        let icon_data = include_bytes!("../../../assets/images/e_logo.png");
        let img = image::load_from_memory(icon_data)
            .map_err(|error| error.to_string())?
            .into_rgba8();
        let (width, height) = img.dimensions();
        let icon = tray_icon::Icon::from_rgba(img.into_raw(), width, height)
            .map_err(|error| error.to_string())?;
        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("eBIRForms")
            .with_icon(icon)
            .build()
            .map_err(|error| error.to_string())?;
        Ok::<TrayUi, String>(TrayUi {
            _icon: tray,
            show: show.id().clone(),
            hide: hide.id().clone(),
            quit: quit.id().clone(),
        })
    }));
    match created {
        Ok(Ok(ui)) => Some(ui),
        Ok(Err(error)) => {
            tracing::warn!("system tray unavailable; continuing without it: {error}");
            None
        }
        Err(_) => {
            tracing::warn!("system tray panicked during create; continuing without it");
            None
        }
    }
}

async fn poll_loop(cx: &mut AsyncApp) {
    let mut hotkeys = HotkeyWatch::new();
    loop {
        let stop = cx.update(|cx| {
            let stop = poll_tray(cx);
            hotkeys.poll(cx);
            stop
        });
        if stop {
            break;
        }
        cx.background_executor()
            .timer(std::time::Duration::from_millis(100))
            .await;
    }
}

enum TrayCommand {
    Show,
    Hide,
    Quit,
}

fn poll_tray(cx: &mut App) -> bool {
    while let Ok(event) = tray_icon::menu::MenuEvent::receiver().try_recv() {
        let command = TRAY.with(|slot| {
            let slot = slot.borrow();
            let ui = slot.as_ref()?;
            if event.id == ui.show {
                Some(TrayCommand::Show)
            } else if event.id == ui.hide {
                Some(TrayCommand::Hide)
            } else if event.id == ui.quit {
                Some(TrayCommand::Quit)
            } else {
                None
            }
        });
        match command {
            Some(TrayCommand::Show) => show_app(cx),
            Some(TrayCommand::Hide) => hide_to_tray(),
            Some(TrayCommand::Quit) => {
                crate::app_menu::quit_application(cx);
                return true;
            }
            None => {}
        }
    }

    // A click opens the menu when the window is up. While the app is hidden,
    // the same click is the way back (the menu's Show item does it too).
    while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
        let restore = matches!(
            event,
            tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } | tray_icon::TrayIconEvent::DoubleClick {
                button: tray_icon::MouseButton::Left,
                ..
            }
        );
        if restore && app_is_hidden() {
            show_app(cx);
        }
    }
    false
}

struct HotkeyWatch {
    manager: Option<global_hotkey::GlobalHotKeyManager>,
    registered: Option<HotKey>,
    combo: Option<String>,
    tick: u32,
}

impl HotkeyWatch {
    fn new() -> Self {
        let manager = match global_hotkey::GlobalHotKeyManager::new() {
            Ok(manager) => Some(manager),
            Err(error) => {
                tracing::warn!("global show hotkey unavailable: {error}");
                None
            }
        };
        Self {
            manager,
            registered: None,
            combo: None,
            tick: 0,
        }
    }

    fn poll(&mut self, cx: &mut App) {
        if self.manager.is_some() {
            while let Ok(event) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
                if event.state == global_hotkey::HotKeyState::Pressed {
                    toggle(cx);
                }
            }
        }

        self.tick = self.tick.wrapping_add(1);
        if self.tick != 1 && !self.tick.is_multiple_of(10) {
            return;
        }
        let stored = crate::bir_module::stored_global_hotkey();
        if stored == self.combo {
            return;
        }
        if let (Some(manager), Some(old)) = (self.manager.as_ref(), self.registered.take()) {
            let _ = manager.unregister(old);
        }
        if let (Some(manager), Some(combo)) = (self.manager.as_ref(), stored.as_deref()) {
            match crate::hotkey::build_hotkey(combo) {
                Some(hotkey) => match manager.register(hotkey) {
                    Ok(()) => {
                        self.registered = Some(hotkey);
                        tracing::info!("global show hotkey registered: {combo}");
                    }
                    Err(error) => {
                        tracing::warn!("could not register global hotkey '{combo}': {error}");
                    }
                },
                None => tracing::warn!("invalid global hotkey combo '{combo}'"),
            }
        }
        self.combo = stored;
    }
}

/// Hide every window and, on macOS, leave the Dock. The process and the
/// status item stay up.
pub fn hide_to_tray() {
    HIDDEN.store(true, Ordering::SeqCst);
    platform_hide();
}

/// Bring the windows back and take focus.
pub fn show_app(cx: &mut App) {
    HIDDEN.store(false, Ordering::SeqCst);
    platform_show();
    cx.activate(true);
}

fn toggle(cx: &mut App) {
    if app_is_hidden() {
        show_app(cx);
    } else {
        hide_to_tray();
    }
}

fn app_is_hidden() -> bool {
    #[cfg(target_os = "macos")]
    if let Some(accessory) = macos_is_accessory() {
        return accessory;
    }
    HIDDEN.load(Ordering::SeqCst)
}

fn platform_hide() {
    #[cfg(target_os = "macos")]
    macos_hide();
    #[cfg(target_os = "windows")]
    windows_set_visible(false);
    #[cfg(target_os = "linux")]
    linux_set_mapped(false);
}

fn platform_show() {
    #[cfg(target_os = "macos")]
    macos_show();
    #[cfg(target_os = "windows")]
    windows_set_visible(true);
    #[cfg(target_os = "linux")]
    linux_set_mapped(true);
}

#[cfg(target_os = "macos")]
fn macos_app() -> Option<objc2::rc::Retained<objc2_app_kit::NSApplication>> {
    let mtm = objc2::MainThreadMarker::new()?;
    Some(objc2_app_kit::NSApplication::sharedApplication(mtm))
}

#[cfg(target_os = "macos")]
fn macos_is_accessory() -> Option<bool> {
    use objc2_app_kit::NSApplicationActivationPolicy;
    Some(macos_app()?.activationPolicy() == NSApplicationActivationPolicy::Accessory)
}

#[cfg(target_os = "macos")]
fn macos_each_window(f: impl Fn(&objc2_app_kit::NSWindow)) {
    let Some(app) = macos_app() else {
        return;
    };
    let windows = app.windows();
    for index in 0..windows.count() {
        f(&windows.objectAtIndex(index));
    }
}

#[cfg(target_os = "macos")]
fn macos_hide() {
    use objc2_app_kit::NSApplicationActivationPolicy;
    macos_each_window(|window| window.orderOut(None));
    if let Some(app) = macos_app() {
        let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

#[cfg(target_os = "macos")]
fn macos_show() {
    use objc2_app_kit::NSApplicationActivationPolicy;
    let Some(app) = macos_app() else {
        return;
    };
    let was_regular = app.activationPolicy() == NSApplicationActivationPolicy::Regular;
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    macos_each_window(|window| window.makeKeyAndOrderFront(None));
    if was_regular {
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    } else {
        // Accessory → Regular leaves the menu bar highlighting with nothing
        // behind it until the app is deactivated and activated again.
        reclaim_menu_bar();
    }
}

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
        let Some(app) = macos_app() else {
            return;
        };
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        macos_each_window(|window| window.makeKeyAndOrderFront(None));
    }

    let dock = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(
        "com.apple.dock",
    ));
    let handed_off = if dock.count() > 0 {
        #[allow(deprecated)]
        dock.objectAtIndex(0)
            .activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
    } else {
        false
    };
    if !handed_off {
        activate_self(std::ptr::null_mut());
        return;
    }
    unsafe {
        dispatch_after_f(
            dispatch_time(0, 100_000_000),
            &raw const _dispatch_main_q,
            std::ptr::null_mut(),
            activate_self,
        );
    }
}

#[cfg(target_os = "linux")]
fn linux_set_mapped(mapped: bool) {
    // GPUI has no window-hide on Linux. `xdotool` is the same best-effort
    // the painted app uses; without it the tray Show item still activates.
    let verb = if mapped { "windowmap" } else { "windowunmap" };
    let _ = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "xdotool search --pid {} | xargs -I {{}} xdotool {verb} {{}}",
            std::process::id()
        ))
        .spawn();
}

#[cfg(target_os = "windows")]
fn windows_set_visible(visible: bool) {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, SW_HIDE, SW_RESTORE, SW_SHOW, SetForegroundWindow,
        ShowWindow,
    };
    use windows::core::BOOL;

    unsafe extern "system" fn visit(hwnd: HWND, visible: LPARAM) -> BOOL {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
        if pid == unsafe { GetCurrentProcessId() } {
            if visible.0 == 0 {
                let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
            } else {
                let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
                let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
                let _ = unsafe { SetForegroundWindow(hwnd) };
            }
        }
        BOOL(1)
    }

    let flag = LPARAM(if visible { 1 } else { 0 });
    unsafe {
        let _ = EnumWindows(Some(visit), flag);
    }
}
