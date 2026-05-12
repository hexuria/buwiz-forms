//! VAT registration gate — filters forms by VAT status.

use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct VatRegistrationGate;

impl TaxRule for VatRegistrationGate {
    fn name(&self) -> &'static str {
        "VAT Registration Gate"
    }
    fn law(&self) -> &'static str {
        "NIRC Sec 105-115"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RepublicAct,
            number: "8424".into(),
            section: "Sec 105-115".into(),
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
        match form.requires_vat {
            Some(true) if !facts.is_vat_registered => {
                FormEligibility::Suppressed("Taxpayer is not VAT registered".into())
            }
            Some(false) if facts.is_vat_registered => FormEligibility::Suppressed(
                "VAT-registered taxpayers file VAT returns, not percentage tax".into(),
            ),
            _ => current_state,
        }
    }
}
