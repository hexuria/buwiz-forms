//! PR 11: the excise returns (2200A, 2200AN, 2200C, 2200M, 2200P, 2200S,
//! 2200T) against their printed sheets, and maintainer decision D8 (their
//! penalties run from the date the filer types). The oracles are the frozen
//! sheets' own files (html-frozen/<slug>/writer-cells.json captions and
//! joins, index.html's printed text), the official program's control
//! inventory (rules/forms/<id>/fixtures/runtime-control-inventory-v796.json),
//! the typed drafts' own constants and fields, hand lists from the sheets,
//! and literal drafts saved by the pr0b-push code; never this module's own
//! refusal.

use super::*;

/// `(code, frozen sheet slug, rules form id)` of the PR 11 forms.
const FORMS: &[(&str, &str, &str)] = &[
    ("2200A", "2200a-2020", "2200a-v2020"),
    ("2200AN", "2200an-2018", "2200an-v2018"),
    ("2200C", "2200c-2018", "2200c-v2018"),
    ("2200M", "2200m-2018", "2200m-v2018"),
    ("2200P", "2200p-2020", "2200p-v2020"),
    ("2200S", "2200s-2018", "2200s-v2018"),
    ("2200T", "2200t-2022", "2200t-v2020"),
];

fn repo_file(path: &str) -> String {
    let path = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn writer_cells(slug: &str) -> Value {
    serde_json::from_str(&repo_file(&format!("html-frozen/{slug}/writer-cells.json"))).unwrap()
}

/// Every `writer_key -> printed caption` of a sheet's join lists.
fn printed_captions(slug: &str) -> BTreeMap<String, String> {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins", "date_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .filter_map(|join| {
            Some((
                join["writer_key"].as_str()?.to_string(),
                join["printed"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// The reason writer-cells.json's `not_joined` gives a key, if any.
fn not_joined(slug: &str, key: &str) -> Option<String> {
    writer_cells(slug)["not_joined"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["writer_key"] == key
                || entry["writer_keys"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|k| k == key))
        })
        .map(|entry| entry["reason"].as_str().unwrap().to_string())
}

/// The frozen sheet's printed words, one space between text runs.
fn sheet_text(slug: &str) -> String {
    let html = repo_file(&format!("html-frozen/{slug}/index.html"));
    let mut text = String::new();
    let mut tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                tag = true;
                text.push(' ');
            }
            '>' => tag = false,
            _ if !tag => text.push(c),
            _ => {}
        }
    }
    text.replace("&amp;", "&")
        .replace('\u{2019}', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether the official program has `key` disabled at load.
fn disabled_at_load(form_id: &str, key: &str) -> bool {
    let inventory: Value = serde_json::from_str(&repo_file(&format!(
        "rules/forms/{form_id}/fixtures/runtime-control-inventory-v796.json"
    )))
    .unwrap();
    inventory["controls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == key && c["disabled"] == true)
}

/// One drawn box: its part, item number, item label, key, box label and
/// type; a table cell carries its row's number and its column's caption.
#[derive(Debug, Clone)]
struct Drawn {
    part: String,
    no: String,
    item: String,
    key: String,
    label: String,
    row_label: String,
}

fn drawn(code: &str) -> Vec<Drawn> {
    let form = schema_entry(code).unwrap();
    let mut out = Vec::new();
    for part in form["parts"].as_array().unwrap() {
        let name = part["n"].as_str().unwrap().to_string();
        for item in part["items"].as_array().unwrap() {
            let no = item["no"].as_str().unwrap_or("").to_string();
            let label = item["l"].as_str().unwrap_or("").to_string();
            for field in item["f"].as_array().into_iter().flatten() {
                if field["t"] != "hidden" {
                    out.push(Drawn {
                        part: name.clone(),
                        no: no.clone(),
                        item: label.clone(),
                        key: field["k"].as_str().unwrap().to_string(),
                        label: field["l"].as_str().unwrap_or("").to_string(),
                        row_label: String::new(),
                    });
                }
            }
            for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
                let row_label = item["row_labels"]["rows"][r]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                for (c, key) in row.as_array().unwrap().iter().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] != "hidden" {
                        out.push(Drawn {
                            part: name.clone(),
                            no: no.clone(),
                            item: label.clone(),
                            key: key.as_str().unwrap().to_string(),
                            label: col["l"].as_str().unwrap_or("").to_string(),
                            row_label: row_label.clone(),
                        });
                    }
                }
            }
        }
    }
    out
}

fn placed(code: &str, key: &str) -> Drawn {
    drawn(code)
        .into_iter()
        .find(|d| d.key == key)
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
}

fn is_drawn(code: &str, key: &str) -> bool {
    drawn(code).iter().any(|d| d.key == key)
}

/// Every item of `code` numbered `no`.
fn items(code: &str, no: &str) -> Vec<Value> {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| part["items"].as_array().unwrap())
        .filter(|it| it["no"] == no)
        .cloned()
        .collect()
}

/// The printed item number a caption starts with (`7A`, `17B`, `Item 6`).
fn printed_number(printed: &str) -> Option<String> {
    let printed = printed.strip_prefix("Item ").unwrap_or(printed);
    let head = printed.split_whitespace().next()?;
    let digits = head.trim_end_matches(|c: char| c.is_ascii_uppercase());
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && head.len() <= 3)
        .then(|| head.to_string())
}

fn printed(code: &str, draft: &Value) -> BTreeMap<String, String> {
    with_database(|db| print_map(db, kind_of(code)?, draft))
        .unwrap()
        .0
}

fn cents(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// R3 on every PR 11 form: each box the sheet prints under an item number
/// is drawn under that number, and no drawn box sits in an unnumbered
/// "Additional details" group. The oracle is writer-cells.json's printed
/// captions.
#[test]
fn pr11_boxes_sit_under_their_printed_item_numbers() {
    for &(code, slug, _) in FORMS {
        let boxes = drawn(code);
        let mut checked = 0;
        for (key, caption) in printed_captions(slug) {
            let Some(number) = printed_number(&caption) else {
                continue;
            };
            let Some(at) = boxes.iter().find(|d| d.key == key) else {
                continue;
            };
            let item_number = number.trim_end_matches(|c: char| c.is_ascii_uppercase());
            let at_item = at
                .no
                .trim_end_matches(|c: char| c.is_ascii_lowercase() || c.is_ascii_uppercase());
            assert!(
                at.no == number || at.no == item_number || at_item == number,
                "{code} {key} prints at {caption:?}, drawn under {:?} ({})",
                at.no,
                at.part
            );
            checked += 1;
        }
        assert!(checked >= 10, "{code}: {checked} boxes checked");
        for d in &boxes {
            assert!(
                d.item != "Additional details",
                "{code} {} in {} item {:?}",
                d.key,
                d.part,
                d.no
            );
        }
        // No item heading is cut mid-word (it ends where the sheet's does).
        for part in schema_entry(code).unwrap()["parts"].as_array().unwrap() {
            for it in part["items"].as_array().unwrap() {
                let label = it["l"].as_str().unwrap();
                assert!(!label.ends_with(" or"), "{code}: {label:?} is cut");
                assert!(!label.ends_with("go to th"), "{code}: {label:?} is cut");
            }
        }
    }
}

/// The Part IV and tax agent boxes of each form, by the official keys.
fn part_iv_keys(code: &str) -> (Vec<String>, Vec<String>, [&'static str; 4]) {
    let numbered = |prefix: &str, columns: &[&str], rows: [u32; 4]| -> Vec<String> {
        rows.iter()
            .flat_map(|n| columns.iter().map(move |c| format!("{prefix}{c}{n}")))
            .collect()
    };
    match code {
        "2200S" => (
            numbered(
                "frm2200S:txt",
                &["Agency", "Number", "Date", "Amount"],
                [33, 34, 35, 36],
            )
            .into_iter()
            .filter(|k| k != "frm2200S:txtAgency35" && k != "frm2200S:txtDate36x")
            .chain(["frm2200S:txtParticular36".to_string()])
            .collect(),
            vec![
                "txtTaxAgentNo".into(),
                "txtDateIssue".into(),
                "txtDateExpiry".into(),
            ],
            ["27", "28", "29", "30"],
        ),
        "2200AN" => (
            numbered(
                "frm2200ANv2018:txt",
                &["Number", "Date", "Amount"],
                [24, 25, 26, 27],
            )
            .into_iter()
            .chain([
                "frm2200ANv2018:txtAgency24".to_string(),
                "frm2200ANv2018:txtAgency25".to_string(),
                "frm2200ANv2018:txtAgency27".to_string(),
                "frm2200ANv2018:txtParticular27".to_string(),
            ])
            .collect(),
            vec![
                "txtTaxAgentNo".into(),
                "txtDateIssue".into(),
                "txtDateExpiry".into(),
            ],
            ["25", "26", "27", "28"],
        ),
        "2200C" | "2200M" | "2200P" => {
            let p = match code {
                "2200C" => "frm2200C:txtPg1Pt3",
                "2200M" => "frm2200M:txtPg1Pt4",
                _ => "frm2200P:txtPg1Pt4",
            };
            let form = &p[..p.find(':').unwrap()];
            let keys = [
                "I25CashC1",
                "I25CashC2",
                "I25Date",
                "I25CashC3",
                "I26CheckC1",
                "I26CheckC2",
                "I26Date",
                "I26CheckC3",
                "I27TaxDebitC2",
                "I27Date",
                "I27TaxDebitC3",
                "I28OthersC1",
                "I28OthersC2",
                "I28OthersC3",
                "I28Date",
                "I28OthersC4",
            ]
            .iter()
            .map(|k| format!("{p}{k}"))
            .collect();
            (
                keys,
                ["TaxAcredNo", "DateOfIssue", "DateOfExpiry"]
                    .iter()
                    .map(|k| format!("{form}:txtPg1Pt4{k}"))
                    .collect(),
                ["25", "26", "27", "28"],
            )
        }
        _ => unreachable!(),
    }
}

/// 2200S R3.0 / manual-2, 2200AN R3.1 / manual-0, 2200C R3.2 / manual-3,
/// 2200M manual-3, 2200P R3.5 / manual-3: Part IV and the tax agent's boxes
/// are printed items with a note where the official program disables them
/// at load (the control inventory) and this app writes them for no draft
/// (absent from the field map, so the upload keeps the official default).
#[test]
fn pr11_part_iv_and_tax_agent_are_notes_where_the_program_disables_them() {
    setup();
    for &(code, slug, form_id) in FORMS {
        if matches!(code, "2200A" | "2200T") {
            continue;
        }
        let (part_iv, agent, numbers) = part_iv_keys(code);
        let opened = open(code, 1);
        let map = agent::field_map(kind_of(code).unwrap(), &opened["draft"]).unwrap();
        for key in part_iv.iter().chain(&agent) {
            assert!(
                disabled_at_load(form_id, key),
                "{code}: {key} is disabled at load"
            );
            assert!(!is_drawn(code, key), "{code}: {key} is not drawn");
            assert!(
                !map.contains_key(key),
                "{code}: the field map never writes {key}"
            );
        }
        // Each Part IV row is its printed item, with the note.
        for (no, row) in
            numbers
                .iter()
                .zip(["Cash/Bank Debit Memo", "Check", "Tax Debit Memo", "Others"])
        {
            let found = items(code, no);
            let note = found
                .iter()
                .find(|it| it["note"].is_string())
                .unwrap_or_else(|| panic!("{code}: item {no} is a note"));
            assert!(note["l"].as_str().unwrap().starts_with(row), "{code} {no}");
            assert!(
                note["note"].as_str().unwrap().contains(&format!(
                    "rules/forms/{form_id}/fixtures/runtime-control-inventory-v796.json"
                )),
                "{code} {no}"
            );
            // The sheet prints it under that number.
            assert!(
                sheet_text(slug).contains(&format!("{no} {row}")),
                "{code}: the sheet prints {no} {row}"
            );
        }
        let agent_note = items(code, "")
            .into_iter()
            .find(|it| {
                it["l"]
                    .as_str()
                    .unwrap()
                    .starts_with("Tax Agent Accreditation No.")
            })
            .unwrap_or_else(|| panic!("{code}: the tax agent note"));
        assert!(
            agent_note["note"]
                .as_str()
                .unwrap()
                .contains("disabled at load")
        );
    }
}

/// 2200A R3.1, R3.2, manual-3 and 2200T's Part IV: the payment details the
/// official page keeps open sit under items 25-28 with the printed column
/// captions, and each typed value files under its official key.
#[test]
fn pr11_2200a_and_2200t_part_iv_rows_take_and_file_their_details() {
    setup();
    for (code, p) in [("2200A", "frm2200Av2020:"), ("2200T", "frm2200Tv2020:")] {
        let rows = [
            (
                "25",
                "Cash/Bank Debit Memo",
                vec!["cashBank", "cashNum", "cashDate", "cashAmt"],
            ),
            (
                "26",
                "Check",
                vec!["checkBank", "checkNum", "checkDate", "checkAmt"],
            ),
            ("27", "Tax Debit Memo", vec!["taxNum", "taxDate", "taxAmt"]),
            (
                "28",
                "Others (specify)",
                vec![
                    "othersDetails",
                    "othersBank",
                    "othersNum",
                    "othersDate",
                    "othersAmt",
                ],
            ),
        ];
        let opened = open(code, 1);
        let mut edits = Map::new();
        for (no, heading, keys) in &rows {
            let item = &items(code, no)[0];
            assert_eq!(item["l"], *heading, "{code} {no}");
            let drawn_keys: Vec<String> = item["f"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["k"].as_str().unwrap().to_string())
                .collect();
            let want: Vec<String> = keys.iter().map(|k| format!("{p}{k}")).collect();
            assert_eq!(drawn_keys, want, "{code} {no}");
            for key in &want {
                let label = placed(code, key).label;
                let column = if key.ends_with("Bank") {
                    "Drawee Bank/Agency"
                } else if key.ends_with("Num") {
                    "Number"
                } else if key.ends_with("Date") {
                    "Date (MM/DD/YYYY)"
                } else if key.ends_with("Amt") {
                    "Amount"
                } else {
                    "Particulars"
                };
                assert_eq!(label, column, "{code}: {key}");
                edits.insert(
                    key.clone(),
                    json!(if key.ends_with("Date") {
                        "10/15/2026"
                    } else {
                        "X1"
                    }),
                );
            }
        }
        // 2200T's printed captions agree (writer-cells.json joins them).
        if code == "2200T" {
            let captions = printed_captions("2200t-2022");
            assert_eq!(
                captions[&format!("{p}cashBank")],
                "25 Cash/Bank Debit Memo: Drawee Bank/Agency"
            );
            assert_eq!(captions[&format!("{p}othersNum")], "28 Others: Number");
        } else {
            assert!(
                not_joined("2200a-2020", &format!("{p}cashBank"))
                    .unwrap()
                    .contains("Part IV payment detail")
            );
        }
        let filled = fill(code, &opened["draft"], Value::Object(edits.clone()));
        assert_eq!(filled["rejected"], json!({}), "{code}");
        let map = agent::field_map(kind_of(code).unwrap(), &filled["draft"]).unwrap();
        for (key, value) in &edits {
            assert_eq!(
                map[key],
                value.as_str().unwrap(),
                "{code}: {key} files as typed"
            );
        }
    }
}

/// The fixed Schedule 1 rows of a form's Rust table: `(key, ATC, product,
/// period or bracket, rate)`.
fn fixed_rows(code: &str) -> Vec<(String, String, String, String, String)> {
    match code {
        "2200A" => bir_core::forms::form_2200a::FORM_2200A_FIXED_ROWS
            .iter()
            .map(|r| (r.0.into(), r.1.into(), r.2.into(), r.3.into(), r.4.into()))
            .collect(),
        "2200T" => bir_core::forms::form_2200t::FORM_2200T_FIXED_ROWS
            .iter()
            .map(|r| (r.0.into(), r.1.into(), r.2.into(), r.3.into(), r.4.into()))
            .collect(),
        _ => unreachable!(),
    }
}

/// 2200A R3.4, preview.0, manual-0: Part V Schedule 1 is its own part, one
/// table per ATC as printed, every one of the sixty fixed rows drawn once
/// with its printed period and rate beside it (writer-cells.json's captions
/// name the same period), then the Others rows under 4) Others (specify).
#[test]
fn pr11_2200a_schedule_1_is_by_atc_with_every_fixed_row_once() {
    let p = "frm2200Av2020:";
    let boxes = drawn("2200A");
    let captions = printed_captions("2200a-2020");
    for (key, atc, _product, period, rate) in fixed_rows("2200A") {
        for column in ["Export", "Taxable", "Due"] {
            let cell = format!("{p}txtSched1_{key}_{column}");
            let at: Vec<&Drawn> = boxes.iter().filter(|d| d.key == cell).collect();
            assert_eq!(at.len(), 1, "{cell} is drawn once");
            assert!(
                at[0].part.starts_with("Part V · Schedule 1"),
                "{cell}: {}",
                at[0].part
            );
            assert!(
                at[0].item.starts_with(&format!("{atc} · ")),
                "{cell}: {}",
                at[0].item
            );
            assert_eq!(at[0].row_label, format!("{period} · {rate}"), "{cell}");
            assert!(
                captions[&cell].contains(&period),
                "{cell}: {}",
                captions[&cell]
            );
        }
    }
    let others = &items("2200A", "4")
        .into_iter()
        .find(|it| it["l"] == "Others (specify)")
        .expect("4) Others (specify)");
    assert_eq!(others["rows"], 3);
    assert_eq!(others["keys"][0][0], format!("{p}txtSched1_xa_others1_Atc"));
    assert!(captions[&format!("{p}txtSched1_xa_others1_Atc")].starts_with("4 Others (specify)"));
    // The rows' tick boxes are the page's row switch, not printed.
    for n in 1..=3 {
        let key = format!("{p}txtSched1_xa_others{n}_Check");
        assert!(!is_drawn("2200A", &key), "{key}");
        assert!(
            not_joined("2200a-2020", &key)
                .unwrap()
                .contains("row-enable checkbox")
        );
    }
    assert!(
        !schema_entry("2200A").unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["n"] == "Page 3")
    );
}

/// 2200T R3.3, preview.0, manual-0, manual-1: Schedule 1 is Part V with
/// the rows the August 2022 sheet prints (the ones writer-cells.json joins),
/// by ATC, the Tobacco Inspection Fees and the Others rows at its end; the
/// v2020 program's 2020 and 2021 rows, which the sheet does not print, are
/// under Not on the printed sheet. No part is named Page 3.
#[test]
fn pr11_2200t_schedule_1_prints_the_2022_rows_and_keeps_the_older_ones_apart() {
    let p = "frm2200Tv2020:";
    let boxes = drawn("2200T");
    let captions = printed_captions("2200t-2022");
    let mut printed_rows = 0;
    for (key, atc, _product, period, rate) in fixed_rows("2200T") {
        let cell = format!("{p}txtSched1_{key}_Export");
        let at = boxes
            .iter()
            .find(|d| d.key == cell)
            .unwrap_or_else(|| panic!("{cell}"));
        if captions.contains_key(&cell) {
            printed_rows += 1;
            assert!(
                at.part.starts_with("Part V · Schedule 1"),
                "{cell}: {}",
                at.part
            );
        } else {
            assert_eq!(at.part, "Not on the printed sheet", "{cell}");
            assert!(
                key.contains("2020") || key.contains("2021") || key.starts_with("6_"),
                "{key}"
            );
        }
        assert!(
            at.item.contains(&atc) || at.row_label.contains(&atc),
            "{cell}: {}",
            at.item
        );
        assert!(at.row_label.contains(&rate), "{cell}: {}", at.row_label);
        if !period.is_empty() && !key.starts_with("xt") {
            assert!(at.row_label.starts_with(&period), "{cell}");
        }
    }
    assert_eq!(printed_rows, 30, "22 dated rows and 8 inspection fees");
    let others = placed("2200T", &format!("{p}txtSched1_xt_others1_Export"));
    assert!(others.part.starts_with("Part V · Schedule 1"));
    assert_eq!(others.item, "Others (specify)");
    // Items 17, 21 and 23 are their printed sub-items (manual-2).
    for (no, key, label) in [
        (
            "17A",
            "txtLess_Balance",
            "Balance Carried Over from Previous Return",
        ),
        ("17B", "txtLess_Excise", "Creditable Excise Tax"),
        ("17C", "txtLess_Tot", "Total (Sum of Items 17A and 17B)"),
        ("21A", "txtPen_Surcharge", "Surcharge"),
        ("21B", "txtPen_Interest", "Interest"),
        ("21C", "txtPen_Compromise", "Compromise"),
        ("21D", "txtPen_Tot", "Total Penalties"),
        ("23A", "txtPay_TaxPayment", "Tax Payment/Deposit"),
        ("23B", "txtPay_Penalties", "Penalties (from Item 21D)"),
        ("23C", "txtPay_Tot", "Total Payment Made Today"),
    ] {
        let at = placed("2200T", &format!("{p}{key}"));
        assert_eq!(at.no, no, "{key}");
        assert_eq!(at.label, label, "{key}");
        assert!(captions[&format!("{p}{key}")].starts_with(no), "{key}");
    }
    assert!(
        !schema_entry("2200T").unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|part| part["n"] == "Page 3")
    );
}

/// Lesson 14 on 2200M (manual-2) and 2200P (manual-2): rows 1-8 and 1-25
/// print their ATC, description and rate, which the page shows as the row's
/// label and never as a box; the Others rows the filer describes keep theirs.
#[test]
fn pr11_2200m_and_2200p_show_the_preprinted_rates_never_as_boxes() {
    use bir_core::forms::form_2200m::FORM_2200M_FIXED_ROWS;
    use bir_core::forms::form_2200p::FORM_2200P_FIXED_ROWS;
    for n in 1..=8 {
        for key in [
            format!("frm2200M:txtSched1_LocallyExtractedTaxRateE{n}"),
            format!("frm2200M:txtSched1_ImportedTaxRate{n}"),
        ] {
            assert!(disabled_at_load("2200m-v2018", &key), "{key}");
            assert!(!is_drawn("2200M", &key), "{key}");
        }
        let (atc, description, rate) = FORM_2200M_FIXED_ROWS[n - 1];
        let at = placed(
            "2200M",
            &format!("frm2200M:txtSched1_LocallyExtractedTaxDueF{n}"),
        );
        assert_eq!(at.row_label, format!("{atc} · {description} · {rate}"));
    }
    for n in [9, 10] {
        assert!(is_drawn(
            "2200M",
            &format!("frm2200M:txtSched1_LocallyExtractedTaxRateE{n}")
        ));
        assert!(is_drawn(
            "2200M",
            &format!("frm2200M:txtSched1_ImportedTaxRate{n}")
        ));
        // The row switch and the continuation copy of its description.
        assert!(!is_drawn(
            "2200M",
            &format!("frm2200M:chkBoxSched1Description{n}")
        ));
        assert!(!is_drawn(
            "2200M",
            &format!("frm2200M:txtSched1_descriptionCont{n}")
        ));
    }
    for n in 1..=25 {
        for column in ["ATC", "Description", "UnitOfMeasure", "ApplicableRate"] {
            let key = format!("frm2200P:txtSched1{column}{n}");
            assert!(!is_drawn("2200P", &key), "{key}");
        }
        let (atc, description, unit, rate) = FORM_2200P_FIXED_ROWS[n - 1];
        let at = placed("2200P", &format!("frm2200P:txtSched1BasicExciseTaxDue{n}"));
        assert_eq!(
            at.row_label,
            format!("{atc} · {description} · {unit} · {rate}")
        );
    }
    for n in [26, 27] {
        for column in ["ATC", "Description", "UnitOfMeasure", "ApplicableRate"] {
            let at = placed("2200P", &format!("frm2200P:txtSched1{column}{n}"));
            assert_eq!(at.item, "Others (specify)");
        }
        assert!(!is_drawn("2200P", &format!("frm2200P:chkBoxSched1ATC{n}")));
    }
    // The added rows' tables keep their rows, without the preprinted XM060
    // ATC column and the copied description.
    assert!(!is_drawn("2200M", "frm2200M:txtSched1ATC11"));
    assert!(!is_drawn("2200M", "frm2200M:txtSched1_descriptionCont11"));
    assert!(is_drawn("2200M", "frm2200M:txtSched1_description11"));
    assert!(is_drawn("2200P", "frm2200P:txtSched1ATC28"));
}

/// 2200C manual-0 and manual-2: item 3 is the ATC the sheet prints (XC010),
/// never a box; Part V Schedule 1 is one table of its printed columns A-J,
/// rows 1-10 (writer-cells.json names each cell's row and column).
#[test]
fn pr11_2200c_item_3_and_schedule_1_read_as_printed() {
    let atc = &items("2200C", "3")[0];
    assert_eq!(atc["note"], "XC010 (printed on the form)");
    assert!(
        sheet_text("2200c-2018")
            .contains("3 ATC 4 No. of Sheet/s Attached (MM/DD/YYYY) / / X C 0 1 0")
    );
    assert!(!is_drawn("2200C", "frm2200C:txtPg1I3ATC"));
    assert!(disabled_at_load("2200c-v2018", "frm2200C:txtPg1I3ATC"));
    let captions = printed_captions("2200c-2018");
    let columns = [
        ("CPPexmpt", "A"),
        ("CPPexcise", "B"),
        ("CPPnonexcise", "C"),
        ("GRnetVat", "D"),
        ("GRexciseNet", "E"),
        ("GRexciseExmpt", "F"),
        ("GRnonexcise", "G"),
        ("ACexcise", "H"),
        ("ACvat", "I"),
        ("ACtotAmountBill", "J"),
    ];
    for n in 1..=10 {
        for (stem, letter) in columns {
            let key = format!("frm2200C:{stem}{n}");
            let at = placed("2200C", &key);
            assert!(at.part.starts_with("Part V · Schedule 1"), "{key}");
            assert!(
                at.label.starts_with(&format!("({letter})")),
                "{key}: {}",
                at.label
            );
            assert!(
                captions[&key].starts_with(&format!("Schedule 1 row {n}, column {letter}:")),
                "{key}"
            );
        }
    }
}

/// 2200A preview.0, 2200T preview.0, 2200S preview.0 and R8.0, 2200AN
/// preview.0: the boxes the probe now binds take what is typed and file it
/// under their official key; before, the page could not fill them and the
/// print wrote a constant.
#[test]
fn pr11_newly_bound_boxes_take_and_file_what_is_typed() {
    setup();
    for (code, key) in [
        ("2200A", "frm2200Av2020:txtSched1_3B_2024_Taxable"),
        ("2200A", "frm2200Av2020:txtSched1_3A_2021B_Export"),
        ("2200T", "frm2200Tv2020:txtSched1_xt200_Taxable"),
        ("2200T", "frm2200Tv2020:txtSched1_xt100_Due"),
    ] {
        let opened = open(code, 1);
        let filled = fill(code, &opened["draft"], json!({ key: "1234.5" }));
        assert_eq!(filled["rejected"], json!({}), "{code}: {key}");
        let map = agent::field_map(kind_of(code).unwrap(), &filled["draft"]).unwrap();
        assert_eq!(
            agent::parse_amount(&map[key]),
            Some(1234.5),
            "{code}: {key} files"
        );
        assert_eq!(
            agent::parse_amount(&printed(code, &filled["draft"])[key]),
            Some(1234.5)
        );
    }
    // 2200S: on a prepayment, item 17's Other Similar Schemes text and
    // items 23A-23C (one Edit) file what is typed.
    let opened = open("2200S", 1);
    let prepaid = fill(
        "2200S",
        &opened["draft"],
        json!({"frm2200S:chkPymntManner_2": "true", "frm2200S:chkPymntManner_3": "true"}),
    );
    assert_eq!(prepaid["rejected"], json!({}));
    let schemed = fill(
        "2200S",
        &prepaid["draft"],
        json!({"frm2200S:txtOthMannerofPymnt": "Bond"}),
    );
    assert_eq!(schemed["rejected"], json!({}));
    let keys = [
        "frm2200S:txtTax21A",
        "frm2200S:txtTax21B",
        "frm2200S:txtTax21C",
    ];
    let group = schemed["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| strings(&g["keys"]).contains(keys[0]))
        .expect("23A-23C are one group")
        .clone();
    assert_eq!(group["applies"], true, "on a prepayment");
    let penalized = fill_unlocked(
        "2200S",
        &schemed["draft"],
        json!({"frm2200S:txtTax21A": "10", "frm2200S:txtTax21B": "20", "frm2200S:txtTax21C": "30"}),
        &keys,
    );
    assert_eq!(penalized["rejected"], json!({}));
    let map = agent::field_map(kind_of("2200S").unwrap(), &penalized["draft"]).unwrap();
    assert_eq!(map["frm2200S:txtOthMannerofPymnt"], "Bond");
    for (key, want) in keys.iter().zip([10.0, 20.0, 30.0]) {
        assert_eq!(agent::parse_amount(&map[*key]), Some(want), "{key}");
    }
    assert_eq!(agent::parse_amount(&map["frm2200S:txtTax21D"]), Some(60.0));
    // 2200AN item 3.
    let opened = open("2200AN", 1);
    assert_eq!(placed("2200AN", "frm2200ANv2018:txtSheets").no, "3");
    let filled = fill(
        "2200AN",
        &opened["draft"],
        json!({"frm2200ANv2018:txtSheets": "4"}),
    );
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(
        agent::field_map(kind_of("2200AN").unwrap(), &filled["draft"]).unwrap()["frm2200ANv2018:txtSheets"],
        "4"
    );
}

/// 2200S manual-0 (25B's tick box): 25B's Pay Penalties flag is the page's
/// own (writer-cells.json not_joined: the paper has no box for it), so it is
/// not drawn; the field map still ticks it whenever 25B is not zero.
#[test]
fn pr11_2200s_25b_has_no_tick_box_the_sheet_does_not_print() {
    assert!(!is_drawn("2200S", "frm2200S:PayPenalties"));
    assert!(
        not_joined("2200s-2018", "frm2200S:PayPenalties")
            .unwrap()
            .contains("the paper has no box for the flag")
    );
    let at = placed("2200S", "frm2200S:txtTax23B");
    assert_eq!(at.no, "25B");
}

/// 2200T R2.0/R3.0, 2200AN manual-1, 2200S manual-3, 2200C R2.0/R3.0/
/// manual-1, 2200M R2.0/R3.1/manual-1, 2200P R2.0/R3.1/manual-1: the page 2
/// (and 3) header copies of the TIN and name, and the second RDO select, are
/// not drawn; the print still writes the header from the draft (the sheet's
/// joins and not_joined reasons name where).
#[test]
fn pr11_header_copies_and_duplicate_rdo_boxes_are_not_drawn_but_print() {
    setup();
    let copies: &[(&str, &[&str])] = &[
        (
            "2200T",
            &[
                "frm2200Tv2020:tinA_2",
                "frm2200Tv2020:branchCode_2",
                "frm2200Tv2020:registeredName_2",
                "frm2200Tv2020:tinC_3",
                "frm2200Tv2020:registeredName_3",
            ],
        ),
        (
            "2200AN",
            &[
                "frm2200ANv2018:txtPage2TIN",
                "frm2200ANv2018:txtPage2Taxpayer",
            ],
        ),
        (
            "2200S",
            &[
                "frm2200S:txtSched1TIN1",
                "frm2200S:txtSched1BranchCode",
                "frm2200S:txtSched1TaxpayerName",
            ],
        ),
        (
            "2200C",
            &[
                "frm2200C:txtPg2TinC1",
                "frm2200C:txtPg2TinC4",
                "frm2200C:rdoPg1Pt1I6RDO",
            ],
        ),
        (
            "2200M",
            &[
                "frm2200M:txtPg2TinC1",
                "frm2200M:txtPg2RegisteredName",
                "frm2200M:rdoPg1Pt1I6RDO",
            ],
        ),
        (
            "2200P",
            &[
                "frm2200P:txtPg2TinC2",
                "frm2200P:txtPg2RegisteredName",
                "frm2200P:rdoPg1Pt1I6RDO",
            ],
        ),
    ];
    for (code, keys) in copies {
        for key in *keys {
            assert!(!is_drawn(code, key), "{code}: {key}");
        }
        let opened = open(code, 1);
        let print = printed(code, &opened["draft"]);
        let header: Vec<&String> = print
            .iter()
            .filter(|(k, v)| {
                (k.contains("tin_digits")
                    || k.contains("page_header")
                    || k.ends_with("_2")
                    || k.contains("Pg2")
                    || k.contains("Page2"))
                    && v.contains("123456788")
            })
            .map(|(k, _)| k)
            .collect();
        assert!(
            !header.is_empty(),
            "{code}: the print writes the page 2 TIN"
        );
    }
    for (code, slug, item) in [
        ("2200C", "2200c-2018", "6"),
        ("2200M", "2200m-2018", "5"),
        ("2200P", "2200p-2020", "5"),
    ] {
        let rdo = &items(code, item)[0];
        assert_eq!(rdo["f"].as_array().unwrap().len(), 1, "{code}: one RDO box");
        let key = format!("frm{code}:rdoPg1Pt1I6RDO");
        assert!(
            not_joined(slug, &key).unwrap().starts_with("duplicate of"),
            "{key}"
        );
    }
}

/// 2200AN manual-2, 2200S manual-4, 2200C manual-4, 2200M manual-4, 2200P
/// manual-4 (and 2200A and 2200T, so items 14 and 15 are drawn as printed):
/// Part II is one tick box per printed item, the number on the
/// item and the printed caption on the box (writer-cells.json's tick-box
/// captions), so neither repeats; the specify box sits with its tick box.
#[test]
fn pr11_part_ii_is_one_tick_box_per_printed_item() {
    let forms: &[(&str, &str, [&str; 3], &str)] = &[
        (
            "2200A",
            "2200a-2020",
            ["optPayment_1", "optPayment_2", "optPayment_3"],
            "frm2200Av2020:paymentOther",
        ),
        (
            "2200T",
            "2200t-2022",
            ["optPayment_1", "optPayment_2", "optPayment_3"],
            "frm2200Tv2020:paymentOther",
        ),
        (
            "2200AN",
            "2200an-2018",
            ["MannerOfPayment1", "MannerOfPayment2", "MannerOfPayment3"],
            "frm2200ANv2018:MannerOfPaymentOthers",
        ),
        (
            "2200S",
            "2200s-2018",
            ["chkPymntManner_1", "chkPymntManner_2", "chkPymntManner_3"],
            "frm2200S:txtOthMannerofPymnt",
        ),
        (
            "2200C",
            "2200c-2018",
            [
                "rdoPg1Pt2MOPPaymentActual",
                "rdoPg1Pt2MOPPrepayment",
                "rdoPg1Pt2MOPOther",
            ],
            "frm2200C:txtPg1Pt2MOPOtherDesc",
        ),
        (
            "2200M",
            "2200m-2018",
            [
                "rdoPg1Pt2MOPPaymentActual",
                "rdoPg1Pt2MOPPrepayment",
                "rdoPg1Pt2MOPOther",
            ],
            "frm2200M:txtPg1Pt2MOPOtherDesc",
        ),
        (
            "2200P",
            "2200p-2020",
            [
                "rdoPg1Pt2MOPPaymentActual",
                "rdoPg1Pt2MOPPrepayment",
                "rdoPg1Pt2MOPOther",
            ],
            "frm2200P:txtPg1Pt2MOPOtherDesc",
        ),
    ];
    for (code, slug, ticks, specify) in forms {
        let prefix = match *code {
            "2200AN" => "frm2200ANv2018:".to_string(),
            "2200A" => "frm2200Av2020:".to_string(),
            "2200T" => "frm2200Tv2020:".to_string(),
            other => format!("frm{other}:"),
        };
        let captions = printed_captions(slug);
        for (i, tick) in ticks.iter().enumerate() {
            let key = format!("{prefix}{tick}");
            let at = placed(code, &key);
            let caption = &captions[&key];
            let (number, text) = caption.split_once(' ').unwrap();
            assert_eq!(at.no, number, "{code}: {key}");
            assert_eq!(at.item, "", "{code}: the item does not repeat its box");
            assert_eq!(at.label, text, "{code}: {key} reads as printed");
            if i == 2 {
                let other = placed(code, specify);
                assert_eq!(other.no, number, "{code}: specify sits with its tick box");
                assert_eq!(other.label, "Specify");
            }
        }
    }
}

/// 2200A manual-1/R3.0, 2200M manual-0/R3.0, 2200P manual-0/R3.0: items 10
/// and 11 are their printed items with Region, Province and City.
#[test]
fn pr11_places_of_production_and_removal_are_items_10_and_11() {
    for (code, keys) in [
        (
            "2200A",
            [
                "frm2200Av2020:prodRegion",
                "frm2200Av2020:prodProvince",
                "frm2200Av2020:prodCity",
                "frm2200Av2020:remRegion",
                "frm2200Av2020:remProvince",
                "frm2200Av2020:remCity",
            ],
        ),
        (
            "2200M",
            [
                "frm2200M:txtPg1Pt1I10Region",
                "frm2200M:txtPg1Pt1I10Province",
                "frm2200M:txtPg1Pt1I10City",
                "frm2200M:txtPg1Pt1I11Region",
                "frm2200M:txtPg1Pt1I11Province",
                "frm2200M:txtPg1Pt1I11City",
            ],
        ),
        (
            "2200P",
            [
                "frm2200P:txtPg1Pt1I10Region",
                "frm2200P:txtPg1Pt1I10Province",
                "frm2200P:txtPg1Pt1I10City",
                "frm2200P:txtPg1Pt1I11Region",
                "frm2200P:txtPg1Pt1I11Province",
                "frm2200P:txtPg1Pt1I11City",
            ],
        ),
    ] {
        for (i, key) in keys.iter().enumerate() {
            let at = placed(code, key);
            assert_eq!(at.no, if i < 3 { "10" } else { "11" }, "{code}: {key}");
            assert_eq!(
                at.item,
                if i < 3 {
                    "Place of Production"
                } else {
                    "Place of Removal"
                }
            );
            assert_eq!(
                at.label,
                ["Region", "Province", "City"][i % 3],
                "{code}: {key}"
            );
        }
    }
    assert!(sheet_text("2200a-2020").contains(
        "10 Place of Production Region Province City 11 Place of Removal Region Province City"
    ));
    // 2200P manual-0: items 8 and 9 too.
    assert_eq!(placed("2200P", "frm2200P:txtPg1Pt1I8ContactNumber").no, "8");
    assert_eq!(placed("2200P", "frm2200P:txtPg1Pt1I9Email").no, "9");
}

/// 2200A manual-2, 2200T manual-3, 2200P R3.3, 2200C manual-4, 2200M
/// manual-4: headings and box labels read as printed: item 1's Month, Day
/// and Year, item 7's whole caption, item 12's whole question, and 2200T's
/// item 6, 8 and 25 captions.
#[test]
fn pr11_labels_read_as_printed() {
    for (code, slug, keys) in [
        (
            "2200A",
            "2200a-2020",
            [
                "frm2200Av2020:txtDateMonth",
                "frm2200Av2020:txtDateDay",
                "frm2200Av2020:txtDateYear",
            ],
        ),
        (
            "2200T",
            "2200t-2022",
            [
                "frm2200Tv2020:txtDateMonth",
                "frm2200Tv2020:txtDateDay",
                "frm2200Tv2020:txtDateYear",
            ],
        ),
        (
            "2200S",
            "2200s-2018",
            [
                "frm2200S:txtMonth1",
                "frm2200S:txtDate",
                "frm2200S:txtForYr",
            ],
        ),
    ] {
        let captions = printed_captions(slug);
        for (key, label) in keys.iter().zip(["Month", "Day", "Year"]) {
            assert_eq!(placed(code, key).label, label, "{code}");
            assert!(
                captions[*key].ends_with(&label.to_lowercase()),
                "{code}: {key}"
            );
        }
    }
    for (code, slug, key) in [
        ("2200A", "2200a-2020", "frm2200Av2020:registeredAddress"),
        (
            "2200P",
            "2200p-2020",
            "frm2200P:txtPg1Pt1I7RegisteredAddress",
        ),
        (
            "2200M",
            "2200m-2018",
            "frm2200M:txtPg1Pt1I7RegisteredAddress",
        ),
        ("2200T", "2200t-2022", "frm2200Tv2020:registeredAddress"),
    ] {
        let at = placed(code, key);
        assert_eq!(at.no, "7");
        assert!(
            at.item.ends_with("by using BIR Form No. 1905)"),
            "{code}: {}",
            at.item
        );
        assert!(
            sheet_text(slug).contains(&at.item.replace('\u{2019}', "'")),
            "{code}: printed"
        );
    }
    for code in ["2200A", "2200C", "2200M", "2200P", "2200T"] {
        let question = &items(code, "12")[0];
        assert_eq!(
            question["l"],
            "Are you availing of tax relief under Special Law or International Tax Treaty?",
            "{code}"
        );
    }
    let t = |key: &str| placed("2200T", &format!("frm2200Tv2020:{key}"));
    assert_eq!(t("registeredName").label, "Taxpayer's Name");
    assert!(t("registeredName").item.starts_with("Taxpayer's Name"));
    assert_eq!(t("phoneNumber").label, "Contact Number");
    assert_eq!(
        t("phoneNumber").item,
        "Contact Number (Landline/Cellphone No.)"
    );
    assert_eq!(t("cashDate").label, "Date (MM/DD/YYYY)");
    let captions = printed_captions("2200t-2022");
    assert_eq!(
        captions["frm2200Tv2020:phoneNumber"],
        "8 Contact Number (Landline/Cellphone No.)"
    );
    // 2200M R3.3: the row switches are not drawn, so no key text shows.
    for d in drawn("2200M") {
        assert!(!d.label.starts_with("Box sched"), "{}", d.key);
    }
}

/// 2200S manual-1: item 14 and 14A show exactly what the draft holds; the
/// page's boxes and the print agree for every relief the draft can hold
/// (the screenshot showed a draft older than the export).
#[test]
fn pr11_2200s_item_14_shows_what_the_draft_holds() {
    setup();
    let opened = open("2200S", 1);
    for (relief, yes, no, list) in [
        ("no", "false", "true", "0"),
        ("yes_unspecified", "true", "false", "0"),
        ("special_law", "true", "false", "1"),
        ("international_tax_treaty", "true", "false", "2"),
    ] {
        let mut draft = opened["draft"].clone();
        draft["tax_relief"] = json!(relief);
        let filled = fill("2200S", &draft, json!({}));
        let values = &filled["values"];
        assert_eq!(values["frm2200S:optTreaty_1"], yes, "{relief}");
        assert_eq!(values["frm2200S:optTreaty_2"], no, "{relief}");
        assert_eq!(values["frm2200S:lstTaxTreaty"], list, "{relief}");
        let print = printed("2200S", &filled["draft"]);
        assert_eq!(print["frm2200S:optTreaty_1"], yes, "{relief}");
        assert_eq!(print["frm2200S:lstTaxTreaty"], list, "{relief}");
    }
}

/// 2200S: leaving a prepayment takes Items 23A-23C off the return; with
/// penalties entered there the page asks first (the fill's `removes`), and
/// without any it does not.
#[test]
fn pr11_2200s_leaving_prepayment_asks_before_it_removes_entered_penalties() {
    setup();
    let keys = [
        "frm2200S:txtTax21A",
        "frm2200S:txtTax21B",
        "frm2200S:txtTax21C",
    ];
    let opened = open("2200S", 1);
    let prepaid = fill(
        "2200S",
        &opened["draft"],
        json!({"frm2200S:chkPymntManner_2": "true"}),
    );
    assert!(prepaid["removes"].is_null());
    let entered = fill_unlocked(
        "2200S",
        &prepaid["draft"],
        json!({"frm2200S:txtTax21A": "10", "frm2200S:txtTax21C": "30"}),
        &keys,
    );
    assert_eq!(entered["rejected"], json!({}));
    let actual = fill(
        "2200S",
        &entered["draft"],
        json!({"frm2200S:chkPymntManner_1": "true"}),
    );
    let removes = &actual["removes"];
    assert_eq!(
        removes["rows"], 2,
        "23A and 23C, the two typed (only a typed box overrides)"
    );
    assert_eq!(
        removes["message"],
        "This answer removes what you typed in items 23A-23C. Continue?"
    );
    assert!(actual["draft"]["penalty_overrides"]["surcharge"].is_null());
    // Nothing entered: nothing asked.
    let plain = fill(
        "2200S",
        &prepaid["draft"],
        json!({"frm2200S:chkPymntManner_1": "true"}),
    );
    assert!(plain["removes"].is_null());
}

/// Decision D6: every rewrite in rows_excise::PRINTED_MESSAGES is BIR's own
/// message (verbatim in the form's bir-core source), and its rewrite names
/// the item number the sheet prints for that box (writer-cells.json);
/// errors_json shows the rewrite and keeps the field.
#[test]
fn pr11_messages_name_the_printed_item_numbers() {
    // (form, official message, the box it is about) by hand from the
    // form's validate_on.
    let boxes: &[(&str, &str, &str)] = &[
        ("2200AN", "TIN number on Item 5", "frm2200ANv2018:txtTIN1"),
        ("2200AN", "RDO Code on Item 6", "frm2200ANv2018:txtRDOCode"),
        (
            "2200AN",
            "Taxpayer Name on Item 8",
            "frm2200ANv2018:txtTaxpayerName",
        ),
        (
            "2200AN",
            "Registered Address on Item 10",
            "frm2200ANv2018:txtAddress",
        ),
        ("2200AN", "Zip Code on Item 11", "frm2200ANv2018:txtZipCode"),
        (
            "2200AN",
            "Telephone Number on Item 9",
            "frm2200ANv2018:txtTelNum",
        ),
        ("2200M", "RDO Code on Page 1 Item 6", "frm2200M:txtPg1I5RDO"),
        (
            "2200M",
            "Name field on Page 1 Item 7",
            "frm2200M:txtPg1Pt1I6RegisteredName",
        ),
        (
            "2200M",
            "Address field on Page 1 Item 8",
            "frm2200M:txtPg1Pt1I7RegisteredAddress",
        ),
        (
            "2200M",
            "Zip Code field on Page 1 Item 8A",
            "frm2200M:txPg1I7ZipCode",
        ),
        (
            "2200M",
            "Contact Number field on Page 1 Item 9",
            "frm2200M:txtPg1Pt1I8ContactNumber",
        ),
        (
            "2200M",
            "E-mail address on page 1 item 10 is",
            "frm2200M:txtPg1Pt1I9Email",
        ),
        (
            "2200M",
            "e-mail address on page 1 item 10",
            "frm2200M:txtPg1Pt1I9Email",
        ),
        ("2200P", "RDO Code on Page 1 Item 6", "frm2200P:txtPg1I5RDO"),
        (
            "2200P",
            "Name field on Page 1 Item 7",
            "frm2200P:txtPg1Pt1I6RegisteredName",
        ),
        (
            "2200P",
            "Address field on Page 1 Item 8",
            "frm2200P:txtPg1Pt1I7RegisteredAddress",
        ),
        (
            "2200P",
            "Zip Code field on Page 1 Item 8A",
            "frm2200P:txPg1I7ZipCode",
        ),
        (
            "2200P",
            "Contact Number field on Page 1 Item 9",
            "frm2200P:txtPg1Pt1I8ContactNumber",
        ),
        (
            "2200P",
            "E-mail address on page 1 item 10 is",
            "frm2200P:txtPg1Pt1I9Email",
        ),
        (
            "2200P",
            "e-mail address on page 1 item 10",
            "frm2200P:txtPg1Pt1I9Email",
        ),
    ];
    let numbers_of = |code: &str, key: &str| -> String {
        // The printed item the box sits under on the sheet.
        placed(code, key).no
    };
    assert_eq!(rows_excise::PRINTED_MESSAGES.len(), boxes.len());
    for (form, official, printed_text) in rows_excise::PRINTED_MESSAGES {
        let source = repo_file(&format!(
            "crates/bir-core/src/forms/form_{}.rs",
            form.to_lowercase()
        ));
        assert!(
            source.contains(&format!("\"{official}\"")),
            "{form}: {official:?} is BIR's message"
        );
        let (_, _, key) = boxes
            .iter()
            .find(|(f, part, _)| f == form && official.contains(part))
            .unwrap_or_else(|| panic!("{form}: {official:?} has a box"));
        let no = numbers_of(form, key);
        let slug = FORMS.iter().find(|(c, _, _)| c == form).unwrap().1;
        match printed_captions(slug).get(*key) {
            Some(caption) if printed_number(caption).is_some() => {
                assert!(
                    caption.starts_with(&format!("{no} ")),
                    "{form}: {key} prints at {caption:?}"
                );
            }
            caption => {
                // A caption without its number (the TIN), or a box the print
                // shapes (contact and email): the sheet prints the number
                // before the item's words.
                let text = caption.cloned().unwrap_or_else(|| placed(form, key).item);
                let words = text.split([',', '(']).next().unwrap().trim().to_string();
                assert!(
                    sheet_text(slug).contains(&format!("{no} {words}")),
                    "{form}: {no} {words}"
                );
            }
        }
        assert!(
            printed_text.contains(&format!("Item {no}"))
                || printed_text.contains(&format!("item {no}")),
            "{form}: {printed_text:?} names item {no}"
        );
        assert_ne!(
            official, printed_text,
            "{form}: a rewrite changes the number"
        );
        let kind = kind_of(form).unwrap();
        let shown = errors_json(kind, &[("rdo_code".into(), official.to_string())]);
        assert_eq!(shown[0]["message"], *printed_text, "{form}");
        assert_eq!(shown[0]["field"], "rdo_code");
    }
    // Other forms' messages are untouched.
    let shown = errors_json(
        kind_of("2200A").unwrap(),
        &[(
            "rdo_code".into(),
            "Please enter a valid RDO Code on Page 1 Item 6.".into(),
        )],
    );
    assert_eq!(
        shown[0]["message"],
        "Please enter a valid RDO Code on Page 1 Item 6."
    );
}

/// D6 end to end: a 2200M return whose RDO is blank says so at item 5, the
/// number the sheet prints, and the check itself is BIR's.
#[test]
fn pr11_a_2200m_message_names_item_5() {
    setup();
    let opened = open("2200M", 1);
    let mut draft = opened["draft"].clone();
    draft["rdo_code"] = json!("");
    let checked = call_async("screen_form_validate", vec!["2200M".into(), json(draft)]).unwrap();
    let messages: Vec<&str> = checked["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert!(
        messages.contains(&"Please enter a valid RDO Code on Page 1 Item 5."),
        "{messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("Item 6.")),
        "{messages:?}"
    );
}

// ---------------------------------------------------------------- D8

/// Days from `from` to `to`.
fn days(from: chrono::NaiveDate, to: chrono::NaiveDate) -> f64 {
    (to - from).num_days() as f64
}

/// D8: a new 2200A return's penalties run from Item 1, the removal date.
/// Opened for January 2026 (Item 1 01/31/2026) and filed today, it is late:
/// the 25% surcharge, 12% a year interest and the RMO 7-2015 compromise on
/// the tax still due (the oracles are those rates and the core's compromise
/// table), locked as computed with one Edit. The same return dated today is
/// on time and owes none.
#[test]
fn pr11_a_new_2200a_return_computes_its_penalties_from_the_removal_date() {
    use bir_core::penalties::PenaltyProfile;
    use bir_core::penalties::compromise::lookup_compromise;
    setup();
    let p = "frm2200Av2020:";
    let opened = open("2200A", 1);
    assert_eq!(
        opened["draft"]["penalty_model_version"], 2,
        "a new return is on D8"
    );
    let removal = chrono::NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
    assert_eq!(
        (
            opened["draft"]["month"].as_u64(),
            opened["draft"]["day"].as_u64()
        ),
        (Some(1), Some(31))
    );
    // Specific tax of 66.00 per proof liter on 100 proof liters.
    let filled = fill(
        "2200A",
        &opened["draft"],
        json!({ format!("{p}txtSched1_1B_2024_Taxable"): "100" }),
    );
    assert_eq!(filled["rejected"], json!({}));
    let values = &filled["values"];
    let tax = amount(values, &format!("{p}txtStillDue"));
    assert_eq!(tax, 6_600.0);
    let today = bir_core::forms::penalty_items::today();
    let late = days(removal, today);
    assert!(late > 0.0);
    assert_eq!(
        amount(values, &format!("{p}txtPen_Surcharge")),
        cents(tax * 0.25)
    );
    assert_eq!(
        amount(values, &format!("{p}txtPen_Interest")),
        cents(tax * 0.12 * late / 365.0)
    );
    assert_eq!(
        amount(values, &format!("{p}txtPen_Compromise")),
        cents(lookup_compromise(tax, 0.0, PenaltyProfile::StandardFiling))
    );
    let group = &filled["penalties"][0];
    assert_eq!(
        (group["edited"].clone(), group["kept"].clone()),
        (json!(false), json!(false))
    );
    for key in ["txtPen_Surcharge", "txtPen_Interest", "txtPen_Compromise"] {
        assert_eq!(filled["relocks"][format!("{p}{key}")], penalties::LOCK);
        assert_eq!(opened["locks"][format!("{p}{key}")], penalties::LOCK);
    }
    // Dated today (the day box is the filer's), the removal is on time.
    let mut on_time = filled["draft"].clone();
    on_time["month"] = json!(chrono::Datelike::month(&today));
    on_time["year"] = json!(chrono::Datelike::year(&today));
    let refilled = fill(
        "2200A",
        &on_time,
        json!({ format!("{p}txtDateDay"): chrono::Datelike::day(&today).to_string() }),
    );
    assert_eq!(refilled["rejected"], json!({}));
    for key in [
        "txtPen_Surcharge",
        "txtPen_Interest",
        "txtPen_Compromise",
        "txtPen_Tot",
    ] {
        assert_eq!(
            amount(&refilled["values"], &format!("{p}{key}")),
            0.0,
            "{key}"
        );
    }
}

/// D8: 2200C is monthly, due the 10th of the month after Item 1's month.
/// A new return opened from the page measures its penalties from that day
/// (the engine's own record of its inputs), whatever month Item 1 holds;
/// one dated in December is due January 10 of the next year, and a 10th on
/// a weekend moves to the next business day as every calendar deadline does.
#[test]
fn pr11_2200c_is_due_the_10th_of_the_next_month() {
    use bir_core::forms::form_2200c::Form2200CDraft;
    setup();
    let opened = open("2200C", 1);
    let draft = &opened["draft"];
    assert_eq!(draft["penalty_model_version"], 2, "a new return is on D8");
    let (year, month) = (
        draft["year"].as_i64().unwrap() as i32,
        draft["month"].as_u64().unwrap() as u32,
    );
    let (due_year, due_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    // The 10th, or the Monday after when it falls on a weekend.
    let tenth = chrono::NaiveDate::from_ymd_opt(due_year, due_month, 10).unwrap();
    let due = match chrono::Datelike::weekday(&tenth) {
        chrono::Weekday::Sat => tenth + chrono::Duration::days(2),
        chrono::Weekday::Sun => tenth + chrono::Duration::days(1),
        _ => tenth,
    };
    assert_eq!(
        draft["computed_penalties"]["inputs"]["due_date"],
        json!(due.format("%Y-%m-%d").to_string()),
        "measured from the 10th after Item 1's month"
    );
    let typed: Form2200CDraft = serde_json::from_value(draft.clone()).unwrap();
    let mut december = typed.clone();
    december.year = 2026;
    december.month = 12;
    december.day = 31;
    assert_eq!(
        Form2200CDraft::penalty_declaration().due_date(&december),
        // January 10, 2027 is a Sunday.
        chrono::NaiveDate::from_ymd_opt(2027, 1, 11)
    );
    let mut december_2025 = typed.clone();
    december_2025.year = 2025;
    december_2025.month = 12;
    december_2025.day = 31;
    assert_eq!(
        Form2200CDraft::penalty_declaration().due_date(&december_2025),
        chrono::NaiveDate::from_ymd_opt(2026, 1, 12),
        "Saturday 2026-01-10 moves to Monday"
    );
}

/// `(code, fixture)`: drafts saved by the pr0b-push code (crates/bir-core
/// at ec80b306, PR 11's scratch example, profile of `install_profile`),
/// each holding typed penalties as overrides (125.25, 0.00, 1,000.00), and
/// one 2200A holding none.
const STORED: &[(&str, &str)] = &[
    (
        "2200A",
        include_str!("fixtures/excise-2200a-kept-pr0b.json"),
    ),
    (
        "2200A",
        include_str!("fixtures/excise-2200a-untyped-pr0b.json"),
    ),
    (
        "2200AN",
        include_str!("fixtures/excise-2200an-kept-pr0b.json"),
    ),
    (
        "2200C",
        include_str!("fixtures/excise-2200c-kept-pr0b.json"),
    ),
    (
        "2200M",
        include_str!("fixtures/excise-2200m-kept-pr0b.json"),
    ),
    (
        "2200P",
        include_str!("fixtures/excise-2200p-kept-pr0b.json"),
    ),
    (
        "2200S",
        include_str!("fixtures/excise-2200s-kept-pr0b.json"),
    ),
    (
        "2200T",
        include_str!("fixtures/excise-2200t-kept-pr0b.json"),
    ),
];

/// Store a literal fixture as the app would have, by its own typed draft.
fn store_fixture(code: &str, fixture: &str) {
    let AgentFormKind::Queueable(kind) = kind_of(code).unwrap() else {
        unreachable!()
    };
    bir_core::with_queueable_kind!(kind, F => {
        let typed: F = serde_json::from_str(fixture).unwrap();
        assert_eq!(serde_json::to_string(&typed).unwrap(), fixture, "{code}: reads and writes back as stored");
        let id = with_database(|db| db.save_queueable_draft(&typed).map_err(|e| HostError::new(e.to_string()))).unwrap();
        assert_eq!(Some(id), typed.draft_id(), "{code}: the fixture's own row id");
    });
}

/// The stored JSON of `code`'s return for January 2026.
fn stored_json(code: &str, draft: &Value) -> String {
    let kind = kind_of(code).unwrap();
    let AgentFormKind::Queueable(queueable) = kind else {
        unreachable!()
    };
    let period = stored_period(kind, draft).unwrap();
    bir_core::with_queueable_kind!(queueable, F => {
        let stored = with_database(|db| {
            db.get_queueable_draft::<F>(TIN, 2026, period).map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
        .unwrap();
        serde_json::to_string(&stored).unwrap()
    })
}

/// Lesson 6 and D8: a draft saved before D8 opens and saves back byte for
/// byte (nothing is computed on open), and its typed penalties show as it
/// holds them.
#[test]
fn pr11_a_stored_excise_return_saves_back_byte_for_byte() {
    for (code, fixture) in STORED {
        setup();
        let fixture = fixture.trim_end();
        store_fixture(code, fixture);
        let opened = open(code, 1);
        assert_eq!(opened["draft"]["id"], 1, "{code}: the stored return opened");
        let stored: Value = serde_json::from_str(fixture).unwrap();
        if stored["penalty_overrides"]["surcharge"] == 125.25 {
            let group = &opened["penalties"][0];
            assert_eq!(group["edited"], true, "{code}: the typed amounts are open");
            assert_eq!(group["kept"], true, "{code}: kept as entered");
            let keys = strings(&group["keys"]);
            let shown: Vec<f64> = keys.iter().map(|k| amount(&opened["values"], k)).collect();
            let mut sorted = shown.clone();
            sorted.sort_by(f64::total_cmp);
            assert_eq!(sorted, [0.0, 125.25, 1_000.0], "{code}: {keys:?}");
        }
        save(code, &opened["draft"]);
        // A save records the profile baseline (PR 0c), the row's last key;
        // set aside, the row is the old bytes exactly.
        let saved = stored_json(code, &opened["draft"]);
        let saved = match saved.find(",\"profile_baseline\":") {
            Some(at) => {
                let baseline: Value =
                    serde_json::from_str(&saved[at + 20..saved.len() - 1]).unwrap();
                assert!(baseline.is_object(), "{code}: the baseline is the last key");
                format!("{}}}", &saved[..at])
            }
            None => saved,
        };
        assert_eq!(saved, fixture, "{code}: saved back byte for byte");
    }
}

/// D8 on a stored 2200A holding typed penalties: the amounts stay through
/// other edits (kept until the filer edits them), and "Use the computed
/// amounts" (blanks) puts the return on the typed due date at once: the
/// computed amounts from Item 1 show, and the boxes lock as computed
/// (`relocks`).
#[test]
fn pr11_typed_penalties_stay_until_the_filer_uses_the_computed_amounts() {
    use bir_core::penalties::PenaltyProfile;
    use bir_core::penalties::compromise::lookup_compromise;
    setup();
    let p = "frm2200Av2020:";
    store_fixture("2200A", STORED[0].1.trim_end());
    let opened = open("2200A", 1);
    let keys =
        ["txtPen_Surcharge", "txtPen_Interest", "txtPen_Compromise"].map(|k| format!("{p}{k}"));
    let other = fill(
        "2200A",
        &opened["draft"],
        json!({ format!("{p}txtLess_Balance"): "5" }),
    );
    assert_eq!(other["rejected"], json!({}));
    assert_eq!(
        other["draft"]["penalty_model_version"], 1,
        "an unrelated edit does not join"
    );
    assert_eq!(amount(&other["values"], &keys[0]), 125.25);
    assert_eq!(amount(&other["values"], &keys[2]), 1_000.0);
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let blanks: Map<String, Value> = keys.iter().map(|k| (k.clone(), json!(""))).collect();
    let computed = fill_unlocked("2200A", &other["draft"], Value::Object(blanks), &refs);
    assert_eq!(computed["rejected"], json!({}));
    assert_eq!(computed["draft"]["penalty_model_version"], 2, "joined D8");
    let tax = amount(&computed["values"], &format!("{p}txtStillDue"));
    assert!(tax > 0.0);
    let removal = chrono::NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();
    let late = days(removal, bir_core::forms::penalty_items::today());
    assert_eq!(amount(&computed["values"], &keys[0]), cents(tax * 0.25));
    assert_eq!(
        amount(&computed["values"], &keys[1]),
        cents(tax * 0.12 * late / 365.0)
    );
    assert_eq!(
        amount(&computed["values"], &keys[2]),
        cents(lookup_compromise(tax, 0.0, PenaltyProfile::StandardFiling))
    );
    for key in &keys {
        assert_eq!(computed["relocks"][key], penalties::LOCK, "{key}");
        assert_eq!(computed["relock_notes"][key], NOTE_COMPUTED, "{key}");
    }
    assert_eq!(computed["penalties"][0]["kept"], false);
}

/// D8 on a stored 2200A holding no typed penalties: its 0.00 are kept as
/// stored ("Kept as entered before penalties were computed."), refused
/// without Edit with that reason, and "Use the computed amounts" after Edit
/// joins the typed due date.
#[test]
fn pr11_an_untyped_stored_return_is_kept_as_entered_until_edit() {
    setup();
    let p = "frm2200Av2020:";
    store_fixture("2200A", STORED[1].1.trim_end());
    let opened = open("2200A", 1);
    let keys =
        ["txtPen_Surcharge", "txtPen_Interest", "txtPen_Compromise"].map(|k| format!("{p}{k}"));
    for key in &keys {
        assert_eq!(opened["locks"][key], rows_excise::KEPT_AS_ENTERED, "{key}");
        assert_eq!(
            opened["lock_notes"][key],
            "Kept as entered before penalties were computed."
        );
        assert_eq!(amount(&opened["values"], key), 0.0);
    }
    let group = &opened["penalties"][0];
    assert_eq!(
        (group["edited"].clone(), group["kept"].clone()),
        (json!(false), json!(true))
    );
    let refused = fill("2200A", &opened["draft"], json!({ &keys[0]: "5" }));
    assert_eq!(
        refused["rejected"][&keys[0]],
        format!("`{}` is {}", keys[0], rows_excise::KEPT_AS_ENTERED)
    );
    // With a tax due, an unrelated edit keeps the stored 0.00.
    let taxed = fill(
        "2200A",
        &opened["draft"],
        json!({ format!("{p}txtSched1_1B_2024_Taxable"): "100" }),
    );
    assert_eq!(taxed["draft"]["penalty_model_version"], 1);
    assert_eq!(amount(&taxed["values"], &keys[0]), 0.0, "kept until edited");
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let blanks: Map<String, Value> = keys.iter().map(|k| (k.clone(), json!(""))).collect();
    let joined = fill_unlocked("2200A", &taxed["draft"], Value::Object(blanks), &refs);
    assert_eq!(joined["rejected"], json!({}));
    assert_eq!(joined["draft"]["penalty_model_version"], 2);
    assert_eq!(
        amount(&joined["values"], &keys[0]),
        1_650.0,
        "25% of 6,600.00"
    );
    assert_eq!(joined["relocks"][&keys[0]], penalties::LOCK);
}

// ------------------------------------------------- PR 11 review: gates

/// The draft path a box writes (its binding, from the probed agent data).
fn binding_path(code: &str, key: &str) -> String {
    match &kind_of(code).unwrap().definition().bindings[key].binding {
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. } => {
            path.clone()
        }
        other => panic!("{code}: {key} is {other:?}"),
    }
}

/// Whether the draft holds something at `path` (a text, a non-zero amount).
fn holds(draft: &Value, path: &str) -> bool {
    match draft.pointer(path) {
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Number(number)) => number.as_f64().is_some_and(|n| n != 0.0),
        _ => false,
    }
}

/// Lesson 15 and 8 on every box an answer of the excise returns erases
/// (form_2200*.rs `recompute`). The oracle is the form's own computation:
/// the value written straight into the draft at the box's binding path is
/// cleared by `compute` while the answer does not hold. Then, on the page:
/// the box is off with what turns it on, a typed value is refused with that
/// reason (never taken and lost), the same value is kept once the answer
/// holds (in one batch, the answer first), and the answer that turns it
/// off again asks first (`removes`) naming the printed item.
#[test]
fn pr11_a_box_an_answer_erases_is_shut_refused_and_asked_about() {
    setup();
    let cases: Vec<(&str, &str, &str, Value, Value, &str, &str)> = vec![
        (
            "2200A",
            "frm2200Av2020:treatyY",
            "Sample relief",
            json!({"frm2200Av2020:optTreaty_1": "true"}),
            json!({"frm2200Av2020:optTreaty_2": "true"}),
            "item 12A",
            "Needs item 12 = Yes",
        ),
        (
            "2200A",
            "frm2200Av2020:paymentOther",
            "Sample scheme",
            json!({"frm2200Av2020:optPayment_3": "true"}),
            json!({"frm2200Av2020:optPayment_1": "true"}),
            "item 15",
            "Needs item 15 = Other Similar Schemes",
        ),
        (
            "2200A",
            "frm2200Av2020:txtPrevReturn",
            "100",
            json!({"frm2200Av2020:amendedRtn_1": "true"}),
            json!({"frm2200Av2020:amendedRtn_2": "true"}),
            "item 19",
            "Needs Amended Return = Yes",
        ),
        (
            "2200T",
            "frm2200Tv2020:treatyY",
            "Sample relief",
            json!({"frm2200Tv2020:optTreaty_1": "true"}),
            json!({"frm2200Tv2020:optTreaty_2": "true"}),
            "item 12A",
            "Needs item 12 = Yes",
        ),
        (
            "2200T",
            "frm2200Tv2020:paymentOther",
            "Sample scheme",
            json!({"frm2200Tv2020:optPayment_3": "true"}),
            json!({"frm2200Tv2020:optPayment_1": "true"}),
            "item 15",
            "Needs item 15 = Other Similar Schemes",
        ),
        (
            "2200C",
            "frm2200C:txtPg1I12TaxReliefSpecify",
            "Sample relief",
            json!({"frm2200C:rdoPg1I12TaxReliefYes": "true"}),
            json!({"frm2200C:rdoPg1I12TaxReliefNo": "true"}),
            "item 12A",
            "Needs item 12 = Yes",
        ),
        (
            "2200C",
            "frm2200C:txtPg1Pt2MOPOtherDesc",
            "Sample scheme",
            json!({"frm2200C:rdoPg1Pt2MOPOther": "true"}),
            json!({"frm2200C:rdoPg1Pt2MOPPaymentActual": "true"}),
            "item 15",
            "Needs item 15 = Other Similar Schemes",
        ),
        (
            "2200C",
            "frm2200C:CPPexmpt1",
            "5",
            json!({"frm2200C:rdoPg1Pt2MOPPaymentActual": "true"}),
            json!({"frm2200C:rdoPg1Pt2MOPPrepayment": "true"}),
            "Part V Schedule 1 (1 row)",
            "Needs item 13 or item 15",
        ),
        (
            "2200S",
            "frm2200S:txtOthMannerofPymnt",
            "Sample scheme",
            json!({"frm2200S:chkPymntManner_2": "true", "frm2200S:chkPymntManner_3": "true"}),
            json!({"frm2200S:chkPymntManner_3": "false"}),
            "item 17",
            "Needs item 16 = Prepayment and item 17 = Other Similar Schemes",
        ),
        (
            "2200S",
            "frm2200S:txtSalesValue0",
            "100",
            json!({"frm2200S:chkPymntManner_1": "true"}),
            json!({"frm2200S:chkPymntManner_2": "true"}),
            "Schedule 1 (1 row)",
            "Needs item 15 = Payment on Actual Removal",
        ),
        (
            "2200AN",
            "frm2200ANv2018:MannerOfPaymentOthers",
            "Sample scheme",
            json!({"frm2200ANv2018:MannerOfPayment3": "true"}),
            json!({"frm2200ANv2018:MannerOfPayment1": "true"}),
            "item 15",
            "Needs item 15 = Other Similar Schemes",
        ),
        (
            "2200M",
            "frm2200M:txtPg1I12TaxReliefSpecify",
            "Sample relief",
            json!({"frm2200M:rdoPg1I12TaxReliefYes": "true"}),
            json!({"frm2200M:rdoPg1I12TaxReliefNo": "true"}),
            "item 12A",
            "Needs item 12 = Yes",
        ),
    ];
    for (code, key, value, open_it, close_it, item, tag) in &cases {
        let kind = kind_of(code).unwrap();
        let opened = open(code, 1);
        // The oracle: the form clears it while shut.
        let path = binding_path(code, key);
        let mut direct = opened["draft"].clone();
        let typed = if value.chars().all(|c| c.is_ascii_digit()) {
            json!(value.parse::<f64>().unwrap())
        } else {
            json!(value)
        };
        set_draft_path(&mut direct, &path, typed);
        agent::compute(kind, &mut direct).unwrap();
        assert!(
            !holds(&direct, &path),
            "{code}: the form clears {key} while shut"
        );
        // Off, and refused with the reason.
        assert_eq!(opened["off"][*key], *tag, "{code}: {key} is off");
        let refused = fill(code, &opened["draft"], json!({ *key: value }));
        assert_eq!(refused["rejected"][*key], *tag, "{code}: {key} refused");
        assert!(!holds(&refused["draft"], &path));
        // Accepted once open, the answer in the same batch.
        let mut both = open_it.as_object().unwrap().clone();
        both.insert(key.to_string(), json!(value));
        let taken = fill(code, &opened["draft"], Value::Object(both));
        assert_eq!(
            taken["rejected"],
            json!({}),
            "{code}: {key} taken once open"
        );
        assert!(holds(&taken["draft"], &path), "{code}: {key} kept");
        assert!(taken["removes"].is_null(), "{code}: {key}");
        // The answer that shuts it again asks first.
        let shut = fill(code, &taken["draft"], close_it.clone());
        assert_eq!(
            shut["removes"]["message"],
            format!("This answer removes what you typed in {item}. Continue?"),
            "{code}: {key}"
        );
        assert_eq!(shut["removes"]["rows"], 1, "{code}: {key}");
        assert!(!holds(&shut["draft"], &path));
        // A box the filer blanks themselves is not asked about.
        let blank = fill(code, &taken["draft"], json!({ *key: "" }));
        assert!(blank["removes"].is_null(), "{code}: {key}");
    }
}

/// 2200AN item 3: the official box takes two digits (maxlength 2), so the
/// host refuses more, and takes 99.
#[test]
fn pr11_2200an_item_3_holds_up_to_two_digits() {
    setup();
    let opened = open("2200AN", 1);
    let key = "frm2200ANv2018:txtSheets";
    let refused = fill("2200AN", &opened["draft"], json!({ key: "100" }));
    assert_eq!(refused["rejected"][key], "Item 3 holds up to two digits");
    assert!(refused["draft"].get("number_of_attached_sheets").is_none());
    let taken = fill("2200AN", &opened["draft"], json!({ key: "99" }));
    assert_eq!(taken["rejected"], json!({}));
    assert_eq!(taken["draft"]["number_of_attached_sheets"], 99);
}

/// Review item 6: "Use the computed amounts" replaces what the filer typed,
/// so it asks first, as 1701 does: on a stored pre-D8 return's kept
/// amounts (125.25 and 1,000.00; the 0.00 says nothing), and on a current
/// return's override.
#[test]
fn pr11_using_the_computed_amounts_asks_before_it_replaces_typed_penalties() {
    setup();
    let p = "frm2200Av2020:";
    let keys =
        ["txtPen_Surcharge", "txtPen_Interest", "txtPen_Compromise"].map(|k| format!("{p}{k}"));
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let blanks: Map<String, Value> = keys.iter().map(|k| (k.clone(), json!(""))).collect();
    store_fixture("2200A", STORED[0].1.trim_end());
    let kept = open("2200A", 1);
    let replaced = fill_unlocked(
        "2200A",
        &kept["draft"],
        Value::Object(blanks.clone()),
        &refs,
    );
    assert_eq!(
        replaced["removes"],
        json!({
            "message": "This answer replaces what you typed in item 21A and item 21C. Continue?",
            "done": "Removed 2 entries",
            "rows": 2,
        })
    );
    setup();
    let opened = open("2200A", 1);
    let typed = fill_unlocked("2200A", &opened["draft"], json!({ &keys[1]: "50" }), &refs);
    assert!(typed["removes"].is_null());
    let back = fill_unlocked("2200A", &typed["draft"], Value::Object(blanks), &refs);
    assert_eq!(
        back["removes"],
        json!({
            "message": "This answer replaces what you typed in item 21B. Continue?",
            "done": "Removed 1 entry",
            "rows": 1,
        })
    );
    // Blanks over computed amounts ask nothing.
    let computed = fill_unlocked("2200A", &back["draft"], json!({ &keys[0]: "" }), &refs);
    assert!(computed["removes"].is_null());
}

/// Review item 8: on a draft stored before D8, one blank box keeps the
/// stored model (that box files 0.00, as entered); only the whole group
/// blanked ("Use the computed amounts") joins the typed due date.
#[test]
fn pr11_one_blank_penalty_keeps_a_stored_return_on_its_model() {
    setup();
    let p = "frm2200Av2020:";
    store_fixture("2200A", STORED[0].1.trim_end());
    let opened = open("2200A", 1);
    let one = fill(
        "2200A",
        &opened["draft"],
        json!({ format!("{p}txtPen_Surcharge"): "" }),
    );
    assert_eq!(one["rejected"], json!({}));
    assert_eq!(one["draft"]["penalty_model_version"], 1, "still kept");
    assert_eq!(amount(&one["values"], &format!("{p}txtPen_Surcharge")), 0.0);
    assert_eq!(
        amount(&one["values"], &format!("{p}txtPen_Compromise")),
        1_000.0
    );
    assert!(one["draft"]["computed_penalties"].is_null());
}

/// Review item 9: a write to a penalty group that is not on the return
/// (2200S Items 23A-23C with Part II unanswered) is refused with that
/// reason, not "choose Edit penalties", Edit or not, a blank too.
#[test]
fn pr11_a_penalty_group_off_the_return_refuses_with_that_reason() {
    setup();
    let opened = open("2200S", 1);
    let keys = [
        "frm2200S:txtTax21A",
        "frm2200S:txtTax21B",
        "frm2200S:txtTax21C",
    ];
    for unlocked in [&[][..], &keys[..]] {
        let filled = fill_unlocked(
            "2200S",
            &opened["draft"],
            json!({ keys[0]: "10" }),
            unlocked,
        );
        assert_eq!(
            filled["rejected"][keys[0]],
            format!("`{}` is {}", keys[0], penalties::NOT_ON_RETURN)
        );
    }
    let blank = fill("2200S", &opened["draft"], json!({ keys[0]: "" }));
    assert_eq!(
        blank["rejected"][keys[0]],
        format!("`{}` is {}", keys[0], penalties::NOT_ON_RETURN)
    );
}

// ------------------------------------------------- PR 11 review, round 2

/// The answers that open each host gate (`rows_excise::gate_list`), by
/// form and the item the gate names.
fn gate_opener(code: &str, item: &str) -> Value {
    let p = match code {
        "2200A" => "frm2200Av2020:",
        "2200T" => "frm2200Tv2020:",
        "2200AN" => "frm2200ANv2018:",
        other => return gate_opener_2018(other, item),
    };
    match (code, item) {
        ("2200AN", "item 12A") => json!({ format!("{p}OptSpecialTax1"): "true" }),
        ("2200AN", "item 15") => json!({ format!("{p}MannerOfPayment3"): "true" }),
        ("2200AN", "item 19") => json!({ format!("{p}AmendedRtn1"): "true" }),
        (_, "item 12A") => json!({ format!("{p}optTreaty_1"): "true" }),
        (_, "item 15") => json!({ format!("{p}optPayment_3"): "true" }),
        (_, "item 19") => json!({ format!("{p}amendedRtn_1"): "true" }),
        _ => panic!("{code}: no opener for {item}"),
    }
}

fn gate_opener_2018(code: &str, item: &str) -> Value {
    let p = format!("frm{code}:");
    match (code, item) {
        ("2200S", "item 17") => json!({
            format!("{p}chkPymntManner_2"): "true",
            format!("{p}chkPymntManner_3"): "true",
        }),
        ("2200S", "item 21") => json!({ format!("{p}amendedRtn_1"): "true" }),
        ("2200S", "Schedule 1") => json!({ format!("{p}chkPymntManner_1"): "true" }),
        ("2200C", "Part V Schedule 1") => {
            json!({ format!("{p}rdoPg1Pt2MOPPaymentActual"): "true" })
        }
        (_, "item 12A") => json!({ format!("{p}rdoPg1I12TaxReliefYes"): "true" }),
        (_, "item 15") => json!({ format!("{p}rdoPg1Pt2MOPOther"): "true" }),
        (_, "item 19") => json!({ format!("{p}rdoPg1I2AmendedYes"): "true" }),
        _ => panic!("{code}: no opener for {item}"),
    }
}

/// Write `value` at the draft path of `key` straight into `draft`, adding
/// blank rows (the probe's row templates) up to the key's row. 2200S's
/// Others rate has no binding (the page shows it reformatted, catalogs.rs
/// writes it): its path is `/others/{n}/rate`.
fn write_at_binding(code: &str, draft: &mut Value, key: &str, value: Value) -> String {
    let def = kind_of(code).unwrap().definition();
    let path = match key.strip_prefix("frm2200S:txtsched1AppTaxRate") {
        Some(row) => format!("/others/{row}/rate"),
        None => binding_path(code, key),
    };
    for (list, template) in &def.rows {
        let Some(rest) = path.strip_prefix(&format!("{list}/")) else {
            continue;
        };
        let row: usize = rest.split('/').next().unwrap().parse().unwrap();
        let rows = draft.pointer_mut(list).unwrap().as_array_mut().unwrap();
        while rows.len() <= row {
            rows.push(template.clone());
        }
    }
    set_draft_path(draft, &path, value);
    assert!(
        draft.pointer(&path).is_some(),
        "{code}: {key} written at {path}"
    );
    path
}

/// What `key` shows after the form's own `compute` of `draft`.
fn computed_shows(code: &str, draft: &Value, key: &str) -> String {
    let kind = kind_of(code).unwrap();
    let mut draft = draft.clone();
    agent::compute(kind, &mut draft).unwrap();
    agent::field_map(kind, &draft)
        .unwrap()
        .get(key)
        .cloned()
        .unwrap_or_default()
}

/// Round 2, item 1: every key of every host gate (2200A, 2200AN, 2200C,
/// 2200M, 2200P, 2200S and 2200T; the penalty group aside), checked
/// against the form's own computation. A probe written straight at the
/// key's binding path (an amount 1, a text `X`) is cleared by `compute`
/// exactly while the host reports the gate shut, and kept once the
/// answers that open it hold, when the host no longer reports it off.
#[test]
fn pr11_every_gate_key_is_cleared_exactly_while_its_gate_is_shut() {
    setup();
    let mut checked = 0;
    for code in [
        "2200A", "2200AN", "2200C", "2200M", "2200P", "2200S", "2200T",
    ] {
        let kind = kind_of(code).unwrap();
        let shut = open(code, 1);
        for (item, tag, keys) in rows_excise::gate_list(code) {
            let opened = fill(code, &shut["draft"], gate_opener(code, item));
            assert_eq!(opened["rejected"], json!({}), "{code}: {item} opens");
            for key in &keys {
                let amount = matches!(
                    kind.definition().bindings.get(key).map(|b| &b.binding),
                    Some(Binding::Amount { .. } | Binding::Whole { .. }) | None
                );
                let probe = if amount { json!(1.0) } else { json!("X") };
                let expected = if amount { "1.00" } else { "X" };
                // Shut: the host says so, and the form clears the probe.
                assert_eq!(
                    shut["off"][key.as_str()]
                        .as_str()
                        .map(|why| why == tag || conditions::tag(code, key).as_deref() == Some(why)),
                    Some(true),
                    "{code}: {key} is off with {tag:?} while {item} is shut"
                );
                let mut direct = shut["draft"].clone();
                let path = write_at_binding(code, &mut direct, key, probe.clone());
                let shown = computed_shows(code, &direct, key);
                assert!(
                    shown.trim().is_empty() || agent::parse_amount(&shown) == Some(0.0),
                    "{code}: the form clears {key} ({path}) while {item} is shut: {shown:?}"
                );
                // Open: the host says so, and the form keeps the probe.
                assert!(
                    opened["off"].get(key.as_str()).is_none(),
                    "{code}: {key} is not off once {item} is open"
                );
                let mut direct = opened["draft"].clone();
                write_at_binding(code, &mut direct, key, probe);
                assert_eq!(
                    computed_shows(code, &direct, key),
                    expected,
                    "{code}: the form keeps {key} ({path}) once {item} is open"
                );
                checked += 1;
            }
        }
    }
    // 2200A, 2200T, 2200AN, 2200M, 2200P: 3 each; 2200C 3 + 70; 2200S 2 + 92.
    assert_eq!(checked, 15 + 73 + 94);
}

/// Round 2, item 2: 2200S item 17's text is kept only with item 17 ticked
/// on a prepayment (item 16), so its refusal names both: ticking item 17
/// with the manner unanswered does not open it.
#[test]
fn pr11_2200s_item_17_text_names_items_16_and_17() {
    setup();
    let opened = open("2200S", 1);
    let key = "frm2200S:txtOthMannerofPymnt";
    let tag = "Needs item 16 = Prepayment and item 17 = Other Similar Schemes";
    assert_eq!(opened["off"][key], tag);
    let refused = fill(
        "2200S",
        &opened["draft"],
        json!({ "frm2200S:chkPymntManner_3": "true", key: "Sample scheme" }),
    );
    assert_eq!(refused["rejected"][key], tag);
    assert_eq!(refused["draft"]["other_scheme_text"], "");
    assert_eq!(refused["draft"]["manner"], "unanswered");
}

/// Round 2, item 4: unticking the ticked member of an excise return's
/// three-way manner item leaves the item unanswered (the field's own
/// default), where the shared rule's `null` failed the fill ("not a 2200S
/// draft"). One key per form; 2200C's manner is a yes/no pair beside item
/// 13 in the probe, so its untick keeps the official flip and item 13's
/// untick leaves it unanswered.
#[test]
fn pr11_unticking_the_manner_of_payment_leaves_it_unanswered() {
    setup();
    let cases = [
        (
            "2200A",
            "frm2200Av2020:optPayment_1",
            "/manner_of_payment",
            "none",
        ),
        (
            "2200AN",
            "frm2200ANv2018:MannerOfPayment2",
            "/manner",
            "unanswered",
        ),
        (
            "2200C",
            "frm2200C:rdoPg1Pt2MOPPaymentActual",
            "/manner",
            "unanswered",
        ),
        (
            "2200M",
            "frm2200M:rdoPg1Pt2MOPPrepayment",
            "/manner_of_payment",
            "none",
        ),
        (
            "2200P",
            "frm2200P:rdoPg1Pt2MOPOther",
            "/manner_of_payment",
            "none",
        ),
        (
            "2200S",
            "frm2200S:chkPymntManner_1",
            "/manner",
            "unanswered",
        ),
        (
            "2200T",
            "frm2200Tv2020:optPayment_2",
            "/manner_of_payment",
            "none",
        ),
    ];
    for (code, key, path, unanswered) in cases {
        let opened = open(code, 1);
        assert_eq!(
            opened["draft"].pointer(path),
            Some(&json!(unanswered)),
            "{code}"
        );
        let ticked = fill(code, &opened["draft"], json!({ key: "true" }));
        assert_eq!(ticked["values"][key], "true", "{code}: {key} ticked");
        assert_ne!(
            ticked["draft"].pointer(path),
            Some(&json!(unanswered)),
            "{code}"
        );
        let unticked = fill(code, &ticked["draft"], json!({ key: "false" }));
        assert_eq!(unticked["rejected"], json!({}), "{code}: {key}");
        assert_eq!(unticked["values"][key], "false", "{code}: {key} unticked");
        assert_eq!(
            unticked["draft"].pointer(path),
            Some(&json!(unanswered)),
            "{code}: {key} leaves the item unanswered"
        );
    }
}

/// Round 2, item 5: an answer that clears a schedule asks about every row
/// the filer typed that the batch does not write itself: with three 2200S
/// Schedule 1 rows typed, blanking row 1 in the batch that leaves the
/// payment on actual removal asks about rows 2 and 3.
#[test]
fn pr11_a_cleared_schedule_asks_about_each_row_the_batch_does_not_write() {
    setup();
    let opened = open("2200S", 1);
    let typed = fill(
        "2200S",
        &opened["draft"],
        json!({
            "frm2200S:chkPymntManner_1": "true",
            "frm2200S:txtSalesValue0": "100",
            "frm2200S:txtSalesValue1": "200",
            "frm2200S:txtSalesValue2": "300",
        }),
    );
    assert_eq!(typed["rejected"], json!({}));
    let shut = fill(
        "2200S",
        &typed["draft"],
        json!({ "frm2200S:txtSalesValue0": "", "frm2200S:chkPymntManner_2": "true" }),
    );
    assert_eq!(
        shut["removes"],
        json!({
            "message": "This answer removes what you typed in Schedule 1 (2 rows). Continue?",
            "done": "Removed 2 entries",
            "rows": 2,
        })
    );
    // Writing every typed row itself asks nothing.
    let all = fill(
        "2200S",
        &typed["draft"],
        json!({
            "frm2200S:txtSalesValue0": "",
            "frm2200S:txtSalesValue1": "",
            "frm2200S:txtSalesValue2": "",
            "frm2200S:chkPymntManner_2": "true",
        }),
    );
    assert!(all["removes"].is_null());
}

/// Round 2, item 6: only a box's own empty value passes a shut gate
/// unasked: a blank, or on an amount box zero. A text such as `0`, `.` or
/// `-` is a value, so it is refused with the reason, and taken once open.
#[test]
fn pr11_a_shut_text_box_refuses_zero_with_the_reason() {
    setup();
    let opened = open("2200A", 1);
    let text = "frm2200Av2020:treatyY";
    for value in ["0", "0.00", ".", "-", ","] {
        let refused = fill("2200A", &opened["draft"], json!({ text: value }));
        assert_eq!(
            refused["rejected"][text], "Needs item 12 = Yes",
            "{value:?} is a value"
        );
    }
    let blank = fill("2200A", &opened["draft"], json!({ text: "" }));
    assert_eq!(blank["rejected"], json!({}));
    let taken = fill(
        "2200A",
        &opened["draft"],
        json!({ "frm2200Av2020:optTreaty_1": "true", text: "0" }),
    );
    assert_eq!(taken["rejected"], json!({}));
    assert_eq!(taken["draft"]["tax_relief_specify"], "0");
    // An amount box's zero is its empty value; any other amount refuses.
    let amount = "frm2200Av2020:txtPrevReturn";
    for value in ["0", "0.00"] {
        let zero = fill("2200A", &opened["draft"], json!({ amount: value }));
        assert_eq!(zero["rejected"], json!({}), "{value:?}");
    }
    let refused = fill("2200A", &opened["draft"], json!({ amount: "0.01" }));
    assert_eq!(refused["rejected"][amount], "Needs Amended Return = Yes");
}

/// Round 2, item 7: joining the typed due date changes the draft's model
/// version alone; a key the typed draft does not know survives.
#[test]
fn pr11_joining_the_typed_due_date_keeps_every_other_key() {
    setup();
    let kind = kind_of("2200A").unwrap();
    let mut draft: Value = serde_json::from_str(STORED[0].1.trim_end()).unwrap();
    assert_eq!(draft["penalty_model_version"], 1);
    draft["a_key_this_build_does_not_know"] = json!({ "kept": true });
    let mut expected = draft.clone();
    expected["penalty_model_version"] = json!(2);
    let groups = penalties::groups(kind);
    let blanked = groups[0].keys.clone();
    rows_excise::join_if_group_blanked(kind, &mut draft, &groups, &blanked, &Map::new());
    assert_eq!(draft, expected);
}

/// Decision P4-14: an older excise return kept as entered says so under its
/// penalty boxes even when they hold 0.00 (the host lists them in
/// `notes_always`; the page draws a lock note under a box showing a value
/// only for those), and keeps the one-click "Use the computed amounts"
/// path: blanking the group joins the typed due date, after which nothing
/// is listed and the boxes are computed. Every stored fixture lists its
/// group's boxes, and only those.
#[test]
fn p4_14_a_kept_return_says_so_over_its_amounts() {
    for (code, fixture) in STORED {
        setup();
        store_fixture(code, fixture.trim_end());
        let opened = open(code, 1);
        let group = &opened["penalties"][0];
        assert_eq!(group["kept"], true, "{code}");
        let mut keys: Vec<String> = strings(&group["keys"]).into_iter().collect();
        keys.sort();
        let mut noted: Vec<String> = strings(&opened["notes_always"]).into_iter().collect();
        noted.sort();
        assert_eq!(
            noted, keys,
            "{code}: the kept group's boxes, and only those"
        );
    }
    setup();
    let p = "frm2200Av2020:";
    store_fixture("2200A", STORED[1].1.trim_end());
    let opened = open("2200A", 1);
    let keys =
        ["txtPen_Surcharge", "txtPen_Interest", "txtPen_Compromise"].map(|k| format!("{p}{k}"));
    for key in &keys {
        assert_eq!(opened["values"][key], "0.00", "{key} holds 0.00");
        assert_eq!(
            opened["lock_notes"][key],
            "Kept as entered before penalties were computed."
        );
    }
    // Another edit keeps the note listed.
    let other = fill(
        "2200A",
        &opened["draft"],
        json!({ format!("{p}txtLess_Balance"): "5" }),
    );
    assert_eq!(
        strings(&other["notes_always"]),
        keys.iter().cloned().collect::<BTreeSet<_>>()
    );
    // "Use the computed amounts": the group joins, nothing is listed.
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let blanks: Map<String, Value> = keys.iter().map(|k| (k.clone(), json!(""))).collect();
    let joined = fill_unlocked("2200A", &other["draft"], Value::Object(blanks), &refs);
    assert_eq!(joined["rejected"], json!({}));
    assert_eq!(joined["draft"]["penalty_model_version"], 2);
    assert_eq!(joined["notes_always"], json!([]));
    for key in &keys {
        assert_eq!(joined["relock_notes"][key], NOTE_COMPUTED, "{key}");
    }
}

/// Decision P4-14 pins every other form: a new return of every form lists
/// no box whose lock note shows whatever it holds, so the page draws every
/// other lock note only under a blank box, as before; the exceptions are
/// 1702RT item 42 while shut (decision P4-17) and 1702EX's rows BIR's
/// program keeps shut (decision P4-8).
#[test]
fn p4_14_no_new_return_lists_a_note_shown_always() {
    setup();
    for kind in AgentFormKind::all() {
        let code = kind.form_code();
        let opened = open(code, page_period_of(code));
        let expected: Vec<String> = match code {
            "1702RT" => vec!["frm1702RT:txtPg2Pt4I42MinimumCorporate".to_string()],
            "1702EX" => rows_annual_corporate::ex_attached_keys()
                .filter(|key| key.ends_with("Amt") || key.ends_with("Amount"))
                .collect(),
            _ => Vec::new(),
        };
        assert_eq!(opened["notes_always"], json!(expected), "{code}");
    }
}
