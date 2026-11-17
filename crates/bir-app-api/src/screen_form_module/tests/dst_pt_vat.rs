//! PR 4: the monthly DST, percentage tax and VAT returns (2000, 2553,
//! 2550M) against their printed sheets. The oracles are the frozen sheets'
//! own files (html-frozen/<slug>/writer-cells.json captions and joins,
//! index.html's printed text), the official keys, and hand lists from the
//! sheets; never this module's own refusal.

use super::*;

/// `(code, frozen sheet slug)` of the three forms.
const FORMS: [(&str, &str); 3] = [
    ("2000", "2000-dst-2018"),
    ("2553", "2553-1999"),
    ("2550M", "2550m-2007"),
];

fn writer_cells(slug: &str) -> Value {
    let path = format!(
        "{}/../../html-frozen/{slug}/writer-cells.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).expect("writer-cells"))
        .expect("writer-cells json")
}

/// The frozen sheet's printed words, one space between text runs.
fn sheet_text(slug: &str) -> String {
    let path = format!(
        "{}/../../html-frozen/{slug}/index.html",
        env!("CARGO_MANIFEST_DIR")
    );
    let html = std::fs::read_to_string(&path).expect("frozen sheet");
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

/// Every drawn box: `(part name, item no, item label, key, type)`, table
/// cells numbered by their row when the table starts at an item number.
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
            for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
                let row_no = item["start"]
                    .as_u64()
                    .map_or(String::new(), |start| (start + r as u64).to_string());
                for (c, key) in row.as_array().unwrap().iter().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] != "hidden" {
                        out.push((
                            name.clone(),
                            row_no.clone(),
                            col["l"].as_str().unwrap_or("").to_string(),
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

/// The printed item number a caption starts with (`7A`, `14C`, `Item 12A`).
fn printed_number(printed: &str) -> Option<String> {
    let printed = printed.strip_prefix("Item ").unwrap_or(printed);
    let head = printed.split_whitespace().next()?;
    let digits = head.trim_end_matches(|c: char| c.is_ascii_uppercase());
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && head.len() <= 3)
        .then(|| head.to_string())
}

/// The part and item a key is drawn under.
fn placed(code: &str, key: &str) -> (String, String, String) {
    drawn(code)
        .into_iter()
        .find(|(_, _, _, k, _)| k == key)
        .map(|(part, no, label, _, _)| (part, no, label))
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
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

/// The page's radio-group answer, the way form_page.js sends it: the
/// picked box true, the others false.
fn pick(keys: &[&str], picked: &str) -> Value {
    Value::Object(
        keys.iter()
            .map(|k| {
                (
                    k.to_string(),
                    json!(if *k == picked { "true" } else { "false" }),
                )
            })
            .collect(),
    )
}

const MODE: [&str; 3] = [
    "frm2000:optMode_1",
    "frm2000:optMode_2",
    "frm2000:optMode_3",
];

/// R3 (2000 R3.0, R3.1, manual-0, manual-2; 2553 R3.1, manual-0, manual-2;
/// 2550M manual-0): every box the sheet prints under an item number is
/// drawn under that number. The oracle is writer-cells.json's printed
/// captions; 2553's rows 14-18 are table rows numbered from 14.
#[test]
fn pr4_boxes_sit_under_their_printed_item_numbers() {
    for (code, slug) in FORMS {
        let cells = writer_cells(slug);
        let boxes = drawn(code);
        let mut checked = 0;
        for join in ["joins", "money_joins", "xbox_joins"]
            .iter()
            .flat_map(|list| cells[*list].as_array().into_iter().flatten())
        {
            let key = join["writer_key"].as_str().unwrap();
            let Some(number) = printed_number(join["printed"].as_str().unwrap()) else {
                continue;
            };
            let Some((_, no, _, _, _)) = boxes.iter().find(|(_, _, _, k, _)| k == key) else {
                continue;
            };
            // A row of 2553's table carries the row's number (14), the
            // caption the column's too (14C).
            assert!(
                *no == number || number.trim_end_matches(|c: char| c.is_ascii_uppercase()) == no,
                "{code} {key} prints at {:?}, drawn under {no:?}",
                join["printed"]
            );
            checked += 1;
        }
        assert!(checked >= 20, "{code}: {checked} boxes checked");
    }
    // 2000: item 1 holds the month and the year (manual-2); items 5, 8
    // and 9 are in Part I (manual-0, R3.1); 2550M item 1 too (manual-0).
    let one = |code: &str| -> Vec<String> {
        drawn(code)
            .into_iter()
            .filter(|(_, no, _, _, _)| no == "1")
            .map(|(_, _, _, k, _)| k)
            .collect()
    };
    assert_eq!(one("2000"), ["frm2000:txtMonth", "frm2000:txtYear"]);
    assert_eq!(one("2550M"), ["frm2550m:RtnYear", "frm2550m:txtYear"]);
    for (key, no) in [
        ("frm2000:txtRDOCode", "5"),
        ("frm2000:txtTelNum", "8"),
        ("txtEmail", "9"),
    ] {
        let (part, at, _) = placed("2000", key);
        assert_eq!(
            (part.as_str(), at.as_str()),
            ("Part I · Background Information", no),
            "2000 {key}"
        );
    }
    // No drawn box of the three forms sits in an unnumbered "Additional
    // details" group (2000 R3.1, R3.2).
    for (code, _) in FORMS {
        for (part, no, label, key, _) in drawn(code) {
            assert!(
                label != "Additional details",
                "{code} {key} in {part} item {no:?}"
            );
        }
    }
}

/// R3 (2000 R3.0, R3.4; 2553 and 2550M manual-4): Part III Details of
/// Payment shows each printed item with a note and no box. 2000's boxes
/// are disabled at load in the official program and this app never writes
/// them; 2553 and 2550M have no Part III control at all (each inventory
/// lists none).
#[test]
fn pr4_part_iii_shows_each_printed_item_without_a_box() {
    setup();
    let root = format!("{}/../..", env!("CARGO_MANIFEST_DIR"));
    for (code, slug, items, package) in [
        (
            "2000",
            "2000-dst-2018",
            ["20", "21", "22", "23"],
            "2000-v2018",
        ),
        ("2553", "2553-1999", ["26", "27", "28", "29"], "2553-v1999"),
        (
            "2550M",
            "2550m-2007",
            ["29", "30", "31", "32"],
            "2550m-v2007",
        ),
    ] {
        let form = schema_entry(code).unwrap();
        let part = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["n"] == "Part III · Details of Payment")
            .unwrap_or_else(|| panic!("{code} has a Part III"));
        let numbers: Vec<&str> = part["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|it| it["no"].as_str().unwrap())
            .collect();
        assert_eq!(numbers, items, "{code}");
        for item in part["items"].as_array().unwrap() {
            assert!(item["note"].as_str().is_some_and(|n| n.contains("no box")));
            assert!(
                item["f"]
                    .as_array()
                    .is_none_or(|f| f.iter().all(|f| f["t"] == "hidden")),
                "{code} item {}",
                item["no"]
            );
        }
        assert!(sheet_text(slug).contains("Details of Payment"), "{slug}");
        let inventory: Value = serde_json::from_str(
            &std::fs::read_to_string(format!(
                "{root}/rules/forms/{package}/fixtures/runtime-control-inventory-v796.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let controls = inventory["controls"]
            .as_array()
            .or(inventory["static_controls"].as_array())
            .unwrap();
        let part_iii: Vec<&Value> = controls
            .iter()
            .filter(|c| {
                c["id"].as_str().is_some_and(|id| {
                    [
                        "txtAgency",
                        "txtNumber2",
                        "txtDate2",
                        "txtAmount2",
                        "txtParticular",
                    ]
                    .iter()
                    .any(|stem| id.contains(stem))
                })
            })
            .collect();
        if code == "2000" {
            assert_eq!(part_iii.len(), 16 + 1, "2000's Part III controls");
            assert!(part_iii.iter().all(|c| c["disabled"] == true));
        } else {
            assert!(part_iii.is_empty(), "{code}: {part_iii:?}");
        }
    }
    // 2000's Part III boxes stay on the schema, hidden, and the print map
    // writes them blank, as the official program does.
    let opened = open("2000", 6);
    let map = printed("2000", &opened["draft"]);
    for n in 20..=23 {
        for stem in ["txtAgency", "txtNumber", "txtDate", "txtAmount"] {
            let key = format!("frm2000:{stem}{n}");
            assert!(schema_keys("2000").contains(&key), "{key}");
            assert!(!drawn("2000").iter().any(|(_, _, _, k, _)| *k == key));
            assert!(map.get(&key).is_none_or(|v| v.trim().is_empty()), "{key}");
        }
    }
}

/// 2000 R3.4: the tax agent's Accreditation No., Date of Issue and Date of
/// Expiry show their printed caption with a note after item 19 (where the
/// sheet prints them) and have no box: the official runtime disables them at
/// load and the print leaves them blank.
#[test]
fn pr4_form_2000_tax_agent_boxes_show_their_caption_and_a_note() {
    setup();
    let form = schema_entry("2000").unwrap();
    let part = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == "Part II · Computation of Payable")
        .unwrap();
    let last = part["items"].as_array().unwrap().last().unwrap();
    assert!(
        last["l"]
            .as_str()
            .unwrap()
            .starts_with("Tax Agent Accreditation No.")
    );
    assert!(last["note"].as_str().is_some_and(|n| n.contains("no box")));
    assert!(
        sheet_text("2000-dst-2018")
            .contains("Tax Agent Accreditation No./ Date of Issue Date of Expiry")
    );
    let opened = open("2000", 6);
    let map = printed("2000", &opened["draft"]);
    for key in ["txtTaxAgentNo", "txtDateIssue", "txtDateExpiry"] {
        assert!(schema_keys("2000").contains(key));
        assert!(
            !drawn("2000").iter().any(|(_, _, _, k, _)| k == key),
            "{key}"
        );
        assert!(map.get(key).is_none_or(|v| v.trim().is_empty()), "{key}");
    }
}

/// Everything the printed layout leaves out goes to one last part, "Not on
/// the printed sheet", and only these boxes of it are drawn: 2000's Line
/// of Business and DS106 question, 2553's email; 2550M draws none there.
#[test]
fn pr4_only_named_boxes_are_drawn_off_the_printed_sheet() {
    for (code, expected) in [
        (
            "2000",
            vec!["frm2000:txtLineBus", "ds106modYes", "ds106modNo"],
        ),
        ("2553", vec!["txtEmail"]),
        ("2550M", vec![]),
    ] {
        let off: Vec<String> = drawn(code)
            .into_iter()
            .filter(|(part, _, _, _, _)| part == "Not on the printed sheet")
            .map(|(_, _, _, k, _)| k)
            .collect();
        assert_eq!(off, expected, "{code}");
        let form = schema_entry(code).unwrap();
        let names: Vec<&str> = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["n"].as_str().unwrap())
            .collect();
        let last = names.last().unwrap();
        assert_eq!(*last, "Not on the printed sheet", "{code}: {names:?}");
    }
}

/// 2000 R2.2, preview.0, manual-0: Item 5 RDO Code is drawn, filled from
/// the profile and locked with Edit like the other profile boxes; the edit
/// stays on this return and prints in the sheet's item 5 cell
/// (writer-cells.json joins txtRDOCode at "5 RDO Code").
#[test]
fn pr4_form_2000_item_5_rdo_comes_from_the_profile_and_prints() {
    setup();
    let opened = open("2000", 6);
    assert_eq!(opened["values"]["frm2000:txtRDOCode"], "039");
    assert_eq!(
        opened["locks"]["frm2000:txtRDOCode"],
        agent::Lock::Profile.reason()
    );
    assert!(strings(&opened["profile"]).contains("frm2000:txtRDOCode"));
    assert!(strings(&opened["profile_edit"]).contains("frm2000:txtRDOCode"));
    assert!(
        opened["options"]["frm2000:txtRDOCode"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["value"] == "040")
    );
    let cells = writer_cells("2000-dst-2018");
    let cell = cells["joins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["writer_key"] == "frm2000:txtRDOCode")
        .unwrap();
    assert_eq!(cell["printed"], "5 RDO Code");
    let cell = cell["html_id"].as_str().unwrap();
    let html = document("2000", "2000-dst-2018", &opened["draft"]);
    assert_eq!(bir_print::frozen_html::printed_text(&html, cell), "039");
    // Without Edit, the box refuses; with it, the edit stays on the return.
    let refused = fill(
        "2000",
        &opened["draft"],
        json!({"frm2000:txtRDOCode": "040"}),
    );
    assert!(refused["rejected"].get("frm2000:txtRDOCode").is_some());
    let edited = fill_unlocked(
        "2000",
        &opened["draft"],
        json!({"frm2000:txtRDOCode": "040"}),
        &["frm2000:txtRDOCode"],
    );
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(edited["draft"]["rdo_code"], "040");
    let html = document("2000", "2000-dst-2018", &edited["draft"]);
    assert_eq!(bir_print::frozen_html::printed_text(&html, cell), "040");
    let profile = with_database(|db| {
        db.get_profile_by_tin(TIN)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(profile.rdo_code, "039", "the profile is never written");
}

/// 2000 R2.0: every box the profile fills is in its printed item (Part I)
/// or under Not on the printed sheet, offers Edit, and the page 2 header
/// copies of items 4 and 6 are not drawn while the print still writes the
/// header (writer-cells.json: "prints inside derived:page_header").
#[test]
fn pr4_form_2000_profile_boxes_are_in_their_items_and_open_with_edit() {
    setup();
    let opened = open("2000", 6);
    let editable = strings(&opened["profile_edit"]);
    for key in strings(&opened["profile"]) {
        let Some((part, no, _)) = drawn("2000")
            .into_iter()
            .find(|(_, _, _, k, _)| *k == key)
            .map(|(p, n, l, _, _)| (p, n, l))
        else {
            assert!(key.contains("Pg2"), "2000 {key} is drawn");
            continue;
        };
        assert!(editable.contains(&key), "2000 {key} offers Edit");
        assert!(
            (part == "Part I · Background Information" && !no.is_empty())
                || part == "Not on the printed sheet",
            "2000 {key} in {part} {no}"
        );
    }
    for key in [
        "frm2000:txtPg2TIN1",
        "frm2000:txtPg2TIN2",
        "frm2000:txtPg2TIN3",
        "frm2000:txtPg2BranchCode",
        "frm2000:txtPg2TaxpayerName",
    ] {
        assert!(schema_keys("2000").contains(key));
        assert!(
            !drawn("2000").iter().any(|(_, _, _, k, _)| k == key),
            "{key}"
        );
    }
    let map = printed("2000", &opened["draft"]);
    let header = &map["derived:page_header"];
    assert!(header.starts_with("12345678800000"), "{header}");
    let (part, no, label) = placed("2000", "frm2000:txtLineBus");
    assert_eq!(
        (part.as_str(), no.as_str()),
        ("Not on the printed sheet", "")
    );
    assert_eq!(label, "Line of Business");
    let cells = writer_cells("2000-dst-2018");
    assert!(cells["not_joined"].as_array().unwrap().iter().any(|e| {
        e["writer_key"] == "frm2000:txtLineBus"
            && e["reason"]
                .as_str()
                .unwrap()
                .contains("not on the paper form")
    }));
}

/// 2000 manual-1, R3.2: page 2's Schedules 1-4 are their own parts, each a
/// table of its printed columns in the sheet's order (the cells' left edges
/// in html-frozen/2000-dst-2018/index.html) with three printed rows, and
/// its total under it.
#[test]
fn pr4_form_2000_schedules_are_tables_of_their_printed_columns() {
    let html = std::fs::read_to_string(format!(
        "{}/../../html-frozen/2000-dst-2018/index.html",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let cells = writer_cells("2000-dst-2018");
    let html_id = |key: &str| -> String {
        ["joins", "money_joins", "date_joins"]
            .iter()
            .flat_map(|list| cells[*list].as_array().unwrap())
            .find(|j| j["writer_key"] == key)
            .and_then(|j| {
                ["html_id", "peso_html_id", "mm_html_id"]
                    .iter()
                    .find_map(|f| j[*f].as_str())
            })
            .unwrap_or_else(|| panic!("{key} is joined"))
            .to_string()
    };
    let left = |id: &str| -> f64 {
        let at = html.find(&format!("<div id=\"{id}\"")).unwrap();
        let tag = &html[at..at + html[at..].find('>').unwrap()];
        let x = tag.split("left:").nth(1).unwrap();
        x.split("pt").next().unwrap().parse().unwrap()
    };
    let form = schema_entry("2000").unwrap();
    for (n, part_name, total) in [
        (
            1,
            "Schedule 1 – Summary of Computation of Taxes Due for the Month",
            "frm2000:sched1:txtTotalDue1",
        ),
        (
            2,
            "Schedule 2 – Summary of Constructive Affixture for the Month",
            "frm2000:sched2:txtTotalPayment1",
        ),
        (
            3,
            "Schedule 3 – Summary of DST Payments/Purchases for the Month",
            "frm2000:sched3:txtTotalPayment1",
        ),
        (
            4,
            "Schedule 4 – Summary of Remittance from Collection on Sale of Loose Documentary Stamps",
            "frm2000:sched4:txtTotalRemittance1",
        ),
    ] {
        assert!(
            sheet_text("2000-dst-2018").contains(part_name.split(" – ").nth(1).unwrap()),
            "{part_name}"
        );
        let part = form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["n"] == part_name)
            .unwrap_or_else(|| panic!("{part_name}"));
        let table = &part["items"][0];
        assert_eq!(table["rows"], 3, "Schedule {n}");
        let row0: Vec<String> = table["keys"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap().to_string())
            .collect();
        let edges: Vec<f64> = row0.iter().map(|k| left(&html_id(k))).collect();
        assert!(
            edges.windows(2).all(|w| w[0] < w[1]),
            "Schedule {n} columns in printed order: {row0:?} {edges:?}"
        );
        for (r, row) in table["keys"].as_array().unwrap().iter().enumerate() {
            for key in row.as_array().unwrap() {
                let key = key.as_str().unwrap();
                assert!(key.ends_with(&r.to_string()), "Schedule {n} row {r}: {key}");
            }
        }
        assert_eq!(part["items"][1]["f"][0]["k"], total);
    }
}

/// 2000 preview.1, R4.0: Schedule 3 (eDST System's advance payments) is
/// typed on the page and fills item 15C; each of the nine schedule dates
/// takes a typed MM/DD/YYYY date once item 13 opens its schedule, and the
/// print shows the typed values in the sheet's cells (writer-cells.json
/// date_joins and money_joins). Closed, a schedule box says what opens it.
#[test]
fn pr4_form_2000_schedule_dates_take_typed_dates_once_item_13_opens_them() {
    setup();
    let cells = writer_cells("2000-dst-2018");
    let date_cells = |key: &str| -> Vec<String> {
        let join = cells["date_joins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["writer_key"] == key)
            .unwrap();
        ["mm_html_id", "dd_html_id", "yyyy_html_id"]
            .iter()
            .map(|f| join[*f].as_str().unwrap().to_string())
            .collect()
    };
    let amount_cell = |key: &str| -> String {
        cells["money_joins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["writer_key"] == key)
            .unwrap()["peso_html_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (schedule, mode, date_stem, typed_stem, amount_stem) in [
        (
            2,
            "frm2000:optMode_2",
            "sched2:txtPaymentDate",
            "sched2:txtReceipt",
            "sched2:txtAmountPaid",
        ),
        (
            3,
            "frm2000:optMode_1",
            "sched3:txtPaymentDate",
            "sched3:txtReceipt",
            "sched3:txtAmountPaid",
        ),
        (
            4,
            "frm2000:optMode_3",
            "sched4:txtRemittanceDate",
            "sched4:txtRCOCode",
            "sched4:txtAmountRemitted",
        ),
    ] {
        let key = |stem: &str, row: usize| format!("frm2000:{stem}{row}");
        let month = 6 + i64::from(schedule);
        let opened = open("2000", month);
        // Closed: refused, with what opens it.
        let closed = fill(
            "2000",
            &opened["draft"],
            json!({ key(date_stem, 0): "12/10/2020" }),
        );
        assert_eq!(
            closed["rejected"][key(date_stem, 0)],
            rows_dst_pt_vat::dst_schedule_closed(schedule),
            "Schedule {schedule}"
        );
        let open_mode = fill("2000", &opened["draft"], pick(&MODE, mode));
        assert_eq!(open_mode["rejected"], json!({}));
        let mut draft = open_mode["draft"].clone();
        for row in 0..3 {
            let filled = fill(
                "2000",
                &draft,
                json!({
                    key(date_stem, row): format!("12/1{row}/2020"),
                    key(typed_stem, row): format!("R{schedule}0{row}"),
                    key(amount_stem, row): "100.50",
                }),
            );
            assert_eq!(
                filled["rejected"],
                json!({}),
                "Schedule {schedule} row {row}"
            );
            assert_eq!(
                filled["values"][key(date_stem, row)],
                format!("12/1{row}/2020")
            );
            draft = filled["draft"].clone();
        }
        let shown = fill("2000", &draft, json!({}));
        let total = match schedule {
            2 => "frm2000:txtTax15B",
            3 => "frm2000:txtTax15C",
            _ => "frm2000:txtTax19",
        };
        assert_eq!(
            amount(&shown["values"], total),
            301.5,
            "Schedule {schedule}"
        );
        let html = document("2000", "2000-dst-2018", &draft);
        for row in 0..3 {
            let parts: Vec<String> = date_cells(&key(date_stem, row))
                .iter()
                .map(|cell| bir_print::frozen_html::printed_text(&html, cell))
                .collect();
            assert_eq!(
                parts,
                ["12", format!("1{row}").as_str(), "2020"],
                "Schedule {schedule} row {row}"
            );
            assert_eq!(
                bir_print::frozen_html::printed_text(&html, &amount_cell(&key(amount_stem, row))),
                "100",
                "Schedule {schedule} row {row}"
            );
        }
        save("2000", &draft);
        let reopened = open("2000", month);
        assert_eq!(reopened["values"][key(date_stem, 2)], "12/12/2020");
        assert_eq!(amount(&reopened["values"], total), 301.5);
    }
    // The nine schedule dates are date boxes of their tables.
    let dates: Vec<String> = drawn("2000")
        .into_iter()
        .filter(|(part, _, _, _, t)| part.starts_with("Schedule") && t == "date")
        .map(|(_, _, _, k, _)| k)
        .collect();
    assert_eq!(dates.len(), 9, "{dates:?}");
    // Their tables give each date box a calendar (form_page.js reads
    // `calendar`); no other form's table is marked (lesson 9) but 2552's
    // and 1707's tables with a date column (PR 9,
    // rows_as_needed::mark_calendar_tables).
    for form in schema() {
        for part in form["parts"].as_array().unwrap() {
            for item in part["items"].as_array().unwrap() {
                let marked = item["calendar"] == true;
                let schedule_2_to_4 = form["c"] == "2000"
                    && ["Schedule 2", "Schedule 3", "Schedule 4"]
                        .iter()
                        .any(|n| part["n"].as_str().unwrap().starts_with(n));
                let dated_pr9 = (form["c"] == "2552" || form["c"] == "1707")
                    && item["cols"]
                        .as_array()
                        .is_some_and(|cols| cols.iter().any(|col| col["t"] == "date"));
                assert_eq!(
                    marked,
                    (schedule_2_to_4 && item.get("rows").is_some()) || dated_pr9,
                    "{} {}",
                    form["c"],
                    part["n"]
                );
            }
        }
    }
    // Items 15B and 15C follow the schedules: computed, never typed.
    let opened = open("2000", 6);
    for key in ["frm2000:txtTax15B", "frm2000:txtTax15C"] {
        assert_eq!(opened["lock_notes"][key], NOTE_COMPUTED, "{key}");
    }
}

/// Lesson 8: an answer that removes what the filer typed asks first
/// (`removes`), and nothing typed asks nothing: 2000 item 13 and its
/// schedule rows, item 10 and items 11-12, a cleared Schedule 1 ATC with its
/// tax base; Amended Return No clearing 2000's 15A, 2553's 20A and 2550M's
/// 23D; 2553 row 18's cleared ATC.
#[test]
fn pr4_answers_that_remove_typed_entries_ask_first() {
    setup();
    let opened = open("2000", 6);
    let constructive =
        fill("2000", &opened["draft"], pick(&MODE, "frm2000:optMode_2"))["draft"].clone();
    assert_eq!(
        fill("2000", &constructive, pick(&MODE, "frm2000:optMode_1"))["removes"],
        Value::Null,
        "nothing typed, nothing asked"
    );
    let typed = fill(
        "2000",
        &constructive,
        json!({"frm2000:sched2:txtReceipt0": "R1", "frm2000:sched4:txtRCOCode0": "C1"}),
    );
    assert_eq!(typed["rejected"], json!({}));
    let edst = fill("2000", &typed["draft"], pick(&MODE, "frm2000:optMode_1"));
    assert_eq!(edst["draft"]["schedule2"], json!([]));
    assert_eq!(
        edst["removes"]["message"],
        "Changing item 13 removes the 1 Schedule 2 row and 1 Schedule 4 row you typed. Continue?"
    );
    assert_eq!(edst["removes"]["done"], "Removed 2 rows");
    // Item 10 None clears 11 and 12.
    let party = [
        "frm2000:optParty_1",
        "frm2000:optParty_2",
        "frm2000:optParty_3",
    ];
    let creditor = fill("2000", &opened["draft"], pick(&party, "frm2000:optParty_1"));
    let named = fill(
        "2000",
        &creditor["draft"],
        json!({"frm2000:txtOtherName": "Lender Corp", "frm2000:txtOtherTIN": "123456789000"}),
    );
    assert_eq!(named["rejected"], json!({}));
    let none = fill("2000", &named["draft"], pick(&party, "frm2000:optParty_3"));
    assert_eq!(none["draft"]["other_party_name"], "");
    assert_eq!(
        none["removes"]["message"],
        "Changing item 10 clears items 11 and 12 (the other party's name and TIN). Continue?"
    );
    // A cleared ATC with its tax base.
    let atc = fill(
        "2000",
        &opened["draft"],
        json!({"drpATCCode0": "catalog:0"}),
    );
    let based = fill(
        "2000",
        &atc["draft"],
        json!({"frm2000:sched1:txtTaxBase0": "1000"}),
    );
    let cleared = fill("2000", &based["draft"], json!({"drpATCCode0": ""}));
    assert_eq!(
        cleared["removes"]["message"],
        "Clearing this ATC removes its row and its amount of ₱1,000.00. Continue?"
    );
    // One batch clearing two rows states each row's amount, not a sum.
    let two = fill("2000", &based["draft"], json!({"drpATCCode1": "catalog:1"}));
    let two = fill(
        "2000",
        &two["draft"],
        json!({"frm2000:sched1:txtTaxBase1": "250"}),
    );
    assert_eq!(two["rejected"], json!({}));
    let both = fill(
        "2000",
        &two["draft"],
        json!({"drpATCCode0": "", "drpATCCode1": ""}),
    );
    assert_eq!(
        both["removes"]["message"],
        "Clearing these ATCs removes their rows and amounts of ₱1,000.00 and ₱250.00. Continue?"
    );
    assert_eq!(both["removes"]["done"], "Removed 2 rows");
    // Amended Return No clears the previous return's payment.
    for (code, period, yes, no, key, field, item) in [
        (
            "2000",
            6,
            "frm2000:AmendedRtn_1",
            "frm2000:AmendedRtn_2",
            "frm2000:txtTax15A",
            "balance_carried_over",
            "15A",
        ),
        (
            "2553",
            1,
            "frm2553:optAmended_1",
            "frm2553:optAmended_2",
            "frm2553:txt20A",
            "tax_paid_previous",
            "20A",
        ),
        (
            "2550M",
            3,
            "frm2550m:OptAmendedYN1",
            "frm2550m:OptAmendedYN2",
            "frm2550m:txtTax23D",
            "vat_paid_previous",
            "23D",
        ),
    ] {
        let opened = open(code, period);
        let amended = fill(code, &opened["draft"], pick(&[yes, no], yes));
        let typed = fill(code, &amended["draft"], json!({ key: "250" }));
        assert_eq!(typed["rejected"], json!({}), "{code}");
        assert_eq!(typed["draft"][field], 250.0, "{code}");
        let original = fill(code, &typed["draft"], pick(&[yes, no], no));
        assert_eq!(original["draft"][field], 0.0, "{code}");
        assert_eq!(
            original["removes"]["message"],
            format!("Changing Amended Return to No clears item {item} (₱250.00). Continue?"),
            "{code}"
        );
        let untyped = fill(code, &amended["draft"], pick(&[yes, no], no));
        assert_eq!(untyped["removes"], Value::Null, "{code}");
    }
    // 2550M Schedule 1: "Choose..." on a row removes its sales and output tax.
    let opened = open("2550M", 3);
    let atc = fill(
        "2550M",
        &opened["draft"],
        json!({"frm2550m:txtAtcCde1": "VB010"}),
    );
    let sold = fill(
        "2550M",
        &atc["draft"],
        json!({"frm2550m:txtAmountSales1": "5000"}),
    );
    assert_eq!(sold["rejected"], json!({}));
    let cleared = fill("2550M", &sold["draft"], json!({"frm2550m:txtAtcCde1": ""}));
    assert_eq!(cleared["draft"]["sales_schedule"], json!([]));
    assert_eq!(
        cleared["removes"]["message"],
        "Clearing this ATC removes its row and its amount of ₱5,000.00. Continue?"
    );
    assert_eq!(
        fill("2550M", &atc["draft"], json!({"frm2550m:txtAtcCde1": ""}))["removes"],
        Value::Null,
        "a row with no sales asks nothing"
    );
    // 2553 row 18's ATC cleared with its amount.
    let opened = open("2553", 1);
    let row = fill(
        "2553",
        &opened["draft"],
        json!({"frm2553:txt18B": "catalog:0"}),
    );
    let typed = fill("2553", &row["draft"], json!({"frm2553:txt18C": "2000"}));
    assert_eq!(typed["rejected"], json!({}));
    let cleared = fill("2553", &typed["draft"], json!({"frm2553:txt18B": ""}));
    assert_eq!(
        cleared["removes"]["message"],
        "Clearing row 18 removes its entry (OT010, ₱2,000.00). Continue?"
    );
}

/// 2553 manual-0, manual-1: Part II rows 14-18 are one table of the printed
/// columns (A classification, B ATC, C taxable amount, D rate, E tax due).
/// The sheet preprints rows 14-17's classification, ATC and rate as static
/// text next to the amounts it prints (the oracle:
/// html-frozen/2553-1999/index.html's text), so a new entry there is the
/// row's printed one: typing the amount gives the row that ATC (unless
/// another row holds it, then nothing is stored), its ATC and 5% rate are
/// shut, a rate never makes a row, and clearing the amount or the rate
/// clears the row after asking. A stored row holding another entry opens
/// unchanged, open, with a notice; it can be cleared (asking first) or set
/// to the printed entry. Row 18 is the free row. Validation is the form's.
#[test]
fn pr4_form_2553_rows_14_to_17_follow_the_printed_sheet() {
    setup();
    let sheet = sheet_text("2553-1999");
    for words in [
        "Philippine Amusement and OT 0 1 0 5 % Gaming Corp. (Pagcor) 14A",
        "Clark Development OT 0 1 1 5 % Corporations 15A",
        "Special/Regular Economic/ 5 % OT 0 1 1 Free Port Zone Enterprises 16A",
        "Others OT 0 1 2 17A",
    ] {
        assert!(sheet.contains(words), "{words}");
    }
    let opened = open("2553", 1);
    let waits = &opened["waits"];
    assert_eq!(
        waits["frm2553:txt14A"],
        "Printed: Philippine Amusement and Gaming Corp. (Pagcor), OT010, 5%"
    );
    assert_eq!(
        waits["frm2553:txt17A"],
        "Printed: Others, OT012, rate you enter"
    );
    assert_eq!(waits["frm2553:txt18A"], rows_dst_pt_vat::ATC_18_WAITS);
    for (row, atc) in [(14, "OT010"), (15, "OT011"), (16, "OT011"), (17, "OT012")] {
        assert_eq!(
            waits[format!("frm2553:txt{row}B")],
            format!("Printed: {atc}")
        );
        assert_eq!(
            opened["locks"][format!("frm2553:txt{row}B")],
            rows_dst_pt_vat::PRINTED_LOCK,
            "row {row}'s ATC is the printed one"
        );
        assert!(opened["locks"].get(format!("frm2553:txt{row}C")).is_none());
    }
    for row in [14, 15, 16] {
        assert_eq!(waits[format!("frm2553:txt{row}D")], "Printed: 5%");
        assert_eq!(
            opened["locks"][format!("frm2553:txt{row}D")],
            rows_dst_pt_vat::PRINTED_LOCK
        );
    }
    assert!(
        opened["locks"].get("frm2553:txt17D").is_none(),
        "Others' rate is typed"
    );
    for col in ["B", "C", "D"] {
        assert!(
            opened["locks"].get(format!("frm2553:txt18{col}")).is_none(),
            "row 18 is free"
        );
    }
    for row in 14..=18 {
        assert_eq!(
            opened["locks"][format!("frm2553:txt{row}A")],
            rows_dst_pt_vat::CLASSIFICATION_LOCK
        );
    }
    // The table.
    let form = schema_entry("2553").unwrap();
    let part = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == "Part II · Computation of Tax")
        .unwrap();
    let rows: Vec<Value> = part["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|it| it.get("rows").is_some())
        .flat_map(|it| it["keys"].as_array().unwrap().clone())
        .collect();
    let expected: Vec<Value> = (14..=18)
        .map(|n| json!(["A", "B", "C", "D", "E"].map(|c| format!("frm2553:txt{n}{c}"))))
        .collect();
    assert_eq!(rows, expected);
    // A blank draft: typing 15C gives row 15 its printed entry.
    let filled = fill("2553", &opened["draft"], json!({"frm2553:txt15C": "10000"}));
    assert_eq!(filled["rejected"], json!({}));
    let v = &filled["values"];
    assert_eq!(v["frm2553:txt15A"], "CLARK DEVELOPMENT CORPORATIONS");
    assert_eq!(v["frm2553:txt15B"], "OT011");
    assert_eq!(v["frm2553:txt15D"], "5.0");
    assert_eq!(amount(v, "frm2553:txt15E"), 500.0);
    assert_eq!(amount(v, "frm2553:txt19"), 500.0);
    assert!(notices(&filled["draft"]).is_empty());
    // The print: row 15's amount prints next to the sheet's own
    // "Clark Development ... OT 0 1 1 5 %" text, the entry the draft holds.
    let html = document("2553", "2553-1999", &filled["draft"]);
    let cell = |key: &str| -> String {
        writer_cells("2553-1999")["joins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["writer_key"] == key)
            .unwrap()["html_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        bir_print::frozen_html::printed_text(&html, &cell("frm2553:txt15C")),
        "10,000.00"
    );
    assert!(sheet_text("2553-1999").contains("Clark Development OT 0 1 1 5 % Corporations 15A"));
    assert_eq!(filled["draft"]["schedule"][1]["atc_code"], "OT011");
    // The official rate of a 5% row is not typed.
    let rate = fill("2553", &filled["draft"], json!({"frm2553:txt15D": "3"}));
    assert!(
        rate["rejected"]["frm2553:txt15D"]
            .as_str()
            .unwrap()
            .contains("official rate of 5.0%")
    );
    // A rate never makes a row.
    let early = fill("2553", &opened["draft"], json!({"frm2553:txt17D": "2.5"}));
    assert_eq!(
        early["rejected"]["frm2553:txt17D"],
        "Type the taxable amount in 17C first."
    );
    assert_eq!(early["draft"]["schedule"], json!([]));
    // Row 17: C then D; clearing D, like clearing C, clears the row, asking.
    let others = fill(
        "2553",
        &filled["draft"],
        json!({"frm2553:txt17C": "2000", "frm2553:txt17D": "2.5"}),
    );
    assert_eq!(others["rejected"], json!({}));
    assert_eq!(others["values"]["frm2553:txt17B"], "OT012");
    assert_eq!(amount(&others["values"], "frm2553:txt17E"), 50.0);
    for key in ["frm2553:txt17C", "frm2553:txt17D"] {
        let cleared = fill("2553", &others["draft"], json!({ key: "" }));
        assert_eq!(cleared["rejected"], json!({}), "{key}");
        assert_eq!(cleared["values"]["frm2553:txt17B"], "", "{key}");
        assert_eq!(
            cleared["removes"]["message"],
            "Clearing row 17 removes its entry (OT012, ₱2,000.00). Continue?",
            "{key}"
        );
        assert_eq!(cleared["removes"]["done"], "Removed row 17");
    }
    // One batch clearing two rows states each row's amount.
    let both = fill(
        "2553",
        &others["draft"],
        json!({"frm2553:txt15C": "", "frm2553:txt17C": ""}),
    );
    assert_eq!(
        both["removes"]["message"],
        "Clearing rows 15 and 17 removes their entries (OT011, ₱10,000.00; OT012, ₱2,000.00). Continue?"
    );
    // Row 18 holds OT010: row 14's amount is refused and not stored; row
    // 18's own list refuses an entry another row holds, in the same words.
    let pagcor = fill(
        "2553",
        &opened["draft"],
        json!({"frm2553:txt18B": "catalog:0", "frm2553:txt18C": "100"}),
    );
    assert_eq!(pagcor["rejected"], json!({}));
    assert_eq!(pagcor["values"]["frm2553:txt18A"], "PAGCOR");
    let taken = fill("2553", &pagcor["draft"], json!({"frm2553:txt14C": "1000"}));
    assert_eq!(
        taken["rejected"]["frm2553:txt14C"],
        "Choose this row's ATC; OT010 is already on row 18."
    );
    assert_eq!(taken["values"]["frm2553:txt14B"], "");
    assert_eq!(taken["values"]["frm2553:txt14C"], "0.00");
    assert_eq!(taken["draft"]["schedule"].as_array().unwrap().len(), 5);
    assert_eq!(taken["draft"]["schedule"][0]["taxable_amount"], 0.0);
    let duplicate = fill(
        "2553",
        &others["draft"],
        json!({"frm2553:txt18B": "catalog:1"}),
    );
    assert_eq!(
        duplicate["rejected"]["frm2553:txt18B"],
        "Choose this row's ATC; OT012 is already on row 17."
    );
    // A stored row 14 holding OT020 opens unchanged, open, with a notice
    // (its amount would print under the Pagcor caption); it can be set to
    // the printed entry or cleared, asking first, and nothing else.
    let mut legacy = opened["draft"].clone();
    legacy["schedule"] = json!([
        {"atc_code": "OT020", "description": "OLD ENTRY", "taxable_amount": 1000.0, "tax_rate": 5.0, "tax_due": 50.0}
    ]);
    save("2553", &legacy);
    let reopened = open("2553", 1);
    assert_eq!(reopened["draft"]["schedule"][0]["atc_code"], "OT020");
    assert_eq!(reopened["draft"]["schedule"][0]["description"], "OLD ENTRY");
    assert_eq!(reopened["values"]["frm2553:txt14B"], "OT020");
    assert_eq!(reopened["values"]["frm2553:txt14C"], "1,000.00");
    assert!(reopened["locks"].get("frm2553:txt14B").is_none());
    let notice = "Row 14 prints OT010 (Pagcor); your OT020 entry won't print on this row. Clear it or move it to row 18.";
    assert!(
        reopened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"] == notice && e["keys"] == json!(["frm2553:txt14B"])),
        "{}",
        reopened["errors"]
    );
    let checked = call_async(
        "screen_form_validate",
        vec!["2553".into(), json(reopened["draft"].clone())],
    )
    .unwrap();
    assert!(
        checked["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"] == notice)
    );
    let other = fill(
        "2553",
        &reopened["draft"],
        json!({"frm2553:txt14B": "catalog:1"}),
    );
    assert_eq!(
        other["rejected"]["frm2553:txt14B"],
        "Row 14 prints OT010 (Pagcor); use row 18 for OT012."
    );
    let changed = fill(
        "2553",
        &reopened["draft"],
        json!({"frm2553:txt14B": "catalog:0"}),
    );
    assert_eq!(changed["rejected"], json!({}));
    assert_eq!(changed["draft"]["schedule"][0]["atc_code"], "OT010");
    assert_eq!(changed["draft"]["schedule"][0]["taxable_amount"], 1000.0);
    assert!(notices(&changed["draft"]).is_empty());
    let gone = fill("2553", &reopened["draft"], json!({"frm2553:txt14B": ""}));
    assert_eq!(gone["rejected"], json!({}));
    assert_eq!(gone["values"]["frm2553:txt14B"], "");
    assert_eq!(
        gone["removes"]["message"],
        "Clearing row 14 removes its entry (OT020, ₱1,000.00). Continue?"
    );
    // A stored draft with Others on row 15 (the official popup let it)
    // opens as it is, keeps its amount on that row when retyped, and is
    // reported, never printed silently under the Clark caption.
    let mut stored = gone["draft"].clone();
    stored["schedule"] = json!([
        {"atc_code": "", "description": "", "taxable_amount": 0.0, "tax_rate": 0.0, "tax_due": 0.0},
        {"atc_code": "OT012", "description": "OTHERS", "taxable_amount": 1000.0, "tax_rate": 3.5, "tax_due": 35.0}
    ]);
    save("2553", &stored);
    let reopened = open("2553", 1);
    assert_eq!(reopened["values"]["frm2553:txt15B"], "OT012");
    assert_eq!(reopened["draft"]["schedule"][1]["taxable_amount"], 1000.0);
    assert!(reopened["locks"].get("frm2553:txt15B").is_none());
    assert!(reopened["locks"].get("frm2553:txt15D").is_none());
    assert!(reopened["errors"].as_array().unwrap().iter().any(|e| e["message"]
        == "Row 15 prints OT011 (Clark Development Corporations); your OT012 entry won't print on this row. Clear it or move it to row 18."));
    let retyped = fill(
        "2553",
        &reopened["draft"],
        json!({"frm2553:txt15C": "2000"}),
    );
    assert_eq!(retyped["draft"]["schedule"][1]["atc_code"], "OT012");
    assert_eq!(amount(&retyped["values"], "frm2553:txt15E"), 70.0);
    let html = document("2553", "2553-1999", &retyped["draft"]);
    assert_eq!(
        bir_print::frozen_html::printed_text(&html, &cell("frm2553:txt15C")),
        "2,000.00"
    );
    assert_eq!(
        notices(&retyped["draft"]).len(),
        1,
        "the misplaced row stays reported"
    );
}

/// The page notices the host adds to a 2553 draft's messages.
fn notices(draft: &Value) -> Vec<Value> {
    rows_dst_pt_vat::notices(kind_of("2553").unwrap(), draft)
}

/// 2553 R6.0, manual-3: item 13's list is off while item 13 is No (rule 6:
/// faded, "Needs item 13 = Yes"; fields.json `enabled_when` "Item 13 Yes is
/// selected."), opens blank on Yes, names the relief, and prints it; its
/// raw 0 is never a choice. A stored Special Rate opens as it is.
#[test]
fn pr4_form_2553_item_13_list_waits_for_yes() {
    setup();
    let opened = open("2553", 1);
    assert_eq!(opened["off"]["frm2553:lstTaxTreaty"], "Needs item 13 = Yes");
    let field = drawn("2553")
        .into_iter()
        .find(|(_, _, _, k, _)| k == "frm2553:lstTaxTreaty")
        .unwrap();
    assert_eq!(field.4, "sel");
    let schema_field = schema_entry("2553").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
        .find(|f| f["k"] == "frm2553:lstTaxTreaty")
        .unwrap();
    assert_eq!(
        schema_field["o"],
        json!([{"value": "1", "label": "Special Rate"}, {"value": "2", "label": "International Tax Treaty"}])
    );
    assert_eq!(
        placed("2553", "frm2553:optTreaty_1").2,
        "Are you availing of tax relief under Special Law or International Tax Treaty?"
    );
    assert!(sheet_text("2553-1999").contains(
        "13 Are you availing of tax relief under Special Law or International Tax Treaty? Yes No If yes, specify"
    ));
    // No: the list takes nothing.
    let ignored = fill(
        "2553",
        &opened["draft"],
        json!({"frm2553:lstTaxTreaty": "1"}),
    );
    assert_eq!(ignored["draft"]["tax_treaty"], "no");
    let yes_keys = ["frm2553:optTreaty_1", "frm2553:optTreaty_2"];
    let yes = fill(
        "2553",
        &opened["draft"],
        pick(&yes_keys, "frm2553:optTreaty_1"),
    );
    assert_eq!(yes["draft"]["tax_treaty"], "yes_unspecified");
    assert_eq!(yes["values"]["frm2553:optTreaty_1"], "true");
    assert_eq!(yes["values"]["frm2553:lstTaxTreaty"], "0");
    let checked = call_async(
        "screen_form_validate",
        vec!["2553".into(), json(yes["draft"].clone())],
    )
    .unwrap();
    assert!(checked["off"].get("frm2553:lstTaxTreaty").is_none());
    let treaty = fill("2553", &yes["draft"], json!({"frm2553:lstTaxTreaty": "2"}));
    assert_eq!(treaty["draft"]["tax_treaty"], "international_tax_treaty");
    let map = printed("2553", &treaty["draft"]);
    assert_eq!(map["frm2553:lstTaxTreaty"], "2");
    assert_eq!(
        map["derived:line:tax_relief_specify"],
        "INTERNATIONAL TAX TREATY"
    );
    assert_eq!(map["frm2553:optTreaty_1"], "true");
    let no = fill(
        "2553",
        &treaty["draft"],
        pick(&yes_keys, "frm2553:optTreaty_2"),
    );
    assert_eq!(
        no["removes"]["message"],
        "Changing item 13 to No clears the relief chosen under If yes, specify (International Tax Treaty). Continue?"
    );
    assert_eq!(no["removes"]["done"], "Cleared item 13's relief");
    let unchosen = fill(
        "2553",
        &yes["draft"],
        pick(&yes_keys, "frm2553:optTreaty_2"),
    );
    assert_eq!(
        unchosen["removes"],
        Value::Null,
        "nothing chosen, nothing asked"
    );
    assert_eq!(no["draft"]["tax_treaty"], "no");
    assert_eq!(no["values"]["frm2553:lstTaxTreaty"], "0");
    // A stored Special Rate opens as it is.
    let mut legacy = opened["draft"].clone();
    legacy["tax_treaty"] = json!("special_rate");
    save("2553", &legacy);
    let reopened = open("2553", 1);
    assert_eq!(reopened["draft"]["tax_treaty"], "special_rate");
    assert_eq!(reopened["values"]["frm2553:lstTaxTreaty"], "1");
    assert_eq!(reopened["values"]["frm2553:optTreaty_1"], "true");
}

/// 2553 preview.0: the quarter boxes are the return's own period (ticked
/// from the draft's quarter on the page and the print alike, every
/// quarter, shut as the return's identity), and item 23's boxes are the
/// draft's overpayment choice, chosen on the page once item 23 is below
/// zero.
#[test]
fn pr4_form_2553_quarter_and_item_23_boxes_follow_the_draft() {
    setup();
    let quarters: Vec<String> = (1..=4).map(|q| format!("frm2553:optQtr:_{q}")).collect();
    for quarter in 1..=4 {
        let opened = open("2553", quarter);
        let map = printed("2553", &opened["draft"]);
        for (i, key) in quarters.iter().enumerate() {
            let ticked = if i as i64 + 1 == quarter {
                "true"
            } else {
                "false"
            };
            assert_eq!(opened["values"][key], ticked, "Q{quarter} {key}");
            assert_eq!(map[key], ticked, "Q{quarter} {key}");
            assert_eq!(opened["lock_notes"][key], NOTE_IDENTITY, "Q{quarter} {key}");
        }
    }
    let opened = open("2553", 1);
    let refused = fill(
        "2553",
        &opened["draft"],
        json!({"frm2553:ifoverpay_1": "true", "frm2553:ifoverpay_2": "false"}),
    );
    assert_eq!(
        refused["rejected"]["frm2553:ifoverpay_1"],
        rows_dst_pt_vat::NOT_OVERPAID
    );
    assert!(opened["locks"].get("frm2553:ifoverpay_1").is_none());
    let overpaid = fill(
        "2553",
        &opened["draft"],
        json!({"frm2553:txt14C": "1000", "frm2553:txt20B": "5000"}),
    );
    assert!(amount(&overpaid["values"], "frm2553:txt23") < 0.0);
    let refund = fill(
        "2553",
        &overpaid["draft"],
        json!({"frm2553:ifoverpay_1": "false", "frm2553:ifoverpay_2": "true"}),
    );
    assert_eq!(refund["rejected"], json!({}));
    assert_eq!(refund["draft"]["overpayment"], "tax_credit_certificate");
    let map = printed("2553", &refund["draft"]);
    for key in ["frm2553:ifoverpay_1", "frm2553:ifoverpay_2"] {
        assert_eq!(map[key], refund["values"][key].as_str().unwrap(), "{key}");
    }
    assert_eq!(map["frm2553:ifoverpay_2"], "true");
}

/// 2550M manual-2: item 11 shows exactly what the draft holds, for every
/// answer the draft can hold, as the print does; Yes with International
/// Tax Treaty is what a draft holding that relief shows.
#[test]
fn pr4_form_2550m_item_11_shows_what_the_draft_holds() {
    setup();
    let opened = open("2550M", 3);
    let keys = [
        "frm2550m:OptSpecialTax1",
        "frm2550m:OptSpecialTax2",
        "frm2550m:lstSpecialTax",
    ];
    for (relief, yes, list) in [
        ("no", "false", "0"),
        ("yes_unspecified", "true", "0"),
        ("special_rate", "true", "1"),
        ("international_tax_treaty", "true", "2"),
    ] {
        let mut draft = opened["draft"].clone();
        draft["tax_relief"] = json!(relief);
        let shown = fill("2550M", &draft, json!({}));
        let map = printed("2550M", &shown["draft"]);
        for key in keys {
            assert_eq!(
                shown["values"][key].as_str().unwrap(),
                map[key],
                "{relief} {key}"
            );
        }
        assert_eq!(shown["values"]["frm2550m:OptSpecialTax1"], yes, "{relief}");
        assert_eq!(shown["values"]["frm2550m:lstSpecialTax"], list, "{relief}");
    }
    let treaty = fill(
        "2550M",
        &opened["draft"],
        json!({"frm2550m:lstSpecialTax": "2"}),
    );
    assert_eq!(treaty["draft"]["tax_relief"], "international_tax_treaty");
    assert_eq!(treaty["values"]["frm2550m:OptSpecialTax1"], "true");
    assert_eq!(opened["draft"]["tax_relief"], "no");
    assert_eq!(opened["values"]["frm2550m:OptSpecialTax2"], "true");
}

/// 2550M preview.0: Schedule 1's row 4 ATC is the draft's fourth sales row,
/// written through the page (the host's popup-order fill) and printed the
/// same: never a constant.
#[test]
fn pr4_form_2550m_row_4_atc_is_the_drafts() {
    setup();
    let opened = open("2550M", 3);
    assert!(opened["locks"].get("frm2550m:txtAtcCde4").is_none());
    let mut draft = opened["draft"].clone();
    let codes: Vec<&str> = bir_core::forms::form_2550m::FORM_2550M_ATCS
        .iter()
        .take(4)
        .map(|(code, _)| *code)
        .collect();
    for (n, code) in codes.iter().enumerate() {
        let filled = fill(
            "2550M",
            &draft,
            json!({ format!("frm2550m:txtAtcCde{}", n + 1): code }),
        );
        assert_eq!(filled["rejected"], json!({}), "{code}");
        draft = filled["draft"].clone();
    }
    let shown = fill("2550M", &draft, json!({}));
    assert_eq!(draft["sales_schedule"][3]["atc_code"], codes[3]);
    assert_eq!(shown["values"]["frm2550m:txtAtcCde4"], codes[3]);
    let map = printed("2550M", &draft);
    assert_eq!(map["frm2550m:txtAtcCde4"], codes[3]);
    let html = document("2550M", "2550m-2007", &draft);
    let cell = writer_cells("2550m-2007")["joins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["writer_key"] == "frm2550m:txtAtcCde4")
        .unwrap()["html_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(bir_print::frozen_html::printed_text(&html, &cell), codes[3]);
}

/// 2550M manual-3: page 1 prints the TIN and branch in item 4's combs
/// (bir-print's Rust-side SHEET_JOINS; the frozen writer-cells.json leaves
/// them out) and items 5 and 10 one digit per box (their writer-cells.json
/// joins); the print fill's own test is bir-print's
/// filled_document_2550m_prints_the_tin_and_one_character_per_box).
#[test]
fn pr4_form_2550m_print_carries_the_tin_rdo_and_zip_of_the_draft() {
    setup();
    let opened = open("2550M", 3);
    let html = document("2550M", "2550m-2007", &opened["draft"]);
    let cells = writer_cells("2550m-2007");
    let cell = |key: &str| -> String {
        cells["joins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|j| j["writer_key"] == key)
            .unwrap()["html_id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (key, shown) in [
        ("frm2550m:txtRDOCode", "039"),
        ("frm2550m:txtZipCode", "1100"),
    ] {
        assert_eq!(opened["values"][key], shown, "{key}");
        assert_eq!(
            bir_print::frozen_html::printed_text(&html, &cell(key)),
            shown,
            "{key}"
        );
    }
    // Item 4's combs, left to right on the sheet's item 4 row (data-row 7
    // of html-frozen/2550m-2007/index.html), which the frozen writer-cells
    // leaves unjoined (frozen_html.rs SHEET_JOINS joins them).
    for (key, comb, shown) in [
        ("frm2550m:txtTIN1", "p1c127", "123"),
        ("frm2550m:txtTIN2", "p1c9", "456"),
        ("frm2550m:txtTIN3", "p1c11", "788"),
        ("frm2550m:txtBranchCode", "p1c13", "00000"),
    ] {
        assert_eq!(opened["values"][key], shown, "{key}");
        assert_eq!(
            bir_print::frozen_html::printed_text(&html, comb),
            shown,
            "{key}"
        );
    }
    let mut branch = opened["draft"].clone();
    branch["tin"] = json!("12345678800001");
    let html = document("2550M", "2550m-2007", &branch);
    assert_eq!(
        bir_print::frozen_html::printed_text(&html, "p1c13"),
        "00001"
    );
}

/// Labels (2553 R3.0, R3.1; 2550M manual-0, manual-1; 2000 R3.3): options
/// read as printed (writer-cells.json xbox captions), no heading ends with
/// its own item number, no two Part II items of 2550M share a caption,
/// and each schedule total sits under its own schedule.
#[test]
fn pr4_labels_read_as_printed() {
    let field_label = |code: &str, key: &str| -> String {
        schema_entry(code).unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
            .find(|f| f["k"] == key)
            .unwrap()["l"]
            .as_str()
            .unwrap()
            .to_string()
    };
    for (code, slug) in FORMS {
        let cells = writer_cells(slug);
        for join in cells["xbox_joins"].as_array().unwrap() {
            let key = join["writer_key"].as_str().unwrap();
            let printed = join["printed"].as_str().unwrap();
            if !drawn(code).iter().any(|(_, _, _, k, _)| k == key) {
                continue;
            }
            let label = field_label(code, key);
            // 2553's quarter boxes print as "1st" to "4th" (checked below).
            if key.starts_with("frm2553:optQtr:_") {
                continue;
            }
            assert!(
                printed.ends_with(&label) || printed.to_lowercase().contains(&label.to_lowercase()),
                "{code} {key}: {label:?} is not the printed {printed:?}"
            );
        }
        for (_, no, label, key, _) in drawn(code) {
            assert!(
                no.is_empty() || !label.ends_with(&format!(" {no}")),
                "{code} {key}: {label}"
            );
        }
    }
    // 2000 R3.3: "eDST System" is the printed option.
    assert_eq!(field_label("2000", "frm2000:optMode_1"), "eDST System");
    assert!(sheet_text("2000-dst-2018").contains("13 Mode of Affixture eDST System"));
    // 2553: the options name only their answer.
    for (key, label) in [
        ("frm2553:optAmended_1", "Yes"),
        ("frm2553:optAmended_2", "No"),
        ("frm2553:optTreaty_1", "Yes"),
        ("frm2553:optTreaty_2", "No"),
        ("frm2553:ifoverpay_1", "To be Refunded"),
        (
            "frm2553:ifoverpay_2",
            "To be issued a Tax Credit Certificate",
        ),
    ] {
        assert_eq!(field_label("2553", key), label);
    }
    assert!(sheet_text("2553-1999").contains(
        "Quarter 4 Amended Return? 5 Number of sheets attached 3 2 Year Ended ( MM / YYYY ) 1st 2nd 3rd 4th"
    ));
    for (q, label) in ["1st", "2nd", "3rd", "4th"].iter().enumerate() {
        assert_eq!(
            field_label("2553", &format!("frm2553:optQtr:_{}", q + 1)),
            *label
        );
    }
    // 2553 manual-2: items 20A-20C and 22A-22D each under their printed
    // number, the penalties drawn as peso amounts.
    for (key, no) in [
        ("frm2553:txt20A", "20A"),
        ("frm2553:txt20B", "20B"),
        ("frm2553:txt20C", "20C"),
        ("frm2553:txt22A", "22A"),
        ("frm2553:txt22B", "22B"),
        ("frm2553:txt22C", "22C"),
        ("frm2553:txt22D", "22D"),
    ] {
        let (_, at, _, _, t) = drawn("2553")
            .into_iter()
            .find(|(_, _, _, k, _)| k == key)
            .unwrap();
        assert_eq!((at.as_str(), t.as_str()), (no, "money"), "{key}");
    }
    assert_eq!(
        placed("2553", "frm2553:txt23").2,
        "Total Amount Payable/(Overpayment) (Sum of Items 21 & 22D); if overpayment, mark one box only"
    );
    // 2550M item 1's boxes say what they hold (the binding: /month, /taxable_year).
    let def = kind_of("2550M").unwrap().definition();
    for (key, label, leaf) in [
        ("frm2550m:RtnYear", "Month", "/month"),
        ("frm2550m:txtYear", "Year", "/taxable_year"),
    ] {
        assert_eq!(field_label("2550M", key), label);
        assert!(matches!(
            &def.bindings[key].binding,
            Binding::Whole { path, .. } if path == leaf
        ));
    }
    // 2550M Part II: one caption per item, each from the sheet.
    // Compared without spaces: the sheet's text runs split and join words
    // unevenly ("20E )", "Item19").
    let squeeze = |text: &str| text.split_whitespace().collect::<String>();
    let sheet = squeeze(&sheet_text("2550m-2007"));
    let part = schema_entry("2550M").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == "Part II · Computation of Tax (Attach additional sheets, if necessary)")
        .unwrap()
        .clone();
    let mut seen = BTreeSet::new();
    for item in part["items"].as_array().unwrap() {
        let label = item["l"].as_str().unwrap();
        assert!(seen.insert(label.to_string()), "2550M: {label} twice");
        assert!(
            !["Input tax", "Others", "Total"].contains(&label),
            "{label}"
        );
        // The caption's last clause is printed on the sheet as it is.
        let tail = label.rsplit(": ").next().unwrap();
        assert!(
            sheet.contains(&squeeze(tail))
                || tail.starts_with("Others")
                || tail == "Purchases"
                || tail == "Sales/Receipts"
                // 25D prints without a caption; it is the penalties' total
                // (form_2550m.rs `total_penalties`).
                || (item["no"] == "25D" && tail == "Total Penalties"),
            "2550M {}: {tail:?} is not printed",
            item["no"]
        );
    }
    for (total, schedule) in [
        ("frm2550M:txtmodaltxtTotal12A", "Schedule 1"),
        ("txtmodalTotalAmt", "Schedule 2"),
        ("txtmodalTotalBalanceSched3A", "Schedule 3"),
        ("txtmodalTotalInputTax20ASched3", "Schedule 3"),
        ("txtTotal20BSched4", "Schedule 4"),
        ("txtTotal20CSched5", "Schedule 5"),
        ("txtmodalTotal23A", "Schedule 6"),
        ("txtmodalTotal23B", "Schedule 7"),
        ("txtmodalTotal23C", "Schedule 8"),
    ] {
        let (part, _, label) = placed("2550M", total);
        assert!(
            part.starts_with(&format!("{schedule} –")),
            "{total} in {part}"
        );
        assert_ne!(label, "Schedule amount", "{total}");
    }
    assert!(
        !drawn("2550M")
            .iter()
            .any(|(_, _, label, _, _)| label == "Schedule amount")
    );
}
