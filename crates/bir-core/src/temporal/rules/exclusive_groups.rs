//! Exclusive group rule — ensures mutual exclusion (e.g., 1702 family).

use crate::profile::TaxClassification;
use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct ExclusiveGroupRule;

impl TaxRule for ExclusiveGroupRule {
    fn name(&self) -> &'static str {
        "Exclusive Group Rule"
    }
    fn law(&self) -> &'static str {
        "BIR Form Instructions"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::BirFormInstruction,
            number: "1702".into(),
            section: "Annual Corporate ITR variants".into(),
            year: 2018,
        }
    }
    fn effective_from(&self) -> u16 {
        1997
    }
    fn effective_until(&self) -> Option<u16> {
        None
    }

    fn evaluate(
        &self,
        facts: &EligibilityFacts,
        form: &FormArtifact,
        current_state: FormEligibility,
        _target_year: u16,
    ) -> FormEligibility {
        let group = match &form.exclusive_group {
            Some(g) => g.as_str(),
            None => return current_state,
        };
        if group == "ANNUAL_CORPORATE_ITR" {
            let winner = match &facts.effective_classification {
                Some(TaxClassification::CooperativeExempt) => "1702EX",
                Some(TaxClassification::CooperativeMixed) => "1702MX",
                Some(TaxClassification::Corporation)
                | Some(TaxClassification::CooperativeTaxable) => "1702RT",
                _ => "1702RT", // Default for corporations
            };
            if form.form_code == winner {
                return current_state; // Keep this one
            } else {
                return FormEligibility::Suppressed(format!(
                    "Exclusive group: {} selected instead",
                    winner
                ));
            }
        }
        current_state
    }
}
