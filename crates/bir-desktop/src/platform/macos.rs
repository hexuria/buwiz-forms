//! macOS-specific UI integrations for bir-desktop.

use gpui::*;

// ── Application Lifecycle ────────────────────────────────────────────────────

/// Enforces that only one instance of the application runs at a time.
/// macOS enforces this natively via the application bundle, so this is a no-op.
pub fn enforce_single_instance() {}

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
        KeyBinding::new("cmd-[", EditorPrevField, None),
        KeyBinding::new("cmd-]", EditorNextField, None),
        KeyBinding::new("cmd-.", OpacityIncrease, Some("TypstCalibrationView")),
        KeyBinding::new("cmd-,", OpacityDecrease, Some("TypstCalibrationView")),
        KeyBinding::new("cmd-right", NextPage, Some("TypstCalibrationView")),
        KeyBinding::new("cmd-left", PrevPage, Some("TypstCalibrationView")),
        KeyBinding::new("cmd-right", NextPage, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-left", PrevPage, Some("PdfLayoutEditorView")),
        KeyBinding::new("cmd-1", EditorSelectBox1, Some("PdfLayoutEditorView")),
        KeyBinding::new("up", EditorNudgeUp, Some("PdfLayoutEditorView")),
        KeyBinding::new("down", EditorNudgeDown, Some("PdfLayoutEditorView")),
        KeyBinding::new("left", EditorNudgeLeft, Some("PdfLayoutEditorView")),
        KeyBinding::new("right", EditorNudgeRight, Some("PdfLayoutEditorView")),
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
