//! PR 7: 1700, 1701, 1701A and 1701MS against their printed sheets. Each
//! check reads an oracle the host does not compute: the frozen sheets'
//! printed captions and text (`html-frozen/<slug>/`), the forms' own field
//! and print maps, the rules corpus, and hand lists.

use super::*;

fn repo_file(path: &str) -> String {
    let path = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn repo_json(path: &str) -> Value {
    serde_json::from_str(&repo_file(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
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

/// The sheet's text: every text run of its index.html, whitespace collapsed
/// and the typographic dashes and quotes read as plain ones.
fn sheet_text(slug: &str) -> String {
    let html = repo_file(&format!("html-frozen/{slug}/index.html"));
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                text.push(' ');
            }
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    normal(&text.replace("&amp;", "&"))
}

fn normal(text: &str) -> String {
    text.replace(['–', '—'], "-")
        .replace(['’', '‘'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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

/// Every drawn box with its part, item number and row (for a table, the
/// row's printed number).
fn drawn(code: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for (part, item) in items(code) {
        let no = item["no"].as_str().unwrap_or_default().to_string();
        for field in item["f"].as_array().into_iter().flatten() {
            if field["t"] != "hidden" {
                out.push((
                    part.clone(),
                    no.clone(),
                    field["k"].as_str().unwrap().into(),
                ));
            }
        }
        let start = item["start"].as_u64().unwrap_or(1);
        for (r, row) in item["keys"].as_array().into_iter().flatten().enumerate() {
            for key in row.as_array().into_iter().flatten() {
                out.push((
                    part.clone(),
                    (start + r as u64).to_string(),
                    key.as_str().unwrap().into(),
                ));
            }
        }
    }
    out
}

/// The item a printed caption names: the token after " item " ("5.A item
/// 1, Description", "Part VII item 9 (specify)"), after a leading
/// "Schedule N " ("Schedule 5 1 Amount"), or the first token when it starts
/// with a digit ("26 Tax Due", "9A ZIP Code").
fn caption_item(caption: &str) -> Option<String> {
    let token = |text: &str| -> String {
        text.chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect()
    };
    if let Some((_, rest)) = caption.split_once(" item ") {
        return Some(token(rest));
    }
    let rest = caption
        .strip_prefix("Schedule ")
        .and_then(|rest| rest.split_once(' ').map(|(_, rest)| rest))
        .unwrap_or(caption);
    let first = token(rest);
    first
        .starts_with(|c: char| c.is_ascii_digit())
        .then_some(first)
}

/// A "1", "9A", "17a" item number as a sortable key.
fn order(no: &str) -> (u32, String) {
    let digits: String = no.chars().take_while(char::is_ascii_digit).collect();
    (
        digits.parse().unwrap_or(0),
        no[digits.len()..].to_ascii_uppercase(),
    )
}

fn opened(code: &str) -> Value {
    open(code, 0)
}

/// The page edits that make each form a joint return, in the order the
/// sheet asks them.
fn joint(code: &str) -> Value {
    match code {
        "1700" => json!({
            "frm1700:rdoPg1I15CivilStatusM": "true",
            "frm1700:rdoPg1I16SpouseHasIncomeY": "true",
            "frm1700:rdoPg1I17FilingStatusJ": "true",
        }),
        "1701" => json!({
            "frm1701:rdoPg1I16CivilStatusM": "true",
            "frm1701:rdoPg1I17SpouseIncomeYes": "true",
            "frm1701:rdoPg1I18FilingStatusJ": "true",
        }),
        "1701A" => json!({
            "frm1701A:optCivilStatus_2": "true",
            "frm1701A:optSpouseIncome_1": "true",
            "frm1701A:optFilingStatus_1": "true",
        }),
        _ => json!({
            "frm1701MS:civilStatus": "Married",
            "frm1701MS:txtIfMarriedJointlyNo5": "true",
        }),
    }
}

fn take(code: &str, draft: &Value, edits: Value) -> Value {
    let filled = fill(code, draft, edits.clone());
    assert_eq!(filled["rejected"], json!({}), "{code} takes {edits}");
    filled
}

// ------------------------------------------------------------- layout

/// R3.0-R3.6, R3.10, R5.3, manual-2 to manual-5 (1701), R3.0-R3.2 and
/// manual-1 (1701A): every drawn box with a printed caption sits under the
/// item number the caption names, and the numbered items run down each part
/// in paper order. 1701A's amounts print from `derived:amount_<item><A|B>`
/// (its writer-cells not_joined: "prints from its derived:amount_* key"),
/// whose name carries the item.
#[test]
fn form_1701_and_1701a_items_follow_the_printed_sheet() {
    setup();
    for (code, slug) in [("1701", "1701-2018"), ("1701A", "1701a-2018")] {
        let printed_at = captions(slug);
        let mut checked = 0;
        for (part, no, key) in drawn(code) {
            let caption = printed_at.get(&key).cloned().or_else(|| {
                // 1701A: txt36A prints as derived:amount_36A "36 ...".
                let tail = key.strip_prefix("frm1701A:txt")?;
                let item: String = tail.chars().take_while(char::is_ascii_digit).collect();
                let column = &tail[item.len()..];
                (!item.is_empty() && matches!(column, "A" | "B" | ""))
                    .then(|| printed_at.get(&format!("derived:amount_{item}{column}")))
                    .flatten()
                    .cloned()
            });
            let Some(caption) = caption else {
                continue;
            };
            let Some(item) = caption_item(&caption) else {
                continue;
            };
            assert!(
                no.eq_ignore_ascii_case(&item),
                "{code} {part}: {key} under item {no:?}, printed {caption:?}"
            );
            checked += 1;
        }
        assert!(
            checked > if code == "1701" { 300 } else { 120 },
            "{code}: {checked}"
        );
        // Paper order within each part.
        let parts: BTreeSet<String> = items(code).into_iter().map(|(part, _)| part).collect();
        for part in parts {
            let numbers: Vec<(u32, String)> = items(code)
                .into_iter()
                .filter(|(p, _)| *p == part)
                .filter_map(|(_, it)| {
                    let no = it["no"].as_str().unwrap_or_default();
                    let first = no.split('-').next().unwrap_or_default();
                    (!first.is_empty()).then(|| order(first))
                })
                .collect();
            let mut sorted = numbers.clone();
            sorted.sort();
            assert_eq!(numbers, sorted, "{code}: {part} runs in paper order");
        }
    }
    // manual-3, manual-4, manual-5: no stray copies and each part where the
    // sheet prints it.
    let at = |code: &str, key: &str| -> (String, String) {
        drawn(code)
            .into_iter()
            .find(|(_, _, k)| k == key)
            .map(|(part, no, _)| (part, no))
            .unwrap_or_else(|| panic!("{code}: {key} drawn"))
    };
    // Schedule 6 runs on from page 3 to page 4 as one schedule.
    assert_eq!(
        at("1701", "frm1701:txtPg4IShed6_13D").0,
        at("1701", "frm1701:txtPg3IShed6_8D").0
    );
    assert!(
        at("1701", "frm1701:txtPg4IShed6_13D")
            .0
            .contains("Schedule 6")
    );
    assert!(
        at("1701", "frm1701:txtPg4ISc6_3A")
            .0
            .starts_with("Part VI ")
    );
    assert!(
        at("1701", "frm1701:txtPg4IPart8_1A")
            .0
            .starts_with("Part VIII ")
    );
    assert!(
        at("1701", "frm1701:txtPg4IPart9_11A")
            .0
            .starts_with("Part IX ")
    );
    assert_eq!(at("1701", "frm1701:txtPg2IShed3_9A").1, "9");
    assert_eq!(at("1701", "frm1701:txtPg3IShed4_9A").1, "9");
    assert_eq!(
        at("1701", "frm1701:txtPg2I1TIN1"),
        (
            "Part IV · Background Information of Spouse".into(),
            "1".into()
        )
    );
    assert!(
        at("1701", "frm1701:txtPg2IShed1c_1CI")
            .0
            .contains("Schedule 1")
    );
    // 1701A manual-1: item 42's (add more...) rows sit with item 42.
    let (part, _) = at("1701A", "frm1701A:txtmodalPartIVADesc_1C1");
    assert_eq!(part, at("1701A", "frm1701A:txt42A").0);
}

/// R3.0-R3.3 and manual-1 (1701MS): every two-column amount sits under the
/// item the print fills from it. The oracle is the form's print map: on a
/// draft whose amounts all differ, a box's own value is what the print
/// writes in the cell of its part and item (`derived:tp_p<part>_<item>` for
/// column A, `derived:sp_...` for B, form_1701ms.rs `print_amounts`).
#[test]
fn form_1701ms_amounts_sit_under_the_items_they_print_in() {
    use bir_core::forms::form_1701ms::{
        Form1701MsColumn, Form1701MsDeduction, Form1701MsDraft, Form1701MsSource,
        Form1701MsTaxOption,
    };
    setup();
    let base: Form1701MsDraft = serde_json::from_value(opened("1701MS")["draft"].clone()).unwrap();
    let mut checked = 0;
    for (source, option, deduction) in [
        (
            Form1701MsSource::Mixed,
            Form1701MsTaxOption::Graduated,
            Form1701MsDeduction::Itemized,
        ),
        (
            Form1701MsSource::Business,
            Form1701MsTaxOption::Special,
            Form1701MsDeduction::Itemized,
        ),
        (
            Form1701MsSource::Profession,
            Form1701MsTaxOption::EightPercent,
            Form1701MsDeduction::Unanswered,
        ),
    ] {
        let mut draft = base.clone();
        let column = |n: f64| Form1701MsColumn {
            source,
            tax_option: option,
            deduction,
            gross_compensation: 1_001_000.0 * n,
            non_taxable_compensation: 2_000.0 * n,
            sales: 3_003_000.0 * n,
            sales_returns: 4_000.0 * n,
            cost_of_sales: 5_000.0 * n,
            itemized_deductions: 6_000.0 * n,
            special_allowable_deductions: 7_000.0 * n,
            nolco: 8_000.0 * n,
            non_operating_income: 9_000.0 * n,
            special_rate: 5.0,
            eight_sales: 2_100_000.0 * n,
            eight_other_income: 11_000.0 * n,
            prior_year_excess: 12_000.0 * n,
            quarterly_payments: 13_000.0 * n,
            cwt_q1_q3: 14_000.0 * n,
            cwt_q4: 15_000.0 * n,
            cwt_2316: 16_000.0 * n,
            special_tax_credits: 17_000.0 * n,
            other_credits: 18_000.0 * n,
            share_of_other_agencies: 19.0 * n,
            second_installment: 0.0,
            ..Form1701MsColumn::default()
        };
        draft.civil_status = bir_core::forms::form_1701ms::Form1701MsCivilStatus::Married;
        draft.filing = bir_core::forms::form_1701ms::Form1701MsFiling::Jointly;
        draft.taxpayer = column(1.0);
        draft.spouse = column(2.0);
        draft.recompute();
        let draft = serde_json::to_value(&draft).unwrap();
        let fields = field_map("1701MS", &draft);
        let print = printed("1701MS", &draft);
        for (part, item) in items("1701MS") {
            let no = item["no"].as_str().unwrap_or_default().to_ascii_lowercase();
            let section = if part.starts_with("Part II ") {
                "p2"
            } else if part.starts_with("Part IV") {
                "p4"
            } else if part.starts_with("Part V ") {
                "p5"
            } else if part.starts_with("Part VI ") {
                "p6"
            } else {
                continue;
            };
            let keys: Vec<&str> = item["f"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["t"] == "money")
                .filter_map(|f| f["k"].as_str())
                .collect();
            if keys.len() != 2 {
                continue;
            }
            for (key, side) in keys.iter().zip(["tp", "sp"]) {
                let printed_key = format!("derived:{side}_{section}_{no}");
                let Some(cell) = print.get(&printed_key) else {
                    continue;
                };
                let page = fields[*key].replace(',', "");
                let page: f64 = page.trim_matches(['(', ')']).parse().unwrap_or(0.0);
                let cell: f64 = cell.trim().trim_matches(['(', ')']).parse().unwrap_or(0.0);
                assert_eq!(
                    page.abs().round(),
                    cell,
                    "1701MS {part} item {no}: {key} prints as {printed_key}"
                );
                if page != 0.0 {
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 60, "{checked} amounts checked");
}

/// R3.7-R3.10, R3.3-R3.5 (1701A), manual-2 and manual-6: labels are the
/// printed captions. Every option, column and item label checked here is
/// text of the frozen sheet; none repeats its question or item number or
/// reads like a key.
#[test]
fn annual_forms_read_the_printed_captions() {
    setup();
    for (code, slug) in [
        ("1700", "1700-2018"),
        ("1701", "1701-2018"),
        ("1701A", "1701a-2018"),
        ("1701MS", "1701ms-2024"),
    ] {
        let text = sheet_text(slug);
        for (part, item) in items(code) {
            let no = item["no"].as_str().unwrap_or_default();
            let label = item["l"].as_str().unwrap_or_default();
            assert!(
                no.is_empty() || !label.trim_end().ends_with(&format!(" {no}")),
                "{code} {part}: item {no} {label:?} ends with its own number"
            );
            for field in item["f"].as_array().into_iter().flatten() {
                if field["t"] == "hidden" {
                    continue;
                }
                let l = field["l"].as_str().unwrap_or_default();
                let lower = l.to_ascii_lowercase();
                for key_text in [
                    " shed ",
                    "tp name",
                    " no. ",
                    "frm",
                    "txt",
                    "spouse atc ",
                    "tax rate g",
                    "tax rate p",
                ] {
                    assert!(
                        !format!(" {lower} ").contains(key_text),
                        "{code} {part} item {no}: {:?} reads like a key ({l:?})",
                        field["k"]
                    );
                }
                // An option repeats neither its item number nor "ATC".
                assert!(
                    no.is_empty() || !l.starts_with(&format!("{no} ")),
                    "{code}: {:?} {l:?} repeats item {no}",
                    field["k"]
                );
            }
        }
        // Spot checks against the sheet's own words.
        let must: &[(&str, &str)] = match code {
            "1701" => &[
                (
                    "frm1701:rdoPg1I7ATC_II012",
                    "II012 Business Income-Graduated IT Rates",
                ),
                (
                    "frm1701:rdoPg1I21TaxRateP",
                    "8% in lieu of Graduated Rates under Sec. 24(A) & Percentage Tax under Sec. 116 of NIRC",
                ),
                (
                    "frm1701:rdoPg2I4ATC_II016",
                    "II016 Mixed Income – 8% IT Rate",
                ),
                ("frm1701:rdoPg1I6TaxpayerTypeE", "Estate"),
            ],
            "1701A" => &[
                (
                    "frm1701A:optATC_1",
                    "II012 Business Income - Graduated IT Rates",
                ),
                (
                    "frm1701A:optSpouseATC_2",
                    "II014 Income from Profession - Graduated IT Rates",
                ),
                (
                    "frm1701A:optSpouseATC_4",
                    "II017 Income from Profession - 8% IT Rate",
                ),
            ],
            "1701MS" => &[
                (
                    "frm1701MS:txtTaxpayerNo12e1",
                    "B. Special/Preferential Rate",
                ),
                ("frm1701MS:txtCreditableForm2316No5a", "A. Taxpayer/Filer"),
            ],
            _ => &[],
        };
        for (key, want) in must {
            let label = items(code)
                .into_iter()
                .flat_map(|(_, it)| it["f"].as_array().cloned().unwrap_or_default())
                .find(|f| f["k"] == *key)
                .and_then(|f| f["l"].as_str().map(str::to_string))
                .unwrap_or_else(|| panic!("{code}: {key} drawn"));
            assert_eq!(&label, want, "{code}: {key}");
            let words = normal(want.trim_start_matches("A. ").trim_start_matches("B. "));
            assert!(
                text.contains(&words),
                "{code}: {want:?} is the sheet's text"
            );
        }
    }
    // Item captions cut mid-word before (R3.8, 1701A R3.4, 1701MS R3.4):
    // each is the sheet's text.
    for (code, slug, key) in [
        ("1701", "1701-2018", "frm1701:txtPg1I9Address"),
        ("1701A", "1701a-2018", "frm1701A:txtAddress"),
        ("1701MS", "1701ms-2024", "frm1701MS:perjuryClause"),
        // The ordinal lost before ("for the th Quarter", 1701A R3.5).
        ("1701A", "1701a-2018", "frm1701A:txt60A"),
        ("1701", "1701-2018", "frm1701:txtPg4IPart7_4A"),
        // Item 49's caption, not the rounding instruction (1701A manual-2).
        ("1701A", "1701a-2018", "frm1701A:txt49A"),
    ] {
        let (_, item) = items(code)
            .into_iter()
            .find(|(_, it)| {
                it["f"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|f| f["k"] == key)
            })
            .unwrap();
        let label = normal(item["l"].as_str().unwrap());
        // 1701A sets the "th" of "4th Quarter" as a raised run of its own,
        // so the sheet's text reads "4 Quarter" there.
        let text = sheet_text(slug);
        assert!(
            text.contains(&label) || text.contains(&label.replace(" 4th ", " 4 ")),
            "{code}: {label:?}"
        );
    }
}

// ------------------------------------------------------------ RDO boxes

/// R2.2, preview.0 (1701, 1701A), manual-0 (1701A): items 5 and the spouse's
/// RDO are drawn where the sheet prints them, item 5 from the profile, and
/// the page shows what the print writes from the same draft.
#[test]
fn annual_forms_draw_the_rdo_boxes_the_print_fills() {
    setup();
    for (code, own, spouse, spouse_item) in [
        (
            "1701",
            "frm1701:txtPg1I5RDOCode",
            "frm1701:txtPg2I2SpouseRDOCode",
            "2",
        ),
        (
            "1701A",
            "frm1701A:txtRDOCode",
            "frm1701A:txtSpouseRDOCode",
            "67",
        ),
    ] {
        let at = |key: &str| {
            drawn(code)
                .into_iter()
                .find(|(_, _, k)| k == key)
                .unwrap_or_else(|| panic!("{code}: {key} drawn"))
        };
        assert_eq!(at(own).1, "5");
        assert!(at(own).0.starts_with("Part I "));
        assert_eq!(at(spouse).1, spouse_item);
        let opened = opened(code);
        assert_eq!(
            opened["values"][own], "039",
            "{code}: item 5 from the profile"
        );
        assert!(strings(&opened["profile_edit"]).contains(own));
        assert_eq!(printed(code, &opened["draft"])[own], "039");
        // Page and print agree on the spouse's RDO before there is one.
        let print = printed(code, &opened["draft"]);
        let blank = print
            .get(spouse)
            .or_else(|| print.get("derived:spouse_rdo"));
        assert_eq!(
            blank.map(String::as_str).unwrap_or_default(),
            opened["values"][spouse].as_str().unwrap_or_default(),
            "{code}: the spouse's RDO before there is one"
        );
        // The spouse's RDO, typed on a joint return, files and prints.
        let on = take(code, &opened["draft"], joint(code));
        let typed = take(code, &on["draft"], json!({ spouse: "040" }));
        assert_eq!(typed["values"][spouse], "040", "{code}");
        let print = printed(code, &typed["draft"]);
        let shown = print
            .get(spouse)
            .or_else(|| print.get("derived:spouse_rdo"));
        assert_eq!(shown.map(String::as_str), Some("040"), "{code}");
        let filed = field_map(code, &typed["draft"]);
        assert_eq!(filed[spouse], "040", "{code}: files it");
    }
}

// --------------------------------------------------------- spouse part

/// manual-0 (1700, 1701), manual-3 (1701A), manual-4 (1701MS): on a return
/// with no civil status the spouse items are faded with what opens them and
/// a write there is refused in the filer's words; opened by the printed
/// answers, the spouse's entries file and print. For 1701 this extends the
/// maintainer's decision 11 (made for 1701Q) to its printed item 16.
#[test]
fn annual_forms_open_the_spouse_part_from_the_printed_civil_status() {
    setup();
    for (code, name, tag, refusal) in [
        (
            "1700",
            "frm1700:txtPg1I21SpouseName",
            "Needs item 15 = Married, item 16 = Yes and item 17 = Joint Filing",
            "Part II is for a joint return: choose Married at item 15, Yes at item 16 and Joint Filing at item 17 first.",
        ),
        (
            "1701",
            "frm1701:txtPg2I5SpouseName",
            "Needs item 16 = Married, item 17 = Yes and item 18 = Joint Filing",
            "Part IV is for a joint return: choose Married at item 16, Yes at item 17 and Joint Filing at item 18 first.",
        ),
        (
            "1701A",
            "frm1701A:txtSpouseName",
            "Needs item 16 = Married, item 17 = Yes and item 18 = Joint Filing",
            "Part V is for a joint return: choose Married at item 16, Yes at item 17 and Joint Filing at item 18 first.",
        ),
        (
            "1701MS",
            "frm1701MS:txtTaxpayerNo8b",
            "Needs item 4 = Married and item 5 = Jointly",
            "Column B of Part I is for a joint return: choose Married at item 4 and Jointly at item 5 first.",
        ),
    ] {
        let opened = opened(code);
        assert_eq!(opened["off"][name], tag, "{code}");
        assert!(opened["locks"][name].is_null(), "{code}: faded, not locked");
        let refused = fill(code, &opened["draft"], json!({ name: "SAMPLE SPOUSE" }));
        assert_eq!(refused["rejected"][name], refusal, "{code}");
        assert_eq!(refused["draft"], opened["draft"], "{code}: nothing written");
        // A batch that answers the items and fills the name takes it all.
        let mut batch = joint(code);
        batch[name] = json!("Sample Spouse");
        let filled = take(code, &opened["draft"], batch);
        assert_eq!(filled["values"][name], "SAMPLE SPOUSE", "{code}");
        let print = printed(code, &filled["draft"]);
        let shown = print
            .get(name)
            .or_else(|| print.get("derived:sp_name"))
            .map(String::as_str);
        // 1701's print keeps the name as typed (its XML-shaped map); the
        // others print the filed capitals.
        assert!(
            shown.is_some_and(|shown| shown.eq_ignore_ascii_case("SAMPLE SPOUSE")),
            "{code} prints it: {shown:?}"
        );
        let checked = validated(code, &filled["draft"]);
        assert!(checked["off"].get(name).is_none(), "{code}: open");
    }
    // The items before the part follow the civil status one by one: 1701's
    // item 17 needs Married, item 18 Married and Yes.
    let o = opened("1701");
    assert_eq!(
        o["off"]["frm1701:rdoPg1I17SpouseIncomeYes"],
        "Needs item 16 = Married"
    );
    assert_eq!(
        o["off"]["frm1701:rdoPg1I18FilingStatusJ"],
        "Needs item 16 = Married and item 17 = Yes"
    );
    let married = take(
        "1701",
        &o["draft"],
        json!({"frm1701:rdoPg1I16CivilStatusM": "true"}),
    );
    let checked = validated("1701", &married["draft"]);
    assert!(
        checked["off"]
            .get("frm1701:rdoPg1I17SpouseIncomeYes")
            .is_none()
    );
    assert_eq!(
        checked["off"]["frm1701:rdoPg1I18FilingStatusJ"],
        "Needs item 16 = Married and item 17 = Yes"
    );
    // A radio group named by its group box is refused alike (batch edits).
    let group = fill(
        "1700",
        &opened("1700")["draft"],
        json!({"frm1700:rdoPg1I20SpouseTaxpayerType": "E"}),
    );
    assert!(
        group["rejected"]["frm1700:rdoPg1I20SpouseTaxpayerType"]
            .as_str()
            .is_some_and(|why| why.starts_with("Part II is for a joint return")),
        "{}",
        group["rejected"]
    );
}

/// Lesson 8: an answer that closes the spouse part asks first: the page
/// gets the question and the Undo text, and the count is what the filer had
/// typed there (oracle: the entries typed by hand in this test).
#[test]
fn closing_the_spouse_part_asks_first_on_every_annual_form() {
    setup();
    for (code, typed, single, answers) in [
        (
            "1700",
            json!({
                "frm1700:txtPg1I21SpouseName": "Sample Spouse",
                "frm1700:txtPg1I23SpouseCitizenship": "Filipino",
                "frm1700:rdoPg1I20SpouseTaxpayerTypeE": "true",
            }),
            json!({"frm1700:rdoPg1I15CivilStatusS": "true"}),
            2,
        ),
        (
            "1701",
            json!({
                "frm1701:txtPg2I5SpouseName": "Sample Spouse",
                "frm1701:txtPg2I7Citizenship": "Filipino",
                "frm1701:rdoPg2I3SpouseTypeS": "true",
            }),
            json!({"frm1701:rdoPg1I16CivilStatusS": "true"}),
            2,
        ),
        (
            "1701A",
            json!({
                "frm1701A:txtSpouseName": "Sample Spouse",
                "frm1701A:txtSpouseCitizenship": "Filipino",
                "frm1701A:optSpouseTaxType_1": "true",
            }),
            json!({"frm1701A:optCivilStatus_1": "true"}),
            2,
        ),
        (
            "1701MS",
            json!({
                "frm1701MS:txtTaxpayerNo8b": "Sample Spouse",
                "frm1701MS:txtTelNum10b": "09170000001",
                "frm1701MS:txtTaxpayerNo11a1": "true",
            }),
            json!({"frm1701MS:civilStatus": "Single"}),
            1,
        ),
    ] {
        let opened = opened(code);
        let on = take(code, &opened["draft"], joint(code));
        let filled = take(code, &on["draft"], typed.clone());
        assert_eq!(
            filled["removes"],
            Value::Null,
            "{code}: typing removes nothing"
        );
        let closed = take(code, &filled["draft"], single);
        let removes = &closed["removes"];
        // The three typed spouse entries and the joint answers after the
        // civil status (`joint`).
        let rows = 3 + answers;
        assert_eq!(removes["rows"], rows, "{code}: {removes}");
        assert!(
            removes["message"]
                .as_str()
                .is_some_and(|m| m.starts_with("This answer clears what you typed in ")
                    && m.ends_with(". Continue?")),
            "{code}: {removes}"
        );
        assert_eq!(removes["done"], format!("Removed {rows} entries"), "{code}");
        // An answer that removes nothing asks nothing.
        let quiet = take(code, &on["draft"], json!({}));
        assert_eq!(quiet["removes"], Value::Null, "{code}");
    }
    // 1700's Amended Return No clears item 55 (Tax Paid in Return Previously
    // Filed): asked first too.
    let o = opened("1700");
    let amended = take(
        "1700",
        &o["draft"],
        json!({"frm1700:rdoPg1I2AmendedYes": "true"}),
    );
    let typed = take(
        "1700",
        &amended["draft"],
        json!({"frm1700:txtPg2I55A": "1500"}),
    );
    let no = take(
        "1700",
        &typed["draft"],
        json!({"frm1700:rdoPg1I2AmendedNo": "true"}),
    );
    assert_eq!(
        no["removes"]["message"],
        "This answer clears what you typed in Part V item 55. Continue?"
    );
}

// ------------------------------------------------------------ 1700

/// 1700 manual-1: the email shows as typed (the upload and the print keep
/// BIR's capitals), and the spouse's RDO and the number of attachments
/// show blank until there is a value (the upload keeps 000 and 00).
#[test]
fn form_1700_shows_what_was_typed_and_blank_defaults() {
    setup();
    let opened = opened("1700");
    let draft = &opened["draft"];
    assert_eq!(draft["email"], "books@example.com");
    assert_eq!(
        opened["values"]["frm1700:txtPg1I10Email"],
        "books@example.com"
    );
    assert_eq!(
        field_map("1700", draft)["frm1700:txtPg1I10Email"],
        "BOOKS@EXAMPLE.COM"
    );
    assert_eq!(opened["values"]["frm1700:txtSpouseRDOCode"], "");
    assert_eq!(field_map("1700", draft)["frm1700:txtSpouseRDOCode"], "000");
    assert_eq!(opened["values"]["frm1700:txtPg1I37NumberOfAttachments"], "");
    assert_eq!(
        field_map("1700", draft)["frm1700:txtPg1I37NumberOfAttachments"],
        "00"
    );
    let typed = take(
        "1700",
        draft,
        json!({"frm1700:txtPg1I37NumberOfAttachments": "3"}),
    );
    assert_eq!(typed["values"]["frm1700:txtPg1I37NumberOfAttachments"], "3");
}

/// 1700 preview.0: Schedule 1 rows 3 and 4 mark the Taxpayer or the Spouse
/// from the draft's employers (rows_1700): page and print agree on each.
#[test]
fn form_1700_rows_3_and_4_mark_their_party_from_the_draft() {
    setup();
    let opened = opened("1700");
    let on = take("1700", &opened["draft"], joint("1700"));
    let mut edits = Map::new();
    for (row, name) in [(1, "One"), (2, "Two"), (3, "Three"), (4, "Four")] {
        edits.insert(
            format!("frm1700:txtPg2I{row}PartVIEmployerName1"),
            json!(format!("Employer {name}")),
        );
    }
    edits.insert("frm1700:rdoPg2I3PartVIEmployeeS".into(), json!("true"));
    edits.insert("frm1700:rdoPg2I4PartVIEmployeeT".into(), json!("true"));
    let filled = take("1700", &on["draft"], Value::Object(edits));
    let print = printed("1700", &filled["draft"]);
    for (key, want) in [
        ("frm1700:rdoPg2I3PartVIEmployeeS", "true"),
        ("frm1700:rdoPg2I3PartVIEmployeeT", "false"),
        ("frm1700:rdoPg2I4PartVIEmployeeT", "true"),
        ("frm1700:rdoPg2I4PartVIEmployeeS", "false"),
    ] {
        assert_eq!(filled["values"][key], want, "{key} on the page");
        assert_eq!(print[key], want, "{key} on the print");
    }
}

/// validation.0 (1700, 1701, 1701A): an official message is drawn under the
/// item it cites, its text unchanged (the item numbers are the sheet's).
#[test]
fn an_official_message_sits_under_the_item_it_cites() {
    setup();
    for (code, message, item) in [
        ("1700", "Please select Taxpayer Type on Item no. 6", "6"),
        ("1701", "Please select an option for page 1 Item 7.", "7"),
        ("1701A", "Please select an option for Item 6.", "6"),
        ("1701A", "Please select an option for Item 19.", "19"),
    ] {
        let opened = opened(code);
        let error = opened["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["message"] == message)
            .unwrap_or_else(|| panic!("{code}: {message:?} on a fresh return"));
        let keys = strings(&error["keys"]);
        assert!(!keys.is_empty(), "{code}: {message}");
        let under: BTreeSet<String> = drawn(code)
            .into_iter()
            .filter(|(part, no, _)| no == item && (part.starts_with("Part I ") || code == "1701A"))
            .map(|(_, _, key)| key)
            .collect();
        assert!(keys.is_subset(&under), "{code}: {message} on {keys:?}");
    }
}

// ------------------------------------------------------------- 1701

/// manual-2, R3.9: Schedule 1 is one table of employer rows; a row marked
/// for the spouse shows its employer in the row's name box, its TIN typed box
/// by box files in the row's TIN boxes, and the amounts follow.
#[test]
fn form_1701_schedule_1_is_one_table_of_employers() {
    setup();
    let (_, table) = items("1701")
        .into_iter()
        .find(|(_, it)| it["no"] == "1-2")
        .expect("Schedule 1's table");
    assert_eq!(table["rows"], 2);
    let cols: Vec<&str> = table["cols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["l"].as_str().unwrap())
        .collect();
    assert_eq!(&cols[..3], ["Taxpayer", "Spouse", "a. Name of Employer"]);
    let opened = opened("1701");
    let mut on = joint("1701");
    on["frm1701:rdoPg1I7ATC_II013"] = json!("true");
    on["frm1701:rdoPg2I4ATC_II011"] = json!("true");
    let on = take("1701", &opened["draft"], on);
    let filled = take(
        "1701",
        &on["draft"],
        json!({
            "frm1701:chkPg2IShed2a_2Spouse": "true",
            "frm1701:txtPg2IShed2a_2TPName": "Spouse Employer Corp",
            "frm1701:txtPg2IShed2a_TIN1": "123",
            "frm1701:txtPg2IShed2a_TIN2": "456",
            "frm1701:txtPg2IShed2a_TIN3": "789",
            "frm1701:txtPg2IShed2a_BranchCode": "00000",
            "frm1701:txtPg2IShed1c_2CI": "300000",
        }),
    );
    assert_eq!(
        filled["values"]["frm1701:txtPg2IShed2a_2TPName"],
        "SPOUSE EMPLOYER CORP"
    );
    let fields = field_map("1701", &filled["draft"]);
    assert_eq!(
        fields["frm1701:txtPg2IShed2a_2SName"],
        "SPOUSE EMPLOYER CORP"
    );
    assert_eq!(fields["frm1701:txtPg2IShed2a_TIN3"], "789");
    assert_eq!(fields["frm1701:txtPg2IShed1c_3BCI"], "300,000.00");
}

/// The 1701 boxes the corpus called computed are typed (FILER_TYPED), and
/// the totals the probe had bound to an item are computed (COMPUTED_BY_FORM):
/// sales, returns and cost of sales file and Items 10 and 12 follow; a total
/// is locked.
#[test]
fn form_1701_schedule_3_and_4_take_the_filers_amounts() {
    setup();
    let opened = opened("1701");
    let graduated = take(
        "1701",
        &opened["draft"],
        json!({
            "frm1701:rdoPg1I6TaxpayerTypeS": "true",
            "frm1701:rdoPg1I7ATC_II012": "true",
            "frm1701:rdoPg1I21AMethodDeductionI": "true",
        }),
    );
    let typed = take(
        "1701",
        &graduated["draft"],
        json!({
            "frm1701:txtPg2IShed3_8A": "1000000",
            "frm1701:txtPg2IShed3_9A": "50000",
            "frm1701:txtPg2IShed3_11A": "200000",
            "frm1701:txtPg3IShed4_13A": "120000",
            "frm1701:txtPg3IShed4_3A": "10000",
        }),
    );
    let fields = field_map("1701", &typed["draft"]);
    assert_eq!(fields["frm1701:txtPg2IShed3_10A"], "950,000.00");
    assert_eq!(fields["frm1701:txtPg2IShed3_12A"], "750,000.00");
    assert_eq!(fields["frm1701:txtPg3IShed4_18A"], "130,000.00");
    assert_eq!(fields["frm1701:txtPg2IShed3_16A"], "130,000.00");
    for total in [
        "frm1701:txtPg2IShed3_14A",
        "frm1701:txtPg2IShed3_15A",
        "frm1701:txtPg2IShed3_16A",
        "frm1701:txtPg3IShed6_8D",
        "frm1701:txtPg4IPart9_10A",
        "frm1701:txtPg4IPart9_10B",
    ] {
        let refused = fill("1701", &typed["draft"], json!({ total: "5" }));
        assert!(
            refused["rejected"][total].is_string(),
            "{total} is computed"
        );
    }
}

/// R8.0 (1701): Items 27-29 show the shared model's amounts, locked until
/// Edit penalties; a typed one files and Items 30 and 31 follow. A draft
/// stored before (no penalty state) says its amounts are kept as entered,
/// and Use the computed amounts gives the computed ones.
#[test]
fn form_1701_penalties_are_computed_and_kept_ones_say_so() {
    setup();
    let opened = opened("1701");
    for key in [
        "frm1701:txtPg1I27A",
        "frm1701:txtPg1I28A",
        "frm1701:txtPg1I29A",
    ] {
        assert_eq!(opened["locks"][key], penalties::LOCK, "{key}");
    }
    let unlocked = [
        "frm1701:txtPg1I27A",
        "frm1701:txtPg1I28A",
        "frm1701:txtPg1I29A",
    ];
    let typed = fill_unlocked(
        "1701",
        &opened["draft"],
        json!({"frm1701:txtPg1I29A": "1000"}),
        &unlocked,
    );
    assert_eq!(typed["rejected"], json!({}));
    let fields = field_map("1701", &typed["draft"]);
    assert_eq!(fields["frm1701:txtPg1I29A"], "1,000.00");
    assert_eq!(fields["frm1701:txtPg1I30A"], "1,000.00");
    // A stored draft from before the model.
    let mut legacy = opened["draft"].clone();
    legacy.as_object_mut().unwrap().remove("penalty");
    legacy.as_object_mut().unwrap().remove("spouse_penalty");
    legacy["computations"]["part_ii"]["27"] = json!({"taxpayer": 125.0, "spouse": null});
    let checked = validated("1701", &legacy);
    assert_eq!(checked["values"]["frm1701:txtPg1I27A"], "125.00");
    assert_eq!(
        rows_annual_individual::penalty_lock(
            kind_of("1701").unwrap(),
            &checked["draft"],
            "frm1701:txtPg1I27A"
        ),
        Some(rows_annual_individual::KEPT_AS_ENTERED)
    );
    let group = checked["penalties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| strings(&g["keys"]).contains("frm1701:txtPg1I27A"))
        .unwrap();
    assert_eq!(group["kept"], true);
    let computed = fill_unlocked(
        "1701",
        &checked["draft"],
        json!({"frm1701:txtPg1I27A": "", "frm1701:txtPg1I28A": "", "frm1701:txtPg1I29A": ""}),
        &unlocked,
    );
    assert_eq!(computed["draft"]["penalty"]["penalty_model_version"], 1);
    assert_eq!(
        computed["values"]["frm1701:txtPg1I27A"],
        opened["values"]["frm1701:txtPg1I27A"]
    );
}

/// 1701 Schedule 6 items 7 and 12, the current year's NOLCO row: the year
/// waits for item 3's loss and then shows the taxable year.
#[test]
fn form_1701_current_year_nolco_waits_for_a_loss() {
    setup();
    let opened = opened("1701");
    assert_eq!(
        opened["waits"]["frm1701:txtPg3IShed6_7Year"],
        "Filled from item 3A when it is a loss"
    );
    let itemized = take(
        "1701",
        &opened["draft"],
        json!({
            "frm1701:rdoPg1I6TaxpayerTypeS": "true",
            "frm1701:rdoPg1I7ATC_II012": "true",
            "frm1701:rdoPg1I21AMethodDeductionI": "true",
            "frm1701:txtPg3IShed6_1A": "100000",
            "frm1701:txtPg3IShed6_2A": "150000",
        }),
    );
    assert_eq!(itemized["values"]["frm1701:txtPg3IShed6_7Year"], "2026");
    assert_eq!(itemized["values"]["frm1701:txtPg3IShed6_7A"], "50,000.00");
}

// ----------------------------------------------------------- 1701MS

/// 1701MS: items 9A and 9B take an email, item 9A from the profile; Part IV
/// item 16's rate is typed as a percent; item 16B's dates are typed on a
/// joint return.
#[test]
fn form_1701ms_emails_rates_and_dates_are_typed() {
    setup();
    let opened = opened("1701MS");
    assert_eq!(opened["values"]["frm1701MS:txtEmaila"], "books@example.com");
    assert!(strings(&opened["profile_edit"]).contains("frm1701MS:txtEmaila"));
    let mut edits = joint("1701MS");
    edits["frm1701MS:txtTaxpayerNo11a1"] = json!("true");
    edits["frm1701MS:txtTaxpayerNo12e1"] = json!("true");
    edits["frm1701MS:txtEmailb"] = json!("spouse@example.com");
    edits["frm1701MS:txtTaxRateb"] = json!("5%");
    edits["frm1701MS:txtTaxpayerNo16b"] = json!("01/15/2020");
    edits["frm1701MS:txtTaxpayerNo16b1"] = json!("01/14/2030");
    let filled = take("1701MS", &opened["draft"], edits);
    assert_eq!(
        filled["draft"]["spouse_info"]["email"],
        "spouse@example.com"
    );
    assert_eq!(filled["draft"]["spouse"]["special_rate"], 5.0);
    assert_eq!(filled["values"]["frm1701MS:txtTaxRateb"], "5%");
    assert_eq!(filled["values"]["frm1701MS:txtTaxpayerNo16b"], "01/15/2020");
    let print = printed("1701MS", &filled["draft"]);
    assert_eq!(print["derived:sp_effectivity_from"], "01/15/2020");
    let refused = fill(
        "1701MS",
        &filled["draft"],
        json!({"frm1701MS:txtTaxRateb": "fifty"}),
    );
    assert!(refused["rejected"]["frm1701MS:txtTaxRateb"].is_string());
}

// ------------------------------------------------------- stored drafts

/// Lesson 6: a stored draft opens and saves back byte for byte on every
/// annual individual form, the 1701 one stored before the penalty model
/// with no `penalty` key.
#[test]
fn annual_stored_drafts_open_and_save_unchanged() {
    setup();
    for code in ["1700", "1701", "1701A", "1701MS"] {
        let mut draft = opened(code)["draft"].clone();
        if code == "1701" {
            draft.as_object_mut().unwrap().remove("penalty");
            draft.as_object_mut().unwrap().remove("spouse_penalty");
        }
        let saved = save(code, &draft);
        let reopened = opened(code);
        let stored = saved["draft"].clone();
        assert_eq!(reopened["draft"], stored, "{code}: opens as stored");
        let again = save(code, &reopened["draft"]);
        assert_eq!(again["draft"], stored, "{code}: saves back as stored");
        if code == "1701" {
            assert!(stored.get("penalty").is_none(), "1701 keeps its shape");
        }
    }
}

// ------------------------------------------------------------ profile

/// R2.0 (1701, 1701A, 1701MS): every profile box the review found locked
/// with no way to edit it (the keys of its findings.json entry, listed
/// here) either offers Edit where the sheet prints it, or is a later
/// page's copy of it, not drawn again (rule 3).
#[test]
fn annual_profile_boxes_offer_edit_or_are_copies() {
    setup();
    for (code, keys) in [
        (
            "1701",
            &[
                "txtEmail",
                "frm1701:txtPg3mTaxpayerName",
                "frm1701:txtPg2mTaxpayerName",
                "frm1701:txtPg1I15TelNum",
                "frm1701:txtPg4mTaxpayerName",
                "frm1701:txtPg2TaxpayerName",
                "frm1701:txtPg1I8TaxpayerName",
                "frm1701:txtLineBus",
                "frm1701:txtPg1I9Address",
                "frm1701:txtPg1mTaxpayerName",
                "frm1701:txtPg4TaxpayerName",
                "frm1701:txtPg1I9AZipCode",
            ][..],
        ),
        (
            "1701A",
            &[
                "frm1701A:txtTelNum",
                "frm1701A:txtLineBus",
                "frm1701A:txtPg2TaxpayerName",
                "frm1701A:txtAddress",
                "frm1701A:txtTaxpayerName",
                "frm1701A:txtZipCode",
                "txtEmail",
            ][..],
        ),
        (
            "1701MS",
            &[
                "frm1701MS:txtTelNum10a",
                "frm1701MS:txtRDOCodea",
                "frm1701MS:txtTaxpayerNo8a",
                "frm1701MS:Pg2TaxPayer",
            ][..],
        ),
    ] {
        let opened = opened(code);
        let shown: BTreeSet<String> = drawn(code).into_iter().map(|(_, _, key)| key).collect();
        let hidden: BTreeSet<String> = items(code)
            .into_iter()
            .flat_map(|(_, item)| item["f"].as_array().cloned().unwrap_or_default())
            .filter(|field| field["t"] == "hidden")
            .filter_map(|field| field["k"].as_str().map(str::to_string))
            .collect();
        let edit = strings(&opened["profile_edit"]);
        for key in keys {
            let key = key.to_string();
            if shown.contains(&key) {
                assert!(edit.contains(&key), "{code}: {key} offers Edit");
            } else {
                assert!(
                    hidden.contains(&key),
                    "{code}: {key} is neither drawn nor a copy"
                );
            }
        }
    }
}

/// R3.1-R3.4, R3.6 (1701), R3.0-R3.2 (1701A), R3.0-R3.3 (1701MS): no drawn
/// box sits in an unnumbered "Additional details" group; every drawn item
/// carries its printed number, a table's rows, or (an unnumbered line the
/// sheet prints, like 1701's overpayment boxes) a caption of the sheet.
#[test]
fn annual_forms_have_no_additional_details_groups() {
    setup();
    for (code, slug) in [
        ("1700", "1700-2018"),
        ("1701", "1701-2018"),
        ("1701A", "1701a-2018"),
        ("1701MS", "1701ms-2024"),
    ] {
        let text = sheet_text(slug);
        for (part, item) in items(code) {
            let boxes = item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["t"] != "hidden")
                .count()
                + item["keys"].as_array().map_or(0, Vec::len);
            if boxes == 0 {
                continue;
            }
            let label = item["l"].as_str().unwrap_or_default();
            assert_ne!(label, "Additional details", "{code} {part}");
            if item["no"].as_str().unwrap_or_default().is_empty()
                && item["keys"].is_null()
                && part != "Not on the printed sheet"
            {
                assert!(
                    text.contains(&normal(label)),
                    "{code} {part}: unnumbered {label:?} is not the sheet's text"
                );
            }
        }
    }
}

/// R3.11 (1701): Part III's items 34-37 are drawn with their printed
/// captions and a note instead of boxes. The official program disables
/// their sixteen boxes at load (rules/forms/1701-v2018/fixtures/
/// runtime-control-inventory-v796.json, disabled true), so the upload
/// files them blank as it does, and the page offers none of them.
#[test]
fn form_1701_details_of_payment_show_a_note() {
    setup();
    let part = schema_entry("1701").unwrap()["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["n"] == "Part III · Details of Payment")
        .expect("Part III");
    let shown: Vec<(&str, &str)> = part["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|it| {
            assert!(
                it["note"]
                    .as_str()
                    .is_some_and(|n| n.contains("keeps this item shut"))
            );
            assert_eq!(it["f"], json!([]));
            (it["no"].as_str().unwrap(), it["l"].as_str().unwrap())
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("34", "Cash/Bank Debit Memo"),
            ("35", "Check"),
            ("36", "Tax Debit Memo"),
            ("37", "Others (specify below)"),
        ]
    );
    let opened = opened("1701");
    let fields = field_map("1701", &opened["draft"]);
    let drawn: BTreeSet<String> = drawn("1701").into_iter().map(|(_, _, key)| key).collect();
    for key in [
        "frm1701:txtPg1I34Agency",
        "frm1701:txtPg1I34Number",
        "frm1701:txtPg1I34Date",
        "frm1701:txtPg1I34Amount",
        "frm1701:txtPg1I35Agency",
        "frm1701:txtPg1I35Date",
        "frm1701:txtPg1I35Amount",
        "frm1701:txtPg1I36Number",
        "frm1701:txtPg1I36Date",
        "frm1701:txtPg1I36Amount",
        "frm1701:txtPg1I37Particular",
        "frm1701:txtPg1I37Agency",
        "frm1701:txtPg1I37Number",
        "frm1701:txtPg1I37Date",
        "frm1701:txtPg1I37Amount",
    ] {
        assert!(!drawn.contains(key), "{key} has no box");
        assert_eq!(
            fields.get(key).map(String::as_str).unwrap_or_default(),
            "",
            "{key} files blank"
        );
    }
}

/// The part and item a drawn key sits in.
fn place(code: &str, key: &str) -> (String, String) {
    drawn(code)
        .into_iter()
        .find(|(_, _, k)| k == key)
        .map(|(part, no, _)| (part, no))
        .unwrap_or_else(|| panic!("{code}: {key} drawn"))
}

/// manual-3, manual-4, manual-5 (1701): items 9A and 12A sit in Part I and
/// Part IV only, every Schedule 6 NOLCO box in Schedule 6, and Part VI's
/// and Part VIII's boxes in their own parts.
#[test]
fn form_1701_parts_hold_only_their_own_boxes() {
    setup();
    assert_eq!(
        place("1701", "frm1701:txtPg1I9AZipCode"),
        (
            "Part I · Background Information of Taxpayer/Filer".into(),
            "9A".into()
        )
    );
    assert_eq!(
        place("1701", "frm1701:rdoPg2I12AMethodDeductionI"),
        (
            "Part IV · Background Information of Spouse".into(),
            "12A".into()
        )
    );
    for (part, _, key) in drawn("1701") {
        let tail = key.trim_start_matches("frm1701:");
        if tail.starts_with("txtPg3IShed6_") || tail.starts_with("txtPg4IShed6_") {
            assert!(part.contains("Schedule 6"), "{key} in {part}");
        }
        if tail.starts_with("txtPg4ISc6_") {
            assert!(part.starts_with("Part VI "), "{key} in {part}");
        }
        if tail.starts_with("txtPg4IPart8_") {
            assert!(part.starts_with("Part VIII "), "{key} in {part}");
        }
        if tail.starts_with("txtPg4IPart7_") {
            assert!(part.starts_with("Part VII "), "{key} in {part}");
        }
        if tail.starts_with("txtPg3IShed4_") {
            assert!(part.contains("Schedule 4"), "{key} in {part}");
        }
    }
}

/// R2.4 (1701), R2.3 (1701A): `txtZIP` is the official program's hidden
/// workflow control (rules/forms/1701-v2018/fields.json and
/// 1701a-v2018/fields.json record it with no page and no item; neither
/// sheet's writer cells print it), so the page does not draw it; the
/// profile's ZIP shows, files and prints in item 9A.
#[test]
fn annual_zip_shows_in_item_9a_not_the_hidden_control() {
    setup();
    for (code, zip) in [
        ("1701", "frm1701:txtPg1I9AZipCode"),
        ("1701A", "frm1701A:txtZipCode"),
    ] {
        let hidden = format!("frm{code}:txtZIP");
        assert!(
            drawn(code).iter().all(|(_, _, key)| *key != hidden),
            "{code}"
        );
        assert_eq!(place(code, zip).1, "9A", "{code}");
        let opened = opened(code);
        assert_eq!(opened["values"][zip], "1100", "{code}: from the profile");
        assert_eq!(field_map(code, &opened["draft"])[zip], "1100", "{code}");
        assert_eq!(printed(code, &opened["draft"])[zip], "1100", "{code}");
    }
}

/// R5.3 (1701), R5.0 (1701MS): each TIN is one item of its four boxes
/// (3-3-3-5), which the page draws as one TIN comb (form_page.js
/// `isTinItem`: an item naming the TIN with three to five text boxes).
#[test]
fn annual_tins_are_one_comb_each() {
    setup();
    for (code, tins) in [
        (
            "1701",
            &[
                [
                    "frm1701:txtPg1I4TIN1",
                    "frm1701:txtPg1I4TIN2",
                    "frm1701:txtPg1I4TIN3",
                    "frm1701:txtPg1I4BranchCode",
                ],
                [
                    "frm1701:txtPg2I1TIN1",
                    "frm1701:txtPg2I1TIN2",
                    "frm1701:txtPg2I1TIN3",
                    "frm1701:txtPg2I1BranchCode",
                ],
            ][..],
        ),
        (
            "1701A",
            &[
                [
                    "frm1701A:txtTIN1",
                    "frm1701A:txtTIN2",
                    "frm1701A:txtTIN3",
                    "frm1701A:txtBranchCode",
                ],
                [
                    "frm1701A:txtSpouseTIN1",
                    "frm1701A:txtSpouseTIN2",
                    "frm1701A:txtSpouseTIN3",
                    "frm1701A:txtSpouseBranchCode",
                ],
            ][..],
        ),
        (
            "1701MS",
            &[
                [
                    "frm1701MS:txtPg1TIN1",
                    "frm1701MS:txtPg1TIN2",
                    "frm1701MS:txtPg1TIN3",
                    "frm1701MS:txtPg1BranchCode",
                ],
                [
                    "frm1701MS:txtPg2TIN1Spouse",
                    "frm1701MS:txtPg2TIN2Spouse",
                    "frm1701MS:txtPg2TIN3Spouse",
                    "frm1701MS:txtPg2BranchCodeSpouse",
                ],
            ][..],
        ),
    ] {
        for tin in tins {
            let (_, item) = items(code)
                .into_iter()
                .find(|(_, it)| {
                    it["f"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|f| f["k"] == tin[0])
                })
                .unwrap_or_else(|| panic!("{code}: {} drawn", tin[0]));
            let fields: Vec<&Value> = item["f"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|f| f["t"] != "hidden")
                .collect();
            let keys: Vec<&str> = fields.iter().filter_map(|f| f["k"].as_str()).collect();
            assert_eq!(keys, tin.to_vec(), "{code}: one item holds the TIN");
            assert!(fields.iter().all(|f| f["t"] == "text"), "{code}");
            assert!(
                item["l"]
                    .as_str()
                    .unwrap()
                    .contains("Taxpayer Identification Number"),
                "{code}"
            );
        }
    }
}

/// R7.0, manual-3 (1701MS): the names and item 14/15 entries are text,
/// kept exactly as typed apart from the sheet's capitals.
#[test]
fn form_1701ms_names_are_text() {
    setup();
    let types: BTreeMap<String, String> = items("1701MS")
        .into_iter()
        .flat_map(|(_, it)| it["f"].as_array().cloned().unwrap_or_default())
        .filter_map(|f| Some((f["k"].as_str()?.to_string(), f["t"].as_str()?.to_string())))
        .collect();
    for key in [
        "frm1701MS:txtTaxpayerNo8a",
        "frm1701MS:txtTaxpayerNo8b",
        "frm1701MS:txtTaxpayerNo14a",
        "frm1701MS:txtTaxpayerNo14b",
        "frm1701MS:txtTaxpayerNo15a",
        "frm1701MS:txtTaxpayerNo15b",
    ] {
        assert_eq!(types[key], "text", "{key}");
    }
    assert_eq!(
        types["frm1701MS:Pg2TaxPayer"], "hidden",
        "page 2's copy of item 8"
    );
    let opened = opened("1701MS");
    let mut edits = joint("1701MS");
    edits["frm1701MS:txtTaxpayerNo8b"] = json!("Dela Cruz, Maria 12");
    let filled = take("1701MS", &opened["draft"], edits);
    assert_eq!(
        filled["values"]["frm1701MS:txtTaxpayerNo8b"],
        "DELA CRUZ, MARIA 12"
    );
}

/// R8.0 (1701MS): items 28A-28C of both columns are the shared model's
/// computed amounts, locked until Edit penalties, under their printed
/// captions; item 28D is computed.
#[test]
fn form_1701ms_penalties_are_computed() {
    setup();
    let single = opened("1701MS");
    let joint = take("1701MS", &single["draft"], joint("1701MS"));
    save("1701MS", &joint["draft"]);
    let joint = opened("1701MS");
    // Column B is the spouse's on a joint return.
    for (column, opened) in [("a", &single), ("b", &joint)] {
        // The official keys, Surchange and Compromise28 as spelled.
        for key in [
            format!("frm1701MS:txtSurchangeNo28{column}"),
            format!("frm1701MS:txtInterestNo28{column}"),
            format!("frm1701MS:txtCompromise28{column}"),
        ] {
            assert_eq!(opened["locks"][&key], penalties::LOCK, "{key}");
        }
        let total = format!("frm1701MS:txtTotalPenaltiesNo28{column}");
        let refused = fill("1701MS", &opened["draft"], json!({ &total: "5" }));
        assert!(
            refused["rejected"][&total].is_string(),
            "{total} is computed"
        );
    }
    let captions: Vec<String> = items("1701MS")
        .into_iter()
        .filter(|(_, it)| it["no"].as_str().is_some_and(|no| no.starts_with("28")))
        .map(|(_, it)| it["l"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(captions.len(), 4, "{captions:?}");
    assert_eq!(captions[0], "Add: Penalties – Surcharge");
}

/// R2.3 (1701): Part IV item 6, the spouse's contact number, is the
/// filer's entry on a joint return: it files and prints as typed.
#[test]
fn form_1701_spouse_contact_is_typed() {
    setup();
    let opened = opened("1701");
    let key = "frm1701:txtPg2I6TelNum";
    assert!(opened["locks"][key].is_null());
    let mut edits = joint("1701");
    edits[key] = json!("09170000001");
    let filled = take("1701", &opened["draft"], edits);
    assert_eq!(field_map("1701", &filled["draft"])[key], "09170000001");
    assert_eq!(printed("1701", &filled["draft"])[key], "09170000001");
}

/// Lesson 6 with a literal fixture: `fixtures/1701-pre-pr7.json` is a 1701
/// draft exactly as pr0b-push serialized it (built and written by the
/// pr0b-push bir-core: no `penalty` / `spouse_penalty` keys, Items 27-29
/// typed as 125.00, 0.00 and 1,000.00). Written into a row's `data_json` as
/// those bytes, opened on the page and saved again, the column holds the
/// fixture byte for byte, and the page shows the typed amounts.
#[test]
fn a_1701_draft_stored_before_pr7_saves_back_byte_for_byte() {
    use bir_core::forms::form_1701::Form1701Draft;
    use bir_core::forms::queueable::QueueableForm;
    setup();
    let fixture = include_str!("fixtures/1701-pre-pr7.json");
    let stored: Form1701Draft = serde_json::from_str(fixture).unwrap();
    // No key of the pre-PR 7 shape is one today's struct drops: read and
    // written back, every key and value is there.
    let literal: Value = serde_json::from_str(fixture).unwrap();
    assert_eq!(serde_json::to_value(&stored).unwrap(), literal);
    let (tin, year, period) = (
        stored.tin.clone(),
        stored.taxable_year,
        stored.period_column(),
    );
    let id = with_database(|db| {
        db.save_queueable_draft(&stored)
            .map_err(|error| HostError::new(error.to_string()))
    })
    .unwrap();
    // The row holds the fixture's own bytes, as the old code wrote a saved
    // draft: with its row id (the fixture was written before any save).
    let row_bytes = fixture.replacen("\"id\":null", &format!("\"id\":{id}"), 1);
    assert_ne!(row_bytes, fixture);
    super::raw_exec(
        "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
        &[&row_bytes, &id.to_string()],
    );
    let data_json = || super::raw_row(id).1;
    assert_eq!(data_json(), row_bytes, "stored as the old code stored it");
    let opened = open_for(&tin, "1701", i64::from(year), period);
    assert_eq!(opened["values"]["frm1701:txtPg1I27A"], "125.00");
    assert_eq!(opened["values"]["frm1701:txtPg1I29A"], "1,000.00");
    save("1701", &opened["draft"]);
    // A save records the profile baseline (PR 0c), the last key of the row;
    // set aside, the row is the old bytes exactly.
    let saved = data_json();
    let saved = match saved.find(",\"profile_baseline\":") {
        Some(at) => {
            let baseline: Value = serde_json::from_str(&saved[at + 20..saved.len() - 1]).unwrap();
            assert!(baseline.is_object(), "the baseline is the row's last key");
            format!("{}}}", &saved[..at])
        }
        None => saved,
    };
    assert_eq!(saved, row_bytes, "opened and saved, byte for byte");
}

/// Maintainer decision D6 (2026-10-18): a message names the printed item
/// number. The oracle is the frozen sheet's own captions
/// (html-frozen/<slug>/writer-cells.json, read by the rule 6 tag check's
/// `printed_items`), not the page schema: for each item an official message
/// cites (on a fresh return and a joint one, where most of their checks
/// speak), one of the boxes the message sits on prints under that item, and
/// in the part the message names when its caption names a part ("2 RDO
/// Code (Part IV)", "Part II 19 ...", "Schedule 1 Item 3a ...", "5.A item
/// 1"). A message on boxes the sheet does not caption still cites an item
/// the sheet prints. None of the four forms' messages needed rewording.
#[test]
fn annual_messages_cite_printed_item_numbers() {
    setup();
    for (code, slug) in [
        ("1700", "1700-2018"),
        ("1701", "1701-2018"),
        ("1701A", "1701a-2018"),
        ("1701MS", "1701ms-2024"),
    ] {
        let sheet = super::conditions::tests::printed_items(code);
        let captions = captions(slug);
        let fresh = opened(code);
        let joint = take(code, &fresh["draft"], joint(code));
        let (mut keyed, mut unkeyed) = (0, 0);
        for draft in [&fresh["draft"], &joint["draft"]] {
            let errors = validated(code, draft)["errors"].clone();
            for error in errors.as_array().into_iter().flatten() {
                let message = error["message"].as_str().unwrap_or_default();
                let printed: Vec<(Option<String>, String)> = strings(&error["keys"])
                    .iter()
                    .filter_map(|key| captions.get(key))
                    .filter_map(|caption| caption_place(caption))
                    .collect();
                // The first item cited is the one the message is about (its
                // boxes); a later one names a condition ("Item 12 is
                // required if Item 5 is ..."), which the sheet prints too.
                for (nth, (part, item)) in cited(message).into_iter().enumerate() {
                    // "26A" is item 26's column A where the sheet numbers
                    // the item alone.
                    let column = item
                        .strip_suffix(['A', 'B', 'a', 'b'])
                        .unwrap_or(&item)
                        .to_ascii_uppercase();
                    let item_up = item.to_ascii_uppercase();
                    let same = |no: &String| *no == item_up || *no == column;
                    if printed.is_empty() || nth > 0 {
                        assert!(
                            sheet.contains(&item_up) || sheet.contains(&column),
                            "{code}: {message:?} cites item {item}, which the sheet does not print"
                        );
                        unkeyed += 1;
                        continue;
                    }
                    let found = printed.iter().any(|(caption_part, no)| {
                        same(no)
                            && match (&part, caption_part) {
                                (Some(named), Some(printed)) => named == printed,
                                _ => true,
                            }
                    });
                    assert!(
                        found,
                        "{code}: {message:?} cites {part:?} item {item}; its boxes print as {printed:?}"
                    );
                    keyed += 1;
                }
            }
        }
        assert!(
            keyed > 0,
            "{code}: no cited item checked on its boxes ({unkeyed} unkeyed)"
        );
    }
}

/// The part (when the caption names one) and the item a writer-cells
/// caption prints a box under.
fn caption_place(caption: &str) -> Option<(Option<String>, String)> {
    let mut part = None;
    let mut rest = caption.trim();
    if let Some(tail) = rest.strip_prefix("Part ") {
        let (name, after) = tail.split_once(' ')?;
        part = Some(format!("Part {name}"));
        rest = after;
    } else if let Some(tail) = rest.strip_prefix("Schedule ") {
        let (name, after) = tail.split_once(' ')?;
        part = Some(format!("Schedule {name}"));
        rest = after.strip_prefix("Item ").unwrap_or(after);
    } else if let Some((schedule, after)) = rest.split_once(" item ")
        && schedule.contains('.')
    {
        // "5.A item 2, Description": Schedule 5's item 2.
        part = Some(format!("Schedule {}", schedule.split('.').next()?));
        rest = after;
    }
    if part.is_none()
        && let Some(open) = caption.rfind("(Part ")
    {
        part = Some(caption[open + 1..].trim_end_matches(')').to_string());
    }
    let item: String = rest
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_uppercase();
    item.starts_with(|c: char| c.is_ascii_digit())
        .then_some((part, item))
}

/// `(part or schedule named, item)` for each "Item N" a message cites.
fn cited(message: &str) -> Vec<(Option<String>, String)> {
    let mut out = Vec::new();
    let words: Vec<&str> = message.split_whitespace().collect();
    for (at, word) in words.iter().enumerate() {
        if !word.eq_ignore_ascii_case("item") {
            continue;
        }
        let mut next = words.get(at + 1).copied().unwrap_or_default();
        if next.eq_ignore_ascii_case("no.") {
            next = words.get(at + 2).copied().unwrap_or_default();
        }
        let item: String = next
            .trim_start_matches('#')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if !item.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let part = (at >= 2)
            .then(|| (words[at - 2], words[at - 1]))
            .filter(|(kind, _)| *kind == "Part" || *kind == "Schedule")
            .map(|(kind, name)| format!("{kind} {name}"));
        out.push((part, item));
    }
    out
}

// ------------------------------------------------------- PR 7 review

/// 1701MS item 18's II012 is "Business Income-Graduated IT Rates"
/// (html-frozen/1701ms-2024/guide.html): in either column it puts the
/// column on graduated rates, and the business tax files at those rates.
/// Oracle: the 2023-onwards graduated table (guide.html Table 2) on
/// P600,000 sales with the 40% OSD: P360,000 taxable, 15% of the P110,000
/// over P250,000 = P16,500.
#[test]
fn form_1701ms_ii012_is_graduated_in_both_columns() {
    setup();
    let opened = opened("1701MS");
    let mut edits = joint("1701MS");
    edits["taxCodeDropdown"] = json!("II012");
    edits["taxCodeDropdownSpouse"] = json!("II012");
    let chosen = take("1701MS", &opened["draft"], edits);
    for column in ["taxpayer", "spouse"] {
        assert_eq!(
            chosen["draft"][column]["tax_option"], "graduated",
            "{column}"
        );
        assert_eq!(chosen["draft"][column]["source"], "business", "{column}");
    }
    let typed = take(
        "1701MS",
        &chosen["draft"],
        json!({
            "frm1701MS:txtSalesRevenueNo5a": "600000",
            "frm1701MS:txtSalesRevenueNo5b": "600000",
            "frm1701MS:txtTaxpayerNo17b": "true",
            "frm1701MS:txtSpouseNo17b": "true",
        }),
    );
    let fields = field_map("1701MS", &typed["draft"]);
    for key in [
        "frm1701MS:txtIncomeTaxRegularNo22a",
        "frm1701MS:txtIncomeTaxRegularNo22b",
    ] {
        assert_eq!(
            agent::parse_amount(&fields[key]),
            Some(16_500.0),
            "{key}: {}",
            fields[key]
        );
    }
}

/// A 1701 draft stored before the penalty model (no `penalty` key): Edit
/// penalties, type item 29A 1,000 and it is taken, files and joins the
/// model (form_1701_official.rs `official_recompute`).
#[test]
fn form_1701_legacy_draft_takes_a_typed_penalty() {
    setup();
    let fixture: Value = serde_json::from_str(include_str!("fixtures/1701-pre-pr7.json")).unwrap();
    assert!(fixture.get("penalty").is_none());
    let keys = [
        "frm1701:txtPg1I27A",
        "frm1701:txtPg1I28A",
        "frm1701:txtPg1I29A",
    ];
    let typed = fill_unlocked(
        "1701",
        &fixture,
        json!({"frm1701:txtPg1I29A": "750"}),
        &keys,
    );
    assert_eq!(typed["rejected"], json!({}));
    // Not the 1,000.00 it held: the typed 750.00 shows and files.
    assert_eq!(typed["values"]["frm1701:txtPg1I29A"], "750.00");
    let fields = field_map("1701", &typed["draft"]);
    assert_eq!(fields["frm1701:txtPg1I29A"], "750.00");
    // The amounts it held stay as entered.
    assert_eq!(fields["frm1701:txtPg1I27A"], "125.00");
    assert_eq!(typed["draft"]["penalty"]["penalty_model_version"], 1);
}

/// `make_room` gives a penalty state only to a column that has none: a
/// current draft's state, overrides and all, is left exactly as it is.
#[test]
fn make_room_leaves_a_current_penalty_state_alone() {
    setup();
    let opened = opened("1701");
    let keys = [
        "frm1701:txtPg1I27A",
        "frm1701:txtPg1I28A",
        "frm1701:txtPg1I29A",
    ];
    let typed = fill_unlocked(
        "1701",
        &opened["draft"],
        json!({"frm1701:txtPg1I29A": "500"}),
        &keys,
    );
    let mut draft = typed["draft"].clone();
    assert_eq!(draft["penalty"]["penalty_model_version"], 1);
    let before = draft.clone();
    let kind = kind_of("1701").unwrap();
    for key in keys.iter().chain(&["frm1701:txtPg1I29B"]) {
        rows_annual_individual::make_room(kind, &mut draft, key);
    }
    assert_eq!(draft, before);
}

/// Lesson 8 on the other annual forms: "Use the computed amounts" over a
/// typed penalty asks first ("replaces what you typed"), as 1701 does.
#[test]
fn using_the_computed_penalties_asks_first_on_every_annual_form() {
    setup();
    for code in ["1700", "1701A", "1701MS"] {
        let opened = opened(code);
        let group = opened["penalties"]
            .as_array()
            .unwrap()
            .first()
            .cloned()
            .unwrap_or_else(|| panic!("{code}: a penalty group"));
        let keys: Vec<String> = strings(&group["keys"]).into_iter().collect();
        let open: Vec<&str> = keys.iter().map(String::as_str).collect();
        let typed = fill_unlocked(code, &opened["draft"], json!({ &keys[0]: "500" }), &open);
        assert_eq!(typed["rejected"], json!({}), "{code}");
        assert_eq!(typed["removes"], Value::Null, "{code}");
        let blanks: Map<String, Value> = keys.iter().map(|k| (k.clone(), json!(""))).collect();
        let back = fill_unlocked(code, &typed["draft"], Value::Object(blanks), &open);
        let removes = &back["removes"];
        assert_eq!(removes["rows"], 1, "{code}: {removes}");
        assert!(
            removes["message"]
                .as_str()
                .is_some_and(|m| m.starts_with("This answer replaces what you typed in ")),
            "{code}: {removes}"
        );
        assert_eq!(removes["done"], "Removed 1 entry", "{code}");
    }
}

/// 1701MS item 16, each column's tax rate, takes a rate only while its
/// item 12 is Exempt or Special/Preferential Rate (column B also on a joint
/// return): the form zeroes it otherwise, so the host refuses it and says
/// why, and ending the special rate asks before it clears a typed rate.
#[test]
fn form_1701ms_tax_rates_follow_item_12() {
    setup();
    let opened = opened("1701MS");
    let refused = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtTaxRateb": "5%"}),
    );
    assert_eq!(
        refused["rejected"]["frm1701MS:txtTaxRateb"],
        "Column B of Part I is for a joint return: choose Married at item 4 and Jointly at item 5 first."
    );
    let refused = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtTaxRatea": "5%"}),
    );
    assert_eq!(
        refused["rejected"]["frm1701MS:txtTaxRatea"],
        "Item 16 column A is the rate of an exempt or special/preferential rate income: choose Exempt or Special/Preferential Rate at item 12 column A first."
    );
    assert_eq!(
        opened["off"]["frm1701MS:txtTaxRatea"],
        "Needs item 12 = Exempt or Special/Preferential Rate in column A"
    );
    assert_eq!(opened["values"]["frm1701MS:txtTaxRatea"], "");
    // One batch: Special at item 12, then the rate.
    let special = take(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12e": "true",
            "frm1701MS:txtTaxRatea": "5%",
        }),
    );
    assert_eq!(special["draft"]["taxpayer"]["special_rate"], 5.0);
    let graduated = take(
        "1701MS",
        &special["draft"],
        json!({"frm1701MS:txtTaxpayerNo12b": "true"}),
    );
    assert!(
        graduated["removes"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("item 16")),
        "{}",
        graduated["removes"]
    );
}

/// Item 20's gate reads the batch as it stands, recomputed: one batch that
/// makes item 19A positive (special rate, its rate and sales) and types
/// item 20A is taken whole.
#[test]
fn form_1701ms_item_20_opens_within_one_batch() {
    setup();
    let opened = opened("1701MS");
    let filled = take(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12e": "true",
            "frm1701MS:txtTaxRatea": "5",
            "frm1701MS:txtSalesRevenueNo5a": "1000000",
            "frm1701MS:txtShareOfOtherNo20a": "100",
        }),
    );
    assert_eq!(
        filled["draft"]["taxpayer"]["share_of_other_agencies"],
        100.0
    );
}

/// An imported 1701 joint draft stored before PR 7's review keeps its
/// spouse's RDO only in `spouse.rdo_code` (the upload filed 000). Nothing
/// is changed: page, print and upload agree on what files, and the box
/// carries a notice saying the imported RDO is not on the filing.
#[test]
fn an_imported_1701_spouse_rdo_off_the_filing_has_a_notice() {
    setup();
    let key = "frm1701:txtPg2I2SpouseRDOCode";
    let opened = opened("1701");
    let joint = take("1701", &opened["draft"], joint("1701"));
    let mut legacy = joint["draft"].clone();
    legacy["spouse"]["rdo_code"] = json!("040");
    legacy["spouse_rdo_code"] = json!("");
    let checked = validated("1701", &legacy);
    assert_eq!(checked["values"][key], "");
    assert_eq!(printed("1701", &checked["draft"])[key], "");
    assert_eq!(field_map("1701", &checked["draft"])[key], "000");
    assert_eq!(
        checked["draft"]["spouse"]["rdo_code"], "040",
        "no automatic change"
    );
    let notice = checked["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| {
            e["field"] == "spouse_rdo_code"
                && strings(&e["keys"]).contains(key)
                && e["message"]
                    .as_str()
                    .is_some_and(|m| m.starts_with("Imported"))
        })
        .unwrap_or_else(|| panic!("a notice: {}", checked["errors"]));
    assert_eq!(
        notice["message"],
        "Imported spouse RDO 040 isn't on the filing; check it."
    );
    // Once the RDO is typed, the notice goes.
    let typed = take("1701", &legacy, json!({ key: "040" }));
    let rechecked = validated("1701", &typed["draft"]);
    assert!(
        rechecked["errors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["message"] != notice["message"])
    );
}

/// Lesson 8 for "Use the computed amounts": it replaces penalty amounts
/// the filer typed or kept, so it asks first, on a legacy draft's kept
/// amounts and on a current draft's overrides alike.
#[test]
fn using_the_computed_penalties_asks_first() {
    setup();
    let keys = [
        "frm1701:txtPg1I27A",
        "frm1701:txtPg1I28A",
        "frm1701:txtPg1I29A",
    ];
    let blank = json!({
        "frm1701:txtPg1I27A": "",
        "frm1701:txtPg1I28A": "",
        "frm1701:txtPg1I29A": "",
    });
    // Kept as entered: 125.00 and 1,000.00 (the 0.00 says nothing).
    let fixture: Value = serde_json::from_str(include_str!("fixtures/1701-pre-pr7.json")).unwrap();
    let kept = fill_unlocked("1701", &fixture, blank.clone(), &keys);
    assert_eq!(kept["rejected"], json!({}));
    assert_eq!(
        kept["removes"],
        json!({
            "message": "This answer replaces what you typed in Part II item 27 and Part II item 29. Continue?",
            "done": "Removed 2 entries",
            "rows": 2,
        })
    );
    // A current draft's override.
    let opened = opened("1701");
    let typed = fill_unlocked(
        "1701",
        &opened["draft"],
        json!({"frm1701:txtPg1I29A": "500"}),
        &keys,
    );
    assert_eq!(typed["removes"], Value::Null);
    let back = fill_unlocked("1701", &typed["draft"], blank, &keys);
    assert_eq!(
        back["removes"],
        json!({
            "message": "This answer replaces what you typed in Part II item 29. Continue?",
            "done": "Removed 1 entry",
            "rows": 1,
        })
    );
    // Blanks over computed amounts ask nothing.
    let quiet = fill_unlocked(
        "1701",
        &opened["draft"],
        json!({"frm1701:txtPg1I29A": ""}),
        &keys,
    );
    assert_eq!(quiet["removes"], Value::Null);
}

/// 1701MS column B (item 18, item 12's radios and its amounts) is faded
/// off a joint return and refuses a write; item 20 of each column opens
/// once that column's item 19 is more than zero.
#[test]
fn form_1701ms_column_b_and_item_20_follow_their_answers() {
    setup();
    let opened = opened("1701MS");
    let joint_refusal = "Column B of Part I is for a joint return: choose Married at item 4 and Jointly at item 5 first.";
    for key in [
        "taxCodeDropdownSpouse",
        "frm1701MS:txtTaxpayerNo12b1",
        "frm1701MS:txtSalesRevenueNo5b",
        "frm1701MS:txtCreditableForm2316No5b",
        "frm1701MS:txtShareOfOtherNo20b",
    ] {
        assert_eq!(
            opened["off"][key]
                .as_str()
                .map(|tag| tag.starts_with("Needs")),
            Some(true),
            "{key}: {}",
            opened["off"][key]
        );
        let value = if key == "taxCodeDropdownSpouse" {
            "II012"
        } else if key.contains("No12") {
            "true"
        } else {
            "1000"
        };
        let refused = fill("1701MS", &opened["draft"], json!({ key: value }));
        assert_eq!(refused["rejected"][key], joint_refusal, "{key}");
        assert_eq!(refused["draft"], opened["draft"], "{key}: nothing written");
    }
    // Item 20A: closed while item 19A is zero (graduated rates).
    assert_eq!(
        opened["off"]["frm1701MS:txtShareOfOtherNo20a"],
        "Needs a special-rate income tax due above zero in column A"
    );
    let refused = fill(
        "1701MS",
        &opened["draft"],
        json!({"frm1701MS:txtShareOfOtherNo20a": "100"}),
    );
    assert_eq!(
        refused["rejected"]["frm1701MS:txtShareOfOtherNo20a"],
        "Item 20 column A is for a special or preferential rate tax due: it opens once item 19 column A is more than zero."
    );
    // A special rate with income: item 19A is positive and 20A takes it.
    let special = take(
        "1701MS",
        &opened["draft"],
        json!({
            "frm1701MS:txtTaxpayerNo11a": "true",
            "frm1701MS:txtTaxpayerNo12e": "true",
        }),
    );
    let special = take(
        "1701MS",
        &special["draft"],
        json!({
            "frm1701MS:txtTaxRatea": "5",
            "frm1701MS:txtSalesRevenueNo5a": "1000000",
        }),
    );
    let shared = take(
        "1701MS",
        &special["draft"],
        json!({"frm1701MS:txtShareOfOtherNo20a": "100"}),
    );
    assert_eq!(
        shared["draft"]["taxpayer"]["share_of_other_agencies"],
        100.0
    );
    // Ending the special rate would clear it: asked first.
    let graduated = take(
        "1701MS",
        &shared["draft"],
        json!({"frm1701MS:txtTaxpayerNo12b": "true"}),
    );
    assert!(
        graduated["removes"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("Part II item 20")),
        "{}",
        graduated["removes"]
    );
}

/// 1701MS item 18 column B on a joint return takes every code its list
/// offers, each setting only the spouse's source and tax option (the civil
/// status and filing stay as answered); a blank leaves the return joint.
#[test]
fn form_1701ms_spouse_atc_takes_every_listed_code_on_a_joint_return() {
    setup();
    let opened = opened("1701MS");
    let on = take("1701MS", &opened["draft"], joint("1701MS"));
    let field = items("1701MS")
        .into_iter()
        .flat_map(|(_, it)| it["f"].as_array().cloned().unwrap_or_default())
        .find(|f| f["k"] == "taxCodeDropdownSpouse")
        .expect("item 18 column B drawn");
    let codes: Vec<String> = field["o"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|option| {
            option["value"].as_str().map(str::to_string).or_else(|| {
                option
                    .as_str()
                    .map(|text| text.split(" - ").next().unwrap().to_string())
            })
        })
        .collect();
    assert!(codes.len() >= 7, "{codes:?}");
    for code in codes.iter().chain([&String::new()]) {
        let chosen = take(
            "1701MS",
            &on["draft"],
            json!({ "taxCodeDropdownSpouse": code }),
        );
        assert_eq!(chosen["draft"]["civil_status"], "married", "{code}");
        assert_eq!(chosen["draft"]["filing"], "jointly", "{code}");
    }
}

// ------------------------------------------------- 1701 preview.1

/// The 31 boxes 1701's preview.1 lists as printed constants
/// (docs/form-findings/1701/findings.json preview.1 names the first of
/// them; this list is the whole set, kept here as the oracle).
const PREVIEW_1: [&str; 31] = [
    "frm1701:txtPg2I9ForeignTaxNumber",
    "frm1701:txtPg2IShed3_20Desc",
    "frm1701:txtPg4IPart7_6A",
    "frm1701:txtPg4IPart7_6B",
    "frm1701:txtPg4IPart8_1A",
    "frm1701:txtPg4IPart8_1B",
    "frm1701:txtPg4IPart8_2A",
    "frm1701:txtPg4IPart8_2B",
    "frm1701:txtPg4IPart8_3A",
    "frm1701:txtPg4IPart8_3B",
    "frm1701:txtPg4IPart8_4A",
    "frm1701:txtPg4IPart8_4B",
    "frm1701:txtPg4IPart8_5A",
    "frm1701:txtPg4IPart8_5B",
    "frm1701:txtPg4IPart8_6A",
    "frm1701:txtPg4IPart8_6B",
    "frm1701:txtPg4IPart8_7A",
    "frm1701:txtPg4IPart8_7B",
    "frm1701:txtPg4IPart8_8A",
    "frm1701:txtPg4IPart8_8B",
    "frm1701:txtPg4IPart8_9A",
    "frm1701:txtPg4IPart8_9B",
    "frm1701:txtPg4IPart8_10A",
    "frm1701:txtPg4IPart8_10B",
    "frm1701:txtPg4IPart9_2Particulars",
    "frm1701:txtPg4IPart9_3Particulars",
    "frm1701:txtPg4IPart9_4Particulars",
    "frm1701:txtPg4IPart9_6Particulars",
    "frm1701:txtPg4IPart9_7Particulars",
    "frm1701:txtPg4IPart9_8Particulars",
    "frm1701:txtPg4IPart9_9Particulars",
];

/// Part VIII column A's typed items and its totals.
const PART_VIII_TYPED: [u8; 6] = [1, 2, 4, 6, 8, 9];
const PART_VIII_TOTALS: [u8; 4] = [3, 5, 7, 10];

fn part_viii_b(key: &str) -> bool {
    key.starts_with("frm1701:txtPg4IPart8_") && key.ends_with('B')
}

/// The same value as the screen-form audit's `compare` reads it: the same
/// amount, or the same text once case and spacing are set aside (the print
/// writes the editable-save map's 1100.00 and Sample Text where the page
/// shows 1,100.00 and SAMPLE TEXT).
fn same_shown(page: &str, sheet: &str) -> bool {
    match (agent::parse_amount(page), agent::parse_amount(sheet)) {
        (Some(a), Some(b)) => a == b,
        _ => {
            let norm = |t: &str| {
                t.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_uppercase()
            };
            norm(page) == norm(sheet)
        }
    }
}

/// A joint, amended return with the spouse claiming foreign tax credits:
/// every preview.1 box the official page opens is open.
fn preview_1_return() -> Value {
    let opened = opened("1701");
    let mut answers = joint("1701");
    answers["frm1701:rdoPg1I2AmendedYes"] = json!("true");
    answers["frm1701:rdoPg1I7ATC_II012"] = json!("true");
    answers["frm1701:rdoPg2I4ATC_II012"] = json!("true");
    answers["frm1701:rdoPg2I8ForeignTaxCreditsYes"] = json!("true");
    take("1701", &opened["draft"], answers)["draft"].clone()
}

/// 1701 preview.1: none of the 31 boxes is a constant the page and the
/// print disagree on. Each is bound to the draft, computed from boxes that
/// are, or (Part VIII column B) shut as the official page keeps it: on a
/// joint, amended return the page shows what the print prints, for typed
/// values and for blanks. Part VIII column A's items 1, 2, 4, 6, 8 and 9
/// are the filer's (the tracked runtime inventory records them enabled);
/// its items 3, 5, 7 and 10 follow the printed captions' operators ("Sum of
/// Items 1 and 2", "Item 3 Less Item 4", "Sum of Items 5 and 6", "Sum of
/// Items 8 and 9"), the oracle here; column B files 0.00.
#[test]
fn form_1701_preview_1_boxes_print_what_the_page_shows() {
    setup();
    let kind = kind_of("1701").unwrap();
    let def = kind.definition();
    for key in PREVIEW_1 {
        let reason = def.unmapped.get(key).map(String::as_str);
        if part_viii_b(key) {
            assert_eq!(
                reason,
                Some(bir_core::forms::agent::probe::SHUT_REASON),
                "{key}"
            );
        } else {
            assert_eq!(reason, None, "{key}");
        }
    }
    let on = preview_1_return();
    let mut typed = Map::new();
    let mut amount = 1000;
    for key in PREVIEW_1 {
        let total = PART_VIII_TOTALS
            .iter()
            .any(|n| *key == format!("frm1701:txtPg4IPart8_{n}A"));
        if total || part_viii_b(key) {
            continue;
        }
        let value = if key.contains("Part7_") || key.contains("Part8_") {
            amount += 100;
            json!(amount.to_string())
        } else {
            json!("Sample Text")
        };
        typed.insert(key.to_string(), value);
    }
    let filled = take("1701", &on, Value::Object(typed.clone()));
    let agree = |filled: &Value, blank: bool| {
        let print = printed("1701", &filled["draft"]);
        for key in PREVIEW_1 {
            let page = filled["values"][key].as_str().unwrap_or_default();
            let sheet = print.get(key).map(String::as_str).unwrap_or_default();
            assert!(
                same_shown(page, sheet),
                "{key}: page {page:?}, print {sheet:?}"
            );
            if part_viii_b(key) {
                assert_eq!(page, "0.00", "{key}: shut");
            } else if !blank {
                assert!(!says_nothing_here(page), "{key}: {page:?} typed");
            }
        }
    };
    agree(&filled, false);
    let value = |key: &str| agent::parse_amount(filled["values"][key].as_str().unwrap()).unwrap();
    let item = |n: u8| value(&format!("frm1701:txtPg4IPart8_{n}A"));
    assert_eq!(item(3), item(1) + item(2), "item 3");
    assert_eq!(item(5), item(3) - item(4), "item 5");
    assert_eq!(item(7), item(5) + item(6), "item 7");
    assert_eq!(item(10), item(8) + item(9), "item 10");
    // Typed values file as typed.
    let fields = field_map("1701", &filled["draft"]);
    assert_eq!(fields["frm1701:txtPg4IPart8_1A"], "1,300.00");
    assert_eq!(fields["frm1701:txtPg4IPart9_2Particulars"], "SAMPLE TEXT");
    // Blanks: page and print agree on them too. A blank amount is 0 (the
    // box takes no empty amount), a blank text "".
    let blanks: Map<String, Value> = typed
        .keys()
        .map(|k| {
            let amount = k.contains("Part7_") || k.contains("Part8_");
            (k.clone(), json!(if amount { "0" } else { "" }))
        })
        .collect();
    let cleared = take("1701", &filled["draft"], Value::Object(blanks));
    agree(&cleared, true);
}

fn says_nothing_here(text: &str) -> bool {
    text.trim().trim_matches(['0', '.', ',']).is_empty()
}

/// Part VIII column B stays shut on a joint return: a write is refused with
/// the shut reason and the column files 0.00.
#[test]
fn form_1701_part_viii_column_b_stays_shut_on_a_joint_return() {
    setup();
    let on = preview_1_return();
    let refused = fill("1701", &on, json!({"frm1701:txtPg4IPart8_1B": "500"}));
    assert_eq!(
        refused["rejected"]["frm1701:txtPg4IPart8_1B"]
            .as_str()
            .map(|why| why.ends_with(bir_core::forms::agent::probe::SHUT_REASON)),
        Some(true),
        "{}",
        refused["rejected"]
    );
    assert_eq!(refused["draft"], on);
    let fields = field_map("1701", &on);
    for n in 1..=10 {
        assert_eq!(
            fields[&format!("frm1701:txtPg4IPart8_{n}B")],
            "0.00",
            "{n}B"
        );
    }
}

/// Part VIII column A takes whole pesos, as the official entry filter has
/// it: a decimal or a minus sign is refused with the reason, its own
/// display ("1,500.00") is taken, and the totals are whole-peso sums;
/// item 5 goes negative when item 4 is more than item 3. The four totals
/// refuse a typed write.
#[test]
fn form_1701_part_viii_column_a_takes_whole_pesos() {
    setup();
    let on = preview_1_return();
    for (key, value) in [
        ("frm1701:txtPg4IPart8_1A", "1500.50"),
        ("frm1701:txtPg4IPart8_2A", "-100"),
    ] {
        let refused = fill("1701", &on, json!({ key: value }));
        let item = key
            .trim_start_matches("frm1701:txtPg4IPart8_")
            .trim_end_matches('A');
        assert_eq!(
            refused["rejected"][key],
            rows_annual_individual::whole_peso_refusal(item),
            "{key} {value}"
        );
    }
    let filled = take(
        "1701",
        &on,
        json!({
            "frm1701:txtPg4IPart8_1A": "1,500.00",
            "frm1701:txtPg4IPart8_2A": "250",
            "frm1701:txtPg4IPart8_4A": "2000",
            "frm1701:txtPg4IPart8_6A": "100",
        }),
    );
    let fields = field_map("1701", &filled["draft"]);
    assert_eq!(fields["frm1701:txtPg4IPart8_3A"], "1,750.00");
    assert_eq!(
        agent::parse_amount(&fields["frm1701:txtPg4IPart8_5A"]),
        Some(-250.0)
    );
    assert_eq!(
        agent::parse_amount(&fields["frm1701:txtPg4IPart8_7A"]),
        Some(-150.0)
    );
    for n in PART_VIII_TOTALS {
        let key = format!("frm1701:txtPg4IPart8_{n}A");
        let refused = fill("1701", &filled["draft"], json!({ &key: "5" }));
        assert!(refused["rejected"][&key].is_string(), "{key} is computed");
    }
    // Each typed item takes a whole amount.
    for n in PART_VIII_TYPED {
        let key = format!("frm1701:txtPg4IPart8_{n}A");
        let taken = fill("1701", &on, json!({ &key: "1200" }));
        assert!(taken["rejected"].get(&key).is_none(), "{key}");
    }
}

/// A joint 1701 draft stored with Part VIII amounts opens and saves back
/// as stored.
#[test]
fn form_1701_joint_part_viii_draft_round_trips() {
    setup();
    let on = preview_1_return();
    let filled = take(
        "1701",
        &on,
        json!({"frm1701:txtPg4IPart8_1A": "700", "frm1701:txtPg4IPart8_8A": "300"}),
    );
    let saved = save("1701", &filled["draft"]);
    let reopened = opened("1701");
    assert_eq!(reopened["draft"], saved["draft"], "opens as stored");
    let again = save("1701", &reopened["draft"]);
    assert_eq!(again["draft"], saved["draft"], "saves back as stored");
    assert_eq!(reopened["values"]["frm1701:txtPg4IPart8_1A"], "700.00");
    assert_eq!(reopened["values"]["frm1701:txtPg4IPart8_10A"], "300.00");
}

/// The PR 7 changes keep a stored 1701 draft's upload: the pre-PR 7
/// fixture, recomputed by today's code, files exactly the field map the
/// pr0b-push bir-core filed for it (`fixtures/1701-pre-pr7.fields.json`,
/// written by that code's `QueueableForm::field_map` after `recompute`),
/// Part VIII's 0.00 included.
#[test]
fn form_1701_stored_draft_files_as_before() {
    setup();
    let fixture: Value = serde_json::from_str(include_str!("fixtures/1701-pre-pr7.json")).unwrap();
    let before: BTreeMap<String, String> =
        serde_json::from_str(include_str!("fixtures/1701-pre-pr7.fields.json")).unwrap();
    let mut draft = fixture.clone();
    agent::compute(kind_of("1701").unwrap(), &mut draft).unwrap();
    let now = field_map("1701", &draft);
    let changed: Vec<&String> = before
        .keys()
        .chain(now.keys())
        .filter(|key| before.get(*key) != now.get(*key))
        .collect();
    assert!(changed.is_empty(), "{changed:?}");
    assert_eq!(now["frm1701:txtPg4IPart8_1A"], "0.00");
}

/// Decision P4-10: 1701 Part VIII item 6 stays the filer's entry; its item
/// carries the page's hint (`from`) naming Part VII item 8, whose amount the
/// page shows under it with "Use it" (tools/test_form_decisions_round_3.mjs
/// drives the page). Nothing is copied by itself: Part VII item 8's amount
/// leaves item 6 as it was, and "Use it" sends that amount as shown through
/// the ordinary fill, which takes it into item 6 and its totals. No other
/// item of any form carries a hint.
#[test]
fn form_1701_part_viii_item_6_offers_part_vii_item_8() {
    setup();
    let (from, to) = (
        rows_annual_individual::PART_VII_ITEM_8,
        rows_annual_individual::PART_VIII_ITEM_6,
    );
    let hinted: Vec<(String, Value)> = AgentFormKind::all()
        .into_iter()
        .flat_map(|kind| {
            let code = kind.form_code().to_string();
            items(&code)
                .into_iter()
                .filter(|(_, item)| !item["from"].is_null())
                .map(move |(_, item)| (code.clone(), item["from"].clone()))
        })
        .collect();
    assert_eq!(
        hinted,
        vec![(
            "1701".to_string(),
            json!({ "k": from, "to": to, "l": "From Part VII item 8" })
        )]
    );
    let (_, item) = items("1701")
        .into_iter()
        .find(|(_, item)| !item["from"].is_null())
        .unwrap();
    assert_eq!(item["no"], "6");
    assert_eq!(item["f"][0]["k"], to);
    let on = preview_1_return();
    let typed = take("1701", &on, json!({ from: "1500" }));
    assert_eq!(typed["values"][from], "1,500.00", "the hint's amount");
    assert_eq!(
        typed["values"][to], "0.00",
        "item 6 is not filled by itself"
    );
    // "Use it": the page sends the hint's amount as shown.
    let used = take("1701", &typed["draft"], json!({ to: "1,500.00" }));
    assert_eq!(used["values"][to], "1,500.00");
    assert_eq!(
        amount(&used["values"], "frm1701:txtPg4IPart8_7A"),
        amount(&used["values"], "frm1701:txtPg4IPart8_5A") + 1_500.0,
        "item 7 is the sum of items 5 and 6"
    );
    // A Part VII item 8 with centavos is refused in item 6 as any typed
    // amount there is: column A takes whole pesos.
    let cents = take("1701", &on, json!({ from: "1234.56" }));
    let refused = fill("1701", &cents["draft"], json!({ to: "1,234.56" }));
    assert_eq!(
        refused["rejected"][to],
        rows_annual_individual::whole_peso_refusal("6")
    );
}
