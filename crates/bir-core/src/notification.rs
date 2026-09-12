/// Post a desktop notification from a process that has no window of its own.
///
/// This is the `bir-headless` (and Linux) path. Painted `bir` does **not**
/// enable delivery here: it watches the same alert rows and posts through
/// GPUI's `UNUserNotificationCenter` support, which gives the banner the
/// app's identity, icon, permission prompt and click-to-open. A headless
/// daemon can never be a `.app`, and the deprecated `NSUserNotificationCenter`
/// silently drops posts from anything that is not one, so on macOS it goes
/// through `osascript`, labelled eBIRForms in the subtitle.
pub fn send_notification(title: &str, body: &str) {
    if !delivery_enabled() {
        tracing::debug!(
            title,
            "desktop notification suppressed: delivery not enabled"
        );
        return;
    }
    #[cfg(target_os = "macos")]
    {
        display_via_osascript(title, body);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show();
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

/// Off until `bir-headless` turns it on. Test suites and one-off tools link
/// this crate too, and their cron / alert tests reach `send_notification`
/// with fixture data — which used to post straight into the developer's
/// Notification Center as "1601C 05/99 submitted — Queue Guard Taxpayer".
static DELIVERY_ENABLED: AtomicBool = AtomicBool::new(false);

/// Called once at startup by `bir-headless`.
pub fn enable_desktop_delivery() {
    DELIVERY_ENABLED.store(true, Ordering::SeqCst);
}

pub fn delivery_enabled() -> bool {
    DELIVERY_ENABLED.load(Ordering::SeqCst)
}

#[cfg(target_os = "macos")]
fn display_via_osascript(title: &str, body: &str) -> bool {
    let script = format!(
        "display notification {} with title \"eBIRForms\" subtitle {}",
        applescript_string(body),
        applescript_string(title)
    );
    std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Quote arbitrary text as an AppleScript string literal.
#[cfg(target_os = "macos")]
fn applescript_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Post a desktop notification for an alert, but only when it is *news*.
///
/// The in-app Notifications page only helps someone already looking at the app.
/// This reaches the user when they are not — the case that actually mattered:
/// email confirmation checking stayed broken for a week because nothing ever
/// said so outside a log file.
///
/// Gated on [`AlertRecordOutcome::is_newly_active`] rather than firing per
/// report. The email cron re-reports a broken token every 60 seconds, so
/// notifying each time would be roughly 1,440 banners a day and would train the
/// user to dismiss all of them unread. Callers can pass every `record_alert`
/// result here without gating themselves; the rule lives in one place.
pub fn notify_alert_if_newly_active(
    outcome: crate::db::AlertRecordOutcome,
    title: &str,
    detail: &str,
) {
    if !outcome.is_newly_active() {
        return;
    }
    send_notification(title, &truncate_for_banner(detail, 140));
}

/// Trim a detail string to something a notification banner can show.
///
/// Banners are narrow, and these strings carry arbitrary server text — a full
/// OAuth error becomes an unreadable wall. The complete text stays on the
/// Notifications page.
///
/// Truncation is on a character boundary: slicing by byte index panics when the
/// cut lands mid-character, and a proxy error page or a non-ASCII description is
/// enough to trigger that.
fn truncate_for_banner(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .map(|(i, c)| i + c.len_utf8())
        .take_while(|&i| i <= max)
        .last()
        .unwrap_or(0);
    format!("{}…", &text[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AlertRecordOutcome;

    #[test]
    fn short_detail_is_unchanged() {
        assert_eq!(truncate_for_banner("hello", 140), "hello");
    }

    #[test]
    fn long_detail_is_bounded_and_marked() {
        let out = truncate_for_banner(&"x".repeat(500), 140);
        assert!(out.len() <= 145, "got {}", out.len());
        assert!(out.ends_with('…'));
    }

    /// Byte-index slicing panics mid-character; server text can be non-ASCII.
    #[test]
    fn multibyte_detail_does_not_panic() {
        assert!(!truncate_for_banner(&"日本語テキスト".repeat(50), 140).is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_string("plain"), "\"plain\"");
        assert_eq!(applescript_string("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(applescript_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(applescript_string("line1\nline2"), "\"line1\nline2\"");
    }

    /// Nothing links this crate with delivery on except the two binaries.
    #[test]
    fn delivery_is_off_unless_a_binary_enables_it() {
        assert!(!delivery_enabled());
        send_notification("must not appear", "test suite");
    }

    /// The cron re-reports every 60s. This must post nothing.
    #[test]
    fn a_repeat_report_posts_nothing() {
        notify_alert_if_newly_active(AlertRecordOutcome::StillActive, "must not appear", "detail");
    }
}
