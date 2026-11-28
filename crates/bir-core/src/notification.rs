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

use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// I1: one notifier, scoped to the open room, withdrawn at sign-out and close.
// ---------------------------------------------------------------------------

tokio::task_local! {
    /// The room a send in flight belongs to, scoped at the send's spawn beside
    /// SEND_CONTEXT. The submission notices read it to post through the room gate.
    pub static SEND_ROOM: String
}

/// A send's notice: through the room gate when the send carries its room
/// (the always case in production), unscoped only where no room exists.
pub fn notify_send(title: &str, body: &str) {
    match SEND_ROOM.try_with(|room| room.clone()) {
        Ok(room) => notify(&room, title, body),
        Err(_) => send_notification(title, body),
    }
}

use std::sync::RwLock;

type OpenRoomCheck = Box<dyn Fn(&str) -> bool + Send + Sync>;

/// Filled by the painted session machine (bir-app-api): answers whether
/// `room_id` is THE open room right now. Outside a session (headless, tests,
/// `bir` serving one database) nothing is registered and every notify posts,
/// the single-room behavior these processes already had.
static OPEN_ROOM_CHECK: RwLock<Option<OpenRoomCheck>> = RwLock::new(None);

/// The session machine's wiring, once at startup (and reset by tests).
pub fn set_open_room_check(check: Option<OpenRoomCheck>) {
    if let Ok(mut slot) = OPEN_ROOM_CHECK.write() {
        *slot = check;
    }
}

/// Whether a banner for `room_id` may post: the room is the open one, or no
/// session machine is in play. The unit of the seam's tests.
fn may_post(room_id: &str) -> bool {
    match OPEN_ROOM_CHECK
        .read()
        .ok()
        .and_then(|slot| slot.as_ref().map(|check| check(room_id)))
    {
        Some(open) => open,
        None => true,
    }
}

/// Post the banner of an event that happened in `room_id` — only while that
/// room is the open one. A closed room's event stays in its own alert rows
/// (the callers' `record_alert` writes) and posts nothing.
pub fn notify(room_id: &str, title: &str, body: &str) {
    if !may_post(room_id) {
        tracing::debug!("desktop notification suppressed: not the open room");
        return;
    }
    send_notification(title, body);
}

/// Withdraw every banner this process has delivered — at sign-out and at room
/// close, so nothing about one account's sends stays on the screen of the
/// next. Per platform:
///
/// - macOS / iOS: `UNUserNotificationCenter.removeAllDeliveredNotifications`
///   (the notification center owns the delivered list; a bare binary that
///   posts through the osascript fallback cannot withdraw — its banners are
///   attributed to Script Editor, not the app, and are a dev-run-only case);
/// - other platforms: documented in the release notes of the rooms release.
pub fn withdraw_delivered() {
    #[cfg(target_os = "macos")]
    {
        if macos_native::has_bundle_identifier() {
            macos_native::withdraw_all();
        }
    }
    if let Ok(slot) = WITHDRAW_TEST_HOOK.read()
        && let Some(hook) = slot.as_ref()
    {
        hook();
    }
}

/// Tests observe the withdrawal through this hook; production never sets it.
#[doc(hidden)]
pub static WITHDRAW_TEST_HOOK: RwLock<Option<Box<dyn Fn() + Send + Sync>>> = RwLock::new(None);

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

    /// Empty the delivered list of the notification center: every banner this
    /// app posted disappears, clicked or not.
    pub(super) fn withdraw_all() {
        objc2::rc::autoreleasepool(|_| {
            UNUserNotificationCenter::currentNotificationCenter().removeAllDeliveredNotifications();
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

    /// I1 through the seam: a banner of the open room posts, a closed room's is suppressed,
    /// and with no machine registered everything posts (the single-room processes).
    #[test]
    fn the_open_room_gate_decides_what_posts() {
        // No machine registered: the single-room behavior.
        set_open_room_check(None);
        assert!(may_post("any-room"));
        set_open_room_check(Some(Box::new(|room| room == "room-open")));
        assert!(may_post("room-open"), "the open room's banner posts");
        assert!(
            !may_post("room-closed"),
            "a closed room's banner is suppressed"
        );
        set_open_room_check(None);
    }

    /// The withdrawal runs the platform path and the seam's hook together.
    #[test]
    fn withdraw_delivered_runs_the_hook() {
        static HIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        HIT.store(false, Ordering::SeqCst);
        *WITHDRAW_TEST_HOOK.write().unwrap() = Some(Box::new(|| {
            HIT.store(true, Ordering::SeqCst);
        }));
        withdraw_delivered();
        assert!(
            HIT.load(Ordering::SeqCst),
            "the withdrawal reached its seam"
        );
        *WITHDRAW_TEST_HOOK.write().unwrap() = None;
    }
}
