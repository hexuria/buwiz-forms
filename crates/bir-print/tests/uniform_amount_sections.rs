//! Uniform amount sections (issue #477): when one amount in a section's
//! column exceeds its comb boxes, every amount of that column prints as a
//! plain right-aligned line; a column without overflow keeps its combs, and
//! spouse columns are independent.

use bir_core::forms::form_1701::{Form1701AmountSection, Form1701Draft, Form1701Party};
use bir_core::forms::queueable::QueueableForm;
use bir_core::TaxpayerProfile;
use bir_print::frozen_html::{filled_document, printed_cell};
use serde_json::json;

fn demo_profile() -> TaxpayerProfile {
    serde_json::from_value(json!({
        "id": null,
        "tin": { "segment1": "000", "segment2": "123", "segment3": "453", "branch": "00000" },
        "rdo_code": "044",
        "full_name": "SANTOS, JUANA",
        "registered_address": "123 RIZAL AVE, MAKATI CITY",
        "zip_code": "1200",
        "phone": "09171234567",
        "email": "juana.santos@example.test",
        "line_of_business": "SERVICES",
        "default_form_type": "2551Q"
    }))
    .unwrap()
}

/// The cells of 1701 Part VII column A (items 1..=10) and column B, in item
/// order, from html-frozen/1701-2018/writer-cells.json peso_joins.
const PART7_A: [&str; 10] = [
    "p4c66", "p4c69", "p4c72", "p4c75", "p4c78", "p4c81", "p4c84", "p4c87", "p4c90", "p4c93",
];
const PART7_B: [&str; 10] = [
    "p4c67", "p4c70", "p4c73", "p4c76", "p4c79", "p4c82", "p4c85", "p4c88", "p4c91", "p4c94",
];

fn line_not_comb(doc: &str, cell: &str) -> bool {
    doc.contains(&format!("data-overflow-of=\"{cell}\""))
}

fn part7_draft(overflow_party: Form1701Party) -> Form1701Draft {
    let profile = demo_profile();
    let mut d = Form1701Draft::new_from_profile(&profile, 2025, 12);
    for item in 1u8..=10 {
        let amount = if item == 2 {
            1_234_567_890.0
        } else {
            1_000.0 + item as f64
        };
        d.set_amount(
            Form1701AmountSection::PartVii,
            item,
            overflow_party,
            Some(amount),
        );
    }
    d
}

#[test]
fn one_overflowing_amount_converts_every_amount_of_the_section_column() {
    let d = part7_draft(Form1701Party::Taxpayer);
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    for cell in PART7_A {
        assert!(
            line_not_comb(&doc, cell),
            "Part VII column A cell {cell} must print as a line, not a comb"
        );
    }
    // The lines are right-aligned and hold the amounts: item 1 = 1,001 and
    // item 2 = 1,234,567,890.
    assert_eq!(printed_cell(&doc, "p4c66").0, "1,001");
    assert_eq!(printed_cell(&doc, "p4c69").0, "1,234,567,890");
}

#[test]
fn a_section_without_overflow_keeps_its_combs() {
    let profile = demo_profile();
    let mut d = Form1701Draft::new_from_profile(&profile, 2025, 12);
    for item in 1u8..=10 {
        d.set_amount(
            Form1701AmountSection::PartVii,
            item,
            Form1701Party::Taxpayer,
            Some(1_000.0 + item as f64),
        );
    }
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    for cell in PART7_A {
        assert!(
            !line_not_comb(&doc, cell),
            "no overflow anywhere: {cell} must keep its comb boxes"
        );
    }
}

#[test]
fn spouse_columns_are_independent() {
    // Overflow only in column B: column A keeps every comb.
    let profile = demo_profile();
    let mut d = Form1701Draft::new_from_profile(&profile, 2025, 12);
    for item in 1u8..=10 {
        d.set_amount(
            Form1701AmountSection::PartVii,
            item,
            Form1701Party::Taxpayer,
            Some(1_000.0 + item as f64),
        );
    }
    for item in 1u8..=10 {
        let amount = if item == 5 {
            9_876_543_210.0
        } else {
            2_000.0 + item as f64
        };
        d.set_amount(
            Form1701AmountSection::PartVii,
            item,
            Form1701Party::Spouse,
            Some(amount),
        );
    }
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    for cell in PART7_A {
        assert!(
            !line_not_comb(&doc, cell),
            "column A has no overflow: {cell} must keep its comb"
        );
    }
    for cell in PART7_B {
        assert!(
            line_not_comb(&doc, cell),
            "column B overflowed: {cell} must print as a line"
        );
    }
}

#[test]
fn centavos_in_a_whole_peso_comb_also_convert_the_column() {
    // Part VII combs are whole-peso: a centavo-bearing amount converts just
    // like a too-long one, and takes the column with it.
    let profile = demo_profile();
    let mut d = Form1701Draft::new_from_profile(&profile, 2025, 12);
    for item in 1u8..=10 {
        let amount = if item == 3 {
            500.50
        } else {
            1_000.0 + item as f64
        };
        d.set_amount(
            Form1701AmountSection::PartVii,
            item,
            Form1701Party::Taxpayer,
            Some(amount),
        );
    }
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    for cell in PART7_A {
        assert!(
            line_not_comb(&doc, cell),
            "centavos in a whole-peso comb: {cell} must print as a line"
        );
    }
}

#[test]
fn sections_of_one_column_are_independent() {
    // Overflow in Part VII must not convert Part VI (the section above it on
    // the same page, same column x): its rows keep their combs.
    let d = part7_draft(Form1701Party::Taxpayer);
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    // Part VI column A rows sit directly above Part VII in the same column
    // (p4c50..p4c63 area); none may convert.
    for cell in ["p4c50", "p4c53", "p4c56", "p4c59", "p4c62"] {
        assert!(
            !line_not_comb(&doc, cell),
            "Part VI cell {cell} is another section: it must keep its comb"
        );
    }
}

/// 1702MX Part II items 14-21: one column of eight whole-peso combs on
/// page 1 — a different section shape from 1701's two spouse columns.
const MX_PART2: [(&str, &str); 8] = [
    ("frm1702MX:txtPg1Pt2I14TotalIncome", "p1c59"),
    ("frm1702MX:txtPg1Pt2I15LessTotalTax", "p1c61"),
    ("frm1702MX:txtPg1Pt2I16NetTaxPayable", "p1c63"),
    ("frm1702MX:txtPg1Pt2I17", "p1c66"),
    ("frm1702MX:txtPg1Pt2I18", "p1c68"),
    ("frm1702MX:txtPg1Pt2I19", "p1c70"),
    ("frm1702MX:txtPg1Pt2I20TotalPenalties", "p1c72"),
    ("frm1702MX:txtPg1Pt2I21TotalAmount", "p1c74"),
];

fn mx_fields(overflow: bool) -> std::collections::BTreeMap<String, String> {
    let mut map = std::collections::BTreeMap::new();
    for (key, _) in MX_PART2 {
        let amount = if overflow && key.contains("I15Less") {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(key.to_string(), amount.to_string());
    }
    map
}

#[test]
fn one_other_form_1702mx_part_ii_uniform_column() {
    let doc = filled_document("1702mx-2018c", &mx_fields(true)).expect("1702mx render");
    for (_, cell) in MX_PART2 {
        assert!(
            line_not_comb(&doc, cell),
            "1702MX Part II: {cell} must print as a line"
        );
    }
}

#[test]
fn one_other_form_1702mx_without_overflow_keeps_combs() {
    let doc = filled_document("1702mx-2018c", &mx_fields(false)).expect("1702mx render");
    for (_, cell) in MX_PART2 {
        assert!(
            !line_not_comb(&doc, cell),
            "1702MX Part II without overflow: {cell} must keep its comb"
        );
    }
}
// ---- Section-band grouping tests (#499 review) ----------------------------

fn lines(cells: &[&str], keys: &[&str], overflow_key: &str, slug: &str) -> String {
    let mut map = std::collections::BTreeMap::new();
    for key in keys {
        let amount = if *key == overflow_key {
            "12345678901234"
        } else {
            "1000"
        };
        map.insert(key.to_string(), amount.to_string());
    }
    let _ = cells;
    bir_print::frozen_html::filled_document(slug, &map).expect("render")
}

/// 1702Q Schedule 4: items 1-7 WITH 6a, 6b and the Total are one group — the
/// pitch gap before 6a must not split them (the 1.711 ratio the reviewer
/// caught). Item 2 overflowing converts every one.
#[test]
fn s1702q_schedule4_items_1_to_7_are_one_group() {
    let keys = [
        "derived:Sched4:txtTax1",
        "derived:Sched4:txtTax2",
        "derived:Sched4:txtTax3",
        "derived:Sched4:txtTax4",
        "derived:Sched4:txtTax5",
        "derived:Sched4:txtTax6",
        "derived:Sched4:txtOthrTxCrdtAmnt0",
        "derived:Sched4:txtOthrTxCrdtAmnt1",
        "derived:Sched4:txtTax7",
    ];
    let cells = [
        "p2c97", "p2c99", "p2c101", "p2c103", "p2c105", "p2c107", "p2c112", "p2c116", "p2c118",
    ];
    let doc = lines(&cells, &keys, "derived:Sched4:txtTax2", "1702q-2018");
    for cell in cells {
        assert!(
            line_not_comb(&doc, cell),
            "1702Q Schedule 4: {cell} must print as a line with item 2"
        );
    }
}

/// 1702EX Schedule 3: item 1 groups with items 2-10 (the 1.72 split the
/// reviewer caught).
#[test]
fn s1702ex_schedule3_item1_groups_with_items_2_to_10() {
    let keys = [
        "derived:amount:txtPg3Pt6S3I1NetIncomePerBook",
        "derived:amount:txtPg3Pt6S3I2NonDeductExpenseOtherIncomeAmt",
        "derived:amount:txtPg3Pt6S3I3NonDeductExpenseOtherIncomeAmt",
        "derived:amount:txtPg3Pt6S3I4Total",
        "derived:amount:txtPg3Pt6S3I5NonTaxIncomeAndIncomeSubjectToFinTaxAmt",
        "derived:amount:txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxAmt",
        "derived:amount:txtPg3Pt6S3I7SpecialDeductAmt",
        "derived:amount:txtPg3Pt6S3I8SpecialDeductAmt",
        "derived:amount:txtPg3Pt6S3I9Total",
        "derived:amount:txtPg3Pt6S3I10NetTaxableIncome",
    ];
    let cells = [
        "p3c100", "p3c105", "p3c109", "p3c111", "p3c116", "p3c120", "p3c125", "p3c129", "p3c131",
        "p3c133",
    ];
    let doc = lines(
        &cells,
        &keys,
        "derived:amount:txtPg3Pt6S3I3NonDeductExpenseOtherIncomeAmt",
        "1702ex-2018",
    );
    for cell in cells {
        assert!(
            line_not_comb(&doc, cell),
            "1702EX Schedule 3: {cell} must print as a line"
        );
    }
}

/// 1701A page 2: items 36-46 of one column are one group.
#[test]
fn s1701a_items_36_to_46_are_one_group() {
    let mut map = std::collections::BTreeMap::new();
    let cells = [
        "p2c13", "p2c16", "p2c19", "p2c22", "p2c25", "p2c31", "p2c36", "p2c39", "p2c42", "p2c45",
        "p2c48",
    ];
    for item in 36u8..=46 {
        let amount = if item == 38 { "12345678901234" } else { "1000" };
        map.insert(format!("derived:amount_{item}A"), amount.to_string());
    }
    let doc = bir_print::frozen_html::filled_document("1701a-2018", &map).expect("render");
    for cell in cells {
        assert!(
            line_not_comb(&doc, cell),
            "1701A items 36-46: {cell} must print as a line"
        );
    }
}

/// 1701 page 2 Part IV Schedule 3 (items 8+): one continuous column — the
/// pitch-ratio splits the reviewer caught at items 12|13, 16|17, 18|19 must
/// not separate rows.
#[test]
fn s1701_part_iv_schedule3_is_one_group() {
    let mut map = std::collections::BTreeMap::new();
    // Schedule 3 items 8..=18 column A from writer-cells.json.
    let rows: Vec<(String, u8)> = (8u8..=18)
        .map(|item| (format!("frm1701:txtPg2IShed3_{item}A"), item))
        .collect();
    for (key, item) in &rows {
        let amount = if *item == 10 {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(key.clone(), amount.to_string());
    }
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    // The cells are the peso-join targets; read them from the printed lines.
    let mut converted = 0;
    for m in doc.match_indices("data-overflow-of=\"p2c") {
        let at = m.0 + 18;
        let id = &doc[at..at + 5];
        if id.starts_with("p2c") {
            converted += 1;
        }
    }
    assert!(
        converted >= 10,
        "1701 Part IV Schedule 3: the whole column must convert with item 10, saw {converted} lines"
    );
}

/// A negative amount in a converted column prints its sign in the line.
#[test]
fn negative_amount_in_converted_column_keeps_its_sign() {
    let mut map = std::collections::BTreeMap::new();
    map.insert(
        "derived:Sched4:txtTax1".to_string(),
        "12345678901234.00".to_string(),
    );
    map.insert("derived:Sched4:txtTax2".to_string(), "-2500".to_string());
    map.insert("derived:Sched4:txtTax3".to_string(), "1000".to_string());
    let doc = bir_print::frozen_html::filled_document("1702q-2018", &map).expect("render");
    assert_eq!(printed_cell(&doc, "p2c99").0, "-2,500");
    assert!(line_not_comb(&doc, "p2c99"));
}

/// A blank row of a converted column prints as a blank plain cell: its empty
/// comb boxes go too, so the column never shows boxes beside lines.
#[test]
fn blank_row_in_converted_column_loses_its_boxes() {
    let mut map = std::collections::BTreeMap::new();
    for item in 1u8..=10 {
        let amount = if item == 2 {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(format!("frm1701:txtPg4IPart7_{item}A"), amount.to_string());
    }
    // Item 3 stays blank: no key at all.
    map.remove("frm1701:txtPg4IPart7_3A");
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    assert!(
        line_not_comb(&doc, "p4c72"),
        "blank row p4c72 must be covered like the rest of the column"
    );
    assert_eq!(printed_cell(&doc, "p4c72").0, "");
}

/// One font size per converted group: the smallest size any amount needs.
#[test]
fn converted_group_shares_one_font_size() {
    let mut map = std::collections::BTreeMap::new();
    for item in 1u8..=10 {
        let amount = if item == 2 {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(format!("frm1701:txtPg4IPart7_{item}A"), amount.to_string());
    }
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    let mut sizes = std::collections::BTreeSet::new();
    for cell in PART7_A {
        if let Some((_, Some(size))) = Some(printed_cell(&doc, cell)) {
            sizes.insert(size.to_string());
        }
    }
    assert_eq!(
        sizes.len(),
        1,
        "every line of the column must share one font size, saw {sizes:?}"
    );
}

/// Byte-identical: a document without any conversion is untouched by the
/// uniformity passes, pinned by hash.
#[test]
fn no_overflow_document_is_byte_identical() {
    use sha2::{Digest, Sha256};
    let mut map = std::collections::BTreeMap::new();
    for item in 1u8..=10 {
        map.insert(
            format!("frm1701:txtPg4IPart7_{item}A"),
            "1000.00".to_string(),
        );
    }
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    let hash = format!("{:x}", Sha256::digest(doc.as_bytes()));
    // Pinned at the pre-#477 rendering: the passes must never touch a
    // document with nothing converted.
    assert_eq!(
        hash, "721b4e7a106ab7d72a65ccdf611275c05c321443b35e5afd19115c915c1fa92a",
        "no-overflow document moved: if the change is intentional, re-pin"
    );
    let _ = hash;
}

/// The owner's example, exactly: 1701 Part VII combs hold 9 boxes; ten digits
/// in one item turn every amount of the section's column into right-aligned
/// plain text.
#[test]
fn ten_digits_in_a_nine_box_part7_item_convert_the_section() {
    let mut map = std::collections::BTreeMap::new();
    for item in 1u8..=10 {
        // 1,000,000 is seven digits and fits 9 boxes; item 2 carries ten.
        let amount = if item == 2 { "1234567890" } else { "1000000" };
        map.insert(format!("frm1701:txtPg4IPart7_{item}A"), amount.to_string());
    }
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    assert_eq!(printed_cell(&doc, "p4c69").0, "1,234,567,890");
    for cell in PART7_A {
        assert!(
            line_not_comb(&doc, cell),
            "{cell}: every amount of the section must be a right-aligned plain line"
        );
        let (text, _) = printed_cell(&doc, cell);
        assert!(!text.is_empty(), "{cell} must print its amount");
    }
    // Item 2's line keeps every digit.
    assert_eq!(printed_cell(&doc, "p4c66").0, "1,000,000");
}
