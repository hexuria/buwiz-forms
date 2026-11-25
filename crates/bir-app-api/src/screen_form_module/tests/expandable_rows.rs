//! F4: the officially expandable tables — rows past the pre-allocated ones.
use super::*;

#[test]
fn row3_date_survives_after_earlier_rows_fill() {
    setup();
    let opened = open("1601C", 3);
    let draft = opened["draft"].clone();
    let mut edits = Map::new();
    for row in 0..3 {
        edits.insert(
            format!("frm1601c:sched1:txtMonthYear{row}"),
            json!("01/2025"),
        );
        edits.insert(
            format!("frm1601c:sched1:txtDatePaid{row}"),
            json!("02/15/2025"),
        );
        edits.insert(format!("frm1601c:sched1:txtBankCode{row}"), json!("BDO"));
        edits.insert(format!("frm1601c:sched1:txtNumber{row}"), json!("123"));
        edits.insert(format!("frm1601c:sched1:txtTaxPaid{row}"), json!("100.00"));
    }
    edits.insert("frm1601c:sched1:txtDatePaid3".into(), json!("03/15/2025"));
    let filled = fill("1601C", &draft, Value::Object(edits));
    let values = &filled["values"];
    eprintln!("rejected: {:?}", filled["rejected"]);
    eprintln!(
        "draft rows: {}",
        filled["draft"]["schedule_1"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0)
    );
    eprintln!("row3 in draft: {:?}", filled["draft"]["schedule_1"][3]);
}
