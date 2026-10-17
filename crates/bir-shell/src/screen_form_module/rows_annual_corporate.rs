//! PR 8: the annual corporate income tax returns (1702RT, 1702MX, 1702EX)
//! and 1707A. Boxes the host writes through the draft because the probed
//! bindings cannot say what the printed sheet says, the locks that say why a
//! box is shut, what a blank box waits for, and the answers that remove what
//! the filer typed.
//!
//! - 1702RT Item 10 and the Part III dates: the draft keeps each as a
//!   `Form1702RTDate` (year, month, day), which the probe cannot bind to one
//!   box. The page sends MM/DD/YYYY; the host keeps it as that date, an empty
//!   box clears it, and anything else is refused with what to type.
//! - 1707A Item 12: the draft keeps the answer as `tax_relief` (none for No,
//!   the Item 12A text for Yes), which the probe cannot bind to the two
//!   radios. Yes keeps what 12A holds (blank until typed, which validation
//!   asks for); No clears 12A, which the page asks first ([`removes`]).
//! - Computed boxes the probe left as "derived" say they are computed
//!   ([`relock`]); a blank one says what fills it ([`waits`]).
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use std::collections::BTreeMap;

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_1702rt::Form1702RTDate;
use serde_json::{Map, Value, json};

use super::set_draft_path;

const RT: &str = "frm1702RT:";
const EX: &str = "frm1702EX:";
const A: &str = "frm1707Av2021:";

/// The lock of a box the form computes (the page's note: "Computed by the
/// form.").
const COMPUTED: &str = "computed by BIR's own formulas";

/// 1702RT's date boxes and the draft field each keeps: Item 10 and Part III
/// rows 23 to 26 (`payment_details[0..4]`, form_1702rt_xml.rs
/// insert_payment_rows).
const RT_DATES: [(&str, &str); 5] = [
    ("txtPg1Pt1I10", "/incorporation_date"),
    ("txtPg1Pt3I23DebitMemoC3Date", "/payment_details/0/date"),
    ("txtPg1Pt3I24CheckC3Date", "/payment_details/1/date"),
    ("txtPg1Pt3I25TaxDebitDate", "/payment_details/2/date"),
    ("txtPg1Pt3I26OthersC3Date", "/payment_details/3/date"),
];

/// What a 1702RT date box says when it is not a date.
pub(super) const RT_DATE_REFUSAL: &str = "Enter the date as MM/DD/YYYY (a real calendar date).";

/// 1707A Item 12's radios: Yes and No.
const TAX_RELIEF_YES: &str = "frm1707Av2021:rdoI11TaxTreatyYes";
const TAX_RELIEF_NO: &str = "frm1707Av2021:rdoI11TaxTreatyNo";

fn rt_date_path(key: &str) -> Option<&'static str> {
    let tail = key.strip_prefix(RT)?;
    RT_DATES
        .iter()
        .find(|(stem, _)| *stem == tail)
        .map(|(_, path)| *path)
}

/// Whether the host writes `key` itself (the page shows it open).
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    match kind.form_code() {
        "1702RT" => rt_date_path(key).is_some(),
        "1707A" => key == TAX_RELIEF_YES || key == TAX_RELIEF_NO,
        _ => false,
    }
}

/// Write one of this module's boxes; `None` for any other box.
pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
) -> Option<Result<(), String>> {
    match kind.form_code() {
        "1702RT" => rt_date_path(key).map(|path| fill_1702rt_date(draft, path, value)),
        "1707A" if host_writes(kind, key) => Some(fill_1707a_tax_relief(draft, key, value)),
        _ => None,
    }
}

fn text_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.trim().to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// A 1702RT date box: MM/DD/YYYY keeps that date, blank clears it.
#[inline(never)]
fn fill_1702rt_date(draft: &mut Value, path: &str, value: &Value) -> Result<(), String> {
    let text = text_of(value);
    let date = if text.is_empty() {
        Value::Null
    } else {
        let date = Form1702RTDate::parse(&text).map_err(|_| RT_DATE_REFUSAL.to_string())?;
        serde_json::to_value(date).map_err(|error| error.to_string())?
    };
    set_draft_path(draft, path, date);
    Ok(())
}

fn ticked(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::String(text) => text.trim() == "true",
        _ => false,
    }
}

/// 1707A Item 12: Yes keeps (or opens, blank) the relief named in 12A; No
/// clears it. Unticking either radio alone changes nothing: the page ticks
/// the other one.
#[inline(never)]
fn fill_1707a_tax_relief(draft: &mut Value, key: &str, value: &Value) -> Result<(), String> {
    if !ticked(value) {
        return Ok(());
    }
    if key == TAX_RELIEF_NO {
        set_draft_path(draft, "/tax_relief", Value::Null);
    } else if !draft["tax_relief"].is_string() {
        set_draft_path(draft, "/tax_relief", Value::String(String::new()));
    }
    Ok(())
}

/// 1707A's printed Schedule 1 and 2 row 4 boxes and the continuation rows'
/// column E: what fills each (form_1707a.rs main_row and recompute).
fn a_row4_and_column_e() -> Vec<String> {
    let mut keys: Vec<String> = [
        "txtS1C1DateOfTransactionI4",
        "txtS1C2NameOfCorporateStockI4",
        "txtS1C3SellingPriceI4",
        "txtS1C4CostI4",
        "txtS1C5CapitalGainsI4",
        "txtS1C6TaxPaidI4",
        "txtS2C1DateOfTransactionI4",
        "txtS2C2NameOfCorporateStockI4",
        "txtS2C3SellingPriceI4",
        "txtS2C4CostI4",
        "txtS2C5CapitalLossI4",
    ]
    .iter()
    .map(|stem| format!("{A}{stem}"))
    .collect();
    for row in 1..=3 {
        keys.push(format!("{A}txtS1C5CapitalGainsI{row}"));
        keys.push(format!("{A}txtS2C5CapitalLossI{row}"));
    }
    keys.extend((1..=11).map(|row| format!("{A}txtS1C5CapitalGains_{row}")));
    keys.extend((1..=9).map(|row| format!("{A}txtS2C5CapitalLoss_{row}")));
    keys
}

/// Forms every amount of whose model is whole pesos: each money box shows
/// whole pesos (`w`), computed ones too (the bound ones already do, from
/// their `Whole` binding). 1702RT: every amount of Form1702RTDraft is a
/// `WholePeso`, and the sheet prints each over a whole-peso comb
/// (html-frozen/1702rt-2018c/writer-cells.json peso_joins). 1702MX: every
/// amount of Form1702MXDraft is a `WholePeso` or `WholePesoInput`, and its
/// sheet says "DO NOT enter Centavos" over Part II and Schedules 2, 4, 7.1
/// and 8.1 (html-frozen/1702mx-2018c/index.html).
/// Display only: a typed fraction still shows (form_page.js formatMoney).
const WHOLE_PESO_FORMS: [&str; 2] = ["1702RT", "1702MX"];

/// Mark the money boxes and columns of a whole-peso form `w` (rule 7): the
/// computed ones, which have no binding to say so. A box bound to an
/// amount (a penalty override) keeps taking centavos, as its binding says.
pub(super) fn mark_whole_pesos(form: &mut Value, code: &str) {
    let Some(kind) = AgentFormKind::from_code(code).filter(|_| WHOLE_PESO_FORMS.contains(&code))
    else {
        return;
    };
    let def = kind.definition();
    let unbound = |key: &Value| {
        key.as_str()
            .is_some_and(|key| !def.bindings.contains_key(key))
    };
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        for field in item["f"].as_array_mut().into_iter().flatten() {
            if field["t"] == "money" && unbound(&field["k"]) {
                field["w"] = Value::Bool(true);
            }
        }
        let keys = item["keys"].clone();
        for (c, col) in item["cols"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let cells: Vec<&Value> = keys
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| row.get(c))
                .collect();
            if col["t"] == "money" && !cells.is_empty() && cells.iter().all(|key| unbound(key)) {
                col["w"] = Value::Bool(true);
            }
        }
    }
}

/// 1702EX's last fixed line of each "(more...)" pop-up: Schedule 1 item
/// 17i, Schedule 2 item 4 and Schedule 3 items 3, 6 and 8. Each shows the
/// pop-up's first row, or OTHERS with the pop-up subtotal once it holds two
/// or more (form_1702ex.rs to_field_map). `(description, amount)`.
const EX_FOLDED_LINES: [(&str, &str); 5] = [
    ("txtPg3Pt6I17OthersIDesc", "txtPg3Pt6I17OthersIAmt"),
    ("txtPg3Pt6S2I4Description", "txtPg3Pt6S2I4Amount"),
    (
        "txtPg3Pt6S3I3NonDeductExpenseOtherIncomeDesc",
        "txtPg3Pt6S3I3NonDeductExpenseOtherIncomeAmt",
    ),
    (
        "txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxDesc",
        "txtPg3Pt6S3I6NonTaxIncomeAndIncomeSubjectToFinTaxAmt",
    ),
    (
        "txtPg3Pt6S3I8SpecialDeductDesc",
        "txtPg3Pt6S3I8SpecialDeductAmt",
    ),
];

/// What a 1702EX pop-up's last fixed line waits for while it is blank.
pub(super) const EX_FOLDED_WAITS: &str =
    "Shows the first (more...) row below, or OTHERS once they hold two or more.";

/// 1702EX items 8 and 9: the box writes the draft's whole field, which the
/// field map cuts into the official lines (loadBGData's 60 and 80
/// characters, upper-cased); the page shows the whole field, as the print
/// shapes it over the sheet's combs.
const EX_WHOLE_FIELDS: [(&str, &str); 2] = [
    ("txtPg1Pt1I8RegisteredName", "registered_name"),
    ("txtPg1Pt1I9RegisteredAddress", "registered_address"),
];

/// Each form's Number of Attachments: a count the new draft holds as the
/// official zeros (`000`, `00`), shown empty until the filer types one.
const ATTACHMENTS: [&str; 3] = [
    "frm1702RT:txtPg1Pt2PagesFilled",
    "frm1702MX:txtPg1P2I23NumOfAttachments",
    "frm1702EX:txtPg1P2I23NumOfAttachments",
];

/// What the page shows where the field map's value is not what the box
/// holds (display only: the field map, the XML and the print are
/// unchanged): 1702EX's whole name and address, and an untyped count of
/// attachments.
pub(super) fn shown(kind: AgentFormKind, draft: &Value, map: &mut BTreeMap<String, String>) {
    let code = kind.form_code();
    if code == "1702EX" {
        for (stem, field) in EX_WHOLE_FIELDS {
            if let Some(text) = draft[field].as_str().map(str::trim)
                && !text.is_empty()
            {
                map.insert(format!("{EX}{stem}"), text.to_string());
            }
        }
    }
    for key in ATTACHMENTS
        .iter()
        .filter(|key| key.starts_with(&format!("frm{code}:")))
    {
        if let Some(shown) = map.get_mut(*key)
            && !shown.is_empty()
            && shown.chars().all(|c| c == '0')
        {
            shown.clear();
        }
    }
}

/// The forms this module serves.
const FORMS: [&str; 4] = ["1702RT", "1702MX", "1702EX", "1707A"];

/// The lock the probe gives a box whose value follows other draft fields
/// and never echoes one of them.
const DERIVED: &str = "cannot be filled here: derived by the app";

/// The forms every derived box of which, the TIN's parts aside, the form
/// computes (see [`relock`]).
const DERIVED_IS_COMPUTED: [&str; 2] = ["1702RT", "1707A"];

/// 1702MX's derived boxes the form computes: the MCIT ATC (from the
/// incorporation date), Schedule 2 item 15's tax due and the Schedule 7.1
/// and 8.1 item 7 years (this year's loss). Its other derived boxes (the
/// tax rates, which the draft keeps as a `PercentInput`, and the Schedule
/// 7.1 and 8.1 rows 4 to 6) are the filer's entries the probe could not
/// bind; they keep their reason.
const MX_COMPUTED: [&str; 5] = [
    "frm1702MX:chkPg1I5ATCR1",
    "frm1702MX:txtPg2Sc2It15B",
    "frm1702MX:txtPg2Sc2It15C",
    "frm1702MX:txtPg3IShed7_7Year",
    "frm1702MX:txtPg4IShed8_7Year",
];

/// Locks that say what the form does with a box the probe could only call
/// derived or constant: the form computes it. On 1702RT and 1707A every
/// derived box but the TIN's parts (the return's identity, drawn as one
/// TIN) is a total, a rate or a carried amount the form's own recompute
/// writes; the boxes the filer types that recompute only clears are bound
/// (probe.rs GATED_ENTRY).
pub(super) fn relock(kind: AgentFormKind, locks: &mut Map<String, Value>) {
    let code = kind.form_code();
    if !FORMS.contains(&code) {
        return;
    }
    let identity = |key: &str| key.contains("TIN") || key.contains("Tin");
    let derived: Vec<String> = locks
        .iter()
        .filter(|(key, reason)| {
            let derived = reason.as_str().is_some_and(|r| r.starts_with(DERIVED));
            derived
                && ((DERIVED_IS_COMPUTED.contains(&code) && !identity(key))
                    || MX_COMPUTED.contains(&key.as_str()))
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in derived {
        locks.insert(key, json!(COMPUTED));
    }
    let computed: Vec<String> = match code {
        // Schedule IIIA item 4 is this year's loss: its year and amount
        // follow Schedule III item 3, its columns B to D stay at zero
        // (form_1702rt_official.rs recompute).
        "1702RT" => (1..=6)
            .map(|column| format!("{RT}txtPg4Sc3AI4C{column}"))
            .collect(),
        "1702EX" => EX_FOLDED_LINES
            .iter()
            .flat_map(|(description, amount)| [description, amount])
            .map(|stem| format!("{EX}{stem}"))
            .collect(),
        "1707A" => a_row4_and_column_e(),
        _ => Vec::new(),
    };
    for key in computed {
        if locks.contains_key(&key) {
            locks.insert(key, json!(COMPUTED));
        }
    }
}

/// What 1707A's printed row 4 waits for while it is blank.
pub(super) const A_ROW4_WAITS: &str =
    "Shows row 4 of the continuation below, or OTHERS once that has more rows.";
/// What 1702RT's Schedule IIIA item 4 year waits for.
pub(super) const RT_NOLCO_YEAR_WAITS: &str =
    "Shows this year once Schedule III item 3 is a net operating loss.";

/// What 1702MX's Schedule 7.1 and 8.1 item 7 years wait for: this year's
/// loss (form_1702mx_official.rs recompute fills them from Schedules 7 and 8
/// item 3).
pub(super) const MX_NOLCO_7_YEAR_WAITS: &str =
    "Shows this year once Schedule 7 item 3 is a net operating loss.";
pub(super) const MX_NOLCO_8_YEAR_WAITS: &str =
    "Shows this year once Schedule 8 item 3 is a net operating loss.";

/// What a blank box this module locks waits for (shown in the box).
pub(super) fn waits(kind: AgentFormKind) -> Map<String, Value> {
    let mut out = Map::new();
    match kind.form_code() {
        "1702RT" => {
            out.insert(format!("{RT}txtPg4Sc3AI4C1"), json!(RT_NOLCO_YEAR_WAITS));
        }
        "1702EX" => {
            for (description, _) in EX_FOLDED_LINES {
                out.insert(format!("{EX}{description}"), json!(EX_FOLDED_WAITS));
            }
        }
        "1702MX" => {
            out.insert(
                "frm1702MX:txtPg3IShed7_7Year".into(),
                json!(MX_NOLCO_7_YEAR_WAITS),
            );
            out.insert(
                "frm1702MX:txtPg4IShed8_7Year".into(),
                json!(MX_NOLCO_8_YEAR_WAITS),
            );
        }
        "1707A" => {
            for stem in [
                "txtS1C1DateOfTransactionI4",
                "txtS1C2NameOfCorporateStockI4",
                "txtS2C1DateOfTransactionI4",
                "txtS2C2NameOfCorporateStockI4",
            ] {
                out.insert(format!("{A}{stem}"), json!(A_ROW4_WAITS));
            }
        }
        _ => {}
    }
    out
}

/// What this edit removed that the filer typed: the page asks `message`
/// first and offers Undo with `done`.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    if kind.form_code() == "1707A"
        && edits.contains_key(TAX_RELIEF_NO)
        && let Some(relief) = before["tax_relief"]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
        && after["tax_relief"].is_null()
    {
        return Some(json!({
            "message": format!(
                "Changing item 12 to No clears item 12A (the relief you specified: {relief}). Continue?"
            ),
            "done": "Cleared item 12A",
            "rows": 0,
        }));
    }
    None
}
