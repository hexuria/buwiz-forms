//! The tax facts a year's forms read (`nsh/lib/facts.ts`): the words the Tax profile page, the
//! right panel and the New return dialog use. The facts themselves, who holds each and which
//! forms read them come from the host (`year_facts`); this file only names them. Pure.
//!
//! `facts.test.ts` is mirrored in the tests below.

use serde_json::Value;

/// The facts a year's forms can read (`FactName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FactName {
    BirthDate,
    TaxClassification,
    IncomeTaxElection,
    EoptTier,
    BusinessStartDate,
    LineOfBusiness,
    IsGovernmentWithholdingEntity,
    AtcCodes,
}

impl FactName {
    /// Every fact, in declaration order.
    pub const ALL: [FactName; 8] = [
        FactName::BirthDate,
        FactName::TaxClassification,
        FactName::IncomeTaxElection,
        FactName::EoptTier,
        FactName::BusinessStartDate,
        FactName::LineOfBusiness,
        FactName::IsGovernmentWithholdingEntity,
        FactName::AtcCodes,
    ];

    /// The fact's wire name, as the host's `FactValues` keys it.
    pub fn wire(self) -> &'static str {
        match self {
            FactName::BirthDate => "birth_date",
            FactName::TaxClassification => "tax_classification",
            FactName::IncomeTaxElection => "income_tax_election",
            FactName::EoptTier => "eopt_tier",
            FactName::BusinessStartDate => "business_start_date",
            FactName::LineOfBusiness => "line_of_business",
            FactName::IsGovernmentWithholdingEntity => "is_government_withholding_entity",
            FactName::AtcCodes => "atc_codes",
        }
    }

    /// The fact from its wire name.
    pub fn from_wire(name: &str) -> Option<FactName> {
        FactName::ALL.into_iter().find(|f| f.wire() == name)
    }
}

/// The year's facts as the host sends them (`FactValues`): a map of wire name to value. A fact
/// not set is missing, null or empty.
pub type FactValues = serde_json::Map<String, Value>;

/// What the forms read for a fact, per fact (`NotSetView`): the forms (codes) that use each
/// not-set fact.
#[derive(Debug, Clone, Default)]
pub struct NotSetView {
    pub not_set: Vec<FactName>,
    /// fact wire name -> form codes that read it.
    pub used_by: Vec<(String, Vec<String>)>,
}

/// The election readiness the host gives (`ElectionReadiness`): "ready" when the rate can be
/// recorded.
#[derive(Debug, Clone, Default)]
pub struct ElectionReadiness {
    pub gap: Option<String>,
}

/// Each fact as the page names it (`FACT_LABELS`).
pub fn fact_label(fact: FactName) -> &'static str {
    match fact {
        FactName::BirthDate => "Birth date",
        FactName::TaxClassification => "Income type",
        FactName::IncomeTaxElection => "Income-tax rate",
        FactName::EoptTier => "EOPT tier",
        FactName::BusinessStartDate => "Business start date",
        FactName::LineOfBusiness => "Line of business",
        FactName::IsGovernmentWithholdingEntity => "Government withholding entity",
        FactName::AtcCodes => "Usual ATC codes",
    }
}

/// What a return does with a fact, for the line that says it is not set (`FACT_WHY`).
pub fn fact_why(fact: FactName) -> &'static str {
    match fact {
        FactName::BirthDate => "It goes on this return's taxpayer details.",
        FactName::TaxClassification => "It decides which parts of this return apply.",
        FactName::IncomeTaxElection => "It decides the income-tax rate on this return.",
        FactName::EoptTier => "It sets the penalty rates on this return.",
        FactName::BusinessStartDate => "This return asks for it.",
        FactName::LineOfBusiness => "It goes on this return.",
        FactName::IsGovernmentWithholdingEntity => "It decides how this return is filled in.",
        FactName::AtcCodes => "It fills the tax codes on this return.",
    }
}

/// The facts the Facts section lists, in order (`ROW_FACTS`): birth date, line of business and
/// start date have their own sections.
pub const ROW_FACTS: [FactName; 5] = [
    FactName::TaxClassification,
    FactName::IncomeTaxElection,
    FactName::EoptTier,
    FactName::IsGovernmentWithholdingEntity,
    FactName::AtcCodes,
];

/// The income types a taxpayer type chooses from, as bir-core names them (`incomeTypeChoices`).
pub fn income_type_choices(taxpayer_type: Option<&str>) -> Vec<(&'static str, &'static str)> {
    match taxpayer_type {
        Some("Individual") => vec![
            ("SelfEmployed", "Self-employed"),
            ("MixedIncome", "Mixed income"),
            ("PurelyCompensation", "Compensation only"),
        ],
        Some("Cooperative") => vec![
            ("CooperativeTaxable", "Taxable cooperative"),
            ("CooperativeExempt", "Tax-exempt cooperative"),
            ("CooperativeMixed", "Mixed-income cooperative"),
        ],
        Some("Estate") | Some("Trust") => vec![("EstateOrTrust", "Estate or trust")],
        _ => vec![("Corporation", "Corporation or partnership")],
    }
}

/// Every income type's label, by its value (`INCOME_TYPE_LABELS`).
fn income_type_label(value: &str) -> Option<&'static str> {
    ["Individual", "Cooperative", "Estate", "Corporation"]
        .iter()
        .flat_map(|t| income_type_choices(Some(t)))
        .find(|(v, _)| *v == value)
        .map(|(_, l)| l)
}

/// The four EOPT tiers, as (value, label) pairs (`EOPT_TIERS`).
pub const EOPT_TIERS: [(&str, &str); 4] = [
    ("Micro", "Micro"),
    ("Small", "Small"),
    ("Medium", "Medium"),
    ("Large", "Large"),
];

/// The two choices of the rate row; a graduated method refines "Graduated" (`ELECTION_CHOICES`).
pub const ELECTION_CHOICES: [(&str, &str); 2] = [
    ("EightPercent", "8%"),
    ("GraduatedUnspecified", "Graduated rates"),
];

/// Every rate method's label, by its value (`ELECTION_LABELS`).
fn election_label(value: &str) -> Option<&'static str> {
    Some(match value {
        "EightPercent" => "8%",
        "GraduatedUnspecified" => "Graduated rates",
        "GraduatedOsd" => "Graduated rates, optional standard deduction",
        "GraduatedItemized" => "Graduated rates, itemized deductions",
        _ => return None,
    })
}

/// A fact's value in words, or None when it is not set (`factValue`).
pub fn fact_value(fact: FactName, facts: Option<&FactValues>, year: i64) -> Option<String> {
    let v = facts.and_then(|f| f.get(fact.wire()));
    if fact == FactName::AtcCodes {
        return v
            .and_then(|v| v.as_array())
            .filter(|a| !a.is_empty())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            });
    }
    let v = v?;
    if v.is_null() {
        return None;
    }
    if let Some(s) = v.as_str() {
        if s.is_empty() {
            return None;
        }
    }
    match fact {
        FactName::TaxClassification => Some(
            v.as_str()
                .and_then(|s| {
                    income_type_label(s)
                        .map(str::to_owned)
                        .or_else(|| Some(s.to_owned()))
                })
                .unwrap_or_else(|| v.to_string()),
        ),
        FactName::IncomeTaxElection => Some(format!(
            "{} for {}",
            v.as_str()
                .and_then(|s| election_label(s)
                    .map(str::to_owned)
                    .or_else(|| Some(s.to_owned())))
                .unwrap_or_else(|| v.to_string()),
            year
        )),
        FactName::IsGovernmentWithholdingEntity => {
            // `v ? "Yes" : "No"`: false is No, not not set (JS truthiness on the JSON value).
            Some(if js_truthy(v) {
                "Yes".to_owned()
            } else {
                "No".to_owned()
            })
        }
        _ => {
            if let Some(s) = v.as_str() {
                Some(s.to_owned())
            } else {
                Some(v.to_string())
            }
        }
    }
}

/// JS truthiness of a JSON value (`v ? … : …`): false, null, "" and 0 are falsy; everything else
/// (including empty arrays/objects) is truthy.
fn js_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

/// Where the year's rate was chosen, in words (`electionSourceText`).
pub fn election_source_text(source: Option<&str>) -> Option<String> {
    let source = source?;
    if source.is_empty() {
        return None;
    }
    Some(if source == "Tax profile" {
        "Set on this profile".to_owned()
    } else {
        format!("Chosen on {source}")
    })
}

/// The forms that read a fact, as chip labels (`readerChips`): each code, or one chip that counts
/// them when there are many ("penalties on every form" for the EOPT tier).
pub fn reader_chips(fact: FactName, codes: Option<&[String]>) -> Vec<String> {
    let list = codes.unwrap_or(&[]);
    if list.len() <= 4 {
        return list.to_vec();
    }
    vec![if fact == FactName::EoptTier {
        "penalties on every form".to_owned()
    } else {
        format!("{} forms", list.len())
    }]
}

/// A, A and B, A, B and C (`joinNames`).
pub fn join_names(names: &[&str]) -> String {
    match names.len() {
        0 => String::new(),
        1 => names[0].to_owned(),
        _ => format!(
            "{} and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

/// The host's `year_facts` view as a `NotSetView` (`store.yearFacts`'s shape): null when the read
/// failed or the view is not an object.
pub fn not_set_view(view: &Value) -> Option<NotSetView> {
    if !view.is_object() {
        return None;
    }
    let not_set = view
        .get("not_set")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .filter_map(FactName::from_wire)
                .collect()
        })
        .unwrap_or_default();
    let used_by = view
        .get("used_by")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    Some((
                        k.clone(),
                        v.as_array()?
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect::<Vec<_>>(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Some(NotSetView { not_set, used_by })
}

/// The host's `income_tax_election_readiness` answer as an `ElectionReadiness`; null on failure.
pub fn readiness(view: &Value) -> Option<ElectionReadiness> {
    if !view.is_object() {
        return None;
    }
    Some(ElectionReadiness {
        gap: view.get("gap").and_then(Value::as_str).map(str::to_owned),
    })
}

/// The whole `year_facts` view the Facts section reads (the fields Facts.tsx uses): null when the
/// read failed.
#[derive(Debug, Clone, Default)]
pub struct YearFactsView {
    pub not_set: Vec<FactName>,
    /// fact wire name -> form codes that read it.
    pub used_by: Vec<(String, Vec<String>)>,
    /// The facts map itself (`view.facts`).
    pub facts: FactValues,
    /// fact wire name -> "head_office" | "branch" (birth_date has no entry).
    pub owner: Vec<(String, String)>,
    pub election_locked: bool,
    pub election_source: Option<String>,
    pub tin: String,
    pub year: i64,
}

/// The host's `year_facts` answer as a `YearFactsView`; null on failure.
pub fn year_facts_view(view: &Value) -> Option<YearFactsView> {
    if !view.is_object() {
        return None;
    }
    let base = not_set_view(view)?;
    let facts = view
        .get("facts")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let owner = view
        .get("owner")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|o| (k.clone(), o.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    Some(YearFactsView {
        not_set: base.not_set,
        used_by: base.used_by,
        facts,
        owner,
        election_locked: view
            .get("election_locked")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        election_source: view
            .get("election_source")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tin: view
            .get("tin")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        year: view.get("year").and_then(Value::as_i64).unwrap_or(0),
    })
}

impl YearFactsView {
    /// The codes that read a fact (`view.used_by[fact]`).
    pub fn readers(&self, fact: FactName) -> &[String] {
        self.used_by
            .iter()
            .find(|(k, _)| k == fact.wire())
            .map(|(_, v)| v.as_slice())
            .unwrap_or(&[])
    }

    /// Who holds a fact ("head_office" | "branch"); None for birth_date.
    pub fn owner_of(&self, fact: FactName) -> Option<&str> {
        self.owner
            .iter()
            .find(|(k, _)| k == fact.wire())
            .map(|(_, v)| v.as_str())
    }
}

/// The section's summary (`factsMeta` in logic.ts).
pub fn facts_meta(has_year: bool, shown: usize, not_set: usize) -> String {
    if !has_year {
        return "Add a tax year first".to_owned();
    }
    if shown == 0 {
        return "No form on your list reads one".to_owned();
    }
    if not_set > 0 {
        format!("{not_set} not set")
    } else {
        "All set".to_owned()
    }
}

/// The line under the facts (`hiddenFactsText` in logic.ts).
pub fn hidden_facts_text(hidden: usize) -> Option<String> {
    if hidden == 0 {
        return None;
    }
    Some(if hidden == 1 {
        "1 fact is not shown: no form on your list uses it.".to_owned()
    } else {
        format!("{hidden} facts are not shown: no form on your list uses them.")
    })
}

/// "WC160, WI010" -> ["WC160","WI010"], or None when any code is not an ATC code
/// (`parseAtcCodes` in logic.ts).
pub fn parse_atc_codes(text: &str) -> Option<Vec<String>> {
    let codes: Vec<String> = text
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .map(|c| c.trim().to_uppercase())
        .filter(|c| !c.is_empty())
        .collect();
    // `[].every(..)` is true in JS: empty text parses to an empty list (it clears the codes).
    let ok = codes.iter().all(|c| {
        // /^[A-Z]{2}\d{3}[A-Z0-9]*$/
        let b = c.as_bytes();
        b.len() >= 5
            && b[..2].iter().all(u8::is_ascii_uppercase)
            && b[2..5].iter().all(u8::is_ascii_digit)
            && b[5..]
                .iter()
                .all(|x| x.is_ascii_uppercase() || x.is_ascii_digit())
    });
    if ok { Some(codes) } else { None }
}

/// The facts a form reads that are not set for the year (`notSetForForm`). The rate is left out
/// when the taxpayer does not choose one (the readiness says it is not an individual with
/// business income).
pub fn not_set_for_form(
    view: Option<&NotSetView>,
    code: &str,
    readiness: Option<&ElectionReadiness>,
) -> Vec<FactName> {
    let Some(view) = view else {
        return Vec::new();
    };
    view.not_set
        .iter()
        .copied()
        .filter(|f| {
            let used = view
                .used_by
                .iter()
                .find(|(name, _)| name == f.wire())
                .map(|(_, codes)| codes.iter().any(|c| c == code))
                .unwrap_or(false);
            used && !(*f == FactName::IncomeTaxElection
                && readiness.is_some_and(|r| r.gap.as_deref() != Some("ready")))
        })
        .collect()
}

/// The one line that names the facts a return reads that are not set, or None
/// (`notSetSentence`).
pub fn not_set_sentence(facts: &[FactName]) -> Option<String> {
    if facts.is_empty() {
        return None;
    }
    if facts.len() == 1 {
        let f = facts[0];
        return Some(format!("{} is not set. {}", fact_label(f), fact_why(f)));
    }
    // "EOPT tier" keeps its capitals after the first name; "Birth date" does not.
    let names: Vec<String> = facts
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let label = fact_label(*f);
            if i == 0 {
                label.to_owned()
            } else {
                let lower = label
                    .chars()
                    .next()
                    .map(|c| c.to_lowercase().collect::<String>())
                    .unwrap_or_default();
                if label.len() >= 2 && label.chars().nth(1).is_some_and(|c| c.is_uppercase()) {
                    label.to_owned()
                } else {
                    format!(
                        "{lower}{}",
                        &label[label.chars().next().map(char::len_utf8).unwrap_or(1)..]
                    )
                }
            }
        })
        .collect();
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    Some(format!(
        "{} are not set. This return reads them.",
        join_names(&name_refs)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn facts(pairs: &[(&str, Value)]) -> FactValues {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn t_fact_value_income_type_and_unknown() {
        let f = facts(&[("tax_classification", json!("SelfEmployed"))]);
        assert_eq!(
            fact_value(FactName::TaxClassification, Some(&f), 2026).as_deref(),
            Some("Self-employed")
        );
        let f = facts(&[("tax_classification", json!("EstateOrTrust"))]);
        assert_eq!(
            fact_value(FactName::TaxClassification, Some(&f), 2026).as_deref(),
            Some("Estate or trust")
        );
        let f = facts(&[("tax_classification", json!("Something"))]);
        assert_eq!(
            fact_value(FactName::TaxClassification, Some(&f), 2026).as_deref(),
            Some("Something")
        );
    }

    #[test]
    fn t_fact_value_rate_with_year() {
        let f = facts(&[("income_tax_election", json!("EightPercent"))]);
        assert_eq!(
            fact_value(FactName::IncomeTaxElection, Some(&f), 2026).as_deref(),
            Some("8% for 2026")
        );
        let f = facts(&[("income_tax_election", json!("GraduatedOsd"))]);
        assert_eq!(
            fact_value(FactName::IncomeTaxElection, Some(&f), 2026).as_deref(),
            Some("Graduated rates, optional standard deduction for 2026")
        );
        let f = facts(&[("income_tax_election", json!("Other"))]);
        assert_eq!(
            fact_value(FactName::IncomeTaxElection, Some(&f), 2026).as_deref(),
            Some("Other for 2026")
        );
    }

    #[test]
    fn t_fact_value_yes_no_and_atc() {
        let f = facts(&[("is_government_withholding_entity", json!(true))]);
        assert_eq!(
            fact_value(FactName::IsGovernmentWithholdingEntity, Some(&f), 2026).as_deref(),
            Some("Yes")
        );
        let f = facts(&[("is_government_withholding_entity", json!(false))]);
        assert_eq!(
            fact_value(FactName::IsGovernmentWithholdingEntity, Some(&f), 2026).as_deref(),
            Some("No")
        );
        let f = facts(&[("atc_codes", json!(["II011", "WI010"]))]);
        assert_eq!(
            fact_value(FactName::AtcCodes, Some(&f), 2026).as_deref(),
            Some("II011, WI010")
        );
        let f = facts(&[("atc_codes", json!([]))]);
        assert!(fact_value(FactName::AtcCodes, Some(&f), 2026).is_none());
        let f = facts(&[("atc_codes", Value::Null)]);
        assert!(fact_value(FactName::AtcCodes, Some(&f), 2026).is_none());
        let f = facts(&[]);
        assert!(fact_value(FactName::AtcCodes, Some(&f), 2026).is_none());
        assert!(fact_value(FactName::AtcCodes, None, 2026).is_none());
    }

    #[test]
    fn t_fact_value_missing_null_empty() {
        let f = facts(&[]);
        assert!(fact_value(FactName::BirthDate, Some(&f), 2026).is_none());
        let f = facts(&[("birth_date", Value::Null)]);
        assert!(fact_value(FactName::BirthDate, Some(&f), 2026).is_none());
        let f = facts(&[("birth_date", json!(""))]);
        assert!(fact_value(FactName::BirthDate, Some(&f), 2026).is_none());
        assert!(fact_value(FactName::BirthDate, None, 2026).is_none());
    }

    #[test]
    fn t_fact_value_other_facts_as_text() {
        let f = facts(&[
            ("birth_date", json!("2000-01-02")),
            ("eopt_tier", json!("Micro")),
            ("line_of_business", json!("Retail")),
        ]);
        assert_eq!(
            fact_value(FactName::BirthDate, Some(&f), 2026).as_deref(),
            Some("2000-01-02")
        );
        assert_eq!(
            fact_value(FactName::EoptTier, Some(&f), 2026).as_deref(),
            Some("Micro")
        );
        assert_eq!(
            fact_value(FactName::LineOfBusiness, Some(&f), 2026).as_deref(),
            Some("Retail")
        );
    }

    #[test]
    fn t_election_source_text() {
        assert!(election_source_text(None).is_none());
        assert!(election_source_text(Some("")).is_none());
        assert_eq!(
            election_source_text(Some("Tax profile")).as_deref(),
            Some("Set on this profile")
        );
        assert_eq!(
            election_source_text(Some("2551Q Q1")).as_deref(),
            Some("Chosen on 2551Q Q1")
        );
    }

    #[test]
    fn t_reader_chips_boundary() {
        let codes = |n: usize| (0..n).map(|i| format!("F{i}")).collect::<Vec<_>>();
        assert_eq!(
            reader_chips(FactName::TaxClassification, Some(&codes(2))),
            vec!["F0", "F1"]
        );
        assert_eq!(
            reader_chips(FactName::TaxClassification, Some(&codes(4))).len(),
            4
        );
        assert_eq!(
            reader_chips(FactName::TaxClassification, Some(&codes(5))),
            vec!["5 forms"]
        );
        assert_eq!(
            reader_chips(FactName::EoptTier, Some(&codes(5))),
            vec!["penalties on every form"]
        );
        assert!(reader_chips(FactName::BirthDate, Some(&[])).is_empty());
        assert!(reader_chips(FactName::BirthDate, None).is_empty());
    }

    #[test]
    fn t_join_names() {
        assert_eq!(join_names(&[]), "");
        assert_eq!(join_names(&["A"]), "A");
        assert_eq!(join_names(&["A", "B"]), "A and B");
        assert_eq!(join_names(&["A", "B", "C"]), "A, B and C");
        assert_eq!(join_names(&["A", "B", "C", "D"]), "A, B, C and D");
    }

    #[test]
    fn t_income_type_choices() {
        assert_eq!(
            income_type_choices(Some("Individual")),
            vec![
                ("SelfEmployed", "Self-employed"),
                ("MixedIncome", "Mixed income"),
                ("PurelyCompensation", "Compensation only"),
            ]
        );
        assert_eq!(
            income_type_choices(Some("Cooperative")),
            vec![
                ("CooperativeTaxable", "Taxable cooperative"),
                ("CooperativeExempt", "Tax-exempt cooperative"),
                ("CooperativeMixed", "Mixed-income cooperative"),
            ]
        );
        assert_eq!(
            income_type_choices(Some("Estate")),
            vec![("EstateOrTrust", "Estate or trust")]
        );
        assert_eq!(
            income_type_choices(Some("Trust")),
            vec![("EstateOrTrust", "Estate or trust")]
        );
        assert_eq!(
            income_type_choices(Some("Corporation")),
            vec![("Corporation", "Corporation or partnership")]
        );
        assert_eq!(
            income_type_choices(None),
            vec![("Corporation", "Corporation or partnership")]
        );
    }

    #[test]
    fn t_constant_lists() {
        assert_eq!(
            EOPT_TIERS.map(|(v, _)| v),
            ["Micro", "Small", "Medium", "Large"]
        );
        assert_eq!(
            ELECTION_CHOICES.map(|(v, _)| v),
            ["EightPercent", "GraduatedUnspecified"]
        );
        assert_eq!(FactName::ALL.len(), 8);
        assert_eq!(ROW_FACTS.len(), 5);
        assert_eq!(ROW_FACTS[0], FactName::TaxClassification);
    }

    fn view(not_set: &[FactName], used_by: &[(&str, &[&str])]) -> NotSetView {
        NotSetView {
            not_set: not_set.to_vec(),
            used_by: used_by
                .iter()
                .map(|(f, codes)| (f.to_string(), codes.iter().map(|c| c.to_string()).collect()))
                .collect(),
        }
    }

    #[test]
    fn t_not_set_for_form() {
        let v = view(
            &[FactName::BirthDate, FactName::EoptTier],
            &[("birth_date", &["1701"]), ("eopt_tier", &["2551Q", "1701"])],
        );
        assert_eq!(
            not_set_for_form(Some(&v), "1701", None),
            vec![FactName::BirthDate, FactName::EoptTier]
        );
        assert_eq!(
            not_set_for_form(Some(&v), "2551Q", None),
            vec![FactName::EoptTier]
        );
        assert!(not_set_for_form(Some(&v), "9999", None).is_empty());
        // The rate is left out when the taxpayer does not choose one.
        let rate = view(
            &[FactName::IncomeTaxElection, FactName::BirthDate],
            &[
                ("income_tax_election", &["2551Q"]),
                ("birth_date", &["2551Q"]),
            ],
        );
        let ready = ElectionReadiness {
            gap: Some("ready".to_owned()),
        };
        let not_ready = ElectionReadiness {
            gap: Some("not an individual with business income".to_owned()),
        };
        assert_eq!(
            not_set_for_form(Some(&rate), "2551Q", Some(&ready)),
            vec![FactName::IncomeTaxElection, FactName::BirthDate]
        );
        assert_eq!(
            not_set_for_form(Some(&rate), "2551Q", Some(&not_ready)),
            vec![FactName::BirthDate]
        );
        assert_eq!(
            not_set_for_form(Some(&rate), "2551Q", None),
            vec![FactName::IncomeTaxElection, FactName::BirthDate]
        );
        assert!(not_set_for_form(None, "1701", None).is_empty());
        assert!(not_set_for_form(Some(&NotSetView::default()), "1701", None).is_empty());
    }

    #[test]
    fn t_not_set_sentence() {
        assert!(not_set_sentence(&[]).is_none());
        assert_eq!(
            not_set_sentence(&[FactName::BirthDate]).as_deref(),
            Some("Birth date is not set. It goes on this return's taxpayer details.")
        );
        assert_eq!(
            not_set_sentence(&[FactName::BirthDate, FactName::EoptTier]).as_deref(),
            Some("Birth date and EOPT tier are not set. This return reads them.")
        );
        assert_eq!(
            not_set_sentence(&[FactName::EoptTier, FactName::BirthDate]).as_deref(),
            Some("EOPT tier and birth date are not set. This return reads them.")
        );
        assert_eq!(
            not_set_sentence(&[
                FactName::TaxClassification,
                FactName::BirthDate,
                FactName::AtcCodes
            ])
            .as_deref(),
            Some(
                "Income type, birth date and usual ATC codes are not set. This return reads them."
            )
        );
        assert_eq!(
            not_set_sentence(&[FactName::LineOfBusiness, FactName::EoptTier]).as_deref(),
            Some("Line of business and EOPT tier are not set. This return reads them.")
        );
    }

    #[test]
    fn t_facts_meta_and_hidden() {
        assert_eq!(facts_meta(false, 0, 0), "Add a tax year first");
        assert_eq!(facts_meta(true, 0, 0), "No form on your list reads one");
        assert_eq!(facts_meta(true, 3, 2), "2 not set");
        assert_eq!(facts_meta(true, 3, 0), "All set");
        assert_eq!(hidden_facts_text(0), None);
        assert_eq!(
            hidden_facts_text(1).as_deref(),
            Some("1 fact is not shown: no form on your list uses it.")
        );
        assert_eq!(
            hidden_facts_text(2).as_deref(),
            Some("2 facts are not shown: no form on your list uses them.")
        );
    }

    #[test]
    fn t_parse_atc_codes() {
        assert_eq!(
            parse_atc_codes("wc160, wi010").unwrap(),
            vec!["WC160".to_owned(), "WI010".to_owned()]
        );
        assert_eq!(
            parse_atc_codes("WC160;WI010  PT010").unwrap(),
            vec!["WC160".to_owned(), "WI010".to_owned(), "PT010".to_owned()]
        );
        assert_eq!(parse_atc_codes(""), Some(vec![]));
        assert!(parse_atc_codes("W0160").is_none());
        assert!(parse_atc_codes("WC16").is_none());
        assert!(parse_atc_codes("WC160-").is_none());
    }

    #[test]
    fn t_year_facts_view() {
        let v = json!({
            "tin": "00012345300000",
            "year": 2026,
            "facts": {"eopt_tier": "Micro", "atc_codes": ["WC160"]},
            "owner": {"eopt_tier": "head_office"},
            "used_by": {"eopt_tier": ["2551Q", "1701"]},
            "not_set": ["birth_date"],
            "election_locked": true,
            "election_source": "2551Q Q1"
        });
        let view = year_facts_view(&v).unwrap();
        assert_eq!(view.tin, "00012345300000");
        assert_eq!(view.year, 2026);
        assert_eq!(
            view.readers(FactName::EoptTier),
            &["2551Q".to_owned(), "1701".to_owned()]
        );
        assert!(view.readers(FactName::BirthDate).is_empty());
        assert_eq!(view.owner_of(FactName::EoptTier), Some("head_office"));
        assert_eq!(view.owner_of(FactName::BirthDate), None);
        assert!(view.election_locked);
        assert_eq!(view.election_source.as_deref(), Some("2551Q Q1"));
        assert_eq!(view.not_set, vec![FactName::BirthDate]);
        assert_eq!(
            fact_value(FactName::EoptTier, Some(&view.facts), 2026).as_deref(),
            Some("Micro")
        );
        assert!(year_facts_view(&json!(null)).is_none());
        assert!(year_facts_view(&json!([])).is_none());
    }
}
