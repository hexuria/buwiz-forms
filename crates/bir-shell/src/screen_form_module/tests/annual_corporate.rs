//! PR 8: the annual corporate income tax returns (1702RT, 1702MX, 1702EX)
//! and 1707A against their printed sheets. The oracles are the frozen
//! sheets' own files (html-frozen/<slug>/writer-cells.json captions and
//! joins, index.html's printed text), the official runtime-control
//! inventories, the official keys, and hand lists from the sheets; never
//! this module's own refusal.

use super::*;

/// `(code, frozen sheet slug, rules package)` of the four forms.
const FORMS: [(&str, &str, &str); 4] = [
    ("1702RT", "1702rt-2018c", "1702rt-v2018c"),
    ("1702MX", "1702mx-2018c", "1702mx-v2018c"),
    ("1702EX", "1702ex-2018", "1702ex-v2018c"),
    ("1707A", "1707a-2021", "1707a-v2021"),
];

fn slug_of(code: &str) -> &'static str {
    FORMS.iter().find(|(c, _, _)| *c == code).unwrap().1
}

fn root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

fn writer_cells(slug: &str) -> Value {
    let path = format!("{}/html-frozen/{slug}/writer-cells.json", root());
    serde_json::from_str(&std::fs::read_to_string(&path).expect("writer-cells"))
        .expect("writer-cells json")
}

/// The frozen sheet's printed words, one space between text runs.
fn sheet_text(slug: &str) -> String {
    let path = format!("{}/html-frozen/{slug}/index.html", root());
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

/// The official runtime's control inventory: id -> disabled at load.
fn disabled_at_load(package: &str) -> BTreeMap<String, bool> {
    let path = format!(
        "{}/rules/forms/{package}/fixtures/runtime-control-inventory-v796.json",
        root()
    );
    let inventory: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    inventory["controls"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| Some((c["id"].as_str()?.to_string(), c["disabled"] == true)))
        .collect()
}

/// One drawn box: its part, item number, item label, key and type.
#[derive(Clone, Debug)]
struct Drawn {
    part: String,
    no: String,
    label: String,
    key: String,
    t: String,
}

/// Every drawn box, table cells numbered by their row when the table
/// starts at an item number.
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
                        label: label.clone(),
                        key: field["k"].as_str().unwrap().to_string(),
                        t: field["t"].as_str().unwrap().to_string(),
                    });
                }
            }
            for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
                let row_no = item["start"]
                    .as_u64()
                    .map_or(String::new(), |start| (start + r as u64).to_string());
                for (c, key) in row.as_array().unwrap().iter().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] != "hidden" {
                        out.push(Drawn {
                            part: name.clone(),
                            no: row_no.clone(),
                            label: col["l"].as_str().unwrap_or("").to_string(),
                            key: key.as_str().unwrap().to_string(),
                            t: col["t"].as_str().unwrap().to_string(),
                        });
                    }
                }
            }
        }
    }
    out
}

fn drawn_keys(code: &str) -> BTreeSet<String> {
    drawn(code).into_iter().map(|d| d.key).collect()
}

/// The part and item a key is drawn under.
fn placed(code: &str, key: &str) -> Drawn {
    drawn(code)
        .into_iter()
        .find(|d| d.key == key)
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
}

/// The keys drawn under item `no` of `part`, in order.
fn item_keys(code: &str, part: &str, no: &str) -> Vec<String> {
    drawn(code)
        .into_iter()
        .filter(|d| d.part == part && d.no == no)
        .map(|d| d.key)
        .collect()
}

/// The printed item number a caption starts with (`7A`, `14C`, `Item 12A`).
fn printed_number(printed: &str) -> Option<String> {
    let printed = printed.strip_prefix("Item ").unwrap_or(printed);
    let head = printed.split_whitespace().next()?;
    let digits = head.trim_end_matches(|c: char| c.is_ascii_uppercase());
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) && head.len() <= 3)
        .then(|| head.to_string())
}

/// writer-cells.json `(writer key, printed caption)` of every join; a
/// `derived:amount:<control>` join (1702EX) is that control's box.
fn printed_captions(code: &str, slug: &str) -> Vec<(String, String)> {
    let cells = writer_cells(slug);
    [
        "joins",
        "money_joins",
        "xbox_joins",
        "peso_joins",
        "date_joins",
    ]
    .iter()
    .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
    .filter_map(|join| {
        let key = join["writer_key"].as_str()?;
        let key = match key.strip_prefix("derived:amount:") {
            Some(control) => format!("frm{code}:{control}"),
            None => key.to_string(),
        };
        Some((key, join["printed"].as_str()?.to_string()))
    })
    .collect()
}

fn printed(code: &str, draft: &Value) -> BTreeMap<String, String> {
    with_database(|db| print_map(db, kind_of(code)?, draft))
        .unwrap()
        .0
}

/// The print document of `draft` (the in-app preview's own fill).
fn document(code: &str, draft: &Value) -> String {
    bir_print::frozen_html::filled_document(slug_of(code), &printed(code, draft)).unwrap()
}

/// The html cell a writer key's join targets.
fn html_cell(slug: &str, key: &str) -> String {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins", "peso_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .find(|j| j["writer_key"] == key)
        .and_then(|j| {
            ["html_id", "peso_html_id"]
                .iter()
                .find_map(|f| j[*f].as_str().map(str::to_string))
        })
        .unwrap_or_else(|| panic!("{slug}: {key} is joined"))
}

fn validate(code: &str, draft: &Value) -> Value {
    call_async(
        "screen_form_validate",
        vec![code.into(), json(draft.clone())],
    )
    .unwrap()
}

fn messages(validated: &Value) -> Vec<String> {
    validated["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| e["message"].as_str().map(str::to_string))
        .collect()
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

/// R3 (every form): each box the sheet prints under an item number is
/// drawn under that number. The oracle is writer-cells.json's printed
/// captions (a caption starting with its item number).
#[test]
fn pr8_boxes_sit_under_their_printed_item_numbers() {
    for (code, slug, _) in FORMS {
        let boxes = drawn(code);
        let mut checked = 0;
        for (key, caption) in printed_captions(code, slug) {
            let Some(number) = printed_number(&caption) else {
                continue;
            };
            let Some(at) = boxes.iter().find(|d| d.key == key) else {
                continue;
            };
            assert_eq!(at.no, number, "{code} {key} prints at {caption:?}");
            checked += 1;
        }
        assert!(checked >= 15, "{code}: {checked} boxes checked");
    }
}

/// R3 (every form): no drawn box sits in an unnumbered "Additional
/// details" group, and the page's last part is "Not on the printed sheet".
#[test]
fn pr8_no_box_is_left_in_additional_details() {
    for (code, _, _) in FORMS {
        for d in drawn(code) {
            assert!(
                d.label != "Additional details",
                "{code} {} in {} item {:?}",
                d.key,
                d.part,
                d.no
            );
        }
    }
}

/// Everything the printed layout leaves out goes to the last part, "Not on
/// the printed sheet", and only these boxes of it are drawn: the profile's
/// line of business, which no sheet prints (writer-cells not_joined).
#[test]
fn pr8_only_named_boxes_are_drawn_off_the_printed_sheet() {
    for (code, slug, expected) in [
        ("1702RT", "1702rt-2018c", vec![]),
        ("1702MX", "1702mx-2018c", vec![]),
        ("1702EX", "1702ex-2018", vec!["frm1702EX:txtLOB"]),
        (
            "1707A",
            "1707a-2021",
            vec!["frm1707Av2021:txtLineOfBusiness"],
        ),
    ] {
        let off: Vec<String> = drawn(code)
            .into_iter()
            .filter(|d| d.part == "Not on the printed sheet")
            .map(|d| d.key)
            .collect();
        assert_eq!(off, expected, "{code}");
        let cells = writer_cells(slug);
        for key in &off {
            assert!(
                cells["not_joined"].as_array().unwrap().iter().any(|e| {
                    e["writer_key"] == *key
                        || e["writer_keys"]
                            .as_array()
                            .is_some_and(|keys| keys.iter().any(|k| k == key))
                }),
                "{code} {key}: the sheet does not print it"
            );
        }
        let form = schema_entry(code).unwrap();
        let last = form["parts"].as_array().unwrap().last().unwrap();
        assert_eq!(last["n"], "Not on the printed sheet", "{code}");
    }
}

const A: &str = "frm1707Av2021:";

fn a(stem: &str) -> String {
    format!("{A}{stem}")
}

/// 1707A R3.0, R3.1, R3.2, R3.5, R5.2, manual-0, manual-2: each printed
/// item holds only its own boxes, in the sheet's order (hand list from
/// html-frozen/1707a-2021/index.html: items 1-5, Part I 6-12A, Part II
/// 13-21); the TIN item is a TIN comb; the penalties are 20A-20C.
#[test]
fn pr8_form_1707a_items_hold_their_printed_boxes() {
    let part_i = "Part I · Background Information";
    let part_ii = "Part II · Computation of Tax";
    for (part, no, stems) in [
        ("Page 1", "4", vec!["txtI3Sheets"]),
        ("Page 1", "5", vec!["rdoI4ATCII030", "rdoI4ATCIC110"]),
        (
            part_i,
            "6",
            vec!["txtI5TIN1", "txtI5TIN2", "txtI5TIN3", "txtI5BranchMask"],
        ),
        (part_i, "7", vec!["rdoI6RDO"]),
        (part_i, "8", vec!["txtI8RegisteredName"]),
        (part_i, "9", vec!["txtI9RegisteredAddress"]),
        (part_i, "9A", vec!["txtI10ZipCode"]),
        (part_i, "10", vec!["txtI7TelNo"]),
        (part_i, "11", vec!["txtI11Email"]),
        (
            part_i,
            "12",
            vec!["rdoI11TaxTreatyYes", "rdoI11TaxTreatyNo"],
        ),
        (part_i, "12A", vec!["txtI11ASpecify"]),
        (part_ii, "18A", vec!["txtI16ATotalTaxPaid"]),
        (part_ii, "18B", vec!["txtI16BTotalTaxPaid"]),
        (part_ii, "18C", vec!["txtI16CTotalTaxPaid"]),
        (part_ii, "20A", vec!["txtI18ASurcharge"]),
        (part_ii, "20B", vec!["txtI18BInterest"]),
        (part_ii, "20C", vec!["txtI18CCompromise"]),
        (part_ii, "20D", vec!["txtI18DPenalties"]),
    ] {
        let expected: Vec<String> = stems.iter().map(|s| a(s)).collect();
        assert_eq!(item_keys("1707A", part, no), expected, "1707A {part} {no}");
    }
    let text = sheet_text("1707a-2021");
    for caption in [
        "6 Taxpayer Identification Number (TIN) 7 RDO Code 8 Seller's Name",
        "9A ZIP Code 10 Contact Number 11 Email Address",
        "12 Are you availing of tax relief under Special Law 12A If yes, please specify",
        "20 Add: Penalties 20A Surcharge ● 20B Interest ● 20C Compromise",
    ] {
        assert!(text.contains(caption), "{caption}");
    }
    // Item 6 is drawn as one TIN comb: three to five text boxes only
    // (form_page.js isTinItem), the page 2 copies elsewhere and hidden.
    let tin: Vec<Drawn> = drawn("1707A")
        .into_iter()
        .filter(|d| d.part == part_i && d.no == "6")
        .collect();
    assert_eq!(tin.len(), 4);
    assert!(tin.iter().all(|d| d.t == "text"), "{tin:?}");
    for stem in [
        "txtI5TIN1P2",
        "txtI5TIN2P2",
        "txtI5TIN3P2",
        "txtI5BranchMaskP2",
        "txtI8RegisteredNameP2",
        "txtI4TCII030",
        "txtI4ATCIC110",
        "txtS1I20TotalCost",
        "txtS1I20TotalSellingPrice",
        "txtS2I21TotalSellingPrice",
        "txtS2I21TotalCost",
    ] {
        assert!(schema_keys("1707A").contains(&a(stem)), "{stem}");
        assert!(
            !drawn_keys("1707A").contains(&a(stem)),
            "{stem} is not drawn"
        );
    }
    for key in ["ModalSched1Length", "ModalSched2Length"] {
        assert!(!drawn_keys("1707A").contains(key), "{key}");
    }
    for stem in [
        "txtS1ModalTotalSellingPrice",
        "txtS1ModalTotalCost",
        "txtS1ModalTotalCapitalGains",
        "txtS1ModalTotalTaxPaid",
        "txtS2ModalTotalSellingPrice",
        "txtS2ModalTotalCost",
        "txtS2ModalTotalCapitalLoss",
    ] {
        assert!(!drawn_keys("1707A").contains(&a(stem)), "{stem}");
    }
}

/// Item 7 RDO Code on every form (1702RT preview.0; 1702MX preview.0;
/// 1702EX R2.2, preview.0, manual-1; 1707A R2.2, preview.0, manual-0): the
/// box drawn at item 7 is the one writer-cells.json joins at "7 RDO Code",
/// filled from the profile and locked with Edit, with the RDO list; the
/// edit stays on this return, prints in that cell, and never reaches the
/// profile. The duplicate the extracted schema drew instead is not drawn.
#[test]
fn pr8_item_7_rdo_comes_from_the_profile_and_prints_on_every_form() {
    setup();
    for (code, key, duplicate) in [
        (
            "1702RT",
            "frm1702RT:drpPg1Pt1I7RDOCode",
            Some("frm1702RT:txtRDO"),
        ),
        (
            "1702MX",
            "frm1702MX:drpPg1Pt1I7RDO",
            Some("frm1702MX:txtPg1Pt1I7RDO"),
        ),
        ("1702EX", "frm1702EX:hdnPg1Pt1I7RDO", None),
        ("1707A", "frm1707Av2021:rdoI6RDO", None),
    ] {
        let slug = slug_of(code);
        let joins = writer_cells(slug);
        assert!(
            joins["joins"]
                .as_array()
                .unwrap()
                .iter()
                .any(|j| j["writer_key"] == key && j["printed"] == "7 RDO Code"),
            "{code}: writer-cells prints {key} at 7 RDO Code"
        );
        let at = placed(code, key);
        assert_eq!(
            (at.part.as_str(), at.no.as_str(), at.t.as_str()),
            ("Part I · Background Information", "7", "sel"),
            "{code}"
        );
        assert_eq!(item_keys(code, &at.part, "7"), [key], "{code}");
        if let Some(duplicate) = duplicate {
            assert!(schema_keys(code).contains(duplicate), "{code}");
            assert!(!drawn_keys(code).contains(duplicate), "{code} {duplicate}");
        }
        let opened = open(code, page_period_of(code));
        assert_eq!(opened["values"][key], "039", "{code}");
        assert_eq!(
            opened["locks"][key],
            agent::Lock::Profile.reason(),
            "{code}"
        );
        assert!(strings(&opened["profile_edit"]).contains(key), "{code}");
        assert!(
            opened["options"][key]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["value"] == "040"),
            "{code}"
        );
        let cell = html_cell(slug, key);
        assert_eq!(
            bir_print::frozen_html::printed_text(&document(code, &opened["draft"]), &cell),
            "039",
            "{code}"
        );
        let refused = fill(code, &opened["draft"], json!({ key: "040" }));
        assert!(refused["rejected"].get(key).is_some(), "{code}");
        let edited = fill_unlocked(code, &opened["draft"], json!({ key: "040" }), &[key]);
        assert_eq!(edited["rejected"], json!({}), "{code}");
        assert_eq!(edited["draft"]["rdo_code"], "040", "{code}");
        assert_eq!(
            bir_print::frozen_html::printed_text(&document(code, &edited["draft"]), &cell),
            "040",
            "{code}"
        );
    }
    let profile = with_database(|db| {
        db.get_profile_by_tin(TIN)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(profile.rdo_code, "039", "the profile is never written");
}

/// 1707A manual-1: the page 2 header TIN is not drawn (form_1707a.rs
/// keeps loadBGData's copy, whose third part repeats the second, for the
/// XML); the print's page 2 TIN comes from the TIN itself.
#[test]
fn pr8_form_1707a_page_2_tin_prints_from_the_tin() {
    setup();
    let opened = open("1707A", 0);
    assert_eq!(
        opened["values"][a("txtI5TIN3P2")],
        "456",
        "the XML copy is unchanged"
    );
    assert!(!drawn_keys("1707A").contains(&a("txtI5TIN3P2")));
    let map = printed("1707A", &opened["draft"]);
    assert!(
        map["derived:tin_digits"].starts_with("123456788"),
        "{}",
        map["derived:tin_digits"]
    );
    let cell = html_cell("1707a-2021", "derived:tin_digits");
    let shown = bir_print::frozen_html::printed_text(&document("1707A", &opened["draft"]), &cell);
    assert!(shown.starts_with("123456788"), "{shown}");
}

/// 1707A preview.1 (items 12 and 12A): item 12's radios are the draft's
/// tax_relief (form_1707a.rs writes Yes when it holds a relief). Yes opens
/// 12A blank, which validation then asks for; the typed relief prints; No
/// clears it, after the page asks.
#[test]
fn pr8_form_1707a_item_12_is_the_returns_tax_relief() {
    setup();
    let (yes, no, specify) = (
        a("rdoI11TaxTreatyYes"),
        a("rdoI11TaxTreatyNo"),
        a("txtI11ASpecify"),
    );
    let opened = open("1707A", 0);
    assert!(opened["locks"].get(&yes).is_none() && opened["locks"].get(&no).is_none());
    assert_eq!(opened["values"][&no], "true");
    let picked = fill("1707A", &opened["draft"], pick(&[&yes, &no], &yes));
    assert_eq!(picked["rejected"], json!({}));
    assert_eq!(picked["draft"]["tax_relief"], "");
    assert_eq!(picked["values"][&yes], "true");
    assert_eq!(picked["values"][&no], "false");
    assert!(
        messages(&validate("1707A", &picked["draft"]))
            .contains(&"Please specify the tax relief on item 12A.".to_string())
    );
    let typed = fill("1707A", &picked["draft"], json!({ &specify: "PEZA" }));
    assert_eq!(typed["draft"]["tax_relief"], "PEZA");
    assert!(typed["removes"].is_null());
    let html = document("1707A", &typed["draft"]);
    let yes_cell = writer_cells("1707a-2021")["xbox_joins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["writer_key"] == yes.as_str())
        .unwrap()["html_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(bir_print::frozen_html::printed_text(&html, &yes_cell), "X");
    // No clears 12A: the page is told first, and Yes is the answer again
    // until it continues.
    let cleared = fill("1707A", &typed["draft"], pick(&[&yes, &no], &no));
    assert!(cleared["draft"]["tax_relief"].is_null());
    assert_eq!(cleared["values"][&no], "true");
    assert_eq!(
        cleared["removes"]["message"],
        "Changing item 12 to No clears item 12A (the relief you specified: PEZA). Continue?"
    );
    assert_eq!(cleared["removes"]["done"], "Cleared item 12A");
    // Nothing typed, nothing to ask.
    let plain = fill("1707A", &picked["draft"], pick(&[&yes, &no], &no));
    assert!(plain["removes"].is_null());
    assert!(plain["draft"]["tax_relief"].is_null());
}

/// 1707A R3.6 and the Part III items: the tax agent's boxes and Part III
/// show their printed captions with a note and no box. The official
/// runtime disables every one at load and this app never writes them, so
/// the print leaves them blank.
#[test]
fn pr8_form_1707a_shut_items_show_their_caption_and_a_note() {
    setup();
    let form = schema_entry("1707A").unwrap();
    let part = |name: &str| -> Vec<Value> {
        form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["n"] == name)
            .unwrap_or_else(|| panic!("1707A has {name}"))["items"]
            .as_array()
            .unwrap()
            .clone()
    };
    let iii = part("Part III · Details of Payment");
    let numbers: Vec<&str> = iii.iter().map(|it| it["no"].as_str().unwrap()).collect();
    assert_eq!(numbers, ["22", "23", "24", "25"]);
    assert!(
        iii.iter()
            .all(|it| it["note"].as_str().is_some_and(|n| n.contains("no box")))
    );
    let agent = part("Part II · Computation of Tax").last().unwrap().clone();
    assert!(
        agent["l"]
            .as_str()
            .unwrap()
            .starts_with("Tax Agent Accreditation No.")
    );
    assert!(agent["note"].as_str().is_some_and(|n| n.contains("no box")));
    let text = sheet_text("1707a-2021");
    assert!(text.contains(
        "22 Cash/Bank Debit Memo ● 23 Check ● 24 Tax Debit Memo ● 25 Others (specify below)"
    ));
    assert!(text.contains("Tax Agent Accreditation No./ Date of Issue Date of Expiry"));
    let shut = disabled_at_load("1707a-v2021");
    let opened = open("1707A", 0);
    let map = printed("1707A", &opened["draft"]);
    let mut keys: Vec<String> = [
        "txtPg1Pt4TaxAcredNo",
        "txtPg1Pt4DateOfIssue",
        "txtPg1Pt4DateOfExpiry",
    ]
    .iter()
    .map(|s| a(s))
    .collect();
    for row in ["BankDebitMemo", "Check", "TaxDebitMemo"] {
        keys.extend((1..=4).map(|c| a(&format!("txtPg1Pt2{row}Col{c}"))));
    }
    keys.extend((1..=5).map(|c| a(&format!("txtPg1Pt2OthersCol{c}"))));
    for key in keys {
        assert_eq!(shut.get(&key), Some(&true), "{key} is disabled at load");
        assert!(schema_keys("1707A").contains(&key));
        assert!(!drawn_keys("1707A").contains(&key), "{key}");
        assert!(map.get(&key).is_none_or(|v| v.trim().is_empty()), "{key}");
    }
}

/// 1707A R3.2, R3.5 and the locked-and-blank invariant: the printed row 4
/// says what fills it (the continuation's row 4, or OTHERS), and the
/// computed column E and row 4 say they are computed.
#[test]
fn pr8_form_1707a_row_4_and_column_e_say_what_fills_them() {
    setup();
    let opened = open("1707A", 0);
    for stem in [
        "txtS1C1DateOfTransactionI4",
        "txtS1C2NameOfCorporateStockI4",
        "txtS2C1DateOfTransactionI4",
        "txtS2C2NameOfCorporateStockI4",
    ] {
        assert_eq!(
            opened["waits"][a(stem)],
            rows_annual_corporate::A_ROW4_WAITS,
            "{stem}"
        );
        assert_eq!(opened["lock_notes"][a(stem)], NOTE_COMPUTED, "{stem}");
    }
    for key in [
        a("txtS1C5CapitalGains_1"),
        a("txtS2C5CapitalLoss_9"),
        a("txtS1C5CapitalGainsI2"),
    ] {
        assert_eq!(opened["lock_notes"][&key], NOTE_COMPUTED, "{key}");
    }
    // With four gains the printed row 4 is the continuation's row 4.
    let four = fill(
        "1707A",
        &opened["draft"],
        json!({
            a("txtS1C2NameOfCorporateStock_1"): "D Corp",
            a("txtS1C3SellingPrice_1"): "250",
        }),
    );
    assert_eq!(four["values"][a("txtS1C2NameOfCorporateStockI4")], "D CORP");
    assert_eq!(amount(&four["values"], &a("txtS1C3SellingPriceI4")), 250.0);
    // With a fifth gain the printed row 4 reads OTHERS with the
    // continuation's totals (form_1707a.rs main_row).
    let mut edits = Map::new();
    for (row, price) in [("I1", "100"), ("I2", "100"), ("I3", "100")] {
        edits.insert(a(&format!("txtS1C3SellingPrice{row}")), json!(price));
    }
    edits.insert(a("txtS1C2NameOfCorporateStock_1"), json!("A CORP"));
    edits.insert(a("txtS1C3SellingPrice_1"), json!("200"));
    edits.insert(a("txtS1C2NameOfCorporateStock_2"), json!("B CORP"));
    edits.insert(a("txtS1C3SellingPrice_2"), json!("300"));
    let filled = fill("1707A", &opened["draft"], Value::Object(edits));
    assert_eq!(
        filled["values"][a("txtS1C2NameOfCorporateStockI4")],
        "OTHERS"
    );
    assert_eq!(
        amount(&filled["values"], &a("txtS1C3SellingPriceI4")),
        500.0
    );
}

/// 1707A R3.3, R3.4: item 9's caption is printed whole and item 12's
/// options are "Yes" and "No" (html-frozen/1707a-2021/index.html).
#[test]
fn pr8_form_1707a_labels_read_as_printed() {
    let nine = placed("1707A", &a("txtI9RegisteredAddress"));
    assert!(
        nine.label
            .ends_with("update registered address by using BIR Form No. 1905)"),
        "{}",
        nine.label
    );
    assert!(
        sheet_text("1707a-2021").contains("update registered address by using BIR Form No. 1905)")
    );
    let form = schema_entry("1707A").unwrap();
    let label = |key: &str| -> String {
        form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .flat_map(|it| it["f"].as_array().into_iter().flatten())
            .find(|f| f["k"] == key)
            .map(|f| f["l"].as_str().unwrap().to_string())
            .unwrap()
    };
    assert_eq!(label(&a("rdoI11TaxTreatyYes")), "Yes");
    assert_eq!(label(&a("rdoI11TaxTreatyNo")), "No");
    assert_eq!(placed("1707A", &a("txtI16ApplicableTaxRate")).t, "pct");
    assert!(sheet_text("1707a-2021").contains("15% 16 Applicable Tax Rate"));
}

/// The label a drawn field shows.
fn field_label(code: &str, key: &str) -> String {
    let form = schema_entry(code).unwrap();
    form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .flat_map(|it| it["f"].as_array().into_iter().flatten())
        .find(|f| f["k"] == key)
        .and_then(|f| f["l"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("{code} {key} is a field"))
}

/// The printed caption writer-cells.json gives a key.
fn caption_of(code: &str, key: &str) -> String {
    printed_captions(code, slug_of(code))
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, caption)| caption)
        .unwrap_or_else(|| panic!("{code}: {key} is joined"))
}

/// The radio and option labels of each form read as printed (R3 label
/// findings: 1702RT R3.6; 1702MX R3.9, R3.11; 1702EX R3.6, R3.7; 1707A
/// R3.4): each option's label is the end of its writer-cells xbox caption,
/// without the item's own question or the "ATC" prefix.
#[test]
fn pr8_options_read_as_their_printed_captions() {
    for (code, keys) in [
        (
            "1702RT",
            vec![
                "frm1702RT:rdoPg1I1Calendar",
                "frm1702RT:rdoPg1I3AmmendYes",
                "frm1702RT:rdoPg1I3AmmendNo",
                "frm1702RT:rdoPg1I5Atc",
                "frm1702RT:rdoPg1Pt2I21OverpaymentRefunded",
                "frm1702RT:rdoPg1Pt2I21OverpaymentIssued",
            ],
        ),
        (
            "1702MX",
            vec![
                "frm1702MX:rdoPg1I1Calendar",
                "frm1702MX:rdoPg1I3AmendedYes",
                "frm1702MX:chkPg1I5ATCR1",
                "frm1702MX:rdoPg1Pt2I21Refund",
                "frm1702MX:rdoPg1Pt2I21IssueTCC",
            ],
        ),
        (
            "1702EX",
            vec![
                "frm1702EX:rdoPg1I1Calendar",
                "frm1702EX:rdoPg1I3AmendedYes",
                "frm1702EX:rdoPg1I3AmendedNo",
                "frm1702EX:rdoPg1I4ShortPeriodYes",
                "frm1702EX:rdoPg1I4ShortPeriodNo",
                "frm1702EX:rdoPg1I5ATCR1C3",
                "frm1702EX:rdoPg1I5ATCR2C3",
                "frm1702EX:rdoPg1OverpaymentRefund",
                "frm1702EX:rdoPg1OverpaymentTCC",
                "frm1702EX:rdoPg1OverpaymentCarryOver",
            ],
        ),
        (
            "1707A",
            vec![
                "frm1707Av2021:rdoI11TaxTreatyYes",
                "frm1707Av2021:rdoI11TaxTreatyNo",
            ],
        ),
    ] {
        for key in keys {
            let label = field_label(code, key);
            let caption = caption_of(code, key);
            assert!(
                caption.ends_with(&label),
                "{code} {key}: {label:?} vs printed {caption:?}"
            );
            assert!(!label.starts_with("ATC "), "{code} {key}: {label:?}");
        }
    }
    // 1702MX's instructions A and B: the sheet's own words, without the
    // question repeated (R3.11).
    let text = sheet_text("1702mx-2018c");
    for key in ["frm1702MX:InstAPg2Part4", "frm1702MX:InstBPg2Part4"] {
        let label = field_label("1702MX", key);
        assert!(text.contains(&label), "1702MX {key}: {label:?}");
    }
    // 1702EX item 3: the extracted labels were swapped; Yes is the amended
    // answer (writer-cells "3 Amended Return? Yes").
    assert_eq!(field_label("1702EX", "frm1702EX:rdoPg1I3AmendedYes"), "Yes");
    assert_eq!(field_label("1702EX", "frm1702EX:rdoPg1I3AmendedNo"), "No");
    // Item 1's heading is "For", never its options again.
    for (code, _, _) in FORMS {
        let one: Vec<Drawn> = drawn(code)
            .into_iter()
            .filter(|d| d.part == "Page 1" && d.no == "1")
            .collect();
        assert!(
            !one.is_empty() && one.iter().all(|d| d.label == "For"),
            "{code}: {one:?}"
        );
    }
}

/// R3 labels (1702RT R3.7, R3.8, manual-1; 1702MX R3.10, R3.12, R3.13;
/// 1702EX R3.8; 1707A R3.2, R3.3, R3.5): no drawn label or heading is key
/// text, a counter, or a heading that only repeats its own number; item 9
/// prints its whole caption.
#[test]
fn pr8_no_label_is_key_text_or_a_counter() {
    for (code, slug, _) in FORMS {
        for d in drawn(code) {
            let tail = d.key.rsplit(':').next().unwrap();
            let lower = d.label.to_ascii_lowercase();
            assert!(
                !lower.contains("ctr modal")
                    && !lower.contains("ctrmodal")
                    && !lower.contains("subtotal")
                    && !lower.contains("sub total")
                    && !lower.contains("modal sched")
                    && d.label != tail,
                "{code} {}: {:?}",
                d.key,
                d.label
            );
            assert!(
                !(d.label.starts_with("Item ") && d.label[5..].trim() == d.no),
                "{code} {}: {:?}",
                d.key,
                d.label
            );
        }
        let form = schema_entry(code).unwrap();
        for part in form["parts"].as_array().unwrap() {
            if part["n"] == "Not on the printed sheet" {
                continue;
            }
            for item in part["items"].as_array().unwrap() {
                let heading = item["l"].as_str().unwrap_or("");
                assert!(!heading.contains("Centavos or Less"), "{code}: {heading:?}");
                assert!(!heading.contains(" row "), "{code}: {heading:?}");
                // A heading that is only a key's item ("Item 20T").
                assert!(
                    !(heading.starts_with("Item ") && heading[5..].split_whitespace().count() == 1),
                    "{code}: {heading:?}"
                );
            }
        }
        let nine = drawn(code)
            .into_iter()
            .find(|d| d.part == "Part I · Background Information" && d.no == "9")
            .unwrap();
        assert!(
            nine.label
                .ends_with("update registered address by using BIR Form No. 1905)"),
            "{code}: {}",
            nine.label
        );
        assert!(sheet_text(slug).contains("update registered address by using BIR Form No. 1905)"));
    }
    // 1702RT manual-1: item 49's heading is the printed caption, not the
    // instruction above it.
    assert_eq!(
        placed(
            "1702RT",
            "frm1702RT:txtPg2Pt4I49CreditableTaxWithheldFor4thQuarter"
        )
        .label,
        "Creditable Tax Withheld per BIR Form No. 2307 for the 4th Quarter"
    );
    assert!(
        sheet_text("1702rt-2018c")
            .contains("49 Creditable Tax Withheld per BIR Form No. 2307 for the 4th Quarter")
    );
}

/// No part past the first is a bare page ("Page 5", "Page 6": 1702MX
/// manual-2), and each schedule part holds only its own schedule's boxes
/// (1702MX manual-0, manual-1, manual-3, R3.3; 1702RT manual-0): a key
/// naming a schedule (`Sc2`, `Sc5`) sits in that schedule.
#[test]
fn pr8_schedules_hold_only_their_own_boxes() {
    for (code, _, _) in FORMS {
        let form = schema_entry(code).unwrap();
        for (i, part) in form["parts"].as_array().unwrap().iter().enumerate() {
            let name = part["n"].as_str().unwrap();
            assert!(
                i == 0 || !name.starts_with("Page "),
                "{code}: part {name:?}"
            );
        }
    }
    let schedule_of = |key: &str| -> Option<String> {
        let tail = key.rsplit(':').next().unwrap();
        let at = tail.find("Sc")?;
        let digits: String = tail[at + 2..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        (!digits.is_empty()).then_some(digits)
    };
    for (prefix, number) in [
        ("Schedule 2 ·", "2"),
        ("Schedule 3 ·", "3"),
        ("Schedule 4 ·", "4"),
        ("Schedule 5 ·", "5"),
        ("Schedule 6 ·", "6"),
        ("Schedule 7 ·", "7"),
        ("Schedule 8 ·", "8"),
        ("Schedule 9 ·", "9"),
        ("Schedule 10 ·", "10"),
    ] {
        let boxes: Vec<Drawn> = drawn("1702MX")
            .into_iter()
            .filter(|d| d.part.starts_with(prefix))
            .collect();
        assert!(!boxes.is_empty(), "1702MX {prefix}");
        for d in boxes {
            // Schedule 10 item 10 keeps the official `Sc2Itm10` key.
            if number == "10" && d.key.contains("Sc2Itm10") {
                continue;
            }
            if let Some(schedule) = schedule_of(&d.key) {
                assert_eq!(schedule, number, "1702MX {} in {}", d.key, d.part);
            }
        }
    }
    // Schedule 1 holds the basis of relief only (txtPg2Pt4I31 to I36).
    let one: Vec<Drawn> = drawn("1702MX")
        .into_iter()
        .filter(|d| d.part.starts_with("Schedule 1 ·"))
        .collect();
    assert_eq!(one.len(), 3 * 5 + 1);
    for d in one {
        let tail = d.key.rsplit(':').next().unwrap();
        assert!(
            (31..=36).any(|n| tail.starts_with(&format!("txtPg2Pt4I{n}"))),
            "1702MX Schedule 1 holds {}",
            d.key
        );
    }
    // ZIP only in Part I item 9A; a registered name only in item 8.
    for (code, _, _) in FORMS {
        for d in drawn(code) {
            if d.no == "9A" {
                assert_eq!(
                    d.part, "Part I · Background Information",
                    "{code} {}",
                    d.key
                );
            }
            if d.key.contains("RegisteredName") || d.key.contains("I8Name") {
                assert_eq!(
                    (d.part.as_str(), d.no.as_str()),
                    ("Part I · Background Information", "8"),
                    "{code} {}",
                    d.key
                );
            }
        }
    }
    // 1702RT manual-0: Schedule I item 8 holds Interest alone, Schedule V
    // item 6 its own row, Part IV has no item 8.
    assert_eq!(
        item_keys(
            "1702RT",
            "Schedule I · Ordinary Allowable Itemized Deductions (attach additional sheet/s, if necessary)",
            "8"
        ),
        ["frm1702RT:txtPg3Sc1I8Interest"]
    );
    assert_eq!(
        item_keys(
            "1702RT",
            "Schedule V · Reconciliation of Net Income per Books Against Taxable Income (attach additional sheet/s, if necessary)",
            "6"
        ),
        ["frm1702RT:txtPg4Sc5I6C1", "frm1702RT:txtPg4Sc5I6C2"]
    );
    assert!(item_keys("1702RT", "Part IV · Computation of Tax", "8").is_empty());
}

/// The page 2-4 header copies (TIN, branch, name) are not drawn on any
/// form; the print still fills each page's header from the TIN and the
/// name (1702RT R3.2, R3.3, R3.4, manual-0; 1702MX R3.1, R3.4, R3.5, R3.8,
/// manual-1; 1702EX R2.0, R3.3, R3.5).
#[test]
fn pr8_page_header_copies_are_not_drawn_and_still_print() {
    setup();
    for (code, keys) in [
        (
            "1702RT",
            vec![
                "frm1702RT:txtPg2TIN1",
                "frm1702RT:txtPg3TIN4",
                "frm1702RT:txtPg4TIN2",
                "frm1702RT:txtPg2RegisteredName",
                "frm1702RT:txtPg3RegisteredName",
                "frm1702RT:txtPg4RegisteredName",
            ],
        ),
        (
            "1702MX",
            vec![
                "frm1702MX:txtPg2TIN1",
                "frm1702MX:txtPg3TIN1",
                "frm1702MX:txtPg4TIN4",
                "frm1702MX:txtPg2RegisteredName",
                "frm1702MX:txtPg3RegisteredName",
                "frm1702MX:txtPg4RegisteredName",
                "frm1702MX:txtPg1AttMTIN1",
                "frm1702MX:txtPg2AttMTIN1",
            ],
        ),
        (
            "1702EX",
            vec![
                "frm1702EX:txtPg2TinC1",
                "frm1702EX:txtPg3TinC4",
                "frm1702EX:txtPg2RegisteredName",
                "frm1702EX:txtPg3RegisteredName",
            ],
        ),
    ] {
        for key in keys {
            assert!(schema_keys(code).contains(key), "{code} {key}");
            assert!(!drawn_keys(code).contains(key), "{code} {key} is drawn");
        }
        let opened = open(code, page_period_of(code));
        let map = printed(code, &opened["draft"]);
        assert!(map["derived:tin_digits"].starts_with("123456788"), "{code}");
    }
}

const EX: &str = "frm1702EX:";

fn ex(stem: &str) -> String {
    format!("{EX}{stem}")
}

/// 1702EX R3.0-R3.3, R3.5, R2.0, manual-1: each printed item holds only
/// its own boxes (hand list from html-frozen/1702ex-2018/index.html); the
/// overpayment choice sits under item 22, where the sheet prints it; the
/// line of business, which the sheet does not print, is the only box off
/// the printed sheet; every profile box is in its Part I item and offers
/// Edit.
#[test]
fn pr8_form_1702ex_items_hold_their_printed_boxes() {
    setup();
    let part_i = "Part I · Background Information";
    let part_ii = "Part II · Total Tax Payable";
    for (part, no, stems) in [
        (
            "Page 1",
            "3",
            vec!["rdoPg1I3AmendedYes", "rdoPg1I3AmendedNo"],
        ),
        (
            "Page 1",
            "4",
            vec!["rdoPg1I4ShortPeriodYes", "rdoPg1I4ShortPeriodNo"],
        ),
        ("Page 1", "5", vec!["rdoPg1I5ATCR1C3", "rdoPg1I5ATCR2C3"]),
        (
            part_i,
            "6",
            vec![
                "txtPg1Pt1I6TINC1",
                "txtPg1Pt1I6TINC2",
                "txtPg1Pt1I6TINC3",
                "txtPg1Pt1I6TINC4",
            ],
        ),
        (part_i, "7", vec!["hdnPg1Pt1I7RDO"]),
        (part_i, "8", vec!["txtPg1Pt1I8RegisteredName"]),
        (part_i, "9", vec!["txtPg1Pt1I9RegisteredAddress"]),
        (part_i, "9A", vec!["txPg1I9ZipCode"]),
        (part_i, "10", vec!["txtPg1Pt1I10DateofIncorporation"]),
        (part_i, "11", vec!["txtPg1Pt1I11ContactNumber"]),
        (part_i, "12", vec!["txtPg1Pt1I12Email"]),
        (
            part_i,
            "17",
            vec!["txtPg1Pt1I17EffectivityFrom", "txtPg1Pt1I17EffectivityTo"],
        ),
        (part_ii, "21", vec!["txtPg1Pt2I21PenaltyCompromise"]),
        (
            part_ii,
            "22",
            vec![
                "txtPg1Pt2I22TotalAmtPayable",
                "rdoPg1OverpaymentRefund",
                "rdoPg1OverpaymentTCC",
                "rdoPg1OverpaymentCarryOver",
            ],
        ),
        (part_ii, "23", vec!["txtPg1P2I23NumOfAttachments"]),
    ] {
        let expected: Vec<String> = stems.iter().map(|s| ex(s)).collect();
        assert_eq!(
            item_keys("1702EX", part, no),
            expected,
            "1702EX {part} {no}"
        );
    }
    let text = sheet_text("1702ex-2018");
    assert!(text.contains("TOTAL AMOUNT PAYABLE / (Overpayment) (Sum of Items 20 and 21) If overpayment, mark one (1) box only."));
    assert!(text.contains("6 Taxpayer Identification Number (TIN) 7 RDO Code 8 Registered Name"));
    let opened = open("1702EX", 0);
    let editable = strings(&opened["profile_edit"]);
    for key in strings(&opened["profile"]) {
        let Some(at) = drawn("1702EX").into_iter().find(|d| d.key == key) else {
            assert!(
                key.contains("Pg2") || key.contains("Pg3"),
                "1702EX {key} is drawn"
            );
            continue;
        };
        assert!(editable.contains(&key), "1702EX {key} offers Edit");
        assert!(
            (at.part == part_i && !at.no.is_empty()) || at.part == "Not on the printed sheet",
            "1702EX {key} in {} {}",
            at.part,
            at.no
        );
    }
    let lob = placed("1702EX", &ex("txtLOB"));
    assert_eq!(
        (lob.part.as_str(), lob.label.as_str()),
        ("Not on the printed sheet", "Line of Business")
    );
}

/// 1702EX manual-0, R7.0: item 11 Contact Number is the profile's phone
/// kept as typed: bound to /contact_number with the profile lock
/// (probe.rs PROFILE_NOT_COMPUTED), drawn as a phone box, never money; Edit
/// keeps the edit on this return and it prints at "11 Contact Number". The
/// other non-amounts the finding names are a count and hidden Part III
/// boxes.
#[test]
fn pr8_form_1702ex_contact_number_is_a_phone_from_the_profile() {
    setup();
    let key = ex("txtPg1Pt1I11ContactNumber");
    let def = kind_of("1702EX").unwrap().definition();
    assert!(matches!(
        def.bindings[&key],
        BoundBox {
            binding: Binding::Text { .. },
            lock: Some(agent::Lock::Profile),
            ..
        }
    ));
    let at = placed("1702EX", &key);
    assert_eq!(at.t, "phone");
    assert!(!schema_computed("1702EX", &key));
    let opened = open("1702EX", 0);
    assert_eq!(opened["values"][&key], "0281234567");
    assert_eq!(opened["locks"][&key], agent::Lock::Profile.reason());
    assert!(strings(&opened["profile_edit"]).contains(&key));
    let edited = fill_unlocked(
        "1702EX",
        &opened["draft"],
        json!({ &key: "09171234567" }),
        &[&key],
    );
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(edited["draft"]["contact_number"], "09171234567");
    assert_eq!(edited["values"][&key], "09171234567");
    assert_eq!(caption_of("1702EX", &key), "11 Contact Number");
    let cell = html_cell("1702ex-2018", &key);
    assert_eq!(
        bir_print::frozen_html::printed_text(&document("1702EX", &edited["draft"]), &cell),
        "09171234567"
    );
    assert_eq!(
        placed("1702EX", &ex("txtPg1P2I23NumOfAttachments")).t,
        "int"
    );
    for stem in [
        "txtPg1Pt3I24CashC1",
        "txtPg1Pt3I24CashC2",
        "txtPg1Pt3I25CheckC1",
        "txtPg1Pt3I25CheckC2",
    ] {
        assert!(!drawn_keys("1702EX").contains(&ex(stem)), "{stem}");
    }
}

/// 1702EX R5.2: items 8 and 9 are one box each. The draft keeps one
/// registered name and one registered address; the field map cuts them
/// into the official lines (form_1702ex.rs loadBGData) and the print shapes
/// them over the sheet's combs (derived:address_line1-3), so the cut lines
/// are not drawn and the box shows the whole text.
#[test]
fn pr8_form_1702ex_name_and_address_are_one_box_each() {
    setup();
    let key = ex("txtPg1Pt1I9RegisteredAddress");
    for stem in [
        "txtPg1Pt1I8RegisteredName2",
        "txtPg1Pt1I8RegisteredName3",
        "txtPg1Pt1I9RegisteredAddress2",
        "txtPg1Pt1I9RegisteredAddress3",
    ] {
        assert!(schema_keys("1702EX").contains(&ex(stem)));
        assert!(!drawn_keys("1702EX").contains(&ex(stem)), "{stem}");
    }
    let long = "Unit 1201 Twelfth Floor Example Tower, 1234 Very Long Avenue Corner Another Street, Barangay Sample, Quezon City, Metro Manila";
    let opened = open("1702EX", 0);
    let edited = fill_unlocked("1702EX", &opened["draft"], json!({ &key: long }), &[&key]);
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(edited["draft"]["registered_address"], long);
    assert_eq!(
        edited["values"][&key], long,
        "the box shows the whole address"
    );
    // The official lines, cut by the field map as before, put end to end
    // are what was typed (upper-cased, as loadBGData does).
    let official = agent::field_map(kind_of("1702EX").unwrap(), &edited["draft"]).unwrap();
    let lines: Vec<&str> = ["", "2", "3"]
        .iter()
        .map(|n| official[&format!("{EX}txtPg1Pt1I9RegisteredAddress{n}")].as_str())
        .collect();
    assert!(
        !lines[1].is_empty(),
        "the address runs onto line 2: {lines:?}"
    );
    assert_eq!(
        lines.concat().replace(' ', ""),
        long.to_uppercase().replace(' ', "")
    );
    let map = printed("1702EX", &edited["draft"]);
    assert!(!map["derived:address_line2"].trim().is_empty());
}

/// 1702EX preview.1: items 48 and 49 are the draft's two other credits
/// (bound since PR 8's seed in sample_payloads/form_1702ex.rs) and the
/// schedule rows' amounts are entries (probe.rs USER_CHOSEN): each takes a
/// typed amount and the totals recompute through the form's own compute.
#[test]
fn pr8_form_1702ex_other_credits_and_schedule_rows_take_entries() {
    setup();
    let opened = open("1702EX", 0);
    for key in [
        "txtPg2Pt4I48OtherTaxCrPmtDesc",
        "txtPg2Pt4I48OtherTaxCrPmtAmt",
        "txtPg2Pt4I49OtherTaxCrPmtDesc",
        "txtPg2Pt4I49OtherTaxCrPmtAmt",
        "txtPg3Pt6I17OthersEAmt",
        "txtPg3Pt6S2I2Amount",
    ] {
        assert!(opened["locks"].get(ex(key)).is_none(), "{key} is open");
    }
    let filled = fill(
        "1702EX",
        &opened["draft"],
        json!({
            ex("txtPg2Pt4I48OtherTaxCrPmtDesc"): "Credit A",
            ex("txtPg2Pt4I48OtherTaxCrPmtAmt"): "1500",
            ex("txtPg2Pt4I49OtherTaxCrPmtDesc"): "Credit B",
            ex("txtPg2Pt4I49OtherTaxCrPmtAmt"): "700",
            ex("txtPg3Pt6I17OthersEDesc"): "Supplies",
            ex("txtPg3Pt6I17OthersEAmt"): "300",
            ex("txtPg3Pt6S2I2Description"): "Training",
            ex("txtPg3Pt6S2I2LegalBasis"): "RA 1",
            ex("txtPg3Pt6S2I2Amount"): "400",
        }),
    );
    assert_eq!(filled["rejected"], json!({}));
    let v = &filled["values"];
    assert_eq!(amount(v, &ex("txtPg2Pt4I50TotalTaxCrPmt")), 2200.0);
    assert_eq!(
        amount(v, &ex("txtPg3Pt6S1I18TotOrdinaryAllowableItemDeduc")),
        300.0
    );
    assert_eq!(
        amount(v, &ex("txtPg3Pt6S2I5TotSpecialAllowedItemDeduc")),
        400.0
    );
    // The form keeps descriptions in capitals (form_1702ex.rs recompute).
    assert_eq!(
        filled["draft"]["other_credits"][1]["description"],
        "CREDIT B"
    );
    let html = document("1702EX", &filled["draft"]);
    let cell = html_cell("1702ex-2018", &ex("txtPg2Pt4I49OtherTaxCrPmtDesc"));
    assert_eq!(
        bir_print::frozen_html::printed_text(&html, &cell),
        "CREDIT B"
    );
}

/// 1702EX preview.1 (the pop-ups' last fixed lines) and the
/// locked-and-blank invariant: Schedule 1 item 17i, Schedule 2 item 4 and
/// Schedule 3 items 3, 6 and 8 show the first (more...) row, or OTHERS with
/// the subtotal once the pop-up holds two or more (form_1702ex.rs); they
/// say so and say they are computed.
#[test]
fn pr8_form_1702ex_folded_lines_say_what_fills_them() {
    setup();
    let opened = open("1702EX", 0);
    for stem in [
        "txtPg3Pt6I17OthersIDesc",
        "txtPg3Pt6S2I4Description",
        "txtPg3Pt6S3I3NonDeductExpenseOtherIncomeDesc",
        "txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
        "txtPg3Pt6S3I8SpecialDeductDesc",
    ] {
        assert_eq!(
            opened["waits"][ex(stem)],
            rows_annual_corporate::EX_FOLDED_WAITS,
            "{stem}"
        );
        assert_eq!(opened["lock_notes"][ex(stem)], NOTE_COMPUTED, "{stem}");
    }
    let filled = fill(
        "1702EX",
        &opened["draft"],
        json!({
            ex("txtPg3Pt6S1I17_1Col1"): "Other A",
            ex("txtPg3Pt6S1I17_1Col2"): "100",
        }),
    );
    assert_eq!(filled["values"][ex("txtPg3Pt6I17OthersIDesc")], "OTHER A");
    let filled = fill(
        "1702EX",
        &filled["draft"],
        json!({
            ex("txtPg3Pt6S1I17_2Col1"): "Other B",
            ex("txtPg3Pt6S1I17_2Col2"): "50",
        }),
    );
    assert_eq!(filled["values"][ex("txtPg3Pt6I17OthersIDesc")], "OTHERS");
    assert_eq!(
        amount(&filled["values"], &ex("txtPg3Pt6I17OthersIAmt")),
        150.0
    );
}

/// 1702EX manual-3, R3.1, R3.2: Part III and the signature block show
/// their printed captions with a note and no box: the official runtime
/// disables every one at load and the model does not hold them, so the
/// print leaves them blank.
#[test]
fn pr8_form_1702ex_shut_items_show_their_caption_and_a_note() {
    setup();
    let form = schema_entry("1702EX").unwrap();
    let part = |name: &str| -> Vec<Value> {
        form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["n"] == name)
            .unwrap()["items"]
            .as_array()
            .unwrap()
            .clone()
    };
    let iii = part("Part III · Details of Payment");
    let numbers: Vec<&str> = iii.iter().map(|it| it["no"].as_str().unwrap()).collect();
    assert_eq!(numbers, ["24", "25", "26", "27"]);
    assert!(
        iii.iter()
            .all(|it| it["note"].as_str().is_some_and(|n| n.contains("no box")))
    );
    let sign = part("Part II · Total Tax Payable").last().unwrap().clone();
    assert!(
        sign["l"]
            .as_str()
            .unwrap()
            .starts_with("Signature over Printed Name")
    );
    assert!(sign["note"].as_str().is_some_and(|n| n.contains("no box")));
    let shut = disabled_at_load("1702ex-v2018c");
    let opened = open("1702EX", 0);
    let map = printed("1702EX", &opened["draft"]);
    for stem in [
        "txtPg1Pt3I24CashC1",
        "txtPg1Pt3I24Date",
        "txtPg1Pt3I25CheckC3",
        "txtPg1Pt3I26TaxDebitC2",
        "txtPg1Pt3I27OthersC4",
        "txtPg1Pt2AuthorizedRepresentative",
        "txtPg1Pt2Treasurer",
        "txtPg1Pt2TitleofSignatory",
        "txtPg1Pt2TINofSignatory2",
    ] {
        let key = ex(stem);
        assert_eq!(shut.get(&key), Some(&true), "{key} is disabled at load");
        assert!(!drawn_keys("1702EX").contains(&key), "{key}");
        assert!(
            map.get(&key)
                .is_none_or(|v| v.trim().is_empty() || v == "0"),
            "{key}: {:?}",
            map.get(&key)
        );
    }
}

/// 1702EX R8.0 (not a bug): the sheet prints one penalty line, item 21
/// "Add: Penalty - Compromise", which the official page leaves open for the
/// filer (inventory: not disabled) and form_1702ex.rs keeps as the entered
/// `penalties`; there is no surcharge or interest to compute, so rule 8's
/// computed group does not apply. The entry stays the filer's and item 22
/// recomputes from it.
#[test]
fn pr8_form_1702ex_compromise_stays_the_filers_entry() {
    setup();
    let kind = kind_of("1702EX").unwrap();
    assert!(penalties::groups(kind).is_empty());
    let text = sheet_text("1702ex-2018");
    assert!(text.contains("21 Add: Penalty - Compromise"));
    assert!(!text.contains("Surcharge"));
    let key = ex("txtPg1Pt2I21PenaltyCompromise");
    assert_eq!(disabled_at_load("1702ex-v2018c").get(&key), Some(&false));
    let opened = open("1702EX", 0);
    assert!(opened["locks"].get(&key).is_none());
    let filled = fill("1702EX", &opened["draft"], json!({ &key: "500" }));
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(
        amount(&filled["values"], &ex("txtPg1Pt2I22TotalAmtPayable")),
        500.0
    );
}

const RT: &str = "frm1702RT:";

fn rt(stem: &str) -> String {
    format!("{RT}{stem}")
}

/// 1702RT R3.0, R3.1, R3.5, manual-0: item 2 holds the year-end month and
/// year; the signature block's boxes sit under their printed captions in
/// Part II; Schedule V item 1, which the extracted schema left out, is
/// drawn and prints; the page counters are not drawn.
#[test]
fn pr8_form_1702rt_items_hold_their_printed_boxes() {
    setup();
    assert_eq!(
        item_keys("1702RT", "Page 1", "2"),
        [rt("ddlPg1I2Month"), rt("txtPg1I2Year")]
    );
    let part_ii = "Part II · Total Tax Payable";
    assert_eq!(
        item_keys("1702RT", part_ii, "21"),
        [
            rt("txtPg1Pt2I21TotalAmount"),
            rt("rdoPg1Pt2I21OverpaymentRefunded"),
            rt("rdoPg1Pt2I21OverpaymentIssued"),
            rt("rdoPg1Pt2I21OverpaymentCarried"),
        ]
    );
    for (key, caption) in [
        (
            "txtSignaturePresident",
            "Signature over Printed Name of President/Principal Officer/Authorized Representative",
        ),
        (
            "txtPg1Pt2Signatory1",
            "Signature over Printed Name of President/Principal Officer/Authorized Representative",
        ),
        (
            "txtPg1Pt2SignatoryTin1",
            "Signature over Printed Name of President/Principal Officer/Authorized Representative",
        ),
        (
            "txtSignatureTreasurer",
            "Signature over Printed Name of Treasurer/Assistant Treasurer",
        ),
        (
            "txtPg1Pt2Signatory2",
            "Signature over Printed Name of Treasurer/Assistant Treasurer",
        ),
        (
            "txtPg1Pt2SignatoryTin2",
            "Signature over Printed Name of Treasurer/Assistant Treasurer",
        ),
    ] {
        let at = placed("1702RT", &rt(key));
        assert_eq!(
            (at.part.as_str(), at.no.as_str(), at.label.as_str()),
            (part_ii, "", caption),
            "{key}"
        );
    }
    assert_eq!(
        caption_of("1702RT", &rt("txtSignaturePresident")),
        "Signature over Printed Name of President/Principal Officer/Authorized Representative"
    );
    for key in ["txtCurrentPage", "txtMaxPage", "txtRDO"] {
        assert!(!drawn_keys("1702RT").contains(&rt(key)), "{key}");
    }
    let net = rt("txtPg4Sc5I1NetIncome");
    let at = placed("1702RT", &net);
    assert_eq!((at.no.as_str(), at.t.as_str()), ("1", "money"));
    let opened = open("1702RT", 0);
    let filled = fill("1702RT", &opened["draft"], json!({ &net: "1800000" }));
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(
        filled["draft"]["schedule_5"]["item_1_net_income_or_loss_per_books"],
        json!(1_800_000)
    );
    assert_eq!(
        amount(&filled["values"], &rt("txtPg4Sc5I4Total")),
        1_800_000.0
    );
}

/// 1702RT R4.0 and preview.1 (the Part III dates): item 10 and the four
/// Part III dates take a typed MM/DD/YYYY date into the draft's date, an
/// empty box clears it, anything else is refused; the print fills the
/// sheet's MM, DD and YYYY combs from it (writer-cells date_joins).
#[test]
fn pr8_form_1702rt_dates_are_typed_dates() {
    setup();
    let opened = open("1702RT", 0);
    for (stem, path) in [
        ("txtPg1Pt1I10", "/incorporation_date"),
        ("txtPg1Pt3I23DebitMemoC3Date", "/payment_details/0/date"),
        ("txtPg1Pt3I24CheckC3Date", "/payment_details/1/date"),
        ("txtPg1Pt3I25TaxDebitDate", "/payment_details/2/date"),
        ("txtPg1Pt3I26OthersC3Date", "/payment_details/3/date"),
    ] {
        let key = rt(stem);
        assert_eq!(placed("1702RT", &key).t, "date", "{stem}");
        assert!(opened["locks"].get(&key).is_none(), "{stem} is open");
        let typed = fill("1702RT", &opened["draft"], json!({ &key: "03/15/2016" }));
        assert_eq!(typed["rejected"], json!({}), "{stem}");
        assert_eq!(
            typed["draft"].pointer(path),
            Some(&json!({"year": 2016, "month": 3, "day": 15})),
            "{stem}"
        );
        assert_eq!(typed["values"][&key], "03/15/2016", "{stem}");
        let cleared = fill("1702RT", &typed["draft"], json!({ &key: "" }));
        assert_eq!(cleared["draft"].pointer(path), Some(&Value::Null), "{stem}");
        let refused = fill("1702RT", &typed["draft"], json!({ &key: "2016-03-15" }));
        assert_eq!(
            refused["rejected"][&key],
            rows_annual_corporate::RT_DATE_REFUSAL,
            "{stem}"
        );
        assert_eq!(
            refused["draft"].pointer(path),
            typed["draft"].pointer(path),
            "{stem}"
        );
    }
    let typed = fill(
        "1702RT",
        &opened["draft"],
        json!({ rt("txtPg1Pt1I10"): "03/15/2016" }),
    );
    let cells = writer_cells("1702rt-2018c");
    let join = cells["date_joins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["writer_key"] == rt("txtPg1Pt1I10").as_str())
        .unwrap()
        .clone();
    let html = document("1702RT", &typed["draft"]);
    let part =
        |field: &str| bir_print::frozen_html::printed_text(&html, join[field].as_str().unwrap());
    assert_eq!(
        (part("mm_html_id"), part("dd_html_id"), part("yyyy_html_id")),
        ("03".into(), "15".into(), "2016".into())
    );
}

/// 1702RT preview.1 (item 21's overpayment choice): the three boxes are
/// the draft's part_ii.overpayment_disposition (bound since PR 8's overpaid
/// seed in sample_payloads/form_1702rt.rs); on an overpaid return the
/// chosen box prints and validation stops asking; the form's own compute
/// clears the choice while item 16 is not an overpayment.
#[test]
fn pr8_form_1702rt_overpayment_choice_is_the_drafts() {
    setup();
    let boxes = [
        rt("rdoPg1Pt2I21OverpaymentRefunded").as_str().to_string(),
        rt("rdoPg1Pt2I21OverpaymentIssued"),
        rt("rdoPg1Pt2I21OverpaymentCarried"),
    ];
    let names: Vec<&str> = boxes.iter().map(String::as_str).collect();
    let opened = open("1702RT", 0);
    let overpaid = fill(
        "1702RT",
        &opened["draft"],
        json!({
            rt("txtPg2Pt4I27Sales"): "1000000",
            rt("txtPg2Pt4I48CreditableTaxWithheldFromPrevious"): "5000000",
        }),
    );
    assert!(amount(&overpaid["values"], &rt("txtPg1Pt2I16NetTax")) < 0.0);
    let ask = "Please select an option for Overpayment Radio Button on Page 1 Part II ";
    assert!(
        messages(&validate("1702RT", &overpaid["draft"]))
            .iter()
            .any(|m| m.starts_with(ask.trim_end()))
    );
    let picked = fill("1702RT", &overpaid["draft"], pick(&names, &boxes[0]));
    assert_eq!(picked["rejected"], json!({}));
    assert_eq!(
        picked["draft"]["part_ii"]["overpayment_disposition"],
        "Refund"
    );
    assert_eq!(picked["values"][&boxes[0]], "true");
    assert!(
        !messages(&validate("1702RT", &picked["draft"]))
            .iter()
            .any(|m| m.starts_with(ask.trim_end()))
    );
    let cell = writer_cells("1702rt-2018c")["xbox_joins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["writer_key"] == boxes[0].as_str())
        .unwrap()["html_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        bir_print::frozen_html::printed_text(&document("1702RT", &picked["draft"]), &cell),
        "X"
    );
    // Not an overpayment: the form's compute clears the choice.
    let paid = fill("1702RT", &opened["draft"], pick(&names, &boxes[1]));
    assert!(paid["draft"]["part_ii"]["overpayment_disposition"].is_null());
}

/// 1702RT: the (Add more...) items' amounts and Schedule IV item 1 are
/// entries (probe.rs GATED_ENTRY), and every other derived box says the
/// form computes it (preview.1: Schedule IIIA item 4's B to D).
#[test]
fn pr8_form_1702rt_entries_open_and_computed_boxes_say_so() {
    setup();
    let opened = open("1702RT", 0);
    for stem in [
        "txtPg2Pt4I54C2",
        "txtPg3Sc1I17iC2",
        "txtPg3Sc2I4C3",
        "txtPg4Sc5I3C2",
        "txtPg4Sc5I6C2",
        "txtPg4Sc5I8C2",
        "txtPg4Sc4I1C1",
        "txtPg4Sc4I1C3",
    ] {
        assert!(opened["locks"].get(rt(stem)).is_none(), "{stem} is open");
    }
    let filled = fill(
        "1702RT",
        &opened["draft"],
        json!({ rt("txtPg2Pt4I54C1"): "Credit", rt("txtPg2Pt4I54C2"): "1234" }),
    );
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(
        amount(&filled["values"], &rt("txtPg2Pt4I55TotalTaxCredits")),
        1234.0
    );
    for stem in [
        "txtPg4Sc3AI4C3",
        "txtPg4Sc3AI4C4",
        "txtPg4Sc3AI4C5",
        "txtPg2Pt4I33TotalGross",
        "txtPg1Pt2I20TotalPenalties",
    ] {
        assert_eq!(opened["lock_notes"][rt(stem)], NOTE_COMPUTED, "{stem}");
    }
    assert_eq!(opened["values"][rt("txtPg4Sc3AI4C3")], "0");
    assert_eq!(
        opened["waits"][rt("txtPg4Sc3AI4C1")],
        rows_annual_corporate::RT_NOLCO_YEAR_WAITS
    );
}

/// 1702RT manual-2, 1702MX manual-4: every amount the sheet prints over a
/// whole-peso comb (writer-cells peso_joins) is a peso box showing whole
/// pesos; Number of Attachments is a count shown empty until typed (the new
/// draft keeps the official zeros, which still file).
#[test]
fn pr8_whole_peso_amounts_and_counts() {
    setup();
    for (code, slug) in [("1702RT", "1702rt-2018c"), ("1702MX", "1702mx-2018c")] {
        let cells = writer_cells(slug);
        let def = kind_of(code).unwrap().definition();
        let mut checked = 0;
        for join in cells["peso_joins"].as_array().unwrap() {
            let key = join["writer_key"].as_str().unwrap();
            let Some(at) = drawn(code).into_iter().find(|d| d.key == key) else {
                continue;
            };
            if at.t == "pct" {
                continue;
            }
            assert_eq!(at.t, "money", "{code} {key}");
            let form = schema_entry(code).unwrap();
            let whole = form["parts"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|p| p["items"].as_array().unwrap())
                .flat_map(|it| it["f"].as_array().into_iter().flatten())
                .find(|f| f["k"] == key)
                .is_some_and(|f| f["w"] == true);
            let amount_bound = matches!(
                def.bindings.get(key).map(|b| &b.binding),
                Some(Binding::Amount { .. })
            );
            assert_eq!(whole, !amount_bound, "{code} {key}");
            checked += 1;
        }
        assert!(checked >= 8, "{code}: {checked}");
    }
    for (code, key, zeros) in [
        ("1702RT", "frm1702RT:txtPg1Pt2PagesFilled", "000"),
        ("1702MX", "frm1702MX:txtPg1P2I23NumOfAttachments", "00"),
    ] {
        assert_eq!(placed(code, key).t, "int", "{code}");
        let opened = open(code, 0);
        assert_eq!(
            agent::field_map(kind_of(code).unwrap(), &opened["draft"]).unwrap()[key],
            zeros,
            "{code} files the official zeros"
        );
        assert_eq!(opened["values"][key], "", "{code}");
        let typed = fill(code, &opened["draft"], json!({ key: "2" }));
        assert_eq!(typed["values"][key], "2", "{code}");
    }
}

const MX: &str = "frm1702MX:";

fn mx(stem: &str) -> String {
    format!("{MX}{stem}")
}

/// 1702MX R3.0, validation.0: the date of incorporation (txtPg1Pt1I8) is
/// item 10, where the sheet prints it, so the official message naming
/// "Part I Item 10" sits under item 10.
#[test]
fn pr8_form_1702mx_item_10_is_the_date_of_incorporation() {
    setup();
    let key = mx("txtPg1Pt1I8");
    let at = placed("1702MX", &key);
    assert_eq!(
        (at.part.as_str(), at.no.as_str(), at.t.as_str()),
        ("Part I · Background Information", "10", "date")
    );
    assert_eq!(
        caption_of("1702MX", &key),
        "10 Date of Incorporation/Organization (MM/DD/YYYY)"
    );
    let opened = open("1702MX", 0);
    let validated = validate("1702MX", &opened["draft"]);
    let error = validated["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["message"] == "Please provide Date of Incorporation / Organization (Part I Item 10)."
        })
        .expect("the official message");
    assert!(
        error["keys"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k == key.as_str())
    );
}

/// 1702MX R3.14 and the signature block: Part III items 23-26 and the
/// signatories show their printed captions with a note and no box; the
/// official runtime disables every one at load and the field map never
/// writes them.
#[test]
fn pr8_form_1702mx_shut_items_show_their_caption_and_a_note() {
    setup();
    let form = schema_entry("1702MX").unwrap();
    let iii = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == "Part III · Details of Payment")
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .clone();
    let numbers: Vec<&str> = iii.iter().map(|it| it["no"].as_str().unwrap()).collect();
    assert_eq!(numbers, ["23", "24", "25", "26"]);
    assert!(
        iii.iter()
            .all(|it| it["note"].as_str().is_some_and(|n| n.contains("no box")))
    );
    let shut = disabled_at_load("1702mx-v2018c");
    let opened = open("1702MX", 0);
    let map = printed("1702MX", &opened["draft"]);
    let mut keys: Vec<String> = [
        "txtPg1Pt1I27CheckC3",
        "txtPg1Pt1I27CheckC4",
        "txtPg1Pt1I28TaxDebitC3",
        "txtPg1Pt1I28TaxDebitC4",
        "txtPg1Pt1I29OthersC3",
        "txtPg1Pt1I29OthersC4",
        "txtPg1Pt2TitleofSignatory",
        "txtPg1Pt2TINofSignatory",
        "txtPg1Pt2TitleofSignatory2",
        "txtPg1Pt2TINofSignatory2",
        "txtPg1Pt1I26CashC1",
        "txtPg1Pt1I26CashC2",
    ]
    .iter()
    .map(|s| mx(s))
    .collect();
    keys.extend(
        ["txtPg1Pt1I29Others", "txtPg1Pt1I27CheckC1"]
            .iter()
            .map(|s| mx(s)),
    );
    for key in keys {
        assert_eq!(shut.get(&key), Some(&true), "{key} is disabled at load");
        assert!(!drawn_keys("1702MX").contains(&key), "{key}");
        assert!(map.get(&key).is_none_or(|v| v.trim().is_empty()), "{key}");
    }
}

/// 1702MX R2.2 (not a bug): txtPg1AttMPt5ScAIt6 is not the TIN but an
/// Attachment M annex box (rules/forms/1702mx-v2018c/fields.json: item 6
/// of the annex, at most 10 characters), and the four-page sheet has no
/// annex pages (writer-cells not_joined), so it stays off the page and the
/// print.
#[test]
fn pr8_form_1702mx_annex_box_is_not_the_tin() {
    let key = mx("txtPg1AttMPt5ScAIt6");
    let fields: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{}/rules/forms/1702mx-v2018c/fields.json", root()))
            .unwrap(),
    )
    .unwrap();
    let field = fields["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field_key"] == key.as_str())
        .unwrap();
    assert_eq!(field["constraints"]["max_length"], 10);
    assert!(
        writer_cells("1702mx-2018c")["not_joined"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| {
                e["writer_keys"]
                    .as_array()
                    .is_some_and(|keys| keys.iter().any(|k| k == key.as_str()))
                    && e["reason"].as_str().unwrap().contains("annex")
            })
    );
    assert!(!drawn_keys("1702MX").contains(&key));
}

/// 1702MX Schedule 9 item 1 is typed like items 2 and 3 (probe.rs
/// GATED_ENTRY), and the Schedule 7.1 and 8.1 item 7 years say what fills
/// them.
#[test]
fn pr8_form_1702mx_schedule_9_row_1_and_nolco_years() {
    setup();
    let opened = open("1702MX", 0);
    for stem in [
        "txtPg7Sc9I1",
        "txtPg7Sc9I1CA",
        "txtPg7Sc9I1CB",
        "txtPg7Sc9I1CF",
    ] {
        assert!(opened["locks"].get(mx(stem)).is_none(), "{stem} is open");
    }
    assert_eq!(
        opened["waits"][mx("txtPg3IShed7_7Year")],
        rows_annual_corporate::MX_NOLCO_7_YEAR_WAITS
    );
    assert_eq!(
        opened["waits"][mx("txtPg4IShed8_7Year")],
        rows_annual_corporate::MX_NOLCO_8_YEAR_WAITS
    );
    assert_eq!(
        opened["lock_notes"][mx("txtPg3IShed7_7Year")],
        NOTE_COMPUTED
    );
    // The rates the draft keeps as a PercentInput stay shut with their own
    // reason: they are entries, never "computed".
    assert_ne!(opened["lock_notes"][mx("txtPg2Sc2It14B")], NOTE_COMPUTED);
}

/// 1702MX R3.3, R3.6 and the printed order on every form: within each
/// part the item numbers never go backwards (1702MX's Schedule 5 read 18
/// then 1d), and a schedule's rows follow their schedule.
#[test]
fn pr8_item_numbers_run_forward_in_every_part() {
    let lead = |no: &str| -> Option<u32> {
        let digits: String = no.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    };
    for (code, _, _) in FORMS {
        let form = schema_entry(code).unwrap();
        for part in form["parts"].as_array().unwrap() {
            if part["n"] == "Not on the printed sheet" {
                continue;
            }
            let numbers: Vec<u32> = part["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|it| lead(it["no"].as_str().unwrap_or("")))
                .collect();
            assert!(
                numbers.windows(2).all(|pair| pair[0] <= pair[1]),
                "{code} {}: {numbers:?}",
                part["n"]
            );
        }
    }
    // 1702MX Schedule 5: items 1 to 18, Schedule 6 rows 1 to 5 in their own
    // part.
    let five: Vec<String> = schema_entry("1702MX").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"].as_str().unwrap().starts_with("Schedule 5 ·"))
        .unwrap()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| it["no"].as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = (1..=18).map(|n| n.to_string()).collect();
    assert_eq!(five, expected);
}

/// 1702MX R3.11, 1702EX R3.7: item 13's options are the printed methods,
/// never "Method of deduc …".
#[test]
fn pr8_method_of_deduction_options_read_as_printed() {
    for (code, keys) in [
        (
            "1702MX",
            [
                "frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized",
                "frm1702MX:rdoPg1Pt1I13MethodOfDeducOptional",
            ],
        ),
        (
            "1702EX",
            [
                "frm1702EX:rdoPg1Pt1I13MethodOfDeducItemized",
                "frm1702EX:rdoPg1Pt1I13MethodOfDeducOptional",
            ],
        ),
    ] {
        let text = sheet_text(slug_of(code));
        for key in keys {
            let label = field_label(code, key);
            assert!(
                !label.to_ascii_lowercase().contains("method of deduc"),
                "{code} {key}: {label:?}"
            );
            let head = label.split(" [").next().unwrap();
            let head = head.split(" – ").next().unwrap();
            assert!(text.contains(head), "{code} {key}: {head:?} is printed");
        }
    }
}

// ------------------------------------------------------- PR 8 review fixes

/// What a box shows holds nothing: blank, zero or unticked.
fn shows_nothing(values: &Value, key: &str) -> bool {
    let shown = values[key].as_str().unwrap_or_default().trim();
    shown.is_empty()
        || shown == "false"
        || agent::parse_amount(shown).is_some_and(|amount| amount == 0.0)
}

/// An answer that clears boxes the filer typed (the page's flush with the
/// host's `removes`): it asks first, naming every typed box it clears;
/// Cancel keeps the page's draft, which still holds them; Continue applies
/// the answer and clears them; Undo (form_page.js undoRemoval:
/// screen_form_fill of the draft from before the answer, no edits) brings
/// back that draft exactly.
fn asks_then_undoes(code: &str, typed: &Value, answer: Value, lost: &[String]) {
    for key in lost {
        assert!(
            !shows_nothing(&typed["values"], key),
            "{code}: {key} was typed"
        );
    }
    let asked = fill(code, &typed["draft"], answer.clone());
    assert_eq!(asked["rejected"], json!({}), "{code}: {answer}");
    let removes = &asked["removes"];
    let message = removes["message"].as_str().unwrap_or_else(|| {
        panic!("{code}: {answer} asks first: {removes}");
    });
    assert!(message.ends_with("Continue?"), "{code}: {message}");
    let named: BTreeSet<String> = removes["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().to_string())
        .collect();
    for key in lost {
        assert!(named.contains(key), "{code}: {key} is named in {named:?}");
        assert!(
            shows_nothing(&asked["values"], key),
            "{code}: Continue clears {key}: {:?}",
            asked["values"][key]
        );
    }
    assert_eq!(removes["rows"], json!(named.len()), "{code}");
    assert_eq!(
        removes["done"],
        json!(format!(
            "Cleared {} {}",
            named.len(),
            if named.len() == 1 { "box" } else { "boxes" }
        )),
        "{code}"
    );
    // Cancel and Undo: the draft from before the answer, unchanged.
    let back = fill(code, &typed["draft"], json!({}));
    assert_eq!(back["draft"], typed["draft"], "{code}: exactly as before");
    assert!(
        back["removes"].is_null(),
        "{code}: nothing is asked again: {}",
        back["removes"]
    );
    for key in lost {
        assert_eq!(back["values"][key], typed["values"][key], "{code}: {key}");
    }
}

fn typed(code: &str, draft: &Value, edits: Value) -> Value {
    let filled = fill(code, draft, edits.clone());
    assert_eq!(filled["rejected"], json!({}), "{code}: {edits}");
    assert!(filled["removes"].is_null(), "{code}: {edits}");
    filled
}

/// PR 8 review H1: 1702RT Schedule IV column C (Excess MCIT) rows 2 and 3
/// are the form's (recompute writes MCIT less normal income tax; the
/// official page disables them at load, runtime-control-inventory-v796.json
/// source lines 3188 and 3215, like row 1's at 3161): locked, a typed value
/// is refused, and column B, which the probe once bound them to, is
/// untouched.
#[test]
fn pr8_form_1702rt_schedule_iv_column_c_is_the_forms() {
    setup();
    let inventory = disabled_at_load("1702rt-v2018c");
    let opened = open("1702RT", 0);
    let dated = typed(
        "1702RT",
        &opened["draft"],
        json!({ rt("txtPg1Pt1I10"): "01/02/2015", rt("txtPg4Sc4I2C3"): "4000", rt("txtPg4Sc4I3C3"): "3000" }),
    );
    for (row, c, b) in [(1, "txtPg4Sc4I2C4", 4000), (2, "txtPg4Sc4I3C4", 3000)] {
        assert_eq!(inventory.get(&rt(c)), Some(&true), "{c} is shut at load");
        assert!(opened["locks"].get(rt(c)).is_some(), "{c} is locked");
        assert_eq!(opened["lock_notes"][rt(c)], NOTE_COMPUTED, "{c}");
        let refused = fill("1702RT", &dated["draft"], json!({ rt(c): "999" }));
        assert!(refused["rejected"].get(rt(c)).is_some(), "{c} is refused");
        assert_eq!(refused["draft"], dated["draft"], "{c}: nothing changes");
        assert_eq!(
            refused["draft"]["schedule_4"]["rows"][row]["mcit"],
            json!(b),
            "{c}: column B keeps what was typed"
        );
        assert_eq!(amount(&refused["values"], &rt(c)), f64::from(b), "{c}");
    }
}

/// PR 8 review H2 (1702RT): the Optional Standard Deduction clears
/// Schedules I, II and IIIA, the (Add more...) rows and Part V item 57;
/// MCIT no longer applying (a later date of incorporation) clears Schedule
/// IV and items 42 and 45; Amended Return No clears item 51 (oracle:
/// form_1702rt_official.rs official_recompute). Each asks first and Undo
/// brings everything back.
#[test]
fn pr8_form_1702rt_answers_that_clear_typed_boxes_ask_first() {
    setup();
    let opened = open("1702RT", 0);
    let deduction = [
        rt("rdoPg1Pt1I13ItemizedDeduction"),
        rt("rdoPg1Pt1I13OptionalStandard"),
    ];
    let names: Vec<&str> = deduction.iter().map(String::as_str).collect();
    let itemized = typed("1702RT", &opened["draft"], pick(&names, &deduction[0]));
    let osd_lost = [
        rt("txtPg3Sc1I10PensionTrust"),
        rt("txtPg3Sc2I1C3"),
        rt("txtPg2Pt5I57SpecialAllowable"),
        rt("txtPg3Sc1I17i.1C2"),
    ];
    let typed_osd = typed(
        "1702RT",
        &itemized["draft"],
        json!({
            &osd_lost[0]: "5000",
            rt("txtPg3Sc2I1C1"): "Pension",
            rt("txtPg3Sc2I1C2"): "RA 4917",
            &osd_lost[1]: "600",
            &osd_lost[2]: "100",
            rt("txtPg3Sc1I17i.1C1"): "Training",
            &osd_lost[3]: "250",
        }),
    );
    asks_then_undoes("1702RT", &typed_osd, pick(&names, &deduction[1]), &osd_lost);

    let mcit_lost = [
        rt("txtPg4Sc4I1C3"),
        rt("txtPg2Pt4I42MinimumCorporate"),
        rt("txtPg2Pt4I45IncomeTaxPaymentUnderMCIT"),
    ];
    let typed_mcit = typed(
        "1702RT",
        &opened["draft"],
        json!({
            rt("txtPg1Pt1I10"): "01/02/2015",
            rt("txtPg4Sc4I1C1"): "2024",
            &mcit_lost[0]: "1000",
            &mcit_lost[1]: "5000",
            &mcit_lost[2]: "300",
        }),
    );
    asks_then_undoes(
        "1702RT",
        &typed_mcit,
        json!({ rt("txtPg1Pt1I10"): "03/15/2024" }),
        &mcit_lost,
    );

    let amended = typed(
        "1702RT",
        &opened["draft"],
        json!({ rt("rdoPg1I3Ammend"): "Yes" }),
    );
    let item_51 = rt("txtPg2Pt4I51TaxPaidInReturn");
    let typed_51 = typed("1702RT", &amended["draft"], json!({ &item_51: "700" }));
    asks_then_undoes(
        "1702RT",
        &typed_51,
        json!({ rt("rdoPg1I3Ammend"): "No" }),
        &[item_51],
    );
}

/// PR 8 review H2 (1702MX): the Optional Standard Deduction clears column C
/// of Schedules 5, 6 and 7.1; Amended Return No clears item 27; MCIT no
/// longer applying clears Schedule 9 (oracle: form_1702mx_official.rs
/// official_recompute).
#[test]
fn pr8_form_1702mx_answers_that_clear_typed_boxes_ask_first() {
    setup();
    let opened = open("1702MX", 0);
    let method = mx("rdoPg1Pt1I13MethodOfDeduc");
    let itemized = typed("1702MX", &opened["draft"], json!({ &method: "Itemized" }));
    let osd_lost = [mx("txtPg3Sc5Itm1C"), mx("txtPg6Sc6I1CC")];
    let typed_osd = typed(
        "1702MX",
        &itemized["draft"],
        json!({
            &osd_lost[0]: "5000",
            mx("txtPg6Sc6I1description"): "Pension",
            mx("txtPg6Sc6I1legal"): "RA 4917",
            &osd_lost[1]: "300",
        }),
    );
    asks_then_undoes(
        "1702MX",
        &typed_osd,
        json!({ &method: "Optional" }),
        &osd_lost,
    );

    let amended = typed(
        "1702MX",
        &opened["draft"],
        json!({ mx("rdoPg1I3Amended"): "Yes" }),
    );
    let item_27 = mx("txtPg2Sc3It27C");
    let typed_27 = typed("1702MX", &amended["draft"], json!({ &item_27: "700" }));
    asks_then_undoes(
        "1702MX",
        &typed_27,
        json!({ mx("rdoPg1I3Amended"): "No" }),
        &[item_27],
    );

    let schedule_9 = mx("txtPg7Sc9I1CB");
    let typed_9 = typed(
        "1702MX",
        &opened["draft"],
        json!({ mx("txtPg1Pt1I8"): "01/02/2015", &schedule_9: "1000" }),
    );
    asks_then_undoes(
        "1702MX",
        &typed_9,
        json!({ mx("txtPg1Pt1I8"): "03/15/2024" }),
        &[schedule_9],
    );
}

/// PR 8 review H2 (1702EX): the Optional Standard Deduction clears items
/// 17D to 17I and the Schedule 2 rows (form_1702ex.rs recompute:
/// disableForOSD).
#[test]
fn pr8_form_1702ex_osd_asks_before_clearing_typed_boxes() {
    setup();
    let opened = open("1702EX", 0);
    let method = ex("rdoPg1Pt1I13MethodOfDeduc");
    let itemized = typed("1702EX", &opened["draft"], json!({ &method: "Itemized" }));
    let lost = [
        ex("txtPg3Pt6S1I1Ammortization"),
        ex("txtPg3Pt6I17OthersDAmt"),
        ex("txtPg3Pt6S2I1Amount"),
    ];
    let typed_osd = typed(
        "1702EX",
        &itemized["draft"],
        json!({
            &lost[0]: "5000",
            ex("txtPg3Pt6I17OthersDDesc"): "Training",
            &lost[1]: "200",
            ex("txtPg3Pt6S2I1Description"): "Pension",
            ex("txtPg3Pt6S2I1LegalBasis"): "RA 4917",
            &lost[2]: "300",
        }),
    );
    asks_then_undoes("1702EX", &typed_osd, json!({ &method: "Optional" }), &lost);
}

/// PR 8 review M2: item 21's overpayment choice, which recompute clears
/// once item 16 is no longer an overpayment, is a typed answer the page
/// asks about first (1702RT and 1702MX).
#[test]
fn pr8_overpayment_choice_cleared_by_an_amount_asks_first() {
    setup();
    for (code, sales, credit, net, refund) in [
        (
            "1702RT",
            rt("txtPg2Pt4I27Sales"),
            rt("txtPg2Pt4I48CreditableTaxWithheldFromPrevious"),
            rt("txtPg1Pt2I16NetTax"),
            rt("rdoPg1Pt2I21OverpaymentRefunded"),
        ),
        (
            "1702MX",
            mx("txtPg2Sc2Itm1C"),
            mx("txtPg2Sc3It20C"),
            mx("txtPg1Pt2I16NetTaxPayable"),
            mx("rdoPg1Pt2I21Refund"),
        ),
    ] {
        let opened = open(code, 0);
        let overpaid = typed(
            code,
            &opened["draft"],
            json!({ &sales: "1000000", &credit: "5000000" }),
        );
        assert!(amount(&overpaid["values"], &net) < 0.0, "{code}");
        let chosen = typed(code, &overpaid["draft"], json!({ &refund: "true" }));
        assert_eq!(
            chosen["draft"]["part_ii"]["overpayment_disposition"], "Refund",
            "{code}"
        );
        asks_then_undoes(code, &chosen, json!({ &credit: "0" }), &[refund]);
    }
}

/// PR 8 review H3: 1702MX's rates (Schedule 1 item 4, Schedule 2 item 14 B
/// and C) are typed as percents: the draft keeps hundredths and the lexeme
/// the official field map writes (form_1702mx_official.rs: item 4 one
/// decimal, items 14 two), so the upload is the official one; 15B and 15C
/// follow; validation passes with the IPA relief complete. A rate with more
/// decimals than the field map writes, or of 100 or more, is refused.
#[test]
fn pr8_form_1702mx_rates_are_typed_percents() {
    setup();
    let opened = open("1702MX", 0);
    for key in [
        "txtPg2Pt4I34SpecialTaxRate",
        "txtPg2Sc2It14B",
        "txtPg2Sc2It14C",
    ] {
        assert_eq!(placed("1702MX", &mx(key)).t, "pct", "{key}");
        assert!(opened["locks"].get(mx(key)).is_none(), "{key} is open");
        assert_eq!(opened["values"][mx(key)], "", "{key}: blank until typed");
    }
    let relief = typed(
        "1702MX",
        &opened["draft"],
        json!({
            mx("Inst"): "APg2Part4",
            mx("rdoPg1Pt1I13MethodOfDeduc"): "Itemized",
            mx("txtPg2Sc2Itm1B"): "1000000",
            mx("txtPg2Sc2Itm1C"): "2000000",
            mx("txtPg2Pt4I31CB"): "PEZA",
            mx("txtPg2Pt4I32CB"): "RA 7916",
            mx("txtPg2Pt4I33CB"): "Manufacturing",
            mx("txtPg2Pt4I35CB"): "01/01/2020",
            mx("txtPg2Pt4I36CB"): "12/31/2030",
        }),
    );
    let relief_errors = |validated: &Value| -> Vec<String> {
        validated["errors"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| {
                let field = e["field"].as_str().unwrap_or_default();
                field.starts_with("relief") || field.starts_with("schedule_2.item_14")
            })
            .map(|e| e["message"].as_str().unwrap().to_string())
            .collect()
    };
    assert!(
        !relief_errors(&validate("1702MX", &relief["draft"])).is_empty(),
        "without the special rate the relief is incomplete"
    );
    let rated = typed(
        "1702MX",
        &relief["draft"],
        json!({
            mx("txtPg2Pt4I34SpecialTaxRate"): "5",
            mx("txtPg2Sc2It14B"): "5",
            mx("txtPg2Sc2It14C"): "25%",
        }),
    );
    let draft = &rated["draft"];
    assert_eq!(
        draft["relief_basis"]["special_tax_rate"],
        json!({"hundredths": 500, "raw": "5.0"})
    );
    assert_eq!(
        draft["schedule_2"]["item_14_special_rate"],
        json!({"hundredths": 500, "raw": "5.00"})
    );
    assert_eq!(
        draft["schedule_2"]["item_14_regular_rate"],
        json!({"hundredths": 2500, "raw": "25.00"})
    );
    let map = agent::field_map(kind_of("1702MX").unwrap(), draft).unwrap();
    assert_eq!(map[&mx("txtPg2Pt4I34SpecialTaxRate")], "5.0");
    assert_eq!(map[&mx("txtPg2Sc2It14B")], "5.00");
    assert_eq!(map[&mx("txtPg2Sc2It14C")], "25.00");
    assert_eq!(rated["values"][mx("txtPg2Sc2It14B")], "5.00");
    assert_eq!(amount(&rated["values"], &mx("txtPg2Sc2It15B")), 50_000.0);
    let item_13c = amount(&rated["values"], &mx("txtPg2Sc2It13C"));
    assert!(item_13c > 0.0);
    assert_eq!(
        amount(&rated["values"], &mx("txtPg2Sc2It15C")),
        (item_13c * 0.25 + 0.5).floor()
    );
    assert_eq!(
        relief_errors(&validate("1702MX", draft)),
        Vec::<String>::new(),
        "the IPA relief is complete"
    );
    for (key, text) in [
        ("txtPg2Pt4I34SpecialTaxRate", "5.25"),
        ("txtPg2Sc2It14B", "5.125"),
        ("txtPg2Sc2It14C", "100"),
        ("txtPg2Sc2It14C", "abc"),
    ] {
        let refused = fill("1702MX", draft, json!({ mx(key): text }));
        assert!(refused["rejected"].get(mx(key)).is_some(), "{key} {text}");
        assert_eq!(refused["draft"], *draft, "{key} {text}");
    }
    let cleared = typed("1702MX", draft, json!({ mx("txtPg2Sc2It14B"): "" }));
    assert_eq!(
        cleared["draft"]["schedule_2"]["item_14_special_rate"],
        json!({"hundredths": null, "raw": ""})
    );
    assert_eq!(cleared["values"][mx("txtPg2Sc2It14B")], "");
}

/// PR 8 review H4: 1702RT item 42 MCIT Due is the filer's entry while MCIT
/// applies (rules/forms/1702rt-v2018c/calculations.json 1702rt-item42-mcit:
/// "2020+ leaves the enabled value user-controlled"; the page enables it
/// with enableMCITFields): a 2026 return of a corporation incorporated in
/// 2019 takes it and item 43 follows; before the fourth year it is refused.
#[test]
fn pr8_form_1702rt_item_42_is_an_entry_once_mcit_applies() {
    setup();
    let item_42 = rt("txtPg2Pt4I42MinimumCorporate");
    let def = kind_of("1702RT").unwrap().definition();
    assert!(
        def.fillable().any(|entry| entry.key == item_42),
        "item 42 is an entry"
    );
    let opened = open("1702RT", 0);
    assert_eq!(opened["year"], json!(2026));
    assert!(opened["locks"].get(&item_42).is_none());
    let incorporated = typed(
        "1702RT",
        &opened["draft"],
        json!({ rt("txtPg1Pt1I10"): "06/30/2019" }),
    );
    let mcit = typed(
        "1702RT",
        &incorporated["draft"],
        json!({ &item_42: "50000" }),
    );
    assert_eq!(mcit["draft"]["part_iv"]["item_42_mcit_due"], json!(50_000));
    assert_eq!(amount(&mcit["values"], &item_42), 50_000.0);
    assert_eq!(
        amount(&mcit["values"], &rt("txtPg2Pt4I43TotalIncomeTax")),
        50_000.0,
        "item 43 is the larger of items 41 and 42"
    );
    let young = typed(
        "1702RT",
        &opened["draft"],
        json!({ rt("txtPg1Pt1I10"): "06/30/2024" }),
    );
    let refused = fill("1702RT", &young["draft"], json!({ &item_42: "50000" }));
    assert_eq!(
        refused["rejected"][&item_42],
        rows_annual_corporate::RT_MCIT_REFUSAL
    );
    assert_eq!(refused["draft"], young["draft"]);
}

/// PR 8 review M1: a 1702RT (Add more...) item whose table holds two or
/// more rows reads OTHERS and the subtotal (official_recompute,
/// saveModalTable): its boxes are then locked, say so, and refuse a write;
/// with one row or none they stay the filer's entries.
#[test]
fn pr8_form_1702rt_folded_items_are_locked_while_folded() {
    setup();
    let opened = open("1702RT", 0);
    let (description, amount_key) = (rt("txtPg3Sc1I17iC1"), rt("txtPg3Sc1I17iC2"));
    for key in [&description, &amount_key] {
        assert!(opened["locks"].get(key).is_none(), "{key} is open");
    }
    let one = typed(
        "1702RT",
        &opened["draft"],
        json!({ &description: "Training", &amount_key: "250" }),
    );
    assert_eq!(amount(&one["values"], &amount_key), 250.0);
    let folded = typed(
        "1702RT",
        &opened["draft"],
        json!({
            rt("txtPg3Sc1I17i.1C1"): "Training",
            rt("txtPg3Sc1I17i.1C2"): "100",
            rt("txtPg3Sc1I17i.2C1"): "Seminars",
            rt("txtPg3Sc1I17i.2C2"): "200",
        }),
    );
    assert_eq!(folded["values"][&description], "OTHERS");
    assert_eq!(amount(&folded["values"], &amount_key), 300.0);
    for key in [&description, &amount_key] {
        let refused = fill("1702RT", &folded["draft"], json!({ key: "5" }));
        assert_eq!(
            refused["rejected"][key],
            format!("`{key}` is {}", rows_annual_corporate::RT_FOLDED_LOCK)
        );
        assert_eq!(refused["draft"], folded["draft"], "{key}");
    }
    save("1702RT", &folded["draft"]);
    let reopened = open("1702RT", 0);
    for key in [&description, &amount_key] {
        assert_eq!(
            reopened["locks"][key],
            rows_annual_corporate::RT_FOLDED_LOCK,
            "{key}"
        );
    }
    // Schedule II item 4 folds its legal basis too.
    assert_eq!(
        reopened["locks"].get(rt("txtPg3Sc2I4C2")),
        None,
        "an unfolded table leaves its item open"
    );
}

/// PR 8 review L2: a 1702RT date takes a four-digit year only.
#[test]
fn pr8_form_1702rt_dates_need_a_four_digit_year() {
    setup();
    let opened = open("1702RT", 0);
    let key = rt("txtPg1Pt1I10");
    for text in ["03/15/24", "3/15/2024", "03-15-2024", "03/15/20241"] {
        let refused = fill("1702RT", &opened["draft"], json!({ &key: text }));
        assert_eq!(
            refused["rejected"][&key],
            rows_annual_corporate::RT_DATE_REFUSAL,
            "{text}"
        );
        assert_eq!(refused["draft"], opened["draft"], "{text}");
    }
}

/// PR 8 review L3: Number of Attachments shows empty only while a draft
/// holds the new draft's default (`000`, `00`); a typed 0 shows as typed.
#[test]
fn pr8_a_typed_zero_attachment_count_shows() {
    setup();
    for (code, key) in [
        ("1702RT", "frm1702RT:txtPg1Pt2PagesFilled"),
        ("1702MX", "frm1702MX:txtPg1P2I23NumOfAttachments"),
    ] {
        let opened = open(code, 0);
        assert_eq!(opened["values"][key], "", "{code}: the default shows empty");
        let zero = typed(code, &opened["draft"], json!({ key: "0" }));
        assert_eq!(zero["draft"]["number_of_attachments"], "0", "{code}");
        assert_eq!(zero["values"][key], "0", "{code}: a typed 0 shows");
    }
}

/// Maintainer decision D6 (2026-10-18; replaces review item L6's note): a
/// message naming another item number than the printed sheet gives the
/// thing it names reads the printed number. The checks are unchanged.
/// Oracle: the frozen sheets' item numbers, as the page draws them.
#[test]
fn pr8_messages_name_the_printed_item_numbers() {
    setup();
    // 1707A validation.0: the year ended is printed item 2.
    let opened = open("1707A", 0);
    for stem in ["txtI1YearEndMonth", "txtI1YearEndDay", "txtI1YearEndYear"] {
        assert_eq!(placed("1707A", &a(stem)).no, "2", "{stem}");
    }
    assert_eq!(placed("1707A", &a("txtI3Sheets")).no, "4");
    assert_eq!(placed("1707A", &a("txtI16BTotalTaxPaid")).no, "18B");
    let future = fill(
        "1707A",
        &opened["draft"],
        json!({ a("txtI1YearEndMonth"): "12", a("txtI1YearEndDay"): "31", a("txtI1YearEndYear"): "2099" }),
    );
    let found = messages(&validate("1707A", &future["draft"]));
    assert!(
        found
            .iter()
            .any(|m| m == "Year Ended should not be a future date on Item 2."),
        "{found:?}"
    );
    assert!(!found.iter().any(|m| m.contains("on Item 1")), "{found:?}");
    let mut draft = opened["draft"].clone();
    draft["number_of_attached_sheets"] = json!(100);
    let found = messages(&validate("1707A", &draft));
    assert!(
        found
            .iter()
            .any(|m| m == "Item 4 holds at most two digits."),
        "{found:?}"
    );
    // Item 18B's message: the page's own validate clears the amount on an
    // original return first, so the rewording is checked on the message.
    let mut errors = vec![(
        "tax_paid_previous".to_string(),
        "Item 16B applies only to an amended return.".to_string(),
    )];
    rows_annual_corporate::printed_item_numbers(kind_of("1707A").unwrap(), &mut errors);
    assert_eq!(errors[0].1, "Item 18B applies only to an amended return.");

    // 1702RT: Part III's payment rows are items 23 to 26.
    let rt_open = open("1702RT", 0);
    let mut payments = Vec::new();
    for (stem, item) in [
        ("txtPg1Pt3I23DebitMemoC4Amount", "23"),
        ("txtPg1Pt3I24CheckC4Amount", "24"),
        ("txtPg1Pt3I25TaxDebitC4Amount", "25"),
    ] {
        assert_eq!(placed("1702RT", &rt(stem)).no, item, "{stem}");
        payments.push((rt(stem), item));
    }
    let paid = fill(
        "1702RT",
        &rt_open["draft"],
        Value::Object(
            payments
                .iter()
                .map(|(k, _)| (k.clone(), json!("100")))
                .collect(),
        ),
    );
    let found = messages(&validate("1702RT", &paid["draft"]));
    for (_, item) in &payments {
        let message = format!("Please provide data on Page 1 Part III Item {item}.");
        assert!(found.contains(&message), "{message}: {found:?}");
    }
    // Schedule I's sixth OTHERS row is 17h; the date of incorporation is
    // item 10.
    let mut draft = rt_open["draft"].clone();
    draft["deduction_method"] = json!("Itemized");
    draft["schedule_1"]["other"][4] = json!({"description": "", "amount": 500});
    draft["incorporation_date"] = json!({"year": 2024, "month": 1, "day": 2});
    draft["schedule_4"]["rows"][0]["year"] = json!("2023");
    let found = messages(&validate("1702RT", &draft));
    assert!(
        found.contains(&"Please provide data on Page 3 Schedule 1 Item 17h.".to_string()),
        "{found:?}"
    );
    assert_eq!(placed("1702RT", &rt("txtPg1Pt1I10")).no, "10");
    // (Recompute clears Schedule IV whenever the year could precede the
    // incorporation, so the rewording is checked on the message.)
    let mut errors = vec![(
        "schedule_4.rows[0].year".to_string(),
        "Year incurred cannot be lower than Year of Incorporation [2024] (Page 1 Item 8)."
            .to_string(),
    )];
    rows_annual_corporate::printed_item_numbers(kind_of("1702RT").unwrap(), &mut errors);
    assert_eq!(
        errors[0].1,
        "Year incurred cannot be lower than Year of Incorporation [2024] (Page 1 Item 10)."
    );

    // 1702MX: the registered name is item 8 and the address item 9.
    assert_eq!(placed("1702MX", &mx("txtPg1Pt1I9RegisteredName")).no, "8");
    assert_eq!(
        placed("1702MX", &mx("txtPg1Pt1I10RegisteredAddress")).no,
        "9"
    );
    let mut draft = open("1702MX", 0)["draft"].clone();
    draft["taxpayer_name"] = json!("");
    draft["registered_address"] = json!("");
    let found = messages(&validate("1702MX", &draft));
    for message in [
        "Please provide a Registered Name (Part I Item 8).",
        "Please provide a Registered Address (Part I Item 9).",
    ] {
        assert!(found.iter().any(|m| m == message), "{message}: {found:?}");
    }
}

/// PR 8 review L8: a stored draft of each form opens and saves back with
/// identical JSON (PR 0c's profile_baseline excepted where present), and
/// its field map is unchanged; a rate stored with the older lexeme
/// (PercentInput::from_hundredths: "5.0") keeps it.
#[test]
fn pr8_stored_drafts_open_and_save_unchanged() {
    setup();
    let strip = |mut stored: Value| {
        if let Some(object) = stored.as_object_mut() {
            object.remove("profile_baseline");
        }
        stored
    };
    for (code, edits) in [
        ("1702RT", json!({ rt("txtPg2Pt4I27Sales"): "1000000" })),
        ("1702MX", json!({ mx("txtPg2Sc2Itm1C"): "1000000" })),
        (
            "1702EX",
            json!({ ex("txtPg2Pt4I28SalesReceiptsRevFees"): "1000000" }),
        ),
        ("1707A", json!({ a("txtI3Sheets"): "2" })),
    ] {
        let opened = open(code, 0);
        let mut draft = fill(code, &opened["draft"], edits)["draft"].clone();
        if code == "1702MX" {
            draft["schedule_2"]["item_14_special_rate"] = json!({"hundredths": 500, "raw": "5.0"});
        }
        save(code, &draft);
        let year = opened["year"].as_i64().unwrap();
        let period = opened["period"].as_i64().unwrap();
        let read = || {
            call(
                "get_form_draft",
                vec![TIN.into(), code.into(), year.into(), period.into()],
            )
            .expect("get_form_draft")
        };
        let stored = read();
        let reopened = open(code, 0);
        save(code, &reopened["draft"]);
        let again = read();
        assert_eq!(strip(again.clone()), strip(stored.clone()), "{code}");
        let kind = kind_of(code).unwrap();
        assert_eq!(
            agent::field_map(kind, &again).unwrap(),
            agent::field_map(kind, &stored).unwrap(),
            "{code}"
        );
        if code == "1702MX" {
            assert_eq!(
                again["schedule_2"]["item_14_special_rate"],
                json!({"hundredths": 500, "raw": "5.0"})
            );
        }
    }
}

/// Maintainer decision P2-3 (open 1702MX's others rows as 1702EX's, row by
/// row, only where BIR's runtime enables the box at load and nothing in the
/// port computes it): no Schedule 5 17d-17i amount and no Schedule 6 row
/// 2-4 amount qualifies. The official runtime disables every one at load
/// (runtime-control-inventory-v796.json), where its 17a-17c and row 1
/// amounts are enabled, and fills columns A and B from the attachment
/// schedules (calculation-function-inventory-v796.json computeToMainScD,
/// computeToMainScE), which this port does not hold. They stay locked.
#[test]
fn pr8_form_1702mx_others_rows_stay_as_the_runtime_keeps_them() {
    setup();
    let inventory = disabled_at_load("1702mx-v2018c");
    let opened = open("1702MX", 0);
    let mut shut = Vec::new();
    for letter in ['d', 'e', 'f', 'g', 'h', 'i'] {
        for column in ['A', 'B', 'C'] {
            shut.push(mx(&format!("txtPg3Sc5Itm17{letter}{column}")));
        }
    }
    for ids in [
        ["txtPg6Sc6I2CA", "txtPg3Sc6I2CB", "txtPg6Sc6I2CC"],
        ["txtPg6Sc6I3CA", "txtPg4Sc6I3CB", "txtPg6Sc6I3CC"],
        ["txtPg3Sc6I4CA", "txtPg3Sc6I4CB", "txtPg3Sc6I4CC"],
    ] {
        shut.extend(ids.iter().map(|id| mx(id)));
    }
    for key in &shut {
        assert_eq!(inventory.get(key), Some(&true), "{key} is shut at load");
        assert!(opened["locks"].get(key).is_some(), "{key} stays locked");
        let refused = fill("1702MX", &opened["draft"], json!({ key: "100" }));
        assert!(refused["rejected"].get(key).is_some(), "{key}");
    }
    for key in [
        "txtPg3Sc5Itm17aA",
        "txtPg3Sc5Itm17aC",
        "txtPg6Sc6I1CA",
        "txtPg6Sc6I1CC",
    ] {
        assert_eq!(
            inventory.get(&mx(key)),
            Some(&false),
            "{key} is open at load"
        );
        assert!(opened["locks"].get(mx(key)).is_none(), "{key} is an entry");
    }
}
