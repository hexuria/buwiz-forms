/// Send a native system notification.
///
/// On macOS, we first register our bundle identifier with the notification
/// system so that macOS attributes the notification to eBIRForms and
/// displays the correct app icon.
pub fn send_notification(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        use std::sync::Once;
        static SET_APP: Once = Once::new();
        SET_APP.call_once(|| {
            let _ = mac_notification_sys::set_application("dev.goldcoders.bir");
        });
    }

    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show();
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

    /// The cron re-reports every 60s. This must post nothing.
    #[test]
    fn a_repeat_report_posts_nothing() {
        notify_alert_if_newly_active(AlertRecordOutcome::StillActive, "must not appear", "detail");
    }
}
