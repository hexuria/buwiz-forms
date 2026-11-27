//! `bir-app-api`: the host functions the app's screens call, with no UI.
//!
//! The `bir` module ([`module`]) and the modules it is assembled from moved
//! here from `bir-shell` unchanged; what they need from a window goes through
//! [`hooks::Hooks`]. `gpui_shell` in this crate is `bir-host-shim`, a
//! UI-free copy of the gpui-shell host types: `bir-shell` adapts the module to
//! the real gpui-shell, and the NSH door calls it directly.

pub mod account_gate;
#[cfg(feature = "agent")]
pub mod agent;
pub mod auth_limiter;
pub mod bir_module;
pub mod cloud_account;
pub mod cloud_email;
pub mod dashboard_module;
pub mod hooks;
pub mod hotkey_combo;
pub mod legacy_import_module;
pub mod logging;
pub mod owner_lock_module;
pub mod packaged;
pub mod print_sheet;
pub mod screen_form_module;
pub mod send_state;
pub mod static_html;
pub mod tax_profile_module;

pub use bir_module::module;
pub use gpui_shell as host;
pub use hooks::{Hooks, hooks, install_hooks};
