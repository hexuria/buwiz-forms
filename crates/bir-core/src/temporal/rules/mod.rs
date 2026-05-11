//! Rule registry — collects all tax rules.

pub mod classification;
pub mod dormant;
pub mod eopt_act;
pub mod excise;
pub mod exclusive_groups;
pub mod substituted;
pub mod train_law;
pub mod vat_gate;
pub mod vat_monthly;
pub mod withholding;

use crate::temporal::traits::TaxRule;

/// Returns all registered tax rules in evaluation order.
pub fn all_rules() -> Vec<Box<dyn TaxRule>> {
    vec![
        // === TIMELESS (always active) ===
        Box::new(classification::ClassificationRule),
        Box::new(vat_gate::VatRegistrationGate),
        Box::new(withholding::WithholdingAgentRule),
        Box::new(excise::ExciseTaxRule),
        Box::new(dormant::DormancyRule),
        Box::new(exclusive_groups::ExclusiveGroupRule),
        Box::new(substituted::SubstitutedFilingRule),
        // === ERA-SCOPED ===
        Box::new(train_law::TrainLaw8PercentRule),
        Box::new(train_law::TrainLaw2551MAbolitionRule),
        Box::new(train_law::TrainLaw1601EReplacement),
        Box::new(vat_monthly::VatMonthlyTransitionRule),
        Box::new(eopt_act::EoptMicroSmallRule),
    ]
}
