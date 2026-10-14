//! `bir-shell` — the gpui-shell host for buwiz-forms.
//!
//! The UI lives in the JavaScript application under `app/` at the repository
//! root; this crate owns the window, the [`gpui_shell::ShellRuntime`], the
//! capability policy, and the `bir` host module that bridges JavaScript to
//! bir-core. Business logic stays in Rust; `app/` is presentational.
//!
//! See `docs/plans/gpui-shell-migration.md` (tracking issue #135) for the
//! slice plan.

mod bir_module;
mod host;

pub use host::{app_root, run};
