//! 1602Q Item 20 is an enabled amount box in the official form, not an amended-only answer.

use super::*;

#[test]
fn item_twenty_is_kept_through_fill_save_and_reopen() {
    setup();
    for (period, amended_no) in [(1, false), (2, true)] {
        let opened = open("1602Q", period);
        let mut edits = json!({ "frm1602Q:txt20": "500" });
        if amended_no {
            edits["frm1602Q:AmendedRtn2"] = json!("true");
        }
        let filled = fill("1602Q", &opened["draft"], edits);
        assert_eq!(
            filled["draft"]["tax_remitted_previous"], 500.0,
            "period {period}"
        );
        assert_eq!(
            filled["values"]["frm1602Q:txt20"], "500.00",
            "period {period}"
        );
        assert_eq!(
            filled["values"]["frm1602Q:txt22"], "500.00",
            "period {period}"
        );
        assert_eq!(
            filled["values"]["frm1602Q:txt23"], "-500.00",
            "period {period}"
        );

        let saved = save("1602Q", &filled["draft"]);
        let reopened = open("1602Q", period);
        assert_eq!(
            reopened["draft"]["id"], saved["draft"]["id"],
            "period {period}"
        );
        assert_eq!(
            reopened["draft"]["tax_remitted_previous"], 500.0,
            "period {period}"
        );
        assert_eq!(
            reopened["values"]["frm1602Q:txt20"], "500.00",
            "period {period}"
        );
        assert_eq!(
            reopened["values"]["frm1602Q:txt22"], "500.00",
            "period {period}"
        );
        assert_eq!(
            reopened["values"]["frm1602Q:txt23"], "-500.00",
            "period {period}"
        );
    }
}

#[test]
fn item_twenty_survives_amended_yes_then_no_on_the_page() {
    setup();
    let opened = open("1602Q", 3);
    let yes = fill(
        "1602Q",
        &opened["draft"],
        json!({"frm1602Q:AmendedRtn1": "true", "frm1602Q:txt20": "500"}),
    );
    let no = fill(
        "1602Q",
        &yes["draft"],
        json!({"frm1602Q:AmendedRtn2": "true", "frm1602Q:AmendedRtn1": "false"}),
    );
    assert_eq!(no["draft"]["tax_remitted_previous"], 500.0);
    assert_eq!(no["values"]["frm1602Q:txt20"], "500.00");
}
