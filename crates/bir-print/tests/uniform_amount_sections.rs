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
