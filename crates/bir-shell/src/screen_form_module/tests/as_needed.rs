//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552) against their printed sheets. The oracles are the frozen
//! sheets' own files (html-frozen/<slug>/writer-cells.json captions and
//! joins, index.html's printed text), the official keys and the typed
//! drafts' own fields, and hand lists from the sheets; never this module's
//! own refusal.

use super::*;

/// `(code, frozen sheet slug)` of the forms whose printed layout PR 9
/// builds.
const FORMS: &[(&str, &str)] = &[("0605", "0605-1999"), ("2000OT", "2000-ot-2018")];

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

/// The printed caption writer-cells.json gives `key` (any join list).
fn printed_caption(slug: &str, key: &str) -> Option<String> {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins", "date_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .find(|join| join["writer_key"] == key)
        .and_then(|join| join["printed"].as_str().map(str::to_string))
}

/// One drawn box: its part, item number, item label, key, box label and
/// type; table cells carry their row's number and their column's caption.
#[derive(Debug, Clone)]
struct Drawn {
    part: String,
    no: String,
    item: String,
    key: String,
    label: String,
    t: String,
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
                            item: label.clone(),
                            key: key.as_str().unwrap().to_string(),
                            label: col["l"].as_str().unwrap_or("").to_string(),
                            t: col["t"].as_str().unwrap().to_string(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// Where `key` is drawn.
fn placed(code: &str, key: &str) -> Drawn {
    drawn(code)
        .into_iter()
        .find(|d| d.key == key)
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
}

/// An item of `code` by its printed number.
fn item(code: &str, no: &str) -> Value {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| part["items"].as_array().unwrap())
        .find(|it| it["no"] == no)
        .cloned()
        .unwrap_or_else(|| panic!("{code} has an item {no}"))
}

/// The labels of an item's drawn boxes, in order.
fn labels_of(code: &str, no: &str) -> Vec<String> {
    item(code, no)["f"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["t"] != "hidden")
        .map(|f| f["l"].as_str().unwrap().to_string())
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

fn printed(code: &str, draft: &Value) -> BTreeMap<String, String> {
    with_database(|db| print_map(db, kind_of(code)?, draft))
        .unwrap()
        .0
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

/// R3 on every PR 9 form: each box the sheet prints under an item number
/// is drawn under that number, and no drawn box sits in an unnumbered
/// "Additional details" group. The oracle is writer-cells.json's printed
/// captions.
#[test]
fn pr9_boxes_sit_under_their_printed_item_numbers() {
    for &(code, slug) in FORMS {
        let cells = writer_cells(slug);
        let boxes = drawn(code);
        let mut checked = 0;
        for join in ["joins", "money_joins", "xbox_joins", "date_joins"]
            .iter()
            .flat_map(|list| cells[*list].as_array().into_iter().flatten())
        {
            let key = join["writer_key"].as_str().unwrap();
            let Some(number) = printed_number(join["printed"].as_str().unwrap()) else {
                continue;
            };
            let Some(at) = boxes.iter().find(|d| d.key == key) else {
                continue;
            };
            // A table row carries its row's number, a caption its item's
            // (6A of item 6's table).
            let item_number = number.trim_end_matches(|c: char| c.is_ascii_uppercase());
            // A sub-item the sheet letters but writer-cells.json does not
            // (2000OT's "11 Name" is printed as 11A) is under its number too.
            let at_item = at.no.trim_end_matches(|c: char| c.is_ascii_uppercase());
            assert!(
                at.no == number || at.no == item_number || at_item == number,
                "{code} {key} prints at {:?}, drawn under {:?} ({})",
                join["printed"],
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
    }
}

// ------------------------------------------------------------------ 0605

const QUARTERS_0605: [&str; 4] = [
    "itemQuarter_1",
    "itemQuarter_2",
    "itemQuarter_3",
    "itemQuarter_4",
];

/// 0605 R3.1, R3.2, R3.3, manual-0, manual-1: headings and options read as
/// printed (html-frozen/0605-1999/index.html and writer-cells.json's
/// xbox captions), none repeating its question or its own number.
#[test]
fn pr9_0605_labels_read_as_printed() {
    let slug = "0605-1999";
    let sheet = sheet_text(slug);
    assert_eq!(item("0605", "1")["l"], "For the");
    assert_eq!(labels_of("0605", "1"), ["Calendar", "Fiscal"]);
    for (key, option) in [
        ("frm0605:itemFiscalStartMonth:_1", "Calendar"),
        ("frm0605:itemFiscalStartMonth:_2", "Fiscal"),
    ] {
        let caption = printed_caption(slug, key).unwrap();
        assert!(
            caption.starts_with("1 ") && caption.ends_with(option),
            "{caption}"
        );
    }
    assert!(sheet.contains("For the Calendar Fiscal"));
    assert_eq!(item("0605", "2")["l"], "Year Ended (MM/YYYY)");
    assert_eq!(item("0605", "3")["l"], "Quarter");
    assert_eq!(labels_of("0605", "3"), ["1st", "2nd", "3rd", "4th"]);
    for (key, option) in QUARTERS_0605.iter().zip(["1st", "2nd", "3rd", "4th"]) {
        assert_eq!(
            printed_caption(slug, key).unwrap(),
            format!("3 Quarter: {option}")
        );
    }
    assert!(
        item("0605", "13")["l"]
            .as_str()
            .unwrap()
            .starts_with("Taxpayer's Name")
    );
    assert_eq!(item("0605", "15")["l"], "Registered Address");
    assert_eq!(
        labels_of("0605", "11"),
        ["I - Individual", "N - Non-Individual"]
    );
    assert!(sheet.contains("Taxpayer Classification: I - Individual N - Non-Individual"));
    let item_21 = item("0605", "21");
    assert_eq!(item_21["l"], "Total Amount Payable (Sum of Items 19 & 20D)");
    // No drawn item heading ends with its own number.
    for part in schema_entry("0605").unwrap()["parts"].as_array().unwrap() {
        for it in part["items"].as_array().unwrap() {
            let (no, label) = (it["no"].as_str().unwrap(), it["l"].as_str().unwrap());
            assert!(
                no.is_empty() || !label.ends_with(&format!(" {no}")),
                "{label}"
            );
        }
    }
}

/// 0605 R2.0, R3.0, manual-2: the email and the approval pair, which the
/// 1999 sheet does not print (writer-cells.json not_joined), are the only
/// boxes drawn under "Not on the printed sheet", each in its own labelled
/// item; the email is a profile box with Edit there.
#[test]
fn pr9_0605_boxes_off_the_sheet_are_named_and_labelled() {
    setup();
    let cells = writer_cells("0605-1999");
    let not_joined: BTreeSet<String> = cells["not_joined"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|entry| {
            entry["writer_keys"]
                .as_array()
                .cloned()
                .unwrap_or_else(|| vec![entry["writer_key"].clone()])
        })
        .filter_map(|k| k.as_str().map(str::to_string))
        .collect();
    let off: Vec<Drawn> = drawn("0605")
        .into_iter()
        .filter(|d| d.part == "Not on the printed sheet")
        .collect();
    let keys: Vec<&str> = off.iter().map(|d| d.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "frm0605:itemApprovedYN:_1",
            "frm0605:itemApprovedYN:_2",
            "txtEmail"
        ]
    );
    for d in &off {
        assert!(not_joined.contains(&d.key), "{} is printed", d.key);
        assert!(!d.item.is_empty() && d.item != "Additional details");
    }
    assert_eq!(off[0].label, "Pre-approved by Investigating Office");
    assert_eq!(off[1].label, "Not approved by Investigating Office");
    let opened = open("0605", 1);
    assert!(strings(&opened["profile_edit"]).contains("txtEmail"));
}

/// 0605 preview.0 and the locked quarter boxes: item 3's four tick boxes
/// are the draft's quarter (form_0605.rs `quarter`, written one tick box
/// per quarter by the field map). Ticking one sets it, the print map ticks
/// the same box, and unticking it clears the quarter; the page and the
/// print agree for every quarter.
#[test]
fn pr9_0605_quarter_boxes_are_the_drafts_quarter() {
    setup();
    let opened = open("0605", 1);
    for key in QUARTERS_0605 {
        assert!(opened["locks"][key].is_null(), "{key} is open");
    }
    let mut draft = opened["draft"].clone();
    for (n, key) in QUARTERS_0605.iter().enumerate() {
        let filled = fill("0605", &draft, pick(&QUARTERS_0605, key));
        assert!(
            filled["rejected"].as_object().unwrap().is_empty(),
            "{}",
            filled["rejected"]
        );
        draft = filled["draft"].clone();
        assert_eq!(draft["quarter"], n as u64 + 1);
        let map = printed("0605", &draft);
        for (m, other) in QUARTERS_0605.iter().enumerate() {
            let on = if m == n { "true" } else { "false" };
            assert_eq!(filled["values"][*other], on, "page {other}");
            assert_eq!(map[*other], on, "print {other}");
        }
    }
    // Unticking the ticked quarter clears it; another box's untick does not.
    let kept = fill("0605", &draft, json!({ "itemQuarter_1": "false" }));
    assert_eq!(kept["draft"]["quarter"], 4);
    let cleared = fill("0605", &draft, json!({ "itemQuarter_4": "false" }));
    assert_eq!(cleared["draft"]["quarter"], 0);
    assert!(
        QUARTERS_0605
            .iter()
            .all(|key| cleared["values"][*key] == "false")
    );
    // The upload carries it: the field map is the print's.
    let typed: bir_core::forms::form_0605::Form0605Draft = serde_json::from_value(
        fill("0605", &draft, pick(&QUARTERS_0605, "itemQuarter_2"))["draft"].clone(),
    )
    .unwrap();
    assert_eq!(typed.quarter, 2);
}

/// 0605 R6.1: item 17's Others (Specify) box is off with what turns it on
/// until Others is the manner (the official `disabletxtOthers()`, which
/// form_0605_official.rs keeps: any other manner clears it); then it keeps
/// what is typed, which prints in its item 17 cell.
#[test]
fn pr9_0605_others_box_waits_for_others() {
    setup();
    let key = "frm0605:txtOthersName";
    let opened = open("0605", 1);
    assert!(opened["locks"][key].is_null());
    assert_eq!(opened["off"][key], "Needs item 17 = Others");
    let others = fill(
        "0605",
        &opened["draft"],
        json!({ "frm0605:itemMannerOfPayment:_5": "true" }),
    );
    let typed = fill("0605", &others["draft"], json!({ key: "Bank transfer" }));
    assert!(typed["rejected"].as_object().unwrap().is_empty());
    assert_eq!(typed["draft"]["other_manner_description"], "Bank transfer");
    assert_eq!(printed("0605", &typed["draft"])[key], "Bank transfer");
    let checked = call_async(
        "screen_form_validate",
        vec!["0605".into(), json(typed["draft"].clone())],
    )
    .unwrap();
    assert!(checked["off"].get(key).is_none());
}

// ---------------------------------------------------------------- 2000OT

const ATCS_2000OT: [&str; 3] = ["frm2000OT:ATC_1", "frm2000OT:ATC_2", "frm2000OT:ATC_3"];

/// 2000OT R3.2, manual-0: item 3's options are the printed codes, item
/// 12's the printed natures, item 6 reads in full, and no heading keeps
/// the sheet's "●" amount bullet. Oracle: writer-cells.json's xbox
/// captions and the sheet's printed text.
#[test]
fn pr9_2000ot_labels_read_as_printed() {
    let slug = "2000-ot-2018";
    let sheet = sheet_text(slug);
    assert_eq!(labels_of("2000OT", "3"), ["DO 102", "DO 122", "DO 125"]);
    for key in ATCS_2000OT {
        let caption = printed_caption(slug, key).unwrap();
        assert!(caption.ends_with(&placed("2000OT", key).label), "{caption}");
    }
    let natures = labels_of("2000OT", "12");
    assert_eq!(natures.len(), 3);
    for (n, nature) in natures.iter().enumerate() {
        let key = format!("frm2000OT:rbTransNature_{}", n + 1);
        assert_eq!(
            printed_caption(slug, &key).unwrap(),
            format!("12 Nature: {nature}")
        );
        assert!(sheet.contains(nature.as_str()), "{nature}");
    }
    assert_eq!(
        item("2000OT", "6")["l"],
        "RDO Code of Location of the Property"
    );
    for d in drawn("2000OT") {
        assert!(
            !d.item.contains('●') && !d.label.contains('●'),
            "{}",
            d.item
        );
    }
}

/// 2000OT R2.0, R3.0, R3.1, manual-1: page 2's schedules are one part
/// "Part IV · Schedules" with Schedule 1.A, its total, Schedule 1.B and
/// Schedule 2's lines; the page 2 header copies of the TIN and name are
/// not drawn (the print still writes them from the draft); Line of
/// Business, which the sheet does not print, is the one box under Not on
/// the printed sheet, a profile box with Edit.
#[test]
fn pr9_2000ot_schedules_are_part_iv_and_the_header_copy_is_not_drawn() {
    setup();
    let parts: Vec<String> = schema_entry("2000OT").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["n"].as_str().unwrap().to_string())
        .collect();
    assert!(
        parts.contains(&"Part IV · Schedules".to_string()),
        "{parts:?}"
    );
    assert!(!parts.contains(&"Page 2".to_string()));
    for key in [
        "frm2000OT:sched1A:txtOCTNo0",
        "frm2000OT:sched1A:txtFMVTotal",
        "frm2000OT:sched1B:txtNameOfCorpStock0",
        "frm2000OT:Sched2:txtGrossSellingPrice",
        "frm2000OT:Sched2:txtOthersAmount",
        "frm2000OT:Sched2:txtTaxableBase",
    ] {
        assert_eq!(placed("2000OT", key).part, "Part IV · Schedules", "{key}");
    }
    let header = [
        "frm2000OT:txtPg2TIN1",
        "frm2000OT:txtPg2TIN2",
        "frm2000OT:txtPg2TIN3",
        "frm2000OT:txtPg2BranchCode",
        "frm2000OT:txtPg2TaxpayerName",
    ];
    for key in header {
        assert!(schema_keys("2000OT").contains(key));
        assert!(!drawn("2000OT").iter().any(|d| d.key == key), "{key}");
    }
    let opened = open("2000OT", 1);
    let map = printed("2000OT", &opened["draft"]);
    assert_eq!(
        map["frm2000OT:txtPg2TaxpayerName"],
        opened["values"]["frm2000OT:txtTaxpayerName"]
    );
    let off: Vec<String> = drawn("2000OT")
        .into_iter()
        .filter(|d| d.part == "Not on the printed sheet")
        .map(|d| d.key)
        .collect();
    assert_eq!(off, ["frm2000OT:txtLineBus"]);
    assert!(strings(&opened["profile_edit"]).contains("frm2000OT:txtLineBus"));
}

/// 2000OT manual-2: item 16 is a text box showing the ATC's rate as the
/// official SelectedATC writes it (hand list from the sheet's ATC rates),
/// never a peso amount; the page and the print agree.
#[test]
fn pr9_2000ot_item_16_shows_the_atcs_rate() {
    setup();
    assert_eq!(placed("2000OT", "frm2000OT:txtTax16").t, "text");
    let opened = open("2000OT", 1);
    for (key, rate) in ATCS_2000OT.iter().zip([
        "P1.50 / P200",
        "P15 / P1,000",
        // The field map writes text upper-cased, as the official page does.
        "50% DST PAID ON ORIGINAL ISSUE",
    ]) {
        let filled = fill("2000OT", &opened["draft"], pick(&ATCS_2000OT, key));
        assert_eq!(filled["values"]["frm2000OT:txtTax16"], rate, "{key}");
        assert_eq!(
            printed("2000OT", &filled["draft"])["frm2000OT:txtTax16"],
            rate
        );
    }
}

/// 2000OT's item 1 is the return's identity, bound to /transaction_date
/// (decision 10): a return opened without a date says what sets it
/// (`waits`, shown in the box), and one started for a date shows it,
/// locked.
#[test]
fn pr9_2000ot_item_1_date_says_what_sets_it() {
    setup();
    let key = "frm2000OT:txtTransactionDate";
    let blank = open("2000OT", 1);
    assert_eq!(blank["values"][key], "");
    assert_eq!(blank["waits"][key], rows_as_needed::DATE_FROM_NEW_RETURN);
    let started = call_async(
        "screen_form_open",
        vec![
            TIN.into(),
            "2000OT".into(),
            YEAR.into(),
            1.into(),
            json(Value::Null),
            1015.into(),
        ],
    )
    .unwrap();
    assert_eq!(started["values"][key], "10/15/2026");
    assert_eq!(started["draft"]["transaction_date"], "10/15/2026");
    assert!(started["locks"][key].is_string());
}
