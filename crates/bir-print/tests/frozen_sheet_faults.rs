//! Print faults the 2200T January 2020 sheet must not show.

use bir_print::frozen_html::{filled_document, printed_cell};

#[test]
fn form_2200t_prints_2020_2021_2023_rows_and_item16_sum() {
    let mut map = std::collections::BTreeMap::new();
    for (suffix, due) in [("2020", "100.00"), ("2021", "200.00"), ("2023", "300.00")] {
        for part in ["Export", "Taxable", "Due"] {
            map.insert(
                format!("frm2200Tv2020:txtSched1_1A_{suffix}_{part}"),
                if part == "Due" {
                    due.into()
                } else {
                    "0.00".into()
                },
            );
        }
    }
    map.insert("frm2200Tv2020:txtSched1_TotalDue".into(), "600.00".into());
    map.insert("frm2200Tv2020:txtExciseDue".into(), "600.00".into());

    let doc = filled_document("2200t-2020", &map).unwrap();
    assert_eq!(printed_cell(&doc, "p2c27").0, "100.00");
    assert_eq!(printed_cell(&doc, "p2c32").0, "200.00");
    assert_eq!(printed_cell(&doc, "p2c42").0, "300.00");
    assert_eq!(printed_cell(&doc, "p3c154").0, "600.00");
    assert_eq!(printed_cell(&doc, "p1c900").0, "600");
    assert_eq!(printed_cell(&doc, "p1c901").0, "00");
}

#[test]
fn form_2200t_long_registered_name_prints_whole_over_the_comb() {
    let text = "A".repeat(70);
    let mut map = std::collections::BTreeMap::new();
    map.insert("frm2200Tv2020:registeredName".to_string(), text.clone());
    let doc = filled_document("2200t-2020", &map).unwrap();
    assert_eq!(printed_cell(&doc, "p1c26").0, text);
}
