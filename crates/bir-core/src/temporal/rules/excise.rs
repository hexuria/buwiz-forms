//! Excise tax category rule.

use crate::profile::ExciseTaxCategory;
use crate::temporal::eligibility_facts::EligibilityFacts;
use crate::temporal::forms::FormArtifact;
use crate::temporal::traits::TaxRule;
use crate::temporal::{CitationKind, FormEligibility, LegalCitation};

pub struct ExciseTaxRule;

impl TaxRule for ExciseTaxRule {
    fn name(&self) -> &'static str {
        "Excise Tax Category"
    }
    fn law(&self) -> &'static str {
        "NIRC Title VI"
    }
    fn citation(&self) -> LegalCitation {
        LegalCitation {
            citation_id: String::new(),
            kind: CitationKind::RepublicAct,
            number: "8424".into(),
            section: "Title VI Excise Tax".into(),
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
        if let Some(ref required_cat_str) = form.excise_category {
            // Parse the string from the snapshot into the ExciseTaxCategory enum
            let required_cat = match required_cat_str.as_str() {
                "Alcohol" => ExciseTaxCategory::Alcohol,
                "AutomobilesAndNonEssential" => ExciseTaxCategory::AutomobilesAndNonEssential,
                "Mineral" => ExciseTaxCategory::Mineral,
                "Petroleum" => ExciseTaxCategory::Petroleum,
                "Tobacco" => ExciseTaxCategory::Tobacco,
                "SweetenedBeverages" => ExciseTaxCategory::SweetenedBeverages,
                "CoalAndCoke" => ExciseTaxCategory::CoalAndCoke,
                _ => {
                    return FormEligibility::Suppressed(format!(
                        "Unknown excise category: {}",
                        required_cat_str
                    ));
                }
            };
            if !facts.excise_tax_categories.contains(&required_cat) {
                return FormEligibility::Suppressed(format!(
                    "Not liable for {:?} excise tax",
                    required_cat
                ));
            }
        } else if form.category == "Excise Tax" {
            if facts.excise_tax_categories.is_empty() {
                return FormEligibility::Suppressed("No excise tax liabilities".into());
            }
        }
        current_state
    }
}
