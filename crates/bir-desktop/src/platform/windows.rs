//! Windows-specific UI integrations for bir-desktop.

use gpui::*;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
pub fn enforce_single_instance() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateMutexW(
            lpMutexAttributes: *mut std::ffi::c_void,
            bInitialOwner: i32,
            lpName: *const u16,
        ) -> *mut std::ffi::c_void;
        fn GetLastError() -> u32;
    }

    let name: Vec<u16> = OsStr::new("eBIRForms_Desktop_App_Mutex_Lock")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr());
        if handle.is_null() || GetLastError() == 183 /* ERROR_ALREADY_EXISTS */ {
            std::process::exit(0);
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
        KeyBinding::new("ctrl-e", ToggleEditMode, None),
        KeyBinding::new("ctrl-s", SaveLayout, None),
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

        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                hwnd: isize,
                lpOperation: *const u16,
                lpFile: *const u16,
                lpParameters: *const u16,
                lpDirectory: *const u16,
                nShowCmd: i32,
            ) -> isize;
        }

        let verb: Vec<u16> = std::ffi::OsStr::new("print")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let file_path: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        // SW_HIDE = 0, SW_SHOWNORMAL = 1
        let result = unsafe {
            ShellExecuteW(
                0,
                verb.as_ptr(),
                file_path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1, // SW_SHOWNORMAL
            )
        };

        // ShellExecute returns a value > 32 if successful.
        if result <= 32 {
            let _ = open::that(path);
            return Err("Your default PDF viewer (Edge) does not support the 'print' command.\nThe file has been opened instead. Please press Ctrl+P to print.");
        }
        
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = open::that(path);
        Err("Print command not supported on this platform.")
    }
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
