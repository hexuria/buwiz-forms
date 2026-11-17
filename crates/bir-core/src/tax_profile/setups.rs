//! Setups (owner decision 14): a forms list and the tax facts a new
//! taxpayer, or a new tax year, can start from.
//!
//! Built-in setups live here, in code, and replace the old starter packs.
//! A user's own setups are saved from a branch's year into the `setups`
//! table (migration v24). A setup never holds a date, a line of business or
//! anything personal: no name, TIN, address or contact. Applying one sets
//! the year's forms list as hand-picked (an "adds" setup such as Employer
//! adds its forms to the list instead) and the facts it holds, never over a
//! fact already set unless the caller says so, and never over a locked
//! 8% or graduated choice.

use serde::{Deserialize, Serialize};

use crate::profile::{TaxClassification, TaxpayerType};

use super::year_facts::{Fact, YearFacts, YearFactsPatch};

/// The id prefix of a user's own setup: `user-<row id>`.
pub const USER_SETUP_PREFIX: &str = "user-";

/// One setup: the forms of a year and the facts the forms read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Setup {
    pub id: String,
    pub name: String,
    /// The taxpayer types the setup is for.
    pub taxpayer_types: Vec<TaxpayerType>,
    /// Canonical form codes.
    pub forms: Vec<String>,
    /// No dates and no line of business.
    pub facts: YearFacts,
    pub built_in: bool,
    /// The forms are added to the year's list instead of replacing it
    /// (Employer: it goes with whatever else is picked).
    pub adds_forms: bool,
}

/// What `setups.body_json` holds for a user's setup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetupBody {
    pub taxpayer_types: Vec<TaxpayerType>,
    pub forms: Vec<String>,
    pub facts: YearFacts,
}

impl Setup {
    /// Whether the setup is for `taxpayer_type`.
    pub fn fits(&self, taxpayer_type: &TaxpayerType) -> bool {
        self.taxpayer_types.contains(taxpayer_type)
    }

    /// A user's setup from its stored row.
    pub fn from_row(id: i64, name: String, body: SetupBody) -> Self {
        Self {
            id: format!("{USER_SETUP_PREFIX}{id}"),
            name,
            taxpayer_types: body.taxpayer_types,
            forms: body.forms,
            facts: setup_facts(&body.facts),
            built_in: false,
            adds_forms: false,
        }
    }

    pub fn body(&self) -> SetupBody {
        SetupBody {
            taxpayer_types: self.taxpayer_types.clone(),
            forms: self.forms.clone(),
            facts: setup_facts(&self.facts),
        }
    }
}

/// The row id a user setup's id names, or `None` for a built-in id.
pub fn user_setup_row(id: &str) -> Option<i64> {
    id.strip_prefix(USER_SETUP_PREFIX)?.parse().ok()
}

/// The facts a setup may hold: `facts` without the start date, the line of
/// business, and the year's 8% or graduated choice. That choice is each
/// taxpayer's own, made on the year's first return: a saved setup or a copy
/// from another taxpayer must never record it for somebody else.
pub fn setup_facts(facts: &YearFacts) -> YearFacts {
    YearFacts {
        business_start_date: None,
        line_of_business: None,
        income_tax_election: None,
        election_source: None,
        ..facts.clone()
    }
}

fn built_in(
    id: &str,
    name: &str,
    taxpayer_types: &[TaxpayerType],
    forms: &[&str],
    tax_classification: Option<TaxClassification>,
    adds_forms: bool,
) -> Setup {
    Setup {
        id: id.to_string(),
        name: name.to_string(),
        taxpayer_types: taxpayer_types.to_vec(),
        forms: forms.iter().map(|code| code.to_string()).collect(),
        facts: YearFacts {
            tax_classification,
            ..YearFacts::default()
        },
        built_in: true,
        adds_forms,
    }
}

/// The built-in setups, in the order they are offered.
///
/// Each form's registry entry (`forms::registry::FORM_REGISTRY`) decides
/// the taxpayer types: 2550Q, 2551Q, 1601C and 1604C are for individuals,
/// corporations and partnerships only (not cooperatives, estates or
/// trusts), 1701Q and 1701 for individuals, estates and trusts, 1702Q and
/// 1702RT for corporations, partnerships and cooperatives.
pub fn built_in_setups() -> Vec<Setup> {
    use TaxpayerType::{Corporation, Individual, Partnership};
    vec![
        built_in(
            "non_vat_professional",
            "Non-VAT professional",
            &[Individual],
            &["2551Q", "1701Q", "1701"],
            Some(TaxClassification::SelfEmployed),
            false,
        ),
        built_in(
            "vat_business_individual",
            "VAT business",
            &[Individual],
            &["2550Q", "1701Q", "1701"],
            Some(TaxClassification::SelfEmployed),
            false,
        ),
        built_in(
            "vat_business_corporation",
            "VAT business",
            &[Corporation, Partnership],
            &["2550Q", "1702Q", "1702RT"],
            Some(TaxClassification::Corporation),
            false,
        ),
        built_in(
            "corporation_non_vat",
            "Corporation, non-VAT",
            &[Corporation, Partnership],
            &["2551Q", "1702Q", "1702RT"],
            Some(TaxClassification::Corporation),
            false,
        ),
        built_in(
            "employer",
            "Employer",
            &[Individual, Corporation, Partnership],
            &["1601C", "1604C"],
            None,
            true,
        ),
    ]
}

/// What applying a setup's facts does to a year whose facts are `current`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FactsPlan {
    /// The keys to write.
    pub patch: YearFactsPatch,
    /// Facts the setup sets (they were not set, or `replace` was given).
    pub set: Vec<Fact>,
    /// Facts already set to something else and left as they are: without
    /// `replace`, or the 8% or graduated choice of a locked year.
    pub kept: Vec<Fact>,
}

/// Plan the facts `setup` (a setup's facts) writes over `current`.
/// A fact the setup does not hold, or holds at the value already set, is
/// left alone. `locked`: the year's election cannot change.
pub fn plan_facts(
    setup: &YearFacts,
    current: &YearFacts,
    locked: bool,
    replace: bool,
) -> FactsPlan {
    let mut plan = FactsPlan::default();
    let mut decide = |fact: Fact, held: bool, same: bool, set_now: bool| -> bool {
        if !held || same {
            return false;
        }
        let blocked = fact == Fact::IncomeTaxElection && locked;
        if blocked || (set_now && !replace) {
            plan.kept.push(fact);
            false
        } else {
            plan.set.push(fact);
            true
        }
    };
    if decide(
        Fact::TaxClassification,
        setup.tax_classification.is_some(),
        setup.tax_classification == current.tax_classification,
        current.is_set(Fact::TaxClassification),
    ) {
        plan.patch.tax_classification = Some(setup.tax_classification.clone());
    }
    if decide(
        Fact::IncomeTaxElection,
        setup.income_tax_election.is_some(),
        setup.income_tax_election == current.income_tax_election,
        current.is_set(Fact::IncomeTaxElection),
    ) {
        plan.patch.income_tax_election = Some(setup.income_tax_election.clone());
    }
    if decide(
        Fact::EoptTier,
        setup.eopt_tier.is_some(),
        setup.eopt_tier == current.eopt_tier,
        current.is_set(Fact::EoptTier),
    ) {
        plan.patch.eopt_tier = Some(setup.eopt_tier.clone());
    }
    if decide(
        Fact::IsGovernmentWithholdingEntity,
        setup.is_government_withholding_entity.is_some(),
        setup.is_government_withholding_entity == current.is_government_withholding_entity,
        current.is_set(Fact::IsGovernmentWithholdingEntity),
    ) {
        plan.patch.is_government_withholding_entity = Some(setup.is_government_withholding_entity);
    }
    if decide(
        Fact::AtcCodes,
        !setup.atc_codes.is_empty(),
        setup.atc_codes == current.atc_codes,
        current.is_set(Fact::AtcCodes),
    ) {
        plan.patch.atc_codes = Some(setup.atc_codes.clone());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{EoptTier, IncomeTaxElection};

    #[test]
    fn built_in_setups_name_current_registry_forms_that_fit_their_taxpayer_types() {
        for setup in built_in_setups() {
            assert!(setup.built_in && user_setup_row(&setup.id).is_none());
            assert!(!setup.taxpayer_types.is_empty(), "{}", setup.id);
            for code in &setup.forms {
                let form = crate::forms::find_form(code)
                    .unwrap_or_else(|| panic!("{}: {code} is not in the registry", setup.id));
                assert_eq!(form.code, code, "{}: {code} is canonical", setup.id);
                assert!(form.deprecation_year().is_none(), "{}: {code}", setup.id);
                for taxpayer_type in &setup.taxpayer_types {
                    assert!(
                        form.taxpayer_types.contains(taxpayer_type),
                        "{}: {code} is not for a {taxpayer_type:?}",
                        setup.id
                    );
                }
            }
            assert_eq!(setup.facts.business_start_date, None);
            assert_eq!(setup.facts.line_of_business, None);
        }
    }

    #[test]
    fn built_in_setups_list_by_taxpayer_type() {
        let ids = |taxpayer_type: TaxpayerType| -> Vec<String> {
            built_in_setups()
                .into_iter()
                .filter(|setup| setup.fits(&taxpayer_type))
                .map(|setup| setup.id)
                .collect()
        };
        assert_eq!(
            ids(TaxpayerType::Individual),
            [
                "non_vat_professional",
                "vat_business_individual",
                "employer"
            ]
        );
        for taxpayer_type in [TaxpayerType::Corporation, TaxpayerType::Partnership] {
            assert_eq!(
                ids(taxpayer_type),
                [
                    "vat_business_corporation",
                    "corporation_non_vat",
                    "employer"
                ]
            );
        }
        for taxpayer_type in [
            TaxpayerType::Cooperative,
            TaxpayerType::Estate,
            TaxpayerType::Trust,
        ] {
            assert!(ids(taxpayer_type).is_empty());
        }
        let employer = built_in_setups()
            .into_iter()
            .find(|setup| setup.id == "employer")
            .unwrap();
        assert!(employer.adds_forms);
        assert_eq!(employer.facts, YearFacts::default());
    }

    #[test]
    fn a_plan_fills_what_is_not_set_and_keeps_what_is_unless_replaced() {
        let setup = YearFacts {
            tax_classification: Some(TaxClassification::SelfEmployed),
            income_tax_election: Some(IncomeTaxElection::EightPercent),
            eopt_tier: Some(EoptTier::Micro),
            is_government_withholding_entity: Some(false),
            atc_codes: vec!["PT010".into()],
            ..YearFacts::default()
        };
        let current = YearFacts {
            tax_classification: Some(TaxClassification::MixedIncome),
            is_government_withholding_entity: Some(false),
            ..YearFacts::default()
        };
        let plan = plan_facts(&setup, &current, false, false);
        assert_eq!(
            plan.set,
            [Fact::IncomeTaxElection, Fact::EoptTier, Fact::AtcCodes]
        );
        assert_eq!(plan.kept, [Fact::TaxClassification]);
        assert_eq!(plan.patch.tax_classification, None);
        assert_eq!(
            plan.patch.income_tax_election,
            Some(Some(IncomeTaxElection::EightPercent))
        );
        assert_eq!(
            plan.patch.is_government_withholding_entity, None,
            "the same value is left alone"
        );

        let replaced = plan_facts(&setup, &current, false, true);
        assert!(replaced.kept.is_empty());
        assert_eq!(
            replaced.patch.tax_classification,
            Some(Some(TaxClassification::SelfEmployed))
        );

        let locked = YearFacts {
            income_tax_election: Some(IncomeTaxElection::GraduatedUnspecified),
            ..current.clone()
        };
        let plan = plan_facts(&setup, &locked, true, true);
        assert_eq!(plan.kept, [Fact::IncomeTaxElection], "a locked election");
        assert_eq!(plan.patch.income_tax_election, None);
        let unset_locked = plan_facts(&setup, &current, true, false);
        assert_eq!(unset_locked.patch.income_tax_election, None);
        assert!(unset_locked.kept.contains(&Fact::IncomeTaxElection));
    }

    #[test]
    fn a_setup_body_holds_no_dates_and_no_line_of_business() {
        let facts = YearFacts {
            tax_classification: Some(TaxClassification::SelfEmployed),
            business_start_date: chrono::NaiveDate::from_ymd_opt(2020, 1, 6),
            line_of_business: Some("Bookkeeping".into()),
            election_source: Some("2551Q".into()),
            ..YearFacts::default()
        };
        let setup = Setup::from_row(
            7,
            "Mine".into(),
            SetupBody {
                taxpayer_types: vec![TaxpayerType::Individual],
                forms: vec!["2551Q".into()],
                facts,
            },
        );
        assert_eq!(setup.id, "user-7");
        assert_eq!(user_setup_row(&setup.id), Some(7));
        let body = serde_json::to_string(&setup.body()).unwrap();
        assert!(!body.contains("2020-01-06"));
        assert!(!body.contains("Bookkeeping"));
        assert!(!body.contains("election_source"));
    }
}
