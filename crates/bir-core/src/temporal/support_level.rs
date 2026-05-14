//! Form Support Level — separates legal eligibility from app implementation status.
//!
//! The engine determines whether a form is legally applicable. This module
//! tracks whether the app can actually draft/file that form in-app.

use serde::{Deserialize, Serialize};

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
    match form_code {
        "2551Q" | "1701Q" | "1601C" | "0619E" | "0619F" | "0605" | "2550Q" | "1701" | "1702RT"
        | "1702MX" => FormSupportLevel::ImplementedInApp,
        _ => FormSupportLevel::ExternalOrManualOnly,
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
            form_support_level("1701Q"),
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
    fn test_no_scaffold_only_forms() {
        // All forms are now ImplementedInApp — no more ScaffoldOnly
        assert_ne!(form_support_level("1702MX"), FormSupportLevel::ScaffoldOnly);
    }

    #[test]
    fn test_1702mx_is_implemented() {
        assert_eq!(
            form_support_level("1702MX"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_1702rt_is_implemented() {
        assert_eq!(
            form_support_level("1702RT"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_1701_is_implemented() {
        assert_eq!(
            form_support_level("1701"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_0619e_is_implemented() {
        assert_eq!(
            form_support_level("0619E"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_0619f_is_implemented() {
        assert_eq!(
            form_support_level("0619F"),
            FormSupportLevel::ImplementedInApp
        );
    }

    #[test]
    fn test_0605_is_implemented() {
        assert_eq!(
            form_support_level("0605"),
            FormSupportLevel::ImplementedInApp
        );
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
}
