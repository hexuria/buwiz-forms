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
            let _ = mac_notification_sys::set_application("com.goldcoders.bir");
        });
    }

    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show();
}
