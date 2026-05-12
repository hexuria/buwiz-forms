//! Withholding agent rule — gates withholding forms.

use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct WithholdingAgentRule;

impl TaxRule for WithholdingAgentRule {
    fn name(&self) -> &'static str {
        "Withholding Agent Rule"
    }
    fn law(&self) -> &'static str {
        "NIRC Sec 57-58"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RepublicAct,
            number: "8424".into(),
            section: "Sec 57-58".into(),
            year: 1997,
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
        let trigger = match &form.withholding_trigger {
            Some(t) => t.as_str(),
            None => return current_state,
        };
        // Match against the string representation from the snapshot
        let has_obligation = match trigger {
            "Compensation" => facts.has_employees,
            "Expanded" => facts.is_expanded_withholding_agent,
            "Final" => facts.has_employees || facts.is_expanded_withholding_agent,
            _ => return current_state,
        };
        if !has_obligation {
            FormEligibility::Suppressed(format!("No {} withholding obligation", trigger))
        } else {
            current_state
        }
    }
}
