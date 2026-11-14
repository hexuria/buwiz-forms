//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552) against their printed sheets. The oracles are the frozen
//! sheets' own files (html-frozen/<slug>/writer-cells.json captions and
//! joins, index.html's printed text), the official keys and the typed
//! drafts' own fields, and hand lists from the sheets; never this module's
//! own refusal.

use super::*;

/// `(code, frozen sheet slug)` of the forms whose printed layout PR 9
/// builds.
const FORMS: &[(&str, &str)] = &[
    ("0605", "0605-1999"),
    ("1606", "1606-2018"),
    ("1706", "1706-2018"),
    ("2000OT", "2000-ot-2018"),
    ("2552", "2552-2018"),
    ("1707", "1707-2021"),
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
                // A table numbered from `start` carries its row's number; one
                // whose rows the sheet letters (1707's 6A-6C) its item's.
                let row_no = item["start"]
                    .as_u64()
                    .map_or(no.clone(), |start| (start + r as u64).to_string());
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

/// 0605 preview.0 (fix check 1 undoes review F5): item 3's four tick boxes
/// are the draft's quarter (form_0605.rs `quarter`, written one tick box per
/// quarter by the field map). A typed quarter is stored, printed and filed
/// (the upload's field map), unticking the ticked one clears it, and the
/// return's identity (its entry number, Form0605Draft::filing_period
/// OpenEnded) never moves with it.
#[test]
fn pr9_0605_quarter_boxes_are_the_drafts_quarter() {
    setup();
    let kind = kind_of("0605").unwrap();
    let opened = open("0605", 1);
    let identity = agent::identity(kind, &opened["draft"]).unwrap();
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
        let filed = agent::field_map(kind, &draft).unwrap();
        for (m, other) in QUARTERS_0605.iter().enumerate() {
            let on = if m == n { "true" } else { "false" };
            assert_eq!(filled["values"][*other], on, "page {other}");
            assert_eq!(map[*other], on, "print {other}");
            assert_eq!(filed[*other], on, "upload {other}");
        }
        let now = agent::identity(kind, &draft).unwrap();
        assert_eq!(
            (now.tin, now.taxable_year, now.period),
            (
                identity.tin.clone(),
                identity.taxable_year,
                identity.period.clone()
            )
        );
    }
    let kept = fill("0605", &draft, json!({ "itemQuarter_1": "false" }));
    assert_eq!(kept["draft"]["quarter"], 4);
    let cleared = fill("0605", &draft, json!({ "itemQuarter_4": "false" }));
    assert_eq!(cleared["draft"]["quarter"], 0);
    assert!(
        QUARTERS_0605
            .iter()
            .all(|key| cleared["values"][*key] == "false")
    );
}

/// 0605 R6.1, review F10: item 17's Others box is off with what turns it on
/// until Others is the manner (the official `disabletxtOthers()`, which
/// form_0605_official.rs keeps: any other manner clears it), and refuses
/// what is typed meanwhile with that tag instead of taking it and erasing
/// it; then it keeps what is typed, which prints in its item 17 cell.
#[test]
fn pr9_0605_others_box_waits_for_others() {
    setup();
    let key = "frm0605:txtOthersName";
    let opened = open("0605", 1);
    assert!(opened["locks"][key].is_null());
    assert_eq!(opened["off"][key], "Needs item 17 = Others");
    let shut = fill("0605", &opened["draft"], json!({ key: "Bank transfer" }));
    assert_eq!(shut["rejected"][key], "Needs item 17 = Others");
    // The manner and the text in one batch: the manner first.
    let typed = fill(
        "0605",
        &opened["draft"],
        json!({ "frm0605:itemMannerOfPayment:_5": "true", key: "Bank transfer" }),
    );
    assert!(
        typed["rejected"].as_object().unwrap().is_empty(),
        "{}",
        typed["rejected"]
    );
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
    // A day key the client sends alone writes no date on a trigger form
    // (D8c): the date it names starts the return only when the host offered
    // it for the sample — the page's `new_key_from`.
    let mut parts = Map::new();
    for (part_key, part) in sample_period_parts(kind_of("2000OT").unwrap()) {
        match part {
            PeriodPart::Date => parts.insert(part_key, json!("10/15/2026")),
            PeriodPart::Year => parts.insert(part_key, json!("2026")),
            PeriodPart::Month => parts.insert(part_key, json!("10")),
            PeriodPart::Day => parts.insert(part_key, json!("15")),
            other => panic!("2000OT: unexpected period part {other:?}"),
        };
    }
    let opens = sample_period(TIN, "2000OT", Value::Object(parts))["opens"].clone();
    assert_eq!(opens["new_key"], 1015, "the typed date offers its key");
    let started = open_from("2000OT", &opens);
    assert_eq!(started["values"][key], "10/15/2026");
    assert_eq!(started["draft"]["transaction_date"], "10/15/2026");
    assert!(started["locks"][key].is_string());
}

// ------------------------------------------------------------ 1606, 1706

/// Every key writer-cells.json lists as not printed (`not_joined`).
fn not_joined(slug: &str) -> BTreeSet<String> {
    writer_cells(slug)["not_joined"]
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
        .collect()
}

/// form_page.js `isTinItem`: an item named for a TIN with three to five
/// text boxes is drawn as one TIN comb.
fn is_tin_item(it: &Value) -> bool {
    let fields: Vec<&Value> = it["f"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["t"] != "hidden")
        .collect();
    let label = it["l"].as_str().unwrap();
    (label.contains("TIN") || label.contains("Identification"))
        && (3..=5).contains(&fields.len())
        && fields.iter().all(|f| f["t"] == "text" || f["t"] == "int")
}

/// The items of `code` drawn under printed number `no`, in order.
fn items_numbered(code: &str, no: &str) -> Vec<Value> {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| part["items"].as_array().unwrap())
        .filter(|it| it["no"] == no)
        .cloned()
        .collect()
}

/// 1606 R5.0, manual-2; 1706 R5.2, manual-3: each party's name, address
/// and TIN sit under the sheet's row A of its item, the TIN one comb of
/// its four boxes. The party the filer is not (1606's seller,
/// 1706's buyer) is typed box by box into the one draft TIN, which prints
/// whole in the sheet's TIN cell (writer-cells.json derived joins).
#[test]
fn pr9_1606_1706_party_tins_are_one_comb_and_typed_box_by_box() {
    setup();
    // The sheet's rows A-C are parties; the return's one party is row A:
    // its name, address and TIN (writer-cells.json "7A Buyer's Name",
    // "7A Buyer's Registered Address", "7A Buyer's TIN").
    for (code, rows) in [("1606", ["7A", "8A"]), ("1706", ["6A", "7A"])] {
        for no in rows {
            let items = items_numbered(code, no);
            assert_eq!(items.len(), 3, "{code} row {no}");
            let tins: Vec<bool> = items.iter().map(is_tin_item).collect();
            assert_eq!(tins, [false, false, true], "{code} row {no}");
        }
    }
    // 1606 item 8C, the seller's TIN.
    let opened = open("1606", 1);
    let keys = [
        "frm1606:txtTINS1",
        "frm1606:txtTINS2",
        "frm1606:txtTINS3",
        "frm1606:txtBranchCodeS",
    ];
    for key in keys {
        assert!(opened["locks"][key].is_null(), "1606 {key} is open");
    }
    let filled = fill(
        "1606",
        &opened["draft"],
        json!({ keys[0]: "987", keys[1]: "654", keys[2]: "321", keys[3]: "00000" }),
    );
    assert!(filled["rejected"].as_object().unwrap().is_empty());
    assert_eq!(filled["draft"]["seller_tin"], "98765432100000");
    for (key, part) in keys.iter().zip(["987", "654", "321", "00000"]) {
        assert_eq!(filled["values"][*key], part);
    }
    assert!(
        printed_caption("1606-2018", "derived:seller_tin")
            .unwrap()
            .starts_with("8A")
    );
    assert_eq!(
        printed("1606", &filled["draft"])["derived:seller_tin"],
        "987-654-321-00000"
    );
    // A box out of order is refused; the draft keeps what it had.
    let refused = fill("1606", &opened["draft"], json!({ keys[1]: "654" }));
    assert!(refused["rejected"][keys[1]].is_string());
    // 1706 item 7C, the buyer's TIN; its branch code is its own bound box.
    let opened = open("1706", 1);
    let keys = ["frm1706:txtTINB1", "frm1706:txtTINB2", "frm1706:txtTINB3"];
    for key in keys {
        assert!(opened["locks"][key].is_null(), "1706 {key} is open");
    }
    let filled = fill(
        "1706",
        &opened["draft"],
        json!({ keys[0]: "111", keys[1]: "222", keys[2]: "333", "frm1706:txtBranchCodeB": "00000" }),
    );
    assert!(filled["rejected"].as_object().unwrap().is_empty());
    assert_eq!(filled["draft"]["buyer_tin"], "111222333");
    assert_eq!(
        printed("1706", &filled["draft"])["derived:buyer_tin"],
        "111-222-333-00000"
    );
    // A whole TIN pasted into the first box fills the comb.
    let pasted = fill(
        "1706",
        &opened["draft"],
        json!({ keys[0]: "444-555-666-00001" }),
    );
    assert_eq!(pasted["draft"]["buyer_tin"], "444555666");
    assert_eq!(pasted["draft"]["buyer_branch_code"], "00001");
}

/// 1606 manual-1, 1706 R3.0: options read as printed, without their
/// question (writer-cells.json xbox captions end with the option).
#[test]
fn pr9_1606_1706_options_read_as_printed() {
    assert_eq!(labels_of("1606", "3"), ["Yes", "No"]);
    assert_eq!(labels_of("1606", "6"), ["Private", "Government"]);
    for (code, slug) in [("1606", "1606-2018"), ("1706", "1706-2018")] {
        for d in drawn(code) {
            if d.t != "radio" {
                continue;
            }
            assert!(
                !d.label.contains('?')
                    && !d.label.starts_with("Classification of Property")
                    && !d.label.starts_with("Schedule 2")
                    && !d.label.starts_with("If overremittance")
                    && !d.label.starts_with("Line "),
                "{code} {}: {}",
                d.key,
                d.label
            );
            if let Some(caption) = printed_caption(slug, &d.key) {
                assert!(
                    caption.ends_with(&d.label) || caption.ends_with(&format!("{}?", d.label)),
                    "{code} {}: {caption}",
                    d.key
                );
            }
        }
    }
    assert_eq!(labels_of("1706", "8"), ["Yes", "No"]);
    assert_eq!(labels_of("1706", "9"), ["Yes", "No"]);
    assert_eq!(
        labels_of("1706", "18")[1..],
        ["To be refunded", "To be issued a Tax Credit Certificate"]
    );
}

/// 1606 R3.1, 1706 R3.1: what is drawn under Not on the printed sheet is
/// what writer-cells.json lists as not printed; the FMV-basis tick boxes
/// there say what ticks them (form_1606.rs/form_1706.rs field maps tick
/// them from Schedule 1's amounts).
#[test]
fn pr9_1606_1706_boxes_off_the_sheet_are_not_printed() {
    setup();
    for (code, slug, prefix) in [
        ("1606", "1606-2018", "frm1606:opt27"),
        ("1706", "1706-2018", "frm1706:opt29"),
    ] {
        let off: Vec<Drawn> = drawn(code)
            .into_iter()
            .filter(|d| d.part == "Not on the printed sheet")
            .collect();
        assert!(off.len() >= 6, "{code}");
        let printed_not = not_joined(slug);
        for d in &off {
            assert!(printed_not.contains(&d.key), "{code} {} is printed", d.key);
        }
        let opened = open(code, 1);
        for letter in ["A", "B", "C", "D"] {
            let key = format!("{prefix}{letter}");
            assert!(opened["locks"][&key].is_string());
            assert!(
                opened["waits"][&key]
                    .as_str()
                    .is_some_and(|w| w.starts_with("Ticked by the form once Schedule 1's")),
                "{code} {key}"
            );
        }
        let code_key = format!("frm{code}:txtFMVLand");
        let typed = fill(code, &opened["draft"], json!({ code_key: "1000" }));
        assert_eq!(typed["values"][format!("{prefix}A")], "true", "{code}");
    }
}

/// 1606 R6.1-R6.3, 1706 R6.0-R6.3 and review F1, F10, F12: boxes the
/// official page opens only on another answer (fields.json `enabled_when`)
/// are off with what turns them on and refuse any write but a blank with
/// that tag, judged on the batch's draft as recomputed so far; once on they
/// keep what is typed.
#[test]
fn pr9_1606_1706_boxes_wait_for_their_answer() {
    setup();
    let cases: Vec<(&str, &str, Value, Value, &str)> = vec![
        (
            "1606",
            "frm1606:selTreaty",
            json!({"frm1606:rdTreaty:_1": "true"}),
            json!("0"),
            "0",
        ),
        (
            "1606",
            "frm1606:j_id393_7",
            json!({"frm1606:j_id393_8": "true"}),
            json!("Mixed use"),
            "MIXED USE",
        ),
        (
            "1606",
            "frm1606:txtOthers20",
            json!({"frm1606:j_id395:_5": "true"}),
            json!("Swap"),
            "SWAP",
        ),
        (
            "1706",
            "frm1706:j_id392:_1",
            json!({"frm1706:opt4": "true"}),
            json!("true"),
            "true",
        ),
        (
            "1706",
            "frm1706:j_id393:_2",
            json!({"frm1706:opt4": "true"}),
            json!("true"),
            "true",
        ),
        (
            "1706",
            "frm1706:selTreaty",
            json!({"frm1706:rdTreaty:_1": "true"}),
            json!("1"),
            "1",
        ),
        (
            "1706",
            "frm1706:txtOthers21",
            json!({"frm1706:j_id395:_5": "true"}),
            json!("Swap"),
            "SWAP",
        ),
    ];
    let tags: Value = serde_json::from_str(include_str!(
        "../../../../../app/forms/condition-labels.json"
    ))
    .unwrap();
    let corpus = conditions::tests::derived();
    for (code, key, answer, value, shown) in cases {
        let opened = open(code, 1);
        let condition = corpus[code][key]
            .as_str()
            .unwrap_or_else(|| panic!("{code} {key} has a corpus condition"));
        let tag = &tags["conditions"][code][condition];
        assert_eq!(opened["off"][key], *tag, "{code} {key}");
        let closed = fill(code, &opened["draft"], json!({ key: value }));
        assert_eq!(
            closed["rejected"][key], *tag,
            "{code} {key} refused while off"
        );
        assert_eq!(closed["values"][key], opened["values"][key]);
        // The answer and the box in one batch: the answer runs first.
        let mut batch = answer.clone();
        batch[key] = value.clone();
        let kept = fill(code, &opened["draft"], batch);
        assert!(
            kept["rejected"].as_object().unwrap().is_empty(),
            "{}",
            kept["rejected"]
        );
        assert_eq!(kept["values"][key], shown, "{code} {key} once {answer}");
    }
    // F1 (a): item 11 No and the list in one batch: refused.
    let opened = open("1606", 1);
    let yes = fill(
        "1606",
        &opened["draft"],
        json!({"frm1606:rdTreaty:_1": "true"}),
    );
    let no_and_list = fill(
        "1606",
        &yes["draft"],
        json!({"frm1606:rdTreaty:_2": "true", "frm1606:rdTreaty:_1": "false", "frm1606:selTreaty": "1"}),
    );
    assert_eq!(
        no_and_list["rejected"]["frm1606:selTreaty"],
        "Needs item 11 = Yes"
    );
    assert_eq!(no_and_list["draft"]["tax_relief"], "no");
    // A group key is refused alike.
    let opened = open("1706", 1);
    for (key, value, tag) in [
        (
            "frm1706:j_id392",
            "1",
            "Needs item 3 = an individual seller's ATC",
        ),
        (
            "frm1706:j_id393",
            "1",
            "Needs item 3 = an individual seller's ATC",
        ),
        (
            "frm1706:opt37",
            "1",
            "Needs item 18 below zero (an over-remittance)",
        ),
    ] {
        let group = fill("1706", &opened["draft"], json!({ key: value }));
        assert_eq!(group["rejected"][key], tag, "{key}");
    }
    let opened_1606 = open("1606", 1);
    let group = fill("1606", &opened_1606["draft"], json!({"frm1606:opt37": "1"}));
    assert_eq!(
        group["rejected"]["frm1606:opt37"],
        "Needs item 18 below zero (an over-remittance)"
    );
    // Fix check 11: "false" typed into a text box is text, refused while
    // shut like any other text.
    let others = fill(
        "1606",
        &opened_1606["draft"],
        json!({"frm1606:txtOthers20": "false"}),
    );
    assert_eq!(
        others["rejected"]["frm1606:txtOthers20"],
        "Needs Schedule 2 = Exempt or Others"
    );
    // F10: 1706 item 18's boxes, bound by the overpaid seed, refuse a tick
    // until item 18 is below zero (recompute would erase it).
    let tag = "Needs item 18 below zero (an over-remittance)";
    assert_eq!(opened["off"]["frm1706:opt37:_1"], tag);
    let refused = fill(
        "1706",
        &opened["draft"],
        json!({"frm1706:opt37:_1": "true"}),
    );
    assert_eq!(refused["rejected"]["frm1706:opt37:_1"], tag);
    // Fix check 9: item 18 below zero opens them; a tick is kept.
    let mut overpaid: bir_core::forms::form_1706::Form1706Draft =
        serde_json::from_value(opened["draft"].clone()).unwrap();
    overpaid.set_amended(true);
    overpaid.tax_paid_previous = 5_000_000.0;
    overpaid.recompute();
    let overpaid = serde_json::to_value(&overpaid).unwrap();
    let ticked = fill("1706", &overpaid, json!({"frm1706:opt37:_1": "true"}));
    assert!(
        ticked["rejected"].as_object().unwrap().is_empty(),
        "{}",
        ticked["rejected"]
    );
    assert_eq!(ticked["draft"]["overpayment"], "refund");
    // Fix check 10: the two boxes are one official radio pair (both forms'
    // runtime-control-inventory-v796.json name them frm<form>:opt37), which
    // never unticks: an untick answers nothing on either form.
    let untick = fill(
        "1706",
        &ticked["draft"],
        json!({"frm1706:opt37:_1": "false"}),
    );
    assert_eq!(untick["draft"]["overpayment"], "refund");
    for (code, slug) in [("1606", "1606-v2018"), ("1706", "1706-v2018")] {
        let inventory = std::fs::read_to_string(format!(
            "{}/../../rules/forms/{slug}/fixtures/runtime-control-inventory-v796.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let inventory: Value = serde_json::from_str(&inventory).unwrap();
        let names: Vec<&str> = inventory["controls"]
            .as_array()
            .or(inventory["static_controls"].as_array())
            .unwrap()
            .iter()
            .filter(|c| {
                c["id"]
                    .as_str()
                    .is_some_and(|id| id.ends_with("opt37:_1") || id.ends_with("opt37:_2"))
            })
            .map(|c| c["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [format!("frm{code}:opt37"), format!("frm{code}:opt37")]
        );
    }
    let mut overpaid_1606: bir_core::forms::form_1606::Form1606Draft =
        serde_json::from_value(opened_1606["draft"].clone()).unwrap();
    overpaid_1606.is_amended = true;
    overpaid_1606.tax_remitted_previous = 5_000_000.0;
    overpaid_1606.recompute();
    let overpaid_1606 = serde_json::to_value(&overpaid_1606).unwrap();
    let ticked = fill("1606", &overpaid_1606, json!({"frm1606:opt37:_1": "true"}));
    let untick = fill(
        "1606",
        &ticked["draft"],
        json!({"frm1606:opt37:_1": "false"}),
    );
    assert_eq!(untick["draft"]["overremittance"], "refund");
    // F12: the conditions the gates read, as the corpus words them.
    assert_eq!(
        corpus["1606"]["frm1606:selTreaty"],
        "Item 19 indicates treaty or special-law relief."
    );
    assert_eq!(
        corpus["1706"]["frm1706:selTreaty"],
        "Item 20 indicates tax-treaty relief."
    );
}

/// Review F6 and fix check 6: 1606's and 1706's item 11 No, or the list's
/// blank entry (1606 "", 1706 "0"), clears the relief chosen from its list,
/// so the page asks first (`removes`) and offers Undo; nothing chosen asks
/// nothing.
#[test]
fn pr9_1606_1706_item_11_no_asks_before_it_clears_the_relief() {
    setup();
    for (code, list, choice, blank) in [
        ("1606", "frm1606:selTreaty", "0", ""),
        ("1706", "frm1706:selTreaty", "1", "0"),
    ] {
        let yes_key = format!("frm{code}:rdTreaty:_1");
        let no_key = format!("frm{code}:rdTreaty:_2");
        let opened = open(code, 1);
        let chosen = fill(
            code,
            &opened["draft"],
            json!({ &yes_key: "true", list: choice }),
        );
        assert_eq!(
            chosen["draft"]["tax_relief"], "international_tax_treaty",
            "{code}"
        );
        let no = fill(
            code,
            &chosen["draft"],
            json!({ &no_key: "true", &yes_key: "false" }),
        );
        assert!(
            no["removes"]["message"]
                .as_str()
                .unwrap()
                .contains("International Tax Treaty"),
            "{code}: {}",
            no["removes"]
        );
        assert_eq!(no["draft"]["tax_relief"], "no");
        let emptied = fill(code, &chosen["draft"], json!({ list: blank }));
        assert!(
            emptied["removes"]["message"].is_string(),
            "{code}: {}",
            emptied["removes"]
        );
        assert_eq!(emptied["draft"]["tax_relief"], "no");
        let plain = fill(
            code,
            &opened["draft"],
            json!({ &no_key: "true", &yes_key: "false" }),
        );
        assert!(plain["removes"].is_null(), "{code}");
    }
}

/// Fix check 3: 1606's and 1706's item 11 Yes is "yes, with nothing picked
/// from the list yet" (probe.rs CHOICE_OPTION_SETS -> yes_unspecified), so
/// Yes alone shows the official message and files no relief kind; Yes and a
/// list choice files that choice, and Yes again keeps it. A stored draft
/// holding special law opens and files unchanged.
#[test]
fn pr9_1606_1706_item_11_yes_waits_for_the_list() {
    setup();
    for (code, list, choice, message) in [
        (
            "1606",
            "frm1606:selTreaty",
            "1",
            "Please select a tax relief for Item 11.",
        ),
        (
            "1706",
            "frm1706:selTreaty",
            "2",
            "Please specify tax relief you are availing for Item 11.",
        ),
    ] {
        let kind = kind_of(code).unwrap();
        let yes = format!("frm{code}:rdTreaty:_1");
        let no = format!("frm{code}:rdTreaty:_2");
        let opened = open(code, 1);
        let alone = fill(
            code,
            &opened["draft"],
            json!({ &yes: "true", &no: "false" }),
        );
        assert_eq!(alone["draft"]["tax_relief"], "yes_unspecified", "{code}");
        let checked = call_async(
            "screen_form_validate",
            vec![code.into(), json(alone["draft"].clone())],
        )
        .unwrap();
        let messages: Vec<&str> = checked["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["message"].as_str().unwrap())
            .collect();
        assert!(messages.contains(&message), "{code}: {messages:?}");
        let chosen = fill(code, &alone["draft"], json!({ list: choice }));
        assert_eq!(chosen["draft"]["tax_relief"], "special_law", "{code}");
        assert_eq!(
            agent::field_map(kind, &chosen["draft"]).unwrap()[list],
            choice
        );
        let again = fill(
            code,
            &chosen["draft"],
            json!({ &yes: "true", &no: "false" }),
        );
        assert_eq!(
            again["draft"]["tax_relief"], "special_law",
            "{code} Yes again keeps it"
        );
        // A stored draft from before keeps what it files.
        let saved = save(code, &chosen["draft"]);
        let reopened = open(code, 1);
        assert_eq!(reopened["draft"]["tax_relief"], "special_law", "{code}");
        assert_eq!(
            agent::field_map(kind, &reopened["draft"]).unwrap(),
            agent::field_map(kind, &chosen["draft"]).unwrap(),
            "{code}: {saved}"
        );
    }
}

/// 1706 manual-0: item 3 shows the sheet's printed ATC captions (WI 450 /
/// WC 450); the code the official program uploads beside each box (II420 /
/// IC420, in the official sample payload) is not drawn and still uploads
/// unchanged.
#[test]
fn pr9_1706_item_3_shows_the_printed_atc() {
    setup();
    let sheet = sheet_text("1706-2018");
    assert_eq!(
        labels_of("1706", "3"),
        ["WI 450-Individual", "WC 450-Corporation"]
    );
    assert!(sheet.contains("WI 450-Individual") && sheet.contains("WC 450-Corporation"));
    for key in ["frm1706:txt4", "frm1706:txt4C"] {
        assert!(schema_keys("1706").contains(key));
        assert!(!drawn("1706").iter().any(|d| d.key == key));
    }
    let official = std::fs::read_to_string(format!(
        "{}/../bir-core/tests/sample-payloads/1706-03152025_T123456.official.xml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(official.contains("frm1706:txt4=II420"));
    let opened = open("1706", 1);
    let map = agent::field_map(kind_of("1706").unwrap(), &opened["draft"]).unwrap();
    assert_eq!(map["frm1706:txt4"], "II420");
    assert_eq!(map["frm1706:txt4C"], "IC420");
}

/// Decision D6 (2026-10-18) on 1606 and 1706 (validation.0): a message that
/// names the official program's item number names the printed one on the
/// page. Oracles: each official text is one the form's own validator emits
/// (its source, read here), and each rewritten one names only item numbers
/// the page draws outside the Part IV schedules; a fresh return's messages
/// read them. The checks and the filed data are the form's own.
#[test]
fn pr9_messages_name_the_printed_item_numbers() {
    setup();
    let source = |code: &str| {
        std::fs::read_to_string(format!(
            "{}/../bir-core/src/forms/form_{}.rs",
            env!("CARGO_MANIFEST_DIR"),
            code.to_ascii_lowercase()
        ))
        .unwrap()
        .replace("\\\"", "\"")
    };
    for (code, official, printed) in rows_as_needed::PRINTED_MESSAGES {
        assert!(
            source(code).contains(&format!("\"{official}\"")),
            "{code}: {official:?} is not the validator's"
        );
        let numbers: BTreeSet<String> = schema_entry(code).unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| !p["n"].as_str().unwrap().starts_with("Part IV"))
            .flat_map(|p| p["items"].as_array().unwrap())
            .filter_map(|it| it["no"].as_str().map(str::to_string))
            .collect();
        // Every "Item N" outside a Part IV schedule names a drawn item.
        if printed.contains("Schedule") {
            continue;
        }
        let words: Vec<&str> = printed.split_whitespace().collect();
        for (i, word) in words.iter().enumerate() {
            if matches!(*word, "Item" | "Items") {
                let no = words[i + 1].trim_end_matches(['.', ',']);
                assert!(numbers.contains(no), "{code} {printed:?}: {no}");
            }
        }
    }
    let errors = |code: &str| -> Vec<String> {
        open(code, 1)["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["message"].as_str().unwrap().to_string())
            .collect()
    };
    let shown = errors("1606");
    assert!(
        shown.contains(&"Please select an option for Item 9.".to_string()),
        "{shown:?}"
    );
    assert!(shown.contains(&"Please select an option for Item 6.".to_string()));
    let official = |code: &str, m: &String| {
        rows_as_needed::PRINTED_MESSAGES
            .iter()
            .any(|(form, official, _)| *form == code && official == m)
    };
    assert!(!shown.iter().any(|m| official("1606", m)), "{shown:?}");
    let shown = errors("1706");
    assert!(
        shown.contains(&"Please select an option for Item 3.".to_string()),
        "{shown:?}"
    );
    assert!(!shown.iter().any(|m| official("1706", m)), "{shown:?}");
}

// ------------------------------------------------------------------ 2552

/// On every PR 9 form, a tick box labelled Yes or No writes what it says:
/// its binding's "true" option sets a flag to true for Yes and false for
/// No (2552's item 2 boxes showed the other answer before PR 9). Oracle:
/// the probed bindings (crates/bir-core/data/agent-forms).
#[test]
fn pr9_yes_and_no_boxes_write_what_they_say() {
    let mut checked = 0;
    for &(code, _) in FORMS {
        let def = kind_of(code).unwrap().definition();
        for d in drawn(code) {
            let want = match d.label.as_str() {
                "Yes" => true,
                "No" => false,
                _ => continue,
            };
            let Some(Binding::Choice { options }) = def.bindings.get(&d.key).map(|b| &b.binding)
            else {
                continue;
            };
            let sets: Vec<&Value> = options
                .iter()
                .filter(|o| o.value == "true")
                .flat_map(|o| o.set.values())
                .filter(|v| v.is_boolean())
                .collect();
            if sets.is_empty() {
                continue;
            }
            assert!(
                sets.iter().all(|v| **v == Value::Bool(want)),
                "{code} {} reads {} but writes {sets:?}",
                d.key,
                d.label
            );
            checked += 1;
        }
    }
    assert!(checked >= 6, "{checked}");
    assert_eq!(placed("2552", "frm2552:rdoPg1I2AmendedYes").label, "Yes");
    assert_eq!(placed("2552", "frm2552:rdoPg1I2AmendedNo").label, "No");
}

/// 2552 R3.2, R4.0, R7.0, manual-0, manual-1, manual-4: page 2 is its
/// printed parts. Part IV is one table of its five rows; Part V Schedules
/// 1-3 are tables of their eight printed columns, rows 1-5 numbered from
/// 1, each with its item 6 total and its add-more rows; Part VI a table of
/// its five rows. Number of Shares columns are whole numbers, each table
/// with a date column has its calendar, and Part III shows items 26-29
/// with a note (the official program keeps them disabled).
#[test]
fn pr9_2552_page_2_is_its_printed_parts() {
    let form = schema_entry("2552").unwrap();
    let parts: Vec<&Value> = form["parts"].as_array().unwrap().iter().collect();
    let part = |name: &str| {
        *parts
            .iter()
            .find(|p| p["n"] == name)
            .unwrap_or_else(|| panic!("2552 has {name}"))
    };
    let tables = |name: &str| -> Vec<Value> {
        part(name)["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|it| it["rows"].is_number())
            .cloned()
            .collect()
    };
    let iv = tables("Part IV · Summary of Transactions not subject to Tax");
    assert_eq!(iv.len(), 1);
    assert_eq!(iv[0]["rows"], 5);
    assert_eq!(iv[0]["cols"][0]["l"], "Transaction Classification");
    assert_eq!(iv[0]["cols"][1]["l"], "Amount Involved");
    let v = tables("Part V · Details of Taxable Transactions");
    assert_eq!(v.len(), 6, "three schedules and their add-more rows");
    for (n, table) in v.iter().step_by(2).enumerate() {
        assert_eq!(table["rows"], 5);
        assert_eq!(table["start"], 1);
        assert_eq!(table["cols"].as_array().unwrap().len(), 8);
        assert_eq!(
            table["keys"][0][0],
            format!("frm2552:txtPg2Pt5Sch{}Date1", n + 1)
        );
        assert_eq!(table["cols"][0]["t"], "date");
        assert_eq!(table["cols"][4]["t"], "int", "Number of Shares");
        assert_eq!(table["calendar"], true);
        for (c, letter) in ["a", "b", "c", "d", "e", "f", "g", "h"].iter().enumerate() {
            assert!(
                table["cols"][c]["l"]
                    .as_str()
                    .unwrap()
                    .starts_with(&format!("({letter})"))
            );
        }
    }
    let vi = tables("Part VI · Details of Transactions not Subject to Percentage Tax");
    assert_eq!(vi[0]["rows"], 5);
    assert_eq!(vi[0]["calendar"], true);
    assert_eq!(vi[0]["cols"][4]["t"], "int");
    let sheet = sheet_text("2552-2018");
    for name in [
        "Part IV – Summary of Transactions not subject to Tax",
        "Part V – Details of Taxable Transactions",
        "Part VI – Details of Transactions not Subject to Percentage Tax",
        "Part III – Details of Payment",
    ] {
        assert!(sheet.contains(name), "{name}");
    }
    let iii: Vec<String> = part("Part III · Details of Payment")["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            assert!(it["note"].as_str().is_some_and(|n| n.starts_with("No box")));
            it["no"].as_str().unwrap().to_string()
        })
        .collect();
    assert_eq!(iii, ["26", "27", "28", "29"]);
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json",
            env!("CARGO_MANIFEST_DIR")
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
        .filter(|c| c["id"].as_str().is_some_and(|id| id.contains("Pt3")))
        .collect();
    assert!(!part_iii.is_empty());
    assert!(part_iii.iter().all(|c| c["disabled"] == true));
}

/// 2552 manual-0 and review F8: Part IV rows 2-5, which the corpus marks
/// computed and the official runtime keeps disabled until the row before is
/// used (rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json
/// txtPg2P4I2Amount..I5Amount disabled: true), are off with "Needs Part IV
/// row N-1" and refuse a write until it is used; then they are typed into
/// their row (form_2552.rs exempt_transactions) and print in the row's
/// Amount Involved cell.
#[test]
fn pr9_2552_part_iv_rows_take_their_amounts() {
    setup();
    let inventory = inventory_2552();
    for n in 2..=5 {
        for stem in ["Amount", "TransClass"] {
            let key = format!("frm2552:txtPg2P4I{n}{stem}");
            assert!(
                inventory
                    .iter()
                    .any(|c| c["id"] == key.as_str() && c["disabled"] == true),
                "{key}"
            );
        }
    }
    let opened = open("2552", 1);
    for n in 2..=5 {
        let key = format!("frm2552:txtPg2P4I{n}Amount");
        assert!(opened["locks"][&key].is_null(), "{key}");
        assert_eq!(opened["off"][&key], format!("Needs Part IV row {}", n - 1));
        assert_eq!(
            printed_caption("2552-2018", &key).unwrap(),
            format!("Part IV row {n} Amount Involved")
        );
    }
    let early = fill(
        "2552",
        &opened["draft"],
        json!({"frm2552:txtPg2P4I3Amount": "1500"}),
    );
    assert_eq!(
        early["rejected"]["frm2552:txtPg2P4I3Amount"],
        "Needs Part IV row 2"
    );
    let filled = fill(
        "2552",
        &opened["draft"],
        json!({
            "frm2552:txtPg2P4I1TransClass": "Gift",
            "frm2552:txtPg2P4I2TransClass": "Swap",
            "frm2552:txtPg2P4I3TransClass": "Gift",
            "frm2552:txtPg2P4I3Amount": "1500",
        }),
    );
    assert!(
        filled["rejected"].as_object().unwrap().is_empty(),
        "{}",
        filled["rejected"]
    );
    assert_eq!(filled["draft"]["exempt_transactions"][2]["amount"], 1500.0);
    assert_eq!(
        printed("2552", &filled["draft"])["frm2552:txtPg2P4I3Amount"],
        "1,500.00"
    );
    // Clearing row 3 takes it out of the draft; a blank always goes
    // through, even into a row the gate would shut.
    let cleared = fill(
        "2552",
        &filled["draft"],
        json!({"frm2552:txtPg2P4I3TransClass": "", "frm2552:txtPg2P4I3Amount": ""}),
    );
    assert!(
        cleared["rejected"].as_object().unwrap().is_empty(),
        "{}",
        cleared["rejected"]
    );
    assert_eq!(
        cleared["draft"]["exempt_transactions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let blank_shut = fill(
        "2552",
        &opened["draft"],
        json!({"frm2552:txtPg2P4I5Amount": ""}),
    );
    assert!(blank_shut["rejected"].as_object().unwrap().is_empty());
}

/// The 2552 official runtime controls (rules/forms/2552-v2018/fixtures).
fn inventory_2552() -> Vec<Value> {
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    inventory["controls"]
        .as_array()
        .or(inventory["static_controls"].as_array())
        .unwrap()
        .clone()
}

/// 2552 manual-3 and review F3: 12A and 12B are the filer's whole numbers
/// of shares (form_2552.rs shares_sold, outstanding_shares), which the
/// official runtime keeps disabled until item 11 is an offering (inventory:
/// txtPg1Pt1I12NumOfShare and txtPg1Pt1I12TotShare disabled: true) and
/// recompute clears otherwise: refused with the tag before, typed and
/// printed after item 11 Primary.
#[test]
fn pr9_2552_item_12_takes_whole_numbers_of_shares() {
    setup();
    let inventory = inventory_2552();
    for key in [
        "frm2552:txtPg1Pt1I12TotShare",
        "frm2552:txtPg1Pt1I12NumOfShare",
    ] {
        assert!(
            inventory
                .iter()
                .any(|c| c["id"] == key && c["disabled"] == true),
            "{key}"
        );
        assert_eq!(placed("2552", key).t, "int");
    }
    let opened = open("2552", 1);
    assert_eq!(
        opened["off"]["frm2552:txtPg1Pt1I12TotShare"],
        rows_as_needed::OFFERING_2552
    );
    let early = fill(
        "2552",
        &opened["draft"],
        json!({"frm2552:txtPg1Pt1I12TotShare": "250000"}),
    );
    assert_eq!(
        early["rejected"]["frm2552:txtPg1Pt1I12TotShare"],
        rows_as_needed::OFFERING_2552
    );
    let ipo = fill(
        "2552",
        &opened["draft"],
        pick(&ITEM_11_2552, "frm2552:rdoPg1I11TransactionPrimary"),
    );
    // Item 11 and 12B in one batch: item 11 runs first.
    let mut batch = pick(&ITEM_11_2552, "frm2552:rdoPg1I11TransactionSecondary");
    batch["frm2552:txtPg1Pt1I12TotShare"] = json!("300000");
    let one = fill("2552", &opened["draft"], batch);
    assert!(
        one["rejected"].as_object().unwrap().is_empty(),
        "{}",
        one["rejected"]
    );
    assert_eq!(one["draft"]["outstanding_shares"], "300000");
    let typed = fill(
        "2552",
        &ipo["draft"],
        json!({"frm2552:txtPg1Pt1I12TotShare": "250000", "frm2552:txtPg1Pt1I12NumOfShare": "1000"}),
    );
    assert!(
        typed["rejected"].as_object().unwrap().is_empty(),
        "{}",
        typed["rejected"]
    );
    assert_eq!(typed["draft"]["outstanding_shares"], "250000");
    let map = printed("2552", &typed["draft"]);
    assert_eq!(map["frm2552:txtPg1Pt1I12TotShare"], "250000");
    assert_eq!(map["frm2552:txtPg1Pt1I12NumOfShare"], "1000");
}

/// Review F2: ticking 2552's selected item 11 answer again (the page sends
/// "false" for it in a group of three) keeps the answer, so the draft stays
/// a 2552 draft: another box then fills and the return saves.
#[test]
fn pr9_2552_item_11_untick_keeps_the_answer() {
    setup();
    let opened = open("2552", 1);
    let lse = fill(
        "2552",
        &opened["draft"],
        pick(&ITEM_11_2552, ITEM_11_2552[0]),
    );
    let again = fill("2552", &lse["draft"], json!({ ITEM_11_2552[0]: "false" }));
    assert!(again["rejected"].as_object().unwrap().is_empty());
    assert_eq!(again["draft"]["transaction"], "local_stock_exchange");
    let next = fill(
        "2552",
        &again["draft"],
        json!({"frm2552:txtPg1I3NoOfSheets": "2"}),
    );
    assert!(
        next["rejected"].as_object().unwrap().is_empty(),
        "{}",
        next["rejected"]
    );
    let saved = save("2552", &next["draft"]);
    assert!(saved["id"].is_number(), "{saved}");
}

const ITEM_11_2552: [&str; 3] = [
    "frm2552:rdoPg1I11TransactionLSE",
    "frm2552:rdoPg1I11TransactionPrimary",
    "frm2552:rdoPg1I11TransactionSecondary",
];

/// 2552 item 11 and Part V row 5: changing item 11 clears the rows typed
/// under the old schedule, as the official kindOfTransaction does, so they
/// never move into the new schedule's table, and the page asks first
/// (`removes`) with Undo. Row 5 is typed like rows 1-4 until the add-more
/// rows hold row 5 onward; then it reads OTHERS and says where to edit.
#[test]
fn pr9_2552_item_11_asks_before_it_clears_part_v_and_row_5_fills() {
    setup();
    let opened = open("2552", 1);
    let lse = fill(
        "2552",
        &opened["draft"],
        pick(&ITEM_11_2552, ITEM_11_2552[0]),
    );
    assert!(lse["removes"].is_null());
    let row5 = fill(
        "2552",
        &lse["draft"],
        json!({
            "frm2552:txtPg2Pt5Sch1Seller5": "A Seller",
            "frm2552:txtPg2Pt5Sch1TaxBase5": "10000",
        }),
    );
    assert!(
        row5["rejected"].as_object().unwrap().is_empty(),
        "{}",
        row5["rejected"]
    );
    assert_eq!(row5["draft"]["schedule"][4]["seller"], "A Seller");
    assert_eq!(row5["values"]["frm2552:txtPg2Pt5Sch1Seller5"], "A SELLER");
    assert_eq!(row5["values"]["frm2552:txtPg2Pt5Sch1TaxDue5"], "60.00");
    // Item 11 to Primary: asks first, and the typed rows go.
    let primary = fill("2552", &row5["draft"], pick(&ITEM_11_2552, ITEM_11_2552[1]));
    assert_eq!(primary["removes"]["rows"], 1);
    assert!(
        primary["removes"]["message"]
            .as_str()
            .unwrap()
            .contains("Schedule 1")
    );
    assert_eq!(primary["draft"]["schedule"], json!([]));
    assert_eq!(primary["values"]["frm2552:txtPg2Pt5Sch2Seller5"], "");
    // The same answer again removes nothing.
    let again = fill("2552", &row5["draft"], pick(&ITEM_11_2552, ITEM_11_2552[0]));
    assert!(again["removes"].is_null());
    assert_eq!(again["draft"]["schedule"][4]["seller"], "A Seller");
    // Folded: six rows, row 5 reads OTHERS and refuses with where to edit.
    let mut typed: bir_core::forms::form_2552::Form2552Draft =
        serde_json::from_value(row5["draft"].clone()).unwrap();
    typed.schedule.push(typed.schedule[4].clone());
    let folded = serde_json::to_value(&typed).unwrap();
    let refused = fill(
        "2552",
        &folded,
        json!({"frm2552:txtPg2Pt5Sch1Seller5": "Other"}),
    );
    assert_eq!(
        refused["rejected"]["frm2552:txtPg2Pt5Sch1Seller5"],
        rows_as_needed::ROW5_FOLDED
    );
    assert_eq!(refused["values"]["frm2552:txtPg2Pt5Sch1Seller5"], "OTHERS");
}

/// Review F13: a Schedule 2 row holding only its typed rate is a typed row,
/// and item 11 asks before it clears the old schedule's rows even when the
/// same batch types a row under the new one. Review F15: row 5's date takes
/// what rows 1-4's does (MM/DD/YYYY; a blank clears) and refuses text that
/// is not a date.
#[test]
fn pr9_2552_item_11_counts_every_typed_row_and_row_5_dates_like_rows_1_to_4() {
    setup();
    let opened = open("2552", 1);
    let primary = fill(
        "2552",
        &opened["draft"],
        pick(&ITEM_11_2552, ITEM_11_2552[1]),
    );
    let rate_only = fill(
        "2552",
        &primary["draft"],
        json!({"frm2552:txtPg2Pt5Sch2TaxRate1": "1"}),
    );
    assert_eq!(rate_only["draft"]["schedule"][0]["tax_rate"], 1.0);
    let to_lse = fill(
        "2552",
        &rate_only["draft"],
        pick(&ITEM_11_2552, ITEM_11_2552[0]),
    );
    assert_eq!(to_lse["removes"]["rows"], 1, "{}", to_lse["removes"]);
    // A row typed under Schedule 1, then Primary with a new Schedule 2 row.
    let lse = fill(
        "2552",
        &opened["draft"],
        pick(&ITEM_11_2552, ITEM_11_2552[0]),
    );
    let typed = fill(
        "2552",
        &lse["draft"],
        json!({"frm2552:txtPg2Pt5Sch1Seller1": "A"}),
    );
    let mut batch = pick(&ITEM_11_2552, ITEM_11_2552[1]);
    batch["frm2552:txtPg2Pt5Sch2Seller1"] = json!("B");
    let switched = fill("2552", &typed["draft"], batch);
    assert_eq!(switched["removes"]["rows"], 1, "{}", switched["removes"]);
    assert_eq!(switched["draft"]["schedule"][0]["seller"], "B");
    // Row 5's date, as rows 1-4's.
    let key = "frm2552:txtPg2Pt5Sch1Date5";
    let dated = fill("2552", &lse["draft"], json!({ key: "3/5/2026" }));
    assert!(
        dated["rejected"].as_object().unwrap().is_empty(),
        "{}",
        dated["rejected"]
    );
    assert_eq!(dated["draft"]["schedule"][4]["date"], "03/05/2026");
    let row_1 = fill(
        "2552",
        &lse["draft"],
        json!({"frm2552:txtPg2Pt5Sch1Date1": "3/5/2026"}),
    );
    assert_eq!(row_1["draft"]["schedule"][0]["date"], "03/05/2026");
    let bad = fill("2552", &lse["draft"], json!({ key: "soon" }));
    assert!(
        bad["rejected"][key]
            .as_str()
            .unwrap()
            .contains("MM/DD/YYYY")
    );
    let bad_1 = fill(
        "2552",
        &lse["draft"],
        json!({"frm2552:txtPg2Pt5Sch1Date1": "soon"}),
    );
    assert!(
        bad_1["rejected"]["frm2552:txtPg2Pt5Sch1Date1"]
            .as_str()
            .unwrap()
            .contains("MM/DD/YYYY")
    );
    let cleared = fill("2552", &dated["draft"], json!({ key: "" }));
    assert!(cleared["rejected"].as_object().unwrap().is_empty());
}

// ------------------------------------------------------------------ 1707

/// 1707 R2.2, preview.0, manual-2: item 5, the seller's RDO, is drawn in
/// its printed item, filled from the profile and locked with Edit like the
/// other profile boxes, and what it shows is what prints at "5 RDO Code of
/// the Seller" (writer-cells.json).
#[test]
fn pr9_1707_item_5_rdo_is_drawn_and_prints() {
    setup();
    let key = "frm1707:rdoPg1Pt1I5RDO";
    assert_eq!(
        printed_caption("1707-2021", key).unwrap(),
        "5 RDO Code of the Seller"
    );
    let at = placed("1707", key);
    assert_eq!(
        (at.part.as_str(), at.no.as_str()),
        ("Part I · Background Information", "5")
    );
    let opened = open("1707", 1);
    assert_eq!(opened["values"][key], "039");
    assert!(strings(&opened["profile_edit"]).contains(key));
    assert!(
        opened["options"][key]
            .as_array()
            .is_some_and(|o| o.iter().any(|o| o["value"] == "040"))
    );
    let edited = call(
        "screen_form_fill",
        vec![
            "1707".into(),
            json(opened["draft"].clone()),
            json(json!({ key: "040" })),
            json(json!([key])),
        ],
    )
    .unwrap();
    assert!(edited["rejected"].as_object().unwrap().is_empty());
    assert_eq!(printed("1707", &edited["draft"])[key], "040");
}

/// 1707 manual-0, manual-1, R3.0, R3.1, R3.3: items read as printed. Item
/// 6's sellers and item 7's buyers are tables of Name | Registered Address
/// | TIN on rows 6A-6C and 7A-7C; item 3's boxes are II030 Individual and
/// IC110 Corporation (the uploaded codes beside them not drawn); item 9's
/// options each write the transaction they name (oracle: the probed
/// bindings); the profile copies the sheet does not print are under Not
/// on the printed sheet.
#[test]
fn pr9_1707_items_read_as_printed() {
    let form = schema_entry("1707").unwrap();
    let table = |first: &str| -> Value {
        form["parts"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|p| p["items"].as_array().unwrap())
            .find(|it| it["keys"][0][0] == first)
            .cloned()
            .unwrap_or_else(|| panic!("1707 table {first}"))
    };
    for (first, no, party, rows) in [
        (
            "frm1707:txtPg1I6SellerName0",
            "6",
            "Seller's",
            ["6A", "6B", "6C"],
        ),
        (
            "frm1707:txtPg1I7BuyerName0",
            "7",
            "Buyer's",
            ["7A", "7B", "7C"],
        ),
    ] {
        let t = table(first);
        assert_eq!(t["no"], no);
        let cols: Vec<&str> = t["cols"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["l"].as_str().unwrap())
            .collect();
        assert_eq!(
            cols,
            [
                format!("{party} Name"),
                format!("{party} Registered Address"),
                format!("{party} Taxpayer Identification Number (TIN)")
            ]
        );
        assert_eq!(t["row_labels"]["rows"], json!(rows));
        // Each row's cells are that row's name, address and TIN.
        for (i, row) in t["keys"].as_array().unwrap().iter().enumerate() {
            let caption =
                |key: &Value| printed_caption("1707-2021", key.as_str().unwrap()).unwrap();
            assert!(
                caption(&row[0]).starts_with(rows[i]),
                "{}",
                caption(&row[0])
            );
            assert!(caption(&row[1]).contains("Registered Address"));
            assert!(
                caption(&row[2]).contains("TIN") || caption(&row[2]).contains("Identification")
            );
        }
    }
    assert_eq!(
        labels_of("1707", "3"),
        ["II030 Individual", "IC110 Corporation"]
    );
    let def = kind_of("1707").unwrap().definition();
    for d in drawn("1707")
        .into_iter()
        .filter(|d| d.no == "9" && d.t == "radio")
    {
        let Some(Binding::Choice { options }) = def.bindings.get(&d.key).map(|b| &b.binding) else {
            panic!("{} is bound", d.key);
        };
        let set = options.iter().find(|o| o.value == "true").unwrap().set["/transaction_type"]
            .as_str()
            .unwrap()
            .replace('_', " ");
        let label = d.label.to_ascii_lowercase();
        let first_word = set.split(' ').next().unwrap();
        assert!(
            label.starts_with(first_word),
            "{}: {} writes {set}",
            d.key,
            d.label
        );
    }
    let off: Vec<String> = drawn("1707")
        .into_iter()
        .filter(|d| d.part == "Not on the printed sheet")
        .map(|d| d.key)
        .collect();
    let printed_not = not_joined("1707-2021");
    assert!(off.len() >= 10);
    for key in &off {
        assert!(printed_not.contains(key), "{key} is printed");
    }
}

/// 1707 manual-3: items 13 and Schedule 1 item 6 show the preprinted 15%
/// (writer-cells.json: "item 13 rate is preprinted (15%)"), with no box;
/// R3.6 and manual-4: the tax agent's boxes and Part III items 20-23 show a
/// note (the official program keeps them disabled at load).
#[test]
fn pr9_1707_printed_rates_and_disabled_parts_are_notes() {
    let sheet = sheet_text("1707-2021");
    assert!(sheet.contains("15% 13 Applicable Tax Rate"));
    assert!(sheet.contains("15% 6 Applicable Tax Rate"));
    let form = schema_entry("1707").unwrap();
    let notes: Vec<(String, String, String)> = form["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|p| {
            let name = p["n"].as_str().unwrap().to_string();
            p["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(move |it| {
                    it["note"].as_str().map(|n| {
                        (
                            name.clone(),
                            it["no"].as_str().unwrap().to_string(),
                            n.to_string(),
                        )
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let rates: Vec<&(String, String, String)> = notes
        .iter()
        .filter(|(_, _, n)| n.starts_with("15%"))
        .collect();
    assert_eq!(rates.len(), 2);
    let iii: Vec<&str> = notes
        .iter()
        .filter(|(p, _, _)| p == "Part III · Details of Payment")
        .map(|(_, no, _)| no.as_str())
        .collect();
    assert_eq!(iii, ["20", "21", "22", "23"]);
    assert!(notes.iter().any(|(_, _, n)| n.contains("tax agent")));
    for key in [
        "frm1707:txtPg1P2I13AppTaxRate",
        "frm1707:txtPg2P4Sch1I6ApplcRate",
        "frm1707:txtPg1Pt4TaxAcredNo",
        "frm1707:txtPg1Pt3I28Date",
    ] {
        assert!(!drawn("1707").iter().any(|d| d.key == key), "{key}");
    }
    let inventory: Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/../../rules/forms/1707-v2021/fixtures/runtime-control-inventory-v796.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap(),
    )
    .unwrap();
    let controls = inventory["controls"]
        .as_array()
        .or(inventory["static_controls"].as_array())
        .unwrap();
    let shut: Vec<&Value> = controls
        .iter()
        .filter(|c| {
            c["id"]
                .as_str()
                .is_some_and(|id| id.contains("Pg1Pt3") || id.contains("Pg1Pt4"))
        })
        .collect();
    assert!(shut.len() >= 20);
    assert!(shut.iter().all(|c| c["disabled"] == true));
}

/// 1707 R3.3: item 8's Yes and No are the draft's tax relief (`Some` holds
/// 8A). Yes keeps what 8A holds and prints; No clears it, so the page asks
/// first (`removes`) and offers Undo.
#[test]
fn pr9_1707_item_8_no_asks_before_it_clears_8a() {
    setup();
    let keys = [
        "frm1707:rdoPg1I8TaxReliefYes",
        "frm1707:rdoPg1I8TaxReliefNo",
    ];
    let opened = open("1707", 1);
    for key in keys {
        assert!(opened["locks"][key].is_null(), "{key}");
    }
    let yes = fill("1707", &opened["draft"], pick(&keys, keys[0]));
    let typed = fill(
        "1707",
        &yes["draft"],
        json!({"frm1707:txtPg1I8TaxReliefSpec": "Special law"}),
    );
    assert_eq!(typed["draft"]["tax_relief"], "Special law");
    assert_eq!(typed["values"][keys[0]], "true");
    assert_eq!(printed("1707", &typed["draft"])[keys[0]], "true");
    let no = fill("1707", &typed["draft"], pick(&keys, keys[1]));
    assert!(no["removes"]["message"].as_str().unwrap().contains("8A"));
    assert!(no["draft"]["tax_relief"].is_null());
    assert_eq!(no["values"][keys[1]], "true");
    // Nothing typed in 8A: No removes nothing.
    let plain = fill("1707", &yes["draft"], pick(&keys, keys[1]));
    assert!(plain["removes"].is_null());
}

/// 1707 R3.2, manual-4: Schedules 2 and 3 show rows A-D open, as printed.
/// Row A's No. of Shares (no fields.json box; writer-cells.json "Schedule 2
/// row A No. of Shares") and row D (once the add-more rows hold row D
/// onward it reads OTHERS and says where to edit) are typed and print.
#[test]
fn pr9_1707_schedules_2_and_3_rows_a_to_d_are_open() {
    setup();
    let opened = open("1707", 1);
    let cells = [
        ("frm1707:txtPg2P4Sch2NameOfCorpStock1", "Example Corp"),
        ("frm1707:txtPg2P4Sch2NoOfShares1", "1000"),
        ("frm1707:txtPg2P4Sch2StockCertNo4", "C-4"),
        ("frm1707:txtPg2P4Sch2NameOfCorpStock4", "Fourth Corp"),
        ("frm1707:txtPg2P4Sch2TaxBaseSellPrice4", "5000"),
        ("frm1707:txtPg2P4Sch3Particulars4", "Broker fee"),
        ("frm1707:txtPg2P4Sch3Amount4", "250"),
    ];
    for (key, _) in cells {
        assert!(opened["locks"][key].is_null(), "{key}");
        assert!(placed("1707", key).t != "hidden");
    }
    let edits = Value::Object(
        cells
            .iter()
            .map(|(k, v)| (k.to_string(), json!(v)))
            .collect(),
    );
    let filled = fill("1707", &opened["draft"], edits);
    assert!(
        filled["rejected"].as_object().unwrap().is_empty(),
        "{}",
        filled["rejected"]
    );
    assert_eq!(filled["draft"]["shares"][0]["number_of_shares"], 1000.0);
    assert_eq!(filled["draft"]["shares"][3]["certificate_number"], "C-4");
    assert_eq!(filled["draft"]["expenses"][3]["amount"], 250.0);
    let map = printed("1707", &filled["draft"]);
    assert_eq!(map["frm1707:txtPg2P4Sch2NoOfShares1"], "1,000.000");
    assert_eq!(map["frm1707:txtPg2P4Sch3Amount4"], "250.00");
    let mut typed: bir_core::forms::form_1707::Form1707Draft =
        serde_json::from_value(filled["draft"].clone()).unwrap();
    typed.shares.push(typed.shares[3].clone());
    let folded = serde_json::to_value(&typed).unwrap();
    let refused = fill(
        "1707",
        &folded,
        json!({"frm1707:txtPg2P4Sch2StockCertNo4": "4005"}),
    );
    assert_eq!(
        refused["rejected"]["frm1707:txtPg2P4Sch2StockCertNo4"],
        rows_as_needed::ROW_D_FOLDED
    );
}

/// 1707 R5.2: the profile copy of the address shows the whole address the
/// draft keeps, never the upload's cut first line, so an edit there keeps
/// the rest; the upload's second line is not drawn and the field map is
/// unchanged.
#[test]
fn pr9_1707_profile_address_shows_the_whole_address() {
    setup();
    let opened = open("1707", 1);
    let long = "UNIT 1234 EXAMPLE TOWER, 56 SAMPLE AVENUE CORNER LONG STREET NAME, BARANGAY SAMPLE, QUEZON CITY";
    let mut draft = opened["draft"].clone();
    draft["registered_address"] = json!(long);
    let shown = fill("1707", &draft, json!({}));
    assert_eq!(shown["values"]["frm1707:ProfileAddress1"], long);
    let map = agent::field_map(kind_of("1707").unwrap(), &draft).unwrap();
    assert_eq!(map["frm1707:ProfileAddress1"].chars().count(), 80);
    assert!(!map["frm1707:ProfileAddress2"].is_empty());
    assert!(
        !drawn("1707")
            .iter()
            .any(|d| d.key == "frm1707:ProfileAddress2")
    );
}

/// 1707 R4.0: Schedule 1 item 8 is one date box (shown "Dec 10, 2020",
/// with its calendar) that keeps a typed MM/DD/YYYY date on an installment
/// sale and prints it (writer-cells.json date_joins); item 9 is an amount.
#[test]
fn pr9_1707_schedule_1_date_is_a_date() {
    setup();
    assert_eq!(
        placed("1707", "frm1707:txtPg2P4Sch1I8DateCollection").t,
        "date"
    );
    assert_eq!(
        placed("1707", "frm1707:txtPg2P4Sch1I9TotCollection").t,
        "money"
    );
    let opened = open("1707", 1);
    let installment = fill(
        "1707",
        &opened["draft"],
        json!({"frm1707:rdoPg1I9TransDescInstallment": "true"}),
    );
    let dated = fill(
        "1707",
        &installment["draft"],
        json!({"frm1707:txtPg2P4Sch1I8DateCollection": "03/15/2026"}),
    );
    assert_eq!(
        dated["values"]["frm1707:txtPg2P4Sch1I8DateCollection"],
        "03/15/2026"
    );
    assert_eq!(
        printed("1707", &dated["draft"])["frm1707:txtPg2P4Sch1I8DateCollection"],
        "03/15/2026"
    );
}

// ------------------------------------------------------- one-date boxes

/// The one-date boxes (PR 5's page box): `(form, item, [month, day, year])`.
const ONE_DATE_BOXES: [(&str, &str, [&str; 3]); 3] = [
    (
        "1706",
        "7",
        [
            "frm1706:txtDateMonthI",
            "frm1706:txtDateDayI",
            "frm1706:txtDateYearI",
        ],
    ),
    (
        "1707",
        "1",
        [
            "frm1707:txtPg1I1Month",
            "frm1707:txtPg1I1Day",
            "frm1707:txtPg1I1Year",
        ],
    ),
    (
        "2552",
        "1",
        [
            "frm2552:txtPg1I1Month",
            "frm2552:txtPg1I1Day",
            "frm2552:txtPg1I1Year",
        ],
    ),
];

/// 1706 manual-2, 1707 manual-1, 2552 manual-2: each date the sheet prints
/// as MM/DD/YYYY over three stored boxes is one date box on the page
/// (`one_date`, form_page.js `oneDate`: shown "Mar 15, 2026", typed or
/// picked, sent as the three parts); the draft keeps them split. A return
/// not yet dated shows no date, never "00/00/2026". Lesson 13: a blank
/// clears the parts, anything else must be a MM/DD/YYYY date's parts with a
/// 4-digit year; 2552's item 1 is the return's identity and says what sets
/// it.
#[test]
fn pr9_dates_over_three_boxes_are_one_date_box() {
    setup();
    for (code, no, keys) in ONE_DATE_BOXES {
        let items = items_numbered(code, no);
        let item = items
            .iter()
            .find(|it| it["one_date"] == true)
            .unwrap_or_else(|| panic!("{code} item {no} is one date box"));
        let mut fields: Vec<&str> = item["f"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["k"].as_str().unwrap())
            .collect();
        fields.sort_unstable();
        let mut want = keys.to_vec();
        want.sort_unstable();
        assert_eq!(fields, want, "{code}");
        // form_page.js `dateParts` finds the parts by their labels.
        for (key, word) in keys.iter().zip(["month", "day", "year"]) {
            let label = placed(code, key).label.to_ascii_lowercase();
            assert!(label.contains(word), "{code} {key}: {label}");
        }
    }
    // A return not yet dated: no "00" anywhere in item 1.
    for code in ["1707", "2552"] {
        let opened = open(code, 1);
        let keys = ONE_DATE_BOXES
            .iter()
            .find(|(c, _, _)| *c == code)
            .unwrap()
            .2;
        for key in keys {
            assert_eq!(opened["values"][key], "", "{code} {key}");
        }
        assert_eq!(
            agent::field_map(kind_of(code).unwrap(), &opened["draft"]).unwrap()[keys[1]],
            "00",
            "{code}: the upload keeps the official blank day"
        );
    }
    let opened = open("2552", 1);
    for key in ONE_DATE_BOXES[2].2 {
        assert_eq!(opened["waits"][key], rows_as_needed::DATE_FROM_NEW_RETURN);
        assert!(opened["locks"][key].is_string());
    }
    // Review item 7: the host writes 1706's and 1707's date parts itself
    // (rows_as_needed.rs fill), so no "cannot be filled here" lock applies;
    // 2552's item 1 keeps its identity lock.
    for (code, _, keys) in ONE_DATE_BOXES {
        let kind = kind_of(code).unwrap();
        for key in keys {
            assert_eq!(
                catalogs::host_writes(kind, key),
                code != "2552",
                "{code} {key}"
            );
        }
    }
    // Lesson 13 on 1706's and 1707's boxes, as the page sends them.
    for (code, _, keys) in &ONE_DATE_BOXES[..2] {
        let opened = open(code, 1);
        let date = fill(
            code,
            &opened["draft"],
            json!({ keys[0]: "03", keys[1]: "15", keys[2]: "2026" }),
        );
        assert!(
            date["rejected"].as_object().unwrap().is_empty(),
            "{code}: {}",
            date["rejected"]
        );
        let map = printed(code, &date["draft"]);
        assert_eq!(
            (
                map[keys[0]].as_str(),
                map[keys[1]].as_str(),
                map[keys[2]].as_str()
            ),
            ("03", "15", "2026"),
            "{code}"
        );
        // Each part alone, then the whole date the way the page's
        // parseTypedDate reads it: a real calendar day of 1900-2100, with
        // the page's own message (form_page.js DATE_ERROR).
        let page = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../app/shell/form_page.js"
        ))
        .unwrap();
        assert!(page.contains(&format!(
            "const DATE_ERROR = \"{}\";",
            rows_as_needed::DATE_ERROR
        )));
        for bad in [
            json!({ keys[0]: "13" }),
            json!({ keys[1]: "32" }),
            json!({ keys[2]: "26" }),
            json!({ keys[0]: "March" }),
            json!({ keys[0]: "02", keys[1]: "31", keys[2]: "2026" }),
            json!({ keys[0]: "02", keys[1]: "29", keys[2]: "2026" }),
            json!({ keys[0]: "04", keys[1]: "31" }),
            json!({ keys[2]: "0000" }),
            json!({ keys[2]: "1899" }),
            json!({ keys[2]: "2101" }),
        ] {
            let refused = fill(code, &date["draft"], bad.clone());
            for key in bad.as_object().unwrap().keys() {
                assert_eq!(
                    refused["rejected"][key],
                    rows_as_needed::DATE_ERROR,
                    "{code} {bad}"
                );
            }
            assert_eq!(
                printed(code, &refused["draft"]),
                printed(code, &date["draft"]),
                "{code} {bad}: no part lands alone"
            );
        }
        for good in [
            json!({ keys[0]: "02", keys[1]: "29", keys[2]: "2024" }),
            json!({ keys[0]: "12", keys[1]: "31", keys[2]: "2100" }),
            json!({ keys[0]: "01", keys[1]: "01", keys[2]: "1900" }),
        ] {
            let taken = fill(code, &date["draft"], good.clone());
            assert!(
                taken["rejected"].as_object().unwrap().is_empty(),
                "{code} {good}: {}",
                taken["rejected"]
            );
        }
        let cleared = fill(
            code,
            &date["draft"],
            json!({ keys[0]: "", keys[1]: "", keys[2]: "" }),
        );
        assert!(
            cleared["rejected"].as_object().unwrap().is_empty(),
            "{code}: {}",
            cleared["rejected"]
        );
        assert!(
            keys.iter().all(|k| cleared["values"][*k] == ""),
            "{code}: {}",
            cleared["values"]
        );
    }
}

/// Lesson 13 and 6 on the one-date boxes: a stored draft from pr0b-push
/// (literal fixtures, each holding a legacy partial or short date: 1706's
/// installment date "3"/"5"/"26", 1707's and 2552's month 3 with day 0)
/// opens and saves unchanged, byte for byte, until its filer edits it.
#[test]
fn pr9_stored_one_dates_open_and_save_unchanged() {
    for (code, fixture) in [
        ("1706", include_str!("fixtures/pr9_legacy_1706.json")),
        ("1707", include_str!("fixtures/pr9_legacy_1707.json")),
        ("2552", include_str!("fixtures/pr9_legacy_2552.json")),
    ] {
        setup();
        let stored: Value = serde_json::from_str(fixture).unwrap();
        // A row of the form for the fixture's period, then its bytes in it.
        let mut seed = stored.clone();
        seed["id"] = Value::Null;
        let first = save(code, &seed);
        assert_eq!(first["id"], 1, "{code}");
        with_database(|db| {
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET data_json = ?1 WHERE id = 1",
                    [fixture],
                )
                .map(|_| ())
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap();
        let opened = open(code, 1);
        let raw_now = || -> String {
            with_database(|db| {
                db.conn_for_tests()
                    .query_row("SELECT data_json FROM form_drafts WHERE id = 1", [], |r| {
                        r.get(0)
                    })
                    .map_err(|e| HostError::new(e.to_string()))
            })
            .unwrap()
        };
        assert_eq!(raw_now(), fixture, "{code}: opening writes nothing");
        // 1707's and 2552's stored month 3 with day 0 shows blank, with
        // what is stored said where the date is (display only).
        if code != "1706" {
            let keys = ONE_DATE_BOXES
                .iter()
                .find(|(form, _, _)| *form == code)
                .unwrap()
                .2;
            let why = "The stored date has no day (03/__/2026); type the full date.";
            for key in keys {
                assert_eq!(opened["values"][key], "", "{code} {key}");
            }
            let notice = opened["errors"]
                .as_array()
                .unwrap()
                .iter()
                .find(|e| e["message"] == why)
                .unwrap_or_else(|| panic!("{code}: {}", opened["errors"]));
            assert_eq!(notice["keys"], json!(keys), "{code}");
            assert_eq!(placed(code, keys[0]).no, "1", "{code}");
            if code == "2552" {
                for key in keys {
                    assert_eq!(opened["waits"][key], why, "{code} {key}");
                }
            }
        }
        // The host hands numbers to the page as f64 (`1` and `1.0` alike).
        fn same(a: &Value, b: &Value) -> bool {
            match (a, b) {
                (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
                (Value::Object(x), Value::Object(y)) => {
                    x.len() == y.len()
                        && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
                }
                (Value::Array(x), Value::Array(y)) => {
                    x.len() == y.len() && x.iter().zip(y).all(|(v, w)| same(v, w))
                }
                _ => a == b,
            }
        }
        // The profile baseline (PR 0c) is recorded on open and on save, the
        // row's last key; set aside, the draft is the stored one.
        let mut shown = opened["draft"].clone();
        if let Some(map) = shown.as_object_mut() {
            map.remove("profile_baseline");
        }
        assert!(same(&shown, &stored), "{code}: opens unchanged");
        let filed = |draft: &Value| agent::field_map(kind_of(code).unwrap(), draft).unwrap();
        assert_eq!(
            filed(&opened["draft"]),
            filed(&stored),
            "{code}: files unchanged"
        );
        save(code, &opened["draft"]);
        let saved = raw_now();
        let saved = match saved.find(",\"profile_baseline\":") {
            Some(at) => {
                let baseline: Value =
                    serde_json::from_str(&saved[at + 20..saved.len() - 1]).unwrap();
                assert!(baseline.is_object(), "{code}: the baseline is the last key");
                format!("{}}}", &saved[..at])
            }
            None => saved,
        };
        assert_eq!(saved, fixture, "{code}: saved byte for byte");
    }
}

/// 1707 manual-1: item 3 reads its printed ATC options, and the "ATC Code
/// on Page 1 Item 3 is required." message names item 3's boxes, so the page
/// draws it under item 3, which no Additional details group holds.
#[test]
fn pr9_1707_item_3_message_sits_under_item_3() {
    setup();
    assert_eq!(
        labels_of("1707", "3"),
        ["II030 Individual", "IC110 Corporation"]
    );
    let opened = open("1707", 1);
    let error = opened["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["message"] == "ATC Code on Page 1 Item 3 is required.")
        .expect("item 3's message on a fresh return");
    let keys: Vec<&str> = error["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap())
        .collect();
    // The page draws a message under its first key's item.
    let at = placed("1707", keys[0]);
    assert_eq!(
        (at.no.as_str(), at.part.as_str()),
        ("3", "Page 1"),
        "{keys:?}"
    );
    assert!(
        keys.iter()
            .all(|key| placed("1707", key).item != "Additional details")
    );
}

/// 2552 manual-2: item 5 is one RDO box (the runtime selector stays a
/// hidden copy); items 11 and 12 carry their printed headings and 11A's
/// caption is the printed one (html-frozen/2552-2018/index.html).
#[test]
fn pr9_2552_items_5_11_and_12_read_as_printed() {
    let sheet = sheet_text("2552-2018");
    let five: Vec<String> = items_numbered("2552", "5")
        .iter()
        .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
        .filter(|f| f["t"] != "hidden")
        .map(|f| f["k"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(five, ["frm2552:txtPg1I5RDO"]);
    assert!(
        !drawn("2552")
            .iter()
            .any(|d| d.key == "frm2552:rdoPg1Pt1I5RDO")
    );
    assert!(schema_keys("2552").contains("frm2552:rdoPg1Pt1I5RDO"));
    assert_eq!(item("2552", "11")["l"], "Kinds of Transaction");
    assert!(sheet.contains("11 Kinds of Transaction"));
    let lse = placed("2552", "frm2552:rdoPg1I11TransactionLSE");
    assert_eq!(
        lse.label,
        "11A Shares of Stocks Listed and Traded through the LSE"
    );
    assert!(sheet.contains("11A Shares of Stocks Listed and Traded through the LSE"));
    // Item 12 is its printed heading, with 12A and 12B under it.
    assert!(sheet.contains("12 For Initial Public Offering"));
    assert_eq!(item("2552", "12")["l"], "For Initial Public Offering");
    let twelve: Vec<(String, String)> = items_numbered("2552", "12")
        .iter()
        .flat_map(|it| it["f"].as_array().cloned().unwrap_or_default())
        .map(|f| {
            (
                f["k"].as_str().unwrap().to_string(),
                f["l"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        twelve,
        [
            (
                "frm2552:txtPg1Pt1I12NumOfShare".to_string(),
                "12A No. of Shares Sold, Bartered, or Exchanged".to_string()
            ),
            (
                "frm2552:txtPg1Pt1I12TotShare".to_string(),
                "12B Total Outstanding Shares of Stocks after Listing in the LSE".to_string()
            ),
        ]
    );
    for (key, label) in &twelve {
        assert!(sheet.contains(label.as_str()), "{label}");
        assert_eq!(placed("2552", key).no, "12", "{key}");
    }
    assert!(items_numbered("2552", "12A").is_empty());
    assert!(items_numbered("2552", "12B").is_empty());
}

/// Review item 4: the error-key reorder moves only a message whose subject
/// key is drawn under Not on the printed sheet while another of its keys is
/// drawn under a printed item; on the six forms' fresh returns and samples
/// that is 1707's item 3 message alone. Oracle: each message's keys in the
/// bindings' own order (`message_keys`); the golden pins every message.
#[test]
fn pr9_error_key_order_moves_only_the_1707_item_3_message() {
    setup();
    let mut lines = Vec::new();
    let mut moved = BTreeSet::new();
    for (code, _) in FORMS {
        let kind = kind_of(code).unwrap();
        for (source, opened) in [("open", open(code, 1)), ("sample", sample(code))] {
            for error in opened["errors"].as_array().unwrap() {
                let keys: Vec<String> = error["keys"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|k| k.as_str().unwrap().to_string())
                    .collect();
                let field = error["field"].as_str().unwrap();
                if field != "one_date" {
                    let bound = message_keys(kind, field, schema_keys(code));
                    let mut sorted = bound.clone();
                    sorted.sort();
                    let mut ours = keys.clone();
                    ours.sort();
                    assert_eq!(ours, sorted, "{code} {field}: the same keys");
                    if keys != bound {
                        moved.insert((code.to_string(), error["message"].to_string()));
                    }
                }
                lines.push(format!(
                    "{code}\t{source}\t{field}\t{}\t{}",
                    error["message"].as_str().unwrap(),
                    keys.join(" ")
                ));
            }
        }
    }
    assert_eq!(
        moved,
        BTreeSet::from([(
            "1707".to_string(),
            "\"ATC Code on Page 1 Item 3 is required.\"".to_string()
        )])
    );
    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/screen_form_module/tests/fixtures/pr9_error_keys.golden.tsv"
    );
    let got = lines.join("\n") + "\n";
    if std::env::var_os("UPDATE_PR9_GOLDEN").is_some() {
        std::fs::write(golden_path, &got).unwrap();
    }
    assert_eq!(got, std::fs::read_to_string(golden_path).unwrap());
}

/// Review item 8: the reorder reads Not on the printed sheet's tables
/// (`keys` rows) as well as its fields; a subject key there yields to the
/// first key drawn under a printed item, and a drawn subject never moves.
#[test]
fn pr9_error_key_order_reads_off_sheet_tables() {
    let mut tables = 0;
    for (code, _) in FORMS {
        let kind = kind_of(code).unwrap();
        let schema = schema_entry(code).unwrap();
        let mut off = Vec::new();
        let mut drawn = Vec::new();
        for part in schema["parts"].as_array().unwrap() {
            let here = part["n"] == "Not on the printed sheet";
            for item in part["items"].as_array().unwrap() {
                if let Some(rows) = item["keys"].as_array() {
                    let cells: Vec<String> = rows
                        .iter()
                        .flat_map(|row| row.as_array().cloned().unwrap_or_default())
                        .filter_map(|k| k.as_str().map(str::to_string))
                        .collect();
                    if here {
                        tables += 1;
                        off.extend(cells);
                    } else {
                        drawn.extend(cells);
                    }
                }
                for field in item["f"].as_array().into_iter().flatten() {
                    if field["t"] == "hidden" {
                        continue;
                    }
                    let key = field["k"].as_str().unwrap().to_string();
                    if here { off.push(key) } else { drawn.push(key) }
                }
            }
        }
        let Some(d) = drawn.first() else {
            continue;
        };
        for k in &off {
            let mut keys = vec![k.clone(), "x".to_string(), d.clone()];
            rows_as_needed::order_error_keys(kind, &mut keys);
            assert_eq!(keys, [d.clone(), k.clone(), "x".to_string()], "{code}");
            let mut keys = vec![d.clone(), k.clone()];
            rows_as_needed::order_error_keys(kind, &mut keys);
            assert_eq!(keys, [d.clone(), k.clone()], "{code}");
        }
    }
    // None of the six forms has a table there today; a synthetic schema
    // shows one is read.
    assert_eq!(tables, 0);
    let form = json!({"parts": [
        {"n": "Part I", "items": [{"f": [{"k": "a", "t": "text"}]}]},
        {"n": "Not on the printed sheet", "items": [
            {"rows": 1, "keys": [["t1", "t2"]]},
            {"f": [{"k": "h", "t": "hidden"}]},
        ]},
    ]});
    let mut keys = ["t2", "h", "a"].map(String::from);
    rows_as_needed::order_keys_in(&form, &mut keys);
    assert_eq!(keys, ["a", "t2", "h"]);
    // A hidden copy is drawn nowhere, so it never leads the reorder.
    let mut keys = ["h", "a"].map(String::from);
    rows_as_needed::order_keys_in(&form, &mut keys);
    assert_eq!(keys, ["h", "a"]);
}
