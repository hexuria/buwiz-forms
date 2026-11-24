//! Screen-path audit (withholding forms): rebuild each official sample by
//! entering only what a user can type on the form page — every scalar input
//! through `agent::fill_as_user`, then `agent::compute` — and compare the
//! upload key by key with the official eBIRForms output. Rows are built
//! through the draft's row operations (the page's popup), but every value
//! typed into them goes through the screen path too.
//!
//! Output: one `DIFF` line per mismatched key, one `REFUSED` line per input
//! the screen path rejects, one `PAYLOAD` line per sample whose upload does
//! not validate. Run with `--nocapture`.

use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::official_xml::{self, OfficialLayout};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/sample-payloads")
}

/// `dummy_profile` is a copy of the sample_payloads test's: the same fake
/// taxpayer every sample was entered with.
fn dummy_profile(
    default_form_type: &str,
    taxpayer_type: &str,
) -> bir_core::profile::TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "full_name": "Sample Dummy Taxpayer",
        "tin": {"segment1": "123", "segment2": "456", "segment3": "788", "branch": "00000"},
        "rdo_code": "039",
        "line_of_business": "Sample Consulting Services",
        "registered_address": "123 Sample Street, Barangay Example, Quezon City",
        "zip_code": "1100",
        "phone": "09170000000",
        "email": "sample.taxpayer@example.com",
        "default_form_type": default_form_type,
        "taxpayer_type": taxpayer_type,
        "business_start_date": "2020-01-15",
        "tax_elections": [{
            "taxable_year": 2025,
            "election": "GraduatedUnspecified",
            "elected_at": "2025-01-15T00:00:00",
            "source_form": "sample"
        }]
    }))
    .expect("dummy profile must deserialize")
}

enum Op {
    Set(String, String),
    Click(String),
}

/// The inputs of `<stem>.steps.json`, reduced to the key/value pairs a user
/// types or clicks. Skipped: popup and validation plumbing (`btnAddATC`,
/// `btnOkPop`, `cmdValidate`, timers, `wait`s), the profile-background
/// script and every key that names identity or period, which the draft
/// constructor fills from the profile and the filing period — a user never
/// types them into a return.
fn steps(stem: &str) -> Vec<Op> {
    let path = samples_dir().join(format!("{stem}.steps.json"));
    let raw: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}")),
    )
    .expect("steps.json parses");
    let mut ops = Vec::new();
    let noise = [
        "btn",
        "cmd",
        "AtcCode",
        "txtRate",
        "Sheets",
        "NoSheets",
        "txtSheets",
    ];
    let identity = [
        "TIN1",
        "TIN2",
        "TIN3",
        "TIN4",
        "BranchCode",
        "RDO",
        "TaxpayerName",
        "WithholdingAgentsName",
        "txtAddress",
        "ZipCode",
        "TelNum",
        "Email",
        "LineBus",
        "txtLOB",
        "RegName",
        "ContactNumber",
    ];
    let mut push = |key: &str, op: Op| {
        if noise.iter().any(|n| key.starts_with(n) || key.contains(n))
            || identity.iter().any(|n| key.contains(n))
            || !key.contains("frm")
        {
            return;
        }
        ops.push(match op {
            Op::Click(_) => Op::Click(key.to_string()),
            Op::Set(_, v) => Op::Set(key.to_string(), v),
        });
    };
    for step in raw.as_array().expect("steps is a list") {
        if let Some(key) = step.get("set").and_then(Value::as_str) {
            let value = step.get("value").and_then(Value::as_str).unwrap_or("");
            let key = key.to_string();
            let value = value.to_string();
            push(&key, Op::Set(String::new(), value));
        } else if let Some(key) = step.get("click").and_then(Value::as_str) {
            push(key, Op::Click(String::new()));
        } else if let Some(js) = step.get("call").and_then(Value::as_str) {
            // `getElementById('X').value = 'Y'` pairs out of the replay JS.
            let mut rest = js;
            while let Some(at) = rest.find("getElementById(") {
                rest = &rest[at + "getElementById(".len()..];
                let quote = rest.chars().next().unwrap_or(' ');
                if quote != '\'' && quote != '"' {
                    continue;
                }
                let Some(end) = rest[1..].find(quote) else {
                    break;
                };
                let key = rest[1..1 + end].to_string();
                rest = &rest[1 + end..];
                if !rest.starts_with(").value") {
                    continue;
                }
                let Some(eq) = rest.find('=') else {
                    continue;
                };
                let after = &rest[eq + 1..];
                let after = after.trim_start();
                let q = after.chars().next().unwrap_or(' ');
                if q != '\'' && q != '"' {
                    continue;
                }
                let Some(vend) = after[1..].find(q) else {
                    continue;
                };
                let value = after[1..1 + vend].to_string();
                let key2 = key.clone();
                push(&key, Op::Set(String::new(), value));
                let _ = key2;
            }
        }
    }
    ops
}

struct Audit {
    diffs: Vec<String>,
}

impl Audit {
    fn note(&mut self, line: String) {
        println!("{line}");
        self.diffs.push(line);
    }
}

/// Replay `ops` into the draft JSON through the screen path, then compute.
/// Returns the refused-input notes; the draft is left computed in place.
fn replay(audit: &mut Audit, stem: &str, kind: AgentFormKind, draft: &mut Value, ops: &[Op]) {
    for op in ops {
        let (key, value) = match op {
            Op::Set(key, value) => (key.clone(), json!(value)),
            Op::Click(key) => (key.clone(), json!("true")),
        };
        if let Err(error) = agent::fill_as_user(kind, draft, &key, &value) {
            audit.note(format!("REFUSED|{stem}|{key}|{error}"));
        }
    }
    if let Err(error) = agent::compute(kind, draft) {
        audit.note(format!("COMPUTE-FAILS|{stem}|{error}"));
    }
}

/// Key-by-key comparison of our plaintext with the official upload.
fn diff(audit: &mut Audit, stem: &str, layout: &OfficialLayout, plaintext: &str) {
    let official_path = samples_dir().join(format!("{stem}.official.xml"));
    let official_text = std::fs::read_to_string(&official_path)
        .unwrap_or_else(|e| panic!("missing {official_path:?}: {e}"));
    let official: BTreeMap<String, String> = official_xml::read(layout, &official_text)
        .unwrap_or_else(|e| panic!("{stem} official does not parse: {e}"));
    let ours: BTreeMap<String, String> = match official_xml::read(layout, plaintext) {
        Ok(map) => map,
        Err(error) => {
            audit.note(format!("UNREADABLE|{stem}|{error}"));
            return;
        }
    };
    let keys: std::collections::BTreeSet<&String> = official.keys().chain(ours.keys()).collect();
    for key in keys {
        let theirs = official.get(key).map(String::as_str).unwrap_or("<absent>");
        let mine = ours.get(key).map(String::as_str).unwrap_or("<absent>");
        if theirs != mine {
            audit.note(format!("DIFF|{stem}|{key}|official={theirs}|ours={mine}"));
        }
    }
}

// ---------------------------------------------------------------------------
// 1600PT
// ---------------------------------------------------------------------------

use bir_core::forms::form_1600pt::{
    Form1600PtAgentCategory, Form1600PtDraft, Form1600PtScheduleRow, form_1600pt_atc_index,
};
use bir_core::forms::queueable::QueueableForm as _;

fn audit_1600pt(audit: &mut Audit, stem: &str, month: u8, government: bool) {
    let kind = AgentFormKind::from_code("1600PT").unwrap();
    let mut draft =
        Form1600PtDraft::new_from_profile(&dummy_profile("1600PT", "Corporation"), 2025, month);
    // Long enough for the official 80-character split into two lines.
    draft.registered_address = "Unit 1201 Sample Tower, 123 Sample Street corner Example Avenue, Barangay Example, Quezon City".into();
    draft.set_agent_category(if government {
        Form1600PtAgentCategory::Government
    } else {
        Form1600PtAgentCategory::Private
    });
    draft.set_taxes_withheld(true);
    for (code, rate) in [("WB030", "3.0"), ("WB080", "1.0"), ("WB200", "0.6")] {
        draft
            .toggle_atc(form_1600pt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    if stem == "1600PT-062025" {
        draft.schedule = vec![
            Form1600PtScheduleRow {
                tin: "987654321000".into(),
                payee_name: "Sample Supplier Inc".into(),
                atc: "wb030".into(),
                income_payment: 100_000.0,
                tax_rate: 5.0,
                tax_withheld: 0.0,
            },
            Form1600PtScheduleRow {
                tin: "111222339000".into(),
                payee_name: "Dela Cruz, Sample Payee".into(),
                atc: "WB080".into(),
                income_payment: 20_100.1,
                tax_rate: 5.0,
                tax_withheld: 0.0,
            },
        ];
    }
    draft.keep_entered_penalties();
    let mut value = serde_json::to_value(draft).expect("serializes");
    let ops = steps(stem);
    replay(audit, stem, kind, &mut value, &ops);
    let draft: Form1600PtDraft = serde_json::from_value(value).expect("deserializes");
    match draft.to_bir_xml_payload() {
        Ok(payload) => diff(audit, stem, &draft.official_layout().unwrap(), &payload),
        Err(errors) => audit.note(format!("PAYLOAD|{stem}|{errors:?}")),
    }
}

#[test]
fn audit_1600pt_samples() {
    let mut audit = Audit { diffs: Vec::new() };
    audit_1600pt(&mut audit, "1600PT-062025", 6, true);
}

// ---------------------------------------------------------------------------
// Generic overlay driver: build the sample exactly as the proven builder
// does, then re-enter every user input through the screen path and compute
// again. A key the screen refuses surfaces as REFUSED; a value the screen
// path writes differently surfaces as a DIFF against the official upload.
// ---------------------------------------------------------------------------

use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

fn overlay<D, E>(
    audit: &mut Audit,
    stem: &str,
    code: &str,
    draft: D,
    layout: impl Fn(&D) -> OfficialLayout,
    payload: impl FnOnce(&D) -> Result<String, E>,
) where
    D: Serialize + DeserializeOwned,
    E: Debug,
{
    let kind = AgentFormKind::from_code(code).unwrap_or_else(|| panic!("{code} is an agent form"));
    let mut value = serde_json::to_value(draft).expect("serializes");
    let ops = steps(stem);
    replay(audit, stem, kind, &mut value, &ops);
    let draft: D = serde_json::from_value(value).expect("deserializes");
    let layout = layout(&draft);
    match payload(&draft) {
        Ok(text) => diff(audit, stem, &layout, &text),
        Err(errors) => audit.note(format!("PAYLOAD|{stem}|{errors:?}")),
    }
}

fn fixed(id: &'static str) -> OfficialLayout {
    official_xml::layout(id).expect("layout").clone()
}

// --- 0619E -----------------------------------------------------------------

use bir_core::forms::form_0619e::Form0619EDraft;

#[test]
fn audit_0619e() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form0619EDraft::new_from_profile(&dummy_profile("0619E", "Corporation"), 2025, 6);
    draft.registered_address =
        "Unit 1203 Sample Tower, 456 Example Avenue corner Placeholder Street, Barangay Sample, Quezon City"
            .into();
    draft.is_amended = true;
    draft.any_taxes_withheld = true;
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "0619E-062025",
        "0619E",
        draft,
        |_| fixed("0619e-v2018"),
        |d| d.to_official_xml_payload(),
    );
}

// --- 0619F -----------------------------------------------------------------

use bir_core::forms::form_0619f::{Form0619FDraft, Form0619FTaxType};

#[test]
fn audit_0619f() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form0619FDraft::new_from_profile(&dummy_profile("0619F", "Corporation"), 2025, 6);
    draft.is_amended = true;
    draft.any_taxes_withheld = true;
    draft.tax_type = Form0619FTaxType::WF;
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "0619F-062025WF",
        "0619F",
        draft,
        |_| fixed("0619f-v2018"),
        |d| d.to_official_xml_payload(),
    );
}

// --- 1600PT remaining samples ----------------------------------------------

fn draft_1600pt(month: u8, government: bool, atcs: &[(&str, &str)]) -> Form1600PtDraft {
    let mut draft =
        Form1600PtDraft::new_from_profile(&dummy_profile("1600PT", "Corporation"), 2025, month);
    draft.set_agent_category(if government {
        Form1600PtAgentCategory::Government
    } else {
        Form1600PtAgentCategory::Private
    });
    draft.set_taxes_withheld(true);
    for (code, rate) in atcs {
        draft
            .toggle_atc(form_1600pt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    draft.keep_entered_penalties();
    draft
}

fn run_1600pt(audit: &mut Audit, stem: &str, draft: Form1600PtDraft) {
    overlay(
        audit,
        stem,
        "1600PT",
        draft,
        |d: &Form1600PtDraft| d.official_layout().expect("layout"),
        |d| d.to_bir_xml_payload(),
    );
}

#[test]
fn audit_1600pt_private() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft = draft_1600pt(7, false, &[("WB082", "3.0"), ("WB084", "1.0")]);
    draft.atc_rows[0].tax_base = 41_666.665;
    draft.atc_rows[1].tax_base = 1_000_000.0;
    run_1600pt(&mut audit, "1600PT-072025", draft);
}

#[test]
fn audit_1600pt_other_atcs() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft = draft_1600pt(
        8,
        true,
        &[
            ("WB030", "3.0"),
            ("WB040", "2.0"),
            ("WB070", "2.0"),
            ("WB080", "1.0"),
            ("WB090", "10.0"),
            ("WB140", "18.0"),
            ("WB200", "0.6"),
        ],
    );
    let bases = [
        123_456.785,
        1_000.0,
        77_777.77,
        50_000.005,
        3_333.335,
        45_000.25,
        8_333.33,
        99.995,
    ];
    for (row, base) in draft.atc_rows.iter_mut().zip(bases) {
        row.tax_base = base;
    }
    run_1600pt(&mut audit, "1600PT-082025", draft);
}

// --- 1600VT ----------------------------------------------------------------

use bir_core::forms::form_1600vt::{
    Form1600VtAgentCategory, Form1600VtDraft, Form1600VtScheduleRow, form_1600vt_atc_index,
};

fn draft_1600vt(month: u8, government: bool, atcs: &[(&str, &str)]) -> Form1600VtDraft {
    let mut draft =
        Form1600VtDraft::new_from_profile(&dummy_profile("1600VT", "Corporation"), 2025, month);
    draft.set_agent_category(if government {
        Form1600VtAgentCategory::Government
    } else {
        Form1600VtAgentCategory::Private
    });
    draft.set_taxes_withheld(true);
    for (code, rate) in atcs {
        draft
            .toggle_atc(form_1600vt_atc_index(code, rate).unwrap())
            .unwrap();
    }
    draft.keep_entered_penalties();
    draft
}

fn run_1600vt(audit: &mut Audit, stem: &str, draft: Form1600VtDraft) {
    overlay(
        audit,
        stem,
        "1600VT",
        draft,
        |d: &Form1600VtDraft| d.official_layout().expect("layout"),
        |d| d.to_bir_xml_payload(),
    );
}

#[test]
fn audit_1600vt_government() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft = draft_1600vt(
        6,
        true,
        &[("WV010", "5.0"), ("WV020", "5.0"), ("WV110", "12.0")],
    );
    draft.registered_address = "Unit 1201 Sample Tower, 123 Sample Street corner Example Avenue, Barangay Example, Quezon City".into();
    draft.schedule = vec![
        Form1600VtScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Sample Supplier Inc".into(),
            atc: "wv010".into(),
            income_payment: 100_000.0,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
        Form1600VtScheduleRow {
            tin: "111222339000".into(),
            payee_name: "Dela Cruz, Sample Payee".into(),
            atc: "WV020".into(),
            income_payment: 20_100.1,
            tax_rate: 5.0,
            tax_withheld: 0.0,
        },
    ];
    run_1600vt(&mut audit, "1600VT-062025", draft);
}

#[test]
fn audit_1600vt_private() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft = draft_1600vt(7, false, &[("WV050", "12.0"), ("WV080", "12.0")]);
    draft.atc_rows[0].tax_base = 41_666.665;
    draft.atc_rows[1].tax_base = 1_000_000.0;
    run_1600vt(&mut audit, "1600VT-072025", draft);
}

// --- 1600WP ----------------------------------------------------------------

use bir_core::forms::form_1600wp::{
    Form1600WpAgentCategory, Form1600WpDraft, Form1600WpScheduleRow,
};

fn run_1600wp(audit: &mut Audit, stem: &str, layout: &'static str, draft: Form1600WpDraft) {
    overlay(
        audit,
        stem,
        "1600WP",
        draft,
        |_| fixed(layout),
        |d| d.to_bir_xml_payload(),
    );
}

fn draft_1600wp(month: u8, day: u8) -> Form1600WpDraft {
    let mut draft =
        Form1600WpDraft::new_from_profile(&dummy_profile("1600WP", "Corporation"), 2025, month);
    draft.day = day;
    draft
}

#[test]
fn audit_1600wp_samples() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut two = draft_1600wp(6, 15);
    two.is_amended = true;
    two.set_taxes_withheld(true);
    two.set_agent_category(Form1600WpAgentCategory::Government);
    two.toggle_atc("WB191").unwrap();
    two.toggle_atc("WB192").unwrap();
    two.schedule = vec![
        Form1600WpScheduleRow {
            tin: "987654321000".into(),
            payee_name: "Dela Cruz, Juan Sample".into(),
            atc_code: "WB191".into(),
            amount: 100_000.0,
            tax_withheld: 0.0,
        },
        Form1600WpScheduleRow {
            tin: "111222339000".into(),
            payee_name: "Sample Stables Inc".into(),
            atc_code: "WB192".into(),
            amount: 20_100.105,
            tax_withheld: 0.0,
        },
    ];
    two.keep_entered_penalties();
    run_1600wp(&mut audit, "1600WP-06152025", "1600wp-v2010-atc2", two);

    let mut one = draft_1600wp(7, 1);
    one.set_taxes_withheld(true);
    one.set_agent_category(Form1600WpAgentCategory::Private);
    one.toggle_atc("WB194").unwrap();
    one.atc_rows[0].tax_base = 7_777.775;
    one.keep_entered_penalties();
    run_1600wp(&mut audit, "1600WP-07012025", "1600wp-v2010-atc1", one);

    let mut none = draft_1600wp(8, 31);
    none.set_taxes_withheld(false);
    none.set_agent_category(Form1600WpAgentCategory::Private);
    run_1600wp(&mut audit, "1600WP-08312025", "1600wp-v2010-atc0", none);
}

// --- 1601C -----------------------------------------------------------------

use bir_core::forms::form_1601c::{Form1601CDraft, Form1601CSchedule1Row};

#[test]
fn audit_1601c() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form1601CDraft::new_from_profile(&dummy_profile("1601Cv2018", "Corporation"), 2025, 6);
    draft.auto_compute_penalties = false;
    for component in bir_core::penalties::PenaltyComponent::ALL {
        draft.set_penalty_override(component, Some(0.0));
    }
    draft.schedule_1 = vec![Form1601CSchedule1Row {
        previous_month: "04/2025".to_string(),
        date_paid: "05/09/2025".to_string(),
        drawee_bank_code_or_agency: "SAMPLE BANK".to_string(),
        payment_number: "REF-0001".to_string(),
        tax_paid: 15_000.0,
        should_be_tax_due: 15_250.0,
        adjustment: 0.0,
    }];
    overlay(
        &mut audit,
        "1601C-062025",
        "1601C",
        draft,
        |_| fixed("1601c-v2018"),
        |d| d.clone().try_to_bir_xml_payload(),
    );
}

// --- 1601EQ ----------------------------------------------------------------

use bir_core::forms::form_1601eq::{Form1601EqCategory, Form1601EqDraft};

#[test]
fn audit_1601eq() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form1601EqDraft::new_from_profile(&dummy_profile("1601EQ", "Corporation"), 2025, 1);
    draft.set_any_tax_withheld(true);
    draft.set_category(Form1601EqCategory::Private);
    let bases = [
        ("WC160", 75_432.1),
        ("WI010", 123_456.789),
        ("WI100", 1_000.005),
        ("WI120", 999.995),
        ("WI157", 50_000.0),
        ("WI158", 250_000.0),
        ("WI160", 12_345.675),
        ("WC010", 88_888.88),
    ];
    for (code, _) in bases {
        draft.add_atc(code).unwrap();
    }
    for (code, base) in bases {
        let row = draft
            .schedule
            .iter_mut()
            .find(|row| row.atc_code == code)
            .unwrap();
        row.tax_base = base;
    }
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "1601EQ-2025Q1",
        "1601EQ",
        draft,
        |d: &Form1601EqDraft| d.official_layout().expect("layout"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1601FQ ----------------------------------------------------------------

use bir_core::forms::form_1601fq::{Form1601FqCategory, Form1601FqDraft, Form1601FqTaxRelief};

#[test]
fn audit_1601fq() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form1601FqDraft::new_from_profile(&dummy_profile("1601FQ", "Corporation"), 2025, 2);
    draft.set_any_tax_withheld(true);
    draft.set_category(Form1601FqCategory::Private);
    draft.set_tax_relief(Some(Form1601FqTaxRelief::InternationalTaxTreaty));
    for index in [29, 0, 15, 2, 28] {
        draft.add_atc(index).unwrap();
    }
    for (row, base) in
        draft
            .schedule
            .iter_mut()
            .zip([123_456.789, 1_000.005, 50_000.0, 77_777.775, 999.99])
    {
        row.tax_base = base;
    }
    draft.schedule1[0].treaty_code = "US".into();
    draft.set_schedule1_atc(0, "WI330");
    draft.schedule1[0].income_payment = 100_000.005;
    draft.schedule1[0].tax_rate = 15.0;
    draft.schedule1[1].treaty_code = "JP".into();
    draft.set_schedule1_atc(1, "WC180");
    draft.schedule1[1].income_payment = 50_000.0;
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "1601FQ-2025Q2",
        "1601FQ",
        draft,
        |d: &Form1601FqDraft| d.official_layout().expect("layout"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1602Q -----------------------------------------------------------------

use bir_core::forms::form_1602q::{
    Form1602QCategory, Form1602QDraft, Form1602QPreferentialRow, Form1602QTaxRelief,
    Form1602QTreatyRow,
};

#[test]
fn audit_1602q() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form1602QDraft::new_from_profile(&dummy_profile("1602Q", "Corporation"), 2025, 3);
    draft.is_amended = Some(false);
    draft.category = Some(Form1602QCategory::Private);
    draft.set_any_tax_withheld(true);
    draft.set_availing_tax_relief(true);
    draft.tax_relief = Form1602QTaxRelief::InternationalTaxTreaty;
    draft.schedule2[0] = Form1602QTreatyRow {
        treaty_code: "US".into(),
        atc_code: "WC161".into(),
        interest: 30_000.0,
        tax_rate: 10.0,
        tax_withheld: 0.0,
    };
    draft.schedule3[0] = Form1602QPreferentialRow {
        ipa_code: "PEZA".into(),
        interest: 5_000.0,
        tax_rate: 5.0,
        tax_withheld: 0.0,
    };
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "1602Q-2025Q3",
        "1602Q",
        draft,
        |_| fixed("1602q-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1603Q -----------------------------------------------------------------

use bir_core::forms::form_1603q::{Form1603QCategory, Form1603QDraft, Form1603QTaxRelief};

#[test]
fn audit_1603q() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft =
        Form1603QDraft::new_from_profile(&dummy_profile("1603Q", "Corporation"), 2025, 3);
    draft.category = Some(Form1603QCategory::Government);
    draft.tax_relief = Some(Form1603QTaxRelief::Both);
    draft.set_any_tax_withheld(true);
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "1603Q-2025Q3",
        "1603Q",
        draft,
        |_| fixed("1603q-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1604C -----------------------------------------------------------------

use bir_core::forms::form_1604c::{Form1604cAgentCategory, Form1604cDraft, Form1604cRemittance};

#[test]
fn audit_1604c() {
    let mut audit = Audit { diffs: Vec::new() };
    let month =
        |date: &str, bank: &str, reference: &str, tax: f64, adjustment: f64, penalty: f64| {
            Form1604cRemittance {
                date: date.to_string(),
                bank: bank.to_string(),
                reference: reference.to_string(),
                taxes_withheld: tax,
                adjustment,
                penalties: penalty,
                total_remitted: 0.0,
            }
        };
    let mut draft = Form1604cDraft::new_from_profile(&dummy_profile("1604C", "Corporation"), 2025);
    draft.agent_category = Form1604cAgentCategory::Private;
    draft.top_withholding_agent = true;
    draft.refunds_released = true;
    draft.refund_date = "03/15/2026".to_string();
    draft.overremittance = 4_321.555;
    draft.first_crediting_month = 2;
    draft.months[0] = month(
        "02/10/2025",
        "Sample bank-001",
        "tra1",
        10_000.005,
        0.0,
        25.5,
    );
    draft.months[1] = month("03/10/2025", "sample bank", "tra2", 8_000.5, -500.25, 0.0);
    draft.months[2] = month("04/10/2025", "Bank 3", "tra3", 100.0, -300.0, 0.0);
    draft.months[5] = month("07/10/2025", "Agency 6", "eROR-6", 999.995, 0.0, 0.005);
    draft.months[11] = month(
        "01/15/2026",
        "Bank Code 12",
        "TRA-12",
        12_345.675,
        100.0,
        0.0,
    );
    overlay(
        &mut audit,
        "1604C-2025",
        "1604C",
        draft,
        |_| fixed("1604c-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1604E -----------------------------------------------------------------

use bir_core::forms::form_1604e::{Form1604eAgentCategory, Form1604eDraft, Form1604eRemittance};

#[test]
fn audit_1604e() {
    let mut audit = Audit { diffs: Vec::new() };
    let row =
        |date: &str, bank: &str, reference: &str, tax: f64, penalty: f64| Form1604eRemittance {
            date: date.to_string(),
            bank: bank.to_string(),
            reference: reference.to_string(),
            taxes_withheld: tax,
            penalties: penalty,
            total_remitted: 0.0,
        };
    let mut draft = Form1604eDraft::new_from_profile(&dummy_profile("1604E", "Corporation"), 2025);
    draft.agent_category = Form1604eAgentCategory::Private;
    draft.top_withholding_agent = Some(false);
    draft.schedule1[0] = row("04/30/2025", "SampleBank01", "traQ1", 12_345.675, 100.005);
    draft.schedule1[1] = row("07/31/2025", "Agency2", "eROR2", 999.995, 0.0);
    draft.schedule1[3] = row("01/30/2026", "bank4", "Ear4", 250_000.0, 0.0);
    draft.schedule2[0] = row("02/10/2025", "BankJan", "TRA0101", 1_000.5, 0.5);
    draft.schedule2[5] = row("07/10/2025", "bankjun", "tra06", 2_500.125, 0.0);
    draft.schedule2[11] = row("01/15/2026", "BANKDEC", "TRA12", 333.333, 12.0);
    overlay(
        &mut audit,
        "1604E-2025",
        "1604E",
        draft,
        |_| fixed("1604e-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1604F -----------------------------------------------------------------

use bir_core::forms::form_1604f::{Form1604fAgentCategory, Form1604fDraft, Form1604fRemittance};

#[test]
fn audit_1604f() {
    let mut audit = Audit { diffs: Vec::new() };
    let remittance = |date: &str, reference: &str, tax: f64, penalty: f64| Form1604fRemittance {
        date: date.to_string(),
        reference: reference.to_string(),
        taxes_withheld: tax,
        penalties: penalty,
        total_remitted: 0.0,
    };
    let mut draft = Form1604fDraft::new_from_profile(&dummy_profile("1604F", "Corporation"), 2025);
    draft.agent_category = Form1604fAgentCategory::Private;
    draft.tax_relief = true;
    draft.tax_relief_details = "Rp-us treaty".to_string();
    draft.schedules[0][0] = remittance("04/25/2025", "tra-0001", 12_345.675, 100.005);
    draft.schedules[0][1] = remittance("07/25/2025", "eror 0002", 999.995, 0.0);
    draft.schedules[0][3] = remittance("01/26/2026", "Tra-0004", 250_000.0, 1_234.5);
    draft.schedules[1][3] = remittance("01/30/2026", "ear-4", 5_000.0, 0.0);
    draft.schedules[2][2] = remittance("10/30/2025", "x3", 1.5, 2.25);
    overlay(
        &mut audit,
        "1604F-2025",
        "1604F",
        draft,
        |_| fixed("1604f-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}

// --- 1606 ------------------------------------------------------------------

use bir_core::forms::form_1606::{
    Form1606AgentCategory, Form1606Draft, Form1606SellerType, Form1606TaxRate, Form1606TaxRelief,
    Form1606TaxableBaseBox, Form1606Transaction,
};

#[test]
fn audit_1606() {
    let mut audit = Audit { diffs: Vec::new() };
    let mut draft = Form1606Draft::new_from_profile(&dummy_profile("1606", "Corporation"), 2025, 1);
    draft.transaction_month = 6;
    draft.transaction_day = 15;
    draft.is_amended = true;
    draft.taxes_withheld = Some(true);
    draft.seller_tin = "98765432100000".into();
    draft.seller_rdo_code = "040".into();
    draft.seller_name = "Dela Cruz, Sample Seller".into();
    draft.seller_address = "456 Example Avenue, Makati City".into();
    draft.seller_type = Some(Form1606SellerType::Individual);
    draft.agent_category = Some(Form1606AgentCategory::Private);
    draft.property_rdo_code = "039".into();
    draft.tct_number = "T-123456".into();
    draft.area_sold = "250".into();
    draft.tax_declaration_number = "0123456789".into();
    draft.other_description = "Corner lot".into();
    draft.covers_more_than_one_property = Some(false);
    draft.tax_relief = Form1606TaxRelief::SpecialLaw;
    draft.set_transaction(Form1606Transaction::InstallmentSale);
    draft.gross_selling_price = 5_000_000.005;
    draft.cost_and_expenses = 3_200_000.0;
    draft.mortgage_assumed = 500_000.0;
    draft.initial_year_payments = 1_250_000.5;
    draft.installment_this_month = 104_166.675;
    draft.number_of_installments = 48.0;
    draft.fmv_land_tax_declaration = Some(3_000_000.0);
    draft.fmv_improvements_tax_declaration = Some(1_500_000.0);
    draft.fmv_land_zonal = Some(4_200_000.0);
    draft.fmv_improvements_bir = Some(0.0);
    draft.taxable_base_box = Some(Form1606TaxableBaseBox::InstallmentCollected);
    draft.installment_collected = 104_166.675;
    draft.seller_habitual = Some(true);
    draft.tax_rate = Some(Form1606TaxRate::OnePointFive);
    draft.keep_entered_penalties();
    overlay(
        &mut audit,
        "1606-06152025_T123456",
        "1606",
        draft,
        |_| fixed("1606-v2018"),
        |d| d.to_bir_xml_payload(),
    );
}
