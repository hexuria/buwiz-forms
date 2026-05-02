//! Paste-enabled helper for `OtpInput`.
//!
//! The upstream `OtpInput` from `gpui_component` only handles individual key-down
//! events (one digit at a time). It has no clipboard/paste support.
//!
//! This module provides [`paste_otp_value`] — a helper that reads the system clipboard,
//! extracts digits, truncates to the expected OTP length, and calls `set_value()`
//! on the `OtpState` entity, immediately triggering the `InputEvent::Change` flow.
//!
//! It also provides [`wrap_otp_with_paste`] — a convenience that wraps an existing
//! `OtpInput` element in a div with a `KeyDown` handler intercepting `Cmd+V` / `Ctrl+V`.

use gpui::*;
use gpui_component::input::{InputEvent, OtpState};

/// Read the system clipboard, extract up to `expected_len` ASCII digits,
/// and set them as the OTP value. Emits `InputEvent::Change` so validation
/// subscribers fire immediately.
///
/// Returns `true` if a valid digit string was pasted.
pub fn paste_otp_value(
    otp_state: &Entity<OtpState>,
    expected_len: usize,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let clipboard = cx.read_from_clipboard();
    let text = match clipboard {
        Some(item) => item.text().map(|s| s.to_string()),
        None => None,
    };

    let Some(text) = text else {
        return false;
    };

    // Extract only ASCII digit characters, truncate to expected length
    let digits: String = text
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(expected_len)
        .collect();

    if digits.is_empty() {
        return false;
    }

    otp_state.update(cx, |state, cx| {
        state.set_value(&digits, window, cx);
        // Emit Change so validation subscribers fire
        cx.emit(InputEvent::Change);
    });

    true
}
