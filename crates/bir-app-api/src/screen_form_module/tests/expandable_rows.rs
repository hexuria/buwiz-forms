//! F4: the officially expandable tables file their added rows under the
//! numbered keys the official program builds. No official sample carries
//! more rows than the pre-allocated ones (the fixtures hold none), so the
//! proof is the official key pattern itself, quoted from the evidence:
//! 1601C's `fields.json` row templates `frm1601c:sched1:txtTaxPaid{N>=0}`
//! (and family), form 2000's `btnSched1Add`..`btnSched4Add` script controls
//! with the schedules' `txt…{index}` rows, and 2200A's
//! `txtSched1_xa_others{N>=4}_*` templates, one-based as the official page
//! numbers them.
use super::*;

fn upload_keys(code: &str, draft: &Value) -> Vec<String> {
    let kind = kind_of(code).unwrap();
    agent::field_map(kind, draft)
        .unwrap_or_else(|e| panic!("{code} field map: {e}"))
        .keys()
        .cloned()
        .collect()
}

#[test]
fn schedule_i_files_its_fourth_row_under_the_official_numbered_keys() {
    setup();
    let opened = open("1601C", 3);
    let mut map = Map::new();
    for row in 0..4 {
        map.insert(
            format!("frm1601c:sched1:txtMonthYear{row}"),
            json!("01/2025"),
        );
        map.insert(
            format!("frm1601c:sched1:txtDatePaid{row}"),
            json!("02/15/2025"),
        );
        map.insert(format!("frm1601c:sched1:txtBankCode{row}"), json!("BDO"));
        map.insert(format!("frm1601c:sched1:txtNumber{row}"), json!("123"));
        map.insert(format!("frm1601c:sched1:txtTaxPaid{row}"), json!("100.00"));
    }
    let filled = fill("1601C", &opened["draft"], Value::Object(map));
    assert_eq!(
        filled["rejected"],
        json!({}),
        "nothing refuses: {:?}",
        filled["rejected"]
    );
    let draft = filled["draft"].clone();
    let keys = upload_keys("1601C", &draft);
    // The official pattern `frm1601c:sched1:txt<Stem>{N>=0}`, row index 3:
    for key in [
        "frm1601c:sched1:txtMonthYear3",
        "frm1601c:sched1:txtDatePaid3",
        "frm1601c:sched1:txtBankCode3",
        "frm1601c:sched1:txtNumber3",
        "frm1601c:sched1:txtTaxPaid3",
        "frm1601c:sched1:txtShouldTaxDue3",
        "frm1601c:sched1:txtAdjustments3",
    ] {
        assert!(keys.iter().any(|k| k == key), "1601C row 3 files {key}");
    }
}

#[test]
fn form_2000_files_a_fourth_schedule_row_under_the_official_numbered_keys() {
    setup();
    let opened = open("2000", 3);
    let mut map = Map::new();
    // rows 0-2 then row 3 of Schedule 1: ATC pick then amounts
    for row in 0..4 {
        map.insert(format!("drpATCCode{row}"), json!("DS101"));
        map.insert(format!("frm2000:sched1:txtTaxBase{row}"), json!("1000.00"));
    }
    let draft = fill("2000", &opened["draft"], Value::Object(map))["draft"].clone();
    let keys = upload_keys("2000", &draft);
    for key in [
        "drpATCCode3",
        "frm2000:sched1:txtTaxBase3",
        "frm2000:sched1:txtTaxDue3",
    ] {
        assert!(keys.iter().any(|k| k == key), "2000 row 3 files {key}");
    }
}

#[test]
fn form_2200a_files_a_fourth_others_row_one_based_as_the_official_keys_are() {
    setup();
    let opened = open("2200A", 1);
    let mut map = Map::new();
    for row in 1..=4 {
        map.insert(
            format!("frm2200Av2020:txtSched1_xa_others{row}_Atc"),
            json!("XA035"),
        );
        map.insert(
            format!("frm2200Av2020:txtSched1_xa_others{row}_Taxable"),
            json!("1000.00"),
        );
    }
    let filled = fill("2200A", &opened["draft"], Value::Object(map));
    eprintln!("rejected: {:?}", filled["rejected"]);
    let draft = filled["draft"].clone();
    eprintln!(
        "others rows in draft: {}",
        draft["others"].as_array().map(|a| a.len()).unwrap_or(0)
    );
    let keys = upload_keys("2200A", &draft);
    eprintln!(
        "row1-ish upload keys: {:?}",
        keys.iter()
            .filter(|k| k.contains("others"))
            .take(12)
            .collect::<Vec<_>>()
    );
    for key in [
        "frm2200Av2020:txtSched1_xa_others4_Atc",
        "frm2200Av2020:txtSched1_xa_others4_Taxable",
        "frm2200Av2020:txtSched1_xa_others4_Due",
    ] {
        assert!(
            keys.iter().any(|k| k == key),
            "2200A others row 4 files {key}"
        );
    }
}
