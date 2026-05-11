//! Classification rule — filters forms by TaxClassification.

use crate::profile::TaxpayerProfile;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef};
use crate::temporal::traits::TaxRule;

pub struct ClassificationRule;

impl TaxRule for ClassificationRule {
    fn name(&self) -> &'static str { "Classification Filter" }
    fn law(&self) -> &'static str { "NIRC General" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "8424".into(), section: "NIRC".into(), year: 1997 }
    }
    fn effective_from(&self) -> u16 { 1997 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        // If form has no classification restrictions, pass through
        if form.classifications.is_empty() {
            return current_state;
        }
        // Only filter Income Tax, VAT, and Percentage Tax categories
        let dominated = matches!(form.category.as_str(), "Income Tax" | "Value-Added Tax" | "Percentage Tax");
        if !dominated {
            return current_state;
        }
        if let Some(ref cls) = profile.tax_classification {
            if !form.classifications.contains(cls) {
                return FormEligibility::Suppressed(format!("Not applicable for {:?} classification", cls));
            }
        }
        // Also check entity type
        if !form.taxpayer_types.contains(&profile.taxpayer_type) {
            return FormEligibility::Suppressed("Entity type mismatch".into());
        }
        current_state
    }
}
