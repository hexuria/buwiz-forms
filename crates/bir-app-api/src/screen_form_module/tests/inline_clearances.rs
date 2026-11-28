//! Inline gate clearings are acknowledged by the save whose fill showed
//! them, so a reopen does not offer the same notice again.

use super::*;

fn save_and_reopen_has_no_notice(code: &str, period: i64, draft: &Value) {
    save(code, draft);
    let reopened = open(code, period);
    assert_eq!(
        reopened["notes"],
        json!([]),
        "{code}: reopening after the inline-shown save offers no second notice: {}",
        reopened["notes"]
    );
}

#[test]
fn inline_save_acknowledges_1606_installment_clearing() {
    setup();
    let installment = fill(
        "1606",
        &open("1606", 1)["draft"],
        json!({"frm1606:j_id395:_3": "true"}),
    );
    let typed = fill(
        "1606",
        &installment["draft"],
        json!({"frm1606:txtCost": "42"}),
    );
    let stored = save("1606", &typed["draft"]);
    let closed = fill(
        "1606",
        &stored["draft"],
        json!({"frm1606:j_id395:_1": "true"}),
    );
    assert_eq!(
        closed["relock_notes"]["frm1606:txtCost"],
        bir_core::forms::agent::draft_locks::installment_1606::CLEARED_NOTE
    );
    save_and_reopen_has_no_notice("1606", 1, &closed["draft"]);
}

#[test]
fn inline_save_acknowledges_1601fq_schedule1_row_clearing() {
    setup();
    let yes = fill(
        "1601FQ",
        &open("1601FQ", 1)["draft"],
        json!({"frm1601FQ:SpecialTax_1": "true"}),
    );
    let typed = fill(
        "1601FQ",
        &yes["draft"],
        json!({"txtAmtIncomePay0": "42", "txtRate0": "10"}),
    );
    let stored = save("1601FQ", &typed["draft"]);
    let closed = fill(
        "1601FQ",
        &stored["draft"],
        json!({"frm1601FQ:SpecialTax_2": "true"}),
    );
    let row_id = typed["draft"]["schedule1"][0]["instance_id"]
        .as_str()
        .expect("stable row id");
    let note = closed["relock_notes"]["txtAmtIncomePay0"]
        .as_str()
        .expect("inline clearing note");
    assert_eq!(
        note,
        "Schedule 1 row 1 was cleared: it applies only when item 13 is Yes."
    );
    assert!(!note.contains(row_id));
    save_and_reopen_has_no_notice("1601FQ", 1, &closed["draft"]);
}

fn validate(code: &str, draft: &Value) -> Value {
    call_async(
        "screen_form_validate",
        vec![code.into(), json(draft.clone())],
    )
    .unwrap_or_else(|error| panic!("{code} validates: {error}"))["draft"]
        .clone()
}

fn validate_save_and_reopen_has_no_notice(code: &str, period: i64, draft: &Value) {
    let validated = validate(code, draft);
    save(code, &validated);
    let reopened = open(code, period);
    assert_eq!(
        reopened["notes"],
        json!([]),
        "{code}: fill-validate-save-reopen offers no second notice: {}",
        reopened["notes"]
    );
}

#[test]
fn validate_save_acknowledges_1600pt_previous_payment_clearing() {
    setup();
    use bir_core::forms::agent::draft_locks::previous_payment as gate;
    let spec = gate::SPECS
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
    let closed = fill("1600PT", &stored["draft"], json!({spec.no_key: "true"}));
    assert_eq!(closed["relock_notes"][spec.key], spec.cleared);
    validate_save_and_reopen_has_no_notice("1600PT", 1, &closed["draft"]);
}

#[test]
fn validate_save_acknowledges_2551q_item16_clearing() {
    setup();
    use bir_core::forms::agent::draft_locks::item16_2551q as item16;
    let yes = fill(
        "2551Q",
        &open("2551Q", 1)["draft"],
        json!({"frm2551Qv2018:amendedRtn_1": "true"}),
    );
    let typed = fill("2551Q", &yes["draft"], json!({item16::KEY: "42"}));
    let stored = save("2551Q", &typed["draft"]);
    let closed = fill(
        "2551Q",
        &stored["draft"],
        json!({"frm2551Qv2018:amendedRtn_2": "true"}),
    );
    assert_eq!(
        closed["relock_notes"][item16::KEY],
        item16::CLEARED_AMENDED_NOTE
    );
    validate_save_and_reopen_has_no_notice("2551Q", 1, &closed["draft"]);
}

#[test]
fn validate_save_acknowledges_1606_installment_clearing() {
    setup();
    let installment = fill(
        "1606",
        &open("1606", 1)["draft"],
        json!({"frm1606:j_id395:_3": "true"}),
    );
    let typed = fill(
        "1606",
        &installment["draft"],
        json!({"frm1606:txtCost": "42"}),
    );
    let stored = save("1606", &typed["draft"]);
    let closed = fill(
        "1606",
        &stored["draft"],
        json!({"frm1606:j_id395:_1": "true"}),
    );
    assert_eq!(
        closed["relock_notes"]["frm1606:txtCost"],
        bir_core::forms::agent::draft_locks::installment_1606::CLEARED_NOTE
    );
    validate_save_and_reopen_has_no_notice("1606", 1, &closed["draft"]);
}
