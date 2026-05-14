//! Exclusive group rule — ensures mutual exclusion (e.g., 1702 family, 1701 family).

use crate::profile::TaxClassification;
use crate::temporal::eligibility_facts::{EligibilityFacts, IndividualIncomeKind};
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct ExclusiveGroupRule;

impl TaxRule for ExclusiveGroupRule {
    fn name(&self) -> &'static str {
        "Exclusive Group Rule"
    }
    fn law(&self) -> &'static str {
        "BIR Form Instructions"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::BirFormInstruction,
            number: "1702".into(),
            section: "Annual ITR variant routing".into(),
            year: 2018,
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
        target_year: u16,
    ) -> FormEligibility {
        let group = match &form.exclusive_group {
            Some(g) => g.as_str(),
            None => return current_state,
        };

        // ── Corporate ITR family: 1702RT vs 1702EX vs 1702MX ──
        if group == "ANNUAL_CORPORATE_ITR" {
            let winner = match &facts.effective_classification {
                Some(TaxClassification::CooperativeExempt) => "1702EX",
                Some(TaxClassification::CooperativeMixed) => "1702MX",
                Some(TaxClassification::Corporation)
                | Some(TaxClassification::CooperativeTaxable) => "1702RT",
                _ => "1702RT", // Default for corporations
            };
            if form.form_code == winner {
                return current_state; // Keep this one
            } else {
                return FormEligibility::Suppressed(format!(
                    "Exclusive group: {} selected instead",
                    winner
                ));
            }
        }

        // ── Individual ITR family: 1701 vs 1701A vs 1701MS ──
        //
        // Unlike corporate ITR (strict exclusive selection), individual ITR uses
        // ranked recommendations: the primary form is kept at current_state,
        // alternatives are marked Optional (not Suppressed) so they remain visible
        // but secondary. Only truly inapplicable combinations are Suppressed.
        if group == "ANNUAL_INDIVIDUAL_ITR" {
            match &facts.individual_income_kind {
                // ── Mixed income: only 1701 has both comp + business sections ──
                Some(IndividualIncomeKind::MixedIncome) => {
                    if form.form_code == "1701" {
                        return current_state; // Primary
                    }
                    return FormEligibility::Suppressed(
                        "Mixed income requires full 1701 form".into(),
                    );
                }

                // ── Business/profession only ──
                Some(IndividualIncomeKind::BusinessOrProfessionOnly) => {
                    if facts.has_8_percent_election(target_year) {
                        // 8% elected: 1701A is primary, 1701 is allowed alternative
                        match form.form_code.as_str() {
                            "1701A" => return current_state, // Primary simplified form
                            "1701" => {
                                return FormEligibility::Optional(
                                    "Full form also allowed for 8% filers".into(),
                                );
                            }
                            "1701MS" => {
                                // Let EoptMicroSmallRule handle Recommended vs Suppressed
                                return current_state;
                            }
                            _ => return current_state,
                        }
                    } else {
                        // No 8% election: 1701 is primary, 1701A is allowed alternative
                        match form.form_code.as_str() {
                            "1701" => return current_state, // Primary full form
                            "1701A" => {
                                return FormEligibility::Optional(
                                    "Simplified form available for OSD filers".into(),
                                );
                            }
                            "1701MS" => {
                                // Let EoptMicroSmallRule handle Recommended vs Suppressed
                                return current_state;
                            }
                            _ => return current_state,
                        }
                    }
                }

                // ── Compensation only: 1701 (or 1700 via SubstitutedFilingRule) ──
                Some(IndividualIncomeKind::CompensationOnly) => {
                    if form.form_code == "1701" {
                        return current_state;
                    }
                    return FormEligibility::Suppressed(
                        "Compensation-only filers use 1701/1700".into(),
                    );
                }

                // ── Unconfigured/Estate/Trust: default to 1701, suppress others ──
                None => {
                    if form.form_code == "1701" {
                        return current_state;
                    }
                    return FormEligibility::Suppressed(
                        "Classification not configured; defaulting to 1701".into(),
                    );
                }
            }
        }

        current_state
    }
}
