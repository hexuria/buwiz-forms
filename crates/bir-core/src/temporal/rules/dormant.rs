//! Dormancy rule — annotates or suppresses forms based on business status.

use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct DormancyRule;

impl TaxRule for DormancyRule {
    fn name(&self) -> &'static str {
        "Dormancy Rule"
    }
    fn law(&self) -> &'static str {
        "BIR RMO 1-2019"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RevenueMemoOrder,
            number: "1-2019".into(),
            section: "Dormancy Filing".into(),
            year: 2019,
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
        _form: &FormArtifact,
        current_state: FormEligibility,
        _target_year: u16,
    ) -> FormEligibility {
        if !facts.is_dormant {
            return current_state;
        }
        // Dormant: annotate reason but do NOT suppress
        match current_state {
            FormEligibility::Applicable => {
                FormEligibility::Required("Dormant / No Operations - NIL Filing Required".into())
            }
            other => other,
        }
    }
}
