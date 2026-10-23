//! `bir-shell` — the gpui-shell host for buwiz-forms.
//!
//! The UI lives in the JavaScript application under `app/` at the repository
//! root; this crate owns the window, the [`gpui_shell::ShellRuntime`], the
//! capability policy, and the `bir` host module that bridges JavaScript to
//! bir-core. Business logic stays in Rust; `app/` is presentational.
//!
//! See `docs/plans/gpui-shell-migration.md` (tracking issue #135) for the
//! slice plan.

#[cfg(feature = "agent")]
mod agent;
mod app_menu;
mod auth_limiter;
mod bir_module;
mod blitz_render;
mod dashboard_module;
mod host;
mod hotkey;
mod logging;
mod packaged;
pub mod print_preview_window;
pub mod print_sheet;
mod screen_form_module;
pub mod static_html;
mod tax_profile_module;
mod tray;
mod updater;

pub use host::{app_root, run};
