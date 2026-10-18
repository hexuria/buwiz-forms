//! PR 9: the as-needed transfers and payments (0605, 1606, 1706, 1707,
//! 2000OT, 2552). Boxes the host writes through the typed draft because
//! the probed bindings cannot (a box showing one part of a draft field, a
//! whole number shown as one tick box per value), boxes the official page
//! keeps shut until another answer opens them, and the answers that remove
//! what the filer typed.
//!
//! - 0605 Item 3: one tick box per quarter, all four the draft's `quarter`
//!   (form_0605_xml.rs `insert_one_of_four`), which is the return's period
//!   identity (agent `PERIOD_FIELDS`): the boxes show it, set where the
//!   return is started (a sample's period part), and refuse a write on an
//!   opened return, saying what sets them.
//! - 1606 item 8C and 1706 item 7C: the other party's TIN, one draft field
//!   (`seller_tin`, `buyer_tin`) shown as three boxes and a branch code;
//!   the boxes fill in order (rows_1700.rs `with_tin_part`).
//! - Boxes the official page enables only on another answer refuse every
//!   write but a blank while that answer does not hold, with the tag the
//!   page shows on them ([`off`], rule 6), judged on the draft as the batch
//!   has recomputed it so far: 0605's item 17 Others box; 1606's relief list
//!   (item 11 Yes), Schedule 1 "Others" classification, Schedule 2
//!   specification and item 18's boxes (an over-remittance); 1706's items 8
//!   and 9 (an individual seller at item 3), relief list, Schedule 2
//!   specification and item 18's boxes (all rules/forms fields.json
//!   `enabled_when`); 2552's items 12A and 12B (an initial public offering
//!   at item 11) and Part IV rows 2-5 (the row before them used), which
//!   rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json
//!   keeps disabled at load. A blank (clearing) always goes through.
//! - 2552 item 11 picks the Part V schedule, and all three schedules keep
//!   their rows in the one draft list (form_2552.rs `schedule`): the
//!   binding alone would move the rows typed under one schedule into the
//!   next. The host answers it with `Form2552Draft::set_transaction`, which
//!   clears them as the official `kindOfTransaction` does, and reports the
//!   removal ([`removes`]) so the page asks first and offers Undo.
//! - 2552 boxes the corpus marks computed or the probe could not bind,
//!   which the filer types: Part IV rows 2-5's amounts
//!   (`exempt_transactions`), 12B (`outstanding_shares`) and Part V row 5
//!   while the schedule is not folded (row 5 reads OTHERS once the add-more
//!   rows hold rows 5 onward; they are edited there).
//! - 1707 item 8's Yes and No are the draft's `tax_relief` (`Some` holds
//!   8A): No clears what 8A holds, so it asks first. Schedule 2 row A's
//!   No. of Shares (no fields.json box) and Schedules 2 and 3's row D (row
//!   D reads OTHERS once the add-more rows hold row D onward) are written
//!   here. Its profile copy of the address shows the whole address the
//!   draft keeps, never the upload's cut first line.
//!
//! Field keys, the official XML and the print map are the forms' own and
//! unchanged.

use bir_core::forms::agent::AgentFormKind;
use bir_core::forms::form_0605::Form0605Draft;
use bir_core::forms::form_1606::Form1606Draft;
use bir_core::forms::form_1706::Form1706Draft;
use bir_core::forms::form_1707::{
    FORM_1707_SCHEDULE_ROWS, Form1707Draft, Form1707Expense, Form1707Shares,
};
use bir_core::forms::form_2552::{
    FORM_2552_ROWS, Form2552Draft, Form2552ExemptRow, Form2552ShareRow, Form2552Transaction,
};
use serde_json::{Map, Value};

use super::rows_1700::with_tin_part;

/// A box this module writes or gates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Cell {
    /// 0605 Item 3, quarter 1-4.
    Quarter0605(u8),
    /// 0605 item 17's Others (Specify) box.
    Others0605,
    /// 1606 item 8C: seller TIN box 0-2, or 3 (branch code).
    SellerTin1606(usize),
    /// 1706 item 7C: buyer TIN box 0-2.
    BuyerTin1706(usize),
    /// A box the official page opens only on another answer.
    Gate(Gate),
    /// 2552 item 11: Part V schedule 1-3.
    Transaction2552(u8),
    /// 2552 Part IV row 2-5's amount (row index 1-4).
    PartIvAmount2552(usize),
    /// 2552 item 12B.
    OutstandingShares2552,
    /// 2552 Part V row 5 of schedule 1-3.
    Row5_2552(u8, Row5),
    /// 1707 item 8 Yes (`true`) or No.
    TaxRelief1707(bool),
    /// 1707 Schedule 2 row `.0` (0 = A, 3 = D), column `.1` (0-3).
    Shares1707(usize, usize),
    /// 1707 Schedule 3 row D, column `.0` (0 particulars, 1 amount).
    ExpenseD1707(usize),
}

/// A column of 2552's Part V row 5.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Row5 {
    Date,
    Seller,
    Buyer,
    Corp,
    NumShares,
    TaxBase,
    TaxRate,
}

const ROW5_COLUMNS: [(&str, Row5); 7] = [
    ("Date", Row5::Date),
    ("Seller", Row5::Seller),
    ("Buyer", Row5::Buyer),
    ("Corp", Row5::Corp),
    ("NumShares", Row5::NumShares),
    ("TaxBase", Row5::TaxBase),
    ("TaxRate", Row5::TaxRate),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Gate {
    /// 1606 item 11's list: item 11 Yes.
    Relief1606,
    /// 1606 Schedule 1 "Others (Specify)": classification Others.
    ClassOther1606,
    /// 1606 Schedule 2 specification: Exempt or Others.
    Transaction1606,
    /// 1706 items 8 and 9: an individual seller (item 3 WI 450).
    Individual1706,
    /// 1706 item 11's list: item 11 Yes.
    Relief1706,
    /// 1706 Schedule 2 specification: Exempt or Others.
    Transaction1706,
    /// 1606 and 1706 item 18's boxes: item 18 below zero.
    Overpaid,
    /// 2552 items 12A and 12B: item 11 an initial public offering.
    Offering2552,
    /// 2552 Part IV row `.0` (index 1-4): the row before it used.
    PartIvRow2552(usize),
    /// 0605 item 17's Others box: Others is the manner.
    Others0605,
}

/// The tag of a 2552 gate (2552's corpus records no `enabled_when`; the
/// official runtime keeps these boxes disabled at load, rules/forms/
/// 2552-v2018/fixtures/runtime-control-inventory-v796.json).
pub(super) const OFFERING_2552: &str =
    "Needs item 11 = Initial Public Offering (Primary or Secondary)";

/// The tag of 2552's Part IV row `row` (1-based, 2-5).
pub(super) fn part_iv_tag(row: usize) -> String {
    format!("Needs Part IV row {}", row - 1)
}

pub(super) fn cell(kind: AgentFormKind, key: &str) -> Option<Cell> {
    let tin_part = |suffix: &str| match suffix {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        _ => None,
    };
    match kind.form_code() {
        "0605" => {
            if key == "frm0605:txtOthersName" {
                return Some(Cell::Others0605);
            }
            let n: u8 = key.strip_prefix("itemQuarter_")?.parse().ok()?;
            (1..=4).contains(&n).then_some(Cell::Quarter0605(n))
        }
        "1606" => match key {
            "frm1606:txtBranchCodeS" => Some(Cell::SellerTin1606(3)),
            "frm1606:selTreaty" => Some(Cell::Gate(Gate::Relief1606)),
            "frm1606:j_id393_7" => Some(Cell::Gate(Gate::ClassOther1606)),
            "frm1606:txtOthers20" => Some(Cell::Gate(Gate::Transaction1606)),
            "frm1606:opt37" | "frm1606:opt37:_1" | "frm1606:opt37:_2" => {
                Some(Cell::Gate(Gate::Overpaid))
            }
            _ => tin_part(key.strip_prefix("frm1606:txtTINS")?).map(Cell::SellerTin1606),
        },
        "1706" => match key {
            "frm1706:j_id392" | "frm1706:j_id392:_1" | "frm1706:j_id392:_2" | "frm1706:j_id393"
            | "frm1706:j_id393:_1" | "frm1706:j_id393:_2" => Some(Cell::Gate(Gate::Individual1706)),
            "frm1706:opt37" | "frm1706:opt37:_1" | "frm1706:opt37:_2" => {
                Some(Cell::Gate(Gate::Overpaid))
            }
            "frm1706:selTreaty" => Some(Cell::Gate(Gate::Relief1706)),
            "frm1706:txtOthers21" => Some(Cell::Gate(Gate::Transaction1706)),
            _ => tin_part(key.strip_prefix("frm1706:txtTINB")?).map(Cell::BuyerTin1706),
        },
        "2552" => cell_2552(key),
        "1707" => cell_1707(key),
        _ => None,
    }
}

fn cell_1707(key: &str) -> Option<Cell> {
    let rest = key.strip_prefix("frm1707:")?;
    match rest {
        "rdoPg1I8TaxReliefYes" => return Some(Cell::TaxRelief1707(true)),
        "rdoPg1I8TaxReliefNo" => return Some(Cell::TaxRelief1707(false)),
        // Row A's No. of Shares: fields.json lists no box for it.
        "txtPg2P4Sch2NoOfShares1" => return Some(Cell::Shares1707(0, 1)),
        "txtPg2P4Sch3Particulars4" => return Some(Cell::ExpenseD1707(0)),
        "txtPg2P4Sch3Amount4" => return Some(Cell::ExpenseD1707(1)),
        _ => {}
    }
    let row_d = rest.strip_prefix("txtPg2P4Sch2")?.strip_suffix('4')?;
    let column = [
        "NameOfCorpStock",
        "NoOfShares",
        "StockCertNo",
        "TaxBaseSellPrice",
    ]
    .iter()
    .position(|name| *name == row_d)?;
    Some(Cell::Shares1707(FORM_1707_SCHEDULE_ROWS - 1, column))
}

fn cell_2552(key: &str) -> Option<Cell> {
    let rest = key.strip_prefix("frm2552:")?;
    match rest {
        "rdoPg1I11TransactionLSE" => return Some(Cell::Transaction2552(1)),
        "rdoPg1I11TransactionPrimary" => return Some(Cell::Transaction2552(2)),
        "rdoPg1I11TransactionSecondary" => return Some(Cell::Transaction2552(3)),
        "txtPg1Pt1I12TotShare" => return Some(Cell::OutstandingShares2552),
        "txtPg1Pt1I12NumOfShare" => return Some(Cell::Gate(Gate::Offering2552)),
        _ => {}
    }
    if let Some(n) = rest
        .strip_prefix("txtPg2P4I")
        .and_then(|r| r.strip_suffix("TransClass"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        return (2..=FORM_2552_ROWS)
            .contains(&n)
            .then_some(Cell::Gate(Gate::PartIvRow2552(n - 1)));
    }
    if let Some(n) = rest
        .strip_prefix("txtPg2P4I")
        .and_then(|r| r.strip_suffix("Amount"))
        .and_then(|n| n.parse::<usize>().ok())
    {
        return (2..=FORM_2552_ROWS)
            .contains(&n)
            .then_some(Cell::PartIvAmount2552(n - 1));
    }
    let rest = rest.strip_prefix("txtPg2Pt5Sch")?;
    let schedule = rest.chars().next()?.to_digit(10)? as u8;
    let column = rest[1..].strip_suffix('5')?;
    let (_, col) = ROW5_COLUMNS.iter().find(|(name, _)| *name == column)?;
    // Schedule 1's rate is the fixed 6/10 of 1%.
    ((1..=3).contains(&schedule) && !(schedule == 1 && *col == Row5::TaxRate))
        .then_some(Cell::Row5_2552(schedule, *col))
}

/// Whether the host writes `key` itself (its "cannot be filled here" lock
/// does not apply on the page). A gated box keeps its own binding.
pub(super) fn host_writes(kind: AgentFormKind, key: &str) -> bool {
    cell(kind, key).is_some_and(|cell| !matches!(cell, Cell::Gate(_)))
}

/// A batch fills a TIN's boxes left to right, and a gated box after the
/// answer that opens it (item 11 Yes, then its list).
pub(super) fn edit_order(kind: AgentFormKind, key: &str) -> Option<usize> {
    match cell(kind, key)? {
        Cell::SellerTin1606(part) | Cell::BuyerTin1706(part) => Some(1 + part),
        Cell::Gate(_) => Some(1),
        // Item 11 picks the schedule before its rows are typed.
        Cell::Transaction2552(_) => Some(0),
        Cell::PartIvAmount2552(_) | Cell::OutstandingShares2552 | Cell::Row5_2552(..) => Some(1),
        // Item 8's answer before its 8A text in one batch.
        Cell::TaxRelief1707(_) => Some(0),
        Cell::Shares1707(..) | Cell::ExpenseD1707(_) => Some(1),
        // Item 17's manner before its Others text in one batch.
        Cell::Others0605 => Some(1),
        Cell::Quarter0605(_) => None,
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

/// A value that writes nothing: blank, unticked, or a list's empty choice.
fn blank(value: &Value) -> bool {
    let shown = text(value);
    shown.is_empty() || shown == "false"
}

fn typed<T: serde::de::DeserializeOwned>(draft: &Value) -> Result<T, String> {
    serde_json::from_value(draft.clone()).map_err(|e| e.to_string())
}

fn store<T: serde::Serialize>(draft: &mut Value, form: &T) -> Result<(), String> {
    *draft = serde_json::to_value(form).map_err(|e| e.to_string())?;
    Ok(())
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
    // A gated box is judged on the draft the batch has made so far, as the
    // form recomputes it (item 18's over-remittance is a computed total).
    if let Some(gate) = gate_of(cell)
        && !blank(value)
        && !open(gate, &recomputed(kind, draft))
    {
        return Some(Err(gate_tag(kind, gate, key)));
    }
    Some(match cell {
        Cell::Quarter0605(_) => Err(QUARTER_FROM_THE_PERIOD.to_string()),
        Cell::Others0605 => fill_0605_others(draft, value),
        Cell::SellerTin1606(part) => fill_1606_seller_tin(draft, part, value),
        Cell::BuyerTin1706(part) => fill_1706_buyer_tin(draft, part, value),
        Cell::Transaction2552(schedule) => fill_2552_transaction(draft, schedule, value),
        Cell::PartIvAmount2552(row) => fill_2552_part_iv(draft, row, value),
        Cell::OutstandingShares2552 => fill_2552_outstanding(draft, value),
        Cell::Row5_2552(_, column) => fill_2552_row5(draft, key, column, value),
        Cell::TaxRelief1707(yes) => fill_1707_tax_relief(draft, yes, value),
        Cell::Shares1707(row, column) => fill_1707_shares(draft, row, column, value),
        Cell::ExpenseD1707(column) => fill_1707_expense_d(draft, column, value),
        Cell::Gate(gate) => {
            // Unticking a box of a shut pair writes nothing: its binding's
            // "false" would set the other answer.
            if !open(gate, &recomputed(kind, draft)) && text(value) == "false" {
                Ok(())
            } else {
                ordinary(draft)
            }
        }
    })
}

/// What 0605's quarter boxes say: the quarter is the return's period.
pub(super) const QUARTER_FROM_THE_PERIOD: &str =
    "Set by the quarter you give when you start this return.";

/// `draft` as the form recomputes it (unchanged when it cannot).
fn recomputed(kind: AgentFormKind, draft: &Value) -> Value {
    let mut copy = draft.clone();
    if bir_core::forms::agent::compute(kind, &mut copy).is_err() {
        return draft.clone();
    }
    copy
}

/// The gate a box waits behind, if any.
fn gate_of(cell: Cell) -> Option<Gate> {
    match cell {
        Cell::Gate(gate) => Some(gate),
        Cell::OutstandingShares2552 => Some(Gate::Offering2552),
        Cell::Others0605 => Some(Gate::Others0605),
        Cell::PartIvAmount2552(row) => Some(Gate::PartIvRow2552(row)),
        _ => None,
    }
}

/// What a shut gate says, on the page (`off`) and in a refusal: the
/// corpus condition's reviewed tag (app/forms/condition-labels.json), or
/// 2552's own.
fn gate_tag(kind: AgentFormKind, gate: Gate, key: &str) -> String {
    match gate {
        Gate::Offering2552 => OFFERING_2552.to_string(),
        Gate::PartIvRow2552(row) => part_iv_tag(row + 1),
        _ => super::conditions::tag(kind.form_code(), key)
            .unwrap_or_else(|| "Needs another answer on this form".to_string()),
    }
}

/// Every gated box of `kind`, with its gate.
fn gated_keys(kind: AgentFormKind) -> Vec<(String, Gate)> {
    let keys: Vec<String> = match kind.form_code() {
        "0605" => vec!["frm0605:txtOthersName".into()],
        "1606" => [
            "selTreaty",
            "j_id393_7",
            "txtOthers20",
            "opt37:_1",
            "opt37:_2",
        ]
        .iter()
        .map(|k| format!("frm1606:{k}"))
        .collect(),
        "1706" => [
            "j_id392:_1",
            "j_id392:_2",
            "j_id393:_1",
            "j_id393:_2",
            "selTreaty",
            "txtOthers21",
            "opt37:_1",
            "opt37:_2",
        ]
        .iter()
        .map(|k| format!("frm1706:{k}"))
        .collect(),
        "2552" => {
            let mut keys = vec![
                "frm2552:txtPg1Pt1I12NumOfShare".to_string(),
                "frm2552:txtPg1Pt1I12TotShare".to_string(),
            ];
            for n in 2..=FORM_2552_ROWS {
                keys.push(format!("frm2552:txtPg2P4I{n}TransClass"));
                keys.push(format!("frm2552:txtPg2P4I{n}Amount"));
            }
            keys
        }
        _ => Vec::new(),
    };
    keys.into_iter()
        .filter_map(|key| {
            let gate = gate_of(cell(kind, &key)?)?;
            Some((key, gate))
        })
        .collect()
}

/// A value shown as nothing (conditions.rs `is_blank`).
fn shows_nothing(shown: &str) -> bool {
    let shown = shown.trim();
    shown == "false" || shown.chars().all(|c| matches!(c, '0' | '.' | ',' | '-'))
}

/// The host gates that are shut on `draft` right now, each with its tag,
/// for the boxes that show nothing (rule 6; conditions.rs `off_boxes`
/// takes these, since its probe sees a refusal rather than a clearing).
pub(super) fn off(kind: AgentFormKind, draft: &Value, values: &Value) -> Map<String, Value> {
    let gated = gated_keys(kind);
    if gated.is_empty() {
        return Map::new();
    }
    let seen = recomputed(kind, draft);
    gated
        .into_iter()
        .filter(|(key, gate)| {
            shows_nothing(values[key.as_str()].as_str().unwrap_or_default()) && !open(*gate, &seen)
        })
        .map(|(key, gate)| {
            let tag = gate_tag(kind, gate, &key);
            (key, Value::from(tag))
        })
        .collect()
}

#[cfg(test)]
/// The condition text of a 2552 host gate (2552's corpus has none), for
/// the shared rule-6 tests: `(key, condition)`.
pub(super) fn host_condition(code: &str, key: &str) -> Option<String> {
    let kind = AgentFormKind::from_code(code)?;
    if code != "2552" {
        return None;
    }
    gated_keys(kind)
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, gate)| gate_tag(kind, gate, key))
}

/// 0605 item 17's Others box: written while Others is the manner
/// (form_0605_official.rs clears it otherwise, as `disabletxtOthers()`).
#[inline(never)]
fn fill_0605_others(draft: &mut Value, value: &Value) -> Result<(), String> {
    let mut form: Form0605Draft = typed(draft)?;
    form.other_manner_description = text(value);
    form.recompute();
    store(draft, &form)
}

/// Whether a gated box's answer holds on `draft`.
fn open(gate: Gate, draft: &Value) -> bool {
    let is = |path: &str, values: &[&str]| {
        draft
            .pointer(path)
            .and_then(Value::as_str)
            .is_some_and(|v| values.contains(&v))
    };
    let below_zero = |path: &str| {
        draft
            .pointer(path)
            .and_then(Value::as_f64)
            .is_some_and(|n| n < 0.0)
    };
    match gate {
        // fields.json enabled_when, frm1606:selTreaty "Item 19 indicates
        // treaty or special-law relief." and frm1706:selTreaty "Item 20
        // indicates tax-treaty relief.": item 19/20 is the old numbering of
        // item 11, whose Yes answers "tax relief under an International Tax
        // Treaty or Special Law" (the printed question); the list then
        // picks which. So Yes opens it on both forms.
        Gate::Relief1606 | Gate::Relief1706 => {
            !is("/tax_relief", &["no"]) && draft.pointer("/tax_relief").is_some()
        }
        Gate::ClassOther1606 => is("/property_class", &["others"]),
        Gate::Transaction1606 => is("/transaction", &["exempt", "others"]),
        Gate::Individual1706 => is("/seller_type", &["individual"]),
        Gate::Transaction1706 => is("/transaction_type", &["exempt", "others"]),
        // fields.json "Item 36 is negative.": 1606's total_amount_due,
        // 1706's total_amount_payable.
        Gate::Overpaid => below_zero("/total_amount_due") || below_zero("/total_amount_payable"),
        Gate::Others0605 => is("/manner_of_payment", &["Others"]),
        Gate::Offering2552 => is("/transaction", &["primary_offering", "secondary_offering"]),
        Gate::PartIvRow2552(row) => {
            draft["exempt_transactions"]
                .get(row - 1)
                .is_some_and(|before| {
                    before["classification"]
                        .as_str()
                        .is_some_and(|c| !c.trim().is_empty())
                        || before["amount"].as_f64().is_some_and(|a| a != 0.0)
                })
        }
    }
}

#[inline(never)]
fn fill_1606_seller_tin(draft: &mut Value, part: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1606Draft = typed(draft)?;
    form.seller_tin = with_tin_part(&form.seller_tin, part, &text(value))?;
    form.recompute();
    store(draft, &form)
}

/// 1706 keeps the buyer's nine TIN digits and the branch code apart
/// (`buyer_tin`, `buyer_branch_code`, its own bound box): a whole TIN
/// pasted into the first box fills both.
#[inline(never)]
fn fill_1706_buyer_tin(draft: &mut Value, part: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1706Draft = typed(draft)?;
    let digits = with_tin_part(&form.buyer_tin, part, &text(value))?;
    if digits.len() > 9 {
        form.buyer_branch_code = digits[9..].to_string();
    }
    form.buyer_tin = digits.chars().take(9).collect();
    form.recompute();
    store(draft, &form)
}

/// A typed amount: blank is zero.
fn amount(value: &Value) -> Result<f64, String> {
    let shown = text(value);
    if shown.is_empty() {
        return Ok(0.0);
    }
    bir_core::forms::agent::parse_amount(&shown)
        .filter(|amount| *amount >= 0.0)
        .ok_or_else(|| format!("`{shown}` is not an amount."))
}

/// 2552 item 11: the official `kindOfTransaction` clears the Part V rows
/// when the schedule changes (`Form2552Draft::set_transaction`); unticking
/// a box answers nothing (a radio: another box is ticked instead).
#[inline(never)]
fn fill_2552_transaction(draft: &mut Value, schedule: u8, value: &Value) -> Result<(), String> {
    if !ticked(value) {
        return Ok(());
    }
    let mut form: Form2552Draft = typed(draft)?;
    form.set_transaction(match schedule {
        1 => Form2552Transaction::LocalStockExchange,
        2 => Form2552Transaction::PrimaryOffering,
        _ => Form2552Transaction::SecondaryOffering,
    });
    store(draft, &form)
}

#[inline(never)]
fn fill_2552_part_iv(draft: &mut Value, row: usize, value: &Value) -> Result<(), String> {
    let amount = amount(value)?;
    let mut form: Form2552Draft = typed(draft)?;
    if amount == 0.0 && form.exempt_transactions.len() <= row {
        return Ok(());
    }
    if form.exempt_transactions.len() <= row {
        form.exempt_transactions
            .resize(row + 1, Form2552ExemptRow::default());
    }
    form.exempt_transactions[row].amount = amount;
    while form
        .exempt_transactions
        .last()
        .is_some_and(|last| *last == Form2552ExemptRow::default())
    {
        form.exempt_transactions.pop();
    }
    form.recompute();
    store(draft, &form)
}

#[inline(never)]
fn fill_2552_outstanding(draft: &mut Value, value: &Value) -> Result<(), String> {
    let mut form: Form2552Draft = typed(draft)?;
    form.outstanding_shares = text(value);
    form.recompute();
    store(draft, &form)
}

/// A typed date as rows 1-4's binding takes it (agent `reformat_date`,
/// MM/DD/YYYY): blank stays blank, anything else must read as a date.
fn mdy(text: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some(String::new());
    }
    let parts: Vec<&str> = text
        .trim()
        .split(['/', '-', '.'])
        .filter(|part| !part.is_empty())
        .collect();
    let (year, month, day) = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => (*y, *m, *d),
        [m, d, y] if y.len() == 4 => (*y, *m, *d),
        _ => return None,
    };
    let year: u16 = year.parse().ok()?;
    let month: u8 = month.parse().ok().filter(|m| (1..=12).contains(m))?;
    let day: u8 = day.parse().ok().filter(|d| (1..=31).contains(d))?;
    Some(format!("{month:02}/{day:02}/{year:04}"))
}

/// What a 2552 row 5 box says once the add-more rows hold row 5 onward.
pub(super) const ROW5_FOLDED: &str = "Row 5 reads OTHERS while the add-more rows hold row 5 onward: change it in the add-more table.";

#[inline(never)]
fn fill_2552_row5(draft: &mut Value, key: &str, column: Row5, value: &Value) -> Result<(), String> {
    let mut form: Form2552Draft = typed(draft)?;
    if form.schedule_is_folded() {
        return Err(ROW5_FOLDED.to_string());
    }
    let at = FORM_2552_ROWS - 1;
    let shown = text(value);
    let number = matches!(column, Row5::NumShares | Row5::TaxBase | Row5::TaxRate);
    let filling = if number {
        amount(value)? != 0.0
    } else {
        !shown.is_empty()
    };
    if form.schedule.len() <= at {
        if !filling {
            return Ok(());
        }
        form.schedule.resize(at + 1, Form2552ShareRow::default());
    }
    let row = &mut form.schedule[at];
    match column {
        Row5::Date => {
            row.date = mdy(&shown)
                .ok_or_else(|| format!("`{key}` must be a date as MM/DD/YYYY (got `{shown}`)"))?
        }
        Row5::Seller => row.seller = shown,
        Row5::Buyer => row.buyer = shown,
        Row5::Corp => row.issuing_corporation = shown,
        Row5::NumShares => row.number_of_shares = amount(value)?,
        Row5::TaxBase => row.tax_base = amount(value)?,
        Row5::TaxRate => row.tax_rate = amount(value)?,
    }
    // A cleared last row goes, as the official Delete does.
    let lse = form.transaction == Form2552Transaction::LocalStockExchange;
    while form.schedule.last().is_some_and(|last| {
        last.date.is_empty()
            && last.seller.is_empty()
            && last.buyer.is_empty()
            && last.issuing_corporation.is_empty()
            && last.number_of_shares == 0.0
            && last.tax_base == 0.0
            && (last.tax_rate == 0.0 || lse)
    }) {
        form.schedule.pop();
    }
    form.recompute();
    store(draft, &form)
}

/// 1707 item 8: the ticked box carries the answer; Yes keeps what 8A
/// holds, No clears it (asked first: [`removes`]).
#[inline(never)]
fn fill_1707_tax_relief(draft: &mut Value, yes: bool, value: &Value) -> Result<(), String> {
    if !ticked(value) {
        return Ok(());
    }
    let mut form: Form1707Draft = typed(draft)?;
    form.tax_relief = if yes {
        Some(form.tax_relief.take().unwrap_or_default())
    } else {
        None
    };
    form.recompute();
    store(draft, &form)
}

/// What a 1707 row D box says once the add-more rows hold row D onward.
pub(super) const ROW_D_FOLDED: &str = "Row D reads OTHERS while the add-more rows hold row D onward: change it in the add-more table.";

#[inline(never)]
fn fill_1707_shares(
    draft: &mut Value,
    row: usize,
    column: usize,
    value: &Value,
) -> Result<(), String> {
    let mut form: Form1707Draft = typed(draft)?;
    if row == FORM_1707_SCHEDULE_ROWS - 1 && form.shares.len() > FORM_1707_SCHEDULE_ROWS {
        return Err(ROW_D_FOLDED.to_string());
    }
    let shown = text(value);
    let filling = match column {
        1 | 3 => amount(value)? != 0.0,
        _ => !shown.is_empty(),
    };
    if form.shares.len() <= row {
        if !filling {
            return Ok(());
        }
        form.shares.resize(row + 1, Form1707Shares::default());
    }
    // The official boxes' limits (form_1707.rs validate: corporation names
    // hold 100 characters, certificate numbers up to 40 digits).
    match column {
        0 if shown.chars().count() > 100 => {
            return Err("Corporation names hold at most 100 characters.".to_string());
        }
        2 if shown.len() > 40 || !shown.chars().all(|c| c.is_ascii_digit()) => {
            return Err("Certificate numbers hold up to 40 digits.".to_string());
        }
        _ => {}
    }
    let entry = &mut form.shares[row];
    match column {
        0 => entry.corporation = shown,
        1 => entry.number_of_shares = (!shown.is_empty()).then(|| amount(value)).transpose()?,
        2 => entry.certificate_number = shown,
        _ => entry.selling_price = amount(value)?,
    }
    while form
        .shares
        .last()
        .is_some_and(|last| *last == Form1707Shares::default())
    {
        form.shares.pop();
    }
    form.recompute();
    store(draft, &form)
}

#[inline(never)]
fn fill_1707_expense_d(draft: &mut Value, column: usize, value: &Value) -> Result<(), String> {
    let mut form: Form1707Draft = typed(draft)?;
    if form.expenses.len() > FORM_1707_SCHEDULE_ROWS {
        return Err(ROW_D_FOLDED.to_string());
    }
    let row = FORM_1707_SCHEDULE_ROWS - 1;
    let shown = text(value);
    let filling = if column == 1 {
        amount(value)? != 0.0
    } else {
        !shown.is_empty()
    };
    if form.expenses.len() <= row {
        if !filling {
            return Ok(());
        }
        form.expenses.resize(row + 1, Form1707Expense::default());
    }
    if column == 0 && shown.chars().count() > 100 {
        return Err("Particulars hold at most 100 characters.".to_string());
    }
    if column == 1 {
        form.expenses[row].amount = amount(value)?;
    } else {
        form.expenses[row].particulars = shown;
    }
    while form
        .expenses
        .last()
        .is_some_and(|last| *last == Form1707Expense::default())
    {
        form.expenses.pop();
    }
    form.recompute();
    store(draft, &form)
}

/// What the page shows where the field map's value is not what the draft
/// keeps (display only; the field map, upload and print are unchanged):
/// 1707's profile copy of the address is the whole address, never the
/// upload's cut first line (form_1707.rs address_parts), so an edit there
/// never drops the second line.
pub(super) fn values(
    kind: AgentFormKind,
    draft: &Value,
    map: &mut std::collections::BTreeMap<String, String>,
) {
    if kind.form_code() == "1707"
        && let Some(address) = draft["registered_address"].as_str()
        && map.contains_key("frm1707:ProfileAddress1")
    {
        map.insert("frm1707:ProfileAddress1".into(), address.to_uppercase());
    }
}

/// The boxes this module shuts, and why, over the probed locks: 2552's
/// add-more tax due (h) cells and Schedule 1's fixed rate (g), which
/// form_2552.rs `recompute` computes (the probe saw them change with the
/// item 11 answer and called them derived), and the add-more subtotals.
pub(super) fn relock(kind: AgentFormKind, locks: &mut Map<String, Value>) {
    // 0605's quarter is the return's period identity: an opened return
    // keeps it (a sample's period parts open again in opened_json).
    if kind.form_code() == "0605" {
        for n in 1..=4 {
            locks.insert(
                format!("itemQuarter_{n}"),
                Value::from(bir_core::forms::agent::Lock::Identity.reason()),
            );
        }
        return;
    }
    if kind.form_code() != "2552" {
        return;
    }
    let computed = Value::from("computed by BIR's own formulas");
    for n in 1..=9 {
        for schedule in 1..=3 {
            locks.insert(
                format!("frm2552:txtPg2Pt5Sch{schedule}_{n}Col8"),
                computed.clone(),
            );
        }
        locks.insert(format!("frm2552:txtPg2Pt5Sch1_{n}Col7"), computed.clone());
    }
    for schedule in 1..=3 {
        locks.insert(format!("Pg2Pt5Sch{schedule}SubTotal"), computed.clone());
    }
}

/// Rule 4 for a schedule's date boxes: 2552's and 1707's tables with a date
/// column carry `calendar: true`, so each of their date boxes gets its own
/// calendar on the page (form_page.js `scheduleTable`, PR 4's hunk for
/// 2000, taken unchanged). Other forms' tables are their own groups'.
pub(super) fn mark_calendar_tables(form: &mut Value, code: &str) {
    if !matches!(code, "2552" | "1707") {
        return;
    }
    if code == "1707" {
        order_1707_sellers(form);
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

/// 1707 item 6's extracted table keeps its columns in key order (address,
/// name, TIN): the sheet prints Name | Registered Address | TIN on rows
/// 6A-6C (html-frozen/1707-2021/writer-cells.json "6A Seller name", "6A
/// Seller's Registered Address", "6A Seller's Taxpayer Identification
/// Number").
fn order_1707_sellers(form: &mut Value) {
    let items = form["parts"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array_mut().into_iter().flatten());
    for item in items {
        if item["keys"][0][0] != "frm1707:txtPg1I6SellerAddr0" {
            continue;
        }
        let order = [1, 0, 2];
        let cols = item["cols"].clone();
        item["cols"] = Value::Array(order.iter().map(|&c| cols[c].clone()).collect());
        for (col, caption) in item["cols"].as_array_mut().unwrap().iter_mut().zip([
            "Seller's Name",
            "Seller's Registered Address",
            "Seller's Taxpayer Identification Number (TIN)",
        ]) {
            col["l"] = Value::from(caption);
        }
        if let Some(rows) = item["keys"].as_array_mut() {
            for row in rows {
                let old = row.clone();
                *row = Value::Array(order.iter().map(|&c| old[c].clone()).collect());
            }
        }
        item["no"] = Value::from("6");
        item["l"] = Value::from("Seller's Name, Registered Address and TIN (rows 6A-6C)");
        item["row_labels"] = serde_json::json!({"caption": "Row", "rows": ["6A", "6B", "6C"]});
    }
}

/// What Schedule 1's add-more rate shows while its row is blank.
pub(super) const LSE_RATE_WAITS: &str = "Schedule 1's rate is 6/10 of 1%.";

/// What each box the form fills from another box waits for while it shows
/// blank (the page shows it in the box).
pub(super) fn waits(kind: AgentFormKind, draft: &Value) -> Map<String, Value> {
    let _ = draft;
    let mut out = Map::new();
    let fmv_boxes = |prefix: &str| {
        [
            ("A", "FMV of Land per Tax Declaration"),
            ("B", "FMV of Improvements per Tax Declaration"),
            ("C", "FMV of Land per BIR (zonal value)"),
            ("D", "FMV of Improvements per BIR"),
        ]
        .map(|(letter, what)| {
            (
                format!("{prefix}{letter}"),
                format!("Ticked by the form once Schedule 1's {what} holds an amount."),
            )
        })
    };
    match kind.form_code() {
        "0605" => {
            for n in 1..=4 {
                out.insert(
                    format!("itemQuarter_{n}"),
                    Value::from(QUARTER_FROM_THE_PERIOD),
                );
            }
        }
        "2000OT" => {
            // Item 1 is the return's identity (decision 10: matched by its
            // exact date); a return opened without one says what sets it.
            out.insert(
                "frm2000OT:txtTransactionDate".into(),
                Value::from(DATE_FROM_NEW_RETURN),
            );
        }
        // The older layout's FMV-basis boxes follow Schedule 1's amounts
        // (form_1606.rs / form_1706.rs `to_bir_field_map`).
        "1606" => {
            for (key, why) in fmv_boxes("frm1606:opt27") {
                out.insert(key, Value::from(why));
            }
        }
        "1706" => {
            for (key, why) in fmv_boxes("frm1706:opt29") {
                out.insert(key, Value::from(why));
            }
        }
        "2552" => {
            for n in 1..=9 {
                out.insert(
                    format!("frm2552:txtPg2Pt5Sch1_{n}Col7"),
                    Value::from(LSE_RATE_WAITS),
                );
            }
        }
        _ => {}
    }
    out
}

/// What a blank identity date waits for.
pub(super) const DATE_FROM_NEW_RETURN: &str =
    "Set by the date of transaction you start this return with (New return).";

/// Validation messages that name the official program's item numbers,
/// rewritten to the 2018 sheet's printed ones (maintainer decision D6,
/// 2026-10-18: the wording only; the checks and the filed data stay the
/// same). `(form, official message, printed message)`.
pub(super) const PRINTED_MESSAGES: &[(&str, &str, &str)] = &[
    // 1606: the official program numbers the older layout's items.
    (
        "1606",
        "Item 3 holds at most two digits.",
        "Item 4 holds at most two digits.",
    ),
    (
        "1606",
        "Please select an option for Item 4.",
        "Please select an option for Item 3.",
    ),
    (
        "1606",
        "Item 9 holds at most 50 characters.",
        "Item 7A Buyer's Name holds at most 50 characters.",
    ),
    (
        "1606",
        "Item 10 holds at most 50 characters.",
        "Item 8A Seller's Name holds at most 50 characters.",
    ),
    (
        "1606",
        "Item 11 holds at most 70 characters.",
        "Item 7A Buyer's Registered Address holds at most 70 characters.",
    ),
    (
        "1606",
        "Item 12 holds at most 70 characters.",
        "Item 8A Seller's Registered Address holds at most 70 characters.",
    ),
    (
        "1606",
        "Please select an option for Item 13.",
        "Please select an option for Item 9.",
    ),
    (
        "1606",
        "Please select an option for Item 14.",
        "Please select an option for Item 6.",
    ),
    (
        "1606",
        "Please select an option for Item 15.",
        "Please select an option for the Part IV Schedule 1 Classification.",
    ),
    (
        "1606",
        "Specify the classification of the property on Item 15.",
        "Specify the classification of the property in Part IV Schedule 1.",
    ),
    (
        "1606",
        "Item 15 \"Others\" is specified only when Others is marked.",
        "Part IV Schedule 1 \"Others\" is specified only when Others is marked.",
    ),
    (
        "1606",
        "Item 15 \"Others\" holds at most 60 characters.",
        "Part IV Schedule 1 \"Others\" holds at most 60 characters.",
    ),
    (
        "1606",
        "Please enter the Location of the Property on Item 16.",
        "Please enter the Location of the Property in Part IV Schedule 1.",
    ),
    (
        "1606",
        "Item 16 holds at most 125 characters.",
        "Part IV Schedule 1 Location holds at most 125 characters.",
    ),
    (
        "1606",
        "Please enter the RDO Code on Item 16A.",
        "Please enter the RDO Code on Item 5.",
    ),
    (
        "1606",
        "Item 17 TCT/OCT/CCT No. holds at most 20 letters, digits or dashes.",
        "Part IV Schedule 1 OCT/TCT/CCT No. holds at most 20 letters, digits or dashes.",
    ),
    (
        "1606",
        "Item 17 Area Sold holds at most 20 digits.",
        "Part IV Schedule 1 Area holds at most 20 digits.",
    ),
    (
        "1606",
        "Item 17 Tax Dec. No. holds at most 25 digits.",
        "Part IV Schedule 1 Tax Declaration No. holds at most 25 digits.",
    ),
    (
        "1606",
        "Item 17 Others holds at most 25 characters.",
        "The Others description (the older layout's item 17; the 2018 sheet prints no box for it, see Not on the printed sheet) holds at most 25 characters.",
    ),
    (
        "1606",
        "Please select a tax relief for Item 19.",
        "Please select a tax relief for Item 11.",
    ),
    (
        "1606",
        "Please select an option for Item 20.",
        "Please select an option for Part IV Schedule 2 Item 1.",
    ),
    (
        "1606",
        "Specify the Exempt or Others transaction on Item 20.",
        "Specify the Exempt or Others transaction on Part IV Schedule 2 Item 1.",
    ),
    (
        "1606",
        "Item 20 is specified only for an Exempt or Others transaction.",
        "Part IV Schedule 2 Item 1 is specified only for an Exempt or Others transaction.",
    ),
    (
        "1606",
        "Item 20 specification holds at most 60 characters.",
        "Part IV Schedule 2 Item 1 specification holds at most 60 characters.",
    ),
    (
        "1606",
        "Items 21 to 26 apply only to an installment sale.",
        "Part IV Schedule 2 Items 2 to 7 apply only to an installment sale.",
    ),
    (
        "1606",
        "Items 28C, 28D and 28E are disabled for a cash sale.",
        "Part IV Schedule 3's Bid Price (Item 1A), Items 1C and 1D are disabled for a cash sale.",
    ),
    (
        "1606",
        "Enter all four Item 27 values (0 where there is none) so Item 28B is computed.",
        "Enter all four Part IV Schedule 1 FMV values (0 where there is none) so Schedule 3 Item 1B is computed.",
    ),
    (
        "1606",
        "Item 28E description holds at most 40 characters.",
        "Part IV Schedule 3 Item 1D description holds at most 40 characters.",
    ),
    (
        "1606",
        "Please select the tax rate on Item 31.",
        "Please select the tax rate on Item 13.",
    ),
    (
        "1606",
        "Items 31 and 35 are disabled when Item 4 is No.",
        "Items 13 and 17A to 17C are disabled when Item 3 is No.",
    ),
    (
        "1606",
        "Item 33 applies only to an amended return.",
        "Item 15 applies only to an amended return.",
    ),
    (
        "1606",
        "Please indicate refund type for overremittance in Item 37.",
        "Please indicate refund type for overremittance in Item 18.",
    ),
    // 1706: likewise.
    (
        "1706",
        "Item 3 holds at most two digits.",
        "Item 4 holds at most two digits.",
    ),
    (
        "1706",
        "Please select an option for Item 4.",
        "Please select an option for Item 3.",
    ),
    (
        "1706",
        "Please enter a valid TIN number on Item 5.",
        "Please enter a valid TIN number on Item 6A (the seller's TIN).",
    ),
    (
        "1706",
        "Please enter a valid Seller's RDO Code on Item 6.",
        "Please enter a valid Seller's RDO Code (under Not on the printed sheet).",
    ),
    (
        "1706",
        "Please enter the RDO Code on Item 14A.",
        "Please enter the RDO Code on Item 5.",
    ),
    (
        "1706",
        "Please select an option for Item 15.",
        "Please select an option for the Part IV Schedule 1 Classification.",
    ),
    (
        "1706",
        "Please specify the classification of the property on Item 15.",
        "Please specify the classification of the property in Part IV Schedule 1.",
    ),
    (
        "1706",
        "Item 16 Others holds at most 25 characters.",
        "The Others description (the older layout's item 16; the 2018 sheet prints no box for it, see Not on the printed sheet) holds at most 25 characters.",
    ),
    (
        "1706",
        "Please select an option for Item 17.",
        "Please select an option for Item 8.",
    ),
    (
        "1706",
        "Please select an option for Item 18.",
        "Please select an option for Item 9.",
    ),
    (
        "1706",
        "Please select an option for Item 19.",
        "Please select an option for Item 10.",
    ),
    (
        "1706",
        "Please specify tax relief you are availing for Item 20.",
        "Please specify tax relief you are availing for Item 11.",
    ),
    (
        "1706",
        "Please select an option for Item 21.",
        "Please select an option for Part IV Schedule 2.",
    ),
    (
        "1706",
        "Item 21 description holds at most 60 characters.",
        "Part IV Schedule 2 description holds at most 60 characters.",
    ),
    (
        "1706",
        "Item 28 takes the installment date as MM/DD/YYYY.",
        "Part IV Schedule 2 Item 7 takes the installment date as MM/DD/YYYY.",
    ),
    (
        "1706",
        "Please choose Determination of taxable base in Item 30",
        "Please choose Determination of taxable base in Part IV Schedule 4",
    ),
    (
        "1706",
        "Item 30E applies only when Items 17 and 18 are both Yes.",
        "Part IV Schedule 4 Item 1C applies only when Items 8 and 9 are both Yes.",
    ),
    (
        "1706",
        "Item 30F description holds at most 40 characters.",
        "Part IV Schedule 4 Item 1E description holds at most 40 characters.",
    ),
    (
        "1706",
        "Item 31 cannot be determined yet. Complete Items 29 and 30 for this transaction.",
        "Item 12 cannot be determined yet. Complete Part IV Schedules 1 and 4 for this transaction.",
    ),
    (
        "1706",
        "Item 33 applies only to an amended return.",
        "Item 15 applies only to an amended return.",
    ),
];

/// The page's wording of `message`: the printed item numbers where the
/// official program names its own (D6), else `None`.
pub(super) fn printed_message(kind: AgentFormKind, message: &str) -> Option<&'static str> {
    let code = kind.form_code();
    PRINTED_MESSAGES
        .iter()
        .find(|(form, official, _)| *form == code && *official == message)
        .map(|(_, _, printed)| *printed)
}

/// 1606 and 1706 item 11 No clears the relief chosen from its list.
fn removes_relief(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    let code = kind.form_code();
    if !edits
        .keys()
        .any(|key| key.starts_with(&format!("frm{code}:rdTreaty")))
    {
        return None;
    }
    let held = match before["tax_relief"].as_str()? {
        "international_tax_treaty" => "International Tax Treaty",
        "special_law" => "Special Law",
        _ => return None,
    };
    (after["tax_relief"] == "no").then(|| {
        serde_json::json!({
            "message": format!("Answering No at item 11 clears the relief chosen there ({held}). Continue?"),
            "done": "Cleared item 11's relief",
            "rows": 1,
        })
    })
}

/// 2552 item 11: unticking its answer keeps it (the draft has no
/// unanswered value to go back to once answered; the official radios never
/// untick).
pub(super) fn untick_keeps_the_answer(kind: AgentFormKind, key: &str) -> bool {
    matches!(cell(kind, key), Some(Cell::Transaction2552(_)))
}

/// 1707 item 8 No clears what 8A held.
fn removes_1707(before: &Value, after: &Value, edits: &Map<String, Value>) -> Option<Value> {
    if !edits.contains_key("frm1707:rdoPg1I8TaxReliefNo") {
        return None;
    }
    let held = before["tax_relief"]
        .as_str()
        .filter(|t| !t.trim().is_empty())?;
    after["tax_relief"].is_null().then(|| {
        serde_json::json!({
            "message": format!("Answering No at item 8 clears item 8A (\"{held}\"). Continue?"),
            "done": "Cleared item 8A",
            "rows": 1,
        })
    })
}

/// When `edits` turned `before` into `after` by removing what the filer
/// typed, what the page asks first (`message`), what the Undo toast says
/// (`done`) and how many rows went; `None` when nothing typed went.
pub(super) fn removes(
    kind: AgentFormKind,
    before: &Value,
    after: &Value,
    edits: &Map<String, Value>,
) -> Option<Value> {
    if kind.form_code() == "1707" {
        return removes_1707(before, after, edits);
    }
    if matches!(kind.form_code(), "1606" | "1706") {
        return removes_relief(kind, before, after, edits);
    }
    if kind.form_code() != "2552"
        || !edits
            .keys()
            .any(|key| key.starts_with("frm2552:rdoPg1I11Transaction"))
    {
        return None;
    }
    // Item 11 picks another schedule: `set_transaction` clears every row
    // of the old one, so each row typed before goes, whatever the batch
    // types under the new schedule.
    if before["transaction"] == after["transaction"] {
        return None;
    }
    // A rate the filer types (Schedules 2 and 3) counts; Schedule 1's fixed
    // rate and every row's tax due are the form's.
    let typed_rate = before["transaction"] != "local_stock_exchange";
    let rows = before["schedule"].as_array().map_or(0, |rows| {
        rows.iter()
            .filter(|row| {
                row.as_object().is_some_and(|fields| {
                    fields.iter().any(|(field, value)| match value {
                        Value::String(text) => !text.trim().is_empty(),
                        Value::Number(n) => {
                            field != "tax_due"
                                && (field != "tax_rate" || typed_rate)
                                && n.as_f64().is_some_and(|n| n != 0.0)
                        }
                        _ => false,
                    })
                })
            })
            .count()
    });
    if rows == 0 {
        return None;
    }
    let schedule = match before["transaction"].as_str() {
        Some("local_stock_exchange") => "Schedule 1",
        Some("primary_offering") => "Schedule 2",
        _ => "Schedule 3",
    };
    let plural = if rows == 1 { "" } else { "s" };
    Some(serde_json::json!({
        "message": format!(
            "Changing item 11 removes the {rows} Part V {schedule} row{plural} you typed. Continue?"
        ),
        "done": format!("Removed {rows} {schedule} row{plural}"),
        "rows": rows,
    }))
}
