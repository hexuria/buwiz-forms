//! F2: columns that follow the row's ATC or are preprinted must not say
//! "Can't be filled on this page yet." — the note says what they are, and
//! nothing the filer can type changes.
use super::*;

fn note(code: &str, period: i64, key: &str) -> String {
    open(code, period)["lock_notes"][key]
        .as_str()
        .unwrap_or("")
        .to_string()
}

#[test]
fn f2_derived_columns_say_what_they_are() {
    setup();
    // 1600WP Schedule 1.1: Nature of Payment and Tax Rate follow the row's ATC.
    for key in [
        "frm1600WP:dtSched:naturePayment1",
        "frm1600WP:dtSched:txtRatePercent1",
        "frm1600WP:dtSched:naturePayment10",
        "frm1600WP:dtSched:txtRatePercent10",
    ] {
        assert_eq!(note("1600WP", 1, key), "Computed by the form.", "{key}");
    }
    // 1601FQ Schedule 1: Nature of Income Payment is the ATC's text.
    for row in 0..5 {
        let key = format!("txtNatIncPayment{row}");
        assert_eq!(note("1601FQ", 1, &key), "Computed by the form.", "{key}");
    }
    // 2000 Schedule 1: Tax Rate is the ATC option's (or the DS106 term's).
    for row in 0..3 {
        let key = format!("frm2000:sched1:txtTaxRate{row}");
        assert_eq!(note("2000", 6, &key), "Computed by the form.", "{key}");
    }
    // 1701 NOLCO column E: the unapplied loss is the remainder.
    for row in 4..=7 {
        let key = format!("frm1701:txtPg3IShed6_{row}E");
        assert_eq!(note("1701", 0, &key), "Computed by the form.", "{key}");
    }
    // 2553 rows 14-17 preprinted, row 18 free.
    for item in 14..=17 {
        assert_eq!(
            note("2553", 1, &format!("frm2553:txt{item}A")),
            "Follows the ATC chosen in this row."
        );
        assert_eq!(
            note("2553", 1, &format!("frm2553:txt{item}B")),
            "Printed on the form."
        );
    }
    // Rows 14-16 print a fixed-rate entry, so the rate is printed too; row
    // 17 prints Others OT012, whose rate the filer enters.
    for item in 14..=16 {
        assert_eq!(
            note("2553", 1, &format!("frm2553:txt{item}D")),
            "Printed on the form."
        );
    }
    assert_eq!(note("2553", 1, "frm2553:txt17D"), "");
    assert_eq!(
        note("2553", 1, "frm2553:txt18A"),
        "Follows the ATC chosen in this row."
    );
    // Row 18 is the free row: its ATC and rate take the filer's entry.
    assert_eq!(note("2553", 1, "frm2553:txt18B"), "");
    assert_eq!(note("2553", 1, "frm2553:txt18D"), "");
    // Nowhere in these tables' columns does the not-yet line survive.
    for (code, period, stems) in [
        (
            "1600WP",
            1,
            vec!["dtSched:naturePayment", "dtSched:txtRatePercent"],
        ),
        (
            "2000",
            6,
            vec![
                "sched1:txtTaxRate0",
                "sched1:txtTaxRate1",
                "sched1:txtTaxRate2",
            ],
        ),
        ("2553", 1, vec!["txt14", "txt15", "txt16", "txt17", "txt18"]),
    ] {
        let opened = open(code, period);
        let lingering: Vec<String> = opened["lock_notes"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, note)| {
                note.as_str() == Some("Can't be filled on this page yet.")
                    && stems.iter().any(|stem| {
                        let full = format!("frm{code}:{stem}");
                        key.as_str() == *stem || key.as_str() == full.as_str()
                    })
            })
            .map(|(key, _)| key.clone())
            .collect();
        assert!(
            lingering.is_empty(),
            "{code} still says not-yet: {lingering:?}"
        );
    }
}

#[test]
fn f2_computed_columns_refuse_a_typed_value() {
    setup();
    let opened = open("1600WP", 1);
    let typed = fill(
        "1600WP",
        &opened["draft"],
        json!({"frm1600WP:dtSched:txtRatePercent1": "7"}),
    );
    assert_ne!(
        typed["rejected"],
        json!({}),
        "a typed rate is refused: {:?}",
        typed["rejected"]
    );
}

/// F3: 1702MX Schedules 7.1 and 8.1 NOLCO rows 1-3 amounts A-D take the
/// filer's entry once the deduction method is itemized, and the upload
/// carries them in the official page's own format.
#[test]
fn f3_1702mx_nolco_amounts_are_fillable() {
    setup();
    let opened = open("1702MX", 0);
    // Before the fix every one of these keys said not-yet.
    for key in [
        "frm1702MX:txtPg3IShed7_4A",
        "frm1702MX:txtPg3IShed7_4D",
        "frm1702MX:txtPg4IShed8_6D",
    ] {
        assert_ne!(
            opened["lock_notes"][key].as_str().unwrap_or(""),
            "Can't be filled on this page yet.",
            "{key}"
        );
    }
    // Choose itemized deduction, then type row 1 of Schedule 7.1.
    let itemized = fill(
        "1702MX",
        &opened["draft"],
        json!({"frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized": "true"}),
    );
    let row1 = fill(
        "1702MX",
        &itemized["draft"],
        json!({
            "frm1702MX:txtPg3IShed7_4Year": "2022",
            "frm1702MX:txtPg3IShed7_4A": "300000",
            "frm1702MX:txtPg3IShed7_4B": "100000",
            "frm1702MX:txtPg3IShed7_4C": "0",
            "frm1702MX:txtPg3IShed7_4D": "50000",
        }),
    );
    assert_eq!(
        row1["rejected"],
        json!({}),
        "NOLCO row entries are accepted: {:?}",
        row1["rejected"]
    );
    let map = agent::field_map(kind_of("1702MX").unwrap(), &row1["draft"]).unwrap();
    assert_eq!(map["frm1702MX:txtPg3IShed7_4A"], "300,000.00");
    assert_eq!(map["frm1702MX:txtPg3IShed7_4B"], "100,000.00");
    assert_eq!(map["frm1702MX:txtPg3IShed7_4D"], "50,000.00");
    // Column E (unapplied) is computed: 300000 - 100000 - 0 - 50000.
    assert_eq!(map["frm1702MX:txtPg3IShed7_4E"], "150,000");

    // Print/legacy XML map (to_bir_xml_payload / to_print_field_map) must also
    // carry every typed NOLCO value and comma-formatted computed Col E / Row 8.
    let typed_draft: bir_core::forms::form_1702mx::Form1702MXDraft =
        serde_json::from_value(row1["draft"].clone()).unwrap();
    let typed_xml = typed_draft.to_bir_xml_payload();
    let typed_fields = bir_core::bir_xml::parse_bir_xml_checked(&typed_xml).unwrap();
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed7_4A"], "300,000.00");
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed7_4B"], "100,000.00");
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed7_4C"], "0");
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed7_4D"], "50,000.00");
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed7_4E"], "150,000");
    assert_eq!(typed_fields["frm1702MX:txtPg3IShed78D"], "50,000");
    // Row 4 (item 7) is the form's own current-year row: never an entry.
    for key in [
        "frm1702MX:txtPg3IShed7_7Year",
        "frm1702MX:txtPg4IShed8_7Year",
    ] {
        assert_eq!(
            note("1702MX", 0, key),
            "Computed by the form.",
            "{key} stays derived"
        );
    }
}

/// Reproduces the official sample `1702MX-1225-sched6.official.xml` Schedule 7.1
/// & 8.1 block byte for byte through the page's own write path (typed boxes,
/// untouched boxes, a typed-then-cleared box, Col E, and Row 8 totals) and
/// verifies comma-formatted computed totals on `to_print_field_map`.
#[test]
fn f3_1702mx_nolco_matches_official_sample_xml_byte_for_byte() {
    use bir_core::forms::form_1702mx::Form1702MXDraft;

    setup();
    let opened = open("1702MX", 0);
    let itemized = fill(
        "1702MX",
        &opened["draft"],
        json!({"frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized": "true"}),
    );
    // Type the exact Schedule 7.1 values from 1702MX-1225-sched6.steps.json,
    // plus type and then clear 4C to verify clearing returns to official "0".
    let typed = fill(
        "1702MX",
        &itemized["draft"],
        json!({
            "frm1702MX:txtPg3IShed7_4Year": "2022",
            "frm1702MX:txtPg3IShed7_4A": "300000",
            "frm1702MX:txtPg3IShed7_4B": "100000",
            "frm1702MX:txtPg3IShed7_4C": "25000",
            "frm1702MX:txtPg3IShed7_4D": "50000",
        }),
    );
    assert_eq!(typed["rejected"], json!({}));
    let cleared = fill(
        "1702MX",
        &typed["draft"],
        json!({"frm1702MX:txtPg3IShed7_4C": "0"}),
    );
    assert_eq!(cleared["rejected"], json!({}));

    let draft: Form1702MXDraft = serde_json::from_value(cleared["draft"].clone()).unwrap();
    let layout = draft.official_layout().unwrap();
    let ours_xml = bir_core::official_xml::write(&layout, &draft.to_official_field_map()).unwrap();

    let official_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../bir-core/tests/sample-payloads/1702MX-1225-sched6.official.xml");
    let official_xml = std::fs::read_to_string(&official_path).unwrap();

    let nolco_block = |xml: &str| -> String {
        xml.lines()
            .filter(|line| {
                line.contains("IShed7_")
                    || line.contains("IShed78D")
                    || line.contains("IShed8_")
                    || line.contains("IShed88D")
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let want_block = nolco_block(&official_xml);
    let got_block = nolco_block(&ours_xml);
    assert_eq!(
        want_block.lines().count(),
        50,
        "Schedule 7.1 + 8.1 block has 50 XML entries"
    );
    assert_eq!(
        got_block, want_block,
        "page-path official upload XML Schedule 7.1 & 8.1 block must match official sample byte for byte"
    );

    // Print map also carries comma-formatted computed Col E and Row 8 total.
    let print_map = draft.to_print_field_map();
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_4A"], "300,000.00");
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_4B"], "100,000.00");
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_4C"], "0");
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_4D"], "50,000.00");
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_4E"], "150,000");
    assert_eq!(print_map["frm1702MX:txtPg3IShed78D"], "50,000");
    // Untouched box (row 5 col A) uploads as "0" and stays blank on print.
    assert_eq!(print_map["frm1702MX:txtPg3IShed7_5A"], "");
}
