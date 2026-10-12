//! Desktop side of the Dev Mode switches: resolvers that need the desktop's
//! own cargo features, the header pills, and the "Simulate BIR confirmation"
//! helper forms reuse. The switches themselves live in `bir_core::dev_mode`;
//! the panel is `views::dev_mode_view` (debug builds only).

use gpui::*;
use gpui_component::ActiveTheme;
#[cfg(debug_assertions)]
use std::sync::{Arc, Mutex};

/// Orange used by the Dev Mode button and pills.
pub(crate) fn dev_orange() -> Hsla {
    rgb(0xEA580C).into()
}

/// Uncertified (certification-draft) forms can be opened. Default on in
/// debug builds; `--features dev-tools` keeps it on in any build.
pub(crate) fn show_uncertified_forms() -> bool {
    bir_core::dev_mode::show_uncertified_forms(cfg!(feature = "dev-tools")).value
}

/// Developer diagnostics on forms (2550Q candidate diagnostic, evidence
/// note, raw identity controls). Default off; `dev-tools` forces it on.
pub(crate) fn form_diagnostics() -> bool {
    bir_core::dev_mode::form_diagnostics(cfg!(feature = "dev-tools")).value
}

/// Database file this process opened (shown in the panel).
static OPENED_DATABASE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub(crate) fn set_opened_database_path(path: std::path::PathBuf) {
    let _ = OPENED_DATABASE.set(path);
}

#[cfg(debug_assertions)]
pub(crate) fn opened_database_path() -> std::path::PathBuf {
    OPENED_DATABASE
        .get()
        .cloned()
        .unwrap_or_else(bir_core::db::app_database_path)
}

/// Copy a database file into a ZIP at `destination`, atomically. Shared by
/// `--dev-export-live-database` and the panel's "Export a copy…".
#[cfg(any(debug_assertions, feature = "dev-tools"))]
pub(crate) fn export_database_copy(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> Result<(), String> {
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "Destination must have a valid UTF-8 file name".to_string())?;
    let temporary = destination.with_file_name(format!(
        ".{file_name}.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));

    if let Err(error) = bir_core::export_existing_database_zip(source, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!("Could not export database: {error}"));
    }

    if let Err(error) = std::fs::rename(&temporary, destination) {
        let _ = std::fs::remove_file(&temporary);
        return Err(format!(
            "Could not atomically install {}: {error}",
            destination.display()
        ));
    }

    Ok(())
}

/// One header pill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pill {
    pub label: &'static str,
    /// Red (live BIR) instead of orange.
    pub danger: bool,
}

/// Pills for every active dev switch. Empty in release builds.
pub(crate) fn active_pills() -> Vec<Pill> {
    if !cfg!(debug_assertions) {
        return Vec::new();
    }
    use bir_core::dev_mode as dm;
    let mut pills = Vec::new();
    let orange = |label| Pill {
        label,
        danger: false,
    };
    if dm::live_bir_env() {
        pills.push(Pill {
            label: "LIVE BIR (env)",
            danger: true,
        });
    }
    match dm::panel_submission_mode() {
        Some(dm::SubmissionMode::DryRun) => pills.push(orange("DRY RUN")),
        Some(dm::SubmissionMode::TestServer) => pills.push(orange("TEST SERVER")),
        None if bir_core::transport::is_sftp_dry_run() => pills.push(orange("DRY RUN")),
        None if std::env::var_os(dm::env::SFTP_HOST).is_some() => pills.push(orange("TEST SERVER")),
        None => {}
    }
    if dm::relaxed_tin().value {
        pills.push(orange("RELAXED TIN"));
    }
    if dm::auto_confirm().value {
        pills.push(orange("AUTO-CONFIRM"));
    }
    if !show_uncertified_forms() {
        pills.push(orange("UNCERTIFIED HIDDEN"));
    }
    if form_diagnostics() {
        pills.push(orange("FORM DIAGNOSTICS"));
    }
    if dm::example_alerts().value {
        pills.push(orange("EXAMPLE ALERTS"));
    }
    if dm::verbose_logs().value {
        pills.push(orange("VERBOSE LOGS"));
    }
    if dm::separate_database().value {
        pills.push(orange("DEV DATABASE"));
    }
    pills
}

/// The pill strip for the top of the content area. `None` when empty.
pub(crate) fn render_pills(cx: &App) -> Option<AnyElement> {
    let pills = active_pills();
    if pills.is_empty() {
        return None;
    }
    let danger = cx.theme().danger;
    Some(
        div()
            .id("dev_mode_pills")
            .flex()
            .flex_wrap()
            .gap_1()
            .px_3()
            .py_1()
            .children(pills.into_iter().map(|pill| {
                let color = if pill.danger { danger } else { dev_orange() };
                div()
                    .px_2()
                    .rounded_full()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(gpui::white())
                    .bg(color)
                    .child(pill.label)
            }))
            .into_any_element(),
    )
}

/// "Simulate BIR confirmation": moves a Submitted return to Confirmed
/// through the email-confirmation path. Debug builds only.
#[cfg(debug_assertions)]
pub(crate) fn simulate_confirmation(
    db: &Arc<Mutex<bir_core::db::Database>>,
    submission_filename: Option<&str>,
) -> Result<(), String> {
    let filename = submission_filename
        .filter(|name| !name.is_empty())
        .ok_or_else(|| "This return has no submitted file name to confirm.".to_string())?;
    let db = db.lock().map_err(|error| error.to_string())?;
    match bir_core::dev_mode::simulate_bir_confirmation(&db, filename)
        .map_err(|error| error.to_string())?
    {
        bir_core::db::ReceiptConfirmationOutcome::Confirmed => {
            bir_core::ipc::post_db_changed();
            Ok(())
        }
        bir_core::db::ReceiptConfirmationOutcome::Ignored => {
            Err("The simulated receipt did not match this submission.".to_string())
        }
    }
}

/// The orange button forms show on a Submitted return (debug builds only).
#[cfg(debug_assertions)]
pub(crate) fn simulate_confirmation_button(
    id: impl Into<ElementId>,
) -> gpui_component::button::Button {
    use gpui_component::button::ButtonVariants;
    gpui_component::button::Button::new(id)
        .label("Simulate BIR confirmation")
        .warning()
}

#[cfg(test)]
mod tests {
    #[test]
    fn release_defaults_hold_outside_debug_builds() {
        // In a debug test build the resolvers are live; check the pure
        // release branch through bir-core's resolver instead.
        let r = bir_core::dev_mode::resolve_flag(false, None, Some("true"), true, false);
        assert!(!r.value);
        assert_eq!(r.source, bir_core::dev_mode::Source::Release);
    }

    #[test]
    fn form_diagnostics_forced_on_by_dev_tools() {
        assert!(bir_core::dev_mode::form_diagnostics(true).value);
    }

    #[test]
    fn show_uncertified_default_is_on_in_debug() {
        if cfg!(debug_assertions) {
            // Nothing saved in a test process: dev default applies.
            assert!(bir_core::dev_mode::show_uncertified_forms(false).value);
        }
    }
}
