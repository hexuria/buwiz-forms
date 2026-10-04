//! Headless eBIRForms agent CLI (`bir`).
//!
//! Same daemon as the in-repo `bir-headless` target. No GPU window.
//! Requires `GPUI_AGENT=1` and `GPUI_AGENT_TOKEN` at runtime.

fn main() -> std::process::ExitCode {
    bir_desktop::agent::headless::run()
}
