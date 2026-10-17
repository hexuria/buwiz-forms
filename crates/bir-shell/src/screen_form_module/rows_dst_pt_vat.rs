//! PR 4: the monthly DST, percentage tax and VAT returns (2000, 2553,
//! 2550M). Boxes the host writes through the typed draft because the probed
//! bindings cannot say what the printed sheet says, the boxes that wait for
//! another box of their row, and the answers that remove what the filer
//! typed.
//!
//! - 2000 Schedules 2, 3 and 4 belong to the Item 13 mode that opens them
//!   (`Form2000AffixtureMode::schedules`, the official
//!   `processModeAffixture`); `recompute` clears a schedule its mode does not
//!   use, so a write there before Item 13 opens it is refused with what to
//!   answer, never taken and silently cleared.
//! - 2553 rows 14 to 17: the July 1999 sheet preprints each row's
//!   classification, ATC and rate (html-frozen/2553-1999/index.html: Pagcor
//!   OT010 5%, Clark Development Corporations OT011 5%, Special/Regular
//!   Economic/Free Port Zone Enterprises OT011 5%, Others OT012), and the
//!   print fills only their amounts (writer-cells.json rule: "rows 14-17
//!   expose only some columns"). The page types a row's taxable amount; the
//!   host gives the row its printed ATC through `Form2553Draft::set_atc`, so
//!   the rate and the tax due follow. Row 18 takes an ATC from the official
//!   popup's list (`FORM_2553_ATC_OPTIONS`, `getATCCode`), whose duplicate
//!   check is the form's own. A stored draft keeps its rows as they are.
//! - 2553 Item 13: Yes opens the list blank (`YesUnspecified`, which
//!   validation asks for), No closes it; while it is No the list takes
//!   nothing (fields.json `enabled_when`: "Item 13 Yes is selected."), so
//!   rule 6 shows it off.
//! - 2553 Item 23's boxes take a choice only while Item 23 is an
//!   overpayment (the official `checkOverpayment`).
//! - An answer that removes what the filer typed (2000 Item 13's schedule
//!   rows, Item 10's other party, an Amended Return "No" that clears 2000's
//!   15A, 2553's 20A or 2550M's 23D, a cleared ATC with its tax base) is
//!   reported ([`removes`]) so the page asks first and offers Undo, the way
//!   PR 2's `rows_withholding::removes` does for the quarterly withholding
//!   returns.
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::forms::form_2000::Form2000Draft;
use bir_core::forms::form_2553::{FORM_2553_ATC_OPTIONS, Form2553AtcOption, Form2553Draft};
use serde_json::{Map, Value, json};

/// A box this module writes or closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// A 2000 Schedule 2, 3 or 4 box (`frm2000:sched<n>:...`).
    DstSchedule(u8),
    /// 2553 row `14 + .0` column C (taxable amount).
    PtAmount(usize),
    /// 2553 row `14 + .0` column D (tax rate).
    PtRate(usize),
    /// 2553 item 18B: the row's ATC from the popup's list.
    PtAtc18,
    /// 2553 item 13 Yes (`true`) or No.
    PtItem13(bool),
    /// 2553 item 13's list (`lstTaxTreaty`).
    PtItem13List,
    /// 2553 item 23's overpayment boxes.
    PtOverpayment,
}

/// The 2553 rows the sheet preprints, each with its printed entry of the
/// official popup (`FORM_2553_ATC_OPTIONS` index) and its printed caption.
pub(super) const PT_PRINTED_ROWS: [(usize, &str, &str, &str); 4] = [
    (
        0,
        "Philippine Amusement and Gaming Corp. (Pagcor)",
        "OT010",
        "5%",
    ),
    (2, "Clark Development Corporations", "OT011", "5%"),
    (
        3,
        "Special/Regular Economic/Free Port Zone Enterprises",
        "OT011",
        "5%",
    ),
    (1, "Others", "OT012", ""),
];
/// Rows 14-18.
const PT_ROWS: usize = 5;
/// 2553's first ATC item.
const PT_FIRST_ITEM: usize = 14;

/// What a 2000 Schedule 1 rate waits for while it is blank (PR 2's words
/// for the same box on 1601EQ and 1601FQ).
pub(super) const RATE_WAITS: &str = "Shows the ATC's rate once this row's ATC is chosen.";
/// What 2553's item 18A waits for.
pub(super) const ATC_18_WAITS: &str = "Shows the classification of the ATC chosen in 18B.";
/// The lock reason of a 2553 box the sheet preprints.
pub(super) const PRINTED_LOCK: &str =
    "printed on the form; filled in with the row's ATC when its taxable amount is typed";

fn pt_option(index: usize) -> &'static Form2553AtcOption {
    &FORM_2553_ATC_OPTIONS[index]
}

/// The printed option of a 2553 row 14-17.
fn printed_option(row: usize) -> Option<&'static Form2553AtcOption> {
    PT_PRINTED_ROWS
        .get(row)
        .map(|(index, _, _, _)| pt_option(*index))
}

/// What a schedule the 2000 Item 13 answer has not opened says when it is
/// written to.
pub(super) fn dst_schedule_closed(schedule: u8) -> &'static str {
    match schedule {
        2 => "Schedule 2 is for Constructive Affixture: choose it at item 13 first.",
        3 => "Schedule 3 is for the eDST System: choose it at item 13 first.",
        _ => {
            "Schedule 4 is for Constructive Affixture or Loose Stamps: choose one at item 13 first."
        }
    }
}

/// What 2553's item 23 boxes say while item 23 is not an overpayment.
pub(super) const NOT_OVERPAID: &str =
    "Item 23 is not an overpayment: mark a box here only when it is below zero.";

fn pt_item(key: &str, column: char) -> Option<usize> {
    let rest = key.strip_prefix("frm2553:txt")?;
    let n: usize = rest.strip_suffix(column)?.parse().ok()?;
    (PT_FIRST_ITEM..PT_FIRST_ITEM + PT_ROWS)
        .contains(&n)
        .then(|| n - PT_FIRST_ITEM)
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    match kind.form_code() {
        "2000" => {
            let rest = key.strip_prefix("frm2000:sched")?;
            let n = rest.chars().next()?.to_digit(10)?;
            (rest[1..].starts_with(':') && (2..=4).contains(&n))
                .then_some(Cell::DstSchedule(n as u8))
        }
        "2553" => match key {
            "frm2553:txt18B" => Some(Cell::PtAtc18),
            "frm2553:optTreaty_1" => Some(Cell::PtItem13(true)),
            "frm2553:optTreaty_2" => Some(Cell::PtItem13(false)),
            "frm2553:lstTaxTreaty" => Some(Cell::PtItem13List),
            "frm2553:ifoverpay_1" | "frm2553:ifoverpay_2" => Some(Cell::PtOverpayment),
            _ => pt_item(key, 'C')
                .map(Cell::PtAmount)
                .or_else(|| pt_item(key, 'D').map(Cell::PtRate)),
        },
        _ => None,
    }
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page). A 2000 schedule box and a 2553 item 23 box
/// are written by the ordinary fill once their answer holds.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    cell(kind, key).is_some_and(|cell| !matches!(cell, Cell::DstSchedule(_) | Cell::PtOverpayment))
}

/// A row's ATC is chosen, or its amount typed, before its rate.
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    match cell(kind, key)? {
        Cell::PtAmount(_) => Some(1),
        Cell::PtRate(_) => Some(2),
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

/// A typed amount: blank is zero.
fn amount(value: &Value) -> Result<f64, String> {
    let typed_text = text(value);
    if typed_text.is_empty() {
        return Ok(0.0);
    }
    agent::parse_amount(&typed_text)
        .filter(|amount| *amount >= 0.0)
        .ok_or_else(|| format!("`{typed_text}` is not an amount."))
}

/// Write `key` when this module owns it: `None` leaves it to the ordinary
/// fill. `ordinary` is that fill, for a box this module only gates.
pub(super) fn fill(
    kind: AgentFormKind,
    draft: &mut Value,
    key: &str,
    value: &Value,
    ordinary: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Option<Result<(), String>> {
    let cell = cell(kind, key)?;
    Some(match cell {
        Cell::DstSchedule(schedule) => fill_dst_schedule(draft, schedule, value, ordinary),
        Cell::PtOverpayment => {
            let overpaid = draft["total_amount_payable"].as_f64().unwrap_or(0.0) < 0.0;
            if ticked(value) && !overpaid {
                Err(NOT_OVERPAID.to_string())
            } else {
                ordinary(draft)
            }
        }
        Cell::PtAmount(row) => fill_pt_amount(draft, row, value),
        Cell::PtRate(row) => fill_pt_rate(draft, row, value),
        Cell::PtAtc18 => fill_pt_atc_18(draft, value),
        Cell::PtItem13(yes) => fill_pt_item_13(draft, yes, value),
        Cell::PtItem13List => fill_pt_item_13_list(draft, value),
    })
}

fn fill_dst_schedule(
    draft: &mut Value,
    schedule: u8,
    value: &Value,
    ordinary: impl FnOnce(&mut Value) -> Result<(), String>,
) -> Result<(), String> {
    let form: Form2000Draft = typed(draft)?;
    let (two, three, four) = form.affixture_mode.schedules();
    let open = match schedule {
        2 => two,
        3 => three,
        _ => four,
    };
    let blank = {
        let shown = text(value);
        shown.is_empty() || agent::parse_amount(&shown) == Some(0.0)
    };
    if !open && !blank {
        return Err(dst_schedule_closed(schedule).to_string());
    }
    ordinary(draft)
}

/// A 2553 row 14-17's taxable amount (or row 18's once its ATC is chosen).
fn fill_pt_amount(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let amount = amount(value)?;
    let mut form: Form2553Draft = typed(draft)?;
    let held = form
        .schedule
        .get(row)
        .filter(|entry| !entry.is_empty())
        .cloned();
    match (held, printed_option(row)) {
        // Cleared: a row holding its printed ATC goes back to blank.
        (Some(entry), Some(printed))
            if amount == 0.0
                && entry.atc_code == printed.code
                && entry.description == printed.description =>
        {
            form.clear_atc(row);
        }
        (Some(_), _) => {
            form.schedule[row].taxable_amount = amount;
            form.recompute();
        }
        (None, _) if amount == 0.0 => return Ok(()),
        (None, Some(printed)) => {
            form.set_atc(row, printed)?;
            form.schedule[row].taxable_amount = amount;
            form.recompute();
        }
        (None, None) => {
            return Err(format!("Choose the ATC in {}B first.", PT_FIRST_ITEM + row));
        }
    }
    store(draft, &form)
}

/// A 2553 row's rate: typed only for an ATC whose rate the filer enters
/// (OT012 Others, `rate_is_editable`); the others keep the official rate.
fn fill_pt_rate(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let rate = amount(value)?;
    let mut form: Form2553Draft = typed(draft)?;
    let item = PT_FIRST_ITEM + row;
    let option = match form.schedule.get(row).filter(|entry| !entry.is_empty()) {
        Some(entry) => FORM_2553_ATC_OPTIONS.iter().find(|option| {
            option.code == entry.atc_code && option.description == entry.description
        }),
        None if rate == 0.0 => return Ok(()),
        None => match printed_option(row) {
            Some(printed) if printed.rate_is_editable => {
                form.set_atc(row, printed)?;
                Some(printed)
            }
            Some(_) | None => {
                return Err(format!(
                    "Type the taxable amount in {item}C first; the rate follows its ATC."
                ));
            }
        },
    };
    match option {
        Some(option) if option.rate_is_editable => {
            form.schedule[row].tax_rate = rate;
            form.recompute();
            store(draft, &form)
        }
        Some(option) => Err(format!(
            "Item {item}D is the official rate of {}% for {}.",
            option.rate_text, option.code
        )),
        None => Err(format!(
            "Item {item}B is not an ATC the official form offers."
        )),
    }
}

/// `catalog:<index>` -> that entry of the official popup.
fn popup_entry(value: &Value) -> Result<Option<&'static Form2553AtcOption>, String> {
    let choice = text(value);
    if choice.is_empty() {
        return Ok(None);
    }
    choice
        .strip_prefix("catalog:")
        .and_then(|i| i.parse::<usize>().ok())
        .and_then(|i| FORM_2553_ATC_OPTIONS.get(i))
        .map(Some)
        .ok_or_else(|| "Choose an ATC from the official list.".to_string())
}

fn fill_pt_atc_18(draft: &mut Value, value: &Value) -> Result<(), String> {
    let row = PT_ROWS - 1;
    let mut form: Form2553Draft = typed(draft)?;
    match popup_entry(value)? {
        None => form.clear_atc(row),
        Some(option) => form.set_atc(row, option)?,
    }
    store(draft, &form)
}

fn fill_pt_item_13(draft: &mut Value, yes: bool, value: &Value) -> Result<(), String> {
    use bir_core::forms::form_2553::Form2553TaxTreaty;
    // A one-answer pair: the ticked box carries the answer.
    if !ticked(value) {
        return Ok(());
    }
    let mut form: Form2553Draft = typed(draft)?;
    form.tax_treaty = match (yes, form.tax_treaty) {
        (true, held) if held.is_yes() => held,
        // `enableSelTreaty`: Yes opens the list with nothing chosen.
        (true, _) => Form2553TaxTreaty::YesUnspecified,
        (false, _) => Form2553TaxTreaty::No,
    };
    form.recompute();
    store(draft, &form)
}

fn fill_pt_item_13_list(draft: &mut Value, value: &Value) -> Result<(), String> {
    use bir_core::forms::form_2553::Form2553TaxTreaty;
    let mut form: Form2553Draft = typed(draft)?;
    // Closed while item 13 is No: the list takes nothing.
    if !form.tax_treaty.is_yes() {
        return Ok(());
    }
    form.tax_treaty = match text(value).as_str() {
        "" | "0" => Form2553TaxTreaty::YesUnspecified,
        "1" => Form2553TaxTreaty::SpecialRate,
        "2" => Form2553TaxTreaty::InternationalTaxTreaty,
        other => return Err(format!("`{other}` is not an item 13 choice.")),
    };
    form.recompute();
    store(draft, &form)
}

/// 2553 item 18B's list: the official popup's entries, by position, the
/// row's own entry marked.
pub(super) fn options(kind: AgentFormKind, key: &str, draft: &Value) -> Option<Vec<Value>> {
    if cell(kind, key)? != Cell::PtAtc18 {
        return None;
    }
    let form: Option<Form2553Draft> = typed(draft).ok();
    let held = form
        .as_ref()
        .and_then(|form| form.schedule.get(PT_ROWS - 1).cloned())
        .filter(|row| !row.is_empty());
    Some(
        FORM_2553_ATC_OPTIONS
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let rate = if option.rate_is_editable {
                    "rate you enter".to_string()
                } else {
                    format!("{}%", option.rate_text)
                };
                json!({
                    "value": format!("catalog:{i}"),
                    "label": format!("{} — {} ({rate})", option.code, option.description),
                    "selected": held.as_ref().is_some_and(|row| row.atc_code == option.code && row.description == option.description),
                })
            })
            .collect(),
    )
}

/// Rule 4 for a schedule's date boxes: 2000's Schedules 2-4 tables (2000
/// R4.0) carry `calendar: true`, so each of their date boxes gets its own
/// calendar on the page (form_page.js `scheduleTable`). Other forms' tables
/// are their own groups' and keep typing only.
pub(super) fn mark_calendar_tables(form: &mut Value, code: &str) {
    if code != "2000" {
        return;
    }
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        let dated = item["cols"]
            .as_array()
            .is_some_and(|cols| cols.iter().any(|col| col["t"] == "date"));
        if dated {
            item["calendar"] = Value::Bool(true);
        }
    }
}

/// What each box the form fills from another box of its row waits for while
/// it shows blank: 2000's Schedule 1 rates follow the ATC; 2553's preprinted
/// rows 14-17 show their printed classification, ATC and rate, and 18A the
/// ATC chosen in 18B.
pub(super) fn waits(kind: AgentFormKind) -> Map<String, Value> {
    let mut out = Map::new();
    match kind.form_code() {
        "2000" => {
            for row in 0..bir_core::forms::form_2000::FORM_2000_ROWS {
                out.insert(format!("frm2000:sched1:txtTaxRate{row}"), json!(RATE_WAITS));
            }
        }
        "2553" => {
            for (row, (_, caption, atc, rate)) in PT_PRINTED_ROWS.iter().enumerate() {
                let item = PT_FIRST_ITEM + row;
                out.insert(
                    format!("frm2553:txt{item}A"),
                    json!(format!("Printed: {caption}")),
                );
                out.insert(
                    format!("frm2553:txt{item}B"),
                    json!(format!("Printed: {atc}")),
                );
                if !rate.is_empty() {
                    out.insert(
                        format!("frm2553:txt{item}D"),
                        json!(format!("Printed: {rate}")),
                    );
                }
            }
            out.insert("frm2553:txt18A".into(), json!(ATC_18_WAITS));
        }
        _ => {}
    }
    out
}

/// The boxes this module shuts, and why, over the probed locks: 2553's
/// preprinted cells and 18A, its quarter boxes (the return's own period),
/// and 2000's 15B and 15C, which `recompute` totals from Schedules 2 and 3.
pub(super) fn relock(kind: AgentFormKind, locks: &mut Map<String, Value>) {
    match kind.form_code() {
        "2000" => {
            for key in ["frm2000:txtTax15B", "frm2000:txtTax15C"] {
                locks.insert(key.into(), json!("computed by BIR's own formulas"));
            }
        }
        "2553" => {
            for key in waits(kind).keys() {
                locks.insert(key.clone(), json!(PRINTED_LOCK));
            }
            for quarter in 1..=4 {
                locks.insert(
                    format!("frm2553:optQtr:_{quarter}"),
                    json!(agent::Lock::Identity.reason()),
                );
            }
        }
        _ => {}
    }
}

fn amount_of(row: &Value, field: &str) -> f64 {
    row[field].as_f64().unwrap_or(0.0)
}

fn rows<'a>(draft: &'a Value, list: &str) -> Vec<&'a Value> {
    draft[list]
        .as_array()
        .map(|rows| rows.iter().collect())
        .unwrap_or_default()
}

/// Rows of a 2000 payment or remittance schedule the filer typed into.
fn typed_rows(draft: &Value, list: &str) -> usize {
    rows(draft, list)
        .into_iter()
        .filter(|row| {
            row.as_object().is_some_and(|fields| {
                fields.values().any(|value| match value {
                    Value::String(text) => !text.trim().is_empty(),
                    Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
                    _ => false,
                })
            })
        })
        .count()
}

fn count(n: usize, what: &str) -> String {
    format!("{n} {what} row{}", if n == 1 { "" } else { "s" })
}

/// `1234.5` -> `1,234.50`.
fn pesos(amount: f64) -> String {
    let cents = (amount * 100.0).round() as i64;
    let negative = cents < 0;
    let (whole, cents) = ((cents / 100).abs(), (cents % 100).abs());
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{}{grouped}.{cents:02}", if negative { "-" } else { "" })
}

/// When `edits` turned `before` into `after` by removing what the filer
/// typed, what the page asks first (`message`), what the Undo toast says
/// (`done`) and how many rows went; `None` when nothing typed went. The
/// host applies the edit either way; the page keeps its old draft until the
/// filer chooses Continue.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let code = kind.form_code();
    let edited = |prefix: &str| edits.keys().any(|key| key.starts_with(prefix));
    if code == "2000" && edited("frm2000:optMode_") {
        let lost: Vec<(usize, &str)> = [
            ("schedule2", "Schedule 2"),
            ("schedule3", "Schedule 3"),
            ("schedule4", "Schedule 4"),
        ]
        .into_iter()
        .map(|(list, name)| {
            (
                typed_rows(before, list).saturating_sub(typed_rows(after, list)),
                name,
            )
        })
        .filter(|(n, _)| *n > 0)
        .collect();
        let total: usize = lost.iter().map(|(n, _)| n).sum();
        if total > 0 {
            let parts: Vec<String> = lost.iter().map(|(n, what)| count(*n, what)).collect();
            return Some(json!({
                "message": format!("Changing item 13 removes the {} you typed. Continue?", parts.join(" and ")),
                "done": format!("Removed {total} row{}", if total == 1 { "" } else { "s" }),
                "rows": total,
            }));
        }
    }
    if code == "2000" && edited("frm2000:optParty_") {
        let typed = |draft: &Value| {
            ["other_party_name", "other_party_tin"]
                .iter()
                .any(|field| draft[*field].as_str().is_some_and(|s| !s.trim().is_empty()))
        };
        if typed(before) && !typed(after) {
            return Some(json!({
                "message": "Changing item 10 clears items 11 and 12 (the other party's name and TIN). Continue?",
                "done": "Cleared items 11 and 12",
                "rows": 0,
            }));
        }
    }
    // Amended Return Yes -> No clears the previous return's payment.
    let amended = match code {
        "2000" => Some(("frm2000:AmendedRtn_", "balance_carried_over", "15A")),
        "2553" => Some(("frm2553:optAmended_", "tax_paid_previous", "20A")),
        "2550M" => Some(("frm2550m:OptAmendedYN", "vat_paid_previous", "23D")),
        _ => None,
    };
    if let Some((prefix, field, item)) = amended
        && edited(prefix)
    {
        let previous = amount_of(before, field);
        if previous != 0.0 && amount_of(after, field) == 0.0 {
            return Some(json!({
                "message": format!(
                    "Changing Amended Return to No clears item {item} (₱{}). Continue?",
                    pesos(previous)
                ),
                "done": format!("Cleared item {item}"),
                "rows": 0,
            }));
        }
    }
    // A cleared ATC takes its row's base with it.
    let cleared_atc = edits.iter().any(|(key, value)| {
        text(value).is_empty()
            && ((code == "2000" && key.starts_with("drpATCCode"))
                || cell(kind, key) == Some(Cell::PtAtc18))
    });
    if cleared_atc {
        let (list, field) = if code == "2000" {
            ("schedule1", "tax_base")
        } else {
            ("schedule", "taxable_amount")
        };
        let base = |draft: &Value| -> f64 {
            rows(draft, list)
                .into_iter()
                .map(|row| amount_of(row, field))
                .sum()
        };
        let lost = base(before) - base(after);
        if lost > 0.0 {
            return Some(json!({
                "message": format!(
                    "Clearing this ATC removes its row and its amount of ₱{}. Continue?",
                    pesos(lost)
                ),
                "done": "Removed the row",
                "rows": 1,
            }));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn pesos_group_and_keep_the_sign() {
        assert_eq!(super::pesos(1234.5), "1,234.50");
        assert_eq!(super::pesos(1_000_000.0), "1,000,000.00");
        assert_eq!(super::pesos(-0.5), "-0.50");
        assert_eq!(super::pesos(0.0), "0.00");
    }

    /// The preprinted rows are the official popup's entries with the
    /// sheet's codes, and only Others takes a typed rate.
    #[test]
    fn printed_rows_are_the_popup_entries_with_their_printed_codes() {
        for (row, (_, _, atc, rate)) in super::PT_PRINTED_ROWS.iter().enumerate() {
            let option = super::printed_option(row).unwrap();
            assert_eq!(option.code, *atc, "row {}", 14 + row);
            assert_eq!(option.rate_is_editable, rate.is_empty(), "row {}", 14 + row);
            if !option.rate_is_editable {
                assert_eq!(format!("{}%", option.rate as u32), *rate);
            }
        }
    }
}
