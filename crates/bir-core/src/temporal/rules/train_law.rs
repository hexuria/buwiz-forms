//! TRAIN Law rules (RA 10963, effective 2018+).

use crate::profile::TaxpayerProfile;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef};
use crate::temporal::traits::TaxRule;

/// 8% flat rate suppresses 2551Q only.
/// CRITICAL: Does NOT suppress annual ITR (1701/1701A/1701MS).
pub struct TrainLaw8PercentRule;

impl TaxRule for TrainLaw8PercentRule {
    fn name(&self) -> &'static str { "TRAIN Law - 8% Flat Rate" }
    fn law(&self) -> &'static str { "RA 10963 (TRAIN Law)" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "10963".into(), section: "Sec 24(A)(2)(b) - 8% in lieu of graduated + percentage tax".into(), year: 2018 }
    }
    fn effective_from(&self) -> u16 { 2018 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, target_year: u16) -> FormEligibility {
        if !profile.has_8_percent_election(target_year) {
            return current_state;
        }
        // Only suppress percentage tax — NOT annual ITR
        if form.code == "2551Q" {
            return FormEligibility::Suppressed("8% flat rate replaces percentage tax".into());
        }
        current_state
    }
}

/// 2551M abolished by TRAIN Law.
pub struct TrainLaw2551MAbolitionRule;

impl TaxRule for TrainLaw2551MAbolitionRule {
    fn name(&self) -> &'static str { "TRAIN Law - 2551M Abolition" }
    fn law(&self) -> &'static str { "RA 10963 (TRAIN Law)" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "10963".into(), section: "Sec 116 - Monthly percentage tax abolished".into(), year: 2018 }
    }
    fn effective_from(&self) -> u16 { 2018 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, _profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        if form.code == "2551M" {
            return FormEligibility::Deprecated("Monthly percentage tax abolished by TRAIN Law 2018".into());
        }
        current_state
    }
}

/// 1601E replaced by 0619E + 1601EQ under TRAIN.
pub struct TrainLaw1601EReplacement;

impl TaxRule for TrainLaw1601EReplacement {
    fn name(&self) -> &'static str { "TRAIN Law - 1601E Replacement" }
    fn law(&self) -> &'static str { "RA 10963, RR 11-2018" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RevenueRegulation, number: "11-2018".into(), section: "1601E replaced by 0619E/1601EQ".into(), year: 2018 }
    }
    fn effective_from(&self) -> u16 { 2018 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, _profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        if form.code == "1601E" {
            return FormEligibility::Deprecated("Replaced by 0619E and 1601EQ under TRAIN Law".into());
        }
        current_state
    }
}
