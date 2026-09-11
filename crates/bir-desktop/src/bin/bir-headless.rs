//! Headless BIR agent daemon. No GPU window. Speaks protocol v2 over TCP.
//!
//! Requires `--features agent`, `GPUI_AGENT=1`, and `GPUI_AGENT_TOKEN`
//! (same `from_env` as painted `bir`; `GPUI_AGENT_INSECURE_NO_TOKEN=1` is
//! demo-only). Opens `bir_core::db::app_database_path()` — default
//! `platform::data_dir()/bir_data.db` with the same SQLCipher key as the GUI.

fn main() -> std::process::ExitCode {
    bir_desktop::agent::headless::run()
}
