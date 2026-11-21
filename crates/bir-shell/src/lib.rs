//! `bir-shell` — the gpui-shell host for buwiz-forms.
//!
//! The UI lives in the JavaScript application under `app/` at the repository
//! root; this crate owns the window, the [`gpui_shell::ShellRuntime`], the
//! capability policy, and the `bir` host module that bridges JavaScript to
//! bir-core. Business logic stays in Rust; `app/` is presentational.

#[cfg(feature = "agent")]
mod agent;

#[cfg(feature = "agent")]
pub fn run_headless() -> std::process::ExitCode {
    bir_host::install_hooks();
    agent::headless::run()
}
mod app_menu;
mod bir_host;
mod blitz_render;
#[cfg(target_os = "linux")]
mod desktop_entry;
mod host;
// The Mac App Store build has no global hotkey (see `tray`).
#[cfg(any(test, not(feature = "mas_build")))]
mod hotkey;
#[cfg(any(test, target_os = "linux"))]
mod linux_csd;
pub mod print_preview_window;
mod scroll_module;
mod tray;
mod updater;

// The host functions moved to bir-app-api; these keep their `crate::` paths
// for the gpui glue that still calls them.
use bir_app_api::{account_gate, bir_module, cloud_account, packaged};
pub use bir_app_api::{print_sheet, static_html};

pub use host::{app_root, run};
