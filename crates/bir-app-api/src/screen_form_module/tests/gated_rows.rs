//! Gated schedule/specification boxes: shut while their own recompute would
//! clear a valid entry, opened by the controlling answer, and cleared with
//! one notice when that answer closes the gate.

use super::*;

fn payload_map(code: &str, draft: &Value) -> std::collections::BTreeMap<String, String> {
    agent::field_map(kind_of(code).unwrap(), draft).unwrap()
}

fn printed_map(code: &str, draft: &Value) -> std::collections::BTreeMap<String, String> {
    let kind = kind_of(code).unwrap();
    with_database(|db| print_map(db, kind, draft)).unwrap().0
}

fn notes_seen(keys: &[String]) -> Vec<String> {
    call("screen_form_notes_seen", vec![json(json!(keys))])
        .expect("notes seen")
        .as_array()
        .expect("recorded notes")
        .iter()
        .filter_map(|key| key.as_str().map(str::to_string))
        .collect()
}

fn note_texts(opened: &Value) -> Vec<String> {
    opened["notes"]
        .as_array()
        .expect("notes array")
        .iter()
        .filter_map(|note| note["text"].as_str().map(str::to_string))
        .collect()
}

fn note_keys(opened: &Value) -> Vec<String> {
    opened["notes"]
        .as_array()
        .expect("notes array")
        .iter()
        .filter_map(|note| note["key"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn gated_row_lock_notes_are_looked_up_by_form_and_key() {
    setup();
    let rt = open("1702RT", 0);
    assert_eq!(
        rt["lock_notes"]["frm1702RT:txtPg2Pt4I51TaxPaidInReturn"],
        json!("Item 51 is for an amended return.")
    );
    let mx = open("1702MX", 0);
    assert_eq!(
        mx["lock_notes"]["frm1702MX:txtPg2Sc3It27A"],
        json!("Schedule 3 item 27 is for an amended return.")
    );
    let dst = open("2000", 6);
    assert_eq!(
        dst["lock_notes"]["frm2000:txtTax15A"],
        json!("Item 15A is for an amended return.")
    );
    let annual_a = open("1701A", 0);
    assert_eq!(
        annual_a["lock_notes"]["frm1701A:txt23B"],
        json!("Column B is for a joint return.")
    );
    assert_eq!(
        bir_core::forms::agent::gated_rows::lock_note(
            "1702RT",
            "frm1702RT:txtPg2Pt4I51TaxPaidInReturn",
            "MCIT applies to this return."
        ),
        None,
        "another layer's coincidentally shared reason must not borrow this gate's note"
    );
}

#[test]
fn form_2552_tax_relief_specification_is_gated_and_said_once() {
    setup();
    let spec = "frm2552:txtPg1Pt1I10TaxReliefSpec";
    let yes = "frm2552:rdoPg1I10TaxReliefYes";
    let no = "frm2552:rdoPg1I10TaxReliefNo";
    let opened = open("2552", 1);

    let refused = fill("2552", &opened["draft"], json!({ spec: "SPECIAL LAW" }));
    assert_eq!(
        refused["rejected"][spec],
        json!("Item 10 tax relief is Yes.")
    );
    assert_eq!(refused["draft"]["tax_relief_specification"], "");
    let numeric_zero = fill("2552", &opened["draft"], json!({ spec: 0 }));
    assert!(
        numeric_zero["rejected"].get(spec).is_none(),
        "a JSON numeric zero is the same clear as the string zero: {:?}",
        numeric_zero["rejected"]
    );
    let mut assistant_draft = opened["draft"].clone();
    let refusal = bir_core::forms::agent::fill(
        kind_of("2552").expect("2552 kind"),
        &mut assistant_draft,
        spec,
        &json!("ASSISTANT WRITE"),
        None,
    )
    .expect_err("the assistant cannot write a shut specification");
    assert!(
        refusal.contains("Item 10 tax relief is Yes"),
        "assistant refusal: {refusal}"
    );

    let batch = fill(
        "2552",
        &opened["draft"],
        json!({ yes: "true", spec: "SPECIAL LAW" }),
    );
    assert!(
        batch["rejected"].get(spec).is_none(),
        "opened in the same batch: {:?}",
        batch["rejected"]
    );
    assert_eq!(batch["draft"]["tax_relief_specification"], "SPECIAL LAW");

    let closed = fill("2552", &batch["draft"], json!({ no: "true" }));
    assert_eq!(closed["draft"]["tax_relief_specification"], "");
    assert_eq!(
        closed["relock_notes"][spec],
        json!("The tax-relief specification was cleared: Item 10 is not Yes.")
    );
    assert_eq!(payload_map("2552", &closed["draft"])[spec], "");
    assert_eq!(printed_map("2552", &closed["draft"])[spec], "");

    // The page reported that clearing inline. Validation may rewrite the
    // draft, but the save that follows still marks exactly that gate seen:
    // reopening does not offer the same clearing again.
    let saved = save("2552", &batch["draft"]);
    let closed = fill("2552", &saved["draft"], json!({ no: "true" }));
    let checked = validated("2552", &closed["draft"]);
    save("2552", &checked["draft"]);
    let reopened = open("2552", 1);
    assert!(
        !note_texts(&reopened)
            .iter()
            .any(|text| text.contains("tax-relief specification was cleared")),
        "inline-reported clearing repeated: {:?}",
        reopened["notes"]
    );

    // A stored pre-gate draft is the old-draft case: opening clears it, the
    // pending sentence survives a failed first open, and seen once it does
    // not repeat. Write the dormant bytes below the row so this is the old
    // draft case rather than a page fill the page already reported inline.
    let saved_old = save("2552", &open("2552", 2)["draft"]);
    let old_id = saved_old["id"].as_i64().expect("old draft id");
    let mut old = saved_old["draft"].clone();
    old["tax_relief_specification"] = json!("DORMANT RELIEF");
    let old_bytes = serde_json::to_string(&old).expect("old draft json");
    raw_exec(
        "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
        &[&old_bytes, &old_id.to_string()],
    );
    let first = open("2552", 2);
    assert_eq!(first["draft"]["tax_relief_specification"], "");
    assert!(
        note_texts(&first)
            .iter()
            .any(|text| text.contains("tax-relief specification was cleared")),
        "first notes: {:?}",
        first["notes"]
    );
    let second = open("2552", 2);
    assert!(
        note_texts(&second)
            .iter()
            .any(|text| text.contains("tax-relief specification was cleared")),
        "unseen repeat notes: {:?}",
        second["notes"]
    );
    let recorded = notes_seen(&note_keys(&second));
    assert!(!recorded.is_empty());
    let third = open("2552", 2);
    assert!(
        !note_texts(&third)
            .iter()
            .any(|text| text.contains("tax-relief specification was cleared")),
        "seen notes: {:?}",
        third["notes"]
    );

    // A row really stored before this gate (not one re-saved through the new
    // compute) still carries the dormant text. Opening it must force the
    // recompute/save even when the profile has not moved, then offer the note.
    let saved = save("2552", &open("2552", 3)["draft"]);
    let id = saved["id"].as_i64().expect("saved draft id");
    let mut legacy = saved["draft"].clone();
    legacy["tax_relief_specification"] = json!("LEGACY DORMANT RELIEF");
    let bytes = serde_json::to_string(&legacy).expect("legacy draft json");
    raw_exec(
        "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
        &[&bytes, &id.to_string()],
    );
    let reopened = open("2552", 3);
    assert_eq!(reopened["draft"]["tax_relief_specification"], "");
    assert!(
        note_texts(&reopened)
            .iter()
            .any(|text| text.contains("tax-relief specification was cleared")),
        "legacy open notes: {:?}",
        reopened["notes"]
    );
    let (_, stored_json) = raw_row(id);
    assert!(
        !stored_json.contains("LEGACY DORMANT RELIEF"),
        "the open saved the clearing: {stored_json}"
    );
}

#[test]
fn form_2000_other_party_and_balance_are_gated() {
    setup();
    let name = "frm2000:txtOtherName";
    let tin = "frm2000:txtOtherTIN";
    let balance = "frm2000:txtTax15A";
    let opened = open("2000", 6);

    for (key, value) in [
        (name, json!("Lender Corp")),
        (tin, json!("123-456-789")),
        (balance, json!("250")),
    ] {
        let refused = fill("2000", &opened["draft"], json!({ key: value }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let creditor = fill(
        "2000",
        &opened["draft"],
        json!({"frm2000:optParty_1": "true"}),
    );
    let named = fill(
        "2000",
        &creditor["draft"],
        json!({ name: "Lender Corp", tin: "123-456-789" }),
    );
    for key in [name, tin] {
        assert!(
            named["rejected"].get(key).is_none(),
            "creditor opens {key}: {:?}",
            named["rejected"]
        );
    }
    assert_eq!(named["draft"]["other_party_name"], "Lender Corp");
    assert_eq!(named["draft"]["other_party_tin"], "123-456-789");

    let none = fill(
        "2000",
        &named["draft"],
        json!({"frm2000:optParty_3": "true"}),
    );
    assert_eq!(none["draft"]["other_party_name"], "");
    assert_eq!(none["draft"]["other_party_tin"], "");
    for key in [name, tin] {
        assert_eq!(
            none["relock_notes"][key],
            json!("The other-party details were cleared: Item 11 names no other party.")
        );
    }

    let amended = fill(
        "2000",
        &opened["draft"],
        json!({"frm2000:AmendedRtn_1": "true"}),
    );
    let typed = fill("2000", &amended["draft"], json!({ balance: "250" }));
    assert!(
        typed["rejected"].get(balance).is_none(),
        "amended opens 15A: {:?}",
        typed["rejected"]
    );
    assert_eq!(typed["draft"]["balance_carried_over"], 250.0);
}

#[test]
fn form_1707_other_detail_and_installment_rows_are_gated() {
    setup();
    let detail = "frm1707:txtPg1I9TransDescSpec";
    let group = "frm1707:rdoPg1I9TransDesc";
    let selling = "frm1707:txtPg2P4Sch1I1SellPrice";
    let collection_date = "frm1707:txtPg2P4Sch1I8DateCollection";
    let total_collection = "frm1707:txtPg2P4Sch1I9TotCollection";
    let opened = open("1707", 1);

    for (key, value) in [
        (detail, json!("OTHER DETAIL")),
        (selling, json!("1000")),
        (collection_date, json!("01/02/2026")),
        (total_collection, json!("250")),
    ] {
        let refused = fill("1707", &opened["draft"], json!({ key: value }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let other = fill(
        "1707",
        &opened["draft"],
        json!({ group: "Others", detail: "OTHER DETAIL" }),
    );
    assert!(
        other["rejected"].get(detail).is_none(),
        "Others opens the detail: {:?}",
        other["rejected"]
    );
    assert_eq!(other["draft"]["transaction_description"], "OTHER DETAIL");

    let cash = fill("1707", &other["draft"], json!({ group: "Cash" }));
    assert_eq!(cash["draft"]["transaction_description"], "");
    assert_eq!(
        cash["relock_notes"][detail],
        json!("The transaction detail was cleared: Item 9 is not Others.")
    );

    let installment = fill(
        "1707",
        &opened["draft"],
        json!({
            group: "Installment",
            selling: "1000",
            collection_date: "01/02/2026",
            total_collection: "250",
        }),
    );
    assert!(
        installment["rejected"].get(selling).is_none(),
        "Installment opens Schedule 1: {:?}",
        installment["rejected"]
    );
    assert_eq!(installment["draft"]["installment"]["selling_price"], 1000.0);

    let closed = fill("1707", &installment["draft"], json!({ group: "Cash" }));
    assert_eq!(closed["draft"]["installment"]["selling_price"], 0.0);
    for key in [selling, collection_date, total_collection] {
        assert_eq!(
            closed["relock_notes"][key],
            json!("Schedule 1 was cleared: Item 9 is not Installment Sale.")
        );
    }
}

#[test]
fn form_1701_employer_amounts_typed_before_the_owner_tick_survive() {
    setup();
    let opened = open("1701", 0);
    let income = "frm1701:txtPg2IShed1c_1CI";
    let withheld = "frm1701:txtPg2IShed1c_1TW";
    let typed = fill(
        "1701",
        &opened["draft"],
        json!({ income: "1000", withheld: "100" }),
    );
    assert_eq!(
        typed["draft"]["employers"][0]["compensation_income"],
        1000.0
    );
    assert_eq!(typed["draft"]["employers"][0]["tax_withheld"], 100.0);

    let compensation = fill(
        "1701",
        &typed["draft"],
        json!({"frm1701:rdoPg1I7ATC_II013": "true"}),
    );
    assert_eq!(
        compensation["draft"]["employers"][0]["compensation_income"], 1000.0,
        "answering Item 7 after the amounts does not wipe them"
    );
    let owned = fill(
        "1701",
        &compensation["draft"],
        json!({"frm1701:chkPg2IShed1a_1Taxpayer": "true"}),
    );
    assert_eq!(owned["draft"]["employers"][0]["owner"], "Taxpayer");
    assert_eq!(
        owned["draft"]["employers"][0]["compensation_income"],
        1000.0
    );
    assert_eq!(owned["draft"]["employers"][0]["tax_withheld"], 100.0);
}

#[test]
fn form_1701_employer_row_notice_covers_content_without_a_name_and_names_the_owner() {
    setup();
    let opened = open("1701", 0);
    let taxpayer_name = "frm1701:txtPg2IShed1a_1TPName";
    let compensation = fill(
        "1701",
        &opened["draft"],
        json!({
            "frm1701:rdoPg1I7ATC_II013": "true",
            "frm1701:chkPg2IShed1a_1Taxpayer": "true",
            "frm1701:txtPg2IShed1c_1CI": "1000",
            "frm1701:txtPg2IShed1c_1TW": "100",
        }),
    );
    assert_eq!(compensation["draft"]["employers"][0]["employer_name"], "");
    let closed = fill(
        "1701",
        &compensation["draft"],
        json!({"frm1701:rdoPg1I7ATC_II012": "true"}),
    );
    assert_eq!(
        closed["draft"]["employers"][0]["compensation_income"],
        serde_json::Value::Null
    );
    assert_eq!(
        closed["relock_notes"][taxpayer_name],
        json!(
            "Employer row 1 was cleared: Schedule 1 applies only to compensation income of the taxpayer."
        ),
        "a row with no name but typed amounts is still reported, in the taxpayer's words"
    );
}

#[test]
fn form_1701_spouse_item27_foreign_and_schedule5_are_gated() {
    setup();
    let opened = open("1701", 0);
    let spouse_name = "frm1701:txtPg2IShed1a_1SName";
    let item27 = "frm1701:txtPg3IShed3_27Desc";
    let foreign = "frm1701:txtPg1I14ForeignTaxNumber";
    let sched5 = "frm1701:txtPg3IShed5_1Desc";

    for (key, value) in [
        (spouse_name, json!("SPOUSE EMPLOYER")),
        (item27, json!("OTHER EIGHT PERCENT")),
        (foreign, json!("FTN-1")),
        (sched5, json!("ITEMIZED COST")),
    ] {
        let refused = fill("1701", &opened["draft"], json!({ key: value }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let compensation_without_owner = fill(
        "1701",
        &opened["draft"],
        json!({"frm1701:rdoPg2I4ATC_II011": "true"}),
    );
    let unowned = fill(
        "1701",
        &compensation_without_owner["draft"],
        json!({ spouse_name: "SPOUSE EMPLOYER" }),
    );
    assert_eq!(
        unowned["rejected"][spouse_name],
        json!("Schedule 1 row 1 or 2 is marked for the taxpayer or the spouse.")
    );

    let taxpayer_key = "frm1701:txtPg2IShed1a_1TPName";
    assert_eq!(
        bir_core::forms::agent::gated_rows::lock_note(
            "1701",
            taxpayer_key,
            "The taxpayer's ATC is compensation income (ATC II011, II013 or II016)."
        ),
        Some("Schedule 1 is for compensation income of the taxpayer.")
    );

    let joint = fill(
        "1701",
        &opened["draft"],
        json!({
            "frm1701:rdoPg1I16CivilStatusM": "true",
            "frm1701:rdoPg1I17SpouseIncomeYes": "true",
            "frm1701:rdoPg1I18FilingStatusJ": "true",
            "frm1701:rdoPg2I4ATC_II011": "true",
            "frm1701:chkPg2IShed1a_1Spouse": "true",
        }),
    );
    let named = fill(
        "1701",
        &joint["draft"],
        json!({ spouse_name: "SPOUSE EMPLOYER" }),
    );
    assert!(
        named["rejected"].get(spouse_name).is_none(),
        "joint spouse compensation opens the name: {:?}",
        named["rejected"]
    );
    assert_eq!(
        named["draft"]["employers"][0]["employer_name"],
        "SPOUSE EMPLOYER"
    );
    let separate = fill(
        "1701",
        &named["draft"],
        json!({"frm1701:rdoPg1I18FilingStatusS": "true"}),
    );
    assert_eq!(separate["draft"]["employers"][0]["employer_name"], "");
    assert_eq!(
        separate["relock_notes"][spouse_name],
        json!(
            "Employer row 1 was cleared: Schedule 1 applies only to compensation income on a joint return."
        )
    );

    let eight = fill(
        "1701",
        &opened["draft"],
        json!({"frm1701:rdoPg1I7ATC_II015": "true"}),
    );
    let described = fill(
        "1701",
        &eight["draft"],
        json!({ item27: "OTHER EIGHT PERCENT" }),
    );
    assert!(
        described["rejected"].get(item27).is_none(),
        "8% opens item 27's description: {:?}",
        described["rejected"]
    );
    assert_eq!(
        described["draft"]["computations"]["schedule_3_descriptions"]["27"],
        "OTHER EIGHT PERCENT"
    );
    let graduated = fill(
        "1701",
        &described["draft"],
        json!({"frm1701:rdoPg1I7ATC_II012": "true"}),
    );
    assert!(graduated["draft"]["computations"]["schedule_3_descriptions"]["27"].is_null());
    assert_eq!(
        graduated["relock_notes"][item27],
        json!("The Schedule 3 item 27 description was cleared: it applies only to the 8% rate.")
    );

    let foreign_yes = fill(
        "1701",
        &opened["draft"],
        json!({"frm1701:rdoPg1I13ForeignTaxCreditsYes": "true"}),
    );
    let numbered = fill("1701", &foreign_yes["draft"], json!({ foreign: "FTN-1" }));
    assert!(
        numbered["rejected"].get(foreign).is_none(),
        "foreign-tax Yes opens item 14: {:?}",
        numbered["rejected"]
    );
    assert_eq!(numbered["draft"]["foreign_tax_number"], "FTN-1");
    let foreign_no = fill(
        "1701",
        &numbered["draft"],
        json!({"frm1701:rdoPg1I13ForeignTaxCreditsNo": "true"}),
    );
    assert_eq!(foreign_no["draft"]["foreign_tax_number"], "");
    assert_eq!(
        foreign_no["relock_notes"][foreign],
        json!("The foreign tax number was cleared: Item 13 is not Yes.")
    );

    let itemized = fill(
        "1701",
        &opened["draft"],
        json!({
            "frm1701:rdoPg1I7ATC_II012": "true",
            "frm1701:rdoPg1I21AMethodDeductionI": "true",
        }),
    );
    let typed = fill(
        "1701",
        &itemized["draft"],
        json!({ sched5: "ITEMIZED COST" }),
    );
    assert!(
        typed["rejected"].get(sched5).is_none(),
        "graduated itemized opens Schedule 5: {:?}",
        typed["rejected"]
    );
    assert_eq!(
        typed["draft"]["computations"]["schedule_5_taxpayer"][0]["description"],
        "ITEMIZED COST"
    );
    let osd = fill(
        "1701",
        &typed["draft"],
        json!({"frm1701:rdoPg1I21AMethodDeductionO": "true"}),
    );
    assert_eq!(
        osd["draft"]["computations"]["schedule_5_taxpayer"][0]["description"],
        ""
    );
    assert_eq!(
        osd["relock_notes"][sched5],
        json!(
            "Schedule 5 row 1 was cleared: it applies only to graduated rates with itemized deductions."
        )
    );
}

#[test]
fn form_1706_property_class_other_detail_is_gated() {
    setup();
    let detail = "frm1706:j_id391:_7";
    let group = "frm1706:j_id391";
    let opened = open("1706", 1);

    let refused = fill("1706", &opened["draft"], json!({ detail: "OTHER CLASS" }));
    assert_eq!(
        refused["rejected"][detail],
        json!("Item 15 classification is Others.")
    );

    let other = fill(
        "1706",
        &opened["draft"],
        json!({ group: "8", detail: "OTHER CLASS" }),
    );
    assert!(
        other["rejected"].get(detail).is_none(),
        "Others opens the detail: {:?}",
        other["rejected"]
    );
    assert_eq!(other["draft"]["property_class_other"], "OTHER CLASS");

    let residential = fill("1706", &other["draft"], json!({ group: "1" }));
    assert_eq!(residential["draft"]["property_class_other"], "");
    assert_eq!(
        residential["relock_notes"][detail],
        json!("The property-class detail was cleared: Item 15 is not Others.")
    );
}

#[test]
fn form_1701a_joint_rate_and_foreign_boxes_are_gated() {
    setup();
    let opened = open("1701A", 0);
    let second = "frm1701A:txt23B";
    let graduated_amount = "frm1701A:txt36A";
    let eight_amount = "frm1701A:txt47A";
    let foreign = "frm1701A:txtForeignTaxNumber";

    for key in [second, graduated_amount, eight_amount, foreign] {
        let refused = fill("1701A", &opened["draft"], json!({ key: "100" }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let joint = fill(
        "1701A",
        &opened["draft"],
        json!({
            "frm1701A:optCivilStatus_2": "true",
            "frm1701A:optSpouseIncome_1": "true",
            "frm1701A:optFilingStatus_1": "true",
        }),
    );
    let typed_second = fill("1701A", &joint["draft"], json!({ second: "100" }));
    assert!(
        typed_second["rejected"].get(second).is_none(),
        "joint filing opens column B: {:?}",
        typed_second["rejected"]
    );
    let separate = fill(
        "1701A",
        &typed_second["draft"],
        json!({"frm1701A:optFilingStatus_2": "true"}),
    );
    assert_eq!(
        separate["draft"]["spouse_column"]["second_installment"],
        0.0
    );
    assert_eq!(
        separate["relock_notes"][second],
        json!("Column B was cleared: it applies only to a joint return.")
    );

    let graduated = fill(
        "1701A",
        &opened["draft"],
        json!({
            "frm1701A:optTaxType_1": "true",
            "frm1701A:optATC_1": "true",
        }),
    );
    let typed_graduated = fill(
        "1701A",
        &graduated["draft"],
        json!({ graduated_amount: "100" }),
    );
    assert!(
        typed_graduated["rejected"].get(graduated_amount).is_none(),
        "graduated opens Part IV-A: {:?}",
        typed_graduated["rejected"]
    );

    let eight = fill(
        "1701A",
        &typed_graduated["draft"],
        json!({"frm1701A:optATC_3": "true"}),
    );
    assert_eq!(eight["draft"]["taxpayer"]["sales"], 0.0);
    assert_eq!(
        eight["relock_notes"][graduated_amount],
        json!("This schedule was cleared: it applies only to graduated rates.")
    );
    let typed_eight = fill("1701A", &eight["draft"], json!({ eight_amount: "100" }));
    assert!(
        typed_eight["rejected"].get(eight_amount).is_none(),
        "8% opens Part IV-B: {:?}",
        typed_eight["rejected"]
    );

    let foreign_yes = fill(
        "1701A",
        &opened["draft"],
        json!({"frm1701A:optForeignTaxCredits_1": "true"}),
    );
    let numbered = fill("1701A", &foreign_yes["draft"], json!({ foreign: "FTN-1" }));
    assert!(
        numbered["rejected"].get(foreign).is_none(),
        "foreign-tax Yes opens item 14: {:?}",
        numbered["rejected"]
    );
    let foreign_no = fill(
        "1701A",
        &numbered["draft"],
        json!({"frm1701A:optForeignTaxCredits_2": "true"}),
    );
    assert_eq!(foreign_no["draft"]["foreign_tax_number"], "");
    assert_eq!(
        foreign_no["relock_notes"][foreign],
        json!("The foreign tax number was cleared: Item 13 is not Yes.")
    );
}

#[test]
fn form_1701a_zeroing_an_open_column_box_is_not_a_shut_column_clearing() {
    setup();
    let amount = "frm1701A:txt36A";
    let spouse_amount = "frm1701A:txt36B";
    let opened = open("1701A", 0);
    let graduated = fill(
        "1701A",
        &opened["draft"],
        json!({
            "frm1701A:optTaxType_1": "true",
            "frm1701A:optATC_1": "true",
        }),
    );
    assert_eq!(
        graduated["relocks"][amount],
        Value::Null,
        "graduated column A is open"
    );
    assert!(
        graduated["relocks"][spouse_amount].is_string(),
        "column B is shut without a joint return"
    );
    let typed = fill("1701A", &graduated["draft"], json!({ amount: "100" }));
    assert_eq!(typed["draft"]["taxpayer"]["sales"], 100.0);
    let zeroed = fill("1701A", &typed["draft"], json!({ amount: "0" }));
    assert_eq!(zeroed["draft"]["taxpayer"]["sales"], 0.0);
    let cleared = "This schedule was cleared: it applies only to graduated rates.";
    assert_ne!(
        zeroed["relock_notes"][amount],
        json!(cleared),
        "zeroing the open box is not the shut column's clearing"
    );
    assert_ne!(
        zeroed["relock_notes"][spouse_amount],
        json!(cleared),
        "nothing in the shut column was typed or cleared"
    );
    save("1701A", &zeroed["draft"]);
    assert_eq!(
        raw_count(
            "SELECT count(*) FROM settings WHERE key LIKE 'notice_pending:1701a_graduated_schedule:%'"
        ),
        0,
        "the assistant/save path records no false pending notice"
    );
}

#[test]
fn form_1701ms_column_a_boxes_are_gated() {
    setup();
    let opened = open("1701MS", 0);
    let gross = "frm1701MS:txtGrossa";
    let sales = "frm1701MS:txtSalesRevenueNo5a";
    let cost = "frm1701MS:txtCostofSalesNo8a";
    let relief = "frm1701MS:txtTaxpayerNo13a";
    let relief_from = "frm1701MS:txtTaxpayerNo16a";
    let relief_to = "frm1701MS:txtTaxpayerNo16a1";
    let nolco = "frm1701MS:txtNetOperatingLossNo10a";
    let eight = "frm1701MS:txtSalesRevenuesFeesANo19a";
    let previous = "frm1701MS:txtTaxPaidPreviouslyFiledNo6a";

    for key in [
        gross,
        sales,
        cost,
        relief,
        relief_from,
        relief_to,
        nolco,
        eight,
        previous,
    ] {
        let refused = fill("1701MS", &opened["draft"], json!({ key: "100" }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let graduated_business = fill(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12b": "true",
        }),
    );
    let typed_sales = fill(
        "1701MS",
        &graduated_business["draft"],
        json!({ sales: "100" }),
    );
    assert!(
        typed_sales["rejected"].get(sales).is_none(),
        "business graduated opens Schedule B1: {:?}",
        typed_sales["rejected"]
    );

    let itemized = fill(
        "1701MS",
        &typed_sales["draft"],
        json!({"frm1701MS:txtTaxpayerNo17a": "true"}),
    );
    let typed_cost = fill(
        "1701MS",
        &itemized["draft"],
        json!({ cost: "100", nolco: "25" }),
    );
    for key in [cost, nolco] {
        assert!(
            typed_cost["rejected"].get(key).is_none(),
            "itemized opens {key}: {:?}",
            typed_cost["rejected"]
        );
    }
    assert_eq!(typed_cost["draft"]["taxpayer"]["nolco"], 25.0);

    let mixed = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtTaxpayerNo11b": "true"}),
    );
    let typed_gross = fill("1701MS", &mixed["draft"], json!({ gross: "100" }));
    assert!(
        typed_gross["rejected"].get(gross).is_none(),
        "mixed income opens Schedule A: {:?}",
        typed_gross["rejected"]
    );

    let exempt = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtTaxpayerNo12d": "true"}),
    );
    let typed_relief = fill(
        "1701MS",
        &exempt["draft"],
        json!({ relief: "100", relief_from: "01/01/2026", relief_to: "12/31/2026" }),
    );
    for key in [relief, relief_from, relief_to] {
        assert!(
            typed_relief["rejected"].get(key).is_none(),
            "exempt opens {key}: {:?}",
            typed_relief["rejected"]
        );
    }

    let eight_rate = fill(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12c": "true",
        }),
    );
    let typed_eight = fill("1701MS", &eight_rate["draft"], json!({ eight: "100" }));
    assert!(
        typed_eight["rejected"].get(eight).is_none(),
        "8% opens Schedule B2: {:?}",
        typed_eight["rejected"]
    );

    let amended = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtAmendedYesNo2": "true"}),
    );
    let typed_previous = fill("1701MS", &amended["draft"], json!({ previous: "100" }));
    assert!(
        typed_previous["rejected"].get(previous).is_none(),
        "amended opens the prior payment: {:?}",
        typed_previous["rejected"]
    );
}

#[test]
fn form_1701ms_special_rate_business_keeps_cost_of_sales() {
    setup();
    let cost = "frm1701MS:txtCostofSalesNo8a";
    let sales = "frm1701MS:txtSalesRevenueNo5a";
    let opened = open("1701MS", 0);
    let joint = fill(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:civilStatus": "Married",
            "frm1701MS:txtIfMarriedJointlyNo5": "true",
        }),
    );
    let special = fill(
        "1701MS",
        &joint["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12e": "true",
            "frm1701MS:txtTaxpayerNo11a1": "true",
            "frm1701MS:txtTaxpayerNo12d1": "true",
        }),
    );
    assert_eq!(special["draft"]["taxpayer"]["tax_option"], "special");
    assert_eq!(special["draft"]["spouse"]["tax_option"], "exempt");
    // BIR-Form1701MS.hta incomeIsSubjectToTP L10287-10288 (spouse L10539)
    // ticks Itemized before disabling Item 17, so the payload and print must
    // keep that tick even though the filer cannot change the locked radio.
    for (column, yes, no) in [
        (
            "taxpayer",
            "frm1701MS:txtTaxpayerNo17a",
            "frm1701MS:txtTaxpayerNo17b",
        ),
        (
            "spouse",
            "frm1701MS:txtSpouseNo17a",
            "frm1701MS:txtSpouseNo17b",
        ),
    ] {
        assert_eq!(
            special["draft"][column]["deduction"], "itemized",
            "{column}"
        );
        assert_eq!(
            payload_map("1701MS", &special["draft"])[yes],
            "true",
            "{yes}"
        );
        assert_eq!(
            payload_map("1701MS", &special["draft"])[no],
            "false",
            "{no}"
        );
        assert_eq!(
            printed_map("1701MS", &special["draft"])[yes],
            "true",
            "print {yes}"
        );
        assert_eq!(
            printed_map("1701MS", &special["draft"])[no],
            "false",
            "print {no}"
        );
    }
    let with_sales = fill("1701MS", &special["draft"], json!({ sales: "1000" }));
    let typed = fill("1701MS", &with_sales["draft"], json!({ cost: "400" }));
    assert!(
        typed["rejected"].get(cost).is_none(),
        "special-rate business income opens cost of sales: {:?}",
        typed["rejected"]
    );
    assert_eq!(typed["draft"]["taxpayer"]["cost_of_sales"], 400.0);
    assert_eq!(typed["draft"]["taxpayer"]["gross_income"], 600.0);
}

#[test]
fn form_1702rt_mcit_and_prior_payment_boxes_are_gated() {
    setup();
    let opened = open("1702RT", 0);
    let item45 = "frm1702RT:txtPg2Pt4I45IncomeTaxPaymentUnderMCIT";
    let item51 = "frm1702RT:txtPg2Pt4I51TaxPaidInReturn";
    let row_year = "frm1702RT:txtPg4Sc4I1C1";

    // The demo corporation is old enough that MCIT is its opening ATC;
    // a recent incorporation date closes the MCIT fields first.
    let regular_start = fill(
        "1702RT",
        &opened["draft"],
        json!({"frm1702RT:txtPg1Pt1I10": "01/02/2024"}),
    );
    for (key, value) in [
        (item45, json!("100")),
        (item51, json!("100")),
        (row_year, json!("2025")),
    ] {
        let refused = fill("1702RT", &regular_start["draft"], json!({ key: value }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let mcit = fill(
        "1702RT",
        &regular_start["draft"],
        json!({"frm1702RT:txtPg1Pt1I10": "01/02/2020"}),
    );
    let typed45 = fill("1702RT", &mcit["draft"], json!({ item45: "100" }));
    assert!(
        typed45["rejected"].get(item45).is_none(),
        "MCIT opens item 45: {:?}",
        typed45["rejected"]
    );
    let typed_row = fill("1702RT", &typed45["draft"], json!({ row_year: "2025" }));
    assert!(
        typed_row["rejected"].get(row_year).is_none(),
        "MCIT opens Schedule 4: {:?}",
        typed_row["rejected"]
    );
    let regular = fill(
        "1702RT",
        &typed_row["draft"],
        json!({"frm1702RT:txtPg1Pt1I10": "01/02/2024"}),
    );
    assert_eq!(regular["draft"]["schedule_4"]["rows"][0]["year"], "");
    assert_eq!(
        regular["relock_notes"][row_year],
        json!("Schedule 4 row 1 was cleared: MCIT does not apply.")
    );

    let amended = fill(
        "1702RT",
        &opened["draft"],
        json!({"frm1702RT:rdoPg1I3AmmendYes": "true"}),
    );
    let typed51 = fill("1702RT", &amended["draft"], json!({ item51: "100" }));
    assert!(
        typed51["rejected"].get(item51).is_none(),
        "amended opens item 51: {:?}",
        typed51["rejected"]
    );
    let unamended = fill(
        "1702RT",
        &typed51["draft"],
        json!({"frm1702RT:rdoPg1I3AmmendNo": "true"}),
    );
    assert_eq!(
        unamended["draft"]["part_iv"]["tax_credits"]["item_51_tax_paid_on_previous_return"],
        0
    );
    assert_eq!(
        unamended["relock_notes"][item51],
        json!("Item 51 was cleared: this is not an amended return.")
    );
}

#[test]
fn form_1702mx_item27_and_schedule9_are_gated() {
    setup();
    let opened = open("1702MX", 0);
    let item27 = "frm1702MX:txtPg2Sc3It27A";
    let row_year = "frm1702MX:txtPg7Sc9I1";

    for (key, value) in [(item27, json!("100")), (row_year, json!("2025"))] {
        let refused = fill("1702MX", &opened["draft"], json!({ key: value }));
        assert!(
            refused["rejected"].get(key).is_some(),
            "{key} closed: {:?}",
            refused["rejected"]
        );
    }

    let amended = fill(
        "1702MX",
        &opened["draft"],
        json!({"frm1702MX:rdoPg1I3AmendedYes": "true"}),
    );
    let typed27 = fill("1702MX", &amended["draft"], json!({ item27: "100" }));
    assert!(
        typed27["rejected"].get(item27).is_none(),
        "amended opens Schedule 3 item 27: {:?}",
        typed27["rejected"]
    );
    let unamended = fill(
        "1702MX",
        &typed27["draft"],
        json!({"frm1702MX:rdoPg1I3AmendedNo": "true"}),
    );
    assert_eq!(
        unamended["draft"]["schedule_3"]["items_20_to_33"][7]["exempt"]["amount"],
        0
    );
    assert_eq!(
        unamended["relock_notes"][item27],
        json!("Schedule 3 item 27 was cleared: this is not an amended return.")
    );

    // Item 5's MCIT box is derived: the four-year incorporation rule is
    // what opens Schedule 9.
    let mcit = fill(
        "1702MX",
        &opened["draft"],
        json!({"frm1702MX:txtPg1Pt1I8": "01/01/2020"}),
    );
    let typed_row = fill("1702MX", &mcit["draft"], json!({ row_year: "2025" }));
    assert!(
        typed_row["rejected"].get(row_year).is_none(),
        "MCIT opens Schedule 9: {:?}",
        typed_row["rejected"]
    );
    let regular = fill(
        "1702MX",
        &typed_row["draft"],
        json!({"frm1702MX:txtPg1Pt1I8": "01/01/2024"}),
    );
    assert_eq!(regular["draft"]["schedule_9"]["rows"][0]["year"], "");
    assert_eq!(
        regular["relock_notes"][row_year],
        json!("Schedule 9 row 1 was cleared: MCIT does not apply.")
    );
}

#[test]
fn blanking_an_earlier_1702mx_mcit_row_keeps_the_notice_on_the_right_stable_line() {
    setup();
    let opened = open("1702MX", 0);
    let row1 = "frm1702MX:txtPg7Sc9I1";
    let row2 = "frm1702MX:txtPg7Sc9I2";
    let date = "frm1702MX:txtPg1Pt1I8";

    let mcit = fill("1702MX", &opened["draft"], json!({ date: "01/01/2020" }));
    let typed1 = fill("1702MX", &mcit["draft"], json!({ row1: "2024" }));
    let typed2 = fill("1702MX", &typed1["draft"], json!({ row2: "2025" }));
    assert_eq!(typed2["draft"]["schedule_9"]["rows"][0]["year"], "2024");
    assert_eq!(typed2["draft"]["schedule_9"]["rows"][1]["year"], "2025");

    let deleted = fill("1702MX", &typed2["draft"], json!({ row1: "" }));
    assert_eq!(deleted["draft"]["schedule_9"]["rows"][0]["year"], "");
    assert_eq!(
        deleted["draft"]["schedule_9"]["rows"][1]["year"], "2025",
        "these printed lines are fixed slots; blanking line 1 does not move line 2"
    );

    let closed = fill("1702MX", &deleted["draft"], json!({ date: "01/01/2024" }));
    assert_eq!(closed["draft"]["schedule_9"]["rows"][1]["year"], "");
    assert_eq!(
        closed["relock_notes"][row2],
        json!("Schedule 9 row 2 was cleared: MCIT does not apply.")
    );
    assert_eq!(
        closed["relock_notes"][row1],
        json!("Schedule 9 applies only when MCIT applies."),
        "the blanked first line must not take the second line's notice"
    );
    let clearances = bir_core::forms::agent::gated_rows::clearances(
        AgentFormKind::from_code("1702MX").unwrap(),
        &deleted["draft"],
        &closed["draft"],
    );
    assert_eq!(
        clearances
            .iter()
            .map(|clearance| clearance.gate.as_str())
            .collect::<Vec<_>>(),
        vec!["1702mx_schedule9_mcit:mcit-line-2"],
        "{clearances:?}"
    );

    // The same row key carries the open migration's pending note: once the
    // page records it seen, reopening does not offer it again. Rewrite the
    // stored bytes so this is the legacy-row case, not a fill the page has
    // already reported inline.
    let saved = save("1702MX", &typed2["draft"]);
    let id = saved["id"].as_i64().expect("1702MX draft id");
    let mut legacy = saved["draft"].clone();
    legacy["incorporation_date"] = json!("01/01/2024");
    let bytes = serde_json::to_string(&legacy).expect("legacy 1702MX json");
    raw_exec(
        "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
        &[&bytes, &id.to_string()],
    );
    let migrated = open("1702MX", 0);
    assert_eq!(migrated["draft"]["schedule_9"]["rows"][1]["year"], "");
    assert!(
        note_texts(&migrated)
            .iter()
            .any(|text| text.contains("Schedule 9 row 2 was cleared")),
        "migration notes: {:?}",
        migrated["notes"]
    );
    let recorded = notes_seen(&note_keys(&migrated));
    assert!(
        recorded
            .iter()
            .any(|key| key.contains(":1702mx_schedule9_mcit:mcit-line-2:")),
        "row-keyed seen keys: {recorded:?}"
    );
    let reopened = open("1702MX", 0);
    assert!(
        !note_texts(&reopened)
            .iter()
            .any(|text| text.contains("Schedule 9 row 2 was cleared")),
        "seen row-keyed note repeated: {:?}",
        reopened["notes"]
    );
}
