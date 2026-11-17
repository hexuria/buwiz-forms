//! Screen diagnostics for the Logs tab.
//!
//! QuickJS `console.*` is already emitted by gpui-shell on `gpui_shell::script`.
//! Caught screen errors (a host call that threw, a render that threw) go
//! through [`record_screen_log`] so they hit the same daily file.

use bir_core::logging::redact_secrets;

/// `level` is `error`, `warn` / `warning`, `info`, or `debug`. Anything else
/// is info. The message is scrubbed before it is recorded.
pub fn record_screen_log(level: &str, message: &str) {
    let message = redact_secrets(message);
    if message.trim().is_empty() {
        return;
    }
    match level {
        "debug" => tracing::debug!(target: "bir_shell::script", "{message}"),
        "warn" | "warning" => tracing::warn!(target: "bir_shell::script", "{message}"),
        "error" => tracing::error!(target: "bir_shell::script", "{message}"),
        _ => tracing::info!(target: "bir_shell::script", "{message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_core::logging::{
        InstallOptions, LogLineLevel, log_line_level, shell_filter, with_thread_subscriber,
    };

    #[test]
    fn screen_errors_core_warnings_and_panics_reach_the_log_the_logs_tab_reads() {
        let dir = std::env::temp_dir().join(format!(
            "bir-shell-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let options = InstallOptions {
            developer_mode: false,
            default_filter: shell_filter(false),
        };
        temp_env::with_vars(
            [("BIR_LOG_DIR", Some(dir.as_os_str())), ("RUST_LOG", None)],
            || {
                with_thread_subscriber(options, || {
                    tracing::warn!(
                        target: "bir_core::email",
                        "IMAP login failed for inbox@example.com password=s3cret-token"
                    );
                    record_screen_log(
                        "error",
                        "screen: inbox verify failed token=ya29.SUPERSECRET",
                    );
                    record_screen_log("debug", "hidden-script-debug-9f3a");
                    let previous = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|info| {
                        bir_core::logging::report_panic(info);
                    }));
                    let panicked = std::panic::catch_unwind(|| {
                        panic!("probe-panic-9f3a password=panic-secret");
                    });
                    std::panic::set_hook(previous);
                    assert!(panicked.is_err());
                });

                let tail = crate::bir_module::job_log_tail().expect("log tail");
                let json = crate::bir_module::to_json(&tail);
                let content = json["content"].as_str().unwrap_or("");
                let file_name = json["file_name"].as_str().unwrap_or("");
                assert!(
                    file_name.starts_with("ebirforms.") && file_name.ends_with(".log"),
                    "{file_name}"
                );
                assert!(content.contains("IMAP login failed"), "{content}");
                assert!(content.contains("screen: inbox verify failed"), "{content}");
                assert!(content.contains("probe-panic-9f3a"), "{content}");
                assert!(!content.contains("s3cret-token"), "{content}");
                assert!(!content.contains("ya29.SUPERSECRET"), "{content}");
                assert!(!content.contains("panic-secret"), "{content}");
                assert!(!content.contains("hidden-script-debug-9f3a"), "{content}");
                assert!(json["lines"].as_u64().unwrap_or(0) >= 3, "{json}");
                assert_eq!(
                    log_line_level(
                        content
                            .lines()
                            .find(|line| line.contains("inbox verify failed"))
                            .unwrap()
                    ),
                    LogLineLevel::Error
                );
                assert_eq!(
                    log_line_level(
                        content
                            .lines()
                            .find(|line| line.contains("IMAP login failed"))
                            .unwrap()
                    ),
                    LogLineLevel::Warn
                );
            },
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
