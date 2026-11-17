//! `bir-shell` — runs the buwiz-forms JavaScript UI on gpui-shell.
//!
//! Painted `bir` (`bir-shell`) is untouched; this binary is the migration
//! target tracked in issue #135.

fn main() {
    // Painted `bir` writes `ebirforms.YYYY-MM-DD.log` under the app log
    // directory and the Logs tab tails that file. A stdout-only subscriber
    // leaves the file empty — and a shipped GUI has no console — so the same
    // installer is used here: rolling file, `bir_core` warnings, QuickJS
    // `console`, and panics.
    let developer_mode = bir_core::logging::developer_mode_enabled();
    let _ = bir_core::logging::install(bir_core::logging::InstallOptions {
        developer_mode,
        // The host functions now log from bir-app-api: give it bir_shell's level.
        default_filter: if developer_mode {
            concat_filter(bir_core::logging::shell_filter(true), ",bir_app_api=debug")
        } else {
            concat_filter(bir_core::logging::shell_filter(false), ",bir_app_api=info")
        },
    });

    if let Err(error) = bir_shell::run(bir_shell::app_root()) {
        tracing::error!("bir-shell exited: {error:#}");
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

/// One filter string for the process, built once at start-up.
fn concat_filter(base: &str, extra: &str) -> &'static str {
    Box::leak(format!("{base}{extra}").into_boxed_str())
}
