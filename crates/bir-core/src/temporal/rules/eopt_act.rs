//! EOPT Act rules (RA 11976, effective 2024+).

use crate::profile::EoptTier;
use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

/// 1701-MS is RECOMMENDED (not mandatory) for Micro/Small.
/// 1701 and 1701A remain ALLOWED — BIR says "you are not required to switch".
pub struct EoptMicroSmallRule;

impl TaxRule for EoptMicroSmallRule {
    fn name(&self) -> &'static str {
        "EOPT Act - Micro/Small"
    }
    fn law(&self) -> &'static str {
        "RA 11976 (EOPT Act)"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RepublicAct,
            number: "11976".into(),
            section: "Sec 45(a) Simplified ITR for Micro/Small".into(),
            year: 2024,
        }
    }
    fn effective_from(&self) -> u16 {
        2024
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
        let is_micro_small = matches!(
            facts.eopt_tier,
            Some(EoptTier::Micro) | Some(EoptTier::Small)
        );

        if form.form_code == "1701MS" {
            // Respect prior suppression from a higher-priority rule (e.g., ExclusiveGroupRule
            // suppresses 1701MS for mixed-income because it lacks compensation sections).
            if matches!(current_state, FormEligibility::Suppressed(_)) {
                return current_state;
            }

            if is_micro_small {
                return FormEligibility::Recommended(
                    "EOPT: Simplified ITR available for Micro/Small taxpayers".into(),
                );
            } else {
                return FormEligibility::Suppressed(
                    "1701-MS restricted to Micro/Small taxpayers".into(),
                );
            }
        }
        current_state
    }
}
