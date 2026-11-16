//! Stored global-show hotkey combo language for the shell app.
//!
//! A bare key (`"E"`) means the platform default modifiers: Control+Option on
//! macOS, Super+Shift elsewhere. The setting is `global_hotkey_key`.

use bir_app_api::hotkey_combo::split_combo;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};

/// Build a `global_hotkey` chord from a stored combo. `None` when the key is
/// not one this listener can register.
pub fn build_hotkey(combo: &str) -> Option<HotKey> {
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

fn key_char_to_code(key: &str) -> Option<Code> {
    Some(match key.to_uppercase().as_str() {
        "A" => Code::KeyA,
        "B" => Code::KeyB,
        "C" => Code::KeyC,
        "D" => Code::KeyD,
        "E" => Code::KeyE,
        "F" => Code::KeyF,
        "G" => Code::KeyG,
        "H" => Code::KeyH,
        "I" => Code::KeyI,
        "J" => Code::KeyJ,
        "K" => Code::KeyK,
        "L" => Code::KeyL,
        "M" => Code::KeyM,
        "N" => Code::KeyN,
        "O" => Code::KeyO,
        "P" => Code::KeyP,
        "Q" => Code::KeyQ,
        "R" => Code::KeyR,
        "S" => Code::KeyS,
        "T" => Code::KeyT,
        "U" => Code::KeyU,
        "V" => Code::KeyV,
        "W" => Code::KeyW,
        "X" => Code::KeyX,
        "Y" => Code::KeyY,
        "Z" => Code::KeyZ,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_control_alt_letter_parses() {
        assert!(build_hotkey("control+alt+E").is_some());
    }

    #[test]
    fn a_bare_key_uses_the_platform_default_modifiers() {
        let hotkey = build_hotkey("E").expect("bare key");
        let mods = hotkey.mods;
        if cfg!(target_os = "macos") {
            assert!(mods.contains(Modifiers::CONTROL));
            assert!(mods.contains(Modifiers::ALT));
            assert!(!mods.contains(Modifiers::SUPER));
        } else {
            assert!(mods.contains(Modifiers::SUPER));
            assert!(mods.contains(Modifiers::SHIFT));
        }
    }

    /// The host functions accept a combo by bir-app-api's key list; this
    /// listener must register exactly those keys.
    #[test]
    fn the_app_api_key_list_is_the_listeners() {
        for key in bir_app_api::hotkey_combo::KEYS {
            assert!(key_char_to_code(key).is_some(), "{key}");
        }
        for key in ["F13", "SPACE", "TAB", "-", "NOTAKEY"] {
            assert!(key_char_to_code(key).is_none(), "{key}");
            assert!(!bir_app_api::hotkey_combo::is_registrable(&format!(
                "control+{key}"
            )));
        }
    }

    /// This listener registers Shift and a key with no other modifier; the
    /// shared rule (`is_registrable`) is drawn for it. The NSH app refuses
    /// such a combo when it is set instead.
    #[test]
    fn a_shift_only_combo_registers_with_shift_alone() {
        assert!(bir_app_api::hotkey_combo::is_registrable("shift+E"));
        let hotkey = build_hotkey("shift+E").expect("shift and a letter");
        assert_eq!(hotkey.mods, Modifiers::SHIFT);
        assert_eq!(hotkey.key, Code::KeyE);
    }

    #[test]
    fn an_unknown_key_is_rejected() {
        assert!(build_hotkey("control+notakey").is_none());
        assert!(build_hotkey("").is_none());
    }
}
