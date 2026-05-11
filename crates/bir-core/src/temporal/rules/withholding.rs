//! Withholding agent rule — gates withholding forms.

use crate::profile::TaxpayerProfile;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation, TemporalFormDef, WithholdingTrigger};
use crate::temporal::traits::TaxRule;

pub struct WithholdingAgentRule;

impl TaxRule for WithholdingAgentRule {
    fn name(&self) -> &'static str { "Withholding Agent Rule" }
    fn law(&self) -> &'static str { "NIRC Sec 57-58" }
    fn citation(&self) -> LegalCitation {
        LegalCitation { kind: CitationKind::RepublicAct, number: "8424".into(), section: "Sec 57-58".into(), year: 1997 }
    }
    fn effective_from(&self) -> u16 { 1997 }
    fn effective_until(&self) -> Option<u16> { None }

    fn evaluate(&self, profile: &TaxpayerProfile, form: &TemporalFormDef, current_state: FormEligibility, _target_year: u16) -> FormEligibility {
        let trigger = match &form.withholding_trigger {
            Some(t) => t,
            None => return current_state,
        };
        // Use existing boolean fields for backward compat
        let has_obligation = match trigger {
            WithholdingTrigger::Compensation => profile.has_employees,
            WithholdingTrigger::Expanded => profile.is_expanded_withholding_agent,
            WithholdingTrigger::Final => profile.has_employees || profile.is_expanded_withholding_agent,
        };
        if !has_obligation {
            FormEligibility::Suppressed(format!("No {:?} withholding obligation", trigger))
        } else {
            current_state
        }
    }
}
