//! VAT registration gate — filters forms by VAT status.

use crate::profile::TaxpayerProfile;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef};
use crate::temporal::traits::TaxRule;

pub struct VatRegistrationGate;

impl TaxRule for VatRegistrationGate {
    fn name(&self) -> &'static str { "VAT Registration Gate" }
    fn law(&self) -> &'static str { "NIRC Sec 105-115" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "8424".into(), section: "Sec 105-115".into(), year: 1997 }
    }
    fn effective_from(&self) -> u16 { 1997 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        match form.requires_vat {
            Some(true) if !profile.is_vat_registered => {
                FormEligibility::Suppressed("Taxpayer is not VAT registered".into())
            }
            Some(false) if profile.is_vat_registered => {
                FormEligibility::Suppressed("VAT-registered taxpayers file VAT returns, not percentage tax".into())
            }
            _ => current_state,
        }
    }
}
