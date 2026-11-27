//! The amended-return previous-payment gates. Each form's official captured
//! lock rule opens its previous-payment box only at Amended Yes; 1602Q is
//! excluded because its official page leaves item 20 enabled on a new return.

use super::*;
use bir_core::forms::agent::draft_locks::previous_payment::{self, Spec};

fn gate_flow(spec: &Spec) {
    let code = spec.form_code;
    let kind = kind_of(code).unwrap();
    let opened = open(code, 1);
    assert_eq!(
        opened["locks"][spec.key], spec.lock,
        "{code}: fresh opening lock"
    );
    assert_eq!(
        opened["lock_notes"][spec.key], spec.lock,
        "{code}: fresh opening note"
    );
    assert_eq!(
        opened["values"][spec.key], "0.00",
        "{code}: fresh opening value"
    );
    assert!(
        !agent::field_map(kind, &opened["draft"])
            .unwrap()
            .contains_key(spec.key),
        "{code}: unamended field map omits the previous-payment box"
    );

    let refused = fill(code, &opened["draft"], json!({spec.key: "42"}));
    assert_eq!(
        refused["rejected"][spec.key], spec.lock,
        "{code}: non-zero write refused while unanswered"
    );
    assert_eq!(
        refused["draft"][spec.path].as_f64().unwrap_or(0.0),
        0.0,
        "{code}: refused write leaves the model empty"
    );

    let zero = fill(code, &refused["draft"], json!({spec.key: "0"}));
    assert_eq!(zero["rejected"], json!({}), "{code}: zero write is a no-op");

    let yes = fill(code, &zero["draft"], json!({spec.yes_key: "true"}));
    assert_eq!(
        yes["relocks"][spec.key],
        Value::Null,
        "{code}: Amended Yes opens the box"
    );
    let typed = fill(code, &yes["draft"], json!({spec.key: "42"}));
    assert_eq!(
        typed["rejected"],
        json!({}),
        "{code}: amended write accepted"
    );
    assert_eq!(
        typed["draft"][spec.path].as_f64().unwrap_or(0.0),
        42.0,
        "{code}: amended amount is kept"
    );
    assert_eq!(
        agent::field_map(kind, &typed["draft"]).unwrap()[spec.key],
        "42.00",
        "{code}: amended field map carries the amount"
    );

    let no = fill(code, &typed["draft"], json!({spec.no_key: "true"}));
    assert_eq!(
        no["draft"][spec.path].as_f64().unwrap_or(0.0),
        0.0,
        "{code}: closing the gate clears the model"
    );
    assert_eq!(
        no["values"][spec.key], "0.00",
        "{code}: cleared value shown"
    );
    assert_eq!(
        no["relocks"][spec.key], spec.lock,
        "{code}: closing the gate shuts the box"
    );
    assert!(
        !agent::field_map(kind, &no["draft"])
            .unwrap()
            .contains_key(spec.key),
        "{code}: closing the gate omits the field map entry"
    );

    let yes_again = fill(code, &no["draft"], json!({spec.yes_key: "true"}));
    assert_eq!(
        yes_again["draft"][spec.path].as_f64().unwrap_or(0.0),
        0.0,
        "{code}: reopening revives no dormant amount"
    );
    assert_eq!(
        yes_again["values"][spec.key], "0.00",
        "{code}: reopened box remains empty"
    );
}

#[test]
fn amended_return_previous_payment_gates_shut_refuse_clear_and_stay_clear() {
    setup();
    for spec in &previous_payment::SPECS {
        gate_flow(spec);
    }
}

#[test]
fn amended_return_previous_payment_legacy_dormant_amounts_clear_on_open() {
    setup();
    for spec in &previous_payment::SPECS {
        let code = spec.form_code;
        let yes = fill(code, &open(code, 1)["draft"], json!({spec.yes_key: "true"}));
        let typed = fill(code, &yes["draft"], json!({spec.key: "42"}));
        let saved = save(code, &typed["draft"]);
        let id = stored_id(&saved);

        // Make the stored row a pre-fix dormant draft: unamended, yet holding
        // the previous-payment amount. Opening recomputes and clears it.
        let mut legacy: Value = serde_json::from_str(&raw_row(id).1).unwrap();
        legacy["is_amended"] = json!(false);
        legacy[spec.path] = json!(42.0);
        raw_exec(
            "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
            &[&serde_json::to_string(&legacy).unwrap(), &id.to_string()],
        );
        let first = open(code, 1);
        assert_eq!(
            first["draft"][spec.path].as_f64().unwrap_or(0.0),
            0.0,
            "{code}: legacy dormant amount is cleared on open"
        );
        assert_eq!(first["values"][spec.key], "0.00");
        assert_eq!(first["locks"][spec.key], spec.lock);
        let kind = kind_of(code).unwrap();
        assert!(
            !agent::field_map(kind, &first["draft"])
                .unwrap()
                .contains_key(spec.key),
            "{code}: opened legacy draft's field map omits the box"
        );
        let gate = format!(
            "{}_item{}",
            spec.form_code.to_lowercase(),
            spec.item.to_lowercase()
        );
        let notes = first["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 1, "{code}: one pending clearing notice");
        assert_eq!(notes[0]["text"], spec.cleared, "{code}: pending note text");
        let key = notes[0]["key"].as_str().unwrap().to_string();
        assert!(
            key.starts_with(&format!("notice_seen:{gate}:{id}:")),
            "{code}: pending key {key}"
        );
        let second = open(code, 1);
        assert_eq!(
            second["notes"].as_array().unwrap().len(),
            1,
            "{code}: an unseen pending note is offered again"
        );
        assert_eq!(second["notes"][0]["key"], key);
    }
}
