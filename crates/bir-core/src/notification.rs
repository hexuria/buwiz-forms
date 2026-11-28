/// Post a desktop notification from a process that has no window of its own.
///
/// This is the `bir-headless` (and Linux) path. Painted `bir` does **not**
/// enable delivery here: it watches the same alert rows and posts through
/// GPUI's `UNUserNotificationCenter` support, which gives the banner the
/// app's identity, icon, permission prompt and click-to-open.
///
/// On macOS a process with a bundle identifier (a binary inside a `.app`)
/// posts through `UNUserNotificationCenter` too. `osascript` is only the
/// fallback for a bare binary (`cargo run`, a loose `bir-headless`): the
/// notification center aborts the process when there is no bundle, and the
/// App Sandbox blocks `osascript`, which only matters for bundled builds.
fn send_notification(title: &str, body: &str) {
    if !delivery_enabled() {
        tracing::debug!(
            title,
            "desktop notification suppressed: delivery not enabled"
        );
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if macos_native::has_bundle_identifier() {
            macos_native::post(title, body);
        } else {
            display_via_osascript(title, body);
        }
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "android")))]
    {
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show();
    }
    #[cfg(target_os = "android")]
    tracing::debug!(
        title,
        body,
        "desktop notifications are not available on Android"
    );
}

/// The only OS-banner events the app posts. A kind carries no taxpayer data:
/// the banner can never be built from a name, a TIN, a form period, a mailbox
/// or server text. The detailed text stays in the app's Notifications page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    /// A return's send ended. `dry_run` keeps the test-send mark without
    /// naming anyone.
    ReturnSent { dry_run: bool },
    /// BIR's confirmation email arrived.
    ConfirmationArrived,
    /// Anything else that needs the user in the app.
    Attention,
    /// The user asked for the background worker's test notification.
    Heartbeat,
}

/// The fixed `(title, body)` of an OS banner. The body is one of the plan's
/// exact sentences; a dry run leads with the app's test-send sentence instead.
pub(crate) fn banner_text(kind: NotificationKind) -> (&'static str, &'static str) {
    let body = match kind {
        NotificationKind::ReturnSent { dry_run: false } => {
            "A return was sent. Open eBIRForms to see it."
        }
        NotificationKind::ReturnSent { dry_run: true } => {
            "Test send. Nothing reached the BIR. Open eBIRForms to see it."
        }
        NotificationKind::ConfirmationArrived => {
            "BIR's confirmation arrived. Open eBIRForms to see it."
        }
        NotificationKind::Attention => "Something needs your attention in eBIRForms.",
        NotificationKind::Heartbeat => "This is a test notification from eBIRForms.",
    };
    ("eBIRForms", body)
}

/// F9's test seam: every banner kind asked of the OS, delivery or not — the epoch gate's test
/// proves a notice of the old session never reaches even this point.
#[cfg(test)]
pub(crate) static POSTED_KINDS: std::sync::LazyLock<std::sync::Mutex<Vec<NotificationKind>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

/// Post one OS banner. Every banner call site selects a [`NotificationKind`]
/// here rather than assembling text of its own.
pub fn notify(kind: NotificationKind) {
    #[cfg(test)]
    POSTED_KINDS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(kind);
    let (title, body) = banner_text(kind);
    send_notification(title, body);
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

/// `UNUserNotificationCenter` delivery for bundled processes.
#[cfg(target_os = "macos")]
mod macos_native {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSBundle, NSError, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest,
        UNUserNotificationCenter,
    };
    use std::sync::Once;

    /// `UNUserNotificationCenter` raises (and aborts the process) when the
    /// main bundle has no identifier, which is how a bare binary runs.
    pub(super) fn has_bundle_identifier() -> bool {
        objc2::rc::autoreleasepool(|_| NSBundle::mainBundle().bundleIdentifier().is_some())
    }

    /// Ask for permission once per process. macOS shows the prompt only the
    /// first time; afterwards this just reports the stored choice.
    fn request_authorization_once(center: &UNUserNotificationCenter) {
        static REQUESTED: Once = Once::new();
        REQUESTED.call_once(|| {
            let completion = RcBlock::new(|granted: Bool, error: *mut NSError| {
                // SAFETY: when non-null, `error` is a valid NSError for the
                // duration of the callback.
                if let Some(error) = unsafe { error.as_ref() } {
                    tracing::warn!(
                        "notification authorization failed: {}",
                        error.localizedDescription()
                    );
                } else if !granted.as_bool() {
                    tracing::info!("notification authorization denied");
                }
            });
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &completion,
            );
        });
    }

    pub(super) fn post(title: &str, body: &str) {
        // Callers include tokio worker threads, which have no autorelease
        // pool of their own.
        objc2::rc::autoreleasepool(|_| {
            let center = UNUserNotificationCenter::currentNotificationCenter();
            request_authorization_once(&center);

            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(title));
            content.setBody(&NSString::from_str(body));
            // A nil trigger delivers immediately.
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&uuid::Uuid::new_v4().to_string()),
                &content,
                None,
            );
            let completion = RcBlock::new(|error: *mut NSError| {
                // SAFETY: as above.
                if let Some(error) = unsafe { error.as_ref() } {
                    tracing::warn!(
                        "desktop notification not delivered: {}",
                        error.localizedDescription()
                    );
                }
            });
            center.addNotificationRequest_withCompletionHandler(&request, Some(&completion));
        });
    }
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
/// result here without gating themselves; the generic attention banner is the
/// only text posted, while the detailed alert stays on the Notifications page.
pub fn notify_alert_if_newly_active(outcome: crate::db::AlertRecordOutcome) {
    if !outcome.is_newly_active() {
        return;
    }
    notify(NotificationKind::Attention);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::AlertRecordOutcome;

    #[cfg(target_os = "macos")]
    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(applescript_string("plain"), "\"plain\"");
        assert_eq!(applescript_string("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(applescript_string("a\\b"), "\"a\\\\b\"");
        assert_eq!(applescript_string("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn banner_texts_are_fixed_and_carry_no_identity() {
        let cases = [
            (
                NotificationKind::ReturnSent { dry_run: false },
                "A return was sent. Open eBIRForms to see it.",
            ),
            (
                NotificationKind::ConfirmationArrived,
                "BIR's confirmation arrived. Open eBIRForms to see it.",
            ),
            (
                NotificationKind::Attention,
                "Something needs your attention in eBIRForms.",
            ),
        ];
        for (kind, body) in cases {
            assert_eq!(banner_text(kind), ("eBIRForms", body));
        }
        assert_eq!(
            banner_text(NotificationKind::Heartbeat),
            ("eBIRForms", "This is a test notification from eBIRForms.")
        );
        assert_eq!(
            banner_text(NotificationKind::ReturnSent { dry_run: true }),
            (
                "eBIRForms",
                "Test send. Nothing reached the BIR. Open eBIRForms to see it."
            )
        );
        for kind in [
            NotificationKind::ReturnSent { dry_run: false },
            NotificationKind::ReturnSent { dry_run: true },
            NotificationKind::ConfirmationArrived,
            NotificationKind::Attention,
            NotificationKind::Heartbeat,
        ] {
            let (title, body) = banner_text(kind);
            for text in [title, body] {
                assert!(!text.contains("Juan Dela Cruz"), "{text}");
                assert!(!text.contains("000-000-000-00000"), "{text}");
                assert!(!text.contains("00000000000000"), "{text}");
            }
        }
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
        notify_alert_if_newly_active(AlertRecordOutcome::StillActive);
    }
}
