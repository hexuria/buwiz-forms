//! Linux-specific UI integrations for bir-desktop.

use gpui::*;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
pub fn enforce_single_instance() {
    // Basic stub for Linux compatibility
}

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
        KeyBinding::new("ctrl-shift-t", ToggleTheme, None),
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
        // ── PDF Layout Editor ────────────────────────────────────────────
        KeyBinding::new("ctrl-n", EditorNewBox, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-enter", EditorRenameField, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-c", EditorSetCharCount, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-d", EditorDuplicateBox, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "ctrl-backspace",
            EditorDeleteBox,
            Some("PdfLayoutEditorView"),
        ),
        KeyBinding::new("ctrl-f", EditorFocusSearch, Some("PdfLayoutEditorView")),
        KeyBinding::new("escape", EditorEscape, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-[", EditorPrevField, None),
        KeyBinding::new("ctrl-]", EditorNextField, None),
        // ── Typst Calibration ────────────────────────────────────────────
        KeyBinding::new("ctrl-.", OpacityIncrease, Some("TypstCalibrationView")),
        KeyBinding::new("ctrl-,", OpacityDecrease, Some("TypstCalibrationView")),
        KeyBinding::new("ctrl-right", NextPage, Some("TypstCalibrationView")),
        KeyBinding::new("ctrl-left", PrevPage, Some("TypstCalibrationView")),
        KeyBinding::new("ctrl-right", NextPage, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-left", PrevPage, Some("PdfLayoutEditorView")),
        // ── Editor Box Selection & Nudging ───────────────────────────────
        KeyBinding::new("ctrl-1", EditorSelectBox1, Some("PdfLayoutEditorView")),
        KeyBinding::new("up", EditorNudgeUp, Some("PdfLayoutEditorView")),
        KeyBinding::new("down", EditorNudgeDown, Some("PdfLayoutEditorView")),
        KeyBinding::new("left", EditorNudgeLeft, Some("PdfLayoutEditorView")),
        KeyBinding::new("right", EditorNudgeRight, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-2", EditorSelectBox2, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-3", EditorSelectBox3, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-4", EditorSelectBox4, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-5", EditorSelectBox5, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-6", EditorSelectBox6, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-7", EditorSelectBox7, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-8", EditorSelectBox8, Some("PdfLayoutEditorView")),
        KeyBinding::new("ctrl-9", EditorSelectBox9, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "ctrl-shift-0",
            EditorSelectLastBox,
            Some("PdfLayoutEditorView"),
        ),
        KeyBinding::new("ctrl-t", EditorCycleType, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "ctrl-shift-d",
            EditorToggleDirection,
            Some("PdfLayoutEditorView"),
        ),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in the default Linux file manager.
///
/// Attempts to open the parent directory, as there is no universal
/// "select file in manager" command on Linux.
#[allow(dead_code)]
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
pub fn print_pdf(path: &std::path::Path) -> Result<(), &'static str> {
    let path = path.to_path_buf();

    std::thread::spawn(move || {
        let result = std::process::Command::new("lp").arg(&path).spawn();

        if result.is_err() {
            let _ = open::that(&path);
        }
    });

    Ok(())
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
pub const MONOSPACE_FONT: &str = "monospace";

// ── Dock Management ──────────────────────────────────────────────────────────

/// Hides the application from the dock/taskbar and tiling managers.
/// On Linux, GPUI does not natively support window hiding, so we attempt to use `xdotool`
/// as a best-effort fallback to unmap the windows belonging to this process.
pub fn hide_from_dock() {
    #[cfg(target_os = "linux")]
    {
        let pid = std::process::id();
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&format!(
                "xdotool search --pid {} | xargs -I {{}} xdotool windowunmap {{}}",
                pid
            ))
            .spawn();
    }
}

/// Restores the application to the dock/taskbar.
pub fn show_in_dock() {
    #[cfg(target_os = "linux")]
    {
        let pid = std::process::id();
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg(&format!(
                "xdotool search --pid {} | xargs -I {{}} xdotool windowmap {{}}",
                pid
            ))
            .spawn();
    }
}

pub fn toggle_app_visibility() {
    // Basic stub for Linux
}
