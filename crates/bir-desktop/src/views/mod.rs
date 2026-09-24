pub mod admin_calendar_dashboard;
pub mod cron_tasks;
pub mod dashboard;
pub mod debug_log_viewer;
pub mod email_confirmation_view;
pub mod form_2550q_diagnostics;
pub mod form_agent_patches;
pub mod form_html_preview_launcher;
pub mod form_inventory_view;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub mod frozen_html_preview;
pub mod global_dashboard;
pub mod import_export;
pub mod lock_screen;
pub mod notifications;
pub mod profile_manager;
pub(crate) mod secondary_window;
pub mod settings;
