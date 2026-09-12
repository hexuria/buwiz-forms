//! macOS window PNG capture for the agent mailbox.
//!
//! `gpui_agent::capture_window_via_screencapture` spawns `screencapture` onto
//! a hidden `.{stem}.{pid}-n.tmp` path (no `.png` suffix). Spawned from `bir`,
//! that argv often exits 0 and writes nothing — the same `-l <CGWindowID>`
//! from a Terminal shell still works. This host uses `/usr/sbin/screencapture`
//! with a visible `.png` dest, the two-arg `-l` form, and a Screen Recording
//! error that names `bir` (TCC is per-process; a Terminal grant does not cover us).

use std::path::{Path, PathBuf};

use gpui_agent::{DispatchResult, confine_screenshot_path, require_screenshot_path};

pub const SCREENCAPTURE_BIN: &str = "/usr/sbin/screencapture";

/// TCC copy used when the child writes no PNG. Keep "grant Screen Recording to bir"
/// so agents can match it without parsing System Settings prose.
pub const SCREEN_RECORDING_TCC_HINT: &str = "grant Screen Recording to bir (System Settings → Privacy & Security → Screen Recording). \
     A Terminal / shell grant does not cover the bir process";

pub fn screen_recording_unavailable(detail: impl Into<String>) -> String {
    gpui_agent::screenshot_unavailable(format!("{}. {SCREEN_RECORDING_TCC_HINT}.", detail.into()))
}

/// Argv after `/usr/sbin/screencapture`. Two-arg `-l` matches the working
/// shell form; dest must be a `.png` that does not start with `-`.
pub fn screencapture_argv(window_id: u32, dest: &str) -> Result<Vec<String>, String> {
    let dest = require_screenshot_path(Some(dest))?;
    if dest.starts_with('-') {
        return Err(
            "screenshot path must not start with '-' (would look like a screencapture flag)".into(),
        );
    }
    if window_id == 0 {
        return Err(gpui_agent::screenshot_unavailable(
            "refusing screencapture without a CGWindowID (would not be this app window)",
        ));
    }
    if !dest.ends_with(".png") {
        return Err("screenshot capture dest must end with .png".into());
    }
    let name = Path::new(dest)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if name.starts_with('.') {
        return Err("screenshot capture dest must not be a hidden file".into());
    }
    Ok(vec![
        "-l".into(),
        window_id.to_string(),
        "-o".into(),
        "-x".into(),
        dest.into(),
    ])
}

pub fn capture_temp_png(dest: &Path, n: u32) -> PathBuf {
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    parent.join(format!("bir-cap-{}-{n}.png", std::process::id()))
}

/// Viewport capture: confined client `.png` under the screenshot dir.
pub fn capture_window_to_client_path(
    window_id: u32,
    client_path: &str,
) -> Result<DispatchResult, String> {
    let path = require_screenshot_path(Some(client_path))?;
    let dest = confine_screenshot_path(path)?;
    let png = capture_window_png_bytes(window_id)?;
    gpui_agent::atomic_write_png(&dest, &png)?;
    let dest_str = dest
        .to_str()
        .ok_or_else(|| "screenshot path is not utf-8".to_string())?;
    gpui_agent::accept_written_png(dest_str)
}

/// One window tile as PNG bytes (scrolled stitch). Never the SDK hidden `.tmp`.
pub fn capture_window_png_bytes(window_id: u32) -> Result<Vec<u8>, String> {
    let dest = gpui_agent::screenshot_base_dir().join("tile.png");
    let bytes = capture_window_png_to_unique_temp(window_id, &dest)?;
    if !bytes.starts_with(b"\x89PNG") {
        return Err(gpui_agent::screenshot_unavailable(
            "screencapture wrote a non-PNG tile; refusing to stitch it",
        ));
    }
    Ok(bytes)
}

fn capture_window_png_to_unique_temp(window_id: u32, dest: &Path) -> Result<Vec<u8>, String> {
    if let Some(parent) = dest.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    let mut n = 0u32;
    let tmp = loop {
        let candidate = capture_temp_png(dest, n);
        if !candidate.exists() {
            break candidate;
        }
        n = n.checked_add(1).ok_or_else(|| {
            gpui_agent::screenshot_unavailable("could not allocate a capture temp png")
        })?;
        if n > 10_000 {
            return Err(gpui_agent::screenshot_unavailable(
                "could not allocate a capture temp png",
            ));
        }
    };
    let tmp_str = tmp
        .to_str()
        .ok_or_else(|| "screenshot temp path is not utf-8".to_string())?;
    let result = run_screencapture(window_id, tmp_str);
    let bytes = match result {
        Ok(()) => std::fs::read(&tmp).map_err(|err| {
            let _ = std::fs::remove_file(&tmp);
            screen_recording_unavailable(format!("screencapture produced no file ({err})"))
        }),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err)
        }
    };
    let _ = std::fs::remove_file(&tmp);
    bytes
}

#[cfg(target_os = "macos")]
fn run_screencapture(window_id: u32, dest: &str) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let args = screencapture_argv(window_id, dest)?;
    let cwd = Path::new(dest)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = Command::new(SCREENCAPTURE_BIN)
        .args(&args)
        .current_dir(cwd)
        .env("PATH", "/usr/sbin:/usr/bin:/bin")
        .stdin(Stdio::null())
        .output()
        .map_err(|err| screen_recording_unavailable(format!("screencapture exec failed: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        return Err(screen_recording_unavailable(format!(
            "screencapture exited {}{}",
            output.status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(" ({stderr})")
            }
        )));
    }

    if !Path::new(dest).is_file() {
        return Err(screen_recording_unavailable(
            "screencapture produced no file",
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn run_screencapture(_window_id: u32, _dest: &str) -> Result<(), String> {
    Err(gpui_agent::screenshot_unavailable(
        "screencapture -l is macOS-only; this OS has no production GPUI framebuffer export",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_matches_working_shell_form() {
        let args = screencapture_argv(4242, "/tmp/gpui-agent-screenshots/form.png").unwrap();
        assert_eq!(
            args,
            vec![
                "-l",
                "4242",
                "-o",
                "-x",
                "/tmp/gpui-agent-screenshots/form.png"
            ]
        );
        assert_eq!(SCREENCAPTURE_BIN, "/usr/sbin/screencapture");
    }

    #[test]
    fn argv_rejects_dotfile_and_non_png() {
        assert!(screencapture_argv(1, "/tmp/.tile.1-0.tmp").is_err());
        assert!(screencapture_argv(1, "/tmp/tile.tmp").is_err());
        assert!(screencapture_argv(0, "/tmp/tile.png").is_err());
    }

    #[test]
    fn capture_temp_is_visible_png() {
        let dest = Path::new("/tmp/gpui-agent-screenshots/form.png");
        let tmp = capture_temp_png(dest, 0);
        let name = tmp.file_name().unwrap().to_str().unwrap();
        assert!(name.ends_with(".png"), "{name}");
        assert!(!name.starts_with('.'), "{name}");
        assert!(name.starts_with("bir-cap-"), "{name}");
        assert_eq!(tmp.parent(), dest.parent());
    }

    #[test]
    fn tcc_hint_names_bir() {
        let err = screen_recording_unavailable("screencapture produced no file");
        assert!(gpui_agent::is_screenshot_unavailable(&err), "{err}");
        assert!(err.contains("grant Screen Recording to bir"), "{err}");
        assert!(err.contains("Terminal"), "{err}");
    }

    #[test]
    fn sdk_hidden_tmp_is_why_this_host_wraps_capture() {
        let dest = Path::new("/tmp/gpui-agent-screenshots/form.png");
        let sdk = gpui_agent::png_write_temp_path(dest, 0);
        let name = sdk.file_name().unwrap().to_str().unwrap();
        assert!(
            name.starts_with('.'),
            "sdk temp is a hidden file (the Mac bir spawn miss): {name}"
        );
        assert!(
            !name.ends_with(".png"),
            "sdk temp has no .png suffix (screencapture infers format from extension): {name}"
        );
    }
}
