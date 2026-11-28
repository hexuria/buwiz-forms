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
    // the same page, same column x): its rows keep their combs. Part VI
    // carries amounts too — empty boxes could never convert either way and
    // the test would prove nothing.
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
            Form1701Party::Taxpayer,
            Some(amount),
        );
    }
    let mut fields = d.print_field_map();
    for item in 1u8..=5 {
        fields.insert(format!("frm1701:txtPg4ISc6_{item}A"), "1000.00".to_string());
    }
    let doc = filled_document("1701-2018", &fields).expect("1701-2018 render");
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
    // Column A of Schedule 3 (items 8..=18) is the 18-row group
    // p2c98..p2c156 of GROUPS.md: every one of its rows converts, and
    // nothing outside the two Schedule 3 columns does.
    let mut in_column = 0;
    let mut outside = 0;
    for m in doc.match_indices("data-overflow-of=\"p2c") {
        let at = m.0 + "data-overflow-of=\"p2c".len();
        let num: u32 = doc[at..at + 3]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(0);
        if (98..=157).contains(&num) {
            in_column += 1;
        } else {
            outside += 1;
        }
    }
    assert_eq!(
        in_column, 18,
        "1701 Schedule 3 column A: all 18 rows must convert with item 10, saw {in_column}"
    );
    assert_eq!(
        outside, 0,
        "no cell outside the Schedule 3 columns may convert (saw {outside})"
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
        assert!(
            line_not_comb(&doc, cell),
            "{cell} must print as a line for the font comparison to mean anything"
        );
        let (_, size) = printed_cell(&doc, cell);
        sizes.insert(size.expect("a line carries its font size").to_string());
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

// ---- #499 review round 2 ------------------------------------------------

/// The rect box (l,t,r,b) of the line printed over `cell`, parsed from
/// `data-rect-box`.
fn rect_box(doc: &str, cell: &str) -> (f64, f64, f64, f64) {
    let needle = format!("data-overflow-of=\"{cell}\"");
    let at = doc
        .find(&needle)
        .unwrap_or_else(|| panic!("{cell} has no line"));
    let tag_start = doc[..at].rfind("<input").unwrap();
    let tag_end = at + doc[at..].find('>').unwrap();
    let tag = &doc[tag_start..tag_end];
    let rb = tag.split("data-rect-box=\"").nth(1).expect("rect box");
    let vals: Vec<f64> = rb[..rb.find('"').unwrap()]
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    (vals[0], vals[1], vals[2], vals[3])
}

/// P1: a blanked row of a converted centavo column loses the PAIRED centavo
/// comb too — its covering rectangle spans the same peso+centavo union as a
/// written row's, never leaving the two centavo boxes beside the blank line.
#[test]
fn blanked_row_blanks_the_paired_centavo_comb() {
    let mut map = std::collections::BTreeMap::new();
    // 1601C Part II: item 14 (peso p1c56, centavo p1c58) overflows; item 15
    // (peso p1c61, centavo p1c63) stays empty.
    map.insert(
        "frm1601c:txtTax14".to_string(),
        "12345678901234.00".to_string(),
    );
    let doc = bir_print::frozen_html::filled_document("1601c-2018", &map).expect("1601c render");
    assert!(
        line_not_comb(&doc, "p1c61"),
        "the empty row's peso cell must be covered like the rest of the column"
    );
    let written = rect_box(&doc, "p1c56");
    let blank = rect_box(&doc, "p1c61");
    assert!(
        (written.2 - blank.2).abs() < 1.0,
        "the blank row's covering rectangle must span the centavo comb too: written right edge {}, blank right edge {}",
        written.2,
        blank.2
    );
}

/// P2: one overflow warns once — the forced conversions of its column mates
/// are uniform by design, not warnings.
#[test]
fn one_overflow_reports_one_fitted_line_warning() {
    let d = part7_draft(Form1701Party::Taxpayer);
    let doc = filled_document("1701-2018", &d.print_field_map()).expect("1701-2018 render");
    let warnings = bir_print::frozen_html::overflow_warnings(&doc);
    let fitted = warnings.iter().filter(|w| w.kind == "amount-comb").count();
    assert_eq!(
        fitted, 1,
        "one overflow must warn once, saw {fitted} amount-comb warnings"
    );
}

/// P3: 1701 page 2 Schedule 1 — items 1, 2, 3A and 3B are ONE group: the
/// cross-reference line "Schedule 2 Item 4A and Part VII Item 5A" printed
/// inside the section is not a band and must not cut item 3B off.
#[test]
fn s1701_schedule1_item3b_groups_with_items_1_to_3a() {
    let mut map = std::collections::BTreeMap::new();
    for (key, amount) in [
        ("frm1701:txtPg2IShed1c_1CI", "12345678901234.00"),
        ("frm1701:txtPg2IShed1c_2CI", "1000.00"),
        ("frm1701:txtPg2IShed1c_3ACI", "1000.00"),
        ("frm1701:txtPg2IShed1c_3BCI", "1000.00"),
    ] {
        map.insert(key.to_string(), amount.to_string());
    }
    let doc = bir_print::frozen_html::filled_document("1701-2018", &map).expect("render");
    for cell in ["p2c68", "p2c77"] {
        assert!(
            line_not_comb(&doc, cell),
            "Schedule 1 CI column: {cell} must print as a line with item 1"
        );
    }
}

/// Byte-identical pins, one per sheet shape: a centavo sheet (2551Q,
/// quarterly) and an annual-corporate sheet (1702EX) with no conversion are
/// untouched by the uniformity passes.
#[test]
fn no_overflow_documents_are_byte_identical_on_more_sheets() {
    use sha2::{Digest, Sha256};
    let pins: [(&str, &str, &str); 2] = [
        (
            "2551q-2018",
            "frm2551q:txtTax14",
            "c713f49efa74bc5e4909186768d6678842f54d5003e8e00a731202026ecb478f",
        ),
        (
            "1702ex-2018",
            "frm1702EX:txtPg1Pt2I14TotalIncome",
            "2dd2b2b9005137237f98cc400bcf32d5f12874556af817ad63ba04a299d17694",
        ),
    ];
    for (slug, key, pin) in pins {
        let mut map = std::collections::BTreeMap::new();
        map.insert(key.to_string(), "1000.00".to_string());
        let doc = bir_print::frozen_html::filled_document(slug, &map).expect("render");
        let hash = format!("{:x}", Sha256::digest(doc.as_bytes()));
        assert_eq!(hash, pin, "{slug}: no-overflow document moved");
    }
}

/// Finding 1: 1600PT's tax-base rows print through cloned template rows; the
/// printed rows' coordinates, never the hidden template's, decide the group.
/// One overflowing tax base converts the other four.
#[test]
fn f1600pt_tax_bases_group_by_the_printed_rows() {
    let mut map = std::collections::BTreeMap::new();
    for row in 1u8..=5 {
        let amount = if row == 1 {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(
            format!("frm1600PT:txtPg1P2TaxBase{row}"),
            amount.to_string(),
        );
    }
    let doc = bir_print::frozen_html::filled_document("1600-pt-2018", &map).expect("render");
    for cell in ["p1c51", "p1c61", "p1c71", "p1c81", "p1c91"] {
        assert!(
            line_not_comb(&doc, cell),
            "1600PT tax base {cell} must print as a line with row 1"
        );
    }
}

/// Finding 3: 1702RT Schedule IIIA — the stacked NOLCO sub-tables share one
/// column x but their column headers split them: an overflow in the amount
/// column must not convert the unapplied-loss column.
#[test]
fn f1702rt_nolco_columns_split_at_their_headers() {
    // The upper sub-table's x=412 column (C3, cells p4c25..p4c46) and the
    // lower sub-table's x=412 column (C6 unapplied, cells p4c54..p4c66) share
    // one x: only their column headers separate them.
    let mut map = std::collections::BTreeMap::new();
    for row in 4u8..=7 {
        let amount = if row == 4 {
            "12345678901234.00"
        } else {
            "1000.00"
        };
        map.insert(format!("frm1702RT:txtPg4Sc3AI{row}C3"), amount.to_string());
        map.insert(
            format!("frm1702RT:txtPg4Sc3AI{row}C6"),
            "1000.00".to_string(),
        );
    }
    let doc = bir_print::frozen_html::filled_document("1702rt-2018c", &map).expect("render");
    // The upper table's column converts with row 4…
    assert!(
        line_not_comb(&doc, "p4c25"),
        "the overflowing upper-table column must print as lines"
    );
    // …but the lower table's unapplied column at the same x keeps its combs.
    assert!(
        !line_not_comb(&doc, "p4c54"),
        "the unapplied-loss column below the headers must keep its combs"
    );
}
