use bir_core::forms::form_1601eq::Form1601EqDraft;
use bir_core::forms::form_2551q::Item13Election;
use bir_core::forms::{Form2551QDraft, Schedule1Row};
use serde_json::json;

fn read<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let raw = std::fs::read_to_string(path).expect("read draft json");
    serde_json::from_str(&raw).expect("decode draft json")
}

fn selected(map: &std::collections::BTreeMap<String, String>, needles: &[&str]) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (key, value) in map {
        if needles.iter().any(|needle| key.contains(needle)) {
            out.insert(key.clone(), json!(value));
        }
    }
    json!(out)
}

fn main() {
    let out = "/home/ubuntu/forms-validation/delete-row-main-proof";
    let mut q: Form2551QDraft = read(&format!("{out}/session-2551q-after-draft.json"));
    // The runtime scratch draft is intentionally minimal. These two in-memory repairs only make
    // unrelated queue validation pass; the row data under test is not changed.
    q.item_13_election = Item13Election::NotApplicable;
    if let Some(official) = Schedule1Row::new("PT040") {
        if let Some(row) = q.schedule_1.iter_mut().find(|row| row.atc == "PT040") {
            row.atc_description = official.atc_description;
        }
    }
    q.recompute(None);
    let q_map = q.to_bir_field_map();
    let q_print = q.to_print_field_map();
    let q_selected = selected(&q_map, &["drpATC", "txtATCAmt", "txtATCRate", "txtATCDue", "txtTotalSched1", "txt14"]);
    let q_print_selected = selected(&q_print, &["drpATC", "txtATCAmt", "txtATCRate", "txtATCDue", "txtTotalSched1", "txt14"]);
    let q_payload = q.to_bir_xml_payload();
    let mut q_summary = json!({
        "draft_rows": q.schedule_1,
        "field_map_selected": q_selected,
        "print_field_map_selected": q_print_selected,
    });
    match q_payload {
        Ok(payload) => {
            std::fs::write(format!("{out}/session-2551q-generated-payload.txt"), &payload).unwrap();
            q_summary["payload"] = json!({"ok": true, "bytes": payload.len(), "path": "session-2551q-generated-payload.txt"});
        }
        Err(errors) => q_summary["payload"] = json!({"ok": false, "errors": errors}),
    }
    std::fs::write(format!("{out}/session-2551q-payload-inspection.json"), serde_json::to_string_pretty(&q_summary).unwrap()).unwrap();

    let mut eq: Form1601EqDraft = read(&format!("{out}/session-1601eq-after-draft.json"));
    // The scratch return was opened for the still-current fourth quarter. Move only the
    // unrelated period in memory so payload validation isolates the row under test.
    eq.quarter = 1;
    eq.recompute();
    let eq_map = eq.to_bir_field_map();
    let eq_print = eq.to_print_field_map();
    let needles = ["txtAtcCd", "txtTaxBase", "txtTaxRate", "txtTaxbeWithHeld", "txtTotalTaxWithheld", "txtTotTaxWithheld", "AtcCode"];
    let eq_selected = selected(&eq_map, &needles);
    let eq_print_selected = selected(&eq_print, &needles);
    let eq_payload = eq.to_bir_xml_payload();
    let mut eq_summary = json!({
        "draft_rows": eq.schedule,
        "field_map_selected": eq_selected,
        "print_field_map_selected": eq_print_selected,
    });
    match eq_payload {
        Ok(payload) => {
            std::fs::write(format!("{out}/session-1601eq-generated-payload.txt"), &payload).unwrap();
            eq_summary["payload"] = json!({"ok": true, "bytes": payload.len(), "path": "session-1601eq-generated-payload.txt"});
        }
        Err(errors) => eq_summary["payload"] = json!({"ok": false, "errors": errors}),
    }
    std::fs::write(format!("{out}/session-1601eq-payload-inspection.json"), serde_json::to_string_pretty(&eq_summary).unwrap()).unwrap();
    println!("payload inspection written");
}
