//! Form Support Level — separates legal eligibility from app implementation status.
//!
//! The engine determines whether a form is legally applicable. This module
//! tracks whether the app can actually draft/file that form in-app.

use serde::{Deserialize, Serialize};

const FILEABLE_FORM_CODES: &[&str] = &[
    "2551Q", "1601C", "2550Q", "1701Q", "1702RT", "1702MX", "1701", "0605", "0619E", "0619F",
];

/// Whether the app can actually draft/file this form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormSupportLevel {
    /// Fully implemented with an in-app form view.
    ImplementedInApp,
    /// Generated struct exists but formulas/computation not yet verified.
    /// Shows in dashboard but cannot be filed.
    ScaffoldOnly,
    /// Form exists legally but must be filed manually or via eBIRForms official client.
    ExternalOrManualOnly,
    /// Planned for future implementation.
    Planned,
}

impl FormSupportLevel {
    /// Returns a human-readable label for the dashboard.
    pub fn action_label(&self) -> &'static str {
        match self {
            Self::ImplementedInApp => "File in App",
            Self::ScaffoldOnly => "Preview only",
            Self::ExternalOrManualOnly => "Manual filing only",
            Self::Planned => "Coming soon",
        }
    }

    /// Whether the form can be filed through the app UI.
    pub fn is_fileable_in_app(&self) -> bool {
        matches!(self, Self::ImplementedInApp)
    }
}

/// Returns the support level for a given form code.
///
/// Only forms with fully implemented in-app views are marked as `ImplementedInApp`.
/// All other legally applicable forms default to `ExternalOrManualOnly`.
pub fn form_support_level(form_code: &str) -> FormSupportLevel {
    if FILEABLE_FORM_CODES.contains(&form_code) {
        return FormSupportLevel::ImplementedInApp;
    }

    // All scaffold forms have been promoted. Remaining form codes
    // are external-only until savefiles are acquired.
    FormSupportLevel::ExternalOrManualOnly
}

/// Whether the app may place this form in the background submission queue.
pub fn can_queue_for_submission(form_code: &str) -> bool {
    FILEABLE_FORM_CODES.contains(&form_code)
}

/// eBIR/eFPS form type ID for fileable forms.
pub fn fileable_form_type_id(form_code: &str) -> Option<&'static str> {
    match form_code {
        "2551Q" => Some("2551Qv2018"),
        "1601C" => Some("1601Cv2018"),
        "2550Q" => Some("2550Qv2024"),
        "1701Q" => Some("1701Qv2018"),
        "1702RT" => Some("1702RTv2018C"),
        "1702MX" => Some("1702MXv2018C"),
        "1701" => Some("1701v2018"),
        "0605" => Some("0605v1999"),
        "0619E" => Some("0619Ev2018"),
        "0619F" => Some("0619Fv2018"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implemented_forms() {
        assert_eq!(
            form_support_level("2551Q"),
            FormSupportLevel::ImplementedInApp
        );
        assert_eq!(
            form_support_level("1601C"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_unimplemented_forms() {
        assert_eq!(
            form_support_level("1700"),
            FormSupportLevel::ExternalOrManualOnly
        );
    }

    #[test]
    fn test_no_more_scaffold_only_forms() {
        // All originally scaffolded forms have been promoted
        for form_code in ["0605", "0619E", "0619F"] {
            let support = form_support_level(form_code);
            assert_eq!(support, FormSupportLevel::ImplementedInApp);
            assert!(support.is_fileable_in_app());
        }
    }

    #[test]
    fn test_action_labels() {
        assert_eq!(
            FormSupportLevel::ImplementedInApp.action_label(),
            "File in App"
        );
        assert_eq!(
            FormSupportLevel::ExternalOrManualOnly.action_label(),
            "Manual filing only"
        );
        assert_eq!(FormSupportLevel::Planned.action_label(), "Coming soon");
        assert_eq!(
            FormSupportLevel::ScaffoldOnly.action_label(),
            "Preview only"
        );
    }

    #[test]
    fn test_fileable_in_app() {
        assert!(FormSupportLevel::ImplementedInApp.is_fileable_in_app());
        assert!(!FormSupportLevel::ScaffoldOnly.is_fileable_in_app());
        assert!(!FormSupportLevel::ExternalOrManualOnly.is_fileable_in_app());
        assert!(!FormSupportLevel::Planned.is_fileable_in_app());
    }

    #[test]
    fn test_queue_allowlist_matches_support_level() {
        // Unimplemented forms cannot be queued
        for form_code in ["1700", "2316", "9999"] {
            assert!(!can_queue_for_submission(form_code));
            assert_eq!(fileable_form_type_id(form_code), None);
        }

        // All implemented forms can be queued
        for form_code in [
            "2551Q", "1601C", "2550Q", "1701Q", "1702RT", "1702MX", "1701", "0605", "0619E",
            "0619F",
        ] {
            assert!(can_queue_for_submission(form_code));
            assert!(form_support_level(form_code).is_fileable_in_app());
            assert!(fileable_form_type_id(form_code).is_some());
        }
    }
}
