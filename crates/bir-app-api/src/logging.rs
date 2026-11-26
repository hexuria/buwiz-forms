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

    #[test]
    fn a_file_dialog_that_fails_and_work_that_fails_reach_the_log() {
        use crate::bir_module::after_path_prompt;
        use crate::hooks::PathChoice;
        use gpui_shell::host_modules::HostError;

        let dir = std::env::temp_dir().join(format!(
            "bir-dialog-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // The app's own default filter (nsh/src/door.rs): bir_app_api at info.
        let options = InstallOptions {
            developer_mode: false,
            default_filter: "nsh_app_ebirforms=info,bir_app_api=info,bir_core=info,panic=error",
        };
        temp_env::with_vars(
            [("BIR_LOG_DIR", Some(dir.as_os_str())), ("RUST_LOG", None)],
            || {
                with_thread_subscriber(options, || {
                    let failed = after_path_prompt(
                        |answer| {
                            answer(PathChoice::Failed("no file chooser portal 7c1e".into()));
                            true
                        },
                        |_| unreachable!("nothing was picked"),
                    )
                    .unwrap();
                    assert!(futures::executor::block_on(failed).is_err());

                    let refused =
                        after_path_prompt(|_| false, |_| unreachable!("nothing was picked"));
                    assert!(refused.is_err());

                    let work = after_path_prompt(
                        |answer| {
                            answer(PathChoice::Picked("/nowhere/export.zip".into()));
                            true
                        },
                        |_| Err(HostError::new("disk full 7c1e")),
                    )
                    .unwrap();
                    assert!(futures::executor::block_on(work).is_err());

                    let cancelled = after_path_prompt(
                        |answer| {
                            answer(PathChoice::Cancelled);
                            true
                        },
                        |_| unreachable!("nothing was picked"),
                    )
                    .unwrap();
                    assert!(futures::executor::block_on(cancelled).is_ok());
                });

                let tail = crate::bir_module::job_log_tail().expect("log tail");
                let json = crate::bir_module::to_json(&tail);
                let content = json["content"].as_str().unwrap_or("");
                let warn = |needle: &str| {
                    let line = content
                        .lines()
                        .find(|line| line.contains(needle))
                        .unwrap_or_else(|| panic!("no line with {needle:?} in {content}"));
                    assert_eq!(log_line_level(line), LogLineLevel::Warn, "{line}");
                };
                warn("The file dialog could not open: no file chooser portal 7c1e");
                warn("The file dialog could not open: no app is running to open it");
                warn("Working with the file picked failed: disk full 7c1e");
                // A cancel is the person's answer, not a failure.
                assert_eq!(content.matches("7c1e").count(), 2, "{content}");
            },
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
