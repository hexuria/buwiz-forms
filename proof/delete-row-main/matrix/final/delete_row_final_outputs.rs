use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::forms::FormValidator;
use bir_core::forms::queueable::{QueueableForm, QueueableKind};
use bir_core::official_xml;
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;

fn zeroish(value: &str) -> bool {
    let cleaned: String = value
        .chars()
        .filter(|c| !matches!(c, ',' | '₱' | '%' | ' '))
        .collect();
    cleaned == "0" || cleaned == "0.0" || cleaned == "0.00" || cleaned == ".00" || cleaned == "false"
}

fn selected(map: &BTreeMap<String, String>, keys: &[String]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for key in keys {
        if let Some(value) = map.get(key) {
            out.insert(key.clone(), value.clone());
        }
    }
    out
}

fn slot_class(map: &BTreeMap<String, String>, keys: &[String]) -> &'static str {
    if keys.is_empty() {
        return "unknown_no_keys";
    }
    let mut present = 0usize;
    let mut blank = 0usize;
    let mut zero = 0usize;
    let mut content = 0usize;
    for key in keys {
        if let Some(value) = map.get(key) {
            present += 1;
            if value.trim().is_empty() {
                blank += 1;
            } else if zeroish(value) {
                zero += 1;
            } else {
                content += 1;
            }
        }
    }
    if present == 0 {
        "none_keys_absent"
    } else if content > 0 {
        "content_present"
    } else if zero > 0 {
        "zeros"
    } else if blank + zero == present {
        "blank_or_gap"
    } else {
        "unknown"
    }
}

fn mechanical_payload(layout_id: &str, fields: &BTreeMap<String, String>, keys1: &[String], keys2: &[String]) -> serde_json::Value {
    match official_xml::layout(layout_id) {
        Ok(layout) => match official_xml::write(layout, fields) {
            Ok(xml) => {
                let extract = |keys: &[String]| keys.iter().map(|key| {
                    let marker = format!("<div>{key}=");
                    let value = xml.find(&marker).and_then(|at| {
                        let rest = &xml[at + marker.len()..];
                        rest.find(&format!("{key}=</div>")).map(|end| rest[..end].to_string())
                    });
                    (key.clone(), json!({"field_map_value": fields.get(key), "serialized_xml_value": value, "directly_extracted": value.is_some()}))
                }).collect::<BTreeMap<_, _>>();
                json!({"label":"mechanical_payload_after_validation_bypassed","serialization":"ok","bytes":xml.len(),"first_row":extract(keys1),"second_row":extract(keys2),"extraction_note":"values extracted from the mechanical plaintext only when the exact official control key appears; this is serializer evidence, not a queueable payload"})
            }
            Err(error) => json!({"label":"mechanical_payload_after_validation_bypassed","serialization":"error","error":format!("{error:?}"),"first_row":selected(fields,keys1),"second_row":selected(fields,keys2),"extraction_note":"official_xml::write rejected the field map before serialization; direct row extraction impossible"})
        },
        Err(error) => json!({"label":"mechanical_payload_after_validation_bypassed","serialization":"layout_error","error":format!("{error:?}")})
    }
}

fn inspect_typed<T: QueueableForm>(value: serde_json::Value, keys1: &[String], keys2: &[String]) -> serde_json::Value {
    match serde_json::from_value::<T>(value) {
        Ok(mut form) => {
            form.compute();
            let validation = form.validate();
            let fields = form.field_map();
            let print = form.print_field_map();
            let payload = form.official_payload();
            let mechanical = mechanical_payload(T::LAYOUT_ID, &fields, keys1, keys2);
            json!({
                "deserialize": "ok",
                "validation_error_count": validation.len(),
                "validation_errors": validation,
                "field_map_first_row": selected(&fields, keys1),
                "print_map_first_row": selected(&print, keys1),
                "field_map_second_row": selected(&fields, keys2),
                "print_map_second_row": selected(&print, keys2),
                "field_map_slot_class": slot_class(&fields, keys1),
                "print_slot_class": slot_class(&print, keys1),
                "field_map_second_slot_class": slot_class(&fields, keys2),
                "print_second_slot_class": slot_class(&print, keys2),
                "official_payload": match payload {
                    Ok(payload) => json!({"ok": true, "bytes": payload.len()}),
                    Err(errors) => json!({"ok": false, "errors": errors}),
                },
                "mechanical_payload_after_validation_bypassed": mechanical
            })
        }
        Err(error) => json!({"deserialize": "failed", "error": error.to_string()}),
    }
}


fn inspect_2551q(value: serde_json::Value, keys1: &[String], keys2: &[String]) -> serde_json::Value {
    match serde_json::from_value::<Form2551QDraft>(value) {
        Ok(mut form) => {
            form.recompute(None);
            let validation = <Form2551QDraft as FormValidator>::validate(&form);
            let fields = form.to_bir_field_map();
            let print = form.to_print_field_map();
            let payload = form.to_bir_xml_payload();
            let mechanical = mechanical_payload("2551q-v2018", &fields, keys1, keys2);
            json!({
                "deserialize": "ok",
                "validation_error_count": validation.len(),
                "validation_errors": validation,
                "field_map_first_row": selected(&fields, keys1),
                "print_map_first_row": selected(&print, keys1),
                "field_map_second_row": selected(&fields, keys2),
                "print_map_second_row": selected(&print, keys2),
                "field_map_slot_class": slot_class(&fields, keys1),
                "print_slot_class": slot_class(&print, keys1),
                "field_map_second_slot_class": slot_class(&fields, keys2),
                "print_second_slot_class": slot_class(&print, keys2),
                "official_payload": match payload {
                    Ok(payload) => json!({"ok": true, "bytes": payload.len()}),
                    Err(errors) => json!({"ok": false, "errors": errors}),
                },
                "mechanical_payload_after_validation_bypassed": mechanical
            })
        }
        Err(error) => json!({"deserialize": "failed", "error": error.to_string()}),
    }
}

fn main() {
    let dir = "/home/ubuntu/forms-validation/delete-row-main-proof/matrix/final";
    let rows_raw = fs::read_to_string(format!("{dir}/rows.jsonl")).expect("rows.jsonl");
    let mut outputs = Vec::new();
    for line in rows_raw.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let row: serde_json::Value = serde_json::from_str(line).expect("matrix row json");
        let id = row["id"].as_str().unwrap_or("").to_string();
        let form = row["form"].as_str().unwrap_or("").to_string();
        let keys1: Vec<String> = row["first_row_keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        let keys2: Vec<String> = row["second_row_keys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        let path = format!("{dir}/drafts/{id}.json");
        let output = if !matches!(row["status"].as_str(), Some("completed") | Some("completed_direct_seed") | Some("completed_with_seed_rejections")) {
            json!({"inspect": "skipped", "reason": "delete did not complete"})
        } else if form == "1601C" {
            json!({"inspect": "skipped", "reason": "1601C is a special non-generic form outside QueueableKind; output not inspected in this generic harness"})
        } else {
            let raw = match fs::read_to_string(&path) {
                Ok(raw) => raw,
                Err(error) => {
                    outputs.push(json!({"id": id, "form": form, "output_error": format!("read draft: {error}")}));
                    continue;
                }
            };
            let draft: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(draft) => draft,
                Err(error) => {
                    outputs.push(json!({"id": id, "form": form, "output_error": format!("draft json: {error}")}));
                    continue;
                }
            };
            if form == "2551Q" {
                inspect_2551q(draft, &keys1, &keys2)
            } else if let Some(kind) = QueueableKind::from_form_code(&form) {
                bir_core::with_queueable_kind!(kind, F => inspect_typed::<F>(draft, &keys1, &keys2))
            } else {
                json!({"inspect": "skipped", "reason": "form is not a QueueableKind and has no explicit harness"})
            }
        };
        outputs.push(json!({"id": id, "form": form, "output": output}));
    }
    fs::write(
        format!("{dir}/outputs.json"),
        serde_json::to_string_pretty(&outputs).expect("json"),
    )
    .expect("write outputs");
    println!("matrix outputs written: {}", outputs.len());
}
