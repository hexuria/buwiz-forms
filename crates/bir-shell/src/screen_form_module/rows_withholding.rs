//! The Q3 withholding remittances (1601EQ, 1601FQ, 1602Q, 1603Q): boxes the
//! host writes through the typed draft because the probed bindings cannot
//! (the official page re-sorts its ATC rows, or answers an item with a
//! handler that also clears what it closes), and what a box the form fills
//! from another box of its row waits for.
//!
//! - 1601FQ Part II: the official page writes one row per ATC ticked in its
//!   popup, in popup order (`getATCCode`), and the popup has codes that
//!   repeat with another rate, so a row is its popup position
//!   (`Form1601FqAtcRow::popup_index`). A row's ATC is chosen as
//!   `catalog:<position>`: a new row ticks it, a changed row keeps its tax
//!   base, blank unticks it; its tax base is written to the row. Item 4
//!   (`set_any_tax_withheld`) and Item 11 (`set_category`, a new popup list)
//!   clear the rows the way the official handlers do; Items 13 and 13A
//!   (`set_tax_relief`) open and close Schedule 1 (`enableSelTreaty`).
//! - 1603Q Item 13: Yes opens Item 13A blank, No closes it (`tax_relief`).
//! - 1602Q Item 13: Yes and No go through `set_availing_tax_relief`, which
//!   clears Schedule 2 on No (`DrpListTreaty`). While Item 13 is not Yes,
//!   13A and Schedule 2 stay closed: what is written there is dropped, as
//!   the official page keeps them disabled and empty (rule 6 reads that).
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_1601fq::{Form1601FqCategory, Form1601FqDraft, Form1601FqTaxRelief};
use bir_core::forms::form_1602q::Form1602QDraft;
use bir_core::forms::form_1603q::{Form1603QDraft, Form1603QTaxRelief};
use serde_json::{Map, Value, json};

/// Part II rows the 1601FQ page shows: the printed six (items 14-19) and
/// the attachment's "Other Selected ATC" rows 7-41.
pub(super) const FQ_ROWS: usize = 41;
/// 1601EQ Part II rows the page shows (the printed six and rows 7-30).
const EQ_ROWS: usize = 30;

/// What a row's tax rate waits for while it is blank.
pub(super) const RATE_WAITS: &str = "Shows the ATC's rate once this row's ATC is chosen.";

/// 1602Q boxes Item 13 "Yes" opens (`DrpListTreaty`): 13A and Schedule 2.
const FORM_1602Q_ITEM_13: [&str; 9] = [
    "frm1602Q:lstSpecialTax",
    "frm1602Q:txtSched2TreatyCode1",
    "frm1602Q:txtSched2ATC1",
    "frm1602Q:txtSched2TaxRate1",
    "frm1602Q:txtSched2Interest1",
    "frm1602Q:txtSched2TreatyCode2",
    "frm1602Q:txtSched2ATC2",
    "frm1602Q:txtSched2TaxRate2",
    "frm1602Q:txtSched2Interest2",
];

/// A box this module writes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    FqAtc(usize),
    FqBase(usize),
    FqCategory(Form1601FqCategory),
    FqWithheld(bool),
    FqItem13(bool),
    FqItem13A,
    QItem13(bool),
    Q2Item13(bool),
    Q2Gated,
}

fn row_of(key: &str, stem: &str, rows: usize) -> Option<usize> {
    let n: usize = key.strip_prefix(stem)?.parse().ok()?;
    (1..=rows).contains(&n).then(|| n - 1)
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    match kind.form_code() {
        "1601FQ" => {
            if let Some(row) = row_of(key, "frm1601FQ:txtAtcCode", FQ_ROWS) {
                return Some(Cell::FqAtc(row));
            }
            if let Some(row) = row_of(key, "frm1601FQ:txtTaxBase", FQ_ROWS) {
                return Some(Cell::FqBase(row));
            }
            match key {
                "frm1601FQ:CatAgent_P" => Some(Cell::FqCategory(Form1601FqCategory::Private)),
                "frm1601FQ:CatAgent_G" => Some(Cell::FqCategory(Form1601FqCategory::Government)),
                "frm1601FQ:TaxWithheld_1" => Some(Cell::FqWithheld(true)),
                "frm1601FQ:TaxWithheld_2" => Some(Cell::FqWithheld(false)),
                "frm1601FQ:SpecialTax_1" => Some(Cell::FqItem13(true)),
                "frm1601FQ:SpecialTax_2" => Some(Cell::FqItem13(false)),
                "frm1601FQ:drpSpecialTax" => Some(Cell::FqItem13A),
                _ => None,
            }
        }
        "1603Q" => match key {
            "frm1603Q:SpecialTax_1" => Some(Cell::QItem13(true)),
            "frm1603Q:SpecialTax_2" => Some(Cell::QItem13(false)),
            _ => None,
        },
        "1602Q" => match key {
            "frm1602Q:OptSpecialTax1" => Some(Cell::Q2Item13(true)),
            "frm1602Q:OptSpecialTax2" => Some(Cell::Q2Item13(false)),
            _ if FORM_1602Q_ITEM_13.contains(&key) => Some(Cell::Q2Gated),
            _ => None,
        },
        _ => None,
    }
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page). A 1602Q box Item 13 opens is written by the
/// ordinary fill once it is open.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    cell(kind, key).is_some_and(|cell| cell != Cell::Q2Gated)
}

/// An ATC is chosen before its row's tax base.
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    match cell(kind, key)? {
        Cell::FqBase(_) => Some(1),
        _ => None,
    }
}

fn ticked(value: &Value) -> bool {
    matches!(value, Value::Bool(true)) || value.as_str().is_some_and(|s| s.trim() == "true")
}

fn text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        other => other.as_str().unwrap_or_default().trim().to_string(),
    }
}

fn typed<T: serde::de::DeserializeOwned>(draft: &Value) -> Result<T, String> {
    serde_json::from_value(draft.clone()).map_err(|e| e.to_string())
}

fn store<T: serde::Serialize>(draft: &mut Value, form: &T) -> Result<(), String> {
    *draft = serde_json::to_value(form).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write `key` when this module owns it: `None` leaves it to the ordinary
/// fill. `fill` is that ordinary fill, for a 1602Q box Item 13 has open.
pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    ordinary: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Option<Result<(), String>> {
    let cell = cell(kind, key)?;
    Some(match cell {
        Cell::Q2Gated => {
            let open = draft["availing_tax_relief"] == json!(true);
            if open { ordinary(draft) } else { Ok(()) }
        }
        _ => write(cell, draft, value),
    })
}

fn write(cell: Cell, draft: &mut Value, value: &Value) -> Result<(), String> {
    match cell {
        Cell::FqAtc(row) => fill_fq_atc(draft, row, value),
        Cell::FqBase(row) => fill_fq_base(draft, row, value),
        // A one-answer pair: the ticked box carries the answer, the other
        // box's untick in the same batch says nothing more.
        Cell::FqCategory(category) => {
            if !ticked(value) {
                return Ok(());
            }
            let mut form: Form1601FqDraft = typed(draft)?;
            form.set_category(category);
            store(draft, &form)
        }
        Cell::FqWithheld(withheld) => {
            if !ticked(value) {
                return Ok(());
            }
            let mut form: Form1601FqDraft = typed(draft)?;
            form.set_any_tax_withheld(withheld);
            store(draft, &form)
        }
        Cell::FqItem13(yes) => {
            if !ticked(value) {
                return Ok(());
            }
            let mut form: Form1601FqDraft = typed(draft)?;
            match (yes, form.tax_relief) {
                (true, Some(_)) => {}
                (true, None) => form.set_tax_relief(Some(Form1601FqTaxRelief::default())),
                (false, _) => form.set_tax_relief(None),
            }
            store(draft, &form)
        }
        Cell::FqItem13A => {
            let mut form: Form1601FqDraft = typed(draft)?;
            let relief = match text(value).as_str() {
                "" | "0" => Form1601FqTaxRelief::Unspecified,
                "1" => Form1601FqTaxRelief::SpecialRate,
                "2" => Form1601FqTaxRelief::InternationalTaxTreaty,
                "3" => Form1601FqTaxRelief::Both,
                other => return Err(format!("`{other}` is not an Item 13A choice.")),
            };
            // A relief chosen answers Item 13 Yes, as 1602Q's and 1603Q's
            // 13A do (`SpecialTax_1` is `tax_relief.is_some()`); the blank
            // choice leaves a "No" as it is.
            if relief == Form1601FqTaxRelief::Unspecified && form.tax_relief.is_none() {
                return Ok(());
            }
            form.set_tax_relief(Some(relief));
            store(draft, &form)
        }
        Cell::QItem13(yes) => {
            if !ticked(value) {
                return Ok(());
            }
            let mut form: Form1603QDraft = typed(draft)?;
            form.tax_relief = match (yes, form.tax_relief) {
                (true, Some(relief)) => Some(relief),
                // `enableSelTreaty` opens 13A blank.
                (true, None) => Some(Form1603QTaxRelief::Unspecified),
                (false, _) => None,
            };
            form.recompute();
            store(draft, &form)
        }
        Cell::Q2Item13(yes) => {
            if !ticked(value) {
                return Ok(());
            }
            let mut form: Form1602QDraft = typed(draft)?;
            form.set_availing_tax_relief(yes);
            store(draft, &form)
        }
        Cell::Q2Gated => Ok(()),
    }
}

/// `catalog:<popup position>` -> that position.
fn popup_position(value: &Value) -> Result<Option<usize>, String> {
    let choice = text(value);
    if choice.is_empty() {
        return Ok(None);
    }
    choice
        .strip_prefix("catalog:")
        .and_then(|i| i.parse().ok())
        .map(Some)
        .ok_or_else(|| "Choose an ATC from this return's list.".to_string())
}

/// Choose a Part II row's ATC: a new row ticks it in the popup, a changed
/// row keeps its tax base, blank unticks it. Rows re-sort into popup order.
fn fill_fq_atc(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1601FqDraft = typed(draft)?;
    let position = popup_position(value)?;
    let current = form.schedule.get(row).map(|r| r.popup_index);
    match (position, current) {
        (None, None) => return Ok(()),
        (None, Some(current)) => form.remove_atc(current),
        (Some(position), Some(current)) if position == current => return Ok(()),
        (Some(position), current) => {
            let Some(option) = form.atc_options().get(position) else {
                return Err("Not an ATC on the official 1601-FQ list.".to_string());
            };
            if form.schedule.iter().any(|r| r.popup_index == position) {
                return Err(format!("{} is already selected.", option.code));
            }
            match current {
                Some(_) => {
                    let entry = &mut form.schedule[row];
                    entry.popup_index = position;
                    entry.atc_code = option.code.to_string();
                    form.recompute();
                }
                None => form.add_atc(position)?,
            }
        }
    }
    store(draft, &form)
}

fn fill_fq_base(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let typed_text = text(value);
    let amount = if typed_text.is_empty() {
        0.0
    } else {
        bir_core::forms::agent::parse_amount(&typed_text)
            .filter(|amount| *amount >= 0.0)
            .ok_or_else(|| format!("`{typed_text}` is not an amount."))?
    };
    let mut form: Form1601FqDraft = typed(draft)?;
    let Some(entry) = form.schedule.get_mut(row) else {
        if typed_text.is_empty() {
            return Ok(());
        }
        return Err("Choose this row's ATC first.".to_string());
    };
    entry.tax_base = amount;
    form.recompute();
    store(draft, &form)
}

/// The 1601FQ Part II ATC list: the chosen category's popup (private before
/// Item 11 is answered, like `init()`), each entry by its position, the
/// row's own entry marked.
pub(super) fn options(kind: AgentFormKind, key: &str, draft: &Value) -> Option<Vec<Value>> {
    let Cell::FqAtc(row) = cell(kind, key)? else {
        return None;
    };
    // Without a draft (the schema's own lists), the private popup.
    let form: Option<Form1601FqDraft> = typed(draft).ok();
    let chosen = form
        .as_ref()
        .and_then(|form| form.schedule.get(row))
        .map(|r| r.popup_index);
    let list = form.as_ref().map_or(
        Form1601FqCategory::Private.atc_options(),
        Form1601FqDraft::atc_options,
    );
    Some(
        list.iter()
            .enumerate()
            .map(|(i, option)| {
                json!({
                    "value": format!("catalog:{i}"),
                    "label": format!("{} — {} ({}%)", option.code, option.description, option.rate_text),
                    "selected": chosen == Some(i),
                })
            })
            .collect(),
    )
}

/// What each Part II tax rate waits for while it shows blank: 1601EQ's and
/// 1601FQ's rate follows the row's ATC.
pub(super) fn waits(kind: AgentFormKind) -> Map<String, Value> {
    let (stem, rows) = match kind.form_code() {
        "1601EQ" => ("frm1601EQ:txtTaxRate", EQ_ROWS),
        "1601FQ" => ("frm1601FQ:txtTaxRate", FQ_ROWS),
        _ => return Map::new(),
    };
    let shown = super::schema_keys(kind.form_code());
    (1..=rows)
        .map(|n| format!("{stem}{n}"))
        .filter(|key| shown.contains(key))
        .map(|key| (key, json!(RATE_WAITS)))
        .collect()
}
