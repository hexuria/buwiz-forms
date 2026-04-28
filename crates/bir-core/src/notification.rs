/// Send a native system notification.
///
/// On macOS, uses `osascript` with `tell application id` to attribute the
/// notification to our app bundle, showing the eBIRForms icon instead of
/// Script Editor's icon.
///
/// On other platforms, falls back to `notify-rust`.
pub fn send_notification(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        // Use `tell application id "com.goldcoders.bir"` so macOS attributes the
        // notification to our app and displays our icon instead of Script Editor's.
        let safe_title = title.replace('"', "\\\"").replace('\'', "'\\''");
        let safe_body = body.replace('"', "\\\"").replace('\'', "'\\''");
        let script = format!(
            r#"tell application id "com.goldcoders.bir" to display notification "{}" with title "{}""#,
            safe_body, safe_title
        );
        let result = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();

        // If the `tell application id` approach fails (e.g. the app isn't running
        // or isn't scriptable), fall back to a plain display notification which
        // will at least show the message even if with the wrong icon.
        if result.as_ref().map(|o| !o.status.success()).unwrap_or(true) {
            let fallback_script = format!(
                r#"display notification "{}" with title "{}""#,
                safe_body, safe_title
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&fallback_script)
                .spawn();
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = notify_rust::Notification::new()
            .summary(title)
            .body(body)
            .show();
    }
}
