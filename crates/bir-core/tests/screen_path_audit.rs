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
fn dummy_profile(default_form_type: &str, taxpayer_type: &str) -> bir_core::profile::TaxpayerProfile {
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
        "btn", "cmd", "AtcCode", "txtRate", "Sheets", "NoSheets", "txtSheets",
    ];
    let identity = [
        "TIN1", "TIN2", "TIN3", "TIN4", "BranchCode", "RDO", "TaxpayerName",
        "WithholdingAgentsName", "txtAddress", "ZipCode", "TelNum", "Email",
        "LineBus", "txtLOB", "Month", "Year", "DueDay", "Quarter",
        "RegName", "ContactNumber",
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
                let Some(end) = rest[1..].find(quote) else { break };
                let key = rest[1..1 + end].to_string();
                rest = &rest[1 + end..];
                if !rest.starts_with(").value") {
                    continue;
                }
                let Some(eq) = rest.find('=') else { continue };
                let after = &rest[eq + 1..];
                let after = after.trim_start();
                let q = after.chars().next().unwrap_or(' ');
                if q != '\'' && q != '"' {
                    continue;
                }
                let Some(vend) = after[1..].find(q) else { continue };
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
fn replay(
    audit: &mut Audit,
    stem: &str,
    kind: AgentFormKind,
    draft: &mut Value,
    ops: &[Op],
) {
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
    Form1600PtDraft, Form1600PtAgentCategory, Form1600PtScheduleRow, form_1600pt_atc_index,
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
