//! Windows-specific UI integrations for bir-desktop.

use gpui::*;

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
        KeyBinding::new("ctrl-t", ToggleTheme, None),
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
        KeyBinding::new("ctrl-e", ToggleEditMode, None),
        KeyBinding::new("ctrl-s", SaveLayout, None),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in Windows Explorer using `explorer /select,`.
#[allow(dead_code)]
pub fn reveal_in_file_manager(path: &std::path::Path) {
    let _ = std::process::Command::new("explorer")
        .arg("/select,")
        .arg(path)
        .spawn();
}

/// Open a file with the default system application.
pub fn open_in_system(path: &std::path::Path) {
    let _ = open::that(path);
}

// ── Native Print ─────────────────────────────────────────────────────────────

/// Print a PDF using the Windows ShellExecute "print" verb.
///
/// Falls back to `open::that` if print fails.
pub fn print_pdf(path: &std::path::Path) {
    let path = path.to_path_buf();

    std::thread::spawn(move || {
        let result = std::process::Command::new("rundll32")
            .arg("mshtml.dll,PrintHTML")
            .arg(&path)
            .spawn();

        if result.is_err() {
            let _ = open::that(&path);
        }
    });
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
pub const MONOSPACE_FONT: &str = "Cascadia Mono";

// ── Dock Management ──────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentProcessId;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, SW_HIDE, SW_SHOW, ShowWindow,
};

#[cfg(target_os = "windows")]
unsafe extern "system" fn hide_window_callback(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() {
        ShowWindow(hwnd, SW_HIDE);
    }
    BOOL(1) // TRUE to continue enumerating
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn show_window_callback(hwnd: HWND, _: LPARAM) -> BOOL {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid));
    if pid == GetCurrentProcessId() {
        ShowWindow(hwnd, SW_SHOW);
    }
    BOOL(1)
}

/// Hides the application from the dock/taskbar and explicit tiling managers.
pub fn hide_from_dock() {
    #[cfg(target_os = "windows")]
    unsafe {
        let _ = EnumWindows(Some(hide_window_callback), LPARAM(0));
    }
}

/// Restores the application to the dock/taskbar.
pub fn show_in_dock() {
    #[cfg(target_os = "windows")]
    unsafe {
        let _ = EnumWindows(Some(show_window_callback), LPARAM(0));
    }
}
