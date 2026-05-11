//! Excise tax category rule.

use crate::profile::TaxpayerProfile;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef};
use crate::temporal::traits::TaxRule;

pub struct ExciseTaxRule;

impl TaxRule for ExciseTaxRule {
    fn name(&self) -> &'static str { "Excise Tax Category" }
    fn law(&self) -> &'static str { "NIRC Title VI" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "8424".into(), section: "Title VI Excise Tax".into(), year: 1997 }
    }
    fn effective_from(&self) -> u16 { 1997 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        if let Some(ref required_cat) = form.excise_category {
            if !profile.excise_tax_categories.contains(required_cat) {
                return FormEligibility::Suppressed(format!("Not liable for {:?} excise tax", required_cat));
            }
        }
        current_state
    }
}
