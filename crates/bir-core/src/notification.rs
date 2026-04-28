pub fn send_notification(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        // On macOS, background daemons inside an .app bundle often get silently blocked
        // from using NSUserNotificationCenter unless they jump through hoops.
        // Using osascript is a reliable bypass that guarantees delivery.
        let safe_title = title.replace('"', "\\\"");
        let safe_body = body.replace('"', "\\\"");
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            safe_body, safe_title
        );
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(script)
            .spawn();
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show();
    }
}
