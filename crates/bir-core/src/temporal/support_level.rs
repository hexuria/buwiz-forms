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
        "2551Q" | "1701Q" | "1601C" => FormSupportLevel::ImplementedInApp,
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
        assert_eq!(
            form_support_level("1701"),
            FormSupportLevel::ExternalOrManualOnly
        );
        assert_eq!(
            form_support_level("2550Q"),
            FormSupportLevel::ExternalOrManualOnly
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
    }

    #[test]
    fn test_fileable_in_app() {
        assert!(FormSupportLevel::ImplementedInApp.is_fileable_in_app());
        assert!(!FormSupportLevel::ExternalOrManualOnly.is_fileable_in_app());
        assert!(!FormSupportLevel::Planned.is_fileable_in_app());
    }
}
