//! Tax types read from a year's forms list (owner decision 2), and the
//! profile facts each form reads (decision 3).
//!
//! VAT-registered, withholding and the like are not stored: a form on the
//! year's list says the branch has that tax type ("VAT-registered · from
//! 2550Q"). The table is the one the old registered-tax-type filter used
//! (`integration::validation::registered_tax_types_allow_form` and its
//! `is_*_form` helpers); where the old TTCE category list disagreed with
//! it (1604CF under compensation withholding, 1702 and 1704 missing from
//! income tax, 1601E, 1606 and 1621 missing from expanded withholding),
//! the filter wins.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::forms::agent::{AgentFormKind, Binding};
use crate::forms::registry::canonical_form_code;
use crate::profile::RegisteredTaxType;

use super::year_facts::Fact;

/// The order the chips are shown in, with each type's id and plain label.
const TAX_TYPES: [(RegisteredTaxType, &str, &str); 9] = [
    (
        RegisteredTaxType::ValueAddedTax,
        "value_added_tax",
        "VAT-registered",
    ),
    (
        RegisteredTaxType::PercentageTax,
        "percentage_tax",
        "Percentage tax",
    ),
    (
        RegisteredTaxType::WithholdingCompensation,
        "withholding_compensation",
        "Withholds on compensation",
    ),
    (
        RegisteredTaxType::WithholdingExpanded,
        "withholding_expanded",
        "Withholds expanded tax",
    ),
    (
        RegisteredTaxType::WithholdingFinal,
        "withholding_final",
        "Withholds final tax",
    ),
    (
        RegisteredTaxType::WithholdingVatAndPercentage,
        "withholding_vat_and_percentage",
        "Withholds VAT and percentage tax",
    ),
    (RegisteredTaxType::IncomeTax, "income_tax", "Income tax"),
    (RegisteredTaxType::ExciseTax, "excise_tax", "Excise tax"),
    (
        RegisteredTaxType::RegistrationFee,
        "registration_fee",
        "Registration fee",
    ),
];

/// The tax type a form on the list says the branch has, or `None` for a
/// form that tells nothing (an event or one-off transaction form).
pub fn tax_type_of(form_code: &str) -> Option<RegisteredTaxType> {
    let code = canonical_form_code(form_code);
    let tax_type = match code.as_str() {
        "1700" | "1701" | "1701A" | "1701MS" | "1701Q" | "1702" | "1702EX" | "1702MX" | "1702Q"
        | "1702RT" | "1704" => RegisteredTaxType::IncomeTax,
        "2550DS" | "2550M" | "2550Q" => RegisteredTaxType::ValueAddedTax,
        "2551M" | "2551Q" => RegisteredTaxType::PercentageTax,
        "0619E" | "1601E" | "1601EQ" | "1604E" | "1606" | "1621" => {
            RegisteredTaxType::WithholdingExpanded
        }
        "0620" | "1601C" | "1604C" | "2316" => RegisteredTaxType::WithholdingCompensation,
        "0619F" | "1600WP" | "1601F" | "1601FQ" | "1602" | "1602Q" | "1603" | "1603Q" | "1604F" => {
            RegisteredTaxType::WithholdingFinal
        }
        "1600" | "1600PT" | "1600VT" => RegisteredTaxType::WithholdingVatAndPercentage,
        "2200A" | "2200AN" | "2200C" | "2200M" | "2200P" | "2200S" | "2200T" => {
            RegisteredTaxType::ExciseTax
        }
        "0605" => RegisteredTaxType::RegistrationFee,
        _ => return None,
    };
    Some(tax_type)
}

/// One chip: a tax type, its plain label and the year's forms that give it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TaxTypeChip {
    #[serde(rename = "type")]
    pub tax_type: &'static str,
    pub label: &'static str,
    pub forms: Vec<String>,
}

/// The tax types a year's forms list gives, in chip order, each with the
/// forms (canonical codes, sorted) that give it.
pub fn tax_types_for_year<I, S>(codes: I) -> Vec<TaxTypeChip>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut by_type: BTreeMap<RegisteredTaxType, Vec<String>> = BTreeMap::new();
    for code in codes {
        let code = canonical_form_code(code.as_ref());
        if let Some(tax_type) = tax_type_of(&code) {
            let forms = by_type.entry(tax_type).or_default();
            if !forms.contains(&code) {
                forms.push(code);
            }
        }
    }
    TAX_TYPES
        .iter()
        .filter_map(|(tax_type, id, label)| {
            by_type.remove(tax_type).map(|mut forms| {
                forms.sort();
                TaxTypeChip {
                    tax_type: id,
                    label,
                    forms,
                }
            })
        })
        .collect()
}

/// Whether the on-screen form's package binds a box to the draft's
/// `/line_of_business` (its profile prefill).
fn binds_line_of_business(kind: AgentFormKind) -> bool {
    kind.definition().bindings.values().any(
        |bound| matches!(&bound.binding, Binding::Text { path, .. } if path == "/line_of_business"),
    )
}

/// The profile facts a form reads, in [`Fact::ALL`] order. Checked against
/// the readers: birth date (1700, 1701, 1701A, 1701Q prefills), income type
/// (the 2551Q's Item 13 rate question and the 1701's schedule choice), the
/// 8% or graduated choice (2551Q, 1701Q, 1701), EOPT tier (every queueable
/// form's penalties, and the 2550Q's taxpayer classification), business
/// start (2551Q initial quarter, 1702RT and 1702EX incorporation date),
/// government withholding entity (0619E, 0619F), usual ATC codes (1701Q,
/// 1701) and line of business (every package that binds a box to
/// `/line_of_business`).
pub fn facts_used_by(form_code: &str) -> Vec<Fact> {
    let code = canonical_form_code(form_code);
    let kind = AgentFormKind::from_code(&code).filter(|kind| kind.form_code() == code);
    Fact::ALL
        .into_iter()
        .filter(|fact| match fact {
            Fact::BirthDate => matches!(code.as_str(), "1700" | "1701" | "1701A" | "1701Q"),
            Fact::TaxClassification => matches!(code.as_str(), "2551Q" | "1701"),
            Fact::IncomeTaxElection => matches!(code.as_str(), "2551Q" | "1701Q" | "1701"),
            Fact::EoptTier => {
                (kind.is_some() && crate::forms::can_queue_for_submission(&code)) || code == "2550Q"
            }
            Fact::BusinessStartDate => matches!(code.as_str(), "2551Q" | "1702RT" | "1702EX"),
            Fact::IsGovernmentWithholdingEntity => matches!(code.as_str(), "0619E" | "0619F"),
            Fact::AtcCodes => matches!(code.as_str(), "1701Q" | "1701"),
            Fact::LineOfBusiness => kind.is_some_and(binds_line_of_business),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forms::registry::FORM_REGISTRY;

    /// Registry forms that give no tax type: event and one-off transaction
    /// returns, the TIN application, and the annual alphalist of
    /// compensation and final withholding (1604CF, which the old filter did
    /// not map).
    const TELLS_NOTHING: [&str; 11] = [
        "1604CF", "1706", "1707", "1707A", "1800", "1801", "1905", "2000", "2000OT", "2552", "2553",
    ];

    #[test]
    fn every_registry_form_maps_to_a_tax_type_or_is_listed_as_telling_nothing() {
        let mut nothing: Vec<&str> = FORM_REGISTRY
            .iter()
            .map(|form| form.code)
            .filter(|code| tax_type_of(code).is_none())
            .collect();
        nothing.sort_unstable();
        assert_eq!(nothing, TELLS_NOTHING);
        for (tax_type, _, _) in TAX_TYPES {
            assert!(
                FORM_REGISTRY
                    .iter()
                    .any(|form| tax_type_of(form.code) == Some(tax_type.clone())),
                "{tax_type:?} has no registry form"
            );
        }
    }

    #[test]
    fn a_years_forms_give_chips_in_order_with_their_forms() {
        let chips = tax_types_for_year(["1701q", "2550Q", "1701", "1601-C", "2000", "2550Q"]);
        let shown: Vec<(&str, &str, Vec<&str>)> = chips
            .iter()
            .map(|chip| {
                (
                    chip.tax_type,
                    chip.label,
                    chip.forms.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            shown,
            vec![
                ("value_added_tax", "VAT-registered", vec!["2550Q"]),
                (
                    "withholding_compensation",
                    "Withholds on compensation",
                    vec!["1601C"]
                ),
                ("income_tax", "Income tax", vec!["1701", "1701Q"]),
            ]
        );
        assert!(tax_types_for_year(Vec::<String>::new()).is_empty());
    }

    #[test]
    fn the_facts_each_form_reads() {
        assert_eq!(
            facts_used_by("2551Q"),
            vec![
                Fact::TaxClassification,
                Fact::IncomeTaxElection,
                Fact::EoptTier,
                Fact::BusinessStartDate,
            ],
            "the 2551Q package binds no line of business"
        );
        assert_eq!(
            facts_used_by("1701"),
            vec![
                Fact::BirthDate,
                Fact::TaxClassification,
                Fact::IncomeTaxElection,
                Fact::EoptTier,
                Fact::LineOfBusiness,
                Fact::AtcCodes,
            ]
        );
        assert_eq!(
            facts_used_by("0619E"),
            vec![
                Fact::EoptTier,
                Fact::LineOfBusiness,
                Fact::IsGovernmentWithholdingEntity,
            ]
        );
        assert!(facts_used_by("1702RT").contains(&Fact::BusinessStartDate));
        assert!(!facts_used_by("1702RT").contains(&Fact::LineOfBusiness));
        assert!(facts_used_by("1905").is_empty(), "not an on-screen form");
        // Line of business comes from the packages: exactly the ones that
        // bind a box to it.
        let with_line: Vec<&str> = AgentFormKind::all()
            .into_iter()
            .map(AgentFormKind::form_code)
            .filter(|code| facts_used_by(code).contains(&Fact::LineOfBusiness))
            .collect();
        assert!(with_line.contains(&"1601C") && with_line.contains(&"0605"));
        assert!(!with_line.contains(&"2551Q") && !with_line.contains(&"2550Q"));
    }
}
