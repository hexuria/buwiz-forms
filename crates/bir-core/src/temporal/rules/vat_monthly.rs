//! VAT monthly transition rule (2023+).

use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

/// 2550M becomes optional (not abolished) starting 2023.
pub struct VatMonthlyTransitionRule;

impl TaxRule for VatMonthlyTransitionRule {
    fn name(&self) -> &'static str {
        "VAT Monthly Transition"
    }
    fn law(&self) -> &'static str {
        "RMC 5-2023, RMC 52-2023"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RevenueMemoCircular,
            number: "52-2023".into(),
            section: "Optional monthly VAT filing".into(),
            year: 2023,
        }
    }
    fn effective_from(&self) -> u16 {
        2023
    }
    fn effective_until(&self) -> Option<u16> {
        None
    }

    fn evaluate(
        &self,
        _facts: &EligibilityFacts,
        form: &FormArtifact,
        current_state: FormEligibility,
        _target_year: u16,
    ) -> FormEligibility {
        if form.form_code == "2550M"
            && matches!(
                current_state,
                FormEligibility::Applicable | FormEligibility::Required(_)
            )
        {
            return FormEligibility::Optional(
                "Monthly VAT no longer mandatory. File optionally per RMC 52-2023.".into(),
            );
        }
        current_state
    }
}
