//! Exclusive group rule — ensures mutual exclusion (e.g., 1702 family, 1701 family).

use crate::profile::{EoptTier, TaxClassification};
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
        if group == "ANNUAL_INDIVIDUAL_ITR" {
            let winner = match &facts.individual_income_kind {
                // Mixed income always needs the full 1701 form
                // (has both compensation and business/professional sections)
                Some(IndividualIncomeKind::MixedIncome) => "1701",

                // Self-employed / Professional
                Some(IndividualIncomeKind::BusinessOrProfessionOnly) => {
                    if facts.has_8_percent_election(target_year) {
                        // 8% elected: prefer simplified form
                        let is_micro_small = matches!(
                            facts.eopt_tier,
                            Some(EoptTier::Micro) | Some(EoptTier::Small)
                        );
                        if is_micro_small {
                            "1701MS" // EOPT simplified
                        } else {
                            "1701A" // 8% / OSD simplified
                        }
                    } else {
                        // Non-8%: 1701A for OSD, 1701 for itemized.
                        // Default to 1701 (broadest) — user can switch to 1701A.
                        "1701"
                    }
                }

                // Estates/Trusts or unknown → full 1701
                _ => "1701",
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

        current_state
    }
}
