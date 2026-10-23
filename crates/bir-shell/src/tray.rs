//! Menu-bar status item, hide/show, and the global show hotkey.
//!
//! This is the painted app's tray (`tray-icon` + `global-hotkey`), not a new
//! one. Hide removes the window (and, on macOS, the Dock icon). Show is the
//! tray's Show item, a click on the status item while the app is hidden, the
//! stored global hotkey, or a Dock/taskbar reopen.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use global_hotkey::hotkey::HotKey;
use gpui_shell::gpui::{App, AsyncApp};

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
    /// XEmbed status icon for trays that do not host StatusNotifierItem
    /// (stalonetray). Stored so the icon stays registered until the tray is
    /// dropped; nothing else reads it.
    #[cfg(target_os = "linux")]
    #[allow(dead_code)]
    xembed: Option<XembedIcon>,
}

#[cfg(target_os = "linux")]
struct XembedIcon(*mut gtk::ffi::GtkStatusIcon);

#[cfg(target_os = "linux")]
static XEMBED_ACTIVATE: AtomicBool = AtomicBool::new(false);

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

/// StatusNotifierItem registers over D-Bus from GTK callbacks. Nothing else
/// runs the GTK loop, so without this the icon PNG is written and the item
/// never appears. Non-blocking; safe to call on the thread that ran
/// [`init_gtk`].
#[cfg(target_os = "linux")]
fn pump_gtk() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

#[cfg(target_os = "linux")]
impl Drop for XembedIcon {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe extern "C" {
            fn g_object_unref(object: *mut std::ffi::c_void);
        }
        unsafe {
            gtk::ffi::gtk_status_icon_set_visible(self.0, 0);
            g_object_unref(self.0.cast());
        }
    }
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

/// XEmbed icon for system trays that only speak the freedesktop tray
/// protocol. StatusNotifier (libayatana-appindicator) does not embed there.
/// The same `e_logo.png` the SNI icon uses is the pixmap.
#[cfg(target_os = "linux")]
fn install_xembed_icon() -> Option<XembedIcon> {
    let path = write_tray_logo()?;
    let path = std::ffi::CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let tooltip = c"eBIRForms";
    unsafe {
        let icon = gtk::ffi::gtk_status_icon_new_from_file(path.as_ptr());
        if icon.is_null() {
            return None;
        }
        gtk::ffi::gtk_status_icon_set_tooltip_text(icon, tooltip.as_ptr());
        gtk::ffi::gtk_status_icon_set_visible(icon, 1);
        connect_xembed_activate(icon);
        Some(XembedIcon(icon))
    }
}

#[cfg(target_os = "linux")]
fn write_tray_logo() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("ebirforms-tray");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("e_logo.png");
    std::fs::write(&path, include_bytes!("../../../assets/images/e_logo.png")).ok()?;
    Some(path)
}

#[cfg(target_os = "linux")]
unsafe fn connect_xembed_activate(icon: *mut gtk::ffi::GtkStatusIcon) {
    unsafe extern "C" {
        fn g_signal_connect_data(
            instance: *mut std::ffi::c_void,
            detailed_signal: *const std::ffi::c_char,
            c_handler: Option<unsafe extern "C" fn()>,
            data: *mut std::ffi::c_void,
            destroy_data: Option<
                unsafe extern "C" fn(*mut std::ffi::c_void, *mut std::ffi::c_void),
            >,
            connect_flags: u32,
        ) -> u64;
    }

    unsafe extern "C" fn on_activate(_icon: *mut gtk::ffi::GtkStatusIcon) {
        XEMBED_ACTIVATE.store(true, Ordering::SeqCst);
    }

    let callback = unsafe {
        std::mem::transmute::<*const (), Option<unsafe extern "C" fn()>>(on_activate as *const ())
    };
    unsafe {
        g_signal_connect_data(
            icon.cast(),
            c"activate".as_ptr(),
            callback,
            std::ptr::null_mut(),
            None,
            0,
        );
    }
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
        #[cfg(target_os = "linux")]
        let xembed = install_xembed_icon();
        #[cfg(target_os = "linux")]
        pump_gtk();
        Ok::<TrayUi, String>(TrayUi {
            _icon: tray,
            show: show.id().clone(),
            hide: hide.id().clone(),
            quit: quit.id().clone(),
            #[cfg(target_os = "linux")]
            xembed,
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
        // Linux pumps GTK on this tick so StatusNotifier registration and
        // menu events are not stuck behind a 100ms poll. 32ms is one frame
        // at 30fps and stays inside the 16–50ms range the tray needs.
        let wait = if cfg!(target_os = "linux") { 32 } else { 100 };
        cx.background_executor()
            .timer(std::time::Duration::from_millis(wait))
            .await;
    }
}

enum TrayCommand {
    Show,
    Hide,
    Quit,
}

fn poll_tray(cx: &mut App) -> bool {
    #[cfg(target_os = "linux")]
    {
        pump_gtk();
        if XEMBED_ACTIVATE.swap(false, Ordering::SeqCst) && app_is_hidden() {
            show_app(cx);
        }
    }
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
    // The caller (tray poll, menu action, or Dock reopen) already holds the
    // `App` borrow. Ordering a window front notifies GPUI synchronously, and
    // that callback has to borrow `App` itself — doing it here logs
    // "RefCell already borrowed" and, with no window visible yet, AppKit
    // opens an empty dialog. Yield first so this borrow ends.
    let later = cx.to_async();
    cx.foreground_executor()
        .spawn(async move {
            later
                .background_executor()
                .timer(std::time::Duration::from_millis(1))
                .await;
            platform_show();
            later.update(|cx| cx.activate(true));
        })
        .detach();
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

/// Standard document windows only. The status item and its menu are panels at
/// a raised level; ordering those front turns the menu into a second on-screen
/// window (an empty `AXDialog`).
#[cfg(target_os = "macos")]
fn macos_each_app_window(f: impl Fn(&objc2_app_kit::NSWindow)) {
    use objc2_app_kit::NSNormalWindowLevel;
    let Some(app) = macos_app() else {
        return;
    };
    let windows = app.windows();
    for index in 0..windows.count() {
        let window = windows.objectAtIndex(index);
        if window.level() == NSNormalWindowLevel {
            f(&window);
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_hide() {
    use objc2_app_kit::NSApplicationActivationPolicy;
    macos_each_app_window(|window| window.orderOut(None));
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
    // Order the real window front before leaving Accessory. That switch asks
    // the delegate to reopen, and with no visible window AppKit creates a new
    // one. A visible window makes the delegate return without borrowing `App`
    // again (this call is already inside a borrow).
    macos_each_app_window(|window| window.makeKeyAndOrderFront(None));
    let _ = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
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
        macos_each_app_window(|window| window.makeKeyAndOrderFront(None));
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
