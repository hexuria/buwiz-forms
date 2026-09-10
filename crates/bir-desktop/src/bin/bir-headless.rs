//! Headless BIR agent daemon. No GPU window. Speaks protocol v1 over TCP.
//!
//! Requires `--features agent` and `GPUI_AGENT=1` (same `from_env` as painted
//! `bir`). Opens `bir_core::db::app_database_path()` — default
//! `platform::data_dir()/bir_data.db` with the same SQLCipher key as the GUI.

fn main() -> std::process::ExitCode {
    bir_desktop::agent::headless::run()
}
