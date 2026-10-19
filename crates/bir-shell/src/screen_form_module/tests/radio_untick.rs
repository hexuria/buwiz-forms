//! PR 14 (decision P4-18): unticking the ticked member of a radio group of
//! three or more never fails an edit. This file is the sweep and its
//! recorder; it uses only what the base of PR 14 already had, so the same
//! sweep recorded the golden's `before` column on that base
//! (`fixtures/radio-untick-golden.json`). The assertions on the rule are in
//! `radio_untick_rule.rs`.
//!
//! Every form's every one-answer group of three or more: each member is
//! ticked on a draft whose prerequisites are answered first ([`seed`]),
//! then unticked with "false", and its group box is sent "". A member that
//! still does not read back ticked is listed in [`NEVER_TICKED`] with its
//! reason; the rule's tests fail on any other.

use super::*;

/// `(group id, members)` of every one-answer group of three or more, as
/// `plan_choice_edits` groups them: a radio group box's members, or radio
/// boxes without a group box, and not written by the host itself, that
/// write one draft field (id `path:<pointer>`).
pub(super) fn groups_of_three_or_more(kind: AgentFormKind) -> Vec<(String, Vec<String>)> {
    let def = kind.definition();
    let mut groups: Vec<(String, Vec<String>)> = def
        .boxes
        .iter()
        .filter(|entry| entry.radio_group && entry.members.len() >= 3)
        .map(|entry| (entry.key.clone(), entry.members.clone()))
        .collect();
    let mut implicit: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in &def.boxes {
        if entry.group.is_some()
            || !entry.kind.contains("radio")
            || catalogs::host_writes(kind, &entry.key)
        {
            continue;
        }
        let paths = true_option_paths(def, &entry.key);
        if paths.len() == 1 {
            implicit
                .entry(paths[0].clone())
                .or_default()
                .push(entry.key.clone());
        }
    }
    groups.extend(
        implicit
            .into_iter()
            .filter(|(_, members)| members.len() >= 3)
            .map(|(path, members)| (format!("path:{path}"), members)),
    );
    groups
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
                if !group.starts_with("path:") {
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
                if !group.starts_with("path:") {
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
    setup();
    let out: Map<String, Value> = sweep()
        .into_iter()
        .map(|(key, (record, _))| (key, record))
        .collect();
    let path = std::env::var("RADIO_UNTICK_DUMP").expect("RADIO_UNTICK_DUMP");
    std::fs::write(path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
}
