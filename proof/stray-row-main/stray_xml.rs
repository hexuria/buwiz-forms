use bir_core::forms::{Form2551QDraft, Schedule1Row};
use bir_core::forms::form_2551q::Item13Election;

fn main() {
    let path = std::env::args().nth(1).expect("draft json path");
    let raw = std::fs::read_to_string(path).expect("read draft");
    let value: serde_json::Value = serde_json::from_str(&raw).expect("parse json");
    let mut draft: Form2551QDraft = serde_json::from_value(value).expect("decode 2551Q draft");
    draft.item_13_election = Item13Election::NotApplicable;
    let official_pt040 = Schedule1Row::new("PT040").expect("PT040");
    if let Some(row) = draft.schedule_1.iter_mut().find(|row| row.atc == "PT040") {
        row.atc_description = official_pt040.atc_description;
    }
    draft.recompute(None);
    let map = draft.to_bir_field_map();
    let selected: serde_json::Map<String, serde_json::Value> = map
        .iter()
        .filter(|(key, _)| {
            key.starts_with("drpATC")
                || key.starts_with("txtATCAmt")
                || key.starts_with("txtATCRate")
                || key.starts_with("txtATCDue")
                || key.starts_with("txtTaxDue")
                || key.starts_with("txtTotal")
        })
        .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
        .collect();
    println!("FIELD_MAP\n{}", serde_json::to_string_pretty(&selected).unwrap());
    match draft.to_bir_xml_payload() {
        Ok(xml) => {
            std::fs::write("/home/ubuntu/forms-validation/stray-row-main-proof/2551q-generated.xml", &xml).unwrap();
            println!("XML_OK bytes={}", xml.len());
            for key in ["drpATC1", "txtATCAmt1", "txtATCRate1", "txtATCDue1", "drpATC2", "txtATCAmt2", "txtATCRate2", "txtATCDue2", "txtTaxDue", "txtTotalAmtPayable"] {
                if let Some(pos) = xml.find(key) {
                    println!("{} context={}", key, &xml[pos.saturating_sub(30)..(pos + key.len() + 90).min(xml.len())].replace('\n', " "));
                }
            }
        }
        Err(errors) => println!("XML_VALIDATION_ERRORS\n{}", serde_json::to_string_pretty(&errors).unwrap()),
    }
}
