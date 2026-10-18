//! PR 10: the donor's and estate tax returns (1800, 1801) against their
//! printed sheets. The oracles are the frozen sheets' own files
//! (html-frozen/<slug>/writer-cells.json captions and joins, index.html's
//! printed text), the tracked rules corpus (rules/forms fields.json and
//! fixtures), the official keys and the typed drafts' own fields, and hand
//! lists from the sheets; never this module's own refusal.

use super::*;
use crate::screen_form_module::rows_donor_estate;

/// `(code, frozen sheet slug)`.
const FORMS: &[(&str, &str)] = &[("1800", "1800-2018"), ("1801", "1801-2018")];

fn writer_cells(slug: &str) -> Value {
    let path = format!(
        "{}/../../html-frozen/{slug}/writer-cells.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).expect("writer-cells"))
        .expect("writer-cells json")
}

fn joins(slug: &str) -> Vec<Value> {
    let cells = writer_cells(slug);
    ["joins", "money_joins", "xbox_joins", "date_joins"]
        .iter()
        .flat_map(|list| cells[*list].as_array().cloned().unwrap_or_default())
        .collect()
}

/// The printed caption writer-cells.json gives `key` (any join list).
fn printed_caption(slug: &str, key: &str) -> String {
    joins(slug)
        .into_iter()
        .find(|join| join["writer_key"] == key)
        .and_then(|join| join["printed"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("{slug}: {key} is joined"))
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
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn rules_file(package: &str, file: &str) -> Value {
    let path = format!(
        "{}/../../rules/forms/{package}/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(&path).expect("rules file")).expect("json")
}

/// The tracked runtime inventory's control `id`.
fn control(package: &str, id: &str) -> Value {
    rules_file(package, "fixtures/runtime-control-inventory-v796.json")["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("{package}: control {id}"))
}

/// One drawn box: its part, item number, item label, key, box label and
/// type; table cells carry their table's number.
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
            for row in item["keys"].as_array().into_iter().flatten() {
                for (c, key) in row.as_array().unwrap().iter().enumerate() {
                    let col = &item["cols"][c];
                    if col["t"] != "hidden" {
                        out.push(Drawn {
                            part: name.clone(),
                            no: no.clone(),
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

fn placed(code: &str, key: &str) -> Drawn {
    drawn(code)
        .into_iter()
        .find(|d| d.key == key)
        .unwrap_or_else(|| panic!("{code}: {key} is drawn"))
}

fn is_drawn(code: &str, key: &str) -> bool {
    drawn(code).iter().any(|d| d.key == key)
}

/// Every item of `code` with its part's name.
fn items(code: &str) -> Vec<(String, Value)> {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| {
            let name = part["n"].as_str().unwrap().to_string();
            part["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |it| (name.clone(), it.clone()))
        })
        .collect()
}

/// The item whose first box is `key`.
fn item_of(code: &str, key: &str) -> Value {
    items(code)
        .into_iter()
        .map(|(_, it)| it)
        .find(|it| it["f"][0]["k"] == key)
        .unwrap_or_else(|| panic!("{code}: an item starts with {key}"))
}

/// The printed item number a caption starts with (`7A`, `15D`).
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

fn uploaded(code: &str, draft: &Value) -> BTreeMap<String, String> {
    agent::field_map(kind_of(code).unwrap(), draft).unwrap()
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

fn rejected(filled: &Value, key: &str) -> Option<String> {
    filled["rejected"][key].as_str().map(str::to_string)
}

/// R3 on both forms: each box the sheet prints under an item number is
/// drawn under that number, and no drawn box sits in an unnumbered
/// "Additional details" group. The oracle is writer-cells.json's printed
/// captions.
#[test]
fn pr10_boxes_sit_under_their_printed_item_numbers() {
    for &(code, slug) in FORMS {
        let boxes = drawn(code);
        let mut checked = 0;
        for join in joins(slug) {
            let key = join["writer_key"].as_str().unwrap();
            let Some(number) = printed_number(join["printed"].as_str().unwrap()) else {
                continue;
            };
            let Some(at) = boxes.iter().find(|d| d.key == key) else {
                continue;
            };
            assert_eq!(
                at.no, number,
                "{code} {key} prints at {:?}, drawn under {:?} ({})",
                join["printed"], at.no, at.part
            );
            checked += 1;
        }
        assert!(checked >= 30, "{code}: {checked} boxes checked");
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
    // The boxes writer-cells.json joins without a number, under the item the
    // sheet prints them in (hand list from html-frozen/<slug>/index.html).
    for (code, key, no) in [
        ("1800", "frm1800v2018:txtDoneeNameA", "12"),
        ("1800", "frm1800v2018:txtDoneeTINE", "12"),
        ("1800", "frm1800v2018:txtDeductionTitle28", "28"),
        ("1800", "frm1800v2018:txtDeductionAmount32", "32"),
        ("1800", "frm1800v2018:txtZipCode", "8A"),
        ("1800", "frm1800v2018:txtATC", "4"),
        ("1801", "frm1801v2018:txtAtc", "4"),
        ("1801", "frm1801v2018:txtFam_A", "30"),
        ("1801", "frm1801v2018:txtRealProp_B", "29"),
        ("1801", "frm1801v2018:txtTaxable_C", "32"),
        ("1801", "frm1801v2018:txtBusiness_A", "33"),
        ("1801", "frm1801v2018:txtGrossEst_C", "34"),
        ("1801", "frm1801v2018:txtEstAfterDed_A", "36"),
        ("1801", "frm1801v2018:txtStandardDed_C", "37A"),
        ("1801", "frm1801v2018:txtOtherDed_Specify", "37C"),
        ("1801", "frm1801v2018:txtNetEst_C", "38"),
        ("1801", "frm1801v2018:txtShareSpouse_C", "39"),
        ("1801", "frm1801v2018:freq_Mon", "15D"),
        ("1801", "frm1801v2018:freqOtherInput", "15D"),
        ("1801", "frm1801v2018:txtInstallmentYear", "21"),
        ("1801", "frm1801v2018:txtTINE1", "11"),
        ("1801", "frm1801v2018:telephoneNumber", "12"),
    ] {
        assert_eq!(placed(code, key).no, no, "{code} {key}");
    }
}

/// 1800 R3.4 and manual-3; 1801 R3.3, R3.4, manual-1 and manual-4: the
/// headings and options read as printed. The oracles are writer-cells.json
/// xbox captions (each ends with its option) and the sheet's text.
#[test]
fn pr10_labels_read_as_printed() {
    for &(code, slug) in FORMS {
        let cells = writer_cells(slug);
        for join in cells["xbox_joins"].as_array().unwrap() {
            let key = join["writer_key"].as_str().unwrap();
            let caption = join["printed"].as_str().unwrap();
            let option = ["Yes", "No"]
                .into_iter()
                .find(|o| {
                    caption.ends_with(o)
                        || caption.contains(&format!("{o} (left box"))
                        || caption.contains(&format!("{o} (right box"))
                })
                .unwrap_or_else(|| panic!("{code} {key}: {caption}"));
            assert_eq!(placed(code, key).label, option, "{code} {key}");
        }
    }
    let sheet = sheet_text("1800-2018");
    assert!(sheet.contains("Are you availing of tax relief under a"));
    assert!(sheet.contains("Special Law/International Tax Treaty?"));
    assert_eq!(
        placed("1800", "frm1800v2018:TaxTreatyYN_Y").item,
        "Are you availing of tax relief under a Special Law/International Tax Treaty?"
    );
    assert_eq!(placed("1800", "frm1800v2018:txtZipCode").label, "ZIP Code");
    let sheet = sheet_text("1801-2018");
    for printed in [
        "TIN of Executor/",
        "Administrator",
        "Are you availing of tax relief under a",
        "Monthly",
        "Quarterly",
        "Semi-Annual",
        "Others (specify)",
    ] {
        assert!(sheet.contains(printed), "{printed}");
    }
    assert_eq!(
        placed("1801", "frm1801v2018:txtTINE1").item,
        "TIN of Executor/Administrator"
    );
    assert_eq!(
        placed("1801", "frm1801v2018:optTreaty_1").item,
        "Are you availing of tax relief under a Special Law/International Tax Treaty?"
    );
    // Item 22's "1st installment" (the sheet's superscript "st" sits apart
    // from its "1": "Tax Payable st (1 installment)").
    assert!(sheet.contains("22 Tax Payable st (1 installment) (Item 20 Less Item 21)"));
    assert_eq!(
        placed("1801", "frm1801v2018:txtTaxPayable_1st").item,
        "Tax Payable (1st installment) (Item 20 Less Item 21)"
    );
    let freq: Vec<String> = ["freq_Mon", "freq_Qtr", "freq_Semi", "freq_Other"]
        .iter()
        .map(|k| placed("1801", &format!("frm1801v2018:{k}")).label)
        .collect();
    assert_eq!(
        freq,
        ["Monthly", "Quarterly", "Semi-Annual", "Others (specify)"]
    );
    // Item 7 is one name line after the printed ESTATE OF.
    let seven: Vec<Drawn> = drawn("1801").into_iter().filter(|d| d.no == "7").collect();
    assert_eq!(seven.len(), 1, "{seven:?}");
    assert_eq!(seven[0].key, "frm1801v2018:registeredName_1");
    assert_eq!(seven[0].label, "ESTATE OF");
    assert!(sheet.contains("ESTATE OF"));
    // The Part IV columns as printed (1801 manual-3).
    for printed in ["A. Exclusive", "B. Conjugal/Communal", "C. Total"] {
        assert!(sheet.contains(printed), "{printed}");
    }
    for name in [
        "RealProp",
        "Fam",
        "PersonalProp",
        "Taxable",
        "Business",
        "GrossEst",
        "OrdDed",
        "EstAfterDed",
    ] {
        let labels: Vec<String> = ["A", "B", "C"]
            .iter()
            .map(|c| placed("1801", &format!("frm1801v2018:txt{name}_{c}")).label)
            .collect();
        assert_eq!(
            labels,
            ["A. Exclusive", "B. Conjugal/Communal", "C. Total"],
            "{name}"
        );
    }
}

/// 1800 manual-0 and 1801 manual-3: Part V is its own part, with each
/// schedule table and its TOTAL; the page 2 header copies of the TIN and
/// name are not drawn, and the print still writes them from the draft.
#[test]
fn pr10_part_v_is_its_own_part_and_the_header_copy_is_not_drawn() {
    setup();
    for &(code, slug) in FORMS {
        let parts: Vec<String> = schema_entry(code).unwrap()["parts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["n"].as_str().unwrap().to_string())
            .collect();
        assert!(
            parts.contains(&"Part V · Schedules".to_string()),
            "{parts:?}"
        );
        // Every joined schedule box is in Part V.
        let mut schedules = 0;
        for join in joins(slug) {
            let printed = join["printed"].as_str().unwrap();
            let key = join["writer_key"].as_str().unwrap();
            if (printed.starts_with("Schedule") || printed.starts_with("TOTAL"))
                && is_drawn(code, key)
            {
                assert_eq!(placed(code, key).part, "Part V · Schedules", "{code} {key}");
                schedules += 1;
            }
        }
        assert!(schedules >= 20, "{code}: {schedules}");
    }
    for (code, header, shown) in [
        (
            "1800",
            vec![
                "frm1800v2018:txtPg2TIN1",
                "frm1800v2018:txtPg2TIN2",
                "frm1800v2018:txtPg2TIN3",
                "frm1800v2018:txtPg2BranchCode",
                "frm1800v2018:txtPg2TaxpayerName",
            ],
            (
                "frm1800v2018:txtPg2TaxpayerName",
                "frm1800v2018:txtDonorName",
            ),
        ),
        (
            "1801",
            vec![
                "frm1801v2018:tinA_2",
                "frm1801v2018:tinB_2",
                "frm1801v2018:tinC_2",
                "frm1801v2018:branchCode_2",
                "frm1801v2018:registeredName_2",
            ],
            (
                "frm1801v2018:registeredName_2",
                "frm1801v2018:registeredName_1",
            ),
        ),
    ] {
        for key in &header {
            assert!(schema_keys(code).contains(*key), "{code} {key}");
            assert!(!is_drawn(code, key), "{code} {key}");
        }
        let opened = open(code, 1);
        let map = printed(code, &opened["draft"]);
        assert!(!map[shown.0].is_empty());
        assert_eq!(map[shown.0], opened["values"][shown.1], "{code}");
    }
}

/// 1800 R2.0 and R3.2: the line of business, which the profile fills and
/// the January 2018 sheet does not print (writer-cells.json not_joined),
/// is in "Not on the printed sheet" with Edit; the page 2 header copy of
/// the name is not drawn (above).
#[test]
fn pr10_1800_profile_boxes_are_placed() {
    setup();
    let not_joined = writer_cells("1800-2018")["not_joined"].clone();
    assert!(
        not_joined
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["writer_key"] == "frm1800v2018:txtLineBus")
    );
    let lob = placed("1800", "frm1800v2018:txtLineBus");
    assert_eq!(lob.part, "Not on the printed sheet");
    assert_ne!(lob.item, "Additional details");
    let opened = open("1800", 1);
    assert!(strings(&opened["profile_edit"]).contains("frm1800v2018:txtLineBus"));
    assert_eq!(
        opened["values"]["frm1800v2018:txtLineBus"],
        "WHOLESALE TRADING"
    );
}

/// 1800 manual-1, manual-2 and R3.5; items the sheet preprints (15, the 6%
/// rate; 37, the ₱250,000.00 exempt gift) and Part III and the tax agent
/// line, which the official program keeps disabled, are notes: no box is
/// drawn, written or chosen by the page.
#[test]
fn pr10_1800_printed_constants_and_disabled_parts_are_notes() {
    use bir_core::forms::form_1800::{FORM_1800_EXEMPT_GIFT, FORM_1800_TAX_RATE};
    let sheet = sheet_text("1800-2018");
    assert!(
        sheet.contains("6 0 15 Applicable Donors Tax Rate"),
        "{sheet}"
    );
    assert!(sheet.contains("2 5 0 0 0 0 0 0 37 Less: Exempt Gift"));
    assert_eq!(FORM_1800_TAX_RATE, 0.06);
    assert_eq!(FORM_1800_EXEMPT_GIFT, 250_000.0);
    let notes: Vec<(String, String, String)> = items("1800")
        .into_iter()
        .filter_map(|(part, it)| {
            it["note"].as_str().map(|n| {
                assert!(it["f"].as_array().unwrap().is_empty());
                (part, it["no"].as_str().unwrap().to_string(), n.to_string())
            })
        })
        .collect();
    let note = |no: &str| {
        notes
            .iter()
            .find(|(_, n, _)| n == no)
            .unwrap_or_else(|| panic!("note {no}"))
            .2
            .clone()
    };
    assert_eq!(note("15"), "6% (printed on the form)");
    assert_eq!(note("37"), "250,000.00 (printed on the form)");
    let part_iii: Vec<&str> = notes
        .iter()
        .filter(|(p, _, _)| p == "Part III · Details of Payment")
        .map(|(_, no, _)| no.as_str())
        .collect();
    assert_eq!(part_iii, ["21", "22", "23", "24"]);
    assert!(notes.iter().any(|(_, _, n)| n.contains("tax agent")));
    // Every Part III and tax agent box writer-cells.json joins: none is
    // drawn, and the official program has each disabled at load.
    let shut: Vec<String> = joins("1800-2018")
        .into_iter()
        .filter(|j| {
            let printed = j["printed"].as_str().unwrap();
            ["21 ", "22 ", "23 ", "24 "]
                .iter()
                .any(|p| printed.starts_with(p))
                || printed.starts_with("Tax Agent")
                || printed.starts_with("Date of")
        })
        .map(|j| j["writer_key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(shut.len(), 19, "{shut:?}");
    for key in &shut {
        assert!(!is_drawn("1800", key), "{key}");
        assert_eq!(control("1800-v2018", key)["disabled"], true, "{key}");
    }
    // 9B has no box in the official program.
    assert!(sheet.contains("9B RDO Code"));
    assert!(note("9B").starts_with("No box"));
}

/// 1800 R6.0, R3.4 and preview.0: item 13's Yes and No are the draft's tax
/// relief. 13A is off with what turns it on while item 13 is No, refuses
/// text then (the draft keeps nothing), takes it once Yes is ticked, and
/// prints; No asks before it clears 13A. Page and print agree.
#[test]
fn pr10_1800_item_13_answers_and_13a_waits_for_yes() {
    setup();
    let (yes, no, spec) = (
        "frm1800v2018:TaxTreatyYN_Y",
        "frm1800v2018:TaxTreatyYN_N",
        "frm1800v2018:TaxTreaty",
    );
    let corpus = rules_file("1800-v2018", "fields.json");
    let condition = corpus["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["field_key"] == spec)
        .unwrap()["enabled_when"]
        .clone();
    assert_eq!(condition, "Item 13 Yes.");
    let opened = open("1800", 1);
    assert!(opened["draft"]["tax_relief"].is_null());
    assert_eq!(opened["values"][no], "true");
    assert_eq!(opened["off"][spec], "Needs item 13 = Yes");
    assert!(opened["locks"].get(yes).is_none() && opened["locks"].get(no).is_none());
    // Typed while No: refused with its reason, nothing kept.
    let refused = fill("1800", &opened["draft"], json!({ spec: "RA 9504" }));
    assert_eq!(
        rejected(&refused, spec).as_deref(),
        Some(rows_donor_estate::ANSWER_13_FIRST)
    );
    assert_eq!(refused["draft"], opened["draft"]);
    // Yes, then 13A in one batch: the answer goes first.
    let both = fill(
        "1800",
        &opened["draft"],
        json!({ yes: "true", no: "false", spec: "RA 9504" }),
    );
    assert!(both["rejected"].as_object().unwrap().is_empty(), "{both}");
    assert_eq!(both["draft"]["tax_relief"], "RA 9504");
    // Yes alone keeps an empty 13A that validation asks for, and opens it.
    let on = fill("1800", &opened["draft"], pick(&[yes, no], yes));
    assert_eq!(on["draft"]["tax_relief"], "");
    let checked = validated("1800", &on["draft"]);
    assert!(checked["off"].get(spec).is_none(), "{}", checked["off"]);
    let typed = fill("1800", &on["draft"], json!({ spec: "RA 9504" }));
    assert_eq!(typed["values"][spec], "RA 9504");
    let map = printed("1800", &typed["draft"]);
    assert_eq!(map[yes], "true");
    assert_eq!(map[no], "false");
    assert_eq!(map[spec], "RA 9504");
    assert_eq!(uploaded("1800", &typed["draft"])[yes], "true");
    // No clears 13A: the page asks first and offers Undo.
    let cleared = fill("1800", &typed["draft"], pick(&[yes, no], no));
    assert!(cleared["draft"]["tax_relief"].is_null());
    assert_eq!(
        cleared["removes"]["message"],
        "Answering No at item 13 clears item 13A (\"RA 9504\"). Continue?"
    );
    assert_eq!(printed("1800", &cleared["draft"])[no], "true");
    // Nothing typed in 13A: No asks nothing.
    let quiet = fill("1800", &on["draft"], pick(&[yes, no], no));
    assert!(quiet["removes"].is_null());
}

/// 1800 preview.0 and R3.3: items 28-32 are the draft's deductions rows,
/// written by the page, saved and printed; item 33 stays 0.00 as on the
/// official page (form_1800.rs `recompute`). An amount is never negative
/// or past the official round limit; a cleared row goes.
#[test]
fn pr10_1800_deductions_are_typed_and_printed() {
    setup();
    let opened = open("1800", 1);
    for item in 28..=32 {
        let title = format!("frm1800v2018:txtDeductionTitle{item}");
        let amount = format!("frm1800v2018:txtDeductionAmount{item}");
        assert!(opened["locks"].get(&title).is_none(), "{title}");
        assert!(opened["locks"].get(&amount).is_none(), "{amount}");
        assert_eq!(placed("1800", &title).t, "text");
        assert_eq!(placed("1800", &amount).t, "money");
        assert_eq!(
            printed_caption("1800-2018", &title),
            format!("Less: Deduction row {} (item {item}) title", item - 27)
        );
    }
    let filled = fill(
        "1800",
        &opened["draft"],
        json!({
            "frm1800v2018:txtDeductionTitle30": "Dowry",
            "frm1800v2018:txtDeductionAmount30": "10,000.50",
        }),
    );
    assert!(
        filled["rejected"].as_object().unwrap().is_empty(),
        "{filled}"
    );
    let rows = filled["draft"]["deductions"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2], json!({"title": "Dowry", "amount": 10000.5}));
    assert_eq!(
        filled["values"]["frm1800v2018:txtDeductionTitle30"],
        "Dowry"
    );
    assert_eq!(
        filled["values"]["frm1800v2018:txtDeductionAmount30"],
        "10,000.50"
    );
    let saved = save("1800", &filled["draft"]);
    let map = printed("1800", &saved["draft"]);
    assert_eq!(map["frm1800v2018:txtDeductionTitle30"], "Dowry");
    assert_eq!(map["frm1800v2018:txtDeductionAmount30"], "10,000.50");
    assert_eq!(map["frm1800v2018:TotalDeductionsAllowed"], "0.00");
    for bad in ["-5", "(5)", "1000000000000", "abc"] {
        let refused = fill(
            "1800",
            &filled["draft"],
            json!({ "frm1800v2018:txtDeductionAmount30": bad }),
        );
        assert!(
            rejected(&refused, "frm1800v2018:txtDeductionAmount30").is_some(),
            "{bad}"
        );
        assert_eq!(refused["draft"], filled["draft"], "{bad}");
    }
    let cleared = fill(
        "1800",
        &filled["draft"],
        json!({
            "frm1800v2018:txtDeductionTitle30": "",
            "frm1800v2018:txtDeductionAmount30": "",
        }),
    );
    assert_eq!(cleared["draft"]["deductions"], json!([]));
}

/// 1800 manual-3: item 11 shows the email as typed; the upload keeps the
/// official capital() upper case and the print shows it as typed
/// (form_1800.rs `to_print_field_map`).
#[test]
fn pr10_1800_email_shows_as_typed() {
    setup();
    let opened = open("1800", 1);
    let email = opened["draft"]["email"].as_str().unwrap().to_string();
    assert_eq!(email, "books@example.com");
    assert_eq!(opened["values"]["frm1800v2018:txtEmail"], email);
    assert_eq!(
        uploaded("1800", &opened["draft"])["frm1800v2018:txtEmail"],
        "BOOKS@EXAMPLE.COM"
    );
    assert_eq!(
        printed("1800", &opened["draft"])["frm1800v2018:txtEmail"],
        email
    );
}

/// Decision D6: a message names the printed item number. Each official
/// 1800 message that names another number is rewritten to the sheet's
/// (writer-cells.json captions: "5 Donor TIN", "6 RDO Code", "7 Donor's
/// Name"; index.html "13A"); every message either form gives here names
/// only numbers the sheet prints.
#[test]
fn pr10_messages_name_the_printed_item_numbers() {
    setup();
    assert_eq!(
        printed_number(&printed_caption("1800-2018", "frm1800v2018:txtTIN1")).as_deref(),
        Some("5")
    );
    assert_eq!(
        printed_number(&printed_caption("1800-2018", "frm1800v2018:txtRDOCode")).as_deref(),
        Some("6")
    );
    assert_eq!(
        printed_number(&printed_caption("1800-2018", "frm1800v2018:txtDonorName")).as_deref(),
        Some("7")
    );
    assert!(sheet_text("1800-2018").contains("13A If yes,"));
    let opened = open("1800", 1);
    let mut broken = opened["draft"].clone();
    broken["tin"] = json!("");
    broken["rdo_code"] = json!("000");
    broken["donor_name"] = json!("");
    broken["tax_relief"] = json!("");
    let messages: Vec<String> = validated("1800", &broken)["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect();
    for (form, official, wording) in rows_donor_estate::PRINTED_MESSAGES {
        assert_eq!(*form, "1800");
        assert!(!messages.iter().any(|m| m == official), "{official}");
        // The official text, typed by bir-core, is what the rewrite reads.
        assert_eq!(
            rows_donor_estate::printed_message(kind_of("1800").unwrap(), official),
            Some(*wording)
        );
    }
    for expected in [
        "Please specify the Special Treaty or International Law the payee is availing in item 13A.",
        "Please enter a valid RDO Code on Item 6.",
        "Please enter a valid Taxpayer's Name on Item 7.",
    ] {
        assert!(
            messages.iter().any(|m| m == expected),
            "{expected}: {messages:?}"
        );
    }
    // The empty TIN gives "Item 4" officially (bir-core's own check).
    let mut no_tin = opened["draft"].clone();
    no_tin["tin"] = json!("");
    let typed: bir_core::forms::form_1800::Form1800Draft =
        serde_json::from_value(no_tin.clone()).unwrap();
    use bir_core::forms::FormValidator;
    assert!(
        FormValidator::validate(&typed)
            .iter()
            .any(|(_, m)| m == "Please enter a valid TIN number on Item 4.")
    );
    assert!(
        validated("1800", &no_tin)["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["message"] == "Please enter a valid TIN number on Item 5.")
    );
    // Every message on both forms' fresh and broken returns cites a number
    // the sheet prints.
    for (code, slug, draft) in [
        ("1800", "1800-2018", broken),
        ("1800", "1800-2018", opened["draft"].clone()),
        ("1801", "1801-2018", open("1801", 1)["draft"].clone()),
    ] {
        let sheet = sheet_text(slug);
        for error in validated(code, &draft)["errors"].as_array().unwrap() {
            let message = error["message"].as_str().unwrap();
            let words: Vec<&str> = message.split_whitespace().collect();
            for pair in words.windows(2) {
                if pair[0].eq_ignore_ascii_case("item") {
                    let no = pair[1].trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
                    assert!(
                        sheet.contains(&format!(" {no} ")),
                        "{code}: {message:?} names item {no}"
                    );
                }
            }
        }
    }
}

/// 1801 manual-0: item 1 is one date box (the page shows it as "Dec 10,
/// 2020"), blank on a new return, never "00 / 00 / 00"; the draft and the
/// upload keep the official three boxes. A blank clears a part; a part that
/// is not a month, day or four-digit year is refused.
#[test]
fn pr10_1801_item_1_is_one_date_kept_as_three_boxes() {
    setup();
    let (month, day, year) = (
        "frm1801v2018:txtDateMonth",
        "frm1801v2018:txtDateDay",
        "frm1801v2018:txtDateYear",
    );
    let one = item_of("1801", month);
    assert_eq!(one["one_date"], true);
    assert_eq!(one["no"], "1");
    let marked: Vec<String> = items("1801")
        .into_iter()
        .filter(|(_, it)| it["one_date"] == true)
        .map(|(_, it)| it["f"][0]["k"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(marked, [month]);
    assert!(
        items("1800")
            .iter()
            .all(|(_, it)| it.get("one_date").is_none())
    );
    let opened = open("1801", 1);
    for key in [month, day, year] {
        assert_eq!(opened["values"][key], "", "{key}");
    }
    let upload = uploaded("1801", &opened["draft"]);
    assert_eq!(
        [&upload[month], &upload[day], &upload[year]],
        ["00", "00", "00"]
    );
    let typed = fill(
        "1801",
        &opened["draft"],
        json!({ month: "12", day: "10", year: "2020" }),
    );
    assert!(typed["rejected"].as_object().unwrap().is_empty(), "{typed}");
    assert_eq!(
        [
            &typed["draft"]["death_month"],
            &typed["draft"]["death_day"],
            &typed["draft"]["death_year"]
        ],
        [12, 10, 2020]
    );
    assert_eq!(typed["values"][month], "12");
    let map = printed("1801", &typed["draft"]);
    assert_eq!(map[year], "2020");
    let cleared = fill(
        "1801",
        &typed["draft"],
        json!({ month: "", day: "", year: "" }),
    );
    assert!(cleared["rejected"].as_object().unwrap().is_empty());
    assert_eq!(cleared["draft"]["death_month"], 0);
    assert_eq!(cleared["draft"]["death_year"], 0);
    assert_eq!(cleared["values"][year], "");
    for (key, bad) in [
        (month, "13"),
        (month, "0"),
        (day, "32"),
        (year, "20"),
        (year, "20201"),
        (year, "abcd"),
    ] {
        let refused = fill("1801", &typed["draft"], json!({ key: bad }));
        assert!(rejected(&refused, key).is_some(), "{key} {bad}");
        assert_eq!(refused["draft"], typed["draft"]);
    }
}

/// 1801 manual-2: item 11, the executor's TIN, is typed on this return (the
/// profile is the estate's): open boxes the host writes into the draft's
/// `administrator_tin`, in order, drawn without the profile mark
/// (`typed_tin`); the upload and print write the official three boxes.
#[test]
fn pr10_1801_executor_tin_is_typed_per_return() {
    setup();
    let tine = |n: usize| format!("frm1801v2018:txtTINE{n}");
    assert_eq!(item_of("1801", &tine(1))["typed_tin"], true);
    assert!(
        item_of("1801", "frm1801v2018:tinA_1")
            .get("typed_tin")
            .is_none()
    );
    let opened = open("1801", 1);
    let profile = strings(&opened["profile"]);
    for n in 1..=3 {
        assert!(opened["locks"].get(tine(n)).is_none(), "{}", tine(n));
        assert!(!profile.contains(&tine(n)));
        assert_eq!(
            control("1801-v2018", &tine(n))["maxlength"],
            "3",
            "{}",
            tine(n)
        );
    }
    let filled = fill(
        "1801",
        &opened["draft"],
        json!({ tine(1): "123", tine(2): "456", tine(3): "789" }),
    );
    assert!(
        filled["rejected"].as_object().unwrap().is_empty(),
        "{filled}"
    );
    assert_eq!(filled["draft"]["administrator_tin"], "123456789");
    let map = printed("1801", &filled["draft"]);
    let up = uploaded("1801", &filled["draft"]);
    for (n, part) in [(1, "123"), (2, "456"), (3, "789")] {
        assert_eq!(up[&tine(n)], part);
        assert_eq!(filled["values"][tine(n)], part);
        assert_eq!(map.get(&tine(n)).map(String::as_str), Some(part));
    }
    // A whole TIN pasted into the first box fills the branch code too.
    let pasted = fill(
        "1801",
        &opened["draft"],
        json!({ tine(1): "987-654-321-00000" }),
    );
    assert_eq!(pasted["draft"]["administrator_tin"], "987654321");
    assert_eq!(pasted["draft"]["administrator_branch_code"], "00000");
    // Out of order is refused, the draft unchanged.
    let refused = fill("1801", &opened["draft"], json!({ tine(3): "789" }));
    assert!(rejected(&refused, &tine(3)).is_some());
    assert_eq!(refused["draft"], opened["draft"]);
}

/// 1801 R6.0 and R3.3: item 14's Yes and No are the draft's tax relief;
/// 14A is off while No, refuses text then, takes it once Yes; a blank 14A
/// keeps Yes (the probed nullable binding would answer No); No asks before
/// it clears 14A.
#[test]
fn pr10_1801_item_14_answers_and_14a_waits_for_yes() {
    setup();
    let (yes, no, spec) = (
        "frm1801v2018:optTreaty_1",
        "frm1801v2018:optTreaty_2",
        "frm1801v2018:treatyY",
    );
    let opened = open("1801", 1);
    assert!(opened["locks"].get(yes).is_none() && opened["locks"].get(no).is_none());
    assert_eq!(opened["off"][spec], "Needs item 14 = Yes");
    let refused = fill("1801", &opened["draft"], json!({ spec: "RA 9504" }));
    assert_eq!(
        rejected(&refused, spec).as_deref(),
        Some(rows_donor_estate::ANSWER_14_FIRST)
    );
    assert_eq!(refused["draft"], opened["draft"]);
    let on = fill("1801", &opened["draft"], pick(&[yes, no], yes));
    assert_eq!(on["draft"]["tax_relief"], "");
    assert!(validated("1801", &on["draft"])["off"].get(spec).is_none());
    let typed = fill("1801", &on["draft"], json!({ spec: "RA 9504" }));
    assert_eq!(typed["draft"]["tax_relief"], "RA 9504");
    let blank = fill("1801", &typed["draft"], json!({ spec: "" }));
    assert_eq!(blank["draft"]["tax_relief"], "");
    assert_eq!(printed("1801", &blank["draft"])[yes], "true");
    let map = printed("1801", &typed["draft"]);
    assert_eq!(map[yes], "true");
    assert_eq!(map[spec], "RA 9504");
    let cleared = fill("1801", &typed["draft"], pick(&[yes, no], no));
    assert!(cleared["draft"]["tax_relief"].is_null());
    assert_eq!(
        cleared["removes"]["message"],
        "Answering No at item 14 clears item 14A (\"RA 9504\"). Continue?"
    );
}

/// 1801 R3.1 and R6.1: item 15D's frequency waits for 15D Yes and its
/// Others (specify) for Others, by their tracked rule records (the corpus
/// condition for the Others box; the function inventory's
/// `installmentGranted` and `freqOtherChecked`, which read 15D's Yes box).
/// A shut box refuses what is typed, by its own key, its group's key or in
/// a batch; 15D No goes through the form's setter, which clears the
/// frequency, and asks first.
#[test]
fn pr10_1801_frequency_waits_for_15d() {
    setup();
    let p = |k: &str| format!("frm1801v2018:{k}");
    let functions = rules_file(
        "1801-v2018",
        "fixtures/calculation-function-inventory-v796.json",
    );
    let granted = functions["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "installmentGranted")
        .unwrap()["control_ids"]
        .clone();
    for key in [
        "freq_Mon",
        "freq_Qtr",
        "freq_Semi",
        "freq_Other",
        "instGranted_1",
    ] {
        assert!(
            granted.as_array().unwrap().contains(&json!(p(key))),
            "{key}"
        );
    }
    let checks = rules_file(
        "1801-v2018",
        "fixtures/validation-function-inventory-v796.json",
    );
    let other = checks["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "freqOtherChecked")
        .unwrap()["control_ids"]
        .clone();
    assert!(
        other
            .as_array()
            .unwrap()
            .contains(&json!(p("instGranted_1")))
    );
    assert!(
        other
            .as_array()
            .unwrap()
            .contains(&json!(p("freqOtherInput")))
    );
    let freq = [
        p("freq_Mon"),
        p("freq_Qtr"),
        p("freq_Semi"),
        p("freq_Other"),
    ];
    let freq_refs: Vec<&str> = freq.iter().map(String::as_str).collect();
    let opened = open("1801", 1);
    for key in &freq {
        assert_eq!(opened["off"][key], "Needs item 15D = Yes", "{key}");
    }
    assert_eq!(
        opened["off"][p("freqOtherInput")],
        "Needs item 15D = Yes and frequency Others"
    );
    // Shut: by its own key, the group's key, and the Others text.
    for edits in [
        pick(&freq_refs, &p("freq_Mon")),
        json!({ p("freq"): "Mon" }),
        json!({ p("freqOtherInput"): "ANNUAL" }),
    ] {
        let refused = fill("1801", &opened["draft"], edits.clone());
        assert!(
            !refused["rejected"].as_object().unwrap().is_empty(),
            "{edits}"
        );
        assert_eq!(refused["draft"], opened["draft"], "{edits}");
    }
    // 15D Yes and a frequency in one batch: the answer goes first.
    let on = fill(
        "1801",
        &opened["draft"],
        json!({ p("instGranted_1"): "true", p("instGranted_2"): "false", p("freq_Other"): "true" }),
    );
    assert!(on["rejected"].as_object().unwrap().is_empty(), "{on}");
    assert_eq!(on["draft"]["installment_granted"], true);
    assert_eq!(on["draft"]["installment_frequency"], "others");
    let checked = validated("1801", &on["draft"]);
    for key in freq.iter().chain([&p("freqOtherInput")]) {
        assert!(checked["off"].get(key).is_none(), "{key}");
    }
    let typed = fill(
        "1801",
        &on["draft"],
        json!({ p("freqOtherInput"): "Annual" }),
    );
    assert_eq!(typed["draft"]["installment_frequency_other"], "Annual");
    let map = printed("1801", &typed["draft"]);
    assert_eq!(map[&p("freq_Other")], "true");
    assert_eq!(map[&p("freqOtherInput")], "ANNUAL");
    // The group key answers too.
    let semi = fill("1801", &typed["draft"], json!({ p("freq"): "Semi" }));
    assert_eq!(semi["draft"]["installment_frequency"], "semi_annual");
    // 15D No (by its box or its group key) clears the frequency through the
    // setter, and asks first.
    for edits in [
        pick(
            &[&p("instGranted_1"), &p("instGranted_2")],
            &p("instGranted_2"),
        ),
        json!({ p("instGranted"): "2" }),
    ] {
        let no = fill("1801", &typed["draft"], edits.clone());
        assert_eq!(no["draft"]["installment_granted"], false, "{edits}");
        assert!(no["draft"]["installment_frequency"].is_null());
        assert_eq!(no["draft"]["installment_frequency_other"], "");
        assert_eq!(
            no["removes"]["message"],
            "Answering No at item 15D clears its frequency of payment and the Others (specify) text (\"Annual\"). Continue?"
        );
        let map = printed("1801", &no["draft"]);
        assert_eq!(map[&p("freq_Other")], "false");
        assert_eq!(map[&p("instGranted_2")], "true");
    }
}

/// Lesson 6 with literal fixtures: `fixtures/1800-pre-pr10.json` and
/// `fixtures/1801-pre-pr10.json` are drafts exactly as pr0b-push serialized
/// them (built and written by the pr0b-push bir-core, which PR 10 does not
/// change: tax relief, deductions, the executor's TIN, 15D's frequency and
/// Others text typed). Stored, opened on the page and saved again, each row
/// is its fixture byte for byte (the row id aside, which the database
/// assigns), and the page shows what was typed.
#[test]
fn pr10_drafts_stored_before_pr10_save_back_byte_for_byte() {
    use bir_core::forms::form_1800::Form1800Draft;
    use bir_core::forms::form_1801::Form1801Draft;
    use bir_core::forms::queueable::QueueableForm;
    setup();
    fn round_trip<F: QueueableForm + serde::de::DeserializeOwned + serde::Serialize>(
        code: &str,
        fixture: &str,
    ) -> Value {
        let stored: F = serde_json::from_str(fixture).unwrap();
        let (tin, year, period) = (
            stored.tin().to_string(),
            stored.taxable_year(),
            stored.period_column(),
        );
        let row = || -> String {
            let mut draft = with_database(|db| {
                db.get_queueable_draft::<F>(&tin, year, period)
                    .map_err(|error| HostError::new(error.to_string()))
            })
            .unwrap()
            .expect("the stored row");
            draft.set_draft_id(None);
            serde_json::to_string(&draft).unwrap()
        };
        with_database(|db| {
            db.save_queueable_draft(&stored)
                .map(|_| ())
                .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
        assert_eq!(row(), fixture, "{code}: stored as the old code stored it");
        let opened = open_for(&tin, code, i64::from(year), period);
        save(code, &opened["draft"]);
        assert_eq!(row(), fixture, "{code}: opened and saved, byte for byte");
        opened
    }
    let opened = round_trip::<Form1800Draft>("1800", include_str!("fixtures/1800-pre-pr10.json"));
    assert_eq!(opened["values"]["frm1800v2018:TaxTreatyYN_Y"], "true");
    assert_eq!(opened["values"]["frm1800v2018:TaxTreaty"], "RA 9504");
    assert_eq!(
        opened["values"]["frm1800v2018:txtDeductionTitle28"],
        "Dowry"
    );
    let opened = round_trip::<Form1801Draft>("1801", include_str!("fixtures/1801-pre-pr10.json"));
    assert_eq!(opened["values"]["frm1801v2018:txtTINE1"], "123");
    assert_eq!(opened["values"]["frm1801v2018:freq_Other"], "true");
    assert_eq!(opened["values"]["frm1801v2018:freqOtherInput"], "ANNUAL");
    assert_eq!(opened["values"]["frm1801v2018:txtDateYear"], "2025");
}

/// Review fix 3: Amended Return No zeroes 1800's 17C and 1801's 19B (the
/// forms' `recompute` clears `tax_paid_previous`), so the page asks first
/// and offers Undo, by the box or the group box; with nothing typed there
/// it asks nothing.
#[test]
fn pr10_amended_return_no_asks_before_it_clears_the_tax_paid_before() {
    setup();
    for (code, yes, no, group, box_key, item) in [
        (
            "1800",
            "frm1800v2018:AmendedRtn_1",
            "frm1800v2018:AmendedRtn_2",
            "frm1800v2018:AmendedRtn",
            "frm1800v2018:TaxCredit17C",
            "17C",
        ),
        (
            "1801",
            "frm1801v2018:amendedRtn_1",
            "frm1801v2018:amendedRtn_2",
            "frm1801v2018:amendedRtn",
            "frm1801v2018:txtCredits_TaxPrev",
            "19B",
        ),
    ] {
        let opened = open(code, 1);
        let amended = fill(code, &opened["draft"], pick(&[yes, no], yes));
        let quiet = fill(code, &amended["draft"], pick(&[yes, no], no));
        assert!(quiet["removes"].is_null(), "{code}: nothing typed");
        let typed = fill(code, &amended["draft"], json!({ box_key: "1,000.00" }));
        assert_eq!(typed["draft"]["tax_paid_previous"], 1000.0, "{code}");
        for edits in [pick(&[yes, no], no), json!({ group: "2" })] {
            let cleared = fill(code, &typed["draft"], edits.clone());
            assert_eq!(cleared["draft"]["tax_paid_previous"], 0.0, "{code} {edits}");
            assert_eq!(
                cleared["removes"]["message"],
                format!(
                    "Answering No at Amended Return clears item {item} (\u{20b1}1,000.00). Continue?"
                ),
                "{code} {edits}"
            );
            assert_eq!(cleared["removes"]["done"], format!("Cleared item {item}"));
        }
    }
}

/// Review fix 4: one batch that removes several things names them all in
/// one question (1801: item 14 No and 15D No together).
#[test]
fn pr10_one_batch_names_every_removal() {
    setup();
    let p = |k: &str| format!("frm1801v2018:{k}");
    let opened = open("1801", 1);
    let set = fill(
        "1801",
        &opened["draft"],
        json!({
            p("optTreaty_1"): "true",
            p("optTreaty_2"): "false",
            p("treatyY"): "RA 9504",
            p("instGranted_1"): "true",
            p("instGranted_2"): "false",
            p("freq_Other"): "true",
            p("freqOtherInput"): "Annual",
        }),
    );
    assert!(set["rejected"].as_object().unwrap().is_empty(), "{set}");
    let both = fill(
        "1801",
        &set["draft"],
        json!({
            p("optTreaty_1"): "false",
            p("optTreaty_2"): "true",
            p("instGranted_1"): "false",
            p("instGranted_2"): "true",
        }),
    );
    assert!(both["draft"]["tax_relief"].is_null());
    assert_eq!(both["draft"]["installment_granted"], false);
    assert_eq!(
        both["removes"]["message"],
        "Answering No at item 14 clears item 14A (\"RA 9504\"). Answering No at item 15D \
         clears its frequency of payment and the Others (specify) text (\"Annual\"). Continue?"
    );
    assert_eq!(both["removes"]["rows"], 2);
    assert_eq!(
        both["removes"]["done"],
        "Cleared item 14A; Cleared item 15D's frequency"
    );
}

/// Review fix 5: changing 15D's frequency from Others for another choice
/// clears the Others (specify) text, which the official serializer never
/// files then (form_1801.rs `to_bir_field_map` writes freqOtherInput only
/// for Others; `set_installment_granted`, the port of `installmentGranted`,
/// clears it), and unticking the chosen frequency clears it: both ask
/// first. Changing one choice for another with nothing typed asks nothing.
#[test]
fn pr10_1801_frequency_change_asks_before_it_clears() {
    setup();
    let p = |k: &str| format!("frm1801v2018:{k}");
    let freq = [
        p("freq_Mon"),
        p("freq_Qtr"),
        p("freq_Semi"),
        p("freq_Other"),
    ];
    let freq_refs: Vec<&str> = freq.iter().map(String::as_str).collect();
    let opened = open("1801", 1);
    let others = fill(
        "1801",
        &opened["draft"],
        json!({ p("instGranted_1"): "true", p("freq_Other"): "true", p("freqOtherInput"): "Annual" }),
    );
    assert_eq!(others["draft"]["installment_frequency_other"], "Annual");
    let monthly = fill("1801", &others["draft"], pick(&freq_refs, &p("freq_Mon")));
    assert_eq!(monthly["draft"]["installment_frequency"], "monthly");
    assert_eq!(monthly["draft"]["installment_frequency_other"], "");
    assert_eq!(
        monthly["removes"]["message"],
        "Changing item 15D's frequency from Others clears the Others (specify) text (\"Annual\"). Continue?"
    );
    assert_eq!(
        uploaded("1801", &monthly["draft"])[&p("freqOtherInput")],
        ""
    );
    let quarterly = fill("1801", &monthly["draft"], pick(&freq_refs, &p("freq_Qtr")));
    assert_eq!(quarterly["draft"]["installment_frequency"], "quarterly");
    assert!(quarterly["removes"].is_null());
    let unticked = fill(
        "1801",
        &quarterly["draft"],
        json!({ p("freq_Qtr"): "false" }),
    );
    assert!(unticked["draft"]["installment_frequency"].is_null());
    assert_eq!(
        unticked["removes"]["message"],
        "Unticking the frequency of payment at item 15D clears it. Continue?"
    );
    let from_others = fill(
        "1801",
        &others["draft"],
        json!({ p("freq_Other"): "false" }),
    );
    assert!(from_others["draft"]["installment_frequency"].is_null());
    assert_eq!(
        from_others["removes"]["message"],
        "Unticking the frequency of payment at item 15D clears it and the Others (specify) text (\"Annual\"). Continue?"
    );
}

/// Review fix 6: a new return's item 1 date is blank on both forms, and
/// says nothing until the filer checks: the open carries no date message,
/// Check (screen_form_validate) still reports it, and bir-core's own
/// validation is unchanged.
#[test]
fn pr10_a_new_returns_date_is_blank_and_quiet_until_checked() {
    use bir_core::forms::FormValidator;
    setup();
    for (code, keys, field, message) in [
        (
            "1800",
            [
                "frm1800v2018:txtMonth",
                "frm1800v2018:txtDate",
                "frm1800v2018:txtYear",
            ],
            "donation_date",
            "Please enter a valid date on Item 1.",
        ),
        (
            "1801",
            [
                "frm1801v2018:txtDateMonth",
                "frm1801v2018:txtDateDay",
                "frm1801v2018:txtDateYear",
            ],
            "death_date",
            "Please enter a valid Return Date",
        ),
    ] {
        let opened = open(code, 1);
        for key in keys {
            assert_eq!(opened["values"][key], "", "{code} {key}");
        }
        let fields = |errors: &Value| -> Vec<String> {
            errors
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e["field"].as_str().unwrap().to_string())
                .collect()
        };
        assert!(
            !fields(&opened["errors"]).contains(&field.to_string()),
            "{code}"
        );
        let checked = validated(code, &opened["draft"]);
        assert!(
            checked["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["field"] == field && e["message"] == message),
            "{code}: Check reports the date"
        );
        let core: Vec<(String, String)> = match code {
            "1800" => FormValidator::validate(
                &serde_json::from_value::<bir_core::forms::form_1800::Form1800Draft>(
                    opened["draft"].clone(),
                )
                .unwrap(),
            ),
            _ => FormValidator::validate(
                &serde_json::from_value::<bir_core::forms::form_1801::Form1801Draft>(
                    opened["draft"].clone(),
                )
                .unwrap(),
            ),
        };
        assert!(
            core.iter().any(|(f, m)| f == field && m == message),
            "{code}"
        );
        // Saved and opened again with a date typed, its message shows.
        let mut typed = opened["draft"].clone();
        let month = if code == "1800" {
            "donation_month"
        } else {
            "death_month"
        };
        typed[month] = json!(13);
        let saved = save(code, &typed);
        let reopened = open(code, 1);
        assert_eq!(reopened["draft"][month], 13, "{code}: {saved}");
        assert!(
            fields(&reopened["errors"]).contains(&field.to_string()),
            "{code}"
        );
    }
}

/// Review fix 7: a deduction amount with more than two decimals is
/// refused, never rounded; the bound is on the stored amount, so
/// 999999999999.995 (which rounds to 1,000,000,000,000.00) is refused and
/// 999,999,999,999.99 kept.
#[test]
fn pr10_1800_deduction_amounts_keep_their_centavos_or_are_refused() {
    setup();
    let key = "frm1800v2018:txtDeductionAmount28";
    let opened = open("1800", 1);
    for bad in ["10.005", "1.999", "999999999999.995", "1e13"] {
        let refused = fill("1800", &opened["draft"], json!({ key: bad }));
        assert!(rejected(&refused, key).is_some(), "{bad}");
        assert_eq!(refused["draft"], opened["draft"], "{bad}");
    }
    let kept = fill(
        "1800",
        &opened["draft"],
        json!({ key: "999,999,999,999.99" }),
    );
    assert!(kept["rejected"].as_object().unwrap().is_empty(), "{kept}");
    assert_eq!(kept["draft"]["deductions"][0]["amount"], 999_999_999_999.99);
    let two = fill("1800", &opened["draft"], json!({ key: "10.50" }));
    assert_eq!(two["draft"]["deductions"][0]["amount"], 10.5);
}
