//! Linux-specific UI integrations for bir-desktop.

use gpui::*;

// ── Keybindings ──────────────────────────────────────────────────────────────

/// Register global keybindings using the Linux `ctrl` modifier.
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
        KeyBinding::new("ctrl-q", QuitApplication, None),
        KeyBinding::new("alt-f4", CloseWindow, None),
        KeyBinding::new("ctrl-w", CloseWindow, None),
        KeyBinding::new("f11", ToggleFullScreen, None),
        KeyBinding::new("ctrl-=", ZoomIn, None),
        KeyBinding::new("ctrl--", ZoomOut, None),
        KeyBinding::new("ctrl-0", ResetZoom, None),
        KeyBinding::new("ctrl-e", ToggleEditMode, None),
        KeyBinding::new("ctrl-s", SaveLayout, None),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in the default Linux file manager.
///
/// Attempts to open the parent directory, as there is no universal
/// "select file in manager" command on Linux.
pub fn reveal_in_file_manager(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = open::that(parent);
    }
}

/// Open a file with the default system application.
pub fn open_in_system(path: &std::path::Path) {
    let _ = open::that(path);
}

// ── Native Print ─────────────────────────────────────────────────────────────

/// Print a PDF using `lp` (CUPS), falling back to `open::that`.
pub fn print_pdf(path: &std::path::Path) {
    let path = path.to_path_buf();

    std::thread::spawn(move || {
        let result = std::process::Command::new("lp").arg(&path).spawn();

        if result.is_err() {
            let _ = open::that(&path);
        }
    });
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
pub const MONOSPACE_FONT: &str = "monospace";
