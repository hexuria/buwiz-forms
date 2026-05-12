//! Substituted filing rule — PurelyCompensation + single employer.

use crate::temporal::eligibility_facts::{EligibilityFacts, IndividualIncomeKind};
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct SubstitutedFilingRule;

impl TaxRule for SubstitutedFilingRule {
    fn name(&self) -> &'static str {
        "Substituted Filing"
    }
    fn law(&self) -> &'static str {
        "NIRC Sec 51-A"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RepublicAct,
            number: "8424".into(),
            section: "Sec 51-A Substituted Filing".into(),
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
        let is_purely_comp = matches!(
            facts.individual_income_kind,
            Some(IndividualIncomeKind::CompensationOnly)
        );
        if is_purely_comp && facts.has_single_employer && form.form_code == "1700" {
            return FormEligibility::Suppressed(
                "Eligible for Substituted Filing — employer files on behalf".into(),
            );
        }
        current_state
    }
}
