//! Substituted filing rule — PurelyCompensation + single employer.

use crate::profile::{TaxClassification, TaxpayerProfile};
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef};
use crate::temporal::traits::TaxRule;

pub struct SubstitutedFilingRule;

impl TaxRule for SubstitutedFilingRule {
    fn name(&self) -> &'static str { "Substituted Filing" }
    fn law(&self) -> &'static str { "NIRC Sec 51-A" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "8424".into(), section: "Sec 51-A Substituted Filing".into(), year: 1997 }
    }
    fn effective_from(&self) -> u16 { 1997 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        let is_purely_comp = matches!(profile.tax_classification, Some(TaxClassification::PurelyCompensation));
        if is_purely_comp && profile.has_single_employer && form.code == "1700" {
            return FormEligibility::Suppressed("Eligible for Substituted Filing — employer files on behalf".into());
        }
        current_state
    }
}
