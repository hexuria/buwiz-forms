//! macOS-specific UI integrations for bir-desktop.

use gpui::*;

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
        KeyBinding::new("cmd-e", ToggleEditMode, None),
        KeyBinding::new("cmd-s", SaveLayout, None),
        KeyBinding::new("cmd-n", EditorNewBox, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-enter", EditorRenameField, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-c", EditorSetCharCount, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-d", EditorDuplicateBox, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "cmd-backspace",
            EditorDeleteBox,
            Some("PdfLayoutEditorView"),
        ),
        KeyBinding::new("cmd-f", EditorFocusSearch, Some("PdfLayoutEditorView")),
        KeyBinding::new("escape", EditorEscape, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-[", EditorPrevField, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-]", EditorNextField, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-1", EditorSelectBox1, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-2", EditorSelectBox2, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-3", EditorSelectBox3, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-4", EditorSelectBox4, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-5", EditorSelectBox5, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-6", EditorSelectBox6, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-7", EditorSelectBox7, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-8", EditorSelectBox8, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-9", EditorSelectBox9, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "cmd-shift-0",
            EditorSelectLastBox,
            Some("PdfLayoutEditorView"),
        ),
        KeyBinding::new("cmd-t", EditorCycleType, Some("PdfLayoutEditorView")),
        KeyBinding::new(
            "cmd-shift-d",
            EditorToggleDirection,
            Some("PdfLayoutEditorView"),
        ),
    ]);
}

// ── File Operations ──────────────────────────────────────────────────────────

/// Reveal a file in Finder using `open -R`.
pub fn reveal_in_file_manager(path: &std::path::Path) {
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
pub fn print_pdf(path: &std::path::Path) {
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
}

// ── Typography ───────────────────────────────────────────────────────────────

/// The platform's preferred monospace font family.
pub const MONOSPACE_FONT: &str = ".SF NS Mono";
