//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552) against their printed sheets. The oracles are the frozen
//! sheets' own files (html-frozen/<slug>/writer-cells.json captions and
//! joins, index.html's printed text), the official keys and the typed
//! drafts' own fields, and hand lists from the sheets; never this module's
//! own refusal.

use super::*;

/// `(code, frozen sheet slug)` of the forms whose printed layout PR 9
/// builds.
const FORMS: &[(&str, &str)] = &[("0605", "0605-1999")];

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
            assert!(
                at.no == number || at.no == item_number,
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
