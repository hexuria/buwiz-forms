/// Send a native system notification.
pub fn send_notification(title: &str, body: &str) {
    // Attempt to use native NSUserNotification via notify-rust.
    // When running inside a macOS .app bundle, this automatically
    // inherits the app's Info.plist and displays the correct icon
    // without requiring any AppleScript automation permissions.
    let _ = notify_rust::Notification::new()
        .summary(title)
        .body(body)
        .show();
}
