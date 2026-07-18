//! Platform-specific UI integrations (keybindings, file operations, printing).
//!
//! This module hides all `#[cfg(target_os)]` gating behind a clean public API.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

use std::path::{Path, PathBuf};

fn resource_candidates(current_exe: &Path, name: &str) -> Vec<PathBuf> {
    let Some(parent) = current_exe.parent() else {
        return Vec::new();
    };

    let mut candidates = vec![
        // Linux release / Windows install / local run if assets were copied.
        parent.join(name),
        // macOS .app bundle (Resources is a sibling of MacOS).
        parent.join("../Resources").join(name),
    ];

    // Cargo can place the executable either directly in target/{debug,release}
    // or one level deeper (for example target/<triple>/{debug,release}). Keep
    // both workspace-root fallbacks so both layouts work without relying on CWD.
    candidates.push(parent.join("../..").join(name));
    candidates.push(parent.join("../../..").join(name));

    // Distribution packages install immutable application resources here.
    // Check this after Cargo layouts so a developer run cannot silently pick
    // stale resources from an older system installation.
    #[cfg(target_os = "linux")]
    candidates.push(PathBuf::from("/usr/share/ebirforms").join(name));
    candidates
}

/// Dynamically locates application resources across local dev, macOS bundles, and Linux tarballs.
pub fn find_resource_dir(name: &str) -> PathBuf {
    let current_exe = std::env::current_exe().unwrap_or_default();
    for candidate in resource_candidates(&current_exe, name) {
        if candidate.exists() {
            return candidate.canonicalize().unwrap_or(candidate);
        }
    }

    // Fallback to CWD
    std::env::current_dir().unwrap_or_default().join(name)
}

// Re-export common functions
pub use hide_from_dock;
pub use show_in_dock;

// ── Global Hotkey Helpers ────────────────────────────────────────────────────

/// The default key character for the global toggle hotkey on each platform.
pub fn default_hotkey_key() -> &'static str {
    // Ctrl+Option+E on macOS, Win+Shift+E on Windows, Super+Shift+E on Linux
    "E"
}

/// Human-readable modifier labels for the platform's default global hotkey
/// combo. Used as the fallback display for legacy bare-key bindings.
/// Returns a slice of label strings (e.g. `["Ctrl", "Option"]` on macOS).
pub fn hotkey_modifier_labels() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["Ctrl", "Option"]
    } else if cfg!(target_os = "windows") {
        &["Win", "Shift"]
    } else {
        &["Super", "Shift"]
    }
}

/// The default modifier tokens (in stored-combo form) for this platform.
fn default_modifier_tokens() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["control", "alt"]
    } else {
        &["platform", "shift"]
    }
}

/// A human-readable label for one stored modifier token on this platform.
fn modifier_label(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "control" | "ctrl" => Some("Ctrl"),
        "alt" | "option" | "opt" => Some(if cfg!(target_os = "macos") {
            "Option"
        } else {
            "Alt"
        }),
        "shift" => Some("Shift"),
        "platform" | "super" | "cmd" | "meta" | "win" => Some(if cfg!(target_os = "macos") {
            "Cmd"
        } else if cfg!(target_os = "windows") {
            "Win"
        } else {
            "Super"
        }),
        _ => None,
    }
}

/// Split a stored combo ("control+alt+F12") into `(modifier tokens, key)`.
/// A bare key ("F12") returns the platform default modifier tokens.
fn split_combo(combo: &str) -> Option<(Vec<String>, String)> {
    let parts: Vec<&str> = combo
        .split('+')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let (key, mods) = parts.split_last()?;
    let key = key.to_uppercase();
    if mods.is_empty() {
        // Legacy bare-key binding: apply the platform default modifiers.
        return Some((
            default_modifier_tokens()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            key,
        ));
    }
    Some((mods.iter().map(|s| s.to_string()).collect(), key))
}

/// Parse a stored combo into display modifier labels and the key label,
/// e.g. `"control+alt+F12"` → `(["Ctrl", "Option"], "F12")`.
pub fn parse_hotkey_display(combo: &str) -> Option<(Vec<&'static str>, String)> {
    let (mods, key) = split_combo(combo)?;
    let labels: Vec<&'static str> = mods.iter().filter_map(|m| modifier_label(m)).collect();
    Some((labels, key))
}

/// Build a stored-combo string from the pressed modifiers and key.
/// Returns `None` when no modifier is held (a global hotkey needs one).
pub fn build_hotkey_combo(
    control: bool,
    alt: bool,
    shift: bool,
    platform: bool,
    key: &str,
) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    if control {
        parts.push("control");
    }
    if alt {
        parts.push("alt");
    }
    if shift {
        parts.push("shift");
    }
    if platform {
        parts.push("platform");
    }
    if parts.is_empty() {
        return None;
    }
    Some(format!("{}+{}", parts.join("+"), key.to_uppercase()))
}

/// Build a `global_hotkey::hotkey::HotKey` from a stored combo string.
/// A bare key falls back to the platform default modifiers (legacy bindings).
pub fn build_hotkey(combo: &str) -> Option<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{HotKey, Modifiers};

    let (mod_tokens, key) = split_combo(combo)?;
    let code = key_char_to_code(&key)?;

    let mut modifiers = Modifiers::empty();
    for token in &mod_tokens {
        match token.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => modifiers |= Modifiers::CONTROL,
            "alt" | "option" | "opt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            "platform" | "super" | "cmd" | "meta" | "win" => modifiers |= Modifiers::SUPER,
            _ => {}
        }
    }
    let modifiers = if modifiers.is_empty() {
        None
    } else {
        Some(modifiers)
    };
    Some(HotKey::new(modifiers, code))
}

/// Map a single-character key string to a `global_hotkey::hotkey::Code`.
fn key_char_to_code(key: &str) -> Option<global_hotkey::hotkey::Code> {
    use global_hotkey::hotkey::Code;

    let upper = key.to_uppercase();
    match upper.as_str() {
        "A" => Some(Code::KeyA),
        "B" => Some(Code::KeyB),
        "C" => Some(Code::KeyC),
        "D" => Some(Code::KeyD),
        "E" => Some(Code::KeyE),
        "F" => Some(Code::KeyF),
        "G" => Some(Code::KeyG),
        "H" => Some(Code::KeyH),
        "I" => Some(Code::KeyI),
        "J" => Some(Code::KeyJ),
        "K" => Some(Code::KeyK),
        "L" => Some(Code::KeyL),
        "M" => Some(Code::KeyM),
        "N" => Some(Code::KeyN),
        "O" => Some(Code::KeyO),
        "P" => Some(Code::KeyP),
        "Q" => Some(Code::KeyQ),
        "R" => Some(Code::KeyR),
        "S" => Some(Code::KeyS),
        "T" => Some(Code::KeyT),
        "U" => Some(Code::KeyU),
        "V" => Some(Code::KeyV),
        "W" => Some(Code::KeyW),
        "X" => Some(Code::KeyX),
        "Y" => Some(Code::KeyY),
        "Z" => Some(Code::KeyZ),
        "0" => Some(Code::Digit0),
        "1" => Some(Code::Digit1),
        "2" => Some(Code::Digit2),
        "3" => Some(Code::Digit3),
        "4" => Some(Code::Digit4),
        "5" => Some(Code::Digit5),
        "6" => Some(Code::Digit6),
        "7" => Some(Code::Digit7),
        "8" => Some(Code::Digit8),
        "9" => Some(Code::Digit9),
        "F1" => Some(Code::F1),
        "F2" => Some(Code::F2),
        "F3" => Some(Code::F3),
        "F4" => Some(Code::F4),
        "F5" => Some(Code::F5),
        "F6" => Some(Code::F6),
        "F7" => Some(Code::F7),
        "F8" => Some(Code::F8),
        "F9" => Some(Code::F9),
        "F10" => Some(Code::F10),
        "F11" => Some(Code::F11),
        "F12" => Some(Code::F12),
        _ => None,
    }
}

#[cfg(test)]
mod resource_tests {
    use super::*;

    #[test]
    fn cargo_resource_candidates_cover_host_and_target_triple_layouts() {
        let host_candidates =
            resource_candidates(Path::new("/workspace/target/debug/ebirforms"), "assets");
        assert!(host_candidates.contains(&PathBuf::from("/workspace/target/debug/../../assets")));

        let target_triple_candidates = resource_candidates(
            Path::new("/workspace/target/x86_64-unknown-linux-gnu/debug/ebirforms"),
            "assets",
        );
        assert!(target_triple_candidates.contains(&PathBuf::from(
            "/workspace/target/x86_64-unknown-linux-gnu/debug/../../../assets"
        )));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_resource_candidates_include_distribution_share_directory() {
        let candidates = resource_candidates(Path::new("/usr/bin/ebirforms"), "assets");
        assert!(candidates.contains(&PathBuf::from("/usr/share/ebirforms/assets")));
    }
}
