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
//! - 1702MX's tax rates (Schedule 1 item 4, Schedule 2 item 14): the draft
//!   keeps each as a `PercentInput` (hundredths and the official lexeme),
//!   which the probe cannot bind. The host writes the typed rate with the
//!   lexeme the official field map writes, so a typed rate files as the
//!   official program would file it.
//! - 1702RT item 42 MCIT Due takes an amount only while MCIT applies (the
//!   form's own `subject_to_mcit`); otherwise recompute clears it, so the
//!   host refuses it with why. Its (Add more...) items read OTHERS and the
//!   subtotal once the table holds two or more rows: they are then the
//!   form's, locked and refused.
//! - Computed boxes the probe left as "derived" say they are computed
//!   ([`relock`]); a blank one says what fills it ([`waits`]).
//! - An answer whose recompute clears or replaces boxes the filer typed (an
//!   OSD answer, Amended Return No, MCIT no longer applying, an overpayment
//!   gone) is reported ([`removes`]): the page asks first and offers Undo.
//! - A message naming another item number than the printed one reads the
//!   printed number ([`printed_item_numbers`]; maintainer decision D6,
//!   2026-10-18: wording only, the checks and the filed data are unchanged).
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use std::collections::BTreeMap;

use std::collections::BTreeSet;

use bir_core::forms::agent::{self, AgentFormKind, Binding};
use bir_core::forms::form_1702rt::{
    FORM_1702RT_MODALS, Form1702RTDate, Form1702RTDraft, Form1702RTModal, Form1702RTMoreRows,
};
use serde_json::{Map, Value, json};

use super::{schema_entry, schema_keys, set_draft_path};

const RT: &str = "frm1702RT:";
const MX: &str = "frm1702MX:";
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

/// 1702MX's rate boxes: the draft field each keeps and the decimals the
/// official field map writes it with (form_1702mx_official.rs
/// to_official_field_map: item 4B one, items 14B and 14C two).
const MX_RATES: [(&str, &str, usize); 3] = [
    (
        "txtPg2Pt4I34SpecialTaxRate",
        "/relief_basis/special_tax_rate",
        1,
    ),
    ("txtPg2Sc2It14B", "/schedule_2/item_14_special_rate", 2),
    ("txtPg2Sc2It14C", "/schedule_2/item_14_regular_rate", 2),
];

/// What a 1702MX rate box says when it is not a rate it files as typed
/// (`decimals` is the official field map's).
pub(super) fn mx_rate_refusal(decimals: usize) -> String {
    let places = if decimals == 1 {
        "one decimal"
    } else {
        "two decimals"
    };
    format!("Enter the rate as a percent below 100, with at most {places}.")
}

/// 1702RT item 42 MCIT Due.
const RT_MCIT_DUE: &str = "frm1702RT:txtPg2Pt4I42MinimumCorporate";

/// Item 42's lock while MCIT does not apply.
pub(super) const RT_MCIT_REFUSAL: &str = "Item 42 applies once MCIT does: from the fourth taxable year after the year of incorporation (item 10).";

/// Item 42's lock on a return for a year before 2020 (calculations.json
/// 1702rt-item42-mcit: "Before 2020 and when IC 055 is selected, MCIT is 2%
/// of gross"; the port does not compute it, so the box stays as stored).
pub(super) const RT_MCIT_BEFORE_2020: &str =
    "For years before 2020, MCIT is 2% of gross income (item 33)";

/// Item 42's lock from 2020 while net taxable income is zero or less
/// (calculations.json 1702rt-item42-mcit: "2020+ leaves the enabled value
/// user-controlled except nonpositive net forces zero").
pub(super) const RT_MCIT_NO_NET: &str =
    "From 2020, MCIT is zero while net taxable income (item 39) is zero or less.";

/// The lock of a 1702RT (Add more...) item once its table folds.
pub(super) const RT_FOLDED_LOCK: &str =
    "computed from its (Add more...) rows: OTHERS and their subtotal";

/// 1707A Item 12's radios: Yes and No.
const TAX_RELIEF_YES: &str = "frm1707Av2021:rdoI11TaxTreatyYes";
const TAX_RELIEF_NO: &str = "frm1707Av2021:rdoI11TaxTreatyNo";

fn mx_rate(key: &str) -> Option<(&'static str, usize)> {
    let tail = key.strip_prefix(MX)?;
    MX_RATES
        .iter()
        .find(|(stem, _, _)| *stem == tail)
        .map(|(_, path, decimals)| (*path, *decimals))
}

/// The boxes of a 1702RT (Add more...) item: its description, Schedule
/// II's legal basis, and its amount.
fn rt_folded_columns(modal: Form1702RTModal) -> Vec<String> {
    let columns: &[&str] = if modal.has_legal_basis() {
        &["C1", "C2", "C3"]
    } else {
        &["C1", "C2"]
    };
    columns
        .iter()
        .map(|column| format!("{RT}{}{column}", modal.row_name()))
        .collect()
}

/// The (Add more...) item a 1702RT key is a box of.
fn rt_folded_item(key: &str) -> Option<Form1702RTModal> {
    FORM_1702RT_MODALS
        .into_iter()
        .find(|modal| rt_folded_columns(*modal).iter().any(|k| k == key))
}

fn rt_more_rows(draft: &Value) -> Form1702RTMoreRows {
    serde_json::from_value(draft["more_rows"].clone()).unwrap_or_default()
}

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
        "1702MX" => mx_rate(key).is_some(),
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
        "1702RT" => {
            if let Some(path) = rt_date_path(key) {
                return Some(fill_1702rt_date(draft, path, value));
            }
            refuse_1702rt(draft, key, value).map(Err)
        }
        "1702MX" => {
            mx_rate(key).map(|(path, decimals)| fill_1702mx_rate(draft, path, decimals, value))
        }
        "1707A" if host_writes(kind, key) => Some(fill_1707a_tax_relief(draft, key, value)),
        _ => None,
    }
}

/// Why 1702RT item 42 is shut, if it is: the official computeP2Pt4I42
/// (calculations.json 1702rt-item42-mcit) leaves it to the filer only from
/// 2020, while MCIT applies (item 10's four years) and item 39 is above
/// zero.
pub(super) fn rt_item_42_lock(draft: &Value) -> Option<&'static str> {
    let typed: Form1702RTDraft = serde_json::from_value(draft.clone()).ok()?;
    if typed.taxable_year < 2020 {
        Some(RT_MCIT_BEFORE_2020)
    } else if !typed.subject_to_mcit() {
        Some(RT_MCIT_REFUSAL)
    } else if typed.part_iv.item_39_net_taxable_income_or_loss.0 <= 0 {
        Some(RT_MCIT_NO_NET)
    } else {
        None
    }
}

/// A 1702RT box the form keeps right now: item 42 while it is shut (see
/// [`rt_item_42_lock`]), and an (Add more...) item while its table is
/// folded. A blank item 42 is taken: it only clears the box.
#[inline(never)]
fn refuse_1702rt(draft: &Value, key: &str, value: &Value) -> Option<String> {
    if key == RT_MCIT_DUE {
        let text = text_of(value);
        if text.is_empty() || agent::parse_amount(&text) == Some(0.0) {
            return None;
        }
        return rt_item_42_lock(draft).map(str::to_string);
    }
    let modal = rt_folded_item(key)?;
    rt_more_rows(draft)
        .is_folded(modal)
        .then(|| format!("`{key}` is {RT_FOLDED_LOCK}"))
}

/// A 1702MX rate: blank clears it, a percent below 100 with no more
/// decimals than the official field map writes keeps it (with that
/// lexeme), anything else is refused.
#[inline(never)]
fn fill_1702mx_rate(
    draft: &mut Value,
    path: &str,
    decimals: usize,
    value: &Value,
) -> Result<(), String> {
    let typed = text_of(value);
    let text = typed.strip_suffix('%').unwrap_or(&typed).trim();
    let rate = if typed.is_empty() {
        json!({"hundredths": null, "raw": ""})
    } else {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let digits = |s: &str| s.chars().all(|c| c.is_ascii_digit());
        if whole.is_empty() || !digits(whole) || !digits(fraction) || fraction.len() > decimals {
            return Err(mx_rate_refusal(decimals));
        }
        // Checked: a long number is refused, never wrapped into a small rate.
        let fraction: i32 = format!("{fraction:0<2}").parse().unwrap_or(0);
        let hundredths = whole
            .parse::<i32>()
            .ok()
            .and_then(|whole| whole.checked_mul(100))
            .and_then(|whole| whole.checked_add(fraction))
            .filter(|hundredths| (0..10_000).contains(hundredths))
            .ok_or_else(|| mx_rate_refusal(decimals))?;
        let raw = format!("{:.*}", decimals, f64::from(hundredths) / 100.0);
        json!({"hundredths": hundredths, "raw": raw})
    };
    set_draft_path(draft, path, rate);
    Ok(())
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
        // MM/DD/YYYY exactly: a two-digit year is not this box's date.
        let shape = text.len() == 10
            && text.char_indices().all(|(i, c)| {
                if i == 2 || i == 5 {
                    c == '/'
                } else {
                    c.is_ascii_digit()
                }
            });
        if !shape {
            return Err(RT_DATE_REFUSAL.to_string());
        }
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

/// Number of Attachments where the new draft holds a default (form_1702rt.rs
/// and form_1702mx.rs new_from_profile: `000`, `00`; form_1702ex.rs: the
/// number 0): the box shows empty while the draft holds exactly that
/// default, and any other count as typed. 1702RT and 1702MX keep a typed 0
/// apart from their default; 1702EX keeps a number, so its typed 0 is the
/// default and shows empty too. `(key, draft field, default)`.
fn attachments() -> [(&'static str, &'static str, Value); 3] {
    [
        (
            "frm1702RT:txtPg1Pt2PagesFilled",
            "number_of_attachments",
            json!("000"),
        ),
        (
            "frm1702MX:txtPg1P2I23NumOfAttachments",
            "number_of_attachments",
            json!("00"),
        ),
        (
            "frm1702EX:txtPg1P2I23NumOfAttachments",
            "number_of_attachments",
            json!(0),
        ),
    ]
}

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
    // A 1702MX rate never typed shows empty (the field map writes 0.00).
    if code == "1702MX" {
        for (stem, path, _) in MX_RATES {
            if draft.pointer(path).and_then(|rate| rate.get("hundredths")) == Some(&Value::Null)
                && let Some(shown) = map.get_mut(&format!("{MX}{stem}"))
            {
                shown.clear();
            }
        }
    }
    for (key, field, default) in attachments()
        .into_iter()
        .filter(|(key, _, _)| key.starts_with(&format!("frm{code}:")))
    {
        if same(draft.get(field), Some(&default))
            && let Some(shown) = map.get_mut(key)
        {
            shown.clear();
        }
    }
}

/// The forms this module serves.
const FORMS: [&str; 4] = ["1702RT", "1702MX", "1702EX", "1707A"];

/// The locks that follow the draft as it is filled, each key with its
/// reason or `null` (open): 1702RT's (Add more...) items, shut while their
/// table holds two or more rows, and item 42 (see [`rt_item_42_lock`]).
/// `screen_form_open` applies them with the other locks; `screen_form_fill`
/// returns them, so a box shuts or opens as soon as the answer that decides
/// it is typed.
pub(super) fn live_locks(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let mut locks = Map::new();
    if kind.form_code() != "1702RT" {
        return locks;
    }
    let more = rt_more_rows(draft);
    for modal in FORM_1702RT_MODALS {
        let lock = if more.is_folded(modal) {
            json!(RT_FOLDED_LOCK)
        } else {
            Value::Null
        };
        for key in rt_folded_columns(modal) {
            locks.insert(key, lock.clone());
        }
    }
    locks.insert(
        RT_MCIT_DUE.to_string(),
        rt_item_42_lock(draft).map_or(Value::Null, |reason| json!(reason)),
    );
    locks
}

/// The note a live lock shows under its item, in the filer's words: item
/// 42 says why it is shut.
pub(super) fn live_lock_note(reason: &str) -> Option<&'static str> {
    [
        (RT_MCIT_BEFORE_2020, RT_MCIT_BEFORE_2020_NOTE),
        (RT_MCIT_REFUSAL, RT_MCIT_NOT_YET_NOTE),
        (RT_MCIT_NO_NET, RT_MCIT_NO_NET_NOTE),
    ]
    .into_iter()
    .find(|(known, _)| *known == reason)
    .map(|(_, note)| note)
}

/// Item 42's notes (see [`live_lock_note`]).
pub(super) const RT_MCIT_BEFORE_2020_NOTE: &str =
    "Shut for this year: MCIT before 2020 is 2% of gross income, which this app does not compute.";
pub(super) const RT_MCIT_NOT_YET_NOTE: &str =
    "Shut until MCIT applies, from the fourth year after incorporation.";
pub(super) const RT_MCIT_NO_NET_NOTE: &str =
    "Shut while net taxable income is zero or less: MCIT is then zero.";

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
pub(super) fn relock(kind: AgentFormKind, draft: &Value, locks: &mut Map<String, Value>) {
    let code = kind.form_code();
    if !FORMS.contains(&code) {
        return;
    }
    // 1702RT: a folded (Add more...) item and a shut item 42.
    for (key, lock) in live_locks(kind, draft) {
        if !lock.is_null() {
            locks.insert(key, lock);
        }
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
    if ASKS_FIRST.contains(&kind.form_code()) {
        return cleared_entries(kind, before, after, edits);
    }
    None
}

/// The forms whose recompute clears or replaces entries an answer turns
/// off: 1702RT (OSD clears Schedules I, II and IIIA, the pop-up rows and
/// item 57; MCIT no longer applying clears Schedule IV and items 42 and 45;
/// Amended No zeroes item 51; an overpayment gone clears item 21's choice),
/// 1702MX (OSD clears column C of Schedules 5 and 6 and Schedule 7.1 rows
/// 1 to 3, their year incurred and every amount column; Amended No zeroes
/// item 27; MCIT no longer applying clears Schedule 9; an overpayment gone
/// clears item 21's choice) and 1702EX (OSD clears every Schedule 1 item,
/// the other-deduction rows 17d to 17i and their pop-up, and the Schedule 2
/// special rows), in form_1702rt_official.rs, form_1702mx_official.rs and
/// form_1702ex.rs recompute.
const ASKS_FIRST: [&str; 3] = ["1702RT", "1702MX", "1702EX"];

/// The draft fields a binding writes.
fn binding_paths(binding: &Binding) -> Vec<String> {
    match binding {
        Binding::Text { path, .. } | Binding::Amount { path, .. } | Binding::Whole { path, .. } => {
            vec![path.clone()]
        }
        Binding::Choice { options } => {
            let paths: BTreeSet<String> = options
                .iter()
                .flat_map(|option| option.set.keys().cloned())
                .collect();
            paths.into_iter().collect()
        }
    }
}

/// A draft value that holds nothing the filer typed: missing, null, blank,
/// zero or false.
fn holds_nothing(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => {
            let text = text.trim();
            text.is_empty() || text.chars().all(|c| matches!(c, '0' | '.' | ','))
        }
        Some(Value::Number(number)) => number.as_f64() == Some(0.0),
        Some(Value::Bool(flag)) => !flag,
        Some(_) => false,
    }
}

/// The same draft value: numbers by value (the page's transport turns
/// `5000.0` into `5000`).
fn same(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (Some(Value::Number(a)), Some(Value::Number(b))) => a.as_f64() == b.as_f64(),
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same(Some(a), Some(b)))
        }
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            a.len() == b.len() && a.iter().all(|(k, v)| same(Some(v), b.get(k)))
        }
        _ => a == b,
    }
}

/// Where a key is drawn: its part's name and its item number.
fn placement(code: &str, key: &str) -> Option<(String, String)> {
    let form = schema_entry(code)?;
    for part in form["parts"].as_array()? {
        for item in part["items"].as_array()? {
            let fields = item["f"].as_array().into_iter().flatten().map(|f| &f["k"]);
            let cells = item["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|row| row.as_array().into_iter().flatten());
            if fields.chain(cells).any(|k| k == key) {
                return Some((
                    part["n"].as_str()?.to_string(),
                    item["no"].as_str().unwrap_or("").to_string(),
                ));
            }
        }
    }
    None
}

/// A part's short name: "Schedule I", "Part II".
fn short_part(part: &str) -> &str {
    part.split(" · ").next().unwrap_or(part)
}

/// Every entry box (bound, on the page, not one of this edit's) whose
/// draft field held something before this edit and holds something else
/// after it: cleared or replaced by the form's own recompute.
#[inline(never)]
fn cleared_entries(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let code = kind.form_code();
    let def = kind.definition();
    let mut edited: BTreeSet<String> = BTreeSet::new();
    for key in edits.keys() {
        if let Some(bound) = def.bindings.get(key) {
            edited.extend(binding_paths(&bound.binding));
        }
        let group = def.box_def(key).and_then(|entry| entry.group.as_deref());
        if let Some(bound) = group.and_then(|group| def.bindings.get(group)) {
            edited.extend(binding_paths(&bound.binding));
        }
        edited.extend(rt_date_path(key).map(str::to_string));
        edited.extend(mx_rate(key).map(|(path, _)| path.to_string()));
    }
    let on_page = schema_keys(code);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut lost: Vec<String> = Vec::new();
    for (key, bound) in &def.bindings {
        if !on_page.contains(key) {
            continue;
        }
        // A radio or checkbox is named only if it was the ticked one.
        if let Binding::Choice { options } = &bound.binding
            && let Some(ticked) = options.iter().find(|option| option.value == "true")
            && !ticked
                .set
                .iter()
                .all(|(path, value)| before.pointer(path) == Some(value))
        {
            continue;
        }
        for path in binding_paths(&bound.binding) {
            if edited.contains(&path) || seen.contains(&path) {
                continue;
            }
            let was = before.pointer(&path);
            if !holds_nothing(was) && !same(was, after.pointer(&path)) {
                seen.insert(path);
                lost.push(key.clone());
                break;
            }
        }
    }
    if lost.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for key in &lost {
        if let Some((part, _)) = placement(code, key) {
            let part = short_part(&part).to_string();
            if !parts.contains(&part) {
                parts.push(part);
            }
        }
    }
    let trigger = edits
        .keys()
        .find_map(|key| placement(code, key))
        .map(|(part, no)| {
            if no.is_empty() {
                short_part(&part).to_string()
            } else if part.starts_with("Schedule") {
                format!("item {no} of {}", short_part(&part))
            } else {
                format!("item {no}")
            }
        })
        .unwrap_or_else(|| "this answer".to_string());
    let n = lost.len();
    let boxes = if n == 1 { "box" } else { "boxes" };
    Some(json!({
        "message": format!(
            "Changing {trigger} clears {n} {boxes} you typed ({}). Continue?",
            parts.join(", ")
        ),
        "done": format!("Cleared {n} {boxes}"),
        "rows": n,
        "keys": lost,
    }))
}

/// Messages that name another item number than the one the printed sheet
/// gives the thing they name, reworded to the printed number (maintainer
/// decision D6, 2026-10-18; an approved exception to rule 0 for wording
/// only). `(form, field prefix, official words, printed words)`. The
/// printed numbers are the frozen sheets' (html-frozen/<slug>/index.html),
/// as the page draws them:
/// - 1707A: the year ended is item 2 (item 1 is Calendar/Fiscal), the
///   number of sheets attached item 4, the tax paid on a previous return
///   item 18B.
/// - 1702RT: Part III's payment rows are items 23 to 26 (the official
///   labels run three ahead, form_1702rt_official.rs validate), Schedule I's
///   sixth OTHERS row is 17h (the official alert reuses 17g), and the date
///   of incorporation is item 10.
/// - 1702MX: the registered name is item 8 and the registered address item
///   9 (the date of incorporation is item 10).
/// - 1702EX: its "(more...)" pop-up alerts name rows the sheet does not
///   print ("item 17.2"); they read the printed item and the row
///   ([`ex_popup_row`]). Its other messages name their printed item.
const PRINTED_ITEM_NUMBERS: [(&str, &str, &str, &str); 11] = [
    ("1707A", "year_end", "on Item 1", "on Item 2"),
    (
        "1707A",
        "number_of_attached_sheets",
        "Item 3 holds",
        "Item 4 holds",
    ),
    (
        "1707A",
        "tax_paid_previous",
        "Item 16B applies",
        "Item 18B applies",
    ),
    (
        "1702RT",
        "payment_details[0]",
        "Part III Item 26.",
        "Part III Item 23.",
    ),
    (
        "1702RT",
        "payment_details[1]",
        "Part III Item 27.",
        "Part III Item 24.",
    ),
    (
        "1702RT",
        "payment_details[2]",
        "Part III Item 28.",
        "Part III Item 25.",
    ),
    (
        "1702RT",
        "payment_details[3]",
        "Part III Item 29.",
        "Part III Item 26.",
    ),
    (
        "1702RT",
        "schedule_1.other[4]",
        "Schedule 1 Item 17g.",
        "Schedule 1 Item 17h.",
    ),
    (
        "1702RT",
        "schedule_4.rows[",
        "(Page 1 Item 8).",
        "(Page 1 Item 10).",
    ),
    (
        "1702MX",
        "taxpayer_name",
        "(Part I Item 9).",
        "(Part I Item 8).",
    ),
    (
        "1702MX",
        "registered_address",
        "(Part I Item 10).",
        "(Part I Item 9).",
    ),
];

/// 1702EX's "(more...)" pop-ups: the official alerts number their rows
/// "17.1", "17.2", ..., which the printed sheet does not print; the sheet
/// prints the item the pop-up belongs to (Schedule 1 item 17i, Schedule 2
/// item 4, Schedule 3 items 3, 6 and 8; html-frozen/1702ex-2018/index.html)
/// and the page lists the pop-up's rows under it. `(field prefix, official
/// number, printed item)`, from form_1702ex.rs POPUP_*.
const EX_POPUP_ITEMS: [(&str, &str, &str); 5] = [
    ("other_deductions[", "17.", "17i"),
    ("special_rows[", "4.", "4"),
    ("non_deductible[", "3.", "3"),
    ("non_taxable_income[", "6.", "6"),
    ("special_deductions_s3[", "8.", "8"),
];

/// A 1702EX pop-up alert ("... on item 17.2") read with the printed item
/// and the pop-up row: "... on item 17i (row 2)".
fn ex_popup_row(field: &str, message: &str) -> Option<String> {
    let (_, official, printed) = EX_POPUP_ITEMS
        .iter()
        .find(|(prefix, _, _)| field.starts_with(prefix))?;
    let marker = format!("on item {official}");
    let at = message.rfind(&marker)?;
    let row = &message[at + marker.len()..];
    if row.is_empty() || !row.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{}on item {printed} (row {row})", &message[..at]))
}

/// Each message the form's validation gives, with an item number the
/// printed sheet does not give that item reworded to the printed one.
pub(super) fn printed_item_numbers(kind: AgentFormKind, errors: &mut [(String, String)]) {
    let code = kind.form_code();
    for (field, message) in errors.iter_mut() {
        if code == "1702EX"
            && let Some(reworded) = ex_popup_row(field, message)
        {
            *message = reworded;
            continue;
        }
        if let Some((_, _, official, printed)) =
            PRINTED_ITEM_NUMBERS
                .iter()
                .find(|(form, prefix, official, _)| {
                    *form == code && field.starts_with(prefix) && message.contains(official)
                })
        {
            *message = message.replace(official, printed);
        }
    }
}

/// What a 1702RT Schedule IV row says while it holds an MCIT that an older
/// version may have saved from column C.
pub(super) fn rt_column_c_notice(row: usize) -> String {
    format!(
        "Row {row} holds an MCIT amount that may have been typed in column C; check it before you file."
    )
}

/// Notices beside the form's own messages: nothing is changed, the filer
/// checks. 1702RT Schedule IV rows 2 and 3: before PR 8's review, column C
/// (Excess MCIT, which the form computes) was bound to the row's MCIT, so
/// a value typed there was saved as column B and files as MCIT. A stored
/// draft cannot tell which column it was typed in, so each row 2 or 3
/// holding an MCIT says so (as the maintainer's D5 does for 1604E's
/// changed rows); its stored bytes stay as they are.
pub(super) fn notices(kind: AgentFormKind, draft: &Value) -> Vec<Value> {
    if kind.form_code() != "1702RT" {
        return Vec::new();
    }
    let mut out = Vec::new();
    for index in [1usize, 2] {
        let mcit = &draft["schedule_4"]["rows"][index]["mcit"];
        if holds_nothing(Some(mcit)) {
            continue;
        }
        let row = index + 1;
        out.push(json!({
            "field": format!("schedule_4.rows[{index}].mcit"),
            "message": rt_column_c_notice(row),
            "keys": [format!("{RT}txtPg4Sc4I{row}C3")],
        }));
    }
    out
}
