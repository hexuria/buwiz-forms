//! PR 6: the annual information returns (1604C, 1604F, 1604E) against
//! their printed sheets. The oracles are the frozen sheets' own files
//! (html-frozen/<slug>/writer-cells.json captions and joins, index.html's
//! printed text and cell positions), the official keys, and hand lists
//! from the sheets; never this module's own refusal.

use super::*;

/// `(code, frozen sheet slug)` of the three forms.
const FORMS: [(&str, &str); 3] = [
    ("1604C", "1604c-2018"),
    ("1604F", "1604f-2018"),
    ("1604E", "1604e-2018"),
];

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const QUARTERS: [&str; 4] = ["1st Quarter", "2nd Quarter", "3rd Quarter", "4th Quarter"];

fn frozen(slug: &str, file: &str) -> String {
    let path = format!(
        "{}/../../html-frozen/{slug}/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn writer_cells(slug: &str) -> Value {
    serde_json::from_str(&frozen(slug, "writer-cells.json")).expect("writer-cells json")
}

/// Every join of a sheet: `(writer key, html id, printed caption)`.
fn joins(slug: &str) -> Vec<(String, String, String)> {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .map(|j| {
            (
                j["writer_key"].as_str().unwrap_or("").to_string(),
                j["html_id"].as_str().unwrap_or("").to_string(),
                j["printed"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

/// The printed caption writer-cells.json gives `key`.
fn printed_at(slug: &str, key: &str) -> String {
    joins(slug)
        .into_iter()
        .find(|(k, _, _)| k == key)
        .map(|(_, _, printed)| printed)
        .unwrap_or_else(|| panic!("{slug}: {key} is joined"))
}

/// The frozen sheet's printed words, one space between text runs.
fn sheet_text(slug: &str) -> String {
    let html = frozen(slug, "index.html");
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

/// A printed cell's left edge on the frozen sheet, in points.
fn left_edge(slug: &str, id: &str) -> f64 {
    let html = frozen(slug, "index.html");
    let at = html
        .find(&format!("<div id=\"{id}\""))
        .unwrap_or_else(|| panic!("{slug}: cell {id}"));
    let tag = &html[at..at + html[at..].find('>').unwrap()];
    let x = tag.split("left:").nth(1).unwrap();
    x.split("pt").next().unwrap().parse().unwrap()
}

/// Every drawn box: `(part name, item no, item label, key, type)`.
fn drawn(code: &str) -> Vec<(String, String, String, String, String)> {
    let form = schema_entry(code).unwrap();
    let mut out = Vec::new();
    for part in form["parts"].as_array().unwrap() {
        let name = part["n"].as_str().unwrap().to_string();
        for item in part["items"].as_array().unwrap() {
            let no = item["no"].as_str().unwrap_or("").to_string();
            let label = item["l"].as_str().unwrap_or("").to_string();
            for field in item["f"].as_array().into_iter().flatten() {
                if field["t"] != "hidden" {
                    out.push((
                        name.clone(),
                        no.clone(),
                        label.clone(),
                        field["k"].as_str().unwrap().to_string(),
                        field["t"].as_str().unwrap().to_string(),
                    ));
                }
            }
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().unwrap().iter().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] != "hidden" {
                        out.push((
                            name.clone(),
                            no.clone(),
                            label.clone(),
                            key.as_str().unwrap().to_string(),
                            col["t"].as_str().unwrap().to_string(),
                        ));
                    }
                }
            }
        }
    }
    out
}

/// The part, item number and item label a key is drawn under.
fn placed(code: &str, key: &str) -> (String, String, String) {
    drawn(code)
        .into_iter()
        .find(|(_, _, _, k, _)| k == key)
        .map(|(part, no, label, _, _)| (part, no, label))
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
}

/// The printed item number a caption starts with (`7A`, `11`).
fn printed_number(printed: &str) -> Option<String> {
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

/// The print document of `draft` (the in-app preview's own fill).
fn document(code: &str, slug: &str, draft: &Value) -> String {
    bir_print::frozen_html::filled_document(slug, &printed(code, draft)).unwrap()
}

fn part<'a>(code: &str, name: &str) -> &'a Value {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == name)
        .unwrap_or_else(|| panic!("{code} has a part {name:?}"))
}

/// R3 (1604C R3.0, manual-0, manual-2; 1604F R3.0, manual-2; 1604E R3.0):
/// every box the sheet prints under an item number is drawn under that
/// number, and no drawn box sits in an unnumbered "Additional details"
/// group. The oracle is writer-cells.json's printed captions, plus the
/// sheet's 7A ZIP Code on 1604F, whose ZIP prints inside the second address
/// comb ("7 Registered Address (second line, 31 fillable slots) and 7A ZIP
/// Code (last 4 slots)").
#[test]
fn pr6_boxes_sit_under_their_printed_item_numbers() {
    for (code, slug) in FORMS {
        let boxes = drawn(code);
        let mut checked = 0;
        for (key, _, printed) in joins(slug) {
            let Some(number) = printed_number(&printed) else {
                continue;
            };
            let Some((_, no, _, _, _)) = boxes.iter().find(|(_, _, _, k, _)| *k == key) else {
                continue;
            };
            assert_eq!(*no, number, "{code} {key} prints at {printed:?}");
            checked += 1;
        }
        assert!(checked >= 15, "{code}: {checked} boxes checked");
        for (part, no, label, key, _) in &boxes {
            assert_ne!(label, "Additional details", "{code} {key} in {part} {no:?}");
        }
    }
    // Item 5 is drawn on 1604C (it had no box), 7A on 1604F (it had no number).
    assert_eq!(placed("1604C", "frm1604c:rdoCode").1, "5");
    assert!(printed_at("1604f-2018", "derived:address2_zip").contains("7A ZIP Code"));
    assert_eq!(placed("1604F", "frm1604f:zipCode").1, "7A");
    // Part I runs 4, 5, 6, 7, 7A, 8, 8A, 9, 10 and on, as printed.
    let numbers = |code: &str| -> Vec<String> {
        part(code, "Part I · Background Information")["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| it["no"].as_str().unwrap().to_string())
            .collect()
    };
    let base = ["4", "5", "6", "7", "7A", "8", "8A", "9", "10"];
    let with =
        |more: &[&str]| -> Vec<String> { base.iter().chain(more).map(|s| s.to_string()).collect() };
    assert_eq!(numbers("1604C"), with(&["11", "11A", "12", "13"]));
    assert_eq!(numbers("1604F"), with(&["11", "11A"]));
    assert_eq!(numbers("1604E"), with(&[]));
    for (code, slug) in FORMS {
        let text = sheet_text(slug);
        for no in numbers(code) {
            assert!(text.contains(&format!(" {no} ")), "{slug} prints item {no}");
        }
    }
}

/// 1604C manual-2 (and the dropped R3.1), 1604F manual-2, 1604E manual-2:
/// the item captions read as printed. Each caption is a hand list of the
/// sheet's words (the frozen text layer splits items 11 and 12 of 1604C
/// around their numbers, so each piece is checked on the sheet).
#[test]
fn pr6_labels_read_as_printed() {
    let label = |code: &str, no: &str| -> String {
        part(code, "Part I · Background Information")["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|it| it["no"] == no)
            .and_then(|it| it["l"].as_str())
            .unwrap_or_else(|| panic!("{code} item {no}"))
            .to_string()
    };
    for (code, slug, no, pieces) in [
        (
            "1604C",
            "1604c-2018",
            "8",
            vec!["Category of Withholding Agent"],
        ),
        (
            "1604C",
            "1604c-2018",
            "11",
            vec![
                "In case of overwithholding/overremittance after the year-end adjustments on compensation,",
                "have you released the refund/s to your employee/s?",
            ],
        ),
        (
            "1604C",
            "1604c-2018",
            "11A",
            vec!["If Yes, specify the date", "of refund (MM/DD/YYYY)"],
        ),
        (
            "1604C",
            "1604c-2018",
            "12",
            vec![
                "Total amount of overremittance on Tax",
                "withheld under compensation",
            ],
        ),
        (
            "1604C",
            "1604c-2018",
            "13",
            vec!["Month of First Crediting of Overremittance (MM)"],
        ),
        ("1604F", "1604f-2018", "7A", vec!["ZIP Code"]),
        (
            "1604F",
            "1604f-2018",
            "11",
            vec![
                "Are there payees availing of tax relief under",
                "Special Law or International Tax Treaty?",
            ],
        ),
        ("1604F", "1604f-2018", "11A", vec!["If yes, specify"]),
        (
            "1604E",
            "1604e-2018",
            "8",
            vec!["Category of Withholding Agent"],
        ),
    ] {
        let text = sheet_text(slug);
        for piece in &pieces {
            assert!(text.contains(piece), "{slug} prints {piece:?}");
        }
        assert_eq!(label(code, no), pieces.join(" "), "{code} {no}");
    }
    // Item 8's heading no longer repeats its options; its options do.
    for (code, keys) in [
        (
            "1604C",
            ["frm1604c:categoryAgent_1", "frm1604c:categoryAgent_2"],
        ),
        (
            "1604E",
            ["frm1604e:WthldngAgntCtgry_1", "frm1604e:WthldngAgntCtgry_2"],
        ),
    ] {
        let options: Vec<String> = schema_entry(code).unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
            .filter(|f| keys.contains(&f["k"].as_str().unwrap_or("")))
            .map(|f| f["l"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(options, ["Private", "Government"], "{code}");
        assert!(!label(code, "8").contains("Private"), "{code}");
    }
}

/// The schedules of Part II: `(code, slug, table title, the writer-cells
/// caption prefix, row captions)`.
fn schedules() -> Vec<(
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    Vec<&'static str>,
)> {
    vec![
        (
            "1604C",
            "1604c-2018",
            "January to December (Part II and its Continuation)",
            "Part II",
            MONTHS.to_vec(),
        ),
        (
            "1604F",
            "1604f-2018",
            "Schedule 1 – Remittance per BIR Form No. 1601-FQ on Final Taxes Withheld",
            "Schedule 1",
            QUARTERS.to_vec(),
        ),
        (
            "1604F",
            "1604f-2018",
            "Schedule 2 – Remittance per BIR Form No. 1602Q on Interest Payments",
            "Schedule 2",
            QUARTERS.to_vec(),
        ),
        (
            "1604F",
            "1604f-2018",
            "Schedule 3 – Remittance per BIR Form No. 1603Q on Fringe Benefits",
            "Schedule 3",
            QUARTERS.to_vec(),
        ),
        (
            "1604E",
            "1604e-2018",
            "Schedule 1 – Remittance per BIR Form No. 1601-EQ",
            "Schedule 1",
            QUARTERS.to_vec(),
        ),
        (
            "1604E",
            "1604e-2018",
            "Schedule 2 – Remittance per BIR Form No. 1606",
            "Schedule 2",
            MONTHS.to_vec(),
        ),
    ]
}

/// 1604C manual-1, R3.0; 1604F manual-0, R3.0; 1604E manual-0, manual-1,
/// R3.0: Part II is its own part; each schedule is one table titled as
/// printed, a row per printed month or quarter with that caption, its
/// columns the printed ones in the sheet's order, and its TOTAL row under
/// it. Oracle: every cell's writer-cells.json caption ("Schedule 2 January
/// Drawee Bank/Bank Code/Agency", "Part II continuation March Penalties")
/// names the row and column the page draws it in, and the columns' left
/// edges on the frozen sheet rise left to right within each printed block.
#[test]
fn pr6_part_ii_schedules_follow_the_printed_rows_and_columns() {
    for (code, slug, title, prefix, rows) in schedules() {
        let name = match code {
            "1604C" => "Part II · Summary of Remittances per BIR Form No. 1601-C",
            _ => "Part II · Summary of Remittances",
        };
        assert!(
            sheet_text(slug).contains(&name.replace(" · ", " – ")),
            "{slug} prints {name}"
        );
        let items = part(code, name)["items"].as_array().unwrap();
        let at = items
            .iter()
            .position(|it| it["l"] == title)
            .unwrap_or_else(|| panic!("{code}: {title}"));
        if code != "1604C" {
            assert!(sheet_text(slug).contains(title), "{slug} prints {title}");
        }
        let table = &items[at];
        assert_eq!(table["no"], "", "{code} {title}: no invented item number");
        assert_eq!(table["rows"], rows.len(), "{code} {title}");
        assert_eq!(table["row_labels"]["rows"], json!(rows), "{code} {title}");
        let cols: Vec<&str> = table["cols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["l"].as_str().unwrap())
            .collect();
        let keys = table["keys"].as_array().unwrap();
        for (r, row) in keys.iter().enumerate() {
            for (c, key) in row.as_array().unwrap().iter().enumerate() {
                let key = key.as_str().unwrap();
                let printed = printed_at(slug, key);
                let rest = printed
                    .strip_prefix(prefix)
                    .map(|s| {
                        s.trim_start()
                            .trim_start_matches("continuation ")
                            .to_string()
                    })
                    .unwrap_or_else(|| panic!("{key}: {printed:?}"));
                let column = rest
                    .strip_prefix(rows[r])
                    .unwrap_or_else(|| panic!("{code} {key} is row {:?}: {printed:?}", rows[r]))
                    .trim();
                assert!(
                    cols[c].starts_with(column),
                    "{code} {key}: column {:?}, printed {printed:?}",
                    cols[c]
                );
            }
        }
        // Printed order: the left edges rise within a block (1604C's
        // Continuation of Part II starts a second block at Adjustment).
        let row0: Vec<String> = keys[0]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        let id = |key: &str| {
            joins(slug)
                .into_iter()
                .find(|(k, _, _)| k == key)
                .map(|(_, id, _)| id)
                .unwrap()
        };
        let blocks: Vec<&[String]> = if code == "1604C" {
            vec![&row0[..4], &row0[4..]]
        } else {
            vec![&row0[..]]
        };
        for block in blocks {
            let edges: Vec<f64> = block.iter().map(|k| left_edge(slug, &id(k))).collect();
            assert!(
                edges.windows(2).all(|w| w[0] < w[1]),
                "{code} {title}: {block:?} {edges:?}"
            );
        }
        // The TOTAL row: each box under the column it prints in.
        let total = &items[at + 1];
        assert_eq!(total["l"], "TOTAL", "{code} {title}");
        assert_eq!(total["no"], "");
        for field in total["f"].as_array().unwrap() {
            let key = field["k"].as_str().unwrap();
            let printed = printed_at(slug, key);
            let column = printed
                .split(" TOTAL ")
                .nth(1)
                .unwrap_or_else(|| panic!("{key}: {printed:?}"));
            assert_eq!(field["l"], column, "{code} {key}");
            assert!(cols.iter().any(|c| c.starts_with(column)), "{code} {key}");
        }
    }
    // No table keeps an item number made of key text (1604E manual-1's
    // "Schedule 2 month 1", 1604F manual-0's "Schedule 1 quarter 1").
    for (code, _) in FORMS {
        for p in schema_entry(code).unwrap()["parts"].as_array().unwrap() {
            for it in p["items"].as_array().unwrap() {
                if it.get("rows").is_some() {
                    assert_eq!(it["no"], "", "{code} {}", it["l"]);
                }
            }
        }
    }
}

/// Lesson 7: a Part II row typed on the page goes through the form's setter
/// (its row total and the TOTAL row recompute) and prints in the sheet's
/// cells for that month or quarter (writer-cells.json joins).
#[test]
fn pr6_part_ii_rows_compute_and_print_where_the_sheet_prints_them() {
    setup();
    for (code, slug, edits, computed) in [
        (
            "1604C",
            "1604c-2018",
            json!({
                "frm1604c:txtSched1Date3": "03/10/2026",
                "frm1604c:txtSched1BankVal3": "LBP 001",
                "frm1604c:txtTra3": "TRA-3",
                "frm1604c:txtSched1TaxWheld3": "1000",
                "frm1604c:txtSched1Adj3": "50",
                "frm1604c:txtSched1Pen3": "25",
            }),
            vec![
                ("frm1604c:txtSched1TotalAmt3", "1,075.00"),
                ("frm1604c:txtSched1TaxWheldTotal", "1,000.00"),
                ("frm1604c:txtSched1AdjTotal", "50.00"),
                ("frm1604c:txtSched1PenTotal", "25.00"),
                ("frm1604c:txtSched1Total", "1,075.00"),
            ],
        ),
        (
            "1604F",
            "1604f-2018",
            json!({
                "frm1604f:txtSched2Date3": "10/10/2026",
                "frm1604f:txtSched2TRA3": "TRA-Q3",
                "frm1604f:txtSched2TaxWheld3": "2000",
                "frm1604f:txtSched2Pen3": "100",
            }),
            vec![
                ("frm1604f:txtSched2TotalAmt3", "2,100.00"),
                ("frm1604f:txtSched2TaxWheldTotal", "2,000.00"),
                ("frm1604f:txtSched2PenTotal", "100.00"),
                ("frm1604f:txtSched2Total", "2,100.00"),
            ],
        ),
        (
            "1604E",
            "1604e-2018",
            // Schedule 1's 3rd quarter and Schedule 2's February: rows whose
            // Total the probe once bound to their penalties.
            json!({
                "frm1604e:txtSched1RemDate3": "10/20/2026",
                "frm1604e:txtSched1BankCode3": "PNB 003",
                "frm1604e:txtSched1TRANo3": "TRA-Q3",
                "frm1604e:txtSched1TaxWithheld3": "700",
                "frm1604e:txtSched1Penalties3": "30",
                "frm1604e:txtSched2RemDate2": "03/10/2026",
                "frm1604e:txtSched2BankCode2": "LBP 004",
                "frm1604e:txtSched2TRANo2": "TRA-02",
                "frm1604e:txtSched2TaxWithheld2": "40",
                "frm1604e:txtSched2Penalties2": "5",
                "frm1604e:txtSched2RemDate12": "01/10/2027",
                "frm1604e:txtSched2BankCode12": "BDO 002",
                "frm1604e:txtSched2TRANo12": "TRA-12",
                "frm1604e:txtSched2TaxWithheld12": "300",
                "frm1604e:txtSched2Penalties12": "20",
            }),
            vec![
                ("frm1604e:txtSched1TotRemAmt3", "730.00"),
                ("frm1604e:txtSched1TaxWithheldTtl", "700.00"),
                ("frm1604e:txtSched1PenaltiesTtl", "30.00"),
                ("frm1604e:txtSched1TotRemAmtTtl", "730.00"),
                ("frm1604e:txtSched2TotRemAmt2", "45.00"),
                ("frm1604e:txtSched2TotRemAmt12", "320.00"),
                ("frm1604e:txtSched2TaxWithheldTtl", "340.00"),
                ("frm1604e:txtSched2PenaltiesTtl", "25.00"),
                ("frm1604e:txtSched2TotRemAmtTtl", "365.00"),
            ],
        ),
    ] {
        let opened = open(code, 0);
        let filled = fill(code, &opened["draft"], edits.clone());
        assert_eq!(filled["rejected"], json!({}), "{code}");
        for (key, want) in &computed {
            assert_eq!(filled["values"][*key], *want, "{code} {key}");
        }
        let html = document(code, slug, &filled["draft"]);
        for (key, _, printed) in joins(slug) {
            let Some(typed) = edits.get(&key).and_then(Value::as_str) else {
                continue;
            };
            let (_, id, _) = joins(slug).into_iter().find(|(k, _, _)| *k == key).unwrap();
            let shown = bir_print::frozen_html::printed_text(&html, &id);
            let typed_amount = typed.parse::<f64>().ok();
            match typed_amount {
                Some(n) if !typed.contains('/') => assert_eq!(
                    agent::parse_amount(&shown),
                    Some(n),
                    "{code} {key} at {printed:?}: {shown:?}"
                ),
                _ => assert_eq!(shown, typed, "{code} {key} at {printed:?}"),
            }
        }
    }
}

/// 1604C R2.2, preview.0, manual-0: Item 5 RDO Code is drawn, filled from
/// the profile and locked with Edit like the other profile boxes; the edit
/// stays on this return and prints in the sheet's item 5 cell
/// (writer-cells.json joins rdoCode at "5 RDO Code").
#[test]
fn pr6_form_1604c_item_5_rdo_comes_from_the_profile_and_prints() {
    setup();
    let key = "frm1604c:rdoCode";
    let opened = open("1604C", 0);
    assert_eq!(opened["values"][key], "039");
    assert_eq!(opened["locks"][key], agent::Lock::Profile.reason());
    assert!(strings(&opened["profile"]).contains(key));
    assert!(strings(&opened["profile_edit"]).contains(key));
    assert!(
        opened["options"][key]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["value"] == "040")
    );
    assert_eq!(printed_at("1604c-2018", key), "5 RDO Code");
    let (_, cell, _) = joins("1604c-2018")
        .into_iter()
        .find(|(k, _, _)| k == key)
        .unwrap();
    let html = document("1604C", "1604c-2018", &opened["draft"]);
    assert_eq!(bir_print::frozen_html::printed_text(&html, &cell), "039");
    let refused = fill("1604C", &opened["draft"], json!({ key: "040" }));
    assert!(refused["rejected"].get(key).is_some());
    let edited = fill_unlocked("1604C", &opened["draft"], json!({ key: "040" }), &[key]);
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(edited["draft"]["rdo_code"], "040");
    let html = document("1604C", "1604c-2018", &edited["draft"]);
    assert_eq!(bir_print::frozen_html::printed_text(&html, &cell), "040");
    let profile = with_database(|db| {
        db.get_profile_by_tin(TIN)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(profile.rdo_code, "039", "the profile is never written");
}

/// 1604F R2.0, 1604E R2.0 (and 1604C): every box the profile fills is in
/// its printed item (Part I) or under Not on the printed sheet, offers Edit,
/// or is a page 2 header copy that is not drawn while the print still
/// writes it from the same draft field. Item 6's Edit carries to the page 2
/// header (both bind /taxpayer_name).
#[test]
fn pr6_profile_boxes_are_in_their_items_and_open_with_edit() {
    setup();
    for (code, slug) in FORMS {
        let opened = open(code, 0);
        let editable = strings(&opened["profile_edit"]);
        for key in strings(&opened["profile"]) {
            let Some((part, no, _)) = drawn(code)
                .into_iter()
                .find(|(_, _, _, k, _)| *k == key)
                .map(|(p, n, l, _, _)| (p, n, l))
            else {
                // Not drawn: a page 2 header copy, or 1604F's second email.
                assert!(
                    key.contains("Pg2") || (code == "1604F" && key == "txtEmail"),
                    "{code} {key} is drawn"
                );
                assert!(schema_keys(code).contains(&key));
                continue;
            };
            assert!(editable.contains(&key), "{code} {key} offers Edit");
            assert!(
                (part == "Part I · Background Information" && !no.is_empty())
                    || part == "Not on the printed sheet",
                "{code} {key} in {part} {no}"
            );
        }
        if code == "1604C" {
            continue;
        }
        let stem = code.to_ascii_lowercase();
        for tail in ["TIN1", "TIN2", "TIN3", "BranchCode", "TaxpayerName"] {
            let key = format!("frm{stem}:txtPg2{tail}");
            assert!(schema_keys(code).contains(&key), "{key}");
            assert!(
                !drawn(code).iter().any(|(_, _, _, k, _)| *k == key),
                "{key}"
            );
        }
        assert!(
            !drawn(code)
                .iter()
                .any(|(_, _, _, k, _)| k.ends_with("txtCurrentPage")),
            "{code}: the page counter is not drawn"
        );
        // The print still writes page 2's header from the draft.
        let name_key = format!("frm{stem}:txtPg2TaxpayerName");
        let (_, name_cell, printed) = joins(slug)
            .into_iter()
            .find(|(k, _, _)| *k == name_key)
            .unwrap();
        assert_eq!(printed, "Page 2 Withholding Agent’s Name");
        let (_, tin_cell, _) = joins(slug)
            .into_iter()
            .find(|(k, _, _)| k == "derived:tin_digits")
            .unwrap();
        let item6 = if code == "1604F" {
            "frm1604f:registeredName"
        } else {
            "frm1604e:txtWthhldngAgntsNme"
        };
        let edited = fill_unlocked(
            code,
            &opened["draft"],
            json!({ item6: "Another Name Corp" }),
            &[item6],
        );
        assert_eq!(edited["rejected"], json!({}), "{code}");
        let html = document(code, slug, &edited["draft"]);
        assert!(
            bir_print::frozen_html::printed_text(&html, &name_cell)
                .eq_ignore_ascii_case("Another Name Corp"),
            "{code}: page 2's name follows item 6"
        );
        assert_eq!(
            bir_print::frozen_html::printed_text(&html, &tin_cell),
            "12345678800000",
            "{code}: page 2's TIN"
        );
    }
}

/// 1604F manual-1: item 10 shows one email box, the one the sheet prints
/// (writer-cells.json joins frm1604f:email at "10 Email Address"; not_joined
/// names txtEmail "duplicate of frm1604f:email (same value)"). The XML
/// still writes both, the same.
#[test]
fn pr6_form_1604f_item_10_shows_one_email() {
    setup();
    let ten: Vec<String> = drawn("1604F")
        .into_iter()
        .filter(|(_, no, _, _, _)| no == "10")
        .map(|(_, _, _, k, _)| k)
        .collect();
    assert_eq!(ten, ["frm1604f:email"]);
    assert_eq!(
        printed_at("1604f-2018", "frm1604f:email"),
        "10 Email Address"
    );
    let cells = writer_cells("1604f-2018");
    assert!(cells["not_joined"].as_array().unwrap().iter().any(|e| {
        e["writer_key"] == "txtEmail"
            && e["reason"]
                .as_str()
                .unwrap()
                .starts_with("duplicate of frm1604f:email")
    }));
    assert!(schema_keys("1604F").contains("txtEmail"));
    let opened = open("1604F", 0);
    let values = &opened["values"];
    assert!(
        values["txtEmail"]
            .as_str()
            .unwrap()
            .eq_ignore_ascii_case(values["frm1604f:email"].as_str().unwrap())
    );
}

/// Everything the printed layout leaves out goes to "Not on the printed
/// sheet", and only the line of business is drawn there (1604F, 1604E;
/// writer-cells.json not_joined: "not on the paper form"). 1604E R5.2: no
/// TIN parts are drawn outside item 4 (the page 2 copies are not drawn).
#[test]
fn pr6_only_named_boxes_are_drawn_off_the_printed_sheet() {
    for (code, slug, expected) in [
        ("1604C", "1604c-2018", vec![]),
        ("1604F", "1604f-2018", vec!["frm1604f:txtLineBus"]),
        ("1604E", "1604e-2018", vec!["frm1604e:txtLineBus"]),
    ] {
        let off: Vec<String> = drawn(code)
            .into_iter()
            .filter(|(part, _, _, _, _)| part == "Not on the printed sheet")
            .map(|(_, _, _, k, _)| k)
            .collect();
        assert_eq!(off, expected, "{code}");
        for key in &expected {
            assert_eq!(placed(code, key).2, "Line of Business");
            assert!(
                writer_cells(slug)["not_joined"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|e| e["writer_key"] == *key
                        && e["reason"]
                            .as_str()
                            .unwrap()
                            .starts_with("not on the paper form"))
            );
        }
        let tin_parts: Vec<(String, String)> = drawn(code)
            .into_iter()
            .filter(|(_, _, _, k, _)| {
                let tail = k.rsplit(':').next().unwrap();
                tail.contains("TIN")
                    || tail.starts_with("tin")
                    || tail.contains("BranchCode")
                    || tail == "branchCode"
            })
            .map(|(_, no, _, k, _)| (no, k))
            .collect();
        assert_eq!(tin_parts.len(), 4, "{code}: {tin_parts:?}");
        assert!(
            tin_parts.iter().all(|(no, _)| no == "4"),
            "{code}: {tin_parts:?}"
        );
    }
}

/// Lesson 8: Item 11 No clears what the filer typed under it (1604C Items
/// 11A-13, `changeRefund`; 1604F Item 11A, `TaxReliefEnable`), so the host
/// reports it and the page asks first. Nothing typed, nothing asked.
#[test]
fn pr6_item_11_no_asks_before_clearing_what_was_typed() {
    setup();
    let opened = open("1604C", 0);
    let yes = fill(
        "1604C",
        &opened["draft"],
        json!({"frm1604c:releasedOfFunds_1": "true", "frm1604c:releasedOfFunds_2": "false"}),
    );
    assert_eq!(yes["removes"], Value::Null, "Yes removes nothing");
    let no = json!({"frm1604c:releasedOfFunds_1": "false", "frm1604c:releasedOfFunds_2": "true"});
    assert_eq!(
        fill("1604C", &yes["draft"], no.clone())["removes"],
        Value::Null,
        "nothing typed under 11, nothing asked"
    );
    let typed = fill(
        "1604C",
        &yes["draft"],
        json!({
            "frm1604c:txtRefMonth": "03",
            "frm1604c:txtRefDate": "15",
            "frm1604c:txtRefYear": "2026",
            "frm1604c:txt12": "1234.5",
            "frm1604c:select13": "4",
        }),
    );
    assert_eq!(typed["rejected"], json!({}));
    assert_eq!(typed["draft"]["refund_date"], "03/15/2026");
    assert_eq!(typed["draft"]["first_crediting_month"], 4);
    let cleared = fill("1604C", &typed["draft"], no.clone());
    assert_eq!(cleared["draft"]["refunds_released"], false);
    assert_eq!(cleared["draft"]["refund_date"], "");
    assert_eq!(cleared["draft"]["overremittance"], 0.0);
    assert_eq!(cleared["draft"]["first_crediting_month"], 0);
    assert_eq!(
        cleared["removes"],
        json!({
            "message": "Changing item 11 to No clears item 11A (date of refund 03/15/2026), item 12 (₱1,234.50) and item 13 (April). Continue?",
            "done": "Cleared items 11A, 12 and 13",
            "rows": 0,
        })
    );
    // The Undo toast names only what was cleared.
    let only_12 = fill("1604C", &yes["draft"], json!({"frm1604c:txt12": "500"}));
    assert_eq!(
        fill("1604C", &only_12["draft"], no)["removes"],
        json!({
            "message": "Changing item 11 to No clears item 12 (₱500.00). Continue?",
            "done": "Cleared item 12",
            "rows": 0,
        })
    );
    // 1604F Item 11A.
    let opened = open("1604F", 0);
    let yes = fill(
        "1604F",
        &opened["draft"],
        json!({"frm1604f:taxRelief_1": "true", "frm1604f:taxRelief_2": "false"}),
    );
    let typed = fill(
        "1604F",
        &yes["draft"],
        json!({"frm1604f:availedTaxRelief": "PEZA"}),
    );
    assert_eq!(typed["rejected"], json!({}));
    let cleared = fill(
        "1604F",
        &typed["draft"],
        json!({"frm1604f:taxRelief_1": "false", "frm1604f:taxRelief_2": "true"}),
    );
    assert_eq!(cleared["draft"]["tax_relief_details"], "");
    assert_eq!(
        cleared["removes"],
        json!({
            "message": "Changing item 11 to No clears item 11A (PEZA). Continue?",
            "done": "Cleared item 11A",
            "rows": 0,
        })
    );
    // Another form's same-named answer is not this one's.
    let opened = open("1604E", 0);
    let filled = fill(
        "1604E",
        &opened["draft"],
        json!({"frm1604e:AmendedRtn_1": "true", "frm1604e:AmendedRtn_2": "false"}),
    );
    assert_eq!(filled["removes"], Value::Null);
}

/// The shared `grid` change: a grid that names only loose fields builds the
/// table it built before (the extracted tables' cells are a fallback only),
/// and on this branch no form but the three 1604s names a grid.
#[test]
fn pr6_a_grid_of_loose_fields_is_unchanged_by_table_cells() {
    let field = |k: &str, t: &str| json!({"k": k, "l": k, "t": t});
    let form = json!({
        "c": "TEST",
        "parts": [{"n": "P", "items": [
            {"no": "1", "l": "loose", "p": 1, "f": [field("a1", "money"), field("b1", "text")]},
            {"no": "", "l": "table", "p": 2, "cols": [{"l": "A", "t": "date"}, {"l": "B", "t": "sel"}],
             "rows": 1, "keys": [["a1", "b1"]]},
        ]}],
    });
    let entry = json!({"no": "", "l": "g", "grid": {"cols": ["A", "B"], "keys": [["a1", "b1"]]}});
    let mut fields = BTreeMap::new();
    for f in form["parts"][0]["items"][0]["f"].as_array().unwrap() {
        fields.insert(f["k"].as_str().unwrap().to_string(), (f.clone(), json!(1)));
    }
    let before = grid_table(&entry, &fields).unwrap();
    let mut laid = form.clone();
    apply_layout(&mut laid, &json!({"parts": [{"n": "P", "items": [entry]}]}));
    assert_eq!(laid["parts"][0]["items"][0], before);
    assert_eq!(
        before["cols"][0]["t"], "money",
        "the loose field's type, not the cell's"
    );
    let overrides: Value = serde_json::from_str(include_str!(
        "../../../../../app/forms/schema-overrides.json"
    ))
    .unwrap();
    let with_grid: Vec<&String> = overrides
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, fixes)| {
            fixes["layout"]["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|p| p["items"].as_array().into_iter().flatten())
                .any(|it| it.get("grid").is_some())
        })
        .map(|(code, _)| code)
        .collect();
    assert_eq!(with_grid, ["1604C", "1604E", "1604F"]);
}

/// 1604E review (items 1 and 2): every Total Amount Remitted cell of Part
/// II, the 16 rows and the TOTAL rows' 6 boxes, is the form's: drawn as
/// computed, locked with "Computed by the form.", a typed value refused, and
/// a Total edit never moves any row's penalties (the probe bound 10 of them
/// to /scheduleN/k/penalties before COMPUTED_BY_FORM). The oracle is the
/// writer-cells caption of each cell ("... Total Amount Remitted", "...
/// TOTAL ...").
#[test]
fn pr6_form_1604e_totals_are_computed_by_the_form() {
    setup();
    let slug = "1604e-2018";
    let totals: Vec<String> = joins(slug)
        .into_iter()
        .filter(|(k, _, printed)| {
            k.starts_with("frm1604e:txtSched")
                && (printed.ends_with("Total Amount Remitted") || printed.contains(" TOTAL "))
        })
        .map(|(k, _, _)| k)
        .collect();
    assert_eq!(totals.len(), 16 + 6, "{totals:?}");
    let computed_on_page = |key: &str| -> bool {
        let form = schema_entry("1604E").unwrap();
        form["parts"].as_array().unwrap().iter().any(|p| {
            p["items"].as_array().unwrap().iter().any(|it| {
                it["f"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|f| f["k"] == key && f["c"].as_i64() == Some(1))
                    || it["keys"].as_array().into_iter().flatten().any(|row| {
                        row.as_array()
                            .unwrap()
                            .iter()
                            .position(|k| k == key)
                            .is_some_and(|c| it["cols"][c]["c"].as_i64() == Some(1))
                    })
            })
        })
    };
    let opened = open("1604E", 0);
    let penalties = |draft: &Value| -> Vec<Value> {
        ["schedule1", "schedule2"]
            .iter()
            .flat_map(|s| {
                draft[*s]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r["penalties"].clone())
            })
            .collect()
    };
    for key in &totals {
        assert!(computed_on_page(key), "{key} is drawn as computed");
        assert_eq!(
            opened["locks"][key], "computed by BIR's own formulas",
            "{key}"
        );
        assert_eq!(opened["lock_notes"][key], "Computed by the form.", "{key}");
        let typed = fill("1604E", &opened["draft"], json!({ key.as_str(): "1000" }));
        assert!(
            typed["rejected"].get(key).is_some(),
            "{key} refuses a typed value"
        );
        assert_eq!(
            penalties(&typed["draft"]),
            penalties(&opened["draft"]),
            "{key}"
        );
        // Not even through Edit: no Edit opens a computed box.
        let unlocked = fill_unlocked(
            "1604E",
            &opened["draft"],
            json!({ key.as_str(): "1000" }),
            &[key],
        );
        assert!(unlocked["rejected"].get(key).is_some(), "{key} with Edit");
        assert_eq!(
            penalties(&unlocked["draft"]),
            penalties(&opened["draft"]),
            "{key}"
        );
    }
    let def = kind_of("1604E").unwrap().definition();
    assert!(
        totals.iter().all(|key| !def.bindings.contains_key(key)),
        "no Total is bound"
    );
}

/// Review item 6: item 5 RDO Code of 1604E and 1604F is the same catalog
/// select as 1604C's, profile-locked with Edit; the edit stays on this
/// return and prints at "5 RDO Code". A stored draft whose RDO is not in
/// the catalog still shows it, prints it and files it unchanged.
#[test]
fn pr6_item_5_rdo_is_one_catalog_select_on_all_three() {
    setup();
    for (code, slug, key) in [
        ("1604C", "1604c-2018", "frm1604c:rdoCode"),
        ("1604E", "1604e-2018", "frm1604e:txtRDOCode"),
        ("1604F", "1604f-2018", "frm1604f:rdoCode"),
    ] {
        let (_, no, _) = placed(code, key);
        assert_eq!(no, "5");
        assert!(
            drawn(code)
                .iter()
                .any(|(_, _, _, k, t)| k == key && t == "sel"),
            "{code}"
        );
        assert_eq!(printed_at(slug, key), "5 RDO Code");
        let opened = open(code, 0);
        assert_eq!(opened["values"][key], "039");
        assert_eq!(
            opened["locks"][key],
            agent::Lock::Profile.reason(),
            "{code}"
        );
        assert!(strings(&opened["profile_edit"]).contains(key), "{code}");
        let options = opened["options"][key].as_array().unwrap();
        assert_eq!(
            options.len(),
            bir_core::reference::get_all_rdos().len(),
            "{code}: the catalog"
        );
        let refused = fill(code, &opened["draft"], json!({ key: "040" }));
        assert!(refused["rejected"].get(key).is_some(), "{code}");
        let edited = fill_unlocked(code, &opened["draft"], json!({ key: "040" }), &[key]);
        assert_eq!(edited["rejected"], json!({}), "{code}");
        assert_eq!(edited["draft"]["rdo_code"], "040");
        let (_, cell, _) = joins(slug).into_iter().find(|(k, _, _)| k == key).unwrap();
        let html = document(code, slug, &edited["draft"]);
        assert_eq!(bir_print::frozen_html::printed_text(&html, &cell), "040");
        // A legacy value the catalog does not list.
        assert!(!options.iter().any(|o| o["value"] == "9Z9"));
        let mut legacy = opened["draft"].clone();
        legacy["rdo_code"] = json!("9Z9");
        let saved = save(code, &legacy);
        assert_eq!(saved["draft"]["rdo_code"], "9Z9", "{code}");
        let reopened = open(code, 0);
        assert_eq!(
            reopened["draft"]["rdo_code"], "9Z9",
            "{code}: never changed on open"
        );
        assert_eq!(reopened["values"][key], "9Z9", "{code}: shown as stored");
        let html = document(code, slug, &reopened["draft"]);
        assert_eq!(bir_print::frozen_html::printed_text(&html, &cell), "9Z9");
        let values = values_json(kind_of(code).unwrap(), &reopened["draft"]).unwrap();
        assert_eq!(values[key], "9Z9", "{code}: the field map files it");
        // An unrelated edit keeps it.
        let other = fill(code, &reopened["draft"], json!({}));
        assert_eq!(other["draft"]["rdo_code"], "9Z9");
    }
}

/// Review item 7: a layout that names only some cells of an extracted
/// table is a broken override and stops the load, naming the form and the
/// table; naming all of them builds.
#[test]
fn pr6_a_layout_claiming_part_of_a_table_is_an_error() {
    let form = json!({
        "c": "TEST",
        "parts": [{"n": "P", "items": [
            {"no": "", "l": "Schedule 9", "p": 1, "cols": [{"l": "A", "t": "text"}, {"l": "B", "t": "money"}],
             "rows": 2, "keys": [["a1", "b1"], ["a2", "b2"]]},
        ]}],
    });
    let layout = |keys: Value| json!({"parts": [{"n": "P", "items": [{"no": "", "l": "g", "grid": {"keys": keys}}]}]});
    let partial = std::panic::catch_unwind(|| {
        let mut f = form.clone();
        apply_layout(&mut f, &layout(json!([["a1", "b1"]])));
    })
    .expect_err("a partial claim stops the load");
    let message = partial
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(message.starts_with("TEST: "), "{message}");
    assert!(
        message.contains("names 2 of the 4 cells of table \"Schedule 9\""),
        "{message}"
    );
    let mut whole = form.clone();
    apply_layout(&mut whole, &layout(json!([["a1", "b1"], ["a2", "b2"]])));
    assert_eq!(
        whole["parts"][0]["items"][0]["keys"],
        json!([["a1", "b1"], ["a2", "b2"]])
    );
}

/// Review item 11: every Part II date cell is a whole MM/DD/YYYY date, the
/// rows the samples left blank too (the probe bound them as plain text): a
/// date typed or picked (the calendar sends ISO) is stored as MM/DD/YYYY,
/// and part of a date is refused, exactly as on a row the samples dated.
#[test]
fn pr6_every_part_ii_date_cell_keeps_a_whole_date() {
    setup();
    for (code, dated, blank, path) in [
        (
            "1604C",
            "frm1604c:txtSched1Date1",
            "frm1604c:txtSched1Date4",
            "/months/3/date",
        ),
        (
            "1604E",
            "frm1604e:txtSched2RemDate1",
            "frm1604e:txtSched2RemDate2",
            "/schedule2/1/date",
        ),
        (
            "1604F",
            "frm1604f:txtSched1Date1",
            "frm1604f:txtSched2Date1",
            "/schedules/1/0/date",
        ),
    ] {
        let def = kind_of(code).unwrap().definition();
        let dates: Vec<&String> = schema_keys(code)
            .iter()
            .filter(|k| k.contains(":txtSched") && k.contains("Date"))
            .collect();
        assert_eq!(dates.len(), if code == "1604E" { 16 } else { 12 }, "{code}");
        for key in dates {
            assert!(
                matches!(
                    def.bindings.get(key).map(|b| &b.binding),
                    Some(Binding::Text {
                        date: Some(agent::DateFormat::Mdy),
                        part: None,
                        ..
                    })
                ),
                "{code} {key} keeps a whole MM/DD/YYYY date"
            );
        }
        let opened = open(code, 0);
        for key in [dated, blank] {
            let typed = fill(code, &opened["draft"], json!({ key: "04/10/2026" }));
            assert_eq!(typed["rejected"], json!({}), "{code} {key}");
            let picked = fill(code, &opened["draft"], json!({ key: "2026-04-10" }));
            let partial = fill(code, &opened["draft"], json!({ key: "04/2026" }));
            assert!(
                partial["rejected"].get(key).is_some(),
                "{code} {key} refuses part of a date"
            );
            if key == blank {
                assert_eq!(
                    typed["draft"].pointer(path),
                    Some(&json!("04/10/2026")),
                    "{code}"
                );
                assert_eq!(
                    picked["draft"].pointer(path),
                    typed["draft"].pointer(path),
                    "{code} picked"
                );
            }
            assert_eq!(picked["values"][key], typed["values"][key], "{code} {key}");
        }
    }
}

/// Review item 12: no cell of an extracted table is lost; every one is drawn
/// in Part II (the oracle is app/forms/forms-schema.json, the extracted
/// schema before any override).
#[test]
fn pr6_every_extracted_table_cell_is_drawn() {
    let raw: Value =
        serde_json::from_str(include_str!("../../../../../app/forms/forms-schema.json")).unwrap();
    for (code, _) in FORMS {
        let form = raw
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["c"] == code)
            .unwrap();
        let cells: Vec<String> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["keys"].as_array().cloned().unwrap_or_default())
            .flat_map(|row| row.as_array().cloned().unwrap_or_default())
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        let expected = match code {
            "1604C" => 12 * 7,
            "1604F" => 3 * 4 * 5,
            _ => (4 + 12) * 6,
        };
        assert_eq!(cells.len(), expected, "{code}");
        let on_page = drawn(code);
        for cell in &cells {
            assert!(
                on_page
                    .iter()
                    .any(|(part, _, _, k, _)| k == cell && part.starts_with("Part II")),
                "{code} {cell} is drawn in Part II"
            );
        }
    }
}
