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

    #[derive(Debug)]
    struct CapturedEvent {
        level: tracing::Level,
        target: &'static str,
        message: String,
    }

    #[derive(Clone, Default)]
    struct EventCapture {
        events: std::sync::Arc<std::sync::Mutex<Vec<CapturedEvent>>>,
    }

    #[derive(Default)]
    struct EventMessage(String);

    impl tracing::field::Visit for EventMessage {
        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }

    impl<S> tracing_subscriber::Layer<S> for EventCapture
    where
        S: tracing::Subscriber,
    {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _context: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let mut message = EventMessage::default();
            event.record(&mut message);
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(CapturedEvent {
                    level: *event.metadata().level(),
                    target: event.metadata().target(),
                    message: message.0,
                });
        }
    }

    #[test]
    fn a_file_dialog_that_fails_and_work_that_fails_reach_the_log() {
        use crate::bir_module::after_path_prompt;
        use crate::hooks::PathChoice;
        use gpui_shell::host_modules::HostError;
        use tracing_subscriber::layer::SubscriberExt as _;

        // Capture on this thread's own subscriber instead of reading the shared
        // log file. The layer's default interest is `always`, so once this
        // scoped dispatch is registered every later process-wide interest
        // rebuild includes it; the assertions do not depend on winning a
        // rebuild race against another test's dispatcher.
        let capture = EventCapture::default();
        let subscriber = tracing_subscriber::Registry::default().with(capture.clone());
        tracing::subscriber::with_default(subscriber, || {
            let failed = after_path_prompt(
                |answer| {
                    answer(PathChoice::Failed("no file chooser portal 7c1e".into()));
                    true
                },
                |_| unreachable!("nothing was picked"),
            )
            .unwrap();
            assert!(futures::executor::block_on(failed).is_err());

            let refused = after_path_prompt(|_| false, |_| unreachable!("nothing was picked"));
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

        let events = capture
            .events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let warn = |needle: &str| {
            let event = events
                .iter()
                .find(|event| event.message.contains(needle))
                .unwrap_or_else(|| panic!("no event with {needle:?} in {events:?}"));
            assert_eq!(event.level, tracing::Level::WARN, "{event:?}");
            assert_eq!(event.target, "bir_app_api::bir_module", "{event:?}");
        };
        warn("The file dialog could not open: no file chooser portal 7c1e");
        warn("The file dialog could not open: no app is running to open it");
        warn("Working with the file picked failed: disk full 7c1e");
        // A cancel is the person's answer, not a failure.
        assert_eq!(
            events
                .iter()
                .filter(|event| event.message.contains("7c1e"))
                .count(),
            2,
            "{events:?}"
        );
    }
}
