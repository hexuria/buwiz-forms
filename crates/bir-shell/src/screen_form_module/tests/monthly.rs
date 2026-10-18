//! PR 3: the monthly withholding forms (0619E, 0619F, 1600PT, 1600VT,
//! 1600WP, 1601C). The oracles are the printed sheets
//! (html-frozen/<slug>/writer-cells.json captions and index.html text), the
//! official runtime inventory and fields.json under rules/forms (read
//! only), the typed drafts' own setters, and hand lists.

use super::*;

/// `(code, frozen sheet slug)` of the six forms.
const MONTHLY: [(&str, &str); 6] = [
    ("0619E", "0619e-2018"),
    ("0619F", "0619f-2018"),
    ("1600PT", "1600-pt-2018"),
    ("1600VT", "1600-vt-2018"),
    ("1600WP", "1600wp-2010"),
    ("1601C", "1601c-2018"),
];

fn root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!("{}/{path}", root()))
            .unwrap_or_else(|e| panic!("{path}: {e}")),
    )
    .unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn writer_cells(slug: &str) -> Value {
    read_json(&format!("html-frozen/{slug}/writer-cells.json"))
}

/// Every join of a sheet: `(writer key, printed caption)`.
fn joins(slug: &str) -> Vec<(String, String)> {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins", "date_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .map(|join| {
            (
                join["writer_key"].as_str().unwrap().to_string(),
                join["printed"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

/// The frozen sheet's printed words, one space between text runs.
fn sheet_text(slug: &str) -> String {
    let html = std::fs::read_to_string(format!("{}/html-frozen/{slug}/index.html", root()))
        .expect("frozen sheet");
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

/// One box the page draws.
#[derive(Debug, Clone)]
struct Drawn {
    part: String,
    no: String,
    item: String,
    key: String,
    label: String,
    t: String,
    /// A schedule table's cell (its row number is the item number shown).
    cell: bool,
}

/// Every drawn box of a form, fields and table cells, with the item number
/// it shows (a table row shows its own number, `start` + row).
fn drawn(code: &str) -> Vec<Drawn> {
    let form = schema_entry(code).unwrap();
    let mut out = Vec::new();
    for part in form["parts"].as_array().unwrap() {
        let part_name = part["n"].as_str().unwrap_or("").to_string();
        for item in part["items"].as_array().unwrap() {
            let no = item["no"].as_str().unwrap_or("").to_string();
            let label = item["l"].as_str().unwrap_or("").to_string();
            for field in item["f"].as_array().into_iter().flatten() {
                if field["t"] != "hidden" {
                    out.push(Drawn {
                        part: part_name.clone(),
                        no: no.clone(),
                        item: label.clone(),
                        key: field["k"].as_str().unwrap().to_string(),
                        label: field["l"].as_str().unwrap_or("").to_string(),
                        t: field["t"].as_str().unwrap_or("").to_string(),
                        cell: false,
                    });
                }
            }
            for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
                for (c, key) in row.as_array().into_iter().flatten().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] == "hidden" {
                        continue;
                    }
                    let row_no = item["start"]
                        .as_u64()
                        .map(|start| (start + r as u64).to_string())
                        .unwrap_or_else(|| no.clone());
                    out.push(Drawn {
                        part: part_name.clone(),
                        no: row_no,
                        item: label.clone(),
                        key: key.as_str().unwrap().to_string(),
                        label: col["l"].as_str().unwrap_or("").to_string(),
                        t: col["t"].as_str().unwrap_or("").to_string(),
                        cell: true,
                    });
                }
            }
        }
    }
    out
}

fn drawn_key(code: &str, key: &str) -> Option<Drawn> {
    drawn(code).into_iter().find(|d| d.key == key)
}

/// The items of a part: `(no, label, note)`.
fn items_of(code: &str, part: &str) -> Vec<(String, String, Option<String>)> {
    let form = schema_entry(code).unwrap();
    form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == part)
        .unwrap_or_else(|| panic!("{code}: no part {part}"))["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            (
                it["no"].as_str().unwrap_or("").to_string(),
                it["l"].as_str().unwrap_or("").to_string(),
                it["note"].as_str().map(str::to_string),
            )
        })
        .collect()
}

/// The printed item number a caption starts with (`9A`).
fn printed_number(printed: &str) -> Option<String> {
    let head = printed.split_whitespace().next()?;
    let digits = head.trim_end_matches(|c: char| c.is_ascii_uppercase());
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && head.len() <= 3)
        .then(|| head.to_string())
}

fn print_of(code: &str, draft: &Value) -> BTreeMap<String, String> {
    with_database(|db| print_map(db, kind_of(code)?, draft))
        .unwrap()
        .0
}

/// R3: every box the sheet prints under an item number sits under that
/// number on the page (a table row under its own), and every printed
/// option reads as printed. The oracle is writer-cells.json's captions.
#[test]
fn monthly_boxes_sit_under_their_printed_item_numbers() {
    for (code, slug) in MONTHLY {
        let page = drawn(code);
        let mut checked = 0;
        for (key, printed) in joins(slug) {
            let Some(box_) = page.iter().find(|d| d.key == key) else {
                continue;
            };
            let Some(no) = printed_number(&printed) else {
                continue;
            };
            assert_eq!(box_.no, no, "{code} {key} prints at {printed:?}");
            if box_.t == "radio" {
                assert!(
                    printed.ends_with(&box_.label),
                    "{code} {key}: option {:?} prints as {printed:?}",
                    box_.label
                );
            }
            checked += 1;
        }
        assert!(checked >= 12, "{code}: {checked} boxes checked");
    }
}

/// R3: the item captions the findings named read as printed. A hand list,
/// each checked against the frozen sheet's own text (whose text layer runs
/// some captions around their boxes, hence the pieces).
#[test]
fn monthly_captions_read_as_printed() {
    let label = |code: &str, part: &str, no: &str| {
        items_of(code, part)
            .into_iter()
            .find(|(n, _, _)| n == no)
            .map(|(_, l, _)| l)
            .unwrap_or_else(|| panic!("{code} {part} item {no}"))
    };
    const PART_I: &str = "Part I · Background Information";
    let cases: [(&str, &str, &str, &str, &[&str]); 12] = [
        (
            "1601C",
            PART_I,
            "11",
            "Category of Withholding Agent",
            &["11 Category of Withholding Agent Private Government"],
        ),
        (
            "1601C",
            PART_I,
            "13",
            "Are there payees availing of tax relief under Special Law or International Tax Treaty?",
            &[
                "Are there payees availing of tax relief under 13",
                "Special Law or International Tax Treaty?",
            ],
        ),
        (
            "1601C",
            PART_I,
            "13A",
            "If yes, specify",
            &["13A If yes, specify"],
        ),
        (
            "0619E",
            PART_I,
            "10",
            "Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)",
            &[
                "10 Registered Address (Indicate complete address. If branch, indicate the branch address. If the registered address is different from the current address, go to the RDO to update registered address by using BIR Form No. 1905)",
            ],
        ),
        (
            "0619F",
            PART_I,
            "11",
            "Category of Withholding Agent",
            &["11 Category of Withholding Agent Private Government"],
        ),
        (
            "1600PT",
            "Page 1",
            "2",
            "Amended Return?",
            &["Amended Return? Yes No"],
        ),
        (
            "1600PT",
            PART_I,
            "12",
            "Are you availing of tax relief under Special Law or International Tax Treaty?",
            &[
                "Are you availing of tax relief under",
                "Special Law or International Tax Treaty?",
            ],
        ),
        (
            "1600VT",
            PART_I,
            "12",
            "Are you availing of tax relief under Special Law or International Tax Treaty?",
            &[
                "12 Are you availing of tax relief under",
                "Special Law or International Tax Treaty?",
            ],
        ),
        (
            "1600VT",
            PART_I,
            "12A",
            "If Yes, specify",
            &["12A If Yes, specify"],
        ),
        (
            "1600WP",
            PART_I,
            "7",
            "Category of Withholding Agent",
            &["Category of Withholding Agent 7 Private Government"],
        ),
        (
            "1600WP",
            "Part II · Computation of Tax",
            "12",
            "Tax Required to be Withheld and Remitted",
            &["12 12 Tax Required to be Withheld and Remitted"],
        ),
        (
            "1600WP",
            "Part II · Computation of Tax",
            "16",
            "Total Amount Still Due / (Overremittance) (Sum of Items 14 & 15D)",
            &["16 16 Total Amount Still Due / (Overremittance)(Sum of Items 14 & 15D)"],
        ),
    ];
    for (code, part, no, expected, printed) in cases {
        assert_eq!(label(code, part, no), expected, "{code} item {no}");
        let slug = MONTHLY.iter().find(|(c, _)| *c == code).unwrap().1;
        let text = sheet_text(slug);
        for piece in printed {
            assert!(text.contains(piece), "{slug}: {piece:?}");
        }
    }
    // No heading carries its options or its own trailing number, and no
    // option repeats its question ("Category agent p", "Tax relief yes").
    for (code, _) in MONTHLY {
        for d in drawn(code) {
            assert!(
                !d.item.ends_with("Private Government"),
                "{code} {}: {}",
                d.key,
                d.item
            );
            if !d.no.is_empty() && !d.cell {
                assert!(
                    !d.item.ends_with(&format!(" {}", d.no)),
                    "{code} {}: {}",
                    d.key,
                    d.item
                );
            }
            if d.t == "radio" {
                assert!(
                    ["Yes", "No", "Private", "Government"].contains(&d.label.as_str()),
                    "{code} {}: {}",
                    d.key,
                    d.label
                );
            }
        }
    }
}

/// R3: the official program's ATC popup slots, the page-2 header copies,
/// the mirrored RDO and the page counters are not drawn; the print still
/// writes every page-2 header join from the draft.
#[test]
fn monthly_forms_draw_no_popup_slot_header_copy_or_page_counter() {
    setup();
    let hidden =
        |code: &str, key: &str| drawn_key(code, key).is_none() && schema_keys(code).contains(key);
    for n in 1..=31 {
        assert!(
            hidden("1600PT", &format!("AtcCode{n}")),
            "1600PT AtcCode{n}"
        );
    }
    for n in 1..=13 {
        assert!(
            hidden("1600VT", &format!("AtcCode{n}")),
            "1600VT AtcCode{n}"
        );
    }
    for key in ["AtcCd1", "AtcCd2", "SchedIIAtcCde1", "SchedIIAtcCde2"] {
        assert!(hidden("1600WP", key), "1600WP {key}");
    }
    for code in ["1600PT", "1600VT"] {
        for tail in [
            "rdoPg1Pt1I6RDO",
            "txtPg2TinC1",
            "txtPg2TinC2",
            "txtPg2TinC3",
            "txtPg2TinC4",
            "txtPg2RegisteredName",
        ] {
            assert!(hidden(code, &format!("frm{code}:{tail}")), "{code} {tail}");
        }
        // Item 6 shows one RDO box.
        let six: Vec<Drawn> = drawn(code)
            .into_iter()
            .filter(|d| d.no == "6" && !d.cell)
            .collect();
        assert_eq!(six.len(), 1, "{code} item 6: {six:?}");
    }
    for key in [
        "frm1601c:txtCurrentPage",
        "frm1601c:txtMaxPage",
        "frm1601c:txtPg2TIN1",
        "frm1601c:txtPg2TIN2",
        "frm1601c:txtPg2TIN3",
        "frm1601c:txtPg2BranchCode",
        "frm1601c:txtPg2TaxpayerName",
    ] {
        assert!(hidden("1601C", key), "1601C {key}");
    }
    let cells = writer_cells("1601c-2018");
    let counters = cells["not_joined"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| {
            n["writer_keys"]
                .as_array()
                .is_some_and(|k| k.contains(&json!("frm1601c:txtMaxPage")))
        })
        .unwrap();
    assert!(
        counters["reason"]
            .as_str()
            .unwrap()
            .contains("page counter")
    );
    for (code, slug) in [
        ("1600PT", "1600-pt-2018"),
        ("1600VT", "1600-vt-2018"),
        ("1601C", "1601c-2018"),
    ] {
        let opened = open(code, 3);
        let map = print_of(code, &opened["draft"]);
        let mut checked = 0;
        for (key, printed) in joins(slug) {
            if printed.starts_with("Page 2") {
                assert!(
                    !map.get(&key).map_or("", String::as_str).trim().is_empty(),
                    "{code}: the print still writes {key} ({printed})"
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 2, "{code}: the page-2 TIN and name");
    }
}

/// R3: the line of business, which none of these sheets prints
/// (writer-cells has no join for it), is in "Not on the printed sheet".
#[test]
fn monthly_line_of_business_is_not_on_the_printed_sheet() {
    for (code, key) in [
        ("1601C", "frm1601c:txtLineBus"),
        ("0619E", "frm0619E:txtLineBus"),
        ("0619F", "frm0619F:txtLineBus"),
        ("1600PT", "frm1600PT:txtLOB"),
        ("1600VT", "frm1600VT:txtLOB"),
    ] {
        let slug = MONTHLY.iter().find(|(c, _)| *c == code).unwrap().1;
        assert!(
            joins(slug).iter().all(|(k, _)| k != key),
            "{code}: the sheet prints no {key}"
        );
        let d = drawn_key(code, key).unwrap_or_else(|| panic!("{code} {key}"));
        assert_eq!(d.part, "Not on the printed sheet", "{code}");
        assert_eq!(d.no, "", "{code}");
        assert_eq!(d.item, "Line of Business", "{code}");
        assert_eq!(d.label, "Line of Business", "{code}");
    }
}

/// R3, 1601C manual-4, 0619F manual-0: the tax agent line and Part III's
/// payment rows show their printed item numbers and captions with a note,
/// and none of their boxes is drawn: the official program keeps every one
/// disabled (1601C: the runtime inventory's `disabled: true`; 0619F:
/// fields.json `enabled_when`; 0619E: its hidden boxes' entries cite the
/// official .hta's `disabled`), and this app never writes a value there.
#[test]
fn monthly_payment_rows_show_their_printed_items_with_a_note() {
    setup();
    let inventory =
        read_json("rules/forms/1601c-v2018/fixtures/runtime-control-inventory-v796.json");
    let disabled = |id: &str| {
        inventory["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap_or_else(|| panic!("inventory {id}"))["disabled"]
            == true
    };
    let fields_0619f = read_json("rules/forms/0619f-v2018/fields.json");
    let enabled_when_0619f = |key: &str| {
        fields_0619f["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field_key"] == key)
            .and_then(|f| f["enabled_when"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let note = "eBIRForms keeps these boxes disabled, so this page has nothing to fill here.";
    let cases: [(&str, &str, [&str; 4]); 3] = [
        ("1601C", "1601c-2018", ["37", "38", "39", "40"]),
        ("0619E", "0619e-2018", ["19", "20", "21", "22"]),
        ("0619F", "0619f-2018", ["20", "21", "22", "23"]),
    ];
    for (code, slug, numbers) in cases {
        let items = items_of(code, "Part III · Details of Payment");
        let expected: Vec<(String, String, Option<String>)> = std::iter::once((
            String::new(),
            "Tax Agent Accreditation No./Attorney's Roll No. (if applicable), Date of Issue and Date of Expiry".to_string(),
            Some(note.to_string()),
        ))
        .chain(
            numbers
                .iter()
                .zip(["Cash/Bank Debit Memo", "Check", "Tax Debit Memo", "Others (specify below)"])
                .map(|(no, l)| (no.to_string(), l.to_string(), Some(note.to_string()))),
        )
        .collect();
        assert_eq!(items, expected, "{code}");
        let text = sheet_text(slug);
        assert!(
            text.contains(&format!("Cash/Bank Debit Memo {}", numbers[0])),
            "{slug}"
        );
        for (no, caption) in
            numbers[1..]
                .iter()
                .zip(["Check", "Tax Debit Memo", "Others (specify below)"])
        {
            assert!(
                text.contains(&format!("{no} {caption}")),
                "{slug}: {no} {caption}"
            );
        }
        // Every box the sheet prints under the tax agent line or Part III.
        let payment: Vec<(String, String)> = joins(slug)
            .into_iter()
            .filter(|(_, printed)| {
                printed.starts_with("Tax Agent")
                    || printed.starts_with("Date of")
                    || printed_number(printed).is_some_and(|n| numbers.contains(&n.as_str()))
            })
            .collect();
        assert!(payment.len() >= 10, "{code}: {payment:?}");
        for (key, printed) in &payment {
            assert!(
                drawn_key(code, key).is_none(),
                "{code} {key} ({printed}) is not drawn"
            );
            match code {
                "1601C" => assert!(disabled(key), "{code} {key} is disabled at load"),
                "0619F" => assert_eq!(
                    enabled_when_0619f(key),
                    "Official runtime leaves this field disabled in the observed form workflow.",
                    "{code} {key}"
                ),
                _ => {}
            }
        }
        // The app writes nothing there for a new return, on the print or
        // in the upload's field map; no page box can change that.
        let opened = open(code, 3);
        let map = print_of(code, &opened["draft"]);
        let xml = agent::field_map(kind_of(code).unwrap(), &opened["draft"]).unwrap();
        for (key, _) in &payment {
            assert!(
                map.get(key).is_none_or(|v| v.trim().is_empty()),
                "{code} {key}"
            );
            assert!(
                xml.get(key).is_none_or(|v| v.trim().is_empty()),
                "{code} {key}"
            );
            let refused = fill(code, &opened["draft"], json!({ key: "X" }));
            assert!(refused["rejected"][key].is_string(), "{code} {key}");
        }
    }
    // R4 (1601C): none of the six dates is a box on the page any more.
    for key in [
        "txtDateIssue",
        "txtDateExpiry",
        "frm1601c:txtDate37",
        "frm1601c:txtDate38",
        "frm1601c:txtDate39",
        "frm1601c:txtDate40",
    ] {
        assert!(drawn_key("1601C", key).is_none(), "1601C {key}");
        assert!(disabled(key), "1601C {key} is disabled at load");
    }
}

/// 1601C manual-1: Schedule I is a part of its own after Part III, its
/// table and its item 4 total, as page 2 prints them.
#[test]
fn form_1601c_schedule_i_is_its_own_part() {
    let items = schema_entry("1601C").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["n"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    let at = |name: &str| items.iter().position(|n| n == name).unwrap();
    assert!(at("Part III · Details of Payment") < at("Part IV · Schedule"));
    let schedule = &schema_entry("1601C").unwrap()["parts"][at("Part IV · Schedule")]["items"];
    assert_eq!(schedule[0]["rows"], 3);
    assert_eq!(schedule[0]["keys"][0][0], "frm1601c:sched1:txtMonthYear0");
    assert_eq!(schedule[1]["no"], "4");
    assert_eq!(schedule[1]["f"][0]["k"], "frm1601c:sched1:txtTotal1");
    let text = sheet_text("1601c-2018");
    assert!(text.contains("Part IV - Schedule Schedule I"));
    assert!(text.contains("4 Total Adjustment (Sum of Items 1 to 3) (To Part II, Item 26)"));
    assert_eq!(
        drawn_key("1601C", "frm1601c:sched1:txtTotal1").unwrap().no,
        "4"
    );
}

/// 1601C manual-3: item 13A lists the reliefs by name (the draft keeps the
/// name, form_1601c.rs `tax_relief_specification`), and the official blank
/// code 0 is no choice: the page shows Choose... for it (writer-cells
/// not_joined: "dropdown code (0 = none)"). A chosen relief is filed and
/// printed as the draft keeps it.
#[test]
fn form_1601c_item_13a_lists_the_reliefs_by_name() {
    setup();
    let field = drawn_key("1601C", "frm1601c:selTreaty").unwrap();
    assert_eq!((field.no.as_str(), field.t.as_str()), ("13A", "sel"));
    let form = schema_entry("1601C").unwrap();
    let options = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
        .find(|f| f["k"] == "frm1601c:selTreaty")
        .unwrap()["o"]
        .clone();
    let values: Vec<&str> = options
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["value"].as_str().unwrap())
        .collect();
    assert_eq!(values, ["Special Rate", "International Tax Treaty", "Both"]);
    let opened = open("1601C", 3);
    assert_eq!(
        opened["values"]["frm1601c:selTreaty"], "0",
        "the official blank"
    );
    let yes = fill(
        "1601C",
        &opened["draft"],
        json!({"frm1601c:SpecialTax_1": "true", "frm1601c:SpecialTax_2": "false"}),
    );
    let chosen = fill(
        "1601C",
        &yes["draft"],
        json!({"frm1601c:selTreaty": "International Tax Treaty"}),
    );
    assert_eq!(chosen["rejected"], json!({}));
    assert_eq!(
        chosen["draft"]["tax_relief_specification"],
        "International Tax Treaty"
    );
    let map = print_of("1601C", &chosen["draft"]);
    assert_eq!(
        map["derived:line:tax_relief_specify"],
        "International Tax Treaty"
    );
}

/// 0619E preview.0 and manual-1, 0619F preview.0 and manual-2: the
/// Category of Withholding Agent is the filer's choice. The field map (the
/// XML) and the print write both boxes from the draft's
/// `withholding_agent_category`, which the page now sets; the profile's
/// government flag is only the new draft's default.
#[test]
fn monthly_category_is_the_filers_choice_on_0619e_and_0619f() {
    setup();
    for code in ["0619E", "0619F"] {
        let p = format!("frm{code}:optCategory:P");
        let g = format!("frm{code}:optCategory:G");
        let opened = open(code, 3);
        assert!(opened["locks"].get(&p).is_none(), "{code}: {p} is open");
        assert!(opened["locks"].get(&g).is_none(), "{code}: {g} is open");
        assert_eq!(opened["draft"]["withholding_agent_category"], "Private");
        let government = fill(code, &opened["draft"], json!({ &g: "true", &p: "false" }));
        assert_eq!(government["rejected"], json!({}), "{code}");
        assert_eq!(
            government["draft"]["withholding_agent_category"], "Government",
            "{code}"
        );
        assert_eq!(government["values"][&g], "true", "{code}");
        assert_eq!(government["values"][&p], "false", "{code}");
        let kind = kind_of(code).unwrap();
        let xml = agent::field_map(kind, &government["draft"]).unwrap();
        assert_eq!(
            (xml[&g].as_str(), xml[&p].as_str()),
            ("true", "false"),
            "{code}"
        );
        let printed = print_of(code, &government["draft"]);
        assert_eq!(printed[&g], "true", "{code}: the print ticks Government");
        let private = fill(
            code,
            &government["draft"],
            json!({ &p: "true", &g: "false" }),
        );
        assert_eq!(private["draft"]["withholding_agent_category"], "Private");
        // The untick of the other box alone says nothing.
        let untick = fill(code, &government["draft"], json!({ &p: "false" }));
        assert_eq!(untick["draft"]["withholding_agent_category"], "Government");
    }
    // A stored draft that saved the legacy flag opens as it saved.
    let opened = open("0619E", 3);
    let mut legacy = opened["draft"].clone();
    let object = legacy.as_object_mut().unwrap();
    object.remove("withholding_agent_category");
    object.insert("opt_category_g".into(), json!(true));
    let shown = fill("0619E", &legacy, json!({}));
    assert_eq!(shown["values"]["frm0619E:optCategory:G"], "true");
}

/// 0619E manual-3: the registered address is a profile box like every
/// other one (Form0619EDraft::new_from_profile copies the profile's
/// address): folded with the profile boxes with Edit while the profile has
/// one, open where it has none; both lines are one pool (rule 5).
#[test]
fn form_0619e_address_is_a_profile_box() {
    setup();
    let key = "frm0619E:txtAddress";
    let opened = open("0619E", 3);
    assert_eq!(
        opened["draft"]["registered_address"],
        "1 Example Street, Quezon City"
    );
    assert!(strings(&opened["profile"]).contains(key));
    assert!(strings(&opened["profile_edit"]).contains(key));
    assert_eq!(
        opened["locks"][key],
        bir_core::forms::agent::Lock::Profile.reason()
    );
    assert!(combs::pools("0619E").contains_key(key));
    let sparse = install_sparse_profile();
    let blank = open_for(sparse, "0619E", YEAR, 3);
    assert!(
        blank["locks"].get(key).is_none(),
        "a blank profile address is open"
    );
    assert!(!strings(&blank["profile"]).contains(key));
    // Edit keeps the change on this return.
    let edited = fill_unlocked(
        "0619E",
        &opened["draft"],
        json!({ key: "2 OTHER STREET, MAKATI CITY" }),
        &[key],
    );
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(
        edited["draft"]["registered_address"],
        "2 OTHER STREET, MAKATI CITY"
    );
}

/// The 1600PT/1600VT draft's private popup entries, by position.
fn popup_private(code: &str) -> Vec<(usize, &'static str, &'static str, f64)> {
    match code {
        "1600PT" => bir_core::forms::form_1600pt::form_1600pt_popup(
            bir_core::forms::form_1600pt::Form1600PtAgentCategory::Private,
        )
        .map(|(i, o)| (i, o.code, o.rate_text, o.rate))
        .collect(),
        _ => bir_core::forms::form_1600vt::form_1600vt_popup(
            bir_core::forms::form_1600vt::Form1600VtAgentCategory::Private,
        )
        .map(|(i, o)| (i, o.code, o.rate_text, o.rate))
        .collect(),
    }
}

/// 1600PT/1600VT with items 3 Yes and 10 Private answered.
fn popup_ready(code: &str) -> Value {
    let p = format!("frm{code}:");
    let opened = open(code, 3);
    let ready = fill(
        code,
        &opened["draft"],
        json!({
            format!("{p}rdoPg1I3TaxWithheldYes"): "true",
            format!("{p}rdoPg1I3TaxWithheldNo"): "false",
            format!("{p}rdoPg1I10CategoryofWthholdAgentPriv"): "true",
            format!("{p}rdoPg1I10CategoryofWthholdAgentGovt"): "false",
        }),
    );
    assert_eq!(ready["rejected"], json!({}), "{code}");
    ready
}

/// 1600PT and 1600VT manual-2, R3 and R5: Part II's ATC rows are one table
/// at the top of Part II, rows 13 to 17 as printed, then the attachment's
/// rows. A row's ATC is chosen from the category's popup (form's
/// `toggle_atc`, popup order), its rate follows and is never typed, its tax
/// withheld and item 18 follow (the form's `recompute`), and the field map
/// and the print carry what the page shows. Items 3 and 10 go through the
/// form's setters, which clear the rows as the official handlers do.
#[test]
fn form_1600pt_and_1600vt_part_ii_rows_are_chosen_on_the_page() {
    setup();
    for (code, slug, rows) in [
        ("1600PT", "1600-pt-2018", 31),
        ("1600VT", "1600-vt-2018", 13),
    ] {
        let p = format!("frm{code}:");
        let atc = |n: usize| format!("{p}txtAtcCd{n}");
        let base = |n: usize| format!("{p}txtPg1P2TaxBase{n}");
        let rate = |n: usize| format!("{p}txtPg1P2TaxRate{n}");
        let withheld = |n: usize| format!("{p}txtPg1P2TaxWithheld{n}");
        // The table: rows 13-17 at the top of Part II, the rest attached.
        let first = drawn_key(code, &atc(1)).unwrap();
        assert_eq!(
            (first.part.as_str(), first.no.as_str()),
            ("Part II · Computation of Tax", "13")
        );
        assert_eq!(drawn_key(code, &atc(5)).unwrap().no, "17");
        assert_eq!(first.t, "sel");
        let sixth = drawn_key(code, &atc(6)).unwrap();
        assert_eq!(
            sixth.part,
            "Other Selected ATC rows (attachment; the sheet prints five)"
        );
        assert_eq!(sixth.no, "6");
        assert!(drawn_key(code, &atc(rows)).is_some());
        let items = items_of(code, "Part II · Computation of Tax");
        assert_eq!(items[0].0, "13");
        assert_eq!(items[1].0, "18");
        // Rows 13-17 print at items 13-17 (writer-cells).
        for (key, printed) in joins(slug) {
            if let Some(n) = (1..=5).find(|n| key == atc(*n)) {
                assert_eq!(printed, format!("{} ATC", 12 + n), "{code}");
            }
        }
        let ready = popup_ready(code);
        let popup = popup_private(code);
        let options = ready["options"][atc(1)].as_array().unwrap().clone();
        assert_eq!(options.len(), popup.len(), "{code}: the private popup");
        assert!(options.iter().all(|o| o["selected"] == false));
        // Row 1 gets the third private ATC, row 2 the first: popup order.
        let (third, first_entry) = (popup[2], popup[0]);
        let filled = fill(
            code,
            &ready["draft"],
            json!({
                atc(1): format!("catalog:{}", third.0),
                atc(2): format!("catalog:{}", first_entry.0),
                base(2): "1000",
            }),
        );
        assert_eq!(filled["rejected"], json!({}), "{code}");
        let values = &filled["values"];
        assert_eq!(values[atc(1)], first_entry.1, "{code}: popup order");
        assert_eq!(values[atc(2)], third.1, "{code}");
        assert_eq!(
            values[rate(2)],
            format!(" {}", third.2),
            "{code}: the rate follows"
        );
        let expected = (1000.0 * third.3 / 100.0 * 100.0).round() / 100.0;
        assert_eq!(amount(values, &withheld(2)), expected, "{code}");
        assert_eq!(
            amount(values, &format!("{p}txtPg1P2I18TotTaxWithheld")),
            expected,
            "{code}"
        );
        let selected: Vec<&str> = filled["options"][atc(2)]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["selected"] == true)
            .map(|o| o["value"].as_str().unwrap())
            .collect();
        assert_eq!(selected, [format!("catalog:{}", third.0)], "{code}");
        let map = print_of(code, &filled["draft"]);
        assert_eq!(
            map[&atc(2)],
            third.1,
            "{code}: the print shows row 14's ATC"
        );
        assert_eq!(map[&base(2)], "1,000.00", "{code}");
        // A tax base typed alone: the row's tax withheld and items 18-27
        // follow (the form's recompute), with the other payments counted.
        let rebased = fill(
            code,
            &filled["draft"],
            json!({ base(2): "3000", format!("{p}txtPg1P2I20OthrPymntsMade"): "10" }),
        );
        assert_eq!(rebased["rejected"], json!({}), "{code}");
        let v = &rebased["values"];
        let withheld_3000 = (3000.0 * third.3 / 100.0 * 100.0).round() / 100.0;
        assert_eq!(amount(v, &withheld(2)), withheld_3000, "{code}");
        assert_eq!(
            amount(v, &format!("{p}txtPg1P2I18TotTaxWithheld")),
            withheld_3000
        );
        assert_eq!(amount(v, &format!("{p}txtPg1P2I21TotTaxPymntsMade")), 10.0);
        let due = ((withheld_3000 - 10.0) * 100.0).round() / 100.0;
        assert_eq!(
            amount(v, &format!("{p}txtPg1P2I22TaxStillDueOverremit")),
            due
        );
        let penalties = amount(v, &format!("{p}txtPg1P2I26TotPenalties"));
        assert_eq!(
            amount(v, &format!("{p}txtPg1P2I27TotAmtStillDueOverremit")),
            ((due + penalties) * 100.0).round() / 100.0,
            "{code}"
        );
        // A typed rate is refused; the row keeps the ATC's.
        let typed = fill(code, &filled["draft"], json!({ rate(2): "99" }));
        assert!(typed["rejected"][rate(2)].as_str().is_some(), "{code}");
        assert_eq!(typed["values"][rate(2)], format!(" {}", third.2));
        // A changed ATC keeps its tax base.
        let fourth = popup[3];
        let changed = fill(
            code,
            &filled["draft"],
            json!({ atc(2): format!("catalog:{}", fourth.0) }),
        );
        assert_eq!(changed["rejected"], json!({}), "{code}");
        let row = changed["draft"]["atc_rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["atc_code"] == fourth.1 && r["rate_text"] == fourth.2)
            .unwrap()
            .clone();
        assert_eq!(row["tax_base"], 1000.0, "{code}");
        // A tax base needs its row's ATC.
        let early = fill(code, &ready["draft"], json!({ base(4): "50" }));
        assert_eq!(early["rejected"][base(4)], "Choose this row's ATC first.");
        // An ATC already in another row is refused.
        let twice = fill(
            code,
            &filled["draft"],
            json!({ atc(3): format!("catalog:{}", third.0) }),
        );
        assert!(
            twice["rejected"][atc(3)]
                .as_str()
                .unwrap()
                .contains("already chosen")
        );
        // Item 3 No and item 10 go through the form's setters: the rows go.
        let no = fill(
            code,
            &filled["draft"],
            json!({format!("{p}rdoPg1I3TaxWithheldNo"): "true", format!("{p}rdoPg1I3TaxWithheldYes"): "false"}),
        );
        assert_eq!(
            no["draft"]["atc_rows"],
            json!([]),
            "{code}: item 3 No clears the rows"
        );
        assert_eq!(no["draft"]["taxes_withheld"], false);
        let gov = fill(
            code,
            &filled["draft"],
            json!({format!("{p}rdoPg1I10CategoryofWthholdAgentGovt"): "true", format!("{p}rdoPg1I10CategoryofWthholdAgentPriv"): "false"}),
        );
        assert_eq!(
            gov["draft"]["atc_rows"],
            json!([]),
            "{code}: item 10 clears the rows"
        );
        assert_eq!(gov["draft"]["agent_category"], "government");
        assert_eq!(
            amount(&gov["values"], &format!("{p}txtPg1P2I18TotTaxWithheld")),
            0.0
        );
    }
}

/// Lesson 8: an answer or a cleared ATC that removes rows the filer chose
/// or typed says so (the page asks first and offers Undo); one that
/// removes nothing typed says nothing.
#[test]
fn monthly_removing_typed_rows_is_reported() {
    setup();
    for code in ["1600PT", "1600VT"] {
        let p = format!("frm{code}:");
        let popup = popup_private(code);
        let ready = popup_ready(code);
        let one = fill(
            code,
            &ready["draft"],
            json!({ format!("{p}txtAtcCd1"): format!("catalog:{}", popup[0].0), format!("{p}txtPg1P2TaxBase1"): "2500" }),
        );
        let no = fill(
            code,
            &one["draft"],
            json!({format!("{p}rdoPg1I3TaxWithheldNo"): "true", format!("{p}rdoPg1I3TaxWithheldYes"): "false"}),
        );
        assert_eq!(
            no["removes"],
            json!({"message": "Changing item 3 to No removes the 1 Part II ATC row you chose. Continue?", "done": "Removed 1 row", "rows": 1}),
            "{code}"
        );
        let gov = fill(
            code,
            &one["draft"],
            json!({format!("{p}rdoPg1I10CategoryofWthholdAgentGovt"): "true"}),
        );
        assert_eq!(
            gov["removes"]["message"],
            "Changing item 10 removes the 1 Part II ATC row you chose. Continue?",
            "{code}"
        );
        let cleared = fill(code, &one["draft"], json!({ format!("{p}txtAtcCd1"): "" }));
        assert_eq!(cleared["draft"]["atc_rows"], json!([]));
        assert_eq!(
            cleared["removes"]["message"],
            "Clearing this ATC removes its row and its tax base of ₱2,500.00. Continue?",
            "{code}"
        );
        // Nothing typed, nothing asked.
        let bare = fill(
            code,
            &ready["draft"],
            json!({ format!("{p}txtAtcCd1"): format!("catalog:{}", popup[0].0) }),
        );
        let quiet = fill(code, &bare["draft"], json!({ format!("{p}txtAtcCd1"): "" }));
        assert!(quiet["removes"].is_null(), "{code}: a row with no tax base");
        let same = fill(
            code,
            &ready["draft"],
            json!({format!("{p}rdoPg1I3TaxWithheldNo"): "true"}),
        );
        assert!(same["removes"].is_null(), "{code}: no rows");
    }
    // 1600WP items 4 and 7.
    let ready = wp_ready();
    let typed = fill(
        "1600WP",
        &ready["draft"],
        json!({
            "frm1600WP:txtAtcCode1": "WB193",
            "frm1600WP:txtTaxBase1": "1000",
            "frm1600WP:dtSched:txtTin1": "123456789000",
            "frm1600WP:dtSched:drpAtcCode1": "WB193",
            "frm1600WP:dtSched:amount1": "1000",
        }),
    );
    assert_eq!(typed["rejected"], json!({}));
    let no = fill(
        "1600WP",
        &typed["draft"],
        json!({"frm1600WP:AnyTaxHeld_2": "true", "frm1600WP:AnyTaxHeld_1": "false"}),
    );
    assert_eq!(
        no["removes"],
        json!({"message": "Changing item 4 to No removes the 1 Part II ATC row you chose and the 1 Schedule 1.1 row you typed. Continue?", "done": "Removed 2 rows", "rows": 2})
    );
    assert_eq!(
        no["draft"]["schedule"],
        json!([]),
        "set_taxes_withheld clears Schedule 1.1"
    );
    assert_eq!(no["draft"]["atc_rows"], json!([]));
    let gov = fill(
        "1600WP",
        &typed["draft"],
        json!({"frm1600WP:CategoryAgent_G": "true", "frm1600WP:CategoryAgent_P": "false"}),
    );
    assert_eq!(
        gov["removes"]["message"],
        "Changing item 7 removes the 1 Part II ATC row you chose and clears the ATC of 1 Schedule 1.1 row. Continue?"
    );
    assert_eq!(
        gov["draft"]["schedule"][0]["atc_code"], "",
        "set_agent_category clears it"
    );
    assert_eq!(
        gov["draft"]["schedule"][0]["tin"], "123456789000",
        "the row stays"
    );
    let cleared = fill(
        "1600WP",
        &typed["draft"],
        json!({"frm1600WP:txtAtcCode1": ""}),
    );
    assert_eq!(
        cleared["removes"]["message"],
        "Clearing this ATC removes its row and its tax base of ₱1,000.00. Continue?"
    );
}

/// 1600WP with item 4 Yes and item 7 Private.
fn wp_ready() -> Value {
    let opened = open("1600WP", 3);
    let ready = fill(
        "1600WP",
        &opened["draft"],
        json!({
            "frm1600WP:AnyTaxHeld_1": "true",
            "frm1600WP:AnyTaxHeld_2": "false",
            "frm1600WP:CategoryAgent_P": "true",
            "frm1600WP:CategoryAgent_G": "false",
        }),
    );
    assert_eq!(ready["rejected"], json!({}));
    ready
}

/// 1600WP R6.0: Schedule 1.1's boxes wait for item 4 Yes (fields.json
/// `enabled_when` "Item 4 Yes."): faded with "Needs item 4 = Yes" and
/// refused until then, open once it holds. Items 4 and 7 go through the
/// form's setters (`set_taxes_withheld`, `set_agent_category`), so item 12
/// and the schedule follow them.
#[test]
fn form_1600wp_schedule_waits_for_item_4_and_answers_use_the_setters() {
    setup();
    let fields = read_json("rules/forms/1600wp-v2010/fields.json");
    let condition = |key: &str| {
        fields["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field_key"] == key)
            .and_then(|f| f["enabled_when"].as_str())
            .unwrap_or("")
            .to_string()
    };
    let opened = open("1600WP", 3);
    for n in 1..=10 {
        for stem in ["txtTin", "txtFullname", "drpAtcCode", "amount"] {
            let key = format!("frm1600WP:dtSched:{stem}{n}");
            assert_eq!(condition(&key), "Item 4 Yes.", "{key}");
            assert_eq!(opened["off"][&key], "Needs item 4 = Yes", "{key}");
        }
    }
    let refused = fill(
        "1600WP",
        &opened["draft"],
        json!({"frm1600WP:dtSched:txtTin1": "123456789000"}),
    );
    assert_eq!(
        refused["rejected"]["frm1600WP:dtSched:txtTin1"],
        rows_monthly::ANSWER_4_FIRST
    );
    let no = fill(
        "1600WP",
        &opened["draft"],
        json!({"frm1600WP:AnyTaxHeld_2": "true"}),
    );
    assert_eq!(no["draft"]["taxes_withheld"], false);
    let checked = validated("1600WP", &no["draft"]);
    assert_eq!(
        checked["off"]["frm1600WP:dtSched:amount1"],
        "Needs item 4 = Yes"
    );
    let ready = wp_ready();
    let checked = validated("1600WP", &ready["draft"]);
    assert!(
        checked["off"].as_object().unwrap().is_empty(),
        "{}",
        checked["off"]
    );
    let typed = fill(
        "1600WP",
        &ready["draft"],
        json!({
            "frm1600WP:txtAtcCode1": "WB193",
            "frm1600WP:txtTaxBase1": "1000",
            "frm1600WP:dtSched:txtTin1": "123456789000",
            "frm1600WP:dtSched:drpAtcCode1": "WB193",
            "frm1600WP:dtSched:amount1": "1000",
        }),
    );
    assert_eq!(typed["rejected"], json!({}));
    assert_eq!(
        amount(&typed["values"], "frm1600WP:txtTax12"),
        40.0,
        "4% of 1,000"
    );
    assert!(
        !typed["values"]["frm1600WP:dtSched:txtRatePercent1"]
            .as_str()
            .unwrap_or("")
            .trim()
            .is_empty(),
        "the row's rate follows its ATC"
    );
    // Item 7 through set_agent_category: the ATC rows go, item 12 follows.
    let gov = fill(
        "1600WP",
        &typed["draft"],
        json!({"frm1600WP:CategoryAgent_G": "true"}),
    );
    assert_eq!(gov["draft"]["agent_category"], "government");
    assert_eq!(gov["draft"]["atc_rows"], json!([]));
    assert_eq!(amount(&gov["values"], "frm1600WP:txtTax12"), 0.0);
    // Item 4 No through set_taxes_withheld (`cancelAllCompute`).
    let no = fill(
        "1600WP",
        &typed["draft"],
        json!({"frm1600WP:AnyTaxHeld_2": "true"}),
    );
    assert_eq!(no["draft"]["schedule"], json!([]));
    assert_eq!(amount(&no["values"], "frm1600WP:txtTax12"), 0.0);
}

/// 1600WP manual-2 and R3.3: item 11 is on the page, after item 10, with
/// its printed caption and a note, and draws no box: the official program
/// keeps SpecialLaw_1/2 and SpecialLawSelect disabled at load (the runtime
/// inventory) and this app has no draft field for them. Item 1 is two
/// dates, From and To, each its own item with its three boxes (manual-4).
#[test]
fn form_1600wp_item_11_and_the_period_follow_the_sheet() {
    let inventory =
        read_json("rules/forms/1600wp-v2010/fixtures/runtime-control-inventory-v796.json");
    for id in [
        "frm1600WP:SpecialLaw_1",
        "frm1600WP:SpecialLaw_2",
        "frm1600WP:SpecialLawSelect",
    ] {
        let control = inventory["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        assert_eq!(control["disabled"], true, "{id}");
        assert!(drawn_key("1600WP", id).is_none(), "{id}");
        assert!(
            !kind_of("1600WP")
                .unwrap()
                .definition()
                .bindings
                .contains_key(id),
            "{id}"
        );
    }
    let part = items_of("1600WP", "Part I · Background Information");
    let numbers: Vec<&str> = part.iter().map(|(n, _, _)| n.as_str()).collect();
    assert_eq!(numbers, ["5", "6", "7", "8", "9", "10", "11"]);
    let (_, label, note) = &part[6];
    assert!(label.starts_with(
        "Are there payees availing of tax relief under Special Law or International Tax Treaty?"
    ));
    assert!(note.as_deref().unwrap().contains("disabled"));
    assert!(sheet_text("1600wp-2010").contains("Are there payees availing of tax relief under Special Law or 11 International Tax Treaty? Yes No If yes, specify"));
    let page = items_of("1600WP", "Page 1");
    assert_eq!(page[0].1, "For the Period – From (MM/DD/YYYY)");
    assert_eq!(page[1].1, "For the Period – To (MM/DD/YYYY)");
    let from: Vec<String> = drawn("1600WP")
        .into_iter()
        .filter(|d| d.item == page[0].1)
        .map(|d| d.key)
        .collect();
    assert_eq!(
        from,
        [
            "frm1600WP:DateWithholdingMonth",
            "frm1600WP:DateWithholdingDay",
            "frm1600WP:DateWithholdingYear"
        ]
    );
    let to: Vec<String> = drawn("1600WP")
        .into_iter()
        .filter(|d| d.item == page[1].1)
        .map(|d| d.key)
        .collect();
    assert_eq!(
        to,
        [
            "frm1600WP:DateWithholdingToMonth",
            "frm1600WP:DateWithholdingToDay",
            "frm1600WP:DateWithholdingToYear"
        ]
    );
    // Schedule 1.1 is its own part after Part II, one table of ten rows
    // with the printed columns (manual-0, R3.0, R5.0).
    let parts: Vec<String> = schema_entry("1600WP").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["n"].as_str().unwrap().to_string())
        .collect();
    let at = |n: &str| parts.iter().position(|p| p == n).unwrap();
    let schedule = "Schedule 1.1 – Alphabetical List of Payees from Whom Taxes Were Withheld";
    assert!(at("Part II · Computation of Tax") < at(schedule));
    let tin = drawn_key("1600WP", "frm1600WP:dtSched:txtTin1").unwrap();
    assert_eq!(tin.part, schedule);
    assert_eq!(tin.label, "TIN (Including Branch Code)");
    assert_eq!(
        drawn_key("1600WP", "frm1600WP:dtSched:txtTin10")
            .unwrap()
            .no,
        "10"
    );
    // Part II's ATC table is at the top of Part II (manual-1).
    let atc = drawn_key("1600WP", "frm1600WP:txtAtcCode1").unwrap();
    assert_eq!(atc.part, "Part II · Computation of Tax");
    assert_eq!(
        items_of("1600WP", "Part II · Computation of Tax")[1].0,
        "12"
    );
    let text = sheet_text("1600wp-2010");
    assert!(
        text.contains("Schedule 1.1 ALPHABETICAL LIST OF PAYEES FROM WHOM TAXES WERE WITHHELD")
    );
}

/// 1600PT/1600VT manual-3 and manual-4 (and 1600VT R3.0, R5.2): Schedule 1
/// is a part of its own, one table of the five printed rows with the
/// printed columns and the total; the payee TIN is one box of its row.
#[test]
fn form_1600pt_and_1600vt_schedule_1_is_a_table() {
    for (code, slug) in [("1600PT", "1600-pt-2018"), ("1600VT", "1600-vt-2018")] {
        let p = format!("frm{code}:");
        let part = "Schedule 1 – Monthly Alphalist of Payees from Whom Taxes were Withheld";
        let tin = drawn_key(code, &format!("{p}Pg2Sc1TIN1")).unwrap();
        assert_eq!(
            (tin.part.as_str(), tin.label.as_str(), tin.no.as_str()),
            (part, "TIN", "1")
        );
        assert_eq!(
            drawn_key(code, &format!("{p}Pg2Sc1AmtOfTxWthhld5"))
                .unwrap()
                .no,
            "5"
        );
        let total = drawn_key(code, &format!("{p}Pg2Sc1TotTxWthhldAndRmtted")).unwrap();
        assert_eq!(
            (total.part.as_str(), total.item.as_str()),
            (part, "Total Taxes Withheld and Remitted")
        );
        assert!(
            sheet_text(slug)
                .contains("Schedule 1 - Monthly Alphalist of Payees from Whom Taxes were Withheld")
        );
        // The page-2 header TIN is not drawn: no TIN part outside Part I.
        for d in drawn(code) {
            assert!(!d.key.contains("TinC"), "{code} {}", d.key);
        }
    }
}

/// 1600PT/1600VT manual-5: item 1 shows the month's name and the whole
/// year; display only. The draft stores `month: u8` and `year: u16` (four
/// digits); MM and the two-digit YY are only the field map's format
/// (`year % 100`), which the boxes' option values match.
#[test]
fn form_1600pt_and_1600vt_month_reads_as_a_month_and_a_year() {
    setup();
    for code in ["1600PT", "1600VT"] {
        let p = format!("frm{code}:");
        let opened = open(code, 3);
        assert_eq!(opened["values"][format!("{p}txtPg1I1Month")], "03");
        assert_eq!(opened["values"][format!("{p}txtPg1I1Year")], "26");
        let form = schema_entry(code).unwrap();
        let field = |key: &str| {
            form["parts"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p["items"].as_array().unwrap())
                .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
                .find(|f| f["k"] == key)
                .unwrap()
        };
        let month = field(&format!("{p}txtPg1I1Month"));
        assert_eq!(month["t"], "sel");
        assert!(
            month["o"]
                .as_array()
                .unwrap()
                .contains(&json!("03 - March"))
        );
        let year = field(&format!("{p}txtPg1I1Year"));
        let label = year["o"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["value"] == "26")
            .unwrap()["label"]
            .clone();
        assert_eq!(label, "2026");
        let map = print_of(code, &opened["draft"]);
        assert_eq!(map[&format!("{p}txtPg1I1Year")], "26");
        assert_eq!(opened["draft"]["year"], 2026);
        assert_eq!(opened["draft"]["month"], 3);
    }
}

/// What each rate and nature of payment waits for while it shows blank.
#[test]
fn monthly_rates_wait_for_their_atc() {
    setup();
    for (code, keys) in [
        (
            "1600PT",
            vec!["frm1600PT:txtPg1P2TaxRate1", "frm1600PT:txtPg1P2TaxRate31"],
        ),
        (
            "1600VT",
            vec!["frm1600VT:txtPg1P2TaxRate1", "frm1600VT:txtPg1P2TaxRate13"],
        ),
        (
            "1600WP",
            vec![
                "frm1600WP:txtTaxRate1",
                "frm1600WP:txtTaxRate2",
                "frm1600WP:dtSched:txtRatePercent10",
            ],
        ),
    ] {
        let opened = open(code, 3);
        for key in keys {
            assert_eq!(
                opened["waits"][key],
                rows_monthly::RATE_WAITS,
                "{code} {key}"
            );
        }
    }
    let opened = open("1600WP", 3);
    for n in 1..=10 {
        let key = format!("frm1600WP:dtSched:naturePayment{n}");
        assert_eq!(opened["waits"][&key], rows_monthly::NATURE_WAITS, "{key}");
    }
    // The 1600PT/1600VT rate is computed on the page.
    assert!(
        drawn("1600PT")
            .iter()
            .any(|d| d.key == "frm1600PT:txtPg1P2TaxRate1")
    );
    let form = schema_entry("1600PT").unwrap();
    let table = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .find(|it| it["keys"][0][0] == "frm1600PT:txtAtcCd1")
        .unwrap()
        .clone();
    assert_eq!(table["cols"][2]["c"], 1, "the rate column is computed");
}

/// Lesson 6: a 1600PT draft saved with a typed ATC row (the earlier page
/// wrote the code and rate as text) opens and files exactly as it saved;
/// only clearing the row changes it.
#[test]
fn a_legacy_typed_1600pt_row_opens_and_files_as_it_saved() {
    setup();
    let ready = popup_ready("1600PT");
    let mut legacy = ready["draft"].clone();
    legacy["atc_rows"] =
        json!([{"atc_code": "WX999", "rate_text": "7.0", "tax_base": 100.0, "tax_withheld": 0.0}]);
    let kind = kind_of("1600PT").unwrap();
    let before = agent::field_map(kind, &legacy).unwrap();
    let shown = fill("1600PT", &legacy, json!({}));
    let row = &shown["draft"]["atc_rows"][0];
    assert_eq!(shown["draft"]["atc_rows"].as_array().unwrap().len(), 1);
    assert_eq!(
        (row["atc_code"].as_str(), row["rate_text"].as_str()),
        (Some("WX999"), Some("7.0"))
    );
    assert_eq!(row["tax_base"].as_f64(), Some(100.0));
    assert_eq!(shown["values"]["frm1600PT:txtAtcCd1"], "WX999");
    let after = agent::field_map(kind, &shown["draft"]).unwrap();
    assert_eq!(
        before.get("frm1600PT:txtAtcCd1"),
        after.get("frm1600PT:txtAtcCd1")
    );
    assert_eq!(
        before.get("frm1600PT:txtPg1P2TaxRate1"),
        after.get("frm1600PT:txtPg1P2TaxRate1")
    );
    let cleared = fill("1600PT", &legacy, json!({"frm1600PT:txtAtcCd1": ""}));
    assert_eq!(cleared["rejected"], json!({}));
    assert_eq!(cleared["draft"]["atc_rows"], json!([]));
    // A saved row under item 3 No (which the popup would refuse to tick)
    // still clears.
    legacy["taxes_withheld"] = json!(false);
    let cleared = fill("1600PT", &legacy, json!({"frm1600PT:txtAtcCd1": ""}));
    assert_eq!(cleared["rejected"], json!({}));
    assert_eq!(cleared["draft"]["atc_rows"], json!([]));
}

/// 1601C R5.2: item 9's two lines are one pool. A stored draft whose two
/// lines were typed apart shows them joined and is not changed by opening.
/// A typed address splits at the last space before line 1's 40th letter
/// only when both parts fit (line 1 its 40 boxes, line 2 its official 50
/// characters); otherwise it stays whole in line 1 (up to its official
/// 100, as the page took before), and past that it splits at line 1's last
/// space within 100. What the two fields cannot file is refused on the
/// address box, never accepted for the queue to refuse later. Both lines
/// are stored trimmed, and showing them again changes nothing.
#[test]
fn form_1601c_address_lines_are_one_pool() {
    setup();
    let head = "frm1601c:txtAddress";
    let pool = combs::pools("1601C");
    let item = &pool[head];
    assert_eq!(item.keys, ["frm1601c:txtAddress", "frm1601c:txtAddress2"]);
    assert_eq!(item.slots, [40, 31]);
    let opened = open("1601C", 3);
    let mut legacy = opened["draft"].clone();
    legacy["registered_address"] = json!("12 MAIN ST");
    legacy["registered_address_2"] = json!("BLDG 5");
    let kind = kind_of("1601C").unwrap();
    let shown = fill("1601C", &legacy, json!({}));
    assert_eq!(shown["pools"][head], "12 MAIN ST BLDG 5");
    assert_eq!(shown["draft"]["registered_address"], "12 MAIN ST");
    assert_eq!(shown["draft"]["registered_address_2"], "BLDG 5");
    let xml = agent::field_map(kind, &shown["draft"]).unwrap();
    assert_eq!(xml["frm1601c:txtAddress2"], "BLDG 5");
    let address_errors = |draft: &Value| -> Vec<String> {
        validated("1601C", draft)["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.to_string())
            .filter(|e| e.contains("Address") || e.contains("address"))
            .collect()
    };
    let typed_in = |draft: &Value, text: &str| -> Value {
        fill_unlocked("1601C", draft, json!({ head: text }), &[head])
    };
    // The draft's two lines, each checked against the upload's field map
    // after the typed split: txtAddress and txtAddress2 file them as they
    // are (form_1601c_xml.rs uppercases them).
    let lines = |filled: &Value| {
        let one = filled["draft"]["registered_address"]
            .as_str()
            .unwrap()
            .to_string();
        let two = filled["draft"]["registered_address_2"]
            .as_str()
            .unwrap()
            .to_string();
        let filed = agent::field_map(kind, &filled["draft"]).unwrap();
        assert_eq!(filed["frm1601c:txtAddress"], one.to_uppercase());
        assert_eq!(filed["frm1601c:txtAddress2"], two.to_uppercase());
        (one, two)
    };
    // Both parts fit: split at the last space among the first 40 letters.
    let typed = "UNIT 4 SAMPLE TOWER 456 EXAMPLE AVENUE CORNER PLACEHOLDER ST QUEZON CITY";
    let filled = typed_in(&opened["draft"], typed);
    assert_eq!(filled["rejected"], json!({}));
    let cut = typed[..40].rfind(' ').unwrap();
    assert_eq!(
        lines(&filled),
        (typed[..cut].to_string(), typed[cut + 1..].to_string())
    );
    assert_eq!(filled["pools"][head], typed);
    assert!(address_errors(&filled["draft"]).is_empty());
    // Showing it again changes nothing (the round trip).
    let again = typed_in(&filled["draft"], filled["pools"][head].as_str().unwrap());
    assert_eq!(lines(&again), lines(&filled));
    let printed = print_of("1601C", &filled["draft"]);
    let whole = format!(
        "{} {}",
        printed["derived:address_line1"], printed["derived:line:address_line2"]
    );
    assert_eq!(
        whole.split_whitespace().collect::<Vec<_>>(),
        typed.split_whitespace().collect::<Vec<_>>()
    );
    // A space right after the 40th letter does not end line 1 at 40
    // letters: no space among them, so the text stays whole.
    let edge = format!("{} {}", "A".repeat(40), "B".repeat(20));
    let filled = typed_in(&opened["draft"], &edge);
    assert_eq!(lines(&filled), (edge.clone(), String::new()));
    assert_eq!(filled["pools"][head], edge.as_str());
    // 91 to 100 characters, with and without spaces: whole in line 1,
    // which files them, as before.
    for n in 91..=100 {
        for text in [
            "X".repeat(n),
            format!("{} {}", "A".repeat(39), "B".repeat(n - 40)),
            format!("{} {}", "A".repeat(30), "B".repeat(n - 31)),
        ] {
            let filled = typed_in(&opened["draft"], &text);
            assert_eq!(filled["rejected"], json!({}), "{n}");
            assert_eq!(lines(&filled), (text.clone(), String::new()), "{n}");
            assert!(address_errors(&filled["draft"]).is_empty(), "{n}");
            assert_eq!(filled["pools"][head], text.as_str(), "{n}");
        }
    }
    // A stored 96-character line 1 is still fileable after one edit.
    let mut long = opened["draft"].clone();
    let stored = format!("{} {}", "A".repeat(20), "B".repeat(75));
    long["registered_address"] = json!(stored);
    long["registered_address_2"] = json!("");
    assert!(address_errors(&long).is_empty());
    let shown = fill("1601C", &long, json!({}));
    assert_eq!(shown["pools"][head], stored.as_str());
    let edited = typed_in(&long, &format!("{stored}X"));
    assert_eq!(edited["rejected"], json!({}));
    assert!(address_errors(&edited["draft"]).is_empty());
    // Past 100: line 1's last space within 100, the rest on line 2.
    let over = format!("{} {}", "A".repeat(60), "B".repeat(45));
    let filled = typed_in(&opened["draft"], &over);
    assert_eq!(lines(&filled), ("A".repeat(60), "B".repeat(45)));
    assert!(address_errors(&filled["draft"]).is_empty());
    assert_eq!(filled["pools"][head], over.as_str());
    // Past what the two fields file: refused on the address box.
    for text in [
        "X".repeat(101),
        format!("{} {}", "A".repeat(60), "B".repeat(60)),
    ] {
        let refused = typed_in(&opened["draft"], &text);
        assert_eq!(
            refused["rejected"][head],
            combs::TWO_FIELD_TOO_LONG,
            "{}",
            text.len()
        );
        assert_eq!(
            refused["draft"]["registered_address"],
            opened["draft"]["registered_address"]
        );
    }
}

/// 0619E manual-4, 0619F manual-3: item 2 is as the official page has it:
/// the day an ordinary box the filer may change (fields.json: required,
/// not computed, default 10), the month and year computed from the period
/// (fields.json: computed). The page shows the day open and the month and
/// year locked with their values.
#[test]
fn monthly_due_date_day_is_the_filers_as_on_the_official_page() {
    setup();
    for (code, package) in [("0619E", "0619e-v2018"), ("0619F", "0619f-v2018")] {
        let fields = read_json(&format!("rules/forms/{package}/fields.json"));
        let field = |key: &str| {
            fields["fields"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["field_key"] == key)
                .unwrap()
                .clone()
        };
        let p = format!("frm{code}:");
        let day = field(&format!("{p}txtDueDay"));
        assert_eq!(day["computed"], false, "{code}");
        assert_eq!(day["default_value"], "10", "{code}");
        let opened = open(code, 3);
        assert!(
            opened["locks"].get(format!("{p}txtDueDay")).is_none(),
            "{code}"
        );
        assert_eq!(opened["values"][format!("{p}txtDueDay")], "10", "{code}");
        assert_eq!(drawn_key(code, &format!("{p}txtDueDay")).unwrap().no, "2");
        for part in ["txtDueMonth", "txtDueYear"] {
            let key = format!("{p}{part}");
            assert_eq!(field(&key)["computed"], true, "{code} {part}");
            assert!(
                opened["locks"][&key]
                    .as_str()
                    .unwrap()
                    .starts_with("computed"),
                "{code} {key}"
            );
            assert!(
                !opened["values"][&key].as_str().unwrap().is_empty(),
                "{code} {key}"
            );
            assert_eq!(drawn_key(code, &key).unwrap().no, "2");
        }
    }
}

/// PR 3 review item 15: rows_monthly.rs reads the whole typed draft, but
/// that adds no refusal. The host reads the typed draft for every edit
/// (screen_form_fill's identity check, agent::identity), which refuses
/// only what serde cannot deserialize: a field missing without
/// `#[serde(default)]` (1600PT's and 1600VT's `is_amended`, 1600WP's
/// `email`) is refused the same way for an ordinary box as for the answers
/// and an ATC, before any box is written; a field with `#[serde(default)]`
/// is taken when missing, and so are extra keys an older build wrote.
/// Since P2-4, 1600PT's and 1600VT's Part I text boxes are such fields
/// (`KeptText`; `form_1600pt_and_1600vt_drafts_missing_boxes_open_blank`), so
/// the refused field here is `is_amended`, which they still require.
#[test]
fn monthly_answers_refuse_no_draft_the_ordinary_fill_takes() {
    setup();
    for (code, legacy_key) in [
        ("1600PT", "is_amended"),
        ("1600VT", "is_amended"),
        ("1600WP", "email"),
    ] {
        let p = format!("frm{code}:");
        let opened = open(code, 3);
        let mut legacy = opened["draft"].clone();
        legacy.as_object_mut().unwrap().remove(legacy_key);
        let edits: Vec<Value> = if code == "1600WP" {
            vec![
                json!({"frm1600WP:txtSheets": "2"}),
                json!({"frm1600WP:AnyTaxHeld_1": "true"}),
                json!({"frm1600WP:CategoryAgent_G": "true"}),
            ]
        } else {
            vec![
                json!({ format!("{p}txtPg1I4NoOfSheets"): "2" }),
                json!({ format!("{p}rdoPg1I3TaxWithheldYes"): "true" }),
                json!({ format!("{p}rdoPg1I10CategoryofWthholdAgentPriv"): "true" }),
                json!({ format!("{p}txtAtcCd1"): "catalog:0" }),
            ]
        };
        let errors: Vec<String> = edits
            .iter()
            .map(|edits| {
                call(
                    "screen_form_fill",
                    vec![code.into(), json(legacy.clone()), json(edits.clone())],
                )
                .unwrap_err()
                .to_string()
            })
            .collect();
        assert!(
            errors[0].contains(&format!("not a {code} draft")),
            "{code}: {}",
            errors[0]
        );
        assert!(errors.iter().all(|e| *e == errors[0]), "{code}: {errors:?}");
        // Keys an older build kept that the form no longer reads.
        let mut older = opened["draft"].clone();
        older["legacy_note"] = json!("kept by an older build");
        // A field the form defaults (`#[serde(default)] line_of_business`
        // on 1600PT and 1600VT) may be missing.
        if code != "1600WP" {
            older.as_object_mut().unwrap().remove("line_of_business");
        }
        let answers = if code == "1600WP" {
            json!({"frm1600WP:AnyTaxHeld_1": "true", "frm1600WP:CategoryAgent_P": "true"})
        } else {
            json!({
                format!("{p}rdoPg1I3TaxWithheldYes"): "true",
                format!("{p}rdoPg1I10CategoryofWthholdAgentPriv"): "true",
            })
        };
        let answered = fill(code, &older, answers);
        assert_eq!(answered["rejected"], json!({}), "{code}");
        assert_eq!(answered["draft"]["taxes_withheld"], true, "{code}");
    }
}

/// PR 3 review, round 2: the shared split (combs.rs `split_lines`) files
/// exactly what the base (pr0b-push) filed for 1702RT's three lines and
/// prints for 0619E/0619F's two: a space right after a line's last letter
/// ends that line. Golden values, read off the base for these addresses;
/// the 0619E/0619F upload keeps the whole address in txtAddress (their
/// official field map re-splits it at 127 escaped characters).
#[test]
fn shared_address_splits_file_what_the_base_filed() {
    setup();
    let a = format!(
        "{} {} {} {}",
        "A".repeat(10),
        "B".repeat(27),
        "C".repeat(38),
        "D".repeat(30)
    );
    let b = format!("{} {} {}", "A".repeat(10), "B".repeat(29), "C".repeat(30));
    let c = "ROXAS MAKATI EXAMPLE METRO PLACEHOLDER 5 STREET ROXAS PHILIPPINES ROXAS PASEO 5 FLOOR"
        .to_string();
    let rt = [
        "frm1702RT:txtPg1Pt1I9Address1",
        "frm1702RT:txtPg1Pt1I9Address2",
        "frm1702RT:txtPg1Pt1I9Address3",
    ];
    let rt_filed: [(&String, [String; 3]); 3] = [
        (
            &a,
            [
                "A".repeat(10) + " " + &"B".repeat(27),
                "C".repeat(38),
                "D".repeat(30),
            ],
        ),
        (&b, ["A".repeat(10), "B".repeat(29), "C".repeat(30)]),
        (
            &c,
            [
                "ROXAS MAKATI EXAMPLE METRO PLACEHOLDER".to_string(),
                "5 STREET ROXAS PHILIPPINES ROXAS PASEO".to_string(),
                "5 FLOOR".to_string(),
            ],
        ),
    ];
    let opened = open("1702RT", page_period_of("1702RT"));
    let kind = kind_of("1702RT").unwrap();
    for (text, lines) in &rt_filed {
        let filled = fill_unlocked("1702RT", &opened["draft"], json!({ rt[0]: text }), &[rt[0]]);
        assert_eq!(filled["rejected"], json!({}));
        let xml = agent::field_map(kind, &filled["draft"]).unwrap();
        let filed: Vec<String> = rt.iter().map(|k| xml[*k].clone()).collect();
        assert_eq!(filed, lines.to_vec(), "1702RT {text}");
        let lengths: Vec<usize> = filed.iter().map(|l| l.chars().count()).collect();
        if *text == &a {
            assert_eq!(lengths, [38, 38, 30]);
        }
    }
    for code in ["0619E", "0619F"] {
        let keys = [
            format!("frm{code}:txtAddress"),
            format!("frm{code}:txtAddress2"),
        ];
        let printed_lines: [(&String, [String; 2]); 3] = [
            (
                &a,
                [
                    "A".repeat(10) + " " + &"B".repeat(27) + " ",
                    "C".repeat(38) + " " + &"D".repeat(30),
                ],
            ),
            (
                &b,
                ["A".repeat(10) + " " + &"B".repeat(29) + " ", "C".repeat(30)],
            ),
            (
                &c,
                [
                    "ROXAS MAKATI EXAMPLE METRO PLACEHOLDER 5 ".to_string(),
                    "STREET ROXAS PHILIPPINES ROXAS PASEO 5 FLOOR".to_string(),
                ],
            ),
        ];
        let opened = open(code, page_period_of(code));
        let kind = kind_of(code).unwrap();
        for (text, lines) in &printed_lines {
            let filled = fill_unlocked(
                code,
                &opened["draft"],
                json!({ &keys[0]: text }),
                &[&keys[0]],
            );
            assert_eq!(filled["rejected"], json!({}), "{code}");
            let printed = print_of(code, &filled["draft"]);
            let shown: Vec<String> = keys.iter().map(|k| printed[k].clone()).collect();
            assert_eq!(shown, lines.to_vec(), "{code} {text}");
            if *text == &b {
                let lengths: Vec<usize> = shown.iter().map(|l| l.chars().count()).collect();
                assert_eq!(lengths, [41, 30], "{code}");
            }
            let xml = agent::field_map(kind, &filled["draft"]).unwrap();
            assert_eq!(
                xml[&keys[0]], **text,
                "{code}: the upload keeps the whole address"
            );
            assert_eq!(xml[&keys[1]], "", "{code}");
        }
    }
}

/// Literal 1600PT and 1600VT drafts stored without some of their Part I
/// text boxes (an older shape): `fixtures/1600pt-missing-boxes.json` lacks
/// `contact_number`, `fixtures/1600vt-missing-boxes.json` lacks `zip_code`
/// and `email`; both carry no profile_baseline, which PR 0c's save adds.
/// Each is the bytes the save path writes for a new March 2026 draft with
/// those keys cut out. Their computed penalties are stamped 2026-10-18; a
/// Draft's are refreshed on every open, so the test restamps them with the
/// day it runs before storing the row.
const MISSING_BOXES: [(&str, &str, &[(&str, &str)]); 2] = [
    (
        "1600PT",
        include_str!("fixtures/1600pt-missing-boxes.json"),
        &[(
            "contact_number",
            "Contact Number field on Page 1 Item 9 is required.",
        )],
    ),
    (
        "1600VT",
        include_str!("fixtures/1600vt-missing-boxes.json"),
        &[
            ("zip_code", "Zip Code field on Page 1 Item 8A is required."),
            ("email", "E-mail address on page 1 item 11 is required."),
        ],
    ),
];

/// `fixture` with its penalty stamp moved from 2026-10-18 to today.
fn stamped_today(fixture: &str) -> String {
    let today = bir_core::forms::penalty_items::today();
    let due = chrono::NaiveDate::from_ymd_opt(2026, 4, 10).unwrap();
    let late = (today - due).num_days().max(0);
    assert_eq!(fixture.matches("\"2026-10-18\"").count(), 2);
    assert_eq!(fixture.matches("\"days_late\":191,").count(), 1);
    fixture
        .replace("\"2026-10-18\"", &format!("\"{today}\""))
        .replace("\"days_late\":191,", &format!("\"days_late\":{late},"))
}

/// Maintainer decision P2-4 (2026-10-18). Before it, such a draft did not
/// open at all ("the stored 1600PT draft could not be read: missing field
/// `contact_number`") and every edit was refused. Now its missing boxes
/// open blank, not 0 or a default; Check asks for each with the form's own
/// message; opened and saved, the row is the fixture byte for byte (the
/// profile baseline a save adds set aside); and once typed, the box files.
#[test]
fn form_1600pt_and_1600vt_drafts_missing_boxes_open_blank() {
    for (code, fixture, missing) in MISSING_BOXES {
        setup();
        let p = format!("frm{code}:");
        let stored = stamped_today(fixture);
        let literal: Value = serde_json::from_str(&stored).unwrap();
        for (field, _) in missing {
            assert!(literal.get(*field).is_none(), "{code}: {field} is missing");
        }
        let id = save(code, &open(code, 3)["draft"])["id"].as_i64().unwrap();
        assert_eq!(id, 1, "{code}");
        raw_exec(
            "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
            &[&stored, &id.to_string()],
        );
        assert_eq!(raw_row(id).1, stored, "{code}: stored as given");
        let opened = open(code, 3);
        assert_eq!(opened["draft"]["id"], json!(id), "{code}");
        let box_of = |field: &str| match field {
            "contact_number" => format!("{p}txtPg1Pt1I9ContactNumber"),
            "zip_code" => format!("{p}txPg1I8ZipCode"),
            _ => format!("{p}txtPg1Pt1I11Email"),
        };
        let messages: Vec<(String, String)> = opened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e["field"].as_str().unwrap().to_string(),
                    e["message"].as_str().unwrap().to_string(),
                )
            })
            .collect();
        for (field, message) in missing {
            let key = box_of(field);
            assert_eq!(opened["values"][&key], "", "{code}: {key} opens blank");
            assert!(opened["draft"].get(*field).is_none(), "{code}: {field}");
            assert!(
                messages.contains(&(field.to_string(), message.to_string())),
                "{code}: Check asks for {field}: {messages:?}"
            );
        }
        // The boxes the draft holds show as stored.
        assert_eq!(opened["values"][format!("{p}txtPg1I6RDO")], "039");
        assert_eq!(raw_row(id).1, stored, "{code}: opening changes nothing");
        save(code, &opened["draft"]);
        let saved = raw_row(id).1;
        let saved = match saved.find(",\"profile_baseline\":") {
            Some(at) => format!("{}}}", &saved[..at]),
            None => saved,
        };
        assert_eq!(saved, stored, "{code}: opened and saved, byte for byte");
        // Typed, the box files and the message goes.
        let (field, _) = missing[0];
        let key = box_of(field);
        let value = match field {
            "contact_number" => "0281234567",
            "zip_code" => "1100",
            _ => "books@example.com",
        };
        let typed = fill_unlocked(code, &opened["draft"], json!({ &key: value }), &[&key]);
        assert_eq!(typed["rejected"], json!({}), "{code}");
        assert_eq!(typed["draft"][field], value, "{code}");
        let checked = call_async(
            "screen_form_validate",
            vec![code.into(), json(typed["draft"].clone())],
        )
        .unwrap();
        assert!(
            checked["errors"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["field"] != field),
            "{code}: {}",
            checked["errors"]
        );
        let kind = kind_of(code).unwrap();
        assert_eq!(
            agent::field_map(kind, &typed["draft"]).unwrap()[&key],
            value
        );
    }
}
