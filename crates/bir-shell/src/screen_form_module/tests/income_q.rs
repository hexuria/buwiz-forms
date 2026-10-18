//! PR 5: 1701Q and 1702Q against their printed sheets. Each check reads an
//! oracle the host does not compute: the frozen sheets' printed captions
//! (`html-frozen/<slug>/writer-cells.json`), the rules corpus
//! (`rules/forms/<package>/fields.json`), the print map, and hand lists.

use super::*;

const Q: &str = "1701Q";
const C: &str = "1702Q";

/// Maintainer decision 11's words for a closed spouse part.
const NEEDS_MARRIED: &str = "Needs Civil Status = Married";
const MARRIED_FIRST: &str =
    "Part II and column B are for a married filer: choose Married at Civil Status first.";

fn repo_json(path: &str) -> Value {
    let path = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")))
        .unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// Writer key -> printed caption, from a frozen sheet's writer cells.
fn captions(slug: &str) -> BTreeMap<String, String> {
    let cells = repo_json(&format!("html-frozen/{slug}/writer-cells.json"));
    let mut out = BTreeMap::new();
    for list in [
        "joins",
        "money_joins",
        "xbox_joins",
        "peso_joins",
        "date_joins",
    ] {
        for join in cells[list].as_array().into_iter().flatten() {
            if let (Some(key), Some(printed)) =
                (join["writer_key"].as_str(), join["printed"].as_str())
            {
                out.insert(key.to_string(), printed.to_string());
            }
        }
    }
    out
}

fn printed(code: &str, draft: &Value) -> BTreeMap<String, String> {
    let kind = kind_of(code).unwrap();
    with_database(|db| print_map(db, kind, draft)).unwrap().0
}

fn field_map(code: &str, draft: &Value) -> BTreeMap<String, String> {
    agent::field_map(kind_of(code).unwrap(), draft).unwrap()
}

/// Every (part name, item) of a form's page schema.
fn items(code: &str) -> Vec<(String, &'static Value)> {
    schema_entry(code).unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|part| {
            let name = part["n"].as_str().unwrap_or_default().to_string();
            part["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(move |item| (name.clone(), item))
        })
        .collect()
}

/// The item holding `key`, with its part's name.
fn item_of(code: &str, key: &str) -> (String, &'static Value) {
    items(code)
        .into_iter()
        .find(|(_, item)| {
            item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|field| field["k"] == key)
        })
        .unwrap_or_else(|| panic!("{code}: no item holds {key}"))
}

fn field_of(code: &str, key: &str) -> &'static Value {
    item_of(code, key).1["f"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["k"] == key)
        .unwrap()
}

/// The item number a printed caption names: its first token ("26 Tax
/// Due" -> 26) or, on 1702Q's page 2, the token after "Item".
fn caption_item(caption: &str) -> String {
    let rest = caption
        .split_once(" Item ")
        .map_or(caption, |(_, rest)| rest);
    rest.chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
}

/// Part II's boxes and the spouse's column B inputs, by the printed sheet:
/// every 1701Q box whose writer-cells caption is a spouse's (items 17-25A,
/// "(spouse)", "B) Spouse") that the page lets the filer type.
fn spouse_keys() -> Vec<String> {
    let typed = |key: &str| {
        let item: u32 = key
            .strip_prefix("frm1701q:txt")
            .and_then(|rest| rest.strip_suffix('B'))
            .and_then(|n| n.parse().ok())
            .unwrap_or(0);
        // Column B's typed items (Schedules I-IV); the rest are totals.
        !(26..=31).contains(&item)
            && ![38, 40, 41, 45, 46, 49, 51, 53, 54, 62, 63, 67, 68].contains(&item)
    };
    let mut keys: Vec<String> = captions("1701q-2018")
        .into_iter()
        .filter(|(_, caption)| {
            caption.contains("(spouse)")
                || caption.contains("B) Spouse")
                || ["17 ", "19 ", "21 ", "24 "]
                    .iter()
                    .any(|item| caption.starts_with(item))
        })
        .map(|(key, _)| key)
        .filter(|key| typed(key))
        .collect();
    // Item 17's TIN boxes print by name, not by a writer-cells join
    // (html-frozen/1701q-2018/index.html name="frm1701q:txtSpouseTIN1").
    keys.extend(
        ["TIN1", "TIN2", "TIN3"]
            .iter()
            .map(|part| format!("frm1701q:txtSpouse{part}")),
    );
    keys.sort();
    keys
}

/// A 1701Q return for a married Single Proprietor on graduated rates.
fn married(opened: &Value) -> Value {
    let filled = fill(
        Q,
        &opened["draft"],
        json!({
            rows_income_q::CIVIL_STATUS: "Married",
            "frm1701q:optType_1": "true",
            "frm1701q:optATC_1": "true",
            "frm1701q:optMethodOfDeduction:_2": "true",
        }),
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    filled["draft"].clone()
}

// ------------------------------------------------------------ 1701Q layout

/// R3.0, manual-0, manual-1, manual-2, and lesson 4: every numbered item
/// shows the number the sheet prints for its boxes, in paper order; the
/// quarters and item 25A read as printed; item 49 sits in Schedule II;
/// the page counter is not drawn; Line of Business and the contact number,
/// which this sheet does not print, are in Not on the printed sheet.
#[test]
fn form_1701q_items_follow_the_printed_sheet() {
    let printed = captions("1701q-2018");
    let mut checked = 0;
    for (part, item) in items(Q) {
        let no = item["no"].as_str().unwrap_or_default();
        for field in item["f"].as_array().into_iter().flatten() {
            let key = field["k"].as_str().unwrap();
            if field["t"] == "hidden" {
                continue;
            }
            let Some(caption) = printed.get(key) else {
                continue;
            };
            // The writer caption of item 15's two boxes reads "13/14
            // Claiming Foreign Tax Credits?"; the sheet prints the question
            // as item 15 (html-frozen/1701q-2018/index.html: "15 (if
            // applicable) Yes No").
            if key.starts_with("frm1701q:optForeignTaxCredits_") {
                assert_eq!(no, "15", "{key}");
                continue;
            }
            assert_eq!(caption_item(caption), no, "{part}: {key} ({caption})");
            checked += 1;
        }
    }
    assert!(checked > 130, "{checked} printed boxes checked");
    // Paper order: the numbered items run 1, 2, ... 68 down the page.
    let order: Vec<String> = items(Q)
        .into_iter()
        .filter_map(|(_, item)| {
            item["no"]
                .as_str()
                .filter(|n| !n.is_empty())
                .map(str::to_string)
        })
        .collect();
    let number = |no: &String| -> (u32, bool) {
        let digits: String = no.chars().take_while(char::is_ascii_digit).collect();
        (digits.parse().unwrap(), no.ends_with('A'))
    };
    assert!(
        order.windows(2).all(|w| number(&w[0]) < number(&w[1])),
        "{order:?}"
    );
    assert_eq!(order.first().map(String::as_str), Some("1"));
    assert_eq!(order.last().map(String::as_str), Some("68"));
    // R3.0: the quarters read as printed ("2 Quarter: First").
    for n in 1..=3 {
        let key = format!("frm1701q:DateQuarter_{n}");
        let caption = &printed[&key];
        assert_eq!(
            field_of(Q, &key)["l"],
            caption.strip_prefix("2 Quarter: ").unwrap(),
            "{key}"
        );
    }
    // manual-2: item 25A offers the printed methods, never Yes/No.
    for n in 1..=2 {
        let key = format!("frm1701q:optSpouseMethod:_{n}");
        let label = field_of(Q, &key)["l"].as_str().unwrap();
        let caption = printed[&key]
            .strip_prefix("25A Method of Deduction (spouse): ")
            .unwrap();
        assert!(label.starts_with(caption), "{key}: {label}");
    }
    // manual-1: item 49 is Schedule II's, between 48 and 50, and no item
    // is headed by the centavos note.
    let (part, item) = item_of(Q, "frm1701q:txt49A");
    assert!(part.contains("Schedule II"), "{part}");
    assert_eq!(item["no"], "49");
    let schedule_ii: Vec<&str> = items(Q)
        .into_iter()
        .filter(|(name, _)| name == &part)
        .filter_map(|(_, item)| item["no"].as_str())
        .collect();
    assert_eq!(
        schedule_ii,
        ["47", "48", "49", "50", "51", "52", "53", "54"]
    );
    assert!(
        items(Q)
            .iter()
            .all(|(_, item)| !item["l"].as_str().unwrap_or_default().contains("Centavos")),
        "the centavos note heads no item"
    );
    // manual-0: no page counter; what the sheet does not print is last.
    assert_eq!(field_of(Q, "frm1701q:txtCurrentPage")["t"], "hidden");
    for key in ["frm1701q:txtLOB", "frm1701q:txtTelno"] {
        let (part, item) = item_of(Q, key);
        assert_eq!(part, "Not on the printed sheet", "{key}");
        assert_eq!(item["no"], "", "{key}");
    }
    // The contact number is not on this sheet (writer-cells not_joined).
    let cells = repo_json("html-frozen/1701q-2018/writer-cells.json");
    assert!(cells["not_joined"].as_array().unwrap().iter().any(|entry| {
        entry["writer_key"] == "frm1701q:txtTelno"
            && entry["reason"]
                .as_str()
                .unwrap()
                .contains("no contact number box")
    }));
}

/// R3.1 (not a bug): Part IV's boxes are disabled in the official program
/// (rules corpus: "Statically disabled payment-detail field"), so the page
/// leaves them out and the print leaves them blank.
#[test]
fn form_1701q_part_iv_is_left_out_as_the_official_program_disables_it() {
    setup();
    let corpus = repo_json("rules/forms/1701q-v2018/fields.json");
    let keys = [
        "frm1701q:txtAgency32",
        "frm1701q:txtNumber32",
        "frm1701q:txtDate32",
        "frm1701q:txtAmount32",
        "frm1701q:txtAgency33",
        "frm1701q:txtNumber33",
        "frm1701q:txtDate33",
        "frm1701q:txtAmount33",
        "frm1701q:txtNumber34",
        "frm1701q:txtDate34",
        "frm1701q:txtAmount34",
        "frm1701q:txtParticular35",
        "frm1701q:txtAgency35",
        "frm1701q:txtNumber35",
        "frm1701q:txtDate35",
        "frm1701q:txtAmount35",
    ];
    let opened = open(Q, 1);
    let sheet = printed(Q, &opened["draft"]);
    for key in keys {
        let field = corpus["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["field_key"] == key)
            .unwrap_or_else(|| panic!("{key} in the corpus"));
        assert!(
            field["enabled_when"]
                .as_str()
                .unwrap()
                .starts_with("Statically disabled payment-detail field"),
            "{key}"
        );
        assert_eq!(field_of(Q, key)["t"], "hidden", "{key}");
        assert!(
            sheet
                .get(key)
                .is_none_or(|v| v.trim().is_empty() || v == "0.00"),
            "{key} prints {:?}",
            sheet.get(key)
        );
    }
}

/// manual-3: item 11 is one date on the page ("May 17, 1980"), stored and
/// filed as its three boxes.
#[test]
fn form_1701q_item_11_is_one_date_stored_as_three_boxes() {
    setup();
    let (_, item) = item_of(Q, "frm1701q:txtBirthMonth");
    assert_eq!(item["no"], "11");
    assert_eq!(item["one_date"], true);
    let keys: Vec<&str> = item["f"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["k"].as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        [
            "frm1701q:txtBirthMonth",
            "frm1701q:txtBirthDay",
            "frm1701q:txtBirthYear"
        ]
    );
    // No other form's item is marked but 1801's item 1, the date of death
    // (PR 10, rows_donor_estate::mark_schema).
    for kind in AgentFormKind::all() {
        if kind.form_code() != Q && kind.form_code() != "1801" {
            assert!(
                items(kind.form_code())
                    .iter()
                    .all(|(_, item)| item.get("one_date").is_none()),
                "{}",
                kind.form_code()
            );
        }
    }
    let opened = open(Q, 1);
    let filled = fill(
        Q,
        &opened["draft"],
        json!({
            "frm1701q:txtBirthMonth": "05",
            "frm1701q:txtBirthDay": "17",
            "frm1701q:txtBirthYear": "1980",
        }),
    );
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(filled["draft"]["date_of_birth"], "05/17/1980");
    let map = field_map(Q, &filled["draft"]);
    let sheet = printed(Q, &filled["draft"]);
    for (key, part) in [
        ("frm1701q:txtBirthMonth", "05"),
        ("frm1701q:txtBirthDay", "17"),
        ("frm1701q:txtBirthYear", "1980"),
    ] {
        assert_eq!(map[key], part, "{key} files");
        assert_eq!(sheet[key], part, "{key} prints");
    }
}

// --------------------------------------------------- 1701Q spouse (dec. 11)

/// Decision 11, R6.0, R6.3, R6.5-R6.7: the spouse part turns on from Civil
/// Status = Married. Not married, every box of Part II and column B is
/// faded with "Needs Civil Status = Married" and a write there is refused
/// in the filer's words; Married opens them, and what is typed files and
/// prints. An estate or trust reports no spouse.
#[test]
fn form_1701q_spouse_part_opens_from_civil_status_married() {
    setup();
    // The answer: the four civil statuses BIR prints on Form 1701 (item 16).
    let options: Vec<String> = captions("1701-2018")
        .values()
        .filter_map(|caption| caption.strip_prefix("16 Civil Status "))
        .map(str::to_string)
        .collect();
    let field = field_of(Q, rows_income_q::CIVIL_STATUS);
    let listed: Vec<String> = field["o"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        listed.iter().collect::<BTreeSet<_>>(),
        options.iter().collect::<BTreeSet<_>>()
    );
    let (part, item) = item_of(Q, rows_income_q::CIVIL_STATUS);
    assert!(part.starts_with("Part II"), "{part}");
    assert_eq!(item["no"], "", "the sheet prints no item for it");

    let opened = open(Q, 1);
    assert_eq!(opened["values"][rows_income_q::CIVIL_STATUS], "");
    let spouse = spouse_keys();
    assert_eq!(spouse.len(), 43, "{spouse:?}");
    for key in &spouse {
        assert_eq!(opened["off"][key], NEEDS_MARRIED, "{key}");
        assert!(opened["locks"][key].is_null(), "{key} is off, not locked");
    }
    for (key, value) in [
        ("frm1701q:txtSpouseName", "DELA CRUZ, MARIA"),
        ("frm1701q:txtSpouseTIN1", "123"),
        ("frm1701q:txt36B", "1000"),
        ("frm1701q:optSpouseType_1", "true"),
    ] {
        let refused = fill(Q, &opened["draft"], json!({ key: value }));
        assert_eq!(refused["rejected"][key], MARRIED_FIRST, "{key}");
    }

    // Married: Part II takes what is typed, and it files and prints.
    let draft = married(&opened);
    let filled = fill(
        Q,
        &draft,
        json!({
            "frm1701q:txtSpouseTIN1": "123",
            "frm1701q:txtSpouseTIN2": "456",
            "frm1701q:txtSpouseTIN3": "789",
            "frm1701q:txtSpouseBranchCode": "00000",
            "frm1701q:txtSpouseName": "DELA CRUZ, MARIA",
            "frm1701q:txtSpouseRDOCode": "040",
            "frm1701q:optSpouseType_2": "true",
            "frm1701q:optSpouseATC_2": "true",
        }),
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    assert_eq!(filled["values"][rows_income_q::CIVIL_STATUS], "Married");
    assert_eq!(filled["draft"]["spouse_tin"], "12345678900000");
    let checked = validated(Q, &filled["draft"]);
    for key in [
        "frm1701q:txtSpouseTIN1",
        "frm1701q:txtSpouseName",
        "frm1701q:optSpouseType_2",
    ] {
        assert!(checked["off"].get(key).is_none(), "{key} is on");
    }
    let map = field_map(Q, &filled["draft"]);
    let sheet = printed(Q, &filled["draft"]);
    for (key, value) in [
        ("frm1701q:txtSpouseTIN1", "123"),
        ("frm1701q:txtSpouseTIN2", "456"),
        ("frm1701q:txtSpouseTIN3", "789"),
        ("frm1701q:txtSpouseName", "DELA CRUZ, MARIA"),
        ("frm1701q:txtSpouseRDOCode", "040"),
        ("frm1701q:optSpouseType_2", "true"),
        ("frm1701q:optSpouseATC_2", "true"),
    ] {
        assert_eq!(filled["values"][key], value, "{key} shows");
        assert_eq!(map[key], value, "{key} files");
        assert_eq!(sheet[key], value, "{key} prints");
    }

    // An estate reports no spouse (the official disableSpouse).
    let estate = fill(Q, &filled["draft"], json!({"frm1701q:optType_3": "true"}));
    let checked = validated(Q, &estate["draft"]);
    assert_eq!(
        checked["off"]["frm1701q:txtSpouseName"],
        "Needs item 7 = Single Proprietor or Professional"
    );
    let refused = fill(Q, &estate["draft"], json!({"frm1701q:txtSpouseName": "X"}));
    assert!(
        refused["rejected"]["frm1701q:txtSpouseName"]
            .as_str()
            .unwrap()
            .contains("estate or trust"),
        "{}",
        refused["rejected"]
    );
}

/// Decision 11: turning Married off when the spouse part holds what the
/// filer typed asks first (the page's question with Undo: `removes`), and
/// then nothing of it files or prints. With nothing typed there, it asks
/// nothing.
#[test]
fn form_1701q_leaving_married_asks_first_and_clears_the_spouse_part() {
    setup();
    let opened = open(Q, 1);
    let draft = married(&opened);
    let quiet = fill(Q, &draft, json!({ rows_income_q::CIVIL_STATUS: "Single" }));
    assert!(quiet["removes"].is_null(), "nothing typed, nothing asked");
    let typed = fill(
        Q,
        &draft,
        json!({
            "frm1701q:txtSpouseName": "DELA CRUZ, MARIA",
            "frm1701q:optSpouseType_1": "true",
            "frm1701q:optSpouseATC_1": "true",
            "frm1701q:optSpouseMethod:_2": "true",
            "frm1701q:txt36B": "500000",
        }),
    );
    assert_eq!(typed["rejected"], json!({}), "{}", typed["rejected"]);
    assert_eq!(typed["values"]["frm1701q:txt36B"], "500,000.00");
    for answer in ["Single", "Legally Separated", "Widow/er", ""] {
        let left = fill(
            Q,
            &typed["draft"],
            json!({ rows_income_q::CIVIL_STATUS: answer }),
        );
        let removes = &left["removes"];
        assert!(
            removes["message"]
                .as_str()
                .is_some_and(|m| m.contains("Civil Status") && m.contains("Part II")),
            "{answer:?}: {removes}"
        );
        assert_eq!(removes["done"], "Cleared the spouse's part");
        assert_eq!(left["values"][rows_income_q::CIVIL_STATUS], answer);
        assert_eq!(left["draft"]["has_spouse"], false);
        let map = field_map(Q, &left["draft"]);
        let sheet = printed(Q, &left["draft"]);
        assert_eq!(map["frm1701q:txtSpouseName"], "");
        assert_eq!(sheet["frm1701q:txtSpouseName"], "");
        assert_eq!(map["frm1701q:txt36B"], "0.00");
        assert_eq!(map["frm1701q:txt26B"], "0.00");
        // Married again starts the spouse part empty.
        let again = fill(
            Q,
            &left["draft"],
            json!({ rows_income_q::CIVIL_STATUS: "Married" }),
        );
        assert_eq!(again["values"]["frm1701q:txtSpouseName"], "");
    }
}

/// Lesson 6 and the stored shape: a draft stored before Civil Status and
/// the penalty model existed opens exactly as stored (the whole draft), a
/// spouse means Married, none shows the part closed with what opens it, its
/// typed penalties file as they were (a zero too), and saving it again with
/// no edits writes the same stored JSON: no `civil_status`, `penalty` or
/// `spouse_penalty` key appears.
#[test]
fn form_1701q_stored_drafts_open_unchanged() {
    use bir_core::forms::form_1701q::{
        Form1701QAtc, Form1701QDeductionMethod, Form1701QDraft, Form1701QFilerType, Form1701QParty,
        Form1701QSpouseType,
    };
    setup();
    let stored_json = |quarter: u8| -> Value {
        with_database(|db| {
            db.get_form_draft::<Value>(TIN, Q, YEAR as u16, Some(quarter))
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
        .expect("stored")
    };
    let stored_typed = |quarter: u8| -> Form1701QDraft {
        with_database(|db| {
            db.get_queueable_draft::<Form1701QDraft>(TIN, YEAR as u16, i64::from(quarter))
                .map_err(|e| HostError::new(e.to_string()))
        })
        .unwrap()
        .expect("stored")
    };
    let store = |quarter: u8, spouse: bool| -> Value {
        let mut draft = Form1701QDraft {
            taxable_year: YEAR as u16,
            quarter,
            tin: TIN.to_string(),
            rdo_code: "039".into(),
            taxpayer_name: "DELA CRUZ, JUAN".into(),
            registered_address: "1 EXAMPLE STREET".into(),
            zip_code: "1100".into(),
            email: "books@example.com".into(),
            filer_type: Some(Form1701QFilerType::SingleProprietor),
            atc: Some(Form1701QAtc::Ii012),
            deduction_method: Some(Form1701QDeductionMethod::Osd),
            has_spouse: spouse,
            spouse_name: "DELA CRUZ, MARIA".into(),
            spouse_type: Some(Form1701QSpouseType::SingleProprietor),
            spouse_atc: Some(Form1701QAtc::Ii012),
            spouse_deduction_method: Some(Form1701QDeductionMethod::Osd),
            ..Default::default()
        };
        draft.set_amount(36, Form1701QParty::Taxpayer, Some(900_000.0));
        // Items 64-66 typed when they were plain boxes: a zero too.
        draft.set_amount(64, Form1701QParty::Taxpayer, Some(125.0));
        draft.set_amount(65, Form1701QParty::Taxpayer, Some(0.0));
        draft.set_amount(66, Form1701QParty::Taxpayer, Some(1_000.0));
        draft.recompute();
        assert_eq!(draft.penalty.penalty_model_version, 0);
        // Stored twice, so the stored JSON carries its own row id as a
        // saved draft does.
        let save = |draft: &Form1701QDraft| {
            with_database(|db| {
                db.save_queueable_draft(draft)
                    .map_err(|e| HostError::new(e.to_string()))
            })
            .unwrap()
        };
        draft.id = Some(save(&draft));
        save(&draft);
        let raw = stored_json(quarter);
        for key in ["civil_status", "penalty", "spouse_penalty"] {
            assert!(raw.get(key).is_none(), "the stored draft has no {key}");
        }
        raw
    };

    // The host bridge writes a whole amount as an integer (50500 for
    // 50500.0); the drafts compare as the typed drafts they are.
    let as_draft = |value: &Value| -> Form1701QDraft {
        serde_json::from_value(without_baseline(value)).unwrap()
    };
    let stored = store(2, true);
    let bytes = serde_json::to_string(&stored_typed(2)).unwrap();
    let opened = open(Q, 2);
    assert_eq!(
        as_draft(&opened["draft"]),
        as_draft(&stored),
        "the whole draft opens as stored"
    );
    assert_eq!(opened["values"][rows_income_q::CIVIL_STATUS], "Married");
    assert_eq!(
        opened["values"]["frm1701q:txtSpouseName"],
        "DELA CRUZ, MARIA"
    );
    // Its typed penalties file as they were, the zero too.
    for (key, shown) in [
        ("frm1701q:txt64A", "125.00"),
        ("frm1701q:txt65A", "0.00"),
        ("frm1701q:txt66A", "1,000.00"),
    ] {
        assert_eq!(opened["values"][key], shown, "{key}");
        assert_eq!(field_map(Q, &opened["draft"])[key], shown, "{key} files");
    }
    // Saved again with no edit: the same stored JSON (the store writes
    // serde_json::to_string of the typed draft, so equal typed drafts are
    // equal bytes).
    save(Q, &opened["draft"]);
    assert_eq!(without_baseline(&stored_json(2)), stored);
    assert_eq!(
        without_baseline(&serde_json::to_value(stored_typed(2)).unwrap()),
        serde_json::from_str::<Value>(&bytes).unwrap()
    );

    // Stored with no spouse (an older page wrote the name only): the part
    // is closed and says why; the stored name stays in the draft.
    let stored = store(3, false);
    let opened = open(Q, 3);
    assert_eq!(
        as_draft(&opened["draft"]),
        as_draft(&stored),
        "the whole draft opens as stored"
    );
    assert_eq!(opened["values"][rows_income_q::CIVIL_STATUS], "");
    assert_eq!(opened["off"]["frm1701q:txtSpouseName"], NEEDS_MARRIED);
    // Answering Single clears that hidden name: asked first (review 6).
    let single = fill(
        Q,
        &opened["draft"],
        json!({ rows_income_q::CIVIL_STATUS: "Single" }),
    );
    assert!(
        single["removes"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("Civil Status")),
        "{}",
        single["removes"]
    );
    assert_eq!(single["draft"]["spouse_name"], "");
    // Married keeps it, and shows it: nothing to ask.
    let married = fill(
        Q,
        &opened["draft"],
        json!({ rows_income_q::CIVIL_STATUS: "Married" }),
    );
    assert!(married["removes"].is_null());
    assert_eq!(
        married["values"]["frm1701q:txtSpouseName"],
        "DELA CRUZ, MARIA"
    );
}

/// R6.1, R6.2, R6.4, R6.8-R6.13: with the spouse part open, each conditional
/// box of column B follows the spouse's own answers (items 19, 20, 25,
/// 25A), and items 59A/59B follow Amended Return, each saying what turns it
/// on (the reviewed tags in app/forms/condition-labels.json, by hand here).
#[test]
fn form_1701q_conditional_boxes_say_what_turns_them_on() {
    setup();
    let opened = open(Q, 1);
    let base = married(&opened);
    let state = |edits: Value| -> Value {
        let filled = fill(Q, &base, edits);
        assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
        validated(Q, &filled["draft"])
    };
    let off = |checked: &Value, key: &str| checked["off"][key].as_str().map(str::to_string);
    // Spouse on graduated rates, itemized.
    let graduated = state(json!({
        "frm1701q:optSpouseType_1": "true",
        "frm1701q:optSpouseATC_1": "true",
        "frm1701q:optSpouseMethod:_1": "true",
    }));
    for key in [36, 37, 39, 42, 43, 44, 55, 59, 64].map(|n| format!("frm1701q:txt{n}B")) {
        if key == "frm1701q:txt59B" {
            assert_eq!(
                off(&graduated, &key).as_deref(),
                Some("Needs Amended Return = Yes")
            );
        } else {
            assert_eq!(off(&graduated, &key), None, "{key}");
        }
    }
    for key in ["frm1701q:txt47B", "frm1701q:txt48B", "frm1701q:txt50B"] {
        assert_eq!(
            off(&graduated, key).as_deref(),
            Some("Needs item 25 = 8%"),
            "{key}"
        );
    }
    // OSD: items 37B and 39B wait for itemized deductions.
    let osd = state(json!({
        "frm1701q:optSpouseType_1": "true",
        "frm1701q:optSpouseATC_1": "true",
        "frm1701q:optSpouseMethod:_2": "true",
    }));
    for key in ["frm1701q:txt37B", "frm1701q:txt39B"] {
        assert_eq!(
            off(&osd, key).as_deref(),
            Some("Needs item 25 = Graduated Rates and item 25A = Itemized Deduction"),
            "{key}"
        );
    }
    // Spouse on 8%: Schedule II opens, Schedule I and 25A close.
    let eight = state(json!({
        "frm1701q:optSpouseType_1": "true",
        "frm1701q:optSpouseATC_5": "true",
    }));
    for key in ["frm1701q:txt47B", "frm1701q:txt52B"] {
        assert_eq!(off(&eight, key), None, "{key}");
    }
    assert_eq!(
        off(&eight, "frm1701q:txt36B").as_deref(),
        Some("Needs item 25 = Graduated Rates")
    );
    assert_eq!(
        off(&eight, "frm1701q:optSpouseMethod:_1").as_deref(),
        Some("Needs item 25 = Graduated Rates")
    );
    // Mixed income on 8% (II016): item 52B waits.
    let mixed = state(json!({
        "frm1701q:optSpouseType_1": "true",
        "frm1701q:optSpouseATC_7": "true",
    }));
    assert_eq!(
        off(&mixed, "frm1701q:txt52B").as_deref(),
        Some("Needs item 25 = 8% and item 20 other than II016")
    );
    // A compensation earner only: the spouse files no schedule.
    let earner = state(json!({"frm1701q:optSpouseType_3": "true"}));
    for key in [
        "frm1701q:txt55B",
        "frm1701q:txt64B",
        "frm1701q:optSpouseTaxRate_1",
    ] {
        assert_eq!(
            off(&earner, key).as_deref(),
            Some("Needs item 20 other than II011 Compensation Income"),
            "{key}"
        );
    }
    assert_eq!(
        off(&earner, "frm1701q:optSpouseATC_1").as_deref(),
        Some("Needs item 19 = Single Proprietor or Professional")
    );
    // Amended: items 59A and 59B open.
    let amended = state(json!({
        "frm1701q:optSpouseType_1": "true",
        "frm1701q:optSpouseATC_1": "true",
        "frm1701q:AmendedRtn_1": "true",
    }));
    for key in ["frm1701q:txt59A", "frm1701q:txt59B"] {
        assert_eq!(off(&amended, key), None, "{key}");
    }
    // The taxpayer's own column: 52A waits for an ATC other than II016.
    let taxpayer = state(json!({"frm1701q:optATC_6": "true"}));
    assert_eq!(
        off(&taxpayer, "frm1701q:txt52A").as_deref(),
        Some("Needs the 8% income tax rate and an ATC other than II016")
    );
    assert_eq!(
        off(&taxpayer, "frm1701q:txt43Desc").as_deref(),
        Some("Needs the graduated income tax rate")
    );
    assert_eq!(off(&taxpayer, "frm1701q:txt48Desc"), None);
}

/// Lesson 8: an answer that clears amounts typed elsewhere (an ATC that
/// changes the rate, Amended Return No) asks first, naming the printed
/// items it clears.
#[test]
fn form_1701q_an_answer_that_clears_typed_amounts_asks_first() {
    setup();
    let opened = open(Q, 1);
    let draft = married(&opened);
    let typed = fill(
        Q,
        &draft,
        json!({"frm1701q:txt36A": "800000", "frm1701q:AmendedRtn_1": "true", "frm1701q:txt59A": "2500"}),
    );
    assert_eq!(typed["rejected"], json!({}), "{}", typed["rejected"]);
    let eight = fill(Q, &typed["draft"], json!({"frm1701q:optATC_4": "true"}));
    assert_eq!(
        eight["removes"]["message"],
        "This answer clears what you typed in item 36A. Continue?"
    );
    assert_eq!(eight["values"]["frm1701q:txt36A"], "0.00");
    let not_amended = fill(Q, &typed["draft"], json!({"frm1701q:AmendedRtn_2": "true"}));
    assert_eq!(
        not_amended["removes"]["message"],
        "This answer clears what you typed in item 59A. Continue?"
    );
    // Typing an amount clears nothing else: nothing to ask.
    let more = fill(Q, &typed["draft"], json!({"frm1701q:txt42A": "1000"}));
    assert!(more["removes"].is_null(), "{}", more["removes"]);
}

/// R8.0: items 64-66 show what the shared penalty model computes from
/// item 63's tax still due (surcharge 25%, interest 12% a year), locked
/// until Edit penalties; a typed amount files instead, and items 67, 29, 30
/// and 31 add up what is filed.
#[test]
fn form_1701q_penalties_are_computed_and_a_typed_one_overrides() {
    setup();
    let opened = open(Q, 1);
    let filled = fill(
        Q,
        &opened["draft"],
        json!({
            "frm1701q:optType_1": "true",
            "frm1701q:optATC_1": "true",
            "frm1701q:optMethodOfDeduction:_2": "true",
            "frm1701q:txt36A": "2000000",
        }),
    );
    let draft = &filled["draft"];
    let inputs = &draft["penalty"]["computed_penalties"]["inputs"];
    let due = amount(&filled["values"], "frm1701q:txt63A");
    assert!(due > 0.0);
    let days = inputs["days_late"].as_f64().unwrap();
    assert!(days > 0.0, "Q1 {YEAR} was due on 15 May {YEAR}");
    assert_eq!(inputs["due_date"], format!("{YEAR}-05-15"));
    let surcharge = amount(&filled["values"], "frm1701q:txt64A");
    let interest = amount(&filled["values"], "frm1701q:txt65A");
    assert_eq!(surcharge, (due * 0.25).round());
    assert_eq!(interest, (due * 0.12 * days / 365.0).round());
    let compromise = amount(&filled["values"], "frm1701q:txt66A");
    assert_eq!(
        amount(&filled["values"], "frm1701q:txt67A"),
        surcharge + interest + compromise
    );
    let checked = validated(Q, draft);
    let group = checked["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["keys"][0] == "frm1701q:txt64A")
        .unwrap();
    assert_eq!(group["applies"], true);
    assert_eq!(group["edited"], false);
    // Not edited, a typed amount is refused; with Edit it files.
    let refused = fill(Q, draft, json!({"frm1701q:txt64A": "500"}));
    assert!(refused["rejected"]["frm1701q:txt64A"].is_string());
    let edited = fill_unlocked(
        Q,
        draft,
        json!({"frm1701q:txt64A": "500"}),
        &["frm1701q:txt64A", "frm1701q:txt65A", "frm1701q:txt66A"],
    );
    assert_eq!(edited["rejected"], json!({}));
    let values = &edited["values"];
    assert_eq!(amount(values, "frm1701q:txt64A"), 500.0);
    assert_eq!(
        amount(values, "frm1701q:txt65A"),
        interest,
        "the others keep computing"
    );
    let total = 500.0 + interest + compromise;
    for key in ["frm1701q:txt67A", "frm1701q:txt29A"] {
        assert_eq!(amount(values, key), total, "{key}");
    }
    assert_eq!(
        amount(values, "frm1701q:txt31"),
        amount(values, "frm1701q:txt63A") + total
    );
    assert_eq!(field_map(Q, &edited["draft"])["frm1701q:txt64A"], "500.00");
}

/// preview.0: the boxes the probe called app constants are the draft's:
/// the quarter ticks follow the return's own quarter, and columns A and B
/// of Schedules I and II and item 25A show on the page what the print
/// fills, for every quarter and both rates.
#[test]
fn form_1701q_page_and_print_agree_on_the_boxes_called_constants() {
    setup();
    for quarter in 1..=3 {
        let opened = open(Q, quarter);
        let sheet = printed(Q, &opened["draft"]);
        for n in 1..=3 {
            let key = format!("frm1701q:DateQuarter_{n}");
            let expected = if n == quarter { "true" } else { "false" };
            assert_eq!(opened["values"][&key], expected, "Q{quarter} {key}");
            assert_eq!(sheet[&key], expected, "Q{quarter} {key} prints");
        }
    }
    let opened = open(Q, 2);
    let draft = married(&opened);
    // Taxpayer on 8%, spouse on graduated rates, itemized.
    let filled = fill(
        Q,
        &draft,
        json!({
            "frm1701q:optATC_4": "true",
            "frm1701q:optSpouseType_1": "true",
            "frm1701q:optSpouseATC_1": "true",
            "frm1701q:optSpouseMethod:_1": "true",
        }),
    );
    let filled = fill(
        Q,
        &filled["draft"],
        json!({
            "frm1701q:txt47A": "900000",
            "frm1701q:txt48A": "1000",
            "frm1701q:txt50A": "15000",
            "frm1701q:txt52A": "250000",
            "frm1701q:txt36B": "1234567",
            "frm1701q:txt37B": "345678",
            "frm1701q:txt39B": "123456",
            "frm1701q:txt42B": "1000",
            "frm1701q:txt43B": "50000",
            "frm1701q:txt44B": "7000",
        }),
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    let sheet = printed(Q, &filled["draft"]);
    for key in [
        "frm1701q:txt36B",
        "frm1701q:txt37B",
        "frm1701q:txt39B",
        "frm1701q:txt42B",
        "frm1701q:txt43B",
        "frm1701q:txt44B",
        "frm1701q:txt47A",
        "frm1701q:txt48A",
        "frm1701q:txt50A",
        "frm1701q:txt52A",
        "frm1701q:optSpouseMethod:_1",
        "frm1701q:optSpouseMethod:_2",
    ] {
        let page = filled["values"][key].as_str().unwrap();
        assert!(
            !page.is_empty() && page != "0.00" || key.ends_with(":_2"),
            "{key} = {page}"
        );
        assert_eq!(
            sheet[key], page,
            "{key}: the print fills what the page shows"
        );
    }
    assert_eq!(filled["values"]["frm1701q:optSpouseMethod:_1"], "true");
}

// --------------------------------------------------------------- 1702Q

/// R3.1, R3.2, manual-0, manual-1, manual-2: the page follows the 1702Q
/// sheet. Each item shows the number its boxes print under (page 2's
/// schedules number their own items), Schedules 1-4 are parts of their
/// own, item 13 reads Yes/No, item 2 and item 5 read as printed, and the
/// page 2 header copy, the page counter and the row checkboxes of the
/// other-credits list are not drawn.
#[test]
fn form_1702q_items_follow_the_printed_sheet() {
    let printed = captions("1702q-2018");
    let caption = |key: &str| {
        printed
            .get(key)
            .or_else(|| printed.get(&key.replacen("frm1702q:", "derived:", 1)))
            .cloned()
    };
    let mut checked = 0;
    for (part, item) in items(C) {
        let no = item["no"].as_str().unwrap_or_default();
        // A badge never overflows: printed numbers are short.
        assert!(no.len() <= 3, "{part}: {no:?}");
        for field in item["f"].as_array().into_iter().flatten() {
            let key = field["k"].as_str().unwrap();
            if field["t"] == "hidden" {
                continue;
            }
            let Some(caption) = caption(key) else {
                continue;
            };
            // The writer caption reads "13 If yes, specify"; the sheet
            // prints that box as 13A (html-frozen/1702q-2018/index.html:
            // "If yes, ... 13 13A ... specify").
            if key == "frm1702q:txtTxRlfSpcfy" {
                assert_eq!(no, "13A");
                continue;
            }
            assert_eq!(caption_item(&caption), no, "{part}: {key} ({caption})");
            if let Some(schedule) = caption.strip_prefix("Schedule ") {
                let n = &schedule[..1];
                assert!(part.contains(&format!("Schedule {n} ")), "{part}: {key}");
            }
            checked += 1;
        }
    }
    assert!(checked > 70, "{checked}");
    for (key, label) in [
        ("frm1702q:rbTxRlf_1", "Yes"),
        ("frm1702q:rbTxRlf_2", "No"),
        (
            "frm1702q:rbATC_1",
            "IC 055 Minimum Corporate Income Tax (MCIT)",
        ),
        ("frm1702q:rbATC_2", "Other ATC"),
        ("frm1702q:rbQuarter_1", "1st"),
    ] {
        assert_eq!(field_of(C, key)["l"], label, "{key}");
        if let Some(caption) = caption(key) {
            assert!(
                caption.ends_with(label) || key.contains("rbATC_2"),
                "{key}: {caption}"
            );
        }
    }
    assert_eq!(
        item_of(C, "frm1702q:rbYrEndMonth").1["l"],
        "Year Ended (MM/20YY)"
    );
    assert_eq!(
        item_of(C, "frm1702q:cbATC_2").1["l"],
        "Alphanumeric Tax Code (ATC)"
    );
    for key in [
        "frm1702q:Sched4:chkOthrTxCrdts0",
        "frm1702q:Sched4:chkOthrTxCrdts1",
        "frm1702q:txtPg2TIN1",
        "frm1702q:txtPg2TIN2",
        "frm1702q:txtPg2TIN3",
        "frm1702q:txtPg2BranchCode",
        "frm1702q:txtPg2TaxpayerName",
        "frm1702q:txtCurrentPage",
        "resultOtherSpecify",
    ] {
        assert_eq!(field_of(C, key)["t"], "hidden", "{key}");
    }
    // Schedule 4 ends with its own total, item 7, after rows a and b.
    let (part, _) = item_of(C, "frm1702q:Sched4:txtTax7");
    let numbers: Vec<&str> = items(C)
        .into_iter()
        .filter(|(name, _)| name == &part)
        .filter_map(|(_, item)| item["no"].as_str())
        .collect();
    assert_eq!(numbers, ["1", "2", "3", "4", "5", "6", "6a", "6b", "7"]);
    assert_eq!(
        field_of(C, "frm1702q:Sched4:txtOthrTxCrdts0")["l"],
        "Specify"
    );
}

/// The schedules take their entries once item 5 has an ATC, and say so
/// before; a schedule entry files and prints; clearing the ATC asks first.
/// The other-credits rows a and b, and item 10's contact number, are the
/// filer's boxes (they were pinned locked and blank, PR 5).
#[test]
fn form_1702q_schedules_take_their_entries_once_item_5_has_an_atc() {
    setup();
    let opened = open(C, 2);
    for key in ["frm1702q:Sched2:txtTax1", "frm1702q:Sched4:txtOthrTxCrdts0"] {
        assert_eq!(opened["off"][key], "Needs item 5 = an ATC", "{key}");
        assert!(opened["locks"][key].is_null(), "{key}");
    }
    let with_atc = fill(
        C,
        &opened["draft"],
        json!({"frm1702q:cbATC_2": "IC010_25%", "frm1702q:rbATC_2": "true"}),
    );
    assert_eq!(with_atc["rejected"], json!({}), "{}", with_atc["rejected"]);
    let filled = fill(
        C,
        &with_atc["draft"],
        json!({
            "frm1702q:Sched2:txtTax1": "1000000",
            "frm1702q:Sched2:txtTax2": "400000",
            "frm1702q:Sched4:txtOthrTxCrdts0": "SAMPLE CREDIT",
            "frm1702q:Sched4:txtOthrTxCrdtAmnt0": "1234",
            "frm1702q:Sched4:txtTax2": "5000",
        }),
    );
    assert_eq!(filled["rejected"], json!({}), "{}", filled["rejected"]);
    let values = &filled["values"];
    assert_eq!(
        amount(values, "frm1702q:Sched2:txtTax3"),
        600_000.0,
        "item 3 = 1 less 2"
    );
    let checked = validated(C, &filled["draft"]);
    for key in ["frm1702q:Sched2:txtTax1", "frm1702q:Sched4:txtOthrTxCrdts0"] {
        assert!(checked["off"].get(key).is_none(), "{key} is on");
    }
    let map = field_map(C, &filled["draft"]);
    let sheet = printed(C, &filled["draft"]);
    assert_eq!(map["frm1702q:Sched4:txtOthrTxCrdts0"], "SAMPLE CREDIT");
    assert_eq!(sheet["frm1702q:Sched4:txtOthrTxCrdts0"], "SAMPLE CREDIT");
    assert_eq!(map["frm1702q:Sched2:txtTax1"], "1,000,000.00");
    assert_eq!(amount(values, "frm1702q:Sched4:txtTax7"), 6_234.0);
    // A first-quarter return has no previous quarter: item 2 waits.
    let first = open(C, 1);
    let first = fill(
        C,
        &first["draft"],
        json!({"frm1702q:cbATC_2": "IC010_25%", "frm1702q:rbATC_2": "true"}),
    );
    let checked = validated(C, &first["draft"]);
    assert_eq!(
        checked["off"]["frm1702q:Sched4:txtTax2"],
        "Needs item 5 = an ATC, on a 2nd or 3rd quarter return"
    );
    // Clearing the ATC clears the schedules: asked first.
    let cleared = fill(C, &filled["draft"], json!({"frm1702q:rbATC_2": "false"}));
    let message = cleared["removes"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("Schedule 2 item 1") && message.ends_with("Continue?"),
        "{}",
        cleared["removes"]
    );
}

/// Item 10, the contact number, is the profile's box: tucked with Edit like
/// the other profile boxes, and an edit files once under txtTelNum and
/// prints at item 10.
#[test]
fn form_1702q_item_10_contact_number_is_a_profile_box() {
    setup();
    let key = "frm1702q:txtTelNum#occurrence-1";
    let opened = open(C, 1);
    assert_eq!(opened["values"][key], "0281234567");
    assert_eq!(opened["locks"][key], agent::Lock::Profile.reason());
    assert!(strings(&opened["profile_edit"]).contains(key));
    let refused = fill(C, &opened["draft"], json!({ key: "09171234567" }));
    assert!(
        refused["rejected"][key].is_string(),
        "the profile's until Edit"
    );
    let edited = fill_unlocked(C, &opened["draft"], json!({ key: "09171234567" }), &[key]);
    assert_eq!(edited["rejected"], json!({}));
    assert_eq!(edited["draft"]["contact_number"], "09171234567");
    assert_eq!(edited["values"][key], "09171234567");
    assert_eq!(
        field_map(C, &edited["draft"])["frm1702q:txtTelNum"],
        "09171234567"
    );
    assert_eq!(
        printed(C, &edited["draft"])["frm1702q:txtTelNum"],
        "09171234567"
    );
    // Its caption on the sheet.
    assert_eq!(
        captions("1702q-2018")["frm1702q:txtTelNum"],
        "10 Contact Number"
    );
}

/// preview.0 (not a bug): 1702Q's quarter boxes are the return's own
/// quarter, ticked from the draft on the page and the print alike.
#[test]
fn form_1702q_quarter_boxes_agree_with_the_print() {
    setup();
    for quarter in 1..=3 {
        let opened = open(C, quarter);
        let sheet = printed(C, &opened["draft"]);
        for n in 1..=3 {
            let key = format!("frm1702q:rbQuarter_{n}");
            let expected = if n == quarter { "true" } else { "false" };
            assert_eq!(opened["values"][&key], expected, "Q{quarter} {key}");
            assert_eq!(sheet[&key], expected, "Q{quarter} {key} prints");
        }
    }
}

/// The refusals in the filer's words, asked of the host directly: items 43
/// and 48's "(specify)" boxes while no party files their schedule, the
/// spouse part's radio groups and a spouse penalty (even opened with Edit)
/// while Part II is closed, and the spouse's penalties for a compensation
/// earner only (no Edit, faded, refused).
#[test]
fn form_1701q_closed_boxes_refuse_a_direct_write_in_the_filer_words() {
    setup();
    let opened = open(Q, 1);
    let refused = |draft: &Value, key: &str, value: &str, unlocked: &[&str]| -> Value {
        let filled = fill_unlocked(Q, draft, json!({ key: value }), unlocked);
        filled["rejected"][key].clone()
    };
    assert_eq!(
        refused(&opened["draft"], "frm1701q:txt43Desc", "RENTAL", &[]),
        "Item 43 is Schedule I's: choose Graduated Rates at item 16 (or item 25 for the spouse) first."
    );
    assert_eq!(
        refused(&opened["draft"], "frm1701q:txt48Desc", "INTEREST", &[]),
        "Item 48 is Schedule II's: choose the 8% rate at item 16 (or item 25 for the spouse) first."
    );
    for (key, value) in [
        ("frm1701q:optSpouseATC", "II012"),
        ("frm1701q:optSpouseType", "Single"),
        ("frm1701q:optSpouseTaxRate", "Graduated"),
        ("frm1701q:optSpouseMethod", "I"),
        ("frm1701q:optSpouseForeignTaxCred", "Y"),
    ] {
        assert_eq!(
            refused(&opened["draft"], key, value, &[]),
            MARRIED_FIRST,
            "{key}"
        );
    }
    let penalties = ["frm1701q:txt64B", "frm1701q:txt65B", "frm1701q:txt66B"];
    assert_eq!(
        refused(&opened["draft"], "frm1701q:txt64B", "100", &penalties),
        MARRIED_FIRST,
        "a closed spouse penalty, even opened"
    );
    // Graduated rates open item 43's box.
    let graduated = married(&opened);
    let filled = fill(Q, &graduated, json!({"frm1701q:txt43Desc": "RENTAL"}));
    assert_eq!(filled["rejected"], json!({}));
    assert_eq!(
        field_map(Q, &filled["draft"])["frm1701q:txt43Desc"],
        "RENTAL"
    );

    // A compensation earner only: the spouse's column files no penalties.
    let earner = fill(Q, &graduated, json!({"frm1701q:optSpouseType_3": "true"}));
    let checked = validated(Q, &earner["draft"]);
    let group = checked["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["keys"][0] == "frm1701q:txt64B")
        .unwrap();
    assert_eq!(group["applies"], false, "no Edit penalties");
    for key in penalties {
        assert_eq!(
            checked["off"][key], "Needs item 20 other than II011 Compensation Income",
            "{key}"
        );
        assert_eq!(
            refused(&earner["draft"], key, "100", &penalties),
            "A spouse who is a compensation earner only (item 20 II011) files no penalties here: choose the spouse's other ATC at item 20 first.",
            "{key}"
        );
        assert_eq!(field_map(Q, &earner["draft"])[key], "0.00");
    }
}

/// Review 5: the spouse's answers (items 19, 20, 25, 25A) count in what
/// leaving Married removes, with no text typed.
#[test]
fn form_1701q_leaving_married_counts_the_spouse_answers() {
    setup();
    let opened = open(Q, 1);
    let answered = fill(
        Q,
        &married(&opened),
        json!({
            "frm1701q:optSpouseType_1": "true",
            "frm1701q:optSpouseATC_1": "true",
            "frm1701q:optSpouseMethod:_1": "true",
        }),
    );
    assert_eq!(answered["rejected"], json!({}));
    let left = fill(
        Q,
        &answered["draft"],
        json!({ rows_income_q::CIVIL_STATUS: "Single" }),
    );
    // Item 19, item 20 and item 25A, which the filer chose; item 25 is the
    // recompute's own (the ATC sets it), so it is not counted.
    assert_eq!(left["removes"]["rows"], 3, "{}", left["removes"]);
    for key in [
        "spouse_type",
        "spouse_atc",
        "spouse_tax_rate",
        "spouse_deduction_method",
    ] {
        assert!(left["draft"][key].is_null(), "{key}");
    }
}

/// Review 1: an answer that replaces a typed amount with a computed one
/// asks first. 1702Q item 12 OSD replaces Schedule 2 item 6 (the typed
/// deductions) with 40% of the gross income.
#[test]
fn form_1702q_osd_asks_before_it_replaces_typed_deductions() {
    setup();
    let opened = open(C, 2);
    let typed = fill(
        C,
        &opened["draft"],
        json!({
            "frm1702q:cbATC_2": "IC010_25%",
            "frm1702q:rbATC_2": "true",
            "frm1702q:rbMthdOfDdctns_1": "true",
        }),
    );
    let typed = fill(
        C,
        &typed["draft"],
        json!({"frm1702q:Sched2:txtTax1": "1000000", "frm1702q:Sched2:txtTax6": "50000"}),
    );
    assert_eq!(typed["rejected"], json!({}), "{}", typed["rejected"]);
    assert_eq!(
        amount(&typed["values"], "frm1702q:Sched2:txtTax6"),
        50_000.0
    );
    let osd = fill(
        C,
        &typed["draft"],
        json!({"frm1702q:rbMthdOfDdctns_2": "true", "frm1702q:rbMthdOfDdctns_1": "false"}),
    );
    assert_eq!(
        amount(&osd["values"], "frm1702q:Sched2:txtTax6"),
        400_000.0,
        "40% of gross"
    );
    assert_eq!(
        osd["removes"]["message"],
        "This answer replaces what you typed in Schedule 2 item 6. Continue?"
    );
    assert_eq!(osd["removes"]["rows"], 1);
    // Undo refills the draft from before the answer (form_page.js
    // undoRemoval): item 6 is 50,000 again, exactly.
    let undone = fill(C, &typed["draft"], json!({}));
    assert_eq!(undone["values"]["frm1702q:Sched2:txtTax6"], "50,000.00");
    assert_eq!(undone["draft"]["deductions"], typed["draft"]["deductions"]);
    // Typing another amount under OSD replaces nothing typed: no question.
    let more = fill(
        C,
        &osd["draft"],
        json!({"frm1702q:Sched2:txtTax1": "2000000"}),
    );
    assert!(more["removes"].is_null(), "{}", more["removes"]);
}

/// Review 3: a 1702Q box that waits for its gate refuses a write, with the
/// tag rule 6 shows (app/forms/condition-labels.json, for the condition
/// app/forms/conditions.json gives the box), instead of taking it and
/// losing it to the recompute.
#[test]
fn form_1702q_a_gated_box_refuses_a_write_with_what_it_needs() {
    setup();
    let labels = repo_json("app/forms/condition-labels.json");
    let conditions = repo_json("app/forms/conditions.json");
    let tag = |key: &str| -> String {
        let condition = conditions[C][key].as_str().unwrap();
        labels["conditions"][C][condition]
            .as_str()
            .unwrap()
            .to_string()
    };
    let opened = open(C, 1);
    for key in [
        "frm1702q:Sched2:txtTax1",
        "frm1702q:Sched1:txtTax1B",
        "frm1702q:Sched4:txtOthrTxCrdts0",
        "frm1702q:Sched3:txtTax1",
        "frm1702q:txtTxRlfSpcfy",
        "frm1702q:txtTax15",
    ] {
        let filled = fill(C, &opened["draft"], json!({ key: "1000" }));
        assert_eq!(filled["rejected"][key], tag(key), "{key}");
        assert_eq!(
            opened["off"][key],
            tag(key),
            "{key} is faded with the same words"
        );
    }
    assert_eq!(tag("frm1702q:Sched2:txtTax1"), "Needs item 5 = an ATC");
    // With an ATC on a first-quarter return, the previous quarters still wait.
    let with_atc = fill(
        C,
        &opened["draft"],
        json!({"frm1702q:cbATC_2": "IC010_25%", "frm1702q:rbATC_2": "true", "frm1702q:rbMthdOfDdctns_2": "true"}),
    );
    for key in [
        "frm1702q:Sched4:txtTax2",
        "frm1702q:Sched2:txtTax6",
        "frm1702q:Sched4:txtTax6",
    ] {
        let filled = fill(C, &with_atc["draft"], json!({ key: "1000" }));
        assert_eq!(filled["rejected"][key], tag(key), "{key}");
    }
    let filled = fill(
        C,
        &with_atc["draft"],
        json!({"frm1702q:Sched2:txtTax1": "1000"}),
    );
    assert_eq!(filled["rejected"], json!({}), "open once its gate holds");
    // Clearing a gated box (0, as an amount box takes it) is taken: it
    // files nothing either way.
    let cleared = fill(C, &opened["draft"], json!({"frm1702q:Sched2:txtTax1": "0"}));
    assert_eq!(cleared["rejected"], json!({}));
}

/// The stored JSON of a 1701Q draft saved before this review (faf62fe4's
/// serializer wrote `penalty` and `spouse_penalty` on every save, at their
/// default for a column it never computed; the struct's field order is the
/// same), with typed items 64A-66A of 125.00, 0.00 and 1,000.00.
const BASE_ERA: &str = include_str!("fixtures/1701q-base-era.json");

/// Review round 2 (F4): that row opens and saves back byte-identically:
/// the keys it had stay, at their default, and nothing else moves.
#[test]
fn form_1701q_a_base_era_row_saves_back_byte_identically() {
    use bir_core::forms::form_1701q::Form1701QDraft;
    setup();
    let fixture = BASE_ERA.trim_end();
    let typed: Form1701QDraft = serde_json::from_str(fixture).unwrap();
    assert_eq!(
        serde_json::to_string(&typed).unwrap(),
        fixture,
        "reads and writes back as stored"
    );
    let id = with_database(|db| {
        db.save_queueable_draft(&typed)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap();
    assert_eq!(Some(id), typed.id, "the fixture's own row id");
    let opened = open(Q, 1);
    for (key, shown) in [
        ("frm1701q:txt64A", "125.00"),
        ("frm1701q:txt65A", "0.00"),
        ("frm1701q:txt66A", "1,000.00"),
    ] {
        assert_eq!(opened["values"][key], shown, "{key} files as stored");
    }
    save(Q, &opened["draft"]);
    let stored = with_database(|db| {
        db.get_queueable_draft::<Form1701QDraft>(TIN, 2026, 1)
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    let saved = serde_json::to_value(&stored).unwrap();
    assert!(
        saved["profile_baseline"].is_object(),
        "saving records the profile the draft was built from (PR 0c)"
    );
    assert_eq!(
        without_baseline(&saved),
        serde_json::from_str::<Value>(fixture).unwrap(),
        "every other byte as stored"
    );
    let raw = with_database(|db| {
        db.get_form_draft::<Value>(TIN, Q, 2026, Some(1))
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        without_baseline(&raw),
        serde_json::from_str::<Value>(fixture).unwrap()
    );
    assert_eq!(
        raw["penalty"],
        json!({"penalty_model_version": 0, "penalty_overrides": {}})
    );
}

/// A stored 1701Q draft without PR 0c's `profile_baseline` (the profile
/// values the draft was built from, recorded by the app when the draft is
/// saved; never filed or printed). The integration merge of PR 0c and PR 5
/// keeps every other byte of a stored draft unchanged.
fn without_baseline(value: &Value) -> Value {
    let mut value = value.clone();
    if let Some(map) = value.as_object_mut() {
        map.remove("profile_baseline");
    }
    value
}

/// Review round 2 (F3): a column still on the filer's typed amounts says so
/// ("kept as entered ..."), and "Use the computed amounts" (blanks with the
/// boxes opened) gives the computed amounts at once.
#[test]
fn form_1701q_penalties_kept_as_entered_say_so_and_go_computed_in_one_step() {
    use bir_core::forms::form_1701q::Form1701QDraft;
    setup();
    let typed: Form1701QDraft = serde_json::from_str(BASE_ERA.trim_end()).unwrap();
    with_database(|db| {
        db.save_queueable_draft(&typed)
            .map(|_| ())
            .map_err(|e| HostError::new(e.to_string()))
    })
    .unwrap();
    let opened = open(Q, 1);
    let keys = ["frm1701q:txt64A", "frm1701q:txt65A", "frm1701q:txt66A"];
    for key in keys {
        assert_eq!(
            opened["locks"][key],
            "kept as entered before penalties were computed; choose Edit penalties to change them.",
            "{key}"
        );
        assert_eq!(
            opened["lock_notes"][key],
            "Kept as entered before penalties were computed."
        );
    }
    let group = opened["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["keys"][0] == keys[0])
        .unwrap();
    assert_eq!(group["kept"], true);
    assert_eq!(group["edited"], false);
    let refused = fill(Q, &opened["draft"], json!({ keys[0]: "500" }));
    assert_eq!(
        refused["rejected"][keys[0]],
        "`frm1701q:txt64A` is kept as entered before penalties were computed; choose Edit penalties to change them."
    );
    // Use the computed amounts: the three blanks, the boxes opened.
    let computed = fill_unlocked(
        Q,
        &opened["draft"],
        json!({ keys[0]: "", keys[1]: "", keys[2]: "" }),
        &keys,
    );
    assert_eq!(computed["rejected"], json!({}));
    assert_eq!(computed["draft"]["penalty"]["penalty_model_version"], 1);
    let due = amount(&computed["values"], "frm1701q:txt63A");
    assert_eq!(
        amount(&computed["values"], keys[0]),
        (due * 0.25).round(),
        "the computed surcharge"
    );
    let group = computed["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["keys"][0] == keys[0])
        .unwrap();
    assert_eq!(group["kept"], false);
}

/// Review round 2 (F1): leaving Married counts the spouse's typed penalty
/// amounts of a column still on its typed boxes: a stored draft whose only
/// spouse entry is 64B = 500 asks before it goes.
#[test]
fn form_1701q_leaving_married_counts_a_typed_spouse_penalty() {
    use bir_core::forms::form_1701q::{Form1701QDraft, Form1701QParty};
    setup();
    let mut legacy: Form1701QDraft = serde_json::from_str(BASE_ERA.trim_end()).unwrap();
    legacy.has_spouse = true;
    legacy.set_amount(64, Form1701QParty::Spouse, Some(500.0));
    legacy.recompute();
    assert_eq!(legacy.spouse_penalty.penalty_model_version, 0);
    let draft = serde_json::to_value(&legacy).unwrap();
    assert_eq!(
        field_map(Q, &draft)["frm1701q:txt64B"],
        "500.00",
        "it files"
    );
    let left = fill(Q, &draft, json!({ rows_income_q::CIVIL_STATUS: "Single" }));
    assert_eq!(left["removes"]["rows"], 1, "{}", left["removes"]);
    assert_eq!(field_map(Q, &left["draft"])["frm1701q:txt64B"], "0.00");
}

/// Review round 2 (F10): a batch that raises the sales and types item 15
/// is read on its recomputed draft: item 15 opens with the regular tax the
/// batch makes, not the totals stored before it.
#[test]
fn form_1702q_a_batch_that_opens_item_15_takes_it() {
    setup();
    let opened = open(C, 2);
    let with_atc = fill(
        C,
        &opened["draft"],
        json!({"frm1702q:cbATC_2": "IC010_25%", "frm1702q:rbATC_2": "true", "frm1702q:rbMthdOfDdctns_1": "true"}),
    );
    assert_eq!(
        with_atc["values"]["frm1702q:txtTax14"], "0.00",
        "no regular tax before the batch"
    );
    let batch = fill(
        C,
        &with_atc["draft"],
        json!({"frm1702q:Sched2:txtTax1": "1000000", "frm1702q:txtTax15": "1000"}),
    );
    assert_eq!(batch["rejected"], json!({}), "{}", batch["rejected"]);
    assert_eq!(amount(&batch["values"], "frm1702q:txtTax15"), 1_000.0);
}
