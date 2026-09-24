//! Agent host patches for inventory-backed 2551Q / 1601-C.
//!
//! Kept as thin typed adapters so the painted inventory view can accept the
//! same `form.fill` keys the host already emits. The old per-form views
//! are gone.

use bir_core::forms::form_1601c::Form1601CDraft;

/// Item 11 Category of Withholding Agent. Copy-sized so it can live on
/// `Agent1601CHostPatch` without dropping `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Agent1601CCategory {
    Private,
    Government,
}

impl Agent1601CCategory {
    pub(crate) fn as_code(self) -> &'static str {
        match self {
            Self::Private => "P",
            Self::Government => "G",
        }
    }

    pub(crate) fn from_code(code: &str) -> Option<Self> {
        match code {
            "P" => Some(Self::Private),
            "G" => Some(Self::Government),
            _ => None,
        }
    }
}

/// Host-side 1601-C field patch applied on the UI thread.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Agent1601CHostPatch {
    pub tax_14: Option<f64>,
    pub tax_25: Option<f64>,
    pub sheets: Option<u32>,
    pub any_taxes_withheld: Option<bool>,
    pub category_of_agent: Option<Agent1601CCategory>,
    pub save: bool,
    pub validate: bool,
}

/// Writes header flags onto the painted controls and the draft `validate` reads.
pub(crate) fn apply_1601c_host_header_patch(
    patch: &Agent1601CHostPatch,
    any_taxes_withheld: &mut bool,
    category_of_agent: &mut String,
    draft: &mut Form1601CDraft,
) -> bool {
    let mut dirty = false;
    if let Some(value) = patch.any_taxes_withheld {
        if *any_taxes_withheld != value || draft.any_taxes_withheld != value {
            *any_taxes_withheld = value;
            draft.any_taxes_withheld = value;
            dirty = true;
        }
    }
    if let Some(value) = patch.category_of_agent {
        let code = value.as_code().to_string();
        if *category_of_agent != code || draft.category_of_agent != code {
            *category_of_agent = code.clone();
            draft.category_of_agent = code;
            dirty = true;
        }
    }
    dirty
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Agent2551QHostPatch {
    pub creditable_tax_withheld: Option<f64>,
    pub other_tax_credit: Option<f64>,
    pub taxable_amount_0: Option<f64>,
    pub save: bool,
    pub validate: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_11_category_codes_round_trip() {
        assert_eq!(Agent1601CCategory::Private.as_code(), "P");
        assert_eq!(Agent1601CCategory::Government.as_code(), "G");
        assert_eq!(
            Agent1601CCategory::from_code("P"),
            Some(Agent1601CCategory::Private)
        );
        assert_eq!(
            Agent1601CCategory::from_code("G"),
            Some(Agent1601CCategory::Government)
        );
        assert!(Agent1601CCategory::from_code("X").is_none());
    }

    #[test]
    fn production_ui_reports_no_reviewed_2550q_validator_without_running_candidate() {
        use crate::views::form_2550q_diagnostics::{
            Form2550QDiagnosticState, diagnostic_state_from_setup,
        };
        use bir_core::form_rules::Form2550QLiveValidationFacade;

        let setup = Form2550QLiveValidationFacade::setup_repo_default_diagnostic();
        let state = diagnostic_state_from_setup(&setup);
        let Form2550QDiagnosticState::Unavailable(message) = state else {
            panic!("the empty reviewed registry must be unavailable");
        };
        assert!(message.contains("no review-controlled exact 2550Q"));
        assert!(message.contains("reviewed registry entries alone do not authorize activation"));
        assert!(message.contains("No candidate validator was evaluated"));
        assert!(message.contains("does not mean the draft is valid or filing-ready"));
    }
}
