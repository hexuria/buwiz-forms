//! Windows-specific UI integrations for bir-desktop.

use gpui::*;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
pub fn enforce_single_instance() {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::ERROR_ALREADY_EXISTS;
        use windows::Win32::System::Threading::CreateMutexW;
        use windows::core::PCWSTR;

        let name: Vec<u16> = OsStr::new("eBIRForms_Desktop_App_Mutex_Lock")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: `CreateMutexW` is an FFI call into kernel32. The name pointer is
        // valid for the duration of the call (stack-allocated Vec). The handle is
        // intentionally leaked — Windows releases it automatically when the process
        // exits, which is exactly what we want for a single-instance guard.
        let result = unsafe { CreateMutexW(None, false, PCWSTR(name.as_ptr())) };
        match result {
            Err(_) => std::process::exit(0),
            Ok(_handle) => {
                // Check if the mutex already existed (ERROR_ALREADY_EXISTS = 183).
                // We must call GetLastError *after* a successful CreateMutexW.
                use windows::Win32::Foundation::GetLastError;
                // SAFETY: Called immediately after CreateMutexW with no other Win32
                // calls in between, so the thread-local last-error is valid.
                if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                    std::process::exit(0);
                }
                // Intentionally forget the handle — leak it for process lifetime.
                std::mem::forget(_handle);
            }
        }
    }
}

// ── Keybindings ──────────────────────────────────────────────────────────────

/// Register global keybindings using the Windows `ctrl` modifier.
pub fn bind_global_keys(cx: &mut App) {
    use crate::global_actions::*;

    cx.bind_keys([
        KeyBinding::new("ctrl-enter", SubmitCurrentForm, None),
        KeyBinding::new("ctrl-b", ToggleSidebar, None),
        KeyBinding::new("ctrl-shift-b", ToggleSidebarMini, None),
        KeyBinding::new("ctrl-f", FocusSearch, None),
        KeyBinding::new("ctrl-n", CreateProfile, None),
        KeyBinding::new("ctrl-shift-t", ToggleTheme, None),
        KeyBinding::new("ctrl-shift-x", OpenCronTasks, None),
        KeyBinding::new("ctrl-,", OpenSettings, None),
        KeyBinding::new("ctrl-k", OpenCommandPalette, None),
        KeyBinding::new("f1", OpenGlobalDashboard, None),
        KeyBinding::new("alt-f4", QuitApplication, None),
        KeyBinding::new("ctrl-w", CloseWindow, None),
        KeyBinding::new("f11", ToggleFullScreen, None),
        KeyBinding::new("win-m", MinimizeWindow, None),
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ResetZoom, None),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in Windows Explorer using `explorer /select,`.
#[allow(dead_code)]
pub fn reveal_in_file_manager(path: &std::path::Path) {
    let path_str = path.to_string_lossy().replace("/", "\\");
    let mut cmd = std::process::Command::new("explorer");
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let _ = cmd.arg("/select,").arg(path_str).spawn();
}

/// Open a file with the default system application.
pub fn open_in_system(path: &std::path::Path) {
    // Windows ShellExecute treats forward slashes as internet/UNC paths which triggers
    // a "trusted source" security prompt. Force all slashes to backslashes.
    let path_str = path.to_string_lossy().replace("/", "\\");

    // Forcefully remove the Mark of the Web (Zone.Identifier) alternate data stream.
    // This completely unblocks the file and marks it as trusted, avoiding the Windows security prompt
    // for existing files that were copied over with the MotW stream intact.
    let motw_path = format!("{}:Zone.Identifier", path_str);
    let _ = std::fs::remove_file(&motw_path);

    let _ = open::that(path_str);
}

// ── Native Print ─────────────────────────────────────────────────────────────

/// Print a PDF using the Windows ShellExecute "print" verb.
///
/// Falls back to `open::that` if print fails.
pub fn print_pdf(path: &std::path::Path) -> Result<(), &'static str> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        let verb: Vec<u16> = std::ffi::OsStr::new("print")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let file_path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: `ShellExecuteW` is an FFI call into shell32. Both wide-string
        // slices are null-terminated and live for the duration of the call.
        // The hwnd, parameters, and directory arguments are intentionally null
        // (use process default), which is a documented valid usage.
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(file_path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };

        // ShellExecute returns a value > 32 if successful.
        if result.0 as isize <= 32 {
            let _ = open::that(path);
            return Err(
                "Your default PDF viewer (Edge) does not support the 'print' command.\nThe file has been opened instead. Please press Ctrl+P to print.",
            );
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = open::that(path);
        Err("Print command not supported on this platform.")
    }
}

// ── Keyboard Focus Recovery ──────────────────────────────────────────────────

/// Workaround for a robius-authentication bug on Windows.
///
/// After `robius_authentication::Context::authenticate()` returns, the Windows
/// Hello dialog sometimes leaves the Alt key logically pressed, which causes:
///   - Backspace to emit `^H` instead of deleting text
///   - Escape to cycle windows instead of dismissing dialogs
///
/// This function:
///   1. Unconditionally sends a key-up for `VK_MENU` (Alt) so it is never stuck.
///   2. Finds the top-level GPUI window and calls `SetForegroundWindow`
///      so the app regains keyboard input.
///
/// Call this **after** every `robius_authentication::Context::authenticate()` returns.
pub fn reclaim_keyboard_focus() {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            GetAsyncKeyState, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
            keybd_event,
        };
        use windows::Win32::UI::WindowsAndMessaging::{
            EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow,
        };

        // SAFETY: `GetAsyncKeyState` reads keyboard state atomically. The key
        // code VK_MENU is a well-known constant (0x12). No preconditions.
        unsafe {
            if GetAsyncKeyState(VK_MENU.0 as i32) < 0 {
                // SAFETY: `keybd_event` is a legacy but still valid Win32 API.
                // We send a single KEYUP event for VK_MENU with no scan code.
                // No window handle or thread affinity is required.
                keybd_event(
                    VK_MENU.0 as u8,
                    0,
                    KEYBD_EVENT_FLAGS(KEYEVENTF_EXTENDEDKEY.0 | KEYEVENTF_KEYUP.0),
                    0,
                );
            }
        }

        let pid = std::process::id();

        // SAFETY: `enum_callback` is a valid Win32 EnumWindows callback.
        // `pid` is passed as LPARAM (integer) and reinterpreted inside the
        // callback as u32 — the cast is safe because process IDs fit in u32.
        unsafe extern "system" fn enum_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let target_pid = lparam.0 as u32;
            let mut window_pid: u32 = 0;
            // SAFETY: hwnd comes from EnumWindows (always valid). window_pid is
            // a local variable passed by mutable reference.
            unsafe { GetWindowThreadProcessId(hwnd, Some(&mut window_pid)) };

            if window_pid == target_pid
                // SAFETY: hwnd is a valid window handle provided by EnumWindows.
                && unsafe { IsWindowVisible(hwnd) }.as_bool()
            {
                // SAFETY: hwnd belongs to our process and is visible.
                let _ = unsafe { SetForegroundWindow(hwnd) };
                return BOOL(0); // FALSE — stop enumeration
            }
            BOOL(1) // TRUE — continue enumeration
        }

        // SAFETY: `enum_callback` has the correct `unsafe extern "system"` ABI
        // expected by EnumWindows. The LPARAM is our process ID cast to isize.
        unsafe {
            let _ = EnumWindows(Some(enum_callback), LPARAM(pid as isize));
        }
    }
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
pub const MONOSPACE_FONT: &str = "Cascadia Mono";

// ── Dock Management ──────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM};
// `BOOL` moved out of `Win32::Foundation` into core in windows 0.62.
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentProcessId;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SW_HIDE, SW_SHOW, SetForegroundWindow,
    ShowWindow,
};
#[cfg(target_os = "windows")]
use windows::core::BOOL;

// SAFETY: These three functions are Win32 EnumWindows callbacks — the
// `unsafe extern "system"` ABI is an unavoidable requirement of the Win32 API.
// Each function:
//   - Only calls Win32 APIs on `hwnd` values provided by the OS (always valid).
//   - Reads `GetCurrentProcessId()` to filter to our own process windows only.
//   - Returns BOOL(1) to continue enumeration or BOOL(0) to stop.

#[cfg(target_os = "windows")]
unsafe extern "system" fn hide_window_callback(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut pid = 0;
    // SAFETY: hwnd is a valid handle from EnumWindows; pid is a local variable.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == unsafe { GetCurrentProcessId() } {
        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
    BOOL(1) // TRUE to continue enumerating
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn show_window_callback(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut pid = 0;
    // SAFETY: hwnd is a valid handle from EnumWindows; pid is a local variable.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == unsafe { GetCurrentProcessId() } {
        use windows::Win32::UI::WindowsAndMessaging::{SW_RESTORE, SetForegroundWindow};
        // SAFETY: hwnd belongs to our process — safe to show and foreground.
        let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
        let _ = unsafe { SetForegroundWindow(hwnd) };
    }
    BOOL(1)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn toggle_window_callback(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut pid = 0;
    // SAFETY: hwnd is a valid handle from EnumWindows; pid is a local variable.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == unsafe { GetCurrentProcessId() } {
        use windows::Win32::UI::WindowsAndMessaging::{
            IsWindowVisible, SW_RESTORE, SetForegroundWindow,
        };
        // SAFETY: hwnd belongs to our process — safe to query visibility.
        if unsafe { IsWindowVisible(hwnd) }.into() {
            let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
        } else {
            // SAFETY: hwnd belongs to our process — safe to show and foreground.
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
            let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
            let _ = unsafe { SetForegroundWindow(hwnd) };
        }
    }
    BOOL(1)
}

/// Hides the application from the dock/taskbar and explicit tiling managers.
pub fn hide_from_dock() {
    #[cfg(target_os = "windows")]
    // SAFETY: `hide_window_callback` is a correctly-typed Win32 EnumWindows callback.
    unsafe {
        let _ = EnumWindows(Some(hide_window_callback), LPARAM(0));
    }
}

/// Restores the application to the dock/taskbar and brings to foreground.
pub fn show_in_dock() {
    #[cfg(target_os = "windows")]
    // SAFETY: `show_window_callback` is a correctly-typed Win32 EnumWindows callback.
    unsafe {
        let _ = EnumWindows(Some(show_window_callback), LPARAM(0));
    }
}

/// Toggles the application visibility on Windows.
pub fn toggle_app_visibility() {
    #[cfg(target_os = "windows")]
    // SAFETY: `toggle_window_callback` is a correctly-typed Win32 EnumWindows callback.
    unsafe {
        let _ = EnumWindows(Some(toggle_window_callback), LPARAM(0));
    }
}
