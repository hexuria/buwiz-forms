//! The amended-return previous-payment gates. Each kept form's in-repository
//! runtime-control inventory records its box disabled at load; 0619E, 0619F,
//! 2550M and 1602Q are excluded because their captured pages leave the
//! surveyed box enabled on a new return.

use super::*;
use bir_core::forms::agent::draft_locks::previous_payment::{self, Spec};

fn gate_key(spec: &Spec) -> String {
    format!(
        "{}_item{}",
        spec.form_code.to_lowercase(),
        spec.item.to_lowercase()
    )
}

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
    assert_eq!(
        agent::field_map(kind, &opened["draft"]).unwrap()[spec.key],
        "0.00",
        "{code}: unamended field map carries the shut box as zero"
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
    for value in [json!("₱0.00"), json!("(0.00)"), json!(0)] {
        let zero = fill(code, &zero["draft"], json!({spec.key: value}));
        assert_eq!(
            zero["rejected"],
            json!({}),
            "{code}: zero spelling {value} is a no-op"
        );
    }

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
    assert_eq!(
        no["relock_notes"][spec.key], spec.cleared,
        "{code}: closing the gate says what was cleared inline"
    );
    assert_eq!(
        agent::field_map(kind, &no["draft"]).unwrap()[spec.key],
        "0.00",
        "{code}: closing the gate leaves zero in the field map"
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
        assert_eq!(
            agent::field_map(kind, &first["draft"]).unwrap()[spec.key],
            "0.00",
            "{code}: opened legacy draft's field map carries the shut box as zero"
        );
        let gate = gate_key(spec);
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

#[test]
fn amended_return_previous_payment_yes_to_no_save_records_the_clearing() {
    setup();
    for spec in &previous_payment::SPECS {
        let code = spec.form_code;
        let yes = fill(code, &open(code, 1)["draft"], json!({spec.yes_key: "true"}));
        let typed = fill(code, &yes["draft"], json!({spec.key: "42"}));
        let saved = save(code, &typed["draft"]);
        let id = stored_id(&saved);
        let no = fill(code, &saved["draft"], json!({spec.no_key: "true"}));
        assert_eq!(no["relock_notes"][spec.key], spec.cleared);
        save(code, &no["draft"]);

        // The inline note was shown, so the transactional pending notice is
        // immediately acknowledged and removed; its seen key is the evidence
        // that the save recorded the clearing event.
        let gate = gate_key(spec);
        assert_eq!(
            raw_count(&format!(
                "SELECT count(*) FROM settings WHERE key LIKE 'notice_seen:{gate}:{id}:%'"
            )),
            1,
            "{code}: Yes-to-No save records the clearing event"
        );
        assert_eq!(
            raw_count(&format!(
                "SELECT count(*) FROM settings WHERE key = 'notice_pending:{gate}:{id}'"
            )),
            0,
            "{code}: the inline-shown pending notice is acknowledged"
        );
    }
}

#[test]
fn amended_return_previous_payment_unreported_yes_to_no_save_leaves_the_notice_pending() {
    setup();
    let spec = previous_payment::SPECS
        .iter()
        .find(|spec| spec.form_code == "1600PT")
        .unwrap();
    let yes = fill(
        "1600PT",
        &open("1600PT", 1)["draft"],
        json!({spec.yes_key: "true"}),
    );
    let typed = fill("1600PT", &yes["draft"], json!({spec.key: "42"}));
    let saved = save("1600PT", &typed["draft"]);
    let id = stored_id(&saved);

    // A Yes-to-No transition saved without the page's inline-clearing report
    // with the amount still incoming must stay pending
    // for a later open rather than being marked unseen-and-seen by the save.
    let mut closed = saved["draft"].clone();
    closed["is_amended"] = json!(false);
    closed[spec.path] = json!(42.0);
    save("1600PT", &closed);
    let gate = gate_key(spec);
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key = 'notice_pending:{gate}:{id}'"
        )),
        1,
        "an unreported clearing remains pending"
    );
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key LIKE 'notice_seen:{gate}:{id}:%'"
        )),
        0,
        "an unreported clearing is not marked seen"
    );
}

#[test]
fn amended_return_previous_payment_save_amendment_records_yes_to_no() {
    setup();
    let spec = previous_payment::SPECS
        .iter()
        .find(|spec| spec.form_code == "1600PT")
        .unwrap();
    let yes = fill(
        "1600PT",
        &open("1600PT", 1)["draft"],
        json!({spec.yes_key: "true"}),
    );
    let typed = fill("1600PT", &yes["draft"], json!({spec.key: "42"}));
    let saved = save("1600PT", &typed["draft"]);
    let id = stored_id(&saved);
    let filed = with_status(&serde_json::from_str(&raw_row(id).1).unwrap(), "Submitted");
    raw_exec(
        "UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = ?2",
        &[&serde_json::to_string(&filed).unwrap(), &id.to_string()],
    );

    let amended = call_async(
        "screen_form_amend",
        vec![TIN.into(), "1600PT".into(), YEAR.into(), 1.into()],
    )
    .unwrap_or_else(|error| panic!("1600PT amendment opens: {error}"));
    let no = fill("1600PT", &amended["draft"], json!({spec.no_key: "true"}));
    assert_eq!(no["relock_notes"][spec.key], spec.cleared);
    save("1600PT", &no["draft"]);
    let gate = gate_key(spec);
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key LIKE 'notice_seen:{gate}:{id}:%'"
        )),
        1,
        "save_amendment records the clearing event"
    );
}

#[test]
fn amended_return_previous_payment_a_dormant_filed_original_is_not_revived_by_amending() {
    setup();
    let spec = previous_payment::SPECS
        .iter()
        .find(|spec| spec.form_code == "1600PT")
        .unwrap();
    let yes = fill(
        "1600PT",
        &open("1600PT", 1)["draft"],
        json!({spec.yes_key: "true"}),
    );
    let typed = fill("1600PT", &yes["draft"], json!({spec.key: "42"}));
    let saved = save("1600PT", &typed["draft"]);
    let id = stored_id(&saved);
    let mut filed: Value = serde_json::from_str(&raw_row(id).1).unwrap();
    filed = with_status(&filed, "Submitted");
    filed["is_amended"] = json!(false);
    raw_exec(
        "UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = ?2",
        &[&serde_json::to_string(&filed).unwrap(), &id.to_string()],
    );

    let amended = call_async(
        "screen_form_amend",
        vec![TIN.into(), "1600PT".into(), YEAR.into(), 1.into()],
    )
    .unwrap_or_else(|error| panic!("1600PT amendment opens: {error}"));
    assert_eq!(amended["draft"]["is_amended"], true);
    assert_eq!(amended["draft"][spec.path].as_f64().unwrap_or(0.0), 0.0);
    assert_eq!(amended["values"][spec.key], "0.00");

    save("1600PT", &amended["draft"]);
    let gate = gate_key(spec);
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key = 'notice_pending:{gate}:{id}'"
        )),
        1,
        "the amendment save records the dormant original's clearing"
    );
}

#[test]
fn previous_payment_zeroed_by_the_filer_before_closing_records_no_clearing_notice() {
    setup();
    let spec = previous_payment::SPECS
        .iter()
        .find(|spec| spec.form_code == "1600PT")
        .unwrap();
    let yes = fill(
        "1600PT",
        &open("1600PT", 1)["draft"],
        json!({spec.yes_key: "true"}),
    );
    let typed = fill("1600PT", &yes["draft"], json!({spec.key: "42"}));
    let stored = save("1600PT", &typed["draft"]);
    let id = stored_id(&stored);

    // The filer clears the open box themselves. Closing the gate afterwards
    // has nothing for the recompute to clear, so the save records no notice.
    let zeroed = fill("1600PT", &stored["draft"], json!({spec.key: "0"}));
    assert_eq!(zeroed["rejected"], json!({}));
    assert_eq!(zeroed["draft"][spec.path], 0.0);
    let closed = fill("1600PT", &zeroed["draft"], json!({spec.no_key: "true"}));
    assert_ne!(closed["relock_notes"][spec.key], spec.cleared);
    save("1600PT", &closed["draft"]);

    let gate = gate_key(spec);
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key = 'notice_pending:{gate}:{id}'"
        )),
        0,
        "a filer-cleared amount leaves no pending notice"
    );
    assert_eq!(
        raw_count(&format!(
            "SELECT count(*) FROM settings WHERE key LIKE 'notice_seen:{gate}:{id}:%'"
        )),
        0,
        "a filer-cleared amount records no clearing event"
    );
}

#[test]
fn previous_payment_amendment_carryover_notice_names_the_filed_return() {
    let spec = previous_payment::SPECS
        .iter()
        .find(|spec| spec.form_code == "1600PT")
        .unwrap();
    let before = json!({"is_amended": false, spec.path: 42.0});
    let after = json!({"is_amended": true, spec.path: 0.0});
    let clearances = bir_core::forms::agent::draft_locks::pending_notices::clearance(
        kind_of("1600PT").unwrap(),
        &before,
        &after,
    );
    assert_eq!(
        clearances,
        vec![(
            gate_key(spec),
            "The filed return's Item 19 was not carried over.".to_string()
        )]
    );
}
