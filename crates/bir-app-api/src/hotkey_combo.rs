//! The stored global-show hotkey combo language, without the OS listener.
//!
//! A bare key (`"E"`) means the platform default modifiers: Control+Option on
//! macOS, Super+Shift elsewhere. The setting is `global_hotkey_key`. The host
//! maps a combo to its listener's key codes; this module only reads it.

/// What `set_global_hotkey` answers for a combo the listener cannot register;
/// the Settings page shows it as it is.
pub const NOT_REGISTRABLE: &str = "That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.";

/// The keys a stored combo may name (upper case), and the keys the old shell's listener registers
/// (`bir-shell` `hotkey::build_hotkey` maps exactly these, and a test there says so). [`is_registrable`]
/// is drawn for that listener, so it accepts these and nothing else. A word of [`NSH_KEYS`] is not here:
/// the old shell cannot register it and must not be told it can.
pub const KEYS: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "F1",
    "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12",
];

/// The keys only the NSH app's listener registers, as stored words (upper case, never the character: `+` is the
/// separator). Every one is a key `nsh/keymap` can bind (so the shortcut recorder can hear it) and
/// `GlobalShortcut.register` takes on macOS, Windows and Linux (it was registered in a real macOS window; macOS
/// keyboards end at F20, so F21 to F24 are left out).
///
/// | stored word | key | stored word | key |
/// |---|---|---|---|
/// | `SPACE` | space bar | `MINUS` | `-` |
/// | `UP` `DOWN` `LEFT` `RIGHT` | arrow keys | `EQUAL` | `=` |
/// | `HOME` `END` | Home, End | `LEFTBRACKET` `RIGHTBRACKET` | `[` `]` |
/// | `PAGEUP` `PAGEDOWN` | Page Up, Page Down | `BACKSLASH` | `\` |
/// | `F13` to `F20` | function keys | `SEMICOLON` `QUOTE` | `;` `'` |
/// | | | `COMMA` `PERIOD` `SLASH` | `,` `.` `/` |
/// | | | `BACKTICK` | the key above Tab |
///
/// They are **not** in [`KEYS`] and [`is_registrable`] refuses them, so the old shell keeps refusing them in
/// `set_global_hotkey`. The NSH app's hooks say which a host registers
/// ([`crate::hooks::Hooks::global_hotkey_registrable`]); `nsh/src/desktop.rs` `hotkey_shortcut` maps a word to
/// the keystroke `nsh/keymap` spells, and `nsh/lib/settings_model.ts` `HOTKEY_NSH_KEYS` is the same list on the page.
/// A combo stored with one of them, met by the old shell (a database moved between the two apps), is not
/// registered there: `build_hotkey` answers `None`, the tray logs "invalid global hotkey combo" and goes on, and the
/// old Settings page lists the stored text as a choice of its own.
pub const NSH_KEYS: &[&str] = &[
    "SPACE",
    "UP",
    "DOWN",
    "LEFT",
    "RIGHT",
    "HOME",
    "END",
    "PAGEUP",
    "PAGEDOWN",
    "MINUS",
    "EQUAL",
    "LEFTBRACKET",
    "RIGHTBRACKET",
    "BACKSLASH",
    "SEMICOLON",
    "QUOTE",
    "COMMA",
    "PERIOD",
    "SLASH",
    "BACKTICK",
    "F13",
    "F14",
    "F15",
    "F16",
    "F17",
    "F18",
    "F19",
    "F20",
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

    /// The line the old shell's listener draws: the words only the NSH app registers are neither in `KEYS` nor
    /// registrable by the shared rule, however they are spelled, so `set_global_hotkey` keeps refusing them there.
    #[test]
    fn the_nsh_only_keys_stay_outside_the_old_listeners_rule() {
        assert_eq!(NSH_KEYS.len(), 28);
        for key in NSH_KEYS {
            assert!(!KEYS.contains(key), "{key} is the old listener's too");
            assert!(
                key.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()),
                "{key} is a word in upper case (never the character: + is the separator)"
            );
            for combo in [
                format!("control+alt+{key}"),
                format!("control+alt+{}", key.to_ascii_lowercase()),
                format!("super+shift+{key}"),
                (*key).to_owned(),
            ] {
                assert!(!is_registrable(&combo), "{combo}");
            }
        }
        let mut sorted = NSH_KEYS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), NSH_KEYS.len(), "no word twice");
    }

    /// A stored combo with one of the NSH words still splits into modifiers and a key (the word upper case), as
    /// every stored combo does: only the key list says what registers.
    #[test]
    fn a_nsh_word_splits_like_any_other_key() {
        let (mods, key) = split_combo("control+alt+space").expect("splits");
        assert_eq!(mods, ["control", "alt"]);
        assert_eq!(key, "SPACE");
        let (mods, key) = split_combo("control+leftbracket").expect("splits");
        assert_eq!(mods, ["control"]);
        assert_eq!(key, "LEFTBRACKET");
    }

    #[test]
    fn an_unknown_key_is_refused() {
        assert!(!is_registrable("control+notakey"));
        assert!(!is_registrable(""));
        assert!(!is_registrable(" + "));
    }
}
