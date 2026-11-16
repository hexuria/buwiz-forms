//! The stored global-show hotkey combo language, without the OS listener.
//!
//! A bare key (`"E"`) means the platform default modifiers: Control+Option on
//! macOS, Super+Shift elsewhere. The setting is `global_hotkey_key`. The host
//! maps a combo to its listener's key codes; this module only reads it.

/// What `set_global_hotkey` answers for a combo the listener cannot register;
/// the Settings page shows it as it is.
pub const NOT_REGISTRABLE: &str = "That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.";

/// The keys a stored combo may name (upper case).
pub const KEYS: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "F1",
    "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

/// The modifiers a legacy bare-key binding uses on this platform.
fn default_modifier_tokens() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["control", "alt"]
    } else {
        &["platform", "shift"]
    }
}

/// Split `"control+alt+F12"` into modifier tokens and the key (upper case).
/// A bare key returns the platform default modifiers.
pub fn split_combo(combo: &str) -> Option<(Vec<String>, String)> {
    let parts: Vec<&str> = combo
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let (key, mods) = parts.split_last()?;
    let key = key.to_uppercase();
    if mods.is_empty() {
        return Some((
            default_modifier_tokens()
                .iter()
                .map(|token| (*token).to_string())
                .collect(),
            key,
        ));
    }
    Some((mods.iter().map(|token| (*token).to_string()).collect(), key))
}

/// Whether the host's listener can register `combo`: it splits and names one
/// of [`KEYS`].
pub fn is_registrable(combo: &str) -> bool {
    split_combo(combo).is_some_and(|(_, key)| KEYS.contains(&key.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_control_alt_letter_is_registrable() {
        assert!(is_registrable("control+alt+E"));
        assert!(is_registrable("super+shift+f12"));
    }

    #[test]
    fn a_bare_key_gets_the_platform_default_modifiers() {
        let (mods, key) = split_combo("e").expect("bare key");
        assert_eq!(key, "E");
        assert_eq!(mods, default_modifier_tokens());
    }

    /// The line between the two listeners: the old shell registers Shift and a
    /// key alone (`bir-shell` `hotkey::build_hotkey`), so this shared rule
    /// keeps taking it; the NSH app refuses it where it is set (its door).
    #[test]
    fn a_shift_only_combo_stays_registrable_for_the_old_listener() {
        assert!(is_registrable("shift+E"));
        assert!(is_registrable("Shift+F12"));
    }

    #[test]
    fn an_unknown_key_is_refused() {
        assert!(!is_registrable("control+notakey"));
        assert!(!is_registrable(""));
        assert!(!is_registrable(" + "));
    }
}
