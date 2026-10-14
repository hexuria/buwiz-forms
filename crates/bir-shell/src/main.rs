//! `bir-shell` — runs the buwiz-forms JavaScript UI on gpui-shell.
//!
//! Painted `bir` (`bir-desktop`) is untouched; this binary is the migration
//! target tracked in issue #135.

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    if let Err(error) = bir_shell::run(bir_shell::app_root()) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
