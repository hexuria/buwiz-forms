//! B16 (form-fields PR 1): a write to a box another answer keeps shut is
//! refused at the host with the box's own tag — the same tag the page
//! shows for it — while a blank write is a clear and is never refused.
//! The two traced boxes: 0619E item 14 waits on item 4 "Any Taxes
//! Withheld?" Yes (the port's recompute zeroes it otherwise,
//! form_0619e_official.rs `official_recompute`), and 2551Q item 12's
//! specify select waits on item 12 Yes (the field map writes it only
//! while `tax_relief`, form_2551q_xml.rs).

use super::*;

/// 0619E item 14 (amount of remittance).
#[test]
fn form_0619e_item14_takes_a_write_only_after_item4_yes() {
    setup();
    let remit = "frm0619E:txtTax14";
    let yes = "frm0619E:optWithheld:Y";
    let opened = open("0619E", 3);
    // Item 4 unanswered: the box is shut and the write is refused with
    // the condition's own tag, the draft untouched.
    let refused = fill("0619E", &opened["draft"], json!({ remit: "100.00" }));
    assert_eq!(
        refused["rejected"][remit],
        json!("Needs item 4 = Yes"),
        "0619E item 14 while item 4 is unanswered"
    );
    assert_eq!(
        refused["draft"]["item_14_amount_of_remittance"],
        opened["draft"]["item_14_amount_of_remittance"],
        "a refused write leaves the draft as it was"
    );
    // A blank write is a clear, never refused by the gate: "" is not an
    // amount, so the money box's own parse refusal answers instead of the
    // condition's tag.
    let cleared = fill("0619E", &opened["draft"], json!({ remit: "" }));
    assert_ne!(
        cleared["rejected"][remit],
        json!("Needs item 4 = Yes"),
        "0619E item 14 blank write meets the amount parse, not the gate"
    );
    // Item 4 Yes opens the box: the same write lands, staged in one batch
    // and sent after the answer.
    let batch = fill(
        "0619E",
        &opened["draft"],
        json!({ yes: "true", remit: "100.00" }),
    );
    assert!(
        batch["rejected"].get(remit).is_none(),
        "0619E item 14 with item 4 Yes in the same batch: {:?}",
        batch["rejected"]
    );
    let after_yes = fill("0619E", &opened["draft"], json!({ yes: "true" }));
    assert!(after_yes["rejected"].get(yes).is_none());
    let taken = fill("0619E", &after_yes["draft"], json!({ remit: "100.00" }));
    assert!(
        taken["rejected"].get(remit).is_none(),
        "0619E item 14 after item 4 Yes: {:?}",
        taken["rejected"]
    );
    assert_eq!(amount(&taken["values"], remit), 100.0);
}

/// 2551Q item 12's specify select.
#[test]
fn form_2551q_item12_specify_takes_a_write_only_after_yes() {
    setup_taxpayer_filing(&["2551Q"]);
    let specify = "frm2551Qv2018:txtTaxReliefSpecify";
    let yes = "frm2551Qv2018:taxTreaty_1";
    let opened = open("2551Q", 1);
    // Item 12 unanswered: the select is shut and the write is refused
    // with the condition's own tag.
    let refused = fill("2551Q", &opened["draft"], json!({ specify: "1" }));
    assert_eq!(
        refused["rejected"][specify],
        json!("Needs item 12 = Yes"),
        "2551Q item 12 specify while item 12 is unanswered"
    );
    assert_eq!(
        refused["draft"]["tax_relief_specification"], opened["draft"]["tax_relief_specification"],
        "a refused write leaves the draft as it was"
    );
    // A blank write is a clear, never refused.
    let cleared = fill("2551Q", &opened["draft"], json!({ specify: "" }));
    assert!(
        cleared["rejected"].get(specify).is_none(),
        "2551Q item 12 specify blank write: {:?}",
        cleared["rejected"]
    );
    // Item 12 Yes opens the select.
    let batch = fill(
        "2551Q",
        &opened["draft"],
        json!({ yes: "true", specify: "1" }),
    );
    assert!(
        batch["rejected"].get(specify).is_none(),
        "2551Q item 12 specify with item 12 Yes in the same batch: {:?}",
        batch["rejected"]
    );
    let after_yes = fill("2551Q", &opened["draft"], json!({ yes: "true" }));
    assert!(after_yes["rejected"].get(yes).is_none());
    let taken = fill("2551Q", &after_yes["draft"], json!({ specify: "1" }));
    assert!(
        taken["rejected"].get(specify).is_none(),
        "2551Q item 12 specify after item 12 Yes: {:?}",
        taken["rejected"]
    );
    assert_eq!(taken["values"][specify], json!("1"));
}
