//! PR 14 (decision P4-18): unticking the ticked member of a radio group of
//! three or more never fails an edit. This file is the sweep and its
//! recorder; it uses only what the base of PR 14 already had, so the same
//! sweep recorded the golden's `before` column on that base
//! (`fixtures/radio-untick-golden.json`). The assertions on the rule are in
//! `radio_untick_rule.rs`.
//!
//! The groups swept are not the host's own: they are each form's official
//! radio name groups, read from its runtime-control inventory (rules/forms/
//! <package>/fixtures/runtime-control-inventory-v796.json), restricted to
//! the boxes this app can fill, with every deliberate difference in a table
//! with its reason ([`JOINED`], [`HOST_ONLY`], [`NO_INVENTORY`]).
//! `radio_untick_rule.rs` asserts the host groups exactly these.
//!
//! Every member of every such group of three or more is ticked on a draft
//! whose prerequisites are answered first ([`seed`]), then unticked with
//! "false", and its group box, where it has one, is sent "". A member that still does not read
//! back ticked is listed in [`NEVER_TICKED`] with its reason; the rule's
//! tests fail on any other.

use super::*;

/// The official radio name groups of a form (`name` -> distinct control
/// ids, two or more), from its runtime-control inventory; `None` when the
/// rules corpus has no inventory for its package.
pub(super) fn official_name_groups(kind: AgentFormKind) -> Option<BTreeMap<String, Vec<String>>> {
    let path = format!(
        "{}/../../rules/forms/{}/fixtures/runtime-control-inventory-v796.json",
        env!("CARGO_MANIFEST_DIR"),
        kind.package()
    );
    let text = std::fs::read_to_string(&path).ok()?;
    let inventory: Value = serde_json::from_str(&text).expect("inventory json");
    let controls = inventory["controls"]
        .as_array()
        .or(inventory["static_controls"].as_array())
        .expect("inventory controls");
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for control in controls {
        let (Some(id), Some(name)) = (control["id"].as_str(), control["name"].as_str()) else {
            continue;
        };
        if control["control_kind"] != "radio" {
            continue;
        }
        let ids = names.entry(name.to_string()).or_default();
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_string());
        }
    }
    names.retain(|_, ids| ids.len() >= 2);
    Some(names)
}

/// Forms without an official inventory: their groups are the agent
/// definition's ([`definition_groups`]).
pub(super) const NO_INVENTORY: &[(&str, &str)] = &[
    (
        "1701MS",
        "the rules corpus has no runtime-control inventory for 1701ms-v2024",
    ),
    (
        "1701Q",
        "the rules corpus has no runtime-control inventory for 1701q-v2018",
    ),
];

/// Boxes that answer an official name group they are not drawn in:
/// `(form, key, group name, reason)`.
pub(super) const JOINED: &[(&str, &str, &str, &str)] = &[(
    "2200S",
    "frm2200S:chkPymntManner_3",
    "frm2200S:chkPymntManner",
    "item 17 is the manner's third answer (PR 11, decision P4-16); the official page draws it as its own tick box",
)];

/// The agent definition's groups that no official name group gives:
/// `(form, definition group id, reason)`. Their members are the
/// definition's, less any box an official group already claims.
pub(super) const HOST_ONLY: &[(&str, &str, &str)] = &[
    (
        "1601EQ",
        "frm1601EQ:if",
        "the official page draws refund, TCC and carry-over as three tick boxes of their own names and its script keeps them to one over-remittance answer",
    ),
    (
        "1700",
        "frm1700:rdoPg1I12ForeignTaxCredits",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I15CivilStatus",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I16SpouseHasIncome",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I20SpouseTaxpayerType",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I24SpouseForeignTaxCredits",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I2Amended",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg1I6TaxpayerType",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "frm1700:rdoPg2I1PartVIEmployee",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names",
    ),
    (
        "1700",
        "path:/filing_status",
        "the 1700 page is the 2018 screen (app/forms/screen-1700-v2018.json); the rules inventory is 1700-v2013's, whose controls have other names; item 17's Joint and Separate answer one field (the Schedule 1 row 2 Spouse box, host-written, is not in it)",
    ),
    (
        "1701",
        "frm1701:rdoPg1I6TaxpayerType",
        "the official page draws these as tick boxes of one name (control_kind checkbox) and its script keeps them to one answer",
    ),
    (
        "1701",
        "frm1701:rdoPg2I3SpouseType",
        "the official page draws these as tick boxes of one name (control_kind checkbox) and its script keeps them to one answer",
    ),
    (
        "1702Q",
        "frm1702q:rbATC",
        "the app's 1702Q keys are not in the 1702q-v2018c inventory, which names this revision's controls otherwise",
    ),
    (
        "1702Q",
        "frm1702q:rbAmendedRtn",
        "the app's 1702Q keys are not in the 1702q-v2018c inventory, which names this revision's controls otherwise",
    ),
    (
        "1702Q",
        "frm1702q:rbForClndrFscl",
        "the app's 1702Q keys are not in the 1702q-v2018c inventory, which names this revision's controls otherwise",
    ),
    (
        "1702Q",
        "frm1702q:rbMthdOfDdctns",
        "the app's 1702Q keys are not in the 1702q-v2018c inventory, which names this revision's controls otherwise",
    ),
    (
        "1702Q",
        "frm1702q:rbTxRlf",
        "the app's 1702Q keys are not in the 1702q-v2018c inventory, which names this revision's controls otherwise",
    ),
    (
        "1702RT",
        "path:/deduction_method",
        "the official page names item 13's two radios apart, but they answer one field, the method of deduction",
    ),
];

/// The groups the host keeps that no official inventory names, pinned
/// member for member against the printed sheet (review F4: the oracle is
/// this literal table, not a re-derivation): `(form, group id, members)`.
/// The [`NO_INVENTORY`] forms' whole set, and each [`HOST_ONLY`] id's
/// members — which no official name group ever claims.
pub(super) const PINNED_GROUPS: &[(&str, &str, &[&str])] = &[
    // 1601EQ item 32: refund / TCC / carried over.
    ("1601EQ", "frm1601EQ:if", &[
        "frm1601EQ:ifRefund",
        "frm1601EQ:ifIssueCert",
        "frm1601EQ:ifCarriedOver",
    ]),
    // 1700 (the page is the 2018 screen; the rules inventory is 1700-v2013's).
    ("1700", "frm1700:rdoPg1I12ForeignTaxCredits", &[
        "frm1700:rdoPg1I12ForeignTaxCreditsNo",
        "frm1700:rdoPg1I12ForeignTaxCreditsYes",
    ]),
    ("1700", "frm1700:rdoPg1I15CivilStatus", &[
        "frm1700:rdoPg1I15CivilStatusLS",
        "frm1700:rdoPg1I15CivilStatusM",
        "frm1700:rdoPg1I15CivilStatusS",
        "frm1700:rdoPg1I15CivilStatusW",
    ]),
    ("1700", "frm1700:rdoPg1I16SpouseHasIncome", &[
        "frm1700:rdoPg1I16SpouseHasIncomeN",
        "frm1700:rdoPg1I16SpouseHasIncomeY",
    ]),
    ("1700", "frm1700:rdoPg1I20SpouseTaxpayerType", &[
        "frm1700:rdoPg1I20SpouseTaxpayerTypeE",
        "frm1700:rdoPg1I20SpouseTaxpayerTypeN",
    ]),
    ("1700", "frm1700:rdoPg1I24SpouseForeignTaxCredits", &[
        "frm1700:rdoPg1I24SpouseForeignTaxCreditsNo",
        "frm1700:rdoPg1I24SpouseForeignTaxCreditsYes",
    ]),
    ("1700", "frm1700:rdoPg1I2Amended", &[
        "frm1700:rdoPg1I2AmendedNo",
        "frm1700:rdoPg1I2AmendedYes",
    ]),
    ("1700", "frm1700:rdoPg1I6TaxpayerType", &[
        "frm1700:rdoPg1I6TaxpayerTypeE",
        "frm1700:rdoPg1I6TaxpayerTypeN",
    ]),
    ("1700", "frm1700:rdoPg2I1PartVIEmployee", &[
        "frm1700:rdoPg2I1PartVIEmployeeS",
        "frm1700:rdoPg2I1PartVIEmployeeT",
    ]),
    // Item 17's Joint/Separate answer one field; the Schedule 1 row 2
    // Spouse box is the host's own and not in it.
    ("1700", "path:/filing_status", &[
        "frm1700:rdoPg1I17FilingStatusJ",
        "frm1700:rdoPg1I17FilingStatusS",
    ]),
    // 1701 items 6 and Part IV item 3, drawn as tick boxes of one name.
    ("1701", "frm1701:rdoPg1I6TaxpayerType", &[
        "frm1701:rdoPg1I6TaxpayerTypeS",
        "frm1701:rdoPg1I6TaxpayerTypeP",
        "frm1701:rdoPg1I6TaxpayerTypeE",
        "frm1701:rdoPg1I6TaxpayerTypeT",
    ]),
    ("1701", "frm1701:rdoPg2I3SpouseType", &[
        "frm1701:rdoPg2I3SpouseTypeS",
        "frm1701:rdoPg2I3SpouseTypeP",
    ]),
    // 1701MS (no inventory): the definition's radio groups as the sheet
    // shows them — item 5, the taxpayer's and spouse's 11 and 12, the two
    // item 17 pairs.
    ("1701MS", "frm1701MS:txtAmended", &[
        "frm1701MS:txtAmendedYesNo2",
        "frm1701MS:txtAmendedNoNo2",
    ]),
    ("1701MS", "frm1701MS:txtReturnPeriod", &[
        "frm1701MS:txtReturnPeriodYesNo3",
        "frm1701MS:txtReturnPeriodNoNo3",
    ]),
    ("1701MS", "frm1701MS:txtIfMarried", &[
        "frm1701MS:txtIfMarriedJointlyNo5",
        "frm1701MS:txtIfMarriedSeparatelyNo5",
        "frm1701MS:txtIfMarriedNotApplicableNo5",
    ]),
    ("1701MS", "frm1701MS:txtTaxpayerNo11group", &[
        "frm1701MS:txtTaxpayerNo11a",
        "frm1701MS:txtTaxpayerNo11b",
    ]),
    ("1701MS", "frm1701MS:txtTaxpayerNo11", &[
        "frm1701MS:txtTaxpayerNo11a1",
        "frm1701MS:txtTaxpayerNo11b1",
        "frm1701MS:txtTaxpayerNo11c1",
        "frm1701MS:txtTaxpayerNo11d1",
    ]),
    ("1701MS", "frm1701MS:txtTaxpayerNo12", &[
        "frm1701MS:txtTaxpayerNo12b1",
        "frm1701MS:txtTaxpayerNo12c1",
        "frm1701MS:txtTaxpayerNo12d1",
        "frm1701MS:txtTaxpayerNo12e1",
        "frm1701MS:txtTaxpayerNo12c",
        "frm1701MS:txtTaxpayerNo12d",
        "frm1701MS:txtTaxpayerNo12e",
    ]),
    ("1701MS", "frm1701MS:txtTaxpayerNo17", &[
        "frm1701MS:txtTaxpayerNo17a",
        "frm1701MS:txtTaxpayerNo17b",
    ]),
    ("1701MS", "frm1701MS:txtSpouseNo17", &[
        "frm1701MS:txtSpouseNo17a",
        "frm1701MS:txtSpouseNo17b",
    ]),
    // 1701Q (no inventory): the definition's radio groups as the sheet
    // shows them.
    ("1701Q", "frm1701q:AmendedRtn", &[
        "frm1701q:AmendedRtn_1",
        "frm1701q:AmendedRtn_2",
    ]),
    ("1701Q", "frm1701q:optType", &[
        "frm1701q:optType_1",
        "frm1701q:optType_2",
        "frm1701q:optType_3",
        "frm1701q:optType_4",
    ]),
    ("1701Q", "frm1701q:optATC", &[
        "frm1701q:optATC_1",
        "frm1701q:optATC_2",
        "frm1701q:optATC_3",
        "frm1701q:optATC_4",
        "frm1701q:optATC_5",
        "frm1701q:optATC_6",
    ]),
    ("1701Q", "frm1701q:optForeignTaxCredits", &[
        "frm1701q:optForeignTaxCredits_1",
        "frm1701q:optForeignTaxCredits_2",
    ]),
    ("1701Q", "frm1701q:optTaxRate", &[
        "frm1701q:optTaxRate_1",
        "frm1701q:optTaxRate_2",
    ]),
    ("1701Q", "frm1701q:optMethodOfDeduction", &[
        "frm1701q:optMethodOfDeduction:_1",
        "frm1701q:optMethodOfDeduction:_2",
    ]),
    ("1701Q", "frm1701q:optSpouseType", &[
        "frm1701q:optSpouseType_1",
        "frm1701q:optSpouseType_2",
        "frm1701q:optSpouseType_3",
    ]),
    ("1701Q", "frm1701q:optSpouseATC", &[
        "frm1701q:optSpouseATC_1",
        "frm1701q:optSpouseATC_2",
        "frm1701q:optSpouseATC_3",
        "frm1701q:optSpouseATC_4",
        "frm1701q:optSpouseATC_5",
        "frm1701q:optSpouseATC_6",
        "frm1701q:optSpouseATC_7",
    ]),
    ("1701Q", "frm1701q:optSpouseForeignTaxCred", &[
        "frm1701q:optSpouseForeignTaxCred_1",
        "frm1701q:optSpouseForeignTaxCred_2",
    ]),
    ("1701Q", "frm1701q:optSpouseTaxRate", &[
        "frm1701q:optSpouseTaxRate_1",
        "frm1701q:optSpouseTaxRate_2",
    ]),
    ("1701Q", "frm1701q:optSpouseMethod", &[
        "frm1701q:optSpouseMethod:_1",
        "frm1701q:optSpouseMethod:_2",
    ]),
    // 1702Q (the app's keys are not the inventory revision's).
    ("1702Q", "frm1702q:rbATC", &["frm1702q:rbATC_2", "frm1702q:rbATC_1"]),
    ("1702Q", "frm1702q:rbAmendedRtn", &[
        "frm1702q:rbAmendedRtn_1",
        "frm1702q:rbAmendedRtn_2",
    ]),
    ("1702Q", "frm1702q:rbForClndrFscl", &[
        "frm1702q:rbForClndrFscl_1",
        "frm1702q:rbForClndrFscl_2",
    ]),
    ("1702Q", "frm1702q:rbMthdOfDdctns", &[
        "frm1702q:rbMthdOfDdctns_1",
        "frm1702q:rbMthdOfDdctns_2",
    ]),
    ("1702Q", "frm1702q:rbTxRlf", &[
        "frm1702q:rbTxRlf_1",
        "frm1702q:rbTxRlf_2",
    ]),
    // 1702RT item 13's two radios answer one field.
    ("1702RT", "path:/deduction_method", &[
        "frm1702RT:rdoPg1Pt1I13ItemizedDeduction",
        "frm1702RT:rdoPg1Pt1I13OptionalStandard",
    ]),
];

/// Whether this app can fill `key`: a user box with a binding, or one the
/// host writes itself.
pub(super) fn fillable(kind: AgentFormKind, key: &str) -> bool {
    let def = kind.definition();
    def.box_def(key).is_some_and(|entry| entry.is_user_box())
        && (def.bindings.contains_key(key) || catalogs::host_writes(kind, key))
}

/// The agent definition's own groups: a radio group box's members, and
/// radio boxes without a group box, not written by the host itself, that
/// write one draft field (`path:<pointer>`), two or more each.
pub(super) fn definition_groups(kind: AgentFormKind) -> BTreeMap<String, Vec<String>> {
    let def = kind.definition();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &def.boxes {
        if let Some(group) = entry
            .group
            .as_deref()
            .and_then(|group| def.box_def(group))
            .filter(|group| group.radio_group)
        {
            groups
                .entry(group.key.clone())
                .or_default()
                .push(entry.key.clone());
            continue;
        }
        if entry.group.is_some()
            || !entry.kind.contains("radio")
            || catalogs::host_writes(kind, &entry.key)
        {
            continue;
        }
        let paths = true_option_paths(def, &entry.key);
        if paths.len() == 1 {
            groups
                .entry(format!("path:{}", paths[0]))
                .or_default()
                .push(entry.key.clone());
        }
    }
    groups.retain(|_, members| members.len() >= 2);
    groups
}

/// The [`PINNED_GROUPS`] rows of one form, group -> members.
fn pinned_groups(code: &str) -> BTreeMap<String, Vec<String>> {
    PINNED_GROUPS
        .iter()
        .filter(|(form, ..)| *form == code)
        .map(|(_, id, members)| {
            (
                id.to_string(),
                members.iter().map(|m| m.to_string()).collect(),
            )
        })
        .collect()
}

/// The one-answer groups this PR expects of a form: its official name
/// groups over the boxes this app can fill, with [`JOINED`], then the
/// pinned host-kept rest — [`PINNED_GROUPS`], written out against the
/// printed sheet (review F4) rather than re-derived: the whole set for a
/// form in [`NO_INVENTORY`], each [`HOST_ONLY`] id's members otherwise.
pub(super) fn expected_groups(kind: AgentFormKind) -> BTreeMap<String, Vec<String>> {
    let code = kind.form_code();
    let Some(official) = official_name_groups(kind) else {
        return pinned_groups(code);
    };
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, ids) in official {
        let mut members: Vec<String> = ids.into_iter().filter(|key| fillable(kind, key)).collect();
        members.extend(
            JOINED
                .iter()
                .filter(|(form, key, group, _)| {
                    *form == code && *group == name && fillable(kind, key)
                })
                .map(|(_, key, ..)| key.to_string()),
        );
        if members.len() >= 2 {
            out.insert(name, members);
        }
    }
    let claimed: BTreeSet<String> = out.values().flatten().cloned().collect();
    for (group, members) in pinned_groups(code) {
        if !HOST_ONLY
            .iter()
            .any(|(form, id, _)| *form == code && *id == group)
        {
            continue;
        }
        let rest: Vec<String> = members
            .into_iter()
            .filter(|key| !claimed.contains(key))
            .collect();
        if rest.len() >= 2 {
            out.insert(group, rest);
        }
    }
    out
}

/// `(group id, members)` of every expected group of three or more.
pub(super) fn groups_of_three_or_more(kind: AgentFormKind) -> Vec<(String, Vec<String>)> {
    expected_groups(kind)
        .into_iter()
        .filter(|(_, members)| members.len() >= 3)
        .collect()
}

/// What a member needs answered before its tick holds: the period to open
/// the return for (`None`: the form's first that opens) and the edits to
/// fill first, one fill each, in order. Each is the form's own condition
/// (the `off` reason the host gives, or what its recompute clears without):
/// an over-remittance or overpayment for a disposition, a married (and
/// joint) return for a spouse's answer, the filer type an ATC is allowed
/// for, item 15D Yes for 1801's frequency, Prepayment before 2200S's other
/// schemes, and the quarter the return is for on 2550Q.
pub(super) fn seed(code: &str, member: &str) -> (Option<i64>, Vec<Value>) {
    let tail = member.rsplit(':').next().unwrap_or(member);
    let joint_1701 = |spouse_type: Option<&str>| {
        let mut edits = vec![json!({
            "frm1701:rdoPg1I16CivilStatusM": "true",
            "frm1701:rdoPg1I17SpouseIncomeYes": "true",
            "frm1701:rdoPg1I18FilingStatusJ": "true",
        })];
        if let Some(spouse_type) = spouse_type {
            edits.push(json!({ spouse_type: "true" }));
        }
        edits
    };
    let joint_1701a = |spouse_type: &str| {
        vec![
            json!({
                "frm1701A:optCivilStatus_2": "true",
                "frm1701A:optSpouseIncome_1": "true",
                "frm1701A:optFilingStatus_1": "true",
            }),
            json!({ spouse_type: "true" }),
        ]
    };
    let joint_1701ms = || {
        vec![json!({
            "frm1701MS:civilStatus": "Married",
            "frm1701MS:txtIfMarriedJointlyNo5": "true",
        })]
    };
    let edits = match (code, tail) {
        ("1601EQ", "ifRefund" | "ifIssueCert" | "ifCarriedOver") => {
            vec![json!({"frm1601EQ:txtTax20": "1000"})]
        }
        ("1602Q", "OverRemittance1" | "OverRemittance2" | "OverRemittance3") => {
            vec![json!({"frm1602Q:txt18": "1000"})]
        }
        (
            "1701",
            "rdoPg1OverpaymentRefund" | "rdoPg1OverpaymentTCC" | "rdoPg1OverpaymentCarryOver",
        ) => vec![json!({"frm1701:txtPg4IPart7_1A": "5000"})],
        ("1701", "rdoPg2I4ATC_II012" | "rdoPg2I4ATC_II015" | "rdoPg2I12TaxRateG") => {
            joint_1701(Some("frm1701:rdoPg2I3SpouseTypeS"))
        }
        ("1701", "rdoPg2I4ATC_II014" | "rdoPg2I4ATC_II017" | "rdoPg2I12TaxRateP") => {
            joint_1701(Some("frm1701:rdoPg2I3SpouseTypeP"))
        }
        ("1701", tail) if tail.starts_with("rdoPg2I4ATC_") => joint_1701(None),
        ("1701A", "optATC_1" | "optATC_3" | "optTaxRate_1" | "optTaxRate_2") => {
            vec![json!({"frm1701A:optTaxType_1": "true"})]
        }
        ("1701A", "optATC_2" | "optATC_4") => vec![json!({"frm1701A:optTaxType_2": "true"})],
        ("1701A", "optSpouseATC_1" | "optSpouseATC_3") => {
            joint_1701a("frm1701A:optSpouseTaxType_1")
        }
        (
            "1701A",
            "optSpouseATC_2" | "optSpouseATC_4" | "optSpouseTaxRate_1" | "optSpouseTaxRate_2",
        ) => joint_1701a("frm1701A:optSpouseTaxType_2"),
        ("1701A", "optRefund_1" | "optRefund_2" | "optRefund_3") => {
            vec![json!({"frm1701A:txt57A": "5000"})]
        }
        ("1701MS", tail) if tail.starts_with("txtIfMarried") => {
            vec![json!({"frm1701MS:civilStatus": "Married"})]
        }
        ("1701MS", tail)
            if tail.starts_with("txtTaxpayerNo11")
                || (tail.starts_with("txtTaxpayerNo12") && tail.ends_with('1')) =>
        {
            joint_1701ms()
        }
        ("1701Q", tail) if tail.starts_with("optSpouse") => {
            vec![json!({"page:1701Q:civilStatus": "Married", "frm1701q:optType_1": "true"})]
        }
        (
            "1702EX",
            "rdoPg1OverpaymentRefund" | "rdoPg1OverpaymentTCC" | "rdoPg1OverpaymentCarryOver",
        ) => vec![json!({"frm1702EX:txtPg2Pt4I42PriorYearExcessCr": "5000"})],
        ("1702MX", "rdoPg1Pt2I21Refund" | "rdoPg1Pt2I21IssueTCC" | "rdoPg1Pt2I21CarriedOver") => {
            vec![json!({
                "frm1702MX:txtPg2Sc2Itm1C": "1000000",
                "frm1702MX:txtPg2Sc3It20C": "5000000",
            })]
        }
        (
            "1702RT",
            "rdoPg1Pt2I21OverpaymentRefunded"
            | "rdoPg1Pt2I21OverpaymentIssued"
            | "rdoPg1Pt2I21OverpaymentCarried",
        ) => vec![json!({
            "frm1702RT:txtPg2Pt4I27Sales": "1000000",
            "frm1702RT:txtPg2Pt4I48CreditableTaxWithheldFromPrevious": "5000000",
        })],
        ("1801", tail) if tail.starts_with("freq_") => {
            vec![json!({"frm1801v2018:instGranted_1": "true"})]
        }
        ("2200S", "chkPymntManner_3") => vec![json!({"frm2200S:chkPymntManner_2": "true"})],
        ("2550Q", tail) if tail.starts_with("OptQuarter") => {
            let quarter: i64 = tail["OptQuarter".len()..].parse().unwrap();
            return (Some(quarter), Vec::new());
        }
        _ => Vec::new(),
    };
    (None, edits)
}

/// Members that still do not read back ticked once [`seed`] has answered
/// what they need, each with its reason. The rule's tests fail on any
/// other member that does not tick, and on a listed one that does.
pub(super) const NEVER_TICKED: &[(&str, &str, &str)] = &[];

fn open_for(code: &str, period: Option<i64>) -> Value {
    period
        .into_iter()
        .chain([1i64, 3, 0, 6, 7])
        .find_map(|period| {
            call_async(
                "screen_form_open",
                vec![TIN.into(), code.into(), YEAR.into(), period.into()],
            )
            .ok()
        })
        .unwrap_or_else(|| panic!("{code} opens"))
}

pub(super) fn try_fill(
    code: &str,
    draft: &Value,
    edits: Value,
    unlocked: &[String],
) -> Result<Value, String> {
    call(
        "screen_form_fill",
        vec![
            code.into(),
            json(draft.clone()),
            json(edits),
            json(json!(unlocked)),
        ],
    )
    .map_err(|error| error.to_string())
}

/// The draft's values at `paths`, by path.
fn at_paths(draft: &Value, paths: &[String]) -> Value {
    Value::Object(
        paths
            .iter()
            .map(|path| {
                (
                    path.clone(),
                    draft.pointer(path).cloned().unwrap_or(Value::Null),
                )
            })
            .collect(),
    )
}

/// One fill's outcome for the golden: the error, or the group's fields,
/// the box as shown and every refusal with its message.
fn outcome(result: &Result<Value, String>, key: &str, paths: &[String]) -> Value {
    match result {
        Err(error) => json!({ "error": error }),
        Ok(filled) => json!({
            "fields": at_paths(&filled["draft"], paths),
            "shown": filled["values"][key],
            "rejected": filled["rejected"],
        }),
    }
}

/// One case of the sweep, keyed `<code> <member> <edit>` (`tick`,
/// `false`, or `blank (group)`): the draft the edit started from, the paths
/// the member's tick writes, and the fill.
pub(super) struct Case {
    pub(super) ticked: Value,
    pub(super) member_paths: Vec<String>,
    pub(super) result: Result<Value, String>,
}

pub(super) fn sweep() -> BTreeMap<String, (Value, Case)> {
    let mut cases = BTreeMap::new();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let def = kind.definition();
        for (group, members) in groups_of_three_or_more(kind) {
            let paths: Vec<String> = members
                .iter()
                .flat_map(|member| true_option_paths(def, member))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            for member in &members {
                let member_paths = true_option_paths(def, member);
                let (period, prerequisites) = seed(code, member);
                let mut unlocked: Vec<String> = members.clone();
                if def.box_def(&group).is_some() {
                    unlocked.push(group.clone());
                }
                for edits in &prerequisites {
                    unlocked.extend(edits.as_object().unwrap().keys().cloned());
                }
                let mut draft = open_for(code, period)["draft"].clone();
                for edits in &prerequisites {
                    let filled = try_fill(code, &draft, edits.clone(), &unlocked)
                        .unwrap_or_else(|error| panic!("{code} {member}: seed {edits}: {error}"));
                    draft = filled["draft"].clone();
                }
                let tick = try_fill(code, &draft, json!({ member: "true" }), &unlocked);
                cases.insert(
                    format!("{code} {member} tick"),
                    (
                        outcome(&tick, member, &paths),
                        Case {
                            ticked: draft.clone(),
                            member_paths: member_paths.clone(),
                            result: tick.clone(),
                        },
                    ),
                );
                let Ok(ticked) = tick else {
                    continue;
                };
                let mut edits = vec![(member.clone(), "false", "false")];
                if def.box_def(&group).is_some() {
                    edits.push((group.clone(), "", "blank (group)"));
                }
                for (key, sent, label) in edits {
                    let result = try_fill(code, &ticked["draft"], json!({ &key: sent }), &unlocked);
                    cases.insert(
                        format!("{code} {member} {label}"),
                        (
                            outcome(&result, &key, &paths),
                            Case {
                                ticked: ticked["draft"].clone(),
                                member_paths: member_paths.clone(),
                                result,
                            },
                        ),
                    );
                }
            }
        }
    }
    cases
}

/// Writes the sweep's outcomes to `RADIO_UNTICK_DUMP`: how the golden's
/// `before` (on the base of PR 14) and `after` columns were recorded.
#[test]
#[ignore = "writes a file: run only as `cargo test --manifest-path crates/bir-shell/Cargo.toml -- --ignored --exact screen_form_module::tests::radio_untick::pr14_record_radio_untick_outcomes` with RADIO_UNTICK_DUMP=<file>"]
fn pr14_record_radio_untick_outcomes() {
    // Without a file to write, return before the sweep runs (a bare
    // `-- --ignored` must not spend it).
    let Ok(path) = std::env::var("RADIO_UNTICK_DUMP") else {
        return;
    };
    setup();
    let out: Map<String, Value> = sweep()
        .into_iter()
        .map(|(key, (record, _))| (key, record))
        .collect();
    std::fs::write(path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
}
